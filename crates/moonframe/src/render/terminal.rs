//! A `ui.terminal`: the latest screen of its shell, painted cell by cell in
//! runs of one style, with the cursor; keys, pastes and scrolling go back
//! to the shell. Its size follows the space the layout gives it.

use {
    super::{Cx, Draw, Painter, Place, problem, styling::StyleExt as _},
    crate::{color::ThemeColor as _, terminal::TerminalView},
    gpui_kit::{component::ActiveTheme as _, *},
    moonframe_core::{MonoFont, Part, Terminal, TerminalBackend, TerminalEngine},
    moonframe_mux::Preference,
    moonframe_terminal::{Cell, Color, Engine, Screen, Size},
    std::rc::Rc,
};

const FONT_SIZE: f32 = 13.;
/// Line height as a multiple of the font size.
const LINE: f32 = 1.3;
/// Where characters the font lacks are looked for first, unless the
/// stylesheet names its own: the icons Nerd Font prompts and status lines
/// draw, when that font is installed.
const FALLBACKS: &[&str] = &["Symbols Nerd Font Mono"];
/// Space between the grid and the element's edge.
const PADDING: f32 = 8.;
/// The least room a terminal asks for.
const MIN_HEIGHT: f32 = 80.;

impl<'w> Draw<'w> for Terminal {
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement {
        let look = painter.look(Part::Terminal, &self.look, false, place);
        let theme = cx.theme();
        let background = look
            .base
            .background
            .map_or(theme.background, |color| color.hsla(theme));
        let foreground = look
            .base
            .color
            .map_or(theme.foreground, |color| color.hsla(theme));
        let engine = match self.engine {
            TerminalEngine::Alacritty => Engine::Alacritty,
            TerminalEngine::Ghostty => Engine::Ghostty,
        };
        let typeface = Typeface::new(
            &painter.workspace.stylesheet().mono,
            &theme.mono_font_family,
        );
        let frame = div()
            .id(place.id())
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .min_h(px(MIN_HEIGHT))
            .overflow_hidden()
            .bg(background)
            .text_color(foreground);
        // Children are clipped to a rectangle, not to rounded corners: a
        // terminal filling a rounded card rounds its own background to match.
        let frame = match look.base.radius {
            Some(radius) => frame.cornered(radius),
            None => frame,
        };
        if painter.preview {
            // A preview shows what it is; it starts no shell.
            return frame
                .p(px(PADDING))
                .font(typeface.font)
                .text_size(typeface.size)
                .child(format!("$ a live {} terminal goes here", engine.name()))
                .into_any_element();
        }
        let key = format!("{}/{}", place.owner, self.id);
        let preference = match self.backend {
            TerminalBackend::Auto => Preference::Auto,
            TerminalBackend::HerdrCore => Preference::HerdrCore,
            TerminalBackend::Tmux => Preference::Tmux,
            TerminalBackend::Pty => Preference::Pty,
        };
        let view = painter
            .terminals
            .open(&key, engine, preference, self.cwd.as_deref(), cx);
        if let Err(error) = &view.session {
            return problem(format!("terminal `{}`: {error}", self.id), cx);
        }
        painter.terminals.drawn(&key, &view);
        let focus = view.focus.clone();
        let scroll_view = view.clone();
        let hint = hint(view.backend, theme.muted_foreground);
        let line = typeface.line;
        frame
            .relative()
            .key_context("Terminal")
            .track_focus(&view.focus)
            .cursor(CursorStyle::IBeam)
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                focus.focus(window, cx)
            })
            .on_scroll(move |event: &ScrollEvent, _, _| {
                let moved = scroll_view.scrolled.get() + event.delta.pixel_delta(line).y / line;
                let lines = moved.trunc();
                scroll_view.scrolled.set(moved - lines);
                if lines != 0.
                    && let Ok(session) = &scroll_view.session
                {
                    // Wheel down shows older lines: up the scrollback.
                    session.scroll(-(lines as i32));
                }
            })
            .child(grid(view, typeface, background, foreground))
            .child(hint)
            .into_any_element()
    }
}

