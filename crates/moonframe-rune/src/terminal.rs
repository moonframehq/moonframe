//! `ui.terminal`: a shell in a pseudo-terminal, read by the same emulators the
//! GPUI renderer uses. Each row of the latest screen is one rich text in runs
//! of one style, and keys, pastes and the wheel go back to the shell. Dragging
//! selects (a double click a word, a triple click a line), copied as the
//! button is released and again with ⌘C. Shells are kept by their mod and
//! `id`, so they outlive reloads and hidden panes.

#[path = "../../moonframe/src/terminal/selection.rs"]
mod selection;

pub(crate) use selection::{Selection, Spot};

use {
    crate::{Ui, render::Metrics},
    moonframe_mux::{Backends, Preference},
    moonframe_terminal::{
        Cell, Color, Engine, Key as ShellKey, Open, Screen, Session, Size, encode, paste,
    },
    rune::{
        ClipboardContent, Color as RuneColor, El, Key, KeyEvent, Phase, Span, Tx, color::hex, el,
        rich,
    },
    std::{
        cell::RefCell,
        collections::HashMap,
        path::PathBuf,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    },
};

/// Shells one mod can have, and the whole app, as on GPUI: each is a process,
/// and a mod drawing ids in a loop must not start them without end.
const PER_MOD: usize = 8;
const TOTAL: usize = 32;
/// Space between the grid and the element's edge.
pub(crate) const PADDING: f32 = 8.;
/// Points of wheel travel per line of scrollback.
const WHEEL_LINE: f32 = 16.;

/// A shell and what the window keeps for it.
pub(crate) struct View {
    session: Result<Session, String>,
    /// What it was opened with, to open it again after it exits.
    open: Open,
    preference: Preference,
    engine: Engine,
    /// The text chosen with the pointer, in the visible screen's cells.
    pub(crate) selection: Option<Selection>,
    /// What the element sends as the shell exits, once.
    pub(crate) exit: Option<moonframe_core::Data>,
    pub(crate) owner: String,
    pub(crate) told: bool,
}

/// Every shell, and whether any has a new screen to draw.
pub(crate) struct Terminals {
    pub(crate) views: RefCell<HashMap<String, View>>,
    pub(crate) dirty: Arc<AtomicBool>,
    /// herdr-core and tmux, where they are installed; a pty otherwise.
    pub(crate) backends: Arc<Backends>,
    /// The terminal last clicked or typed in: where a paste goes.
    pub(crate) current: RefCell<Option<String>>,
    /// The terminals this frame drew, in order.
    pub(crate) shown: RefCell<Vec<String>>,
    /// Each terminal's box, as last laid out.
    boxes: RefCell<HashMap<String, (f32, f32)>>,
    /// The stylesheet's terminal font, kept as Rune keeps its families.
    family: RefCell<Option<(String, &'static str)>>,
}

impl Default for Terminals {
    fn default() -> Self {
        Self {
            views: RefCell::default(),
            dirty: Arc::default(),
            backends: Arc::new(Backends::new()),
            current: RefCell::default(),
            shown: RefCell::default(),
            boxes: RefCell::default(),
            family: RefCell::default(),
        }
    }
}

fn expand(cwd: &str) -> PathBuf {
    match (cwd.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
        _ if cwd == "~" => std::env::var_os("HOME").map_or_else(|| cwd.into(), PathBuf::from),
        _ => cwd.into(),
    }
}

impl Terminals {
    /// Starts the shell kept as `key`, unless it runs or is one too many.
    pub(crate) fn open(
        &self,
        key: &str,
        owner: &str,
        (cwd, preference, engine): (Option<&str>, Preference, Engine),
        wake: rune::Waker,
    ) {
        let mut views = self.views.borrow_mut();
        if views.contains_key(key) {
            return;
        }
        let mine = views.values().filter(|view| view.owner == owner).count();
        let open = Open {
            key: key.to_owned(),
            cwd: cwd
                .map(expand)
                .or_else(|| moonframe_providers::launch_dir().map(ToOwned::to_owned)),
            size: Size::default(),
        };
        let session = if mine >= PER_MOD {
            Err(format!("`{owner}` already has {PER_MOD} terminals"))
        } else if views.len() >= TOTAL {
            Err(format!("the app already has {TOTAL} terminals"))
        } else {
            self.spawn(&open, (preference, engine), wake)
        };
        views.insert(key.to_owned(), View {
            session,
            open,
            preference,
            engine,
            selection: None,
            exit: None,
            owner: owner.to_owned(),
            told: false,
        });
    }

