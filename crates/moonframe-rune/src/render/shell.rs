//! Where a `ui.terminal` sits in the window; the shell is in `crate::terminal`.

use {
    super::{Painter, Place},
    crate::{
        Ui,
        style::looked,
        terminal::{self, PADDING},
    },
    moonframe_core::{Part, States, Terminal, TerminalBackend, TerminalEngine, Token},
    moonframe_mux::Preference,
    moonframe_terminal::Engine,
    rune::{ClipboardContent, Drag, El, TextStyle, Tx, el, text},
};

/// The terminal's type, as on GPUI: its size, and line height as a multiple.
const FONT_SIZE: f32 = 13.;
const LINE: f32 = 1.3;
/// Characters measured to find a cell's width.
const PROBE: usize = 64;
/// The least room a terminal asks for.
const MIN_HEIGHT: f32 = 80.;
/// The selection over the text: the accent, light enough to read through.
const SELECTION_ALPHA: f32 = 0.3;
/// A monospaced advance as a fraction of the size, until the probe is laid out.
const FIRST_ADVANCE: f32 = 0.6;

/// A cell's size in points.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct Metrics {
    pub(crate) width: f32,
    pub(crate) height: f32,
}

impl<'w> Painter<'w> {
    /// The stylesheet's terminal type: `mono`'s size, weight, line height and
    /// family, else the platform's fixed-width font at the same defaults as
    /// on GPUI. A family Rune can't find falls back to a proportional one, so
    /// only one the stylesheet names is asked for.
    fn mono(&self) -> TextStyle {
        let mono = &self.ui.workspace.stylesheet().mono;
        let size = mono.size.unwrap_or(FONT_SIZE);
        let mut style = TextStyle::new(size, mono.weight.unwrap_or(400.));
        style.family = match &mono.family {
            Some(family) => rune::Family::Named(self.ui.terminals.family(family)),
            None => rune::Family::SystemMono,
        };
        style.line_height = size * mono.line_height.unwrap_or(LINE);
        style
    }

    pub(super) fn terminal(&self, node: &'w Terminal, place: &Place<'w>) -> El {
        let look = self.look(Part::Terminal, &node.look, States::default(), place);
        let key = format!("{}/{}", place.owner, node.id);
        let Some(wake) = self.ui.waker.clone() else {
            return self.not_here_named("ui.terminal", place);
        };
        let preference = match node.backend {
            TerminalBackend::Auto => Preference::Auto,
            TerminalBackend::HerdrCore => Preference::HerdrCore,
            TerminalBackend::Tmux => Preference::Tmux,
            TerminalBackend::Pty => Preference::Pty,
        };
        // A build without Ghostty's emulator (it needs Zig) runs Alacritty's.
        let engine = match node.engine {
            TerminalEngine::Ghostty if Engine::Ghostty.available() => Engine::Ghostty,
            TerminalEngine::Alacritty | TerminalEngine::Ghostty => Engine::Alacritty,
        };
        let how = (node.cwd.as_deref(), preference, engine);
        self.ui.terminals.open(&key, place.owner, how, wake);
        self.ui.terminals.shown.borrow_mut().push(key.clone());
        if let Some(view) = self.ui.terminals.views.borrow_mut().get_mut(&key) {
            view.exit.clone_from(&node.exit);
            view.shell.clone_from(&node.shell);
        }
        let foreground = look
            .base
            .color
            .as_ref()
            .map_or(self.palette.token(Token::Foreground), |color| {
                self.palette.color(color)
            });
        let background = look
            .base
            .background
            .as_ref()
            .map_or(self.palette.token(Token::Background), |color| {
                self.palette.color(color)
            });
        let style = self.mono();
        let metrics = Metrics {
            width: self
                .ui
                .cell_width
                .get()
                .unwrap_or(style.size * FIRST_ADVANCE),
            height: style.line_height,
        };
        self.ui.terminals.refit(&key, metrics);
        // Keyed by the terminal, not its place: two tabs draw theirs in the
        // same box, and one taking over has to be laid out, and so fitted.
        let element = el()
            .key(format!("terminal:{key}"))
            .role("terminal")
            .grow(1.)
            .min_w(0.)
            .min_h(MIN_HEIGHT)
            .focusable(true)
            .no_focus_ring()
            // A screen is not animated: rows fading and gliding between
            // frames leave ghosts of what the shell has already changed.
            .instant_changes()
            .fill(background)
            .clip_rounded(0.);
        // Invisible: its width over its length is a cell's, whatever the font.
        let probe = text("M".repeat(PROBE))
            .font(style)
            .key(format!("{key}-probe"))
            .absolute(0., 0.)
            .opacity(0.)
            .on_layout(move |ui: &mut Ui, rect: [f32; 4]| {
                ui.cell_width.set(Some(rect[2] / PROBE as f32));
            });
        let element = element.child(probe);
        let element = match self.ui.terminals.screen(&key) {
            Ok(screen) => {
                let selected = self.ui.terminals.selection(&key);
                let tint = self.palette.token(Token::Accent).alpha(SELECTION_ALPHA);
                let note = (screen.exited || screen.disconnected).then(|| {
                    let says = if screen.disconnected {
                        "[connection lost; press Enter to reconnect]"
                    } else if self.ui.terminals.exit_handled(&key) {
                        "[the shell exited]"
                    } else {
                        "[the shell exited; press Enter to start another]"
                    };
                    text(says).font(style).color(foreground.alpha(0.6)).pin(
                        Some(PADDING),
                        None,
                        None,
                        Some(PADDING),
                    )
                });
                element
                    .children(terminal::grid(
                        &screen,
                        metrics,
                        (foreground, style),
                        &key,
                        (selected, tint),
                    ))
                    .children(note)
            },
            Err(problem) => element.child(
                text(problem)
                    .font(style)
                    .color(self.palette.token(Token::Danger))
                    .absolute(PADDING, PADDING),
            ),
        };
        let (fit, typed, chosen, wheel) = (key.clone(), key.clone(), key.clone(), key);
        let element = element
            .on_layout(move |ui: &mut Ui, rect: [f32; 4]| {
                ui.terminals.laid_out(&fit, (rect[2], rect[3]), metrics);
            })
            .on_key(move |ui: &mut Ui, event: &rune::KeyEvent, tx: &mut Tx| {
                ui.terminal_key(&typed, event, tx)
            })
            .on_drag(move |ui: &mut Ui, drag: &mut Drag, tx: &mut Tx| {
                let phase = (drag.phase, drag.clicks);
                let text = ui
                    .terminals
                    .select(&chosen, (drag.x, drag.y), phase, metrics);
                if let Some(text) = text {
                    tx.copy_rich(ClipboardContent::new(text));
                }
            })
            .on_wheel(move |ui: &mut Ui, event: rune::WheelEvent, _: &mut Tx| {
                ui.terminals.scroll(&wheel, event.dy);
                true
            });
        looked(element, &look, None, self.palette)
    }
}