/// Which backend runs the terminal, small and faint in its bottom corner,
/// clear of the grid's last row.
fn hint(backend: &'static str, muted: Hsla) -> impl IntoElement {
    div()
        .absolute()
        .bottom(px(1.))
        .right(px(PADDING))
        .text_size(px(9.))
        .line_height(px(PADDING - 1.))
        .text_color(muted.opacity(0.6))
        .child(backend)
}

/// The cell grid, fitted to the space it gets.
fn grid(
    view: Rc<TerminalView>,
    typeface: Typeface,
    background: Hsla,
    foreground: Hsla,
) -> impl IntoElement {
    let resize = view.clone();
    canvas(
        move |bounds, window, _| {
            let metrics = Metrics::measure(typeface, window);
            if let Ok(session) = &resize.session {
                session.resize(metrics.fit(bounds));
            }
            metrics
        },
        move |bounds, metrics, window, cx| {
            let Ok(session) = &view.session else {
                return;
            };
            let screen = session.screen();
            let origin = bounds.origin + point(px(PADDING), px(PADDING));
            let focused = view.focus.is_focused(window);
            let painting = Painting {
                metrics: &metrics,
                origin,
                background,
                foreground,
            };
            for row in 0..screen.rows {
                painting.row(&screen, row, window, cx);
            }
            painting.cursor(&screen, focused, window);
            if screen.exited {
                painting.note(
                    "[the shell exited; press Enter to start another]",
                    &screen,
                    window,
                    cx,
                );
            }
        },
    )
    .size_full()
}

/// The font a terminal draws in, from the stylesheet's `mono`.
struct Typeface {
    font: Font,
    size: Pixels,
    line: Pixels,
}

impl Typeface {
    fn new(mono: &MonoFont, family: &SharedString) -> Self {
        // The theme has already applied `mono.family`, or selected an
        // installed Nerd Font / platform monospace default. A hard-coded
        // macOS family falls back to proportional text on Linux.
        let mut font = font(family.clone());
        if let Some(weight) = mono.weight {
            font.weight = FontWeight(weight);
        }
        let fallbacks = mono
            .fallbacks
            .clone()
            .unwrap_or_else(|| FALLBACKS.iter().map(|name| (*name).to_owned()).collect());
        if !fallbacks.is_empty() {
            font.fallbacks = Some(FontFallbacks::from_fonts(fallbacks));
        }
        if mono.ligatures == Some(false) {
            font.features = FontFeatures::disable_ligatures();
        }
        let size = mono.size.unwrap_or(FONT_SIZE);
        Self {
            font,
            size: px(size),
            line: px((size * mono.line_height.unwrap_or(LINE)).round()),
        }
    }
}

/// The font and the size of one cell.
struct Metrics {
    font: Font,
    size: Pixels,
    cell: gpui_kit::Size<Pixels>,
}

impl Metrics {
    fn measure(typeface: Typeface, window: &Window) -> Self {
        let Typeface { font, size, line } = typeface;
        let width = window
            .text_system()
            .shape_line("M".into(), size, &[run(1, &font, black())], None)
            .width;
        Self {
            font,
            size,
            cell: gpui_kit::size(width, line),
        }
    }

    /// The grid that fits in `bounds`, inside the padding.
    fn fit(&self, bounds: Bounds<Pixels>) -> Size {
        let cells = |room: Pixels, cell: Pixels| {
            ((room - px(PADDING * 2.)) / cell)
                .floor()
                .clamp(1., f32::from(u16::MAX)) as u16
        };
        Size {
            cols: cells(bounds.size.width, self.cell.width),
            rows: cells(bounds.size.height, self.cell.height),
            cell_width: f32::from(self.cell.width).round() as u16,
            cell_height: f32::from(self.cell.height).round() as u16,
        }
    }
}

/// What painting one screen needs.
struct Painting<'a> {
    metrics: &'a Metrics,
    origin: Point<Pixels>,
    background: Hsla,
    foreground: Hsla,
}