    fn spawn(
        &self,
        open: &Open,
        (preference, engine): (Preference, Engine),
        wake: rune::Waker,
    ) -> Result<Session, String> {
        let dirty = Arc::clone(&self.dirty);
        // Tests run as the user: what herdr-core and tmux keep would outlive
        // them, among the user's own terminals.
        let preference = if cfg!(test) {
            Preference::Pty
        } else {
            preference
        };
        self.backends
            .open(preference, open)
            .and_then(|(opened, _)| {
                Session::start(
                    opened,
                    engine,
                    open.size,
                    Arc::new(move || {
                        dirty.store(true, Ordering::Release);
                        wake();
                    }),
                )
            })
    }

    /// Starts the shell at `key` again, after it exited.
    fn restart(&self, key: &str, wake: rune::Waker) {
        let again = self
            .views
            .borrow()
            .get(key)
            .map(|view| (view.open.clone(), (view.preference, view.engine)));
        if let Some((open, how)) = again {
            let session = self.spawn(&open, how, wake);
            if let Some(view) = self.views.borrow_mut().get_mut(key) {
                view.session = session;
                view.selection = None;
                view.told = false;
            }
        }
    }

    /// Whether the mod takes care of `key`'s exit: it asked to hear of it.
    pub(crate) fn exit_handled(&self, key: &str) -> bool {
        self.views
            .borrow()
            .get(key)
            .is_some_and(|view| view.exit.is_some())
    }

    /// The selection of `key`, as last made.
    pub(crate) fn selection(&self, key: &str) -> Option<Selection> {
        self.views.borrow().get(key)?.selection
    }

    /// A press, or the drag that follows it, at the point `(x, y)` of `key`.
    /// `Phase::Up` keeps what was chosen: it is its text, if there is any.
    pub(crate) fn select(
        &self,
        key: &str,
        (x, y): (f32, f32),
        drag: (Phase, u32),
        metrics: Metrics,
    ) -> Option<String> {
        let screen = self.screen(key).ok()?;
        let cell = |at: f32, size: f32, count: u16| {
            let index = ((at - PADDING) / size).floor().max(0.) as u16;
            index.min(count.saturating_sub(1))
        };
        let spot = Spot {
            row: cell(y, metrics.height, screen.rows),
            column: cell(x, metrics.width, screen.cols),
        };
        *self.current.borrow_mut() = Some(key.to_owned());
        let mut views = self.views.borrow_mut();
        let view = views.get_mut(key)?;
        match drag {
            (Phase::Down, clicks) => {
                view.selection = Some(Selection::press(&screen, spot, clicks as usize));
                None
            },
            (Phase::Move, _) => {
                if let Some(selection) = &mut view.selection {
                    selection.extend(&screen, spot);
                }
                None
            },
            (Phase::Up, _) => {
                let selection = view.selection.as_mut()?;
                selection.extend(&screen, spot);
                if selection.is_empty() {
                    view.selection = None;
                    return None;
                }
                Some(selection.text(&screen))
            },
        }
    }

    /// The latest screen of `key`, or why it has none.
    pub(crate) fn screen(&self, key: &str) -> Result<Screen, String> {
        match self.views.borrow().get(key).map(|view| &view.session) {
            Some(Ok(session)) => Ok(session.screen()),
            Some(Err(problem)) => Err(problem.clone()),
            None => Err("no terminal".to_owned()),
        }
    }

