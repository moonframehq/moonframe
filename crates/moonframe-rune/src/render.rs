//! Turns the checked node trees into Rune elements. It mirrors
//! `crates/moonframe/src/render.rs`: each element's look is its default here,
//! then the stylesheet's rules, then its own keys, and clicks go back to the
//! workspace with the module that drew the control.

// keep-sorted start
mod carry;
mod containers;
mod embeds;
mod floating;
mod gallery;
mod leaves;
mod markdown;
mod plots;
mod pointer;
mod prose;
mod shell;
// keep-sorted end

use {
    crate::{Ui, inspect::Crumb, style::Ink, theme::Palette},
    moonframe_core::{
        Direction, IconName, Look, Node, Part, Resolved, Rules, SHELL, Slot, States, Token,
        Workspace,
    },
    rune::{Cursor, Drag, El, Phase, col, row, text},
    std::{cell::RefCell, sync::LazyLock},
};

pub(crate) use {
    carry::{Carrying, Target},
    containers::RowRegions,
    markdown::Document,
    shell::Metrics,
};

static NO_RULES: LazyLock<Rules> = LazyLock::new(Rules::default);

/// Mounts deep enough for any real layout, as the GPUI renderer allows.
const MAX_MOUNTS: usize = 16;
/// The theme's font size when the stylesheet sets none.
const DEFAULT_FONT_SIZE: f32 = 14.;
/// The widest a problem floating over a view gets.
const PROBLEM_WIDTH: f32 = 320.;
/// The tallest a problem's card gets, and the most of its message it shows.
const PROBLEM_HEIGHT: f32 = 72.;
const PROBLEM_TEXT: usize = 300;
/// A broken mod's sign, round.
const BROKEN_SIZE: f32 = 20.;

/// What one frame's drawing shares.
pub(crate) struct Painter<'w> {
    ui: &'w Ui,
    /// The workspace, borrowed for the frame: every window draws from it.
    ws: &'w Workspace,
    palette: Palette<'w>,
    /// Popovers and overlays, drawn last at the window's root: a Rune
    /// overlay has to come after its anchor to draw above it.
    floating: RefCell<Vec<El>>,
    /// The same for a demo's layers, drawn on the browser's stage.
    stage: RefCell<Vec<El>>,
    /// What is wrong with mods that still show their last good view: in a
    /// corner of the window, over everything, so a broken edit never moves
    /// the layout.
    problems: RefCell<Vec<El>>,
}

/// Where a node is drawn: the mod that drew it, its unique path, and what it
/// inherits.
#[derive(Clone)]
pub(crate) struct Place<'w> {
    owner: &'w str,
    path: String,
    parent: Direction,
    ink: Ink,
    mounts: usize,
}

impl Place<'_> {
    fn child(&self, index: usize) -> Self {
        Self {
            path: format!("{}/{index}", self.path),
            ..self.clone()
        }
    }
}

/// The whole window: the shell, or the mod a window of its own shows.
pub(crate) fn window(ui: &Ui) -> El {
    ui.layers.borrow_mut().clear();
    ui.browsers.wanted.borrow_mut().clear();
    // Where things are carried to, found again as this view draws them.
    ui.carry_targets.borrow_mut().clear();
    ui.terminals.shown.borrow_mut().clear();
    ui.videos.frame_began();
    ui.crumbs.borrow_mut().clear();
    let workspace = ui.workspace();
    let painter = Painter {
        ui,
        ws: &workspace,
        palette: Palette {
            dark: ui.dark,
            overrides: &workspace.stylesheet().overrides,
        },
        floating: RefCell::default(),
        stage: RefCell::default(),
        problems: RefCell::default(),
    };
    let base = workspace
        .stylesheet()
        .font_size
        .unwrap_or(DEFAULT_FONT_SIZE);
    let place = Place {
        owner: SHELL,
        path: SHELL.to_owned(),
        parent: Direction::Column,
        ink: Ink {
            color: painter.palette.token(Token::Foreground),
            base,
            size: base,
            weight: 400.,
            mono: false,
        },
        mounts: 0,
    };
    let shell = if let Some(name) = &ui.shows {
        // As GPUI's mod window: the mod fills it.
        painter.slot(name, &place)
    } else if ui.gallery {
        painter.gallery(&place)
    } else {
        painter.slot(SHELL, &place)
    };
    let root = col()
        .w_full()
        .h_full()
        .fill(painter.palette.token(Token::Background))
        // Rune hands a paste to the root: it goes to the terminal in use.
        .on_paste(|ui: &mut Ui, text: &str, _: &mut rune::Tx| {
            ui.terminals.paste(text);
        })
        .child(shell);
    let root = painter
        .floating
        .into_inner()
        .into_iter()
        .fold(root, El::child);
    let problems = painter.problems.into_inner();
    let root = if problems.is_empty() {
        root
    } else {
        root.child(
            col()
                .key("problems")
                .pin(None, None, Some(8.), Some(32.))
                .max_w(PROBLEM_WIDTH)
                .gap(4.)
                .children(problems),
        )
    };
    if ui.inspecting.asker.is_none() || ui.inspecting.clicked() {
        return root;
    }
    root.child(
        rune::el()
            .key("inspect-layer")
            .pin(Some(0.), Some(0.), Some(0.), Some(0.))
            .cursor(Cursor::Crosshair)
            .on_drag(|ui: &mut Ui, drag: &mut Drag, _: &mut rune::Tx| {
                if matches!(drag.phase, Phase::Up) && !drag.cancelled() {
                    ui.pick_at((drag.x, drag.y));
                }
            }),
    )
}

