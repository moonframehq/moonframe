//! Apple's on-device recognizer, `SFSpeechRecognizer`, on a take's file.
//! Recognition never leaves the machine: a language without an on-device
//! model is reported as not ready rather than sent to Apple's servers.

// The framework's bindings are all `unsafe`.
#![allow(unsafe_code)]

use {
    super::Cancel,
    block2::RcBlock,
    objc2::{AllocAnyThread, rc::Retained},
    objc2_foundation::{NSBundle, NSError, NSLocale, NSOperationQueue, NSString, NSURL},
    objc2_speech::{
        SFSpeechRecognitionResult, SFSpeechRecognizer, SFSpeechRecognizerAuthorizationStatus,
        SFSpeechURLRecognitionRequest,
    },
    std::{
        path::Path,
        sync::mpsc::{self, RecvTimeoutError},
        time::{Duration, Instant},
    },
};

const USAGE_KEY: &str = "NSSpeechRecognitionUsageDescription";
const TIMEOUT: Duration = Duration::from_secs(120);
const POLL: Duration = Duration::from_millis(100);
/// How long the user may take to answer the system's question.
const ASKING: Duration = Duration::from_secs(120);

/// Asking for speech recognition aborts a process whose Info.plist doesn't
/// say why, which is any run outside a bundle (`just run`). So nothing may
/// touch the recognizer's authorization before this says yes.
fn declared() -> bool {
    let bundle = NSBundle::mainBundle();
    bundle
        .infoDictionary()
        .is_some_and(|info| info.objectForKey(&NSString::from_str(USAGE_KEY)).is_some())
}

fn recognizer(language: Option<&str>) -> Option<Retained<SFSpeechRecognizer>> {
    // SAFETY: plain constructors; a locale it can't serve gives `None`.
    unsafe {
        match language {
            Some(language) => {
                let locale = NSLocale::initWithLocaleIdentifier(
                    NSLocale::alloc(),
                    &NSString::from_str(language),
                );
                SFSpeechRecognizer::initWithLocale(SFSpeechRecognizer::alloc(), &locale)
            },
            None => SFSpeechRecognizer::init(SFSpeechRecognizer::alloc()),
        }
    }
}

pub(super) fn check(language: Option<&str>) -> Option<String> {
    if !declared() {
        return Some("Apple dictation needs the installed app, not a run from source".into());
    }
    // SAFETY: the usage string is declared, so reading the status is safe.
    let status = unsafe { SFSpeechRecognizer::authorizationStatus() };
    if status == SFSpeechRecognizerAuthorizationStatus::Denied
        || status == SFSpeechRecognizerAuthorizationStatus::Restricted
    {
        return Some("Allow speech recognition in System Settings > Privacy & Security".into());
    }
    let Some(recognizer) = recognizer(language) else {
        return Some("Apple dictation doesn't support this language".into());
    };
    // SAFETY: a plain getter on an object we hold.
    if unsafe { recognizer.supportsOnDeviceRecognition() } {
        None
    } else {
        Some("Turn on Dictation in System Settings > Keyboard to get the on-device model".into())
    }
}

/// Asks the user the first time; later answers are the system's memory.
fn authorize() -> Result<(), String> {
    // SAFETY: only reached after `declared`, by way of `transcribe`.
    let status = unsafe { SFSpeechRecognizer::authorizationStatus() };
    if status == SFSpeechRecognizerAuthorizationStatus::Authorized {
        return Ok(());
    }
    let (answered, answer) = mpsc::channel();
    let handler = RcBlock::new(move |status: SFSpeechRecognizerAuthorizationStatus| {
        let _ = answered.send(status);
    });
    // SAFETY: the block is retained by the call and may run on any thread;
    // the channel it holds is `Send`.
    unsafe { SFSpeechRecognizer::requestAuthorization(&handler) };
    match answer.recv_timeout(ASKING) {
        Ok(SFSpeechRecognizerAuthorizationStatus::Authorized) => Ok(()),
        Ok(_) => Err("Speech recognition isn't allowed".into()),
        Err(_) => Err("No answer about speech recognition".into()),
    }
}

pub(super) fn transcribe(
    file: &Path,
    language: Option<&str>,
    cancel: &Cancel,
) -> Result<String, String> {
    if !declared() {
        return Err("Apple dictation needs the installed app, not a run from source".into());
    }
    authorize()?;
    let recognizer = recognizer(language).ok_or("Apple dictation doesn't support this language")?;
    let url = NSURL::from_file_path(file).ok_or("The take has no usable path")?;
    let (finished, result) = mpsc::channel::<Result<String, String>>();
    let handler = RcBlock::new(
        move |result: *mut SFSpeechRecognitionResult, error: *mut NSError| {
            // SAFETY: the framework passes valid objects or null, for the
            // length of this call.
            let (result, error) = unsafe { (result.as_ref(), error.as_ref()) };
            if let Some(error) = error {
                let _ = finished.send(Err(error.localizedDescription().to_string()));
            } else if let Some(result) = result {
                // SAFETY: plain getters on a live result.
                if unsafe { result.isFinal() } {
                    let text = unsafe { result.bestTranscription().formattedString() };
                    let _ = finished.send(Ok(text.to_string()));
                }
            }
        },
    );
    // SAFETY: results go to a queue of their own, as the app's main thread
    // is GPUI's; the block and request are retained by the task.
    let task = unsafe {
        recognizer.setQueue(&NSOperationQueue::new());
        let request = SFSpeechURLRecognitionRequest::initWithURL(
            SFSpeechURLRecognitionRequest::alloc(),
            &url,
        );
        request.setRequiresOnDeviceRecognition(true);
        request.setShouldReportPartialResults(false);
        recognizer.recognitionTaskWithRequest_resultHandler(&request, &handler)
    };
    let started = Instant::now();
    loop {
        match result.recv_timeout(POLL) {
            Ok(result) => return result,
            Err(RecvTimeoutError::Timeout) if cancel.is_set() => {
                // SAFETY: cancelling a task we hold.
                unsafe { task.cancel() };
                return Err("Cancelled".into());
            },
            Err(RecvTimeoutError::Timeout) if started.elapsed() >= TIMEOUT => {
                // SAFETY: as above.
                unsafe { task.cancel() };
                return Err("Apple dictation took too long".into());
            },
            Err(RecvTimeoutError::Timeout) => {},
            Err(RecvTimeoutError::Disconnected) => {
                return Err("Apple dictation stopped without an answer".into());
            },
        }
    }
}