    fn with(&self, key: &str, run: impl FnOnce(&Session)) {
        if let Some(View {
            session: Ok(session),
            ..
        }) = self.views.borrow().get(key)
        {
            run(session);
        }
    }

    /// `name` as Rune takes a family: leaked, once for each name in use.
    pub(crate) fn family(&self, name: &str) -> &'static str {
        let mut family = self.family.borrow_mut();
        match &*family {
            Some((kept, leaked)) if kept == name => leaked,
            _ => {
                let leaked: &'static str = Box::leak(name.to_owned().into_boxed_str());
                *family = Some((name.to_owned(), leaked));
                leaked
            },
        }
    }

    /// The terminal keys go to: the one last used, while it shows, else the
    /// first on screen.
    pub(crate) fn in_use(&self) -> Option<String> {
        let shown = self.shown.borrow();
        let current = self.current.borrow();
        current
            .as_ref()
            .filter(|key| shown.contains(key))
            .or(shown.first())
            .cloned()
    }

    /// Notes the box `key` was laid out in, and fits the shell to it.
    pub(crate) fn laid_out(&self, key: &str, size: (f32, f32), metrics: Metrics) {
        self.boxes.borrow_mut().insert(key.to_owned(), size);
        self.fit(key, size, metrics);
    }

    /// Fits `key` to its box again, as the view draws: the cell's real width
    /// comes after the first layout, which a box that stays put won't repeat.
    pub(crate) fn refit(&self, key: &str, metrics: Metrics) {
        let size = self.boxes.borrow().get(key).copied();
        if let Some(size) = size {
            self.fit(key, size, metrics);
        }
    }

    /// Fits the shell to a box of `size` points.
    fn fit(&self, key: &str, (width, height): (f32, f32), metrics: Metrics) {
        let cols = ((width - 2. * PADDING) / metrics.width).floor().max(1.) as u16;
        let rows = ((height - 2. * PADDING) / metrics.height).floor().max(1.) as u16;
        self.with(key, |session| {
            session.resize(Size {
                cols,
                rows,
                cell_width: metrics.width.ceil() as u16,
                cell_height: metrics.height.ceil() as u16,
            });
        });
    }

    /// The name [`encode`] knows `key` by, and the text it types.
    pub(crate) fn key(&self, key: &str, event: &KeyEvent) -> bool {
        *self.current.borrow_mut() = Some(key.to_owned());
        if let Some(view) = self.views.borrow_mut().get_mut(key)
            && !matches!(event.key, Key::Command(_))
        {
            view.selection = None;
        }
        // Control held may arrive as the control character itself.
        if let Key::Text(text) = &event.key
            && !text.is_empty()
            && text.chars().all(char::is_control)
        {
            let mut sent = false;
            self.with(key, |session| {
                session.write(text.as_bytes());
                sent = true;
            });
            return sent;
        }
        let name;
        let (named, text): (&str, Option<&str>) = match &event.key {
            Key::Text(text) => {
                name = text.to_lowercase();
                (name.as_str(), Some(text.as_str()))
            },
            // Control held with a letter arrives as a command off macOS.
            Key::Command(letter) if event.mods.ctrl => {
                name = letter.to_string();
                (name.as_str(), None)
            },
            Key::Command(_) => return false,
            Key::Enter => ("enter", None),
            Key::Tab => ("tab", None),
            Key::Backspace => ("backspace", None),
            Key::Escape => ("escape", None),
            Key::Up => ("up", None),
            Key::Down => ("down", None),
            Key::Left => ("left", None),
            Key::Right => ("right", None),
            Key::Home => ("home", None),
            Key::End => ("end", None),
            Key::PageUp => ("pageup", None),
            Key::PageDown => ("pagedown", None),
            Key::Delete => ("delete", None),
            Key::Insert => ("insert", None),
            Key::Function(number) => {
                name = format!("f{number}");
                (name.as_str(), None)
            },
            _ => return false,
        };
        let mut sent = false;
        self.with(key, |session| {
            let shell = ShellKey {
                name: named,
                text,
                ctrl: event.mods.ctrl,
                alt: event.mods.alt,
                shift: event.mods.shift,
            };
            if let Some(bytes) = encode(shell, session.screen().app_cursor) {
                session.write(&bytes);
                sent = true;
            }
        });
        sent
    }