/// What Inspect Element calls `node`, and the line that built it.
fn named(node: &Node) -> Option<(String, Option<u32>)> {
    let (name, look) = match node {
        Node::Stack(stack) => match stack.direction {
            Direction::Row => ("ui.row", &stack.look),
            Direction::Column => ("ui.column", &stack.look),
        },
        Node::Panel(panel) => ("ui.panel", &panel.look),
        Node::Text(text) => ("ui.text", &text.look),
        Node::Button(button) => ("ui.button", &button.look),
        Node::Item(item) => ("ui.item", &item.look),
        Node::Badge(badge) => ("ui.badge", &badge.look),
        Node::Icon(icon) => ("ui.icon", &icon.look),
        Node::Image(image) => ("ui.image", &image.look),
        Node::Input(input) => ("ui.input", &input.look),
        Node::Markdown(markdown) => ("ui.markdown", &markdown.look),
        Node::Terminal(terminal) => ("ui.terminal", &terminal.look),
        Node::Browser(browser) => ("ui.browser", &browser.look),
        _ => return None,
    };
    let label = match look.class.first() {
        Some(class) => format!("{name} .{class}"),
        None => name.to_owned(),
    };
    Some((label, look.line))
}

impl<'w> Painter<'w> {
    fn look(&self, part: Part, look: &Look, states: States, place: &Place) -> Resolved {
        let local = self.ws.slot(place.owner).map_or(&*NO_RULES, Slot::rules);
        self.ws.stylesheet().resolve_in(part, look, states, local)
    }

    /// A text field's selection: the theme's accent, faint. Rune's own, and
    /// its caret, follow its default foreground, white whatever the theme.
    fn selection(&self) -> rune::Color {
        self.palette.token(Token::Accent).alpha(0.3)
    }

    /// The sign of a mod whose view broke: a small warning, what went wrong
    /// in its tooltip, cut short as a problem's card is.
    fn broken(&self, owner: &str, error: &str) -> El {
        let message = format!("{owner}: {error}");
        let mut shown: String = message.chars().take(PROBLEM_TEXT).collect();
        if message.chars().count() > PROBLEM_TEXT {
            shown.push('…');
        }
        let danger = self.palette.token(Token::Danger);
        let sign = IconName::new("triangle-alert")
            .as_ref()
            .and_then(crate::icons::icon)
            .map_or_else(
                || text("!").color(danger),
                |found| rune::icon(found, 12.).color(danger),
            );
        row()
            .key(format!("broken:{owner}"))
            .size(BROKEN_SIZE, BROKEN_SIZE)
            .items_center()
            .justify_center()
            .radius(BROKEN_SIZE / 2.)
            .fill(self.palette.token(Token::Background))
            .border(1., danger)
            .label(format!("{owner} is broken"))
            .help(shown)
            .child(sign)
    }

    /// A small marker in place of what could not be drawn, saying why: as
    /// GPUI's, a card no taller than [`PROBLEM_HEIGHT`] that cuts a long
    /// message (a traceback) short, so it never takes over the window.
    fn marker(&self, message: &str, place: &Place) -> El {
        self.ui
            .left_out
            .borrow_mut()
            .insert(format!("marker: {message}"));
        let mut shown: String = message.chars().take(PROBLEM_TEXT).collect();
        if message.chars().count() > PROBLEM_TEXT {
            shown.push('…');
        }
        let mut ink = place.ink;
        ink.size = (ink.base * 0.85).round();
        col()
            .min_w(0.)
            .max_h(PROBLEM_HEIGHT)
            .padding(6.)
            .radius(6.)
            .clip_rounded(6.)
            .fill(self.palette.token(Token::Background))
            .border(1., self.palette.token(Token::Danger))
            .child(
                text(&shown)
                    .font(ink.text_style())
                    .color(self.palette.token(Token::Danger))
                    .wrap(),
            )
    }

    /// The view of the mod `name`, or what is wrong with it.
    fn slot(&self, name: &'w str, place: &Place<'w>) -> El {
        let slot = self.ws.slot(name);
        self.mounted(name, slot, place)
    }

