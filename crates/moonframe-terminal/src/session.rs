//! A shell on a pseudo-terminal with an emulator reading it.
//!
//! Three threads per session: one reads the shell's output, one runs the
//! emulator (it owns it: libghostty's types cannot change threads), and the
//! window's, which reads the latest [`Screen`] and writes keys straight to
//! the shell. The emulator thread drains every chunk waiting before it
//! publishes, so a burst of output costs one snapshot.

use {
    crate::{Ansi, Engine, Screen, Size},
    portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system},
    std::{
        io::{Read, Write},
        path::PathBuf,
        sync::{
            Arc, Mutex,
            mpsc::{self, Receiver, Sender},
        },
        thread,
    },
};

/// How much output is read at once.
const CHUNK: usize = 64 * 1024;

/// Where and how to start a shell.
#[derive(Clone, Debug)]
pub struct Spawn {
    pub engine: Engine,
    /// The folder the shell starts in; the home folder when `None`.
    pub cwd: Option<PathBuf>,
    pub size: Size,
}

pub struct Session {
    engine: Engine,
    commands: Sender<Command>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Mutex<Box<dyn MasterPty + Send>>,
    screen: Arc<Mutex<Screen>>,
    size: Mutex<Size>,
    /// Ends the shell when the session goes: the reader keeps the terminal
    /// open, so closing it alone would not.
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Ok(killer) = self.killer.get_mut() {
            let _ = killer.kill();
        }
    }
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
    /// Starts the user's login shell. `wake` is called, from another thread,
    /// whenever there is a new screen to paint.
    pub fn spawn(spawn: Spawn, wake: Arc<dyn Fn() + Send + Sync>) -> Result<Self, String> {
        if !spawn.engine.available() {
            return Err(format!(
                "this build has no `{}` terminal; enable the `{}` feature",
                spawn.engine.name(),
                spawn.engine.name()
            ));
        }
        let pair = native_pty_system()
            .openpty(pty_size(spawn.size))
            .map_err(|error| format!("cannot open a terminal: {error}"))?;
        let mut command = CommandBuilder::new_default_prog();
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        if let Some(cwd) = spawn.cwd.filter(|cwd| cwd.is_dir()) {
            command.cwd(cwd);
        } else if let Some(home) = std::env::var_os("HOME") {
            command.cwd(home);
        }
        let mut child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| format!("cannot start the shell: {error}"))?;
        let killer = Mutex::new(child.clone_killer());
        drop(pair.slave);
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| format!("cannot read the terminal: {error}"))?;
        let writer: Arc<Mutex<Box<dyn Write + Send>>> =
            Arc::new(Mutex::new(pair.master.take_writer().map_err(|error| {
                format!("cannot write to the terminal: {error}")
            })?));

        let (commands, received) = mpsc::channel();
        let screen = Arc::new(Mutex::new(Screen::default()));

        let output = commands.clone();
        thread::Builder::new()
            .name("terminal-read".into())
            .spawn(move || {
                let mut buffer = vec![0; CHUNK];
                loop {
                    match reader.read(&mut buffer) {
                        Ok(0) | Err(_) => break,
                        Ok(read) => {
                            if output
                                .send(Command::Output(buffer[..read].to_vec()))
                                .is_err()
                            {
                                return;
                            }
                        },
                    }
                }
                // Reap the shell so it does not linger as a zombie.
                let _ = child.wait();
                let _ = output.send(Command::Exited);
            })
            .map_err(|error| format!("cannot start a thread: {error}"))?;

        let emulate = Emulate {
            engine: spawn.engine,
            size: spawn.size,
            received,
            writer: writer.clone(),
            screen: screen.clone(),
            wake,
        };
        thread::Builder::new()
            .name(format!("terminal-{}", spawn.engine.name()))
            .spawn(move || emulate.run())
            .map_err(|error| format!("cannot start a thread: {error}"))?;

        Ok(Self {
            engine: spawn.engine,
            commands,
            writer,
            master: Mutex::new(pair.master),
            screen,
            size: Mutex::new(spawn.size),
            killer,
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

    /// Sends what the user typed or pasted to the shell.
    pub fn write(&self, bytes: &[u8]) {
        if let Ok(mut writer) = self.writer.lock() {
            let _ = writer.write_all(bytes).and_then(|()| writer.flush());
        }
    }

    /// Fits the shell and the emulator to `size`, if it changed.
    pub fn resize(&self, size: Size) {
        let Ok(mut current) = self.size.lock() else {
            return;
        };
        if *current == size || size.cols == 0 || size.rows == 0 {
            return;
        }
        *current = size;
        if let Ok(master) = self.master.lock() {
            let _ = master.resize(pty_size(size));
        }
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
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
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
                && let Ok(mut writer) = self.writer.lock()
            {
                let _ = writer.write_all(&replies).and_then(|()| writer.flush());
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

fn pty_size(size: Size) -> PtySize {
    PtySize {
        rows: size.rows,
        cols: size.cols,
        pixel_width: size.cols.saturating_mul(size.cell_width),
        pixel_height: size.rows.saturating_mul(size.cell_height),
    }
}