    /// Puts `text` in the terminal last used, if there is one.
    pub(crate) fn paste(&self, text: &str) -> bool {
        let Some(key) = self.current.borrow().clone() else {
            return false;
        };
        self.with(&key, |session| {
            session.write(&paste(text, session.screen().bracketed_paste));
        });
        true
    }

    /// Moves through the scrollback by `points` of the wheel.
    pub(crate) fn scroll(&self, key: &str, points: f32) {
        let lines = (points / WHEEL_LINE).round() as i32;
        if lines != 0 {
            // The text moves from under the selection.
            if let Some(view) = self.views.borrow_mut().get_mut(key) {
                view.selection = None;
            }
            self.with(key, |session| session.scroll(-lines));
        }
    }
}

fn rgb(color: Color, default: RuneColor) -> RuneColor {
    match color {
        Color::Default => default,
        Color::Rgb(red, green, blue) => {
            hex(&format!("#{red:02x}{green:02x}{blue:02x}")).unwrap_or(default)
        },
    }
}

/// Cells that share a look, as one span.
fn same_run(a: &Cell, b: &Cell) -> bool {
    (a.fg, a.bg, a.bold, a.italic, a.underline) == (b.fg, b.bg, b.bold, b.italic, b.underline)
}

/// One row of cells as text and spans. A row keeps its trailing blanks only
/// where they are painted.
pub(crate) fn row_text(cells: &[Cell], foreground: RuneColor) -> (String, Vec<Span>) {
    let painted = cells
        .iter()
        .rposition(|cell| cell.ch != ' ' || cell.bg != Color::Default)
        .map_or(0, |last| last + 1);
    let (mut line, mut spans) = (String::new(), Vec::new());
    let mut run: Option<(usize, Cell)> = None;
    let finish = |run: &Option<(usize, Cell)>, end: usize, spans: &mut Vec<Span>| {
        let Some((start, cell)) = run else {
            return;
        };
        let color = rgb(cell.fg, foreground);
        let mut span = Span::new(*start, end);
        span.color = Some(color);
        span.italic = cell.italic;
        if cell.bold {
            span.weight = Some(700.);
        }
        if cell.bg != Color::Default {
            span.bg = Some(rgb(cell.bg, foreground));
        }
        if cell.underline {
            span.underline = Some(color);
        }
        spans.push(span);
    };
    for cell in &cells[..painted] {
        if cell.spacer || cell.wrap_spacer {
            continue;
        }
        if !run.as_ref().is_some_and(|(_, first)| same_run(first, cell)) {
            finish(&run, line.len(), &mut spans);
            run = Some((line.len(), *cell));
        }
        // A cell is one column, but text would take a tab to its next stop.
        line.push(if cell.ch.is_control() {
            ' '
        } else {
            cell.ch
        });
    }
    finish(&run, line.len(), &mut spans);
    (line, spans)
}