    /// What a mod adds to a row region: its view, or with `spread` each of
    /// that view's children. One with a problem stays whole, to show it.
    fn members(&self, name: &'w str, place: &Place<'w>) -> Vec<El> {
        let slot = self.ws.slot(name);
        match slot
            .filter(|slot| slot.error().is_none())
            .and_then(Slot::view)
        {
            Some(Node::Stack(stack)) if stack.spread && place.mounts < MAX_MOUNTS => {
                let inner = Place {
                    owner: name,
                    path: format!("{}>{name}", place.path),
                    mounts: place.mounts + 1,
                    parent: Direction::Row,
                    ..place.clone()
                };
                self.nodes(&stack.children, &inner)
            },
            _ => vec![self.slot(name, place)],
        }
    }

    fn mounted(&self, owner: &'w str, slot: Option<&'w Slot>, place: &Place<'w>) -> El {
        if place.mounts >= MAX_MOUNTS {
            return self.marker("mounted too deep", place);
        }
        let Some(slot) = slot else {
            return self.marker(&format!("no mod named {owner}"), place);
        };
        let Some(view) = slot.view() else {
            return match slot.error() {
                // Never drawn yet: only the sign, in its place.
                Some(error) => self.broken(owner, &error.to_string()),
                None => rune::el(),
            };
        };
        let inner = Place {
            owner,
            path: format!("{}>{owner}", place.path),
            mounts: place.mounts + 1,
            ..place.clone()
        };
        let drawn = self.node(view, &inner);
        let Some(error) = slot.error() else {
            return drawn;
        };
        // The last good view stays as it was, a broken sign pinned over its
        // corner, the problem in its tooltip: an edit that breaks never
        // moves the layout. A view that holds no children (a lone text)
        // has the sign in the window's corner instead.
        let sign = self.broken(owner, &error.to_string());
        match view {
            Node::Stack(_) | Node::Panel(_) => {
                drawn.child(sign.pin(None, Some(2.), Some(2.), None))
            },
            _ => {
                self.problems.borrow_mut().push(sign);
                drawn
            },
        }
    }

    fn nodes(&self, nodes: &'w [Node], place: &Place<'w>) -> Vec<El> {
        nodes
            .iter()
            .enumerate()
            .map(|(index, node)| self.node(node, &place.child(index)))
            .collect()
    }

    pub(crate) fn node(&self, node: &'w Node, place: &Place<'w>) -> El {
        let drawn = self.drawn(node, place);
        if self.ui.inspecting.asker.is_none() {
            return drawn;
        }
        let Some((element, line)) = named(node) else {
            return drawn;
        };
        self.ui
            .crumbs
            .borrow_mut()
            .insert(place.path.clone(), Crumb {
                element,
                owner: place.owner.to_owned(),
                line,
            });
        drawn.key(place.path.clone())
    }

    fn drawn(&self, node: &'w Node, place: &Place<'w>) -> El {
        match node {
            Node::Stack(stack) => self.stack(stack, place),
            Node::Panel(panel) => self.panel(panel, place),
            Node::Split(split) => self.split(split, place),
            Node::List(list) => self.list(list, place),
            Node::RevisionTimeline(_) => self.not_here_named("ui.revision_timeline", place),
            Node::Region(region) => self.region(region, place),
            Node::Mount(name) => self.slot(name, place),
            Node::Instance(instance) => match self.ws.instance(place.owner, instance) {
                Some((key, slot)) => self.mounted(key, Some(slot), place),
                None => self.marker(&format!("no instance {}", instance.id), place),
            },
            Node::Popover(popover) => self.popover(popover, place),
            Node::Overlay(overlay) => self.overlay(overlay, place),
            Node::Text(text) => self.text(text, place),
            Node::Button(button) => self.button(button, place),
            Node::Item(item) => self.item(item, place),
            Node::Badge(badge) => self.badge(badge, place),
            Node::Icon(icon) => self.icon(icon, place),
            Node::Input(input) => self.input(input, place),
            Node::Markdown(markdown) => self.markdown(markdown, place),
            Node::Line(line) => self.line(line, place),
            Node::Paths(paths) => self.paths(paths, place),
            Node::Hatch(hatch) => self.hatch(hatch, place),
            Node::Image(image) => self.image(image, place),
            Node::Terminal(terminal) => self.terminal(terminal, place),
            Node::Browser(browser) => self.browser(browser, place),
            Node::Map(map) => self.map(map, place),
            Node::Video(video) => self.video(video, place),
            Node::View(view) => match self.ui.views.get(&view.name) {
                Some(draw) => draw(&view.props).key(place.path.clone()),
                None => self.not_here_named("ui.view", place),
            },
            Node::Divider => self.divider(place),
            Node::Spacer => rune::spacer(),
            Node::Problem(problem) => self.marker(problem, place),
        }
    }
}
