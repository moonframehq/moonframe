//! A terminal's program, whatever [`Backend`](crate::Backend) runs it, with
//! an emulator reading it.
//!
//! Three threads per session: one reads the pane's output, one runs the
//! emulator (it owns it: libghostty's types cannot change threads), and the
//! window's, which reads the latest [`Screen`] and writes keys straight to
//! the pane. The emulator thread drains every chunk waiting before it
//! publishes, so a burst of output costs one snapshot.

use {
    crate::{Ansi, Engine, Opened, Pane, Screen, Size},
    std::{
        sync::{
            Arc, Mutex,
            mpsc::{self, Receiver, Sender},
        },
        thread,
    },
};

pub struct Session {
    engine: Engine,
    commands: Sender<Command>,
    pane: Arc<dyn Pane>,
    screen: Arc<Mutex<Screen>>,
    size: Mutex<Size>,
}

/// What the emulator thread is asked to do.
enum Command {
    Output(Vec<u8>),
    Resize(Size),
    Scroll(i32),
    Palette(Option<Ansi>),
    Exited,
}

impl Session {
    /// Emulates `opened`, which is `size` now, on `engine`. `wake` is called,
    /// from another thread, whenever there is a new screen to paint.
    pub fn start(
        opened: Opened,
        engine: Engine,
        size: Size,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> Result<Self, String> {
        if !engine.available() {
            return Err(format!(
                "this build has no `{}` terminal; enable the `{}` feature",
                engine.name(),
                engine.name()
            ));
        }
        let Opened { pane, output } = opened;
        let pane: Arc<dyn Pane> = Arc::from(pane);
        let (commands, received) = mpsc::channel();
        let screen = Arc::new(Mutex::new(Screen::default()));

        let sender = commands.clone();
        thread::Builder::new()
            .name("terminal-read".into())
            .spawn(move || {
                for chunk in output {
                    if sender.send(Command::Output(chunk)).is_err() {
                        return;
                    }
                }
                let _ = sender.send(Command::Exited);
            })
            .map_err(|error| format!("cannot start a thread: {error}"))?;

        let emulate = Emulate {
            engine,
            size,
            received,
            // A backend that answers queries itself must not hear them twice.
            answers: (!pane.answers_queries()).then(|| pane.clone()),
            screen: screen.clone(),
            wake,
        };
        thread::Builder::new()
            .name(format!("terminal-{}", engine.name()))
            .spawn(move || emulate.run())
            .map_err(|error| format!("cannot start a thread: {error}"))?;

        Ok(Self {
            engine,
            commands,
            pane,
            screen,
            size: Mutex::new(size),
        })
    }

    pub fn engine(&self) -> Engine {
        self.engine
    }

    /// The latest snapshot.
    pub fn screen(&self) -> Screen {
        self.screen
            .lock()
            .map(|screen| screen.clone())
            .unwrap_or_default()
    }

    /// Sends what the user typed or pasted to the program.
    pub fn write(&self, bytes: &[u8]) {
        self.pane.write(bytes);
    }

    /// Fits the program and the emulator to `size`, if it changed.
    pub fn resize(&self, size: Size) {
        let Ok(mut current) = self.size.lock() else {
            return;
        };
        if *current == size || size.cols == 0 || size.rows == 0 {
            return;
        }
        *current = size;
        self.pane.resize(size);
        let _ = self.commands.send(Command::Resize(size));
    }

    /// Moves through the scrollback: negative goes up.
    pub fn scroll(&self, lines: i32) {
        let _ = self.commands.send(Command::Scroll(lines));
    }

    /// Paints the 16 named colors as the theme's, or xterm's when `None`.
    pub fn set_palette(&self, palette: Option<Ansi>) {
        let _ = self.commands.send(Command::Palette(palette));
    }
}

/// The emulator thread's state.
struct Emulate {
    engine: Engine,
    size: Size,
    received: Receiver<Command>,
    /// Where the emulator's answers to queries go, unless the backend
    /// answers them itself.
    answers: Option<Arc<dyn Pane>>,
    screen: Arc<Mutex<Screen>>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl Emulate {
    fn run(self) {
        let Some(mut emulator) = self.engine.emulator(self.size) else {
            return;
        };
        let mut exited = false;
        while let Ok(first) = self.received.recv() {
            for command in std::iter::once(first).chain(self.received.try_iter()) {
                match command {
                    Command::Output(bytes) => emulator.feed(&bytes),
                    Command::Resize(size) => emulator.resize(size),
                    Command::Scroll(lines) => emulator.scroll(lines),
                    Command::Palette(palette) => emulator.set_palette(palette),
                    Command::Exited => exited = true,
                }
            }
            let replies = emulator.replies();
            if !replies.is_empty()
                && let Some(pane) = &self.answers
            {
                pane.write(&replies);
            }
            let mut screen = emulator.screen();
            screen.exited = exited;
            if let Ok(mut shared) = self.screen.lock() {
                *shared = screen;
            }
            (self.wake)();
        }
    }
}