/// The grid: each row where the cell metrics put it, and the cursor over it.
pub(crate) fn grid(
    screen: &Screen,
    metrics: Metrics,
    ink: (RuneColor, rune::TextStyle),
    key: &str,
    (selected, tint): (Option<Selection>, RuneColor),
) -> Vec<El> {
    let (foreground, style) = ink;
    let mut rows = Vec::with_capacity(usize::from(screen.rows) + 1);
    for row in 0..screen.rows {
        let (line, spans) = row_text(screen.row(row), foreground);
        if line.is_empty() {
            continue;
        }
        let top = PADDING + f32::from(row) * metrics.height;
        rows.push(
            rich(line, spans)
                .font(style)
                .color(foreground)
                .key(format!("{key}-row{row}"))
                .absolute(PADDING, top)
                .w(f32::from(screen.cols) * metrics.width + metrics.width),
        );
    }
    if let Some(selected) = selected {
        for (row, columns) in selected.rows(screen) {
            rows.push(
                el().key(format!("{key}-selected{row}"))
                    .absolute(
                        PADDING + f32::from(columns.start) * metrics.width,
                        PADDING + f32::from(row) * metrics.height,
                    )
                    .size(
                        f32::from(columns.end - columns.start) * metrics.width,
                        metrics.height,
                    )
                    .fill(tint),
            );
        }
    }
    if let Some((col, row)) = screen.cursor {
        rows.push(
            el().key(format!("{key}-cursor"))
                .absolute(
                    PADDING + f32::from(col) * metrics.width,
                    PADDING + f32::from(row) * metrics.height,
                )
                .size(metrics.width, metrics.height)
                .fill(foreground.alpha(0.6)),
        );
    }
    rows
}

impl Ui {
    /// Tells mods of shells that exited: each once, if it asked to hear.
    pub(crate) fn tell_exits(&mut self) {
        let mut told = Vec::new();
        for view in self.terminals.views.get_mut().values_mut() {
            if !view.told
                && let Ok(session) = &view.session
                && session.exited()
                && let Some(msg) = view.exit.clone()
            {
                view.told = true;
                told.push((view.owner.clone(), msg));
            }
        }
        // A shell its mod has heard of goes once nothing draws it: a tab's is
        // replaced under a new id, and must not hold a slot for ever.
        let shown = self.terminals.shown.borrow();
        self.terminals
            .views
            .get_mut()
            .retain(|key, view| !view.told || shown.contains(key));
        let boxes = self.terminals.boxes.get_mut();
        let views = self.terminals.views.get_mut();
        boxes.retain(|key, _| views.contains_key(key));
        drop(shown);
        for (owner, msg) in told {
            let exited = match msg {
                moonframe_core::Data::Map(mut entries) => {
                    entries.insert("exited".to_owned(), true.into());
                    moonframe_core::Data::Map(entries)
                },
                other => other,
            };
            self.workspace.send(&owner, &owner, exited);
        }
    }

    /// Takes the fact that a shell drew something new.
    pub(crate) fn terminal_changed(&mut self) -> bool {
        self.terminals.dirty.swap(false, Ordering::AcqRel)
    }

    /// A key pressed in the terminal `key`, as Rune hands it over.
    pub(crate) fn terminal_key(&mut self, key: &str, event: &KeyEvent, tx: &mut Tx) -> bool {
        match event.key {
            // ⌘C copies a selection; without one the key is the app's.
            Key::Command('c') if !event.mods.ctrl => {
                let chosen = self
                    .terminals
                    .selection(key)
                    .and_then(|selected| Some(selected.text(&self.terminals.screen(key).ok()?)));
                return match chosen {
                    Some(text) => {
                        tx.copy_rich(ClipboardContent::new(text));
                        true
                    },
                    None => false,
                };
            },
            Key::Command('v') if !event.mods.ctrl => {
                *self.terminals.current.borrow_mut() = Some(key.to_owned());
                tx.request_paste();
                return true;
            },
            _ => {},
        }
        // A shell that ended takes no typing: Enter starts another, unless
        // its mod handles the exit, as a tab does by closing.
        if let Ok(screen) = self.terminals.screen(key)
            && (screen.exited || screen.disconnected)
        {
            let again = screen.disconnected || !self.terminals.exit_handled(key);
            if event.key == Key::Enter
                && again
                && let Some(wake) = self.waker.clone()
            {
                self.terminals.restart(key, wake);
            }
            return true;
        }
        self.terminals.key(key, event)
    }
}
