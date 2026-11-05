//! The microphone. A cpal stream can't leave the thread that built it, so
//! each take runs on a thread of its own, holding the stream until told to
//! finish, and shares what it hears through [`Shared`].

use {
    crate::audio::{self, Audio},
    cpal::{
        FromSample, SampleFormat, SizedSample, StreamConfig,
        traits::{DeviceTrait, HostTrait, StreamTrait},
    },
    std::{
        sync::{
            Arc, Mutex, MutexGuard, PoisonError,
            mpsc::{self, RecvTimeoutError},
        },
        time::Duration,
    },
};

/// How long the system may take to open the microphone.
const OPENING: Duration = Duration::from_secs(5);

/// Something to record from: the real microphone, or a test's.
pub trait Microphone: Send {
    /// Starts a take of at most `seconds`.
    fn open(&self, seconds: u32) -> Result<Box<dyn Take>, String>;
}

/// A recording under way. Dropping it stops it and throws it away.
pub trait Take: Send {
    /// How loud it is now, from 0 to 1.
    fn level(&self) -> f32;
    /// Why it can't go on, once the device failed.
    fn problem(&self) -> Option<String>;
    /// Stops it and hands over what it heard.
    fn finish(self: Box<Self>) -> Audio;
}

/// The default input device.
pub struct System;

#[derive(Default)]
struct Shared {
    audio: Audio,
    level: f32,
    problem: Option<String>,
    /// Samples kept at most, so a forgotten take can't fill the memory.
    limit: usize,
}

fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

struct Running {
    shared: Arc<Mutex<Shared>>,
    /// Dropped to end the thread holding the stream.
    stop: Option<mpsc::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Microphone for System {
    fn open(&self, seconds: u32) -> Result<Box<dyn Take>, String> {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let (opened, opening) = mpsc::channel();
        let (stop, stopped) = mpsc::channel::<()>();
        let held = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("speech-microphone".into())
            .spawn(move || match stream(&held, seconds) {
                Ok(stream) => {
                    let _ = opened.send(Ok(()));
                    // Ends when the take drops its sender.
                    let _ = stopped.recv();
                    drop(stream);
                },
                Err(problem) => {
                    let _ = opened.send(Err(problem));
                },
            })
            .map_err(|error| format!("Could not start recording: {error}"))?;
        match opening.recv_timeout(OPENING) {
            Ok(Ok(())) => Ok(Box::new(Running {
                shared,
                stop: Some(stop),
                thread: Some(thread),
            })),
            Ok(Err(problem)) => Err(problem),
            Err(RecvTimeoutError::Timeout) => Err("The microphone did not open in time".into()),
            Err(RecvTimeoutError::Disconnected) => Err("The microphone stopped opening".into()),
        }
    }
}

impl Running {
    fn end(&mut self) {
        self.stop.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Take for Running {
    fn level(&self) -> f32 {
        lock(&self.shared).level
    }

    fn problem(&self) -> Option<String> {
        lock(&self.shared).problem.clone()
    }

    fn finish(mut self: Box<Self>) -> Audio {
        self.end();
        std::mem::take(&mut lock(&self.shared).audio)
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.end();
    }
}

fn stream(shared: &Arc<Mutex<Shared>>, seconds: u32) -> Result<cpal::Stream, String> {
    let device = cpal::default_host()
        .default_input_device()
        .ok_or("No microphone found")?;
    let supported = device
        .default_input_config()
        .map_err(|error| format!("Could not read the microphone's format: {error}"))?;
    let format = supported.sample_format();
    let config = supported.config();
    {
        let mut shared = lock(shared);
        shared.audio.rate = config.sample_rate;
        shared.limit = config.sample_rate as usize * seconds as usize;
    }
    let stream = match format {
        SampleFormat::F32 => build::<f32>(&device, &config, shared),
        SampleFormat::F64 => build::<f64>(&device, &config, shared),
        SampleFormat::I8 => build::<i8>(&device, &config, shared),
        SampleFormat::I16 => build::<i16>(&device, &config, shared),
        SampleFormat::I32 => build::<i32>(&device, &config, shared),
        SampleFormat::U8 => build::<u8>(&device, &config, shared),
        SampleFormat::U16 => build::<u16>(&device, &config, shared),
        SampleFormat::U32 => build::<u32>(&device, &config, shared),
        other => return Err(format!("The microphone's format {other} isn't supported")),
    }?;
    stream
        .play()
        .map_err(|error| format!("Could not start the microphone: {error}"))?;
    Ok(stream)
}

fn build<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    shared: &Arc<Mutex<Shared>>,
) -> Result<cpal::Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = usize::from(config.channels).max(1);
    let hearing = Arc::clone(shared);
    let failing = Arc::clone(shared);
    device
        .build_input_stream::<T, _, _>(
            *config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                let mono: Vec<f32> = data
                    .chunks(channels)
                    .map(|frame| {
                        frame.iter().map(|s| s.to_sample::<f32>()).sum::<f32>() / frame.len() as f32
                    })
                    .collect();
                let mut shared = lock(&hearing);
                shared.level = audio::level(&mono);
                let room = shared.limit.saturating_sub(shared.audio.samples.len());
                shared
                    .audio
                    .samples
                    .extend_from_slice(&mono[..room.min(mono.len())]);
            },
            move |error| {
                lock(&failing).problem = Some(format!("The microphone stopped: {error}"));
            },
            None,
        )
        .map_err(|error| format!("Could not open the microphone: {error}"))
}
