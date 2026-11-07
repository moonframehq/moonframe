//! The user's login shell on a local pseudo-terminal: the backend that is
//! always there. The shell ends with the app.

use {
    crate::{Backend, Open, Opened, Pane, Size},
    portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system},
    std::{
        io::{Read, Write},
        path::PathBuf,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
            mpsc::{self, Receiver},
        },
        thread,
    },
    sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind},
};

/// How much output is read at once.
const CHUNK: usize = 64 * 1024;

/// Shells on local pseudo-terminals.
#[derive(Clone, Copy, Debug, Default)]
pub struct Pty;

impl Backend for Pty {
    fn name(&self) -> &'static str {
        "pty"
    }

    fn open(&self, open: &Open) -> Result<Opened, String> {
        let pair = native_pty_system()
            .openpty(pty_size(open.size))
            .map_err(|error| format!("cannot open a terminal: {error}"))?;
        let mut command = CommandBuilder::new_default_prog();
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        if let Some(cwd) = open.cwd.as_ref().filter(|cwd| cwd.is_dir()) {
            command.cwd(cwd);
        } else if let Some(home) = std::env::var_os("HOME") {
            command.cwd(home);
        }
        let mut child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| format!("cannot start the shell: {error}"))?;
        let killer = Mutex::new(child.clone_killer());
        let exited = Arc::new(AtomicBool::new(false));
        let pid = child.process_id();
        drop(pair.slave);
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| format!("cannot read the terminal: {error}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| format!("cannot write to the terminal: {error}"))?;
        let master = Arc::new(Mutex::new(Some(pair.master)));
        let closing = master.clone();
        let (finished, status) = mpsc::channel();
        thread::Builder::new()
            .name("terminal-wait".into())
            .spawn(move || {
                let _ = finished.send(child.wait().is_ok());
                // ConPTY keeps output open after the child exits. Closing it
                // can itself wait for output to drain, so the reader must run
                // independently, and resize must not hold up that reader.
                let master = closing.lock().ok().and_then(|mut master| master.take());
                drop(master);
            })
            .map_err(|error| {
                if let Ok(mut killer) = killer.lock() {
                    let _ = killer.kill();
                }
                format!("cannot wait for the shell: {error}")
            })?;
        Ok(Opened {
            pane: Box::new(PtyPane {
                writer: Mutex::new(writer),
                master,
                killer,
                pid,
                exited: exited.clone(),
            }),
            output: Box::new(Reading {
                reader,
                status,
                exited,
                buffer: vec![0; CHUNK],
                done: false,
            }),
        })
    }
}

struct PtyPane {
    writer: Mutex<Box<dyn Write + Send>>,
    master: Arc<Mutex<Option<Box<dyn MasterPty + Send>>>>,
    /// Ends the shell when the pane goes: the reader keeps the terminal
    /// open, so closing it alone would not.
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    /// The shell's process, to read its folder.
    pid: Option<u32>,
    exited: Arc<AtomicBool>,
}

impl Drop for PtyPane {
    fn drop(&mut self) {
        if let Ok(killer) = self.killer.get_mut() {
            let _ = killer.kill();
        }
    }
}

impl Pane for PtyPane {
    fn exited(&self) -> bool {
        self.exited.load(Ordering::Acquire)
    }

    fn write(&self, bytes: &[u8]) {
        if let Ok(mut writer) = self.writer.lock() {
            let _ = writer.write_all(bytes).and_then(|()| writer.flush());
        }
    }

    fn resize(&self, size: Size) {
        if let Ok(master) = self.master.lock()
            && let Some(master) = master.as_ref()
        {
            let _ = master.resize(pty_size(size));
        }
    }

    /// The shell's folder, as the system tells it: a program it runs
    /// mostly starts where the shell is.
    fn cwd(&self) -> Option<PathBuf> {
        let pid = Pid::from_u32(self.pid?);
        let mut system = System::new().ok()?;
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[pid]),
            false,
            ProcessRefreshKind::nothing().with_cwd(UpdateKind::Always),
        );
        system.process(pid)?.cwd().map(PathBuf::from)
    }
}

/// Drains the shell's output before publishing its independently reaped exit.
struct Reading {
    reader: Box<dyn Read + Send>,
    status: Receiver<bool>,
    exited: Arc<AtomicBool>,
    buffer: Vec<u8>,
    done: bool,
}

impl Iterator for Reading {
    type Item = Vec<u8>;

    fn next(&mut self) -> Option<Vec<u8>> {
        if self.done {
            return None;
        }
        match self.reader.read(&mut self.buffer) {
            Ok(0) | Err(_) => {
                self.done = true;
                self.exited
                    .store(self.status.recv().unwrap_or(false), Ordering::Release);
                None
            },
            Ok(read) => Some(self.buffer[..read].to_vec()),
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