impl Painting<'_> {
    fn at(&self, col: u16, row: u16) -> Point<Pixels> {
        self.origin
            + point(
                self.metrics.cell.width * f32::from(col),
                self.metrics.cell.height * f32::from(row),
            )
    }

    fn color(&self, color: Color, default: Hsla) -> Hsla {
        match color {
            Color::Default => default,
            Color::Rgb(r, g, b) => rgb(u32::from_be_bytes([0, r, g, b])).into(),
        }
    }

    /// One row, in runs of cells that look the same: backgrounds first,
    /// then the text of each run where its first cell is.
    fn row(&self, screen: &Screen, row: u16, window: &mut Window, cx: &mut App) {
        let cells = screen.row(row);
        let mut start = 0;
        while start < cells.len() {
            let first = cells[start];
            let mut end = start + 1;
            while end < cells.len() && same_look(&first, &cells[end]) {
                end += 1;
            }
            let (col, count) = (start as u16, (end - start) as u16);
            let origin = self.at(col, row);
            if first.bg != Color::Default {
                let width = self.metrics.cell.width * f32::from(count);
                window.paint_quad(fill(
                    Bounds::new(origin, size(width, self.metrics.cell.height)),
                    self.color(first.bg, self.background),
                ));
            }
            let text: String = cells[start..end]
                .iter()
                .filter(|cell| !cell.spacer)
                .map(|cell| cell.ch)
                .collect();
            if !text.trim().is_empty() {
                let mut font = self.metrics.font.clone();
                if first.bold {
                    font.weight = FontWeight::BOLD;
                }
                if first.italic {
                    font.style = FontStyle::Italic;
                }
                let color = self.color(first.fg, self.foreground);
                let mut style = run(text.len(), &font, color);
                if first.underline {
                    style.underline = Some(UnderlineStyle {
                        thickness: px(1.),
                        color: Some(color),
                        wavy: false,
                    });
                }
                if first.strike {
                    style.strikethrough = Some(StrikethroughStyle {
                        thickness: px(1.),
                        color: Some(color),
                    });
                }
                let shaped =
                    window
                        .text_system()
                        .shape_line(text.into(), self.metrics.size, &[style], None);
                let _ = shaped.paint(
                    origin,
                    self.metrics.cell.height,
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
            start = end;
        }
    }

    /// A block when the terminal has focus, an outline when not.
    fn cursor(&self, screen: &Screen, focused: bool, window: &mut Window) {
        let Some((col, row)) = screen.cursor else {
            return;
        };
        let bounds = Bounds::new(self.at(col, row), self.metrics.cell);
        if focused {
            window.paint_quad(fill(bounds, self.foreground.opacity(0.45)));
        } else {
            window.paint_quad(outline(
                bounds,
                self.foreground.opacity(0.6),
                BorderStyle::Solid,
            ));
        }
    }

    /// A line under the screen's last row, such as why it stopped.
    fn note(&self, text: &str, screen: &Screen, window: &mut Window, cx: &mut App) {
        let row = screen.rows.saturating_sub(1);
        let shaped = window.text_system().shape_line(
            text.to_owned().into(),
            self.metrics.size,
            &[run(
                text.len(),
                &self.metrics.font,
                self.foreground.opacity(0.6),
            )],
            None,
        );
        let _ = shaped.paint(
            self.at(0, row),
            self.metrics.cell.height,
            TextAlign::Left,
            None,
            window,
            cx,
        );
    }
}

/// Whether two neighbouring cells can be shaped as one run. A wide
/// character's spacer joins the run before it, which keeps the columns.
fn same_look(first: &Cell, next: &Cell) -> bool {
    next.spacer
        || (first.fg == next.fg
            && first.bg == next.bg
            && first.bold == next.bold
            && first.italic == next.italic
            && first.underline == next.underline
            && first.strike == next.strike
            && !next.ch.is_ascii_control()
            && next.ch.is_ascii() == first.ch.is_ascii())
}

fn run(len: usize, font: &Font, color: Hsla) -> TextRun {
    TextRun {
        len,
        font: font.clone(),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    }
}
