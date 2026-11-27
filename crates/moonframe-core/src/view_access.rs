//! A mod's drawn view as roles and names, for a provider to read, and what
//! acting on one of its elements sends: a click, filling a field, a key, or
//! choosing an option, as a person's would. Components, mounted mods and
//! regions are read through, so the tree is what shows; each element has a
//! `ref`, its place in that tree, to act on.

use {
    crate::{Action, Data, KeyChord, Node, Workspace},
    std::{cell::Cell, collections::BTreeSet},
};

/// Most elements a tree lists; past it, `truncated` is set.
pub const MAX_VIEW_NODES: usize = 2000;
/// Longest name kept, in characters: a document's text is longer.
const MAX_NAME: usize = 200;
const MAX_TEXT: usize = 4000;
/// Deepest the tree is read, mounts within mounts included.
const MAX_DEPTH: usize = 128;
/// Most elements one read visits, listed or not.
const MAX_VISITS: usize = 50_000;
/// Most bytes of names and values a tree carries; past it, `truncated`.
const MAX_TREE_TEXT: usize = 128 * 1024;

/// One element as a person would describe it.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewNode {
    /// Where it is, to act on: `e0.2.1`.
    pub reference: String,
    /// `button`, `textbox`, `item`, `text`, `dialog`, ...
    pub role: &'static str,
    pub name: String,
    /// A field's text.
    pub value: Option<String>,
    pub selected: bool,
    pub disabled: bool,
    /// How many listed elements it is inside.
    pub depth: usize,
}

/// A mod's view as [`ViewNode`]s, in the order they show.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewTree {
    pub nodes: Vec<ViewNode>,
    /// There was more than it carries: past [`MAX_VIEW_NODES`] elements or
    /// their text's budget.
    pub truncated: bool,
    /// The mods (and copies and components) whose views it read.
    pub owners: BTreeSet<String>,
}

impl ViewTree {
    /// One line per element, indented by depth: `button "Save" [e0.3]`.
    pub fn lines(&self) -> String {
        let mut lines = String::new();
        for node in &self.nodes {
            lines.push_str(&"  ".repeat(node.depth));
            lines.push_str(node.role);
            if !node.name.is_empty() {
                lines.push_str(&format!(" {:?}", node.name));
            }
            if let Some(value) = &node.value {
                lines.push_str(&format!(" value={value:?}"));
            }
            if node.selected {
                lines.push_str(" selected");
            }
            if node.disabled {
                lines.push_str(" disabled");
            }
            lines.push_str(&format!(" [{}]\n", node.reference));
        }
        if self.truncated {
            lines.push_str("…\n");
        }
        lines
    }
}

/// What to do to an element.
#[derive(Clone, Debug, PartialEq)]
pub enum ViewAct {
    /// Click it, or the nearest element around it that takes a click.
    Click,
    /// Put this text in the field, in place of what it holds.
    Fill(String),
    /// Press a key (`enter`, `escape`, `cmd-k`) on it: a field's, an open
    /// popover's or dialog's around it, else its mod's.
    Press(String),
    /// Choose the option of this name inside it: a menu's or a list's.
    Select(String),
}

/// What the host does to carry out a [`ViewAct`].
#[derive(Clone, Debug, PartialEq)]
pub enum Acting {
    /// Dispatches `action` from `owner`, as a click on it would.
    Send { owner: String, action: Action },
    /// Puts `text` in the field `id`, typed into its editor while it is
    /// drawn, which sends its `change`; a field with no `id` (or none
    /// drawn) hears its `change` with the text alone.
    Fill {
        owner: String,
        id: Option<String>,
        to: Option<String>,
        change: Option<Data>,
        text: String,
    },
    /// Puts `text` on the clipboard, as a click on a copying control does,
    /// then dispatches `then` from `owner`, if it sends anything too.
    Copy {
        owner: String,
        text: String,
        then: Option<Action>,
    },
    /// A key the field `id` takes: sends `msg` with the field's text, as
    /// [`crate::Input::message`] does; `value` is the text the mod gave it.
    /// Enter in an address bar first takes the browser `browser` (of the
    /// mod `to`, else `owner`) to what it says.
    FieldKey {
        owner: String,
        id: Option<String>,
        to: Option<String>,
        msg: Option<Data>,
        browser: Option<String>,
        value: String,
    },
}

/// Where a walk goes after an element.
#[derive(Clone, Copy, PartialEq)]
enum Flow {
    /// Into what is inside it.
    Into,
    /// On to the next, past what is inside it.
    Over,
    /// Nowhere: the walk is over.
    Stop,
}

/// How a walk reads a view: the mods it may enter, and how many more
/// elements it may visit.
struct Reading<'r> {
    reaches: &'r dyn Fn(&str) -> bool,
    left: Cell<usize>,
}

impl<'r> Reading<'r> {
    fn new(reaches: &'r dyn Fn(&str) -> bool) -> Self {
        Self {
            reaches,
            left: Cell::new(MAX_VISITS),
        }
    }

    /// It visited as many as it may.
    fn spent(&self) -> bool {
        self.left.get() == 0
    }
}

/// An element as the walk meets it: what drew it.
#[derive(Clone, Copy)]
struct Visited<'w> {
    node: &'w Node,
    owner: &'w str,
}

impl Workspace {
    /// The mod `name`'s view as it shows, as roles and names: only what
    /// mods `reaches` allows draw is read, and `live` gives the text a
    /// field with an `id` shows (by its mod and id), over its `value`.
    pub fn view_tree(
        &self,
        name: &str,
        reaches: &dyn Fn(&str) -> bool,
        live: &dyn Fn(&str, &str) -> Option<String>,
    ) -> Result<ViewTree, String> {
        let reading = Reading::new(reaches);
        let root = self.view_root(name, &reading)?;
        let mut tree = ViewTree::default();
        let mut text = 0;
        let mut path = Vec::new();
        let mut around = Vec::new();
        self.walk(
            &reading,
            root,
            &mut path,
            &mut around,
            &mut |path, at, around| {
                tree.owners.insert(at.owner.to_owned());
                if tree.nodes.len() >= MAX_VIEW_NODES || text > MAX_TREE_TEXT {
                    tree.truncated = true;
                    return Flow::Stop;
                }
                if let Some(listed) = role(at.node, around) {
                    let value = match at.node {
                        Node::Input(input) => Some(cut(
                            &input
                                .id
                                .as_deref()
                                .and_then(|id| live(at.owner, id))
                                .or_else(|| input.value.clone())
                                .unwrap_or_default(),
                            MAX_TEXT,
                        )),
                        _ => None,
                    };
                    let name = self.name(&reading, at);
                    text += name.len() + value.as_ref().map_or(0, String::len);
                    tree.nodes.push(ViewNode {
                        reference: reference(path),
                        role: listed,
                        name,
                        value,
                        selected: matches!(at.node, Node::Item(item) if item.selected)
                            || matches!(at.node, Node::Stack(stack) if stack.selected),
                        disabled: matches!(at.node, Node::Button(button) if button.disabled)
                            || matches!(at.node, Node::Input(input) if input.disabled),
                        depth: around
                            .iter()
                            .enumerate()
                            .filter(|(at, visited)| role(visited.node, &around[..*at]).is_some())
                            .count(),
                    });
                }
                Flow::Into
            },
        );
        tree.truncated |= reading.spent();
        Ok(tree)
    }

    /// What doing `act` to the element `reference` of the mod `name`'s view
    /// sends, for the host to carry out.
    pub fn view_act(
        &self,
        name: &str,
        reaches: &dyn Fn(&str) -> bool,
        reference: &str,
        act: &ViewAct,
    ) -> Result<Acting, String> {
        let reading = Reading::new(reaches);
        let root = self.view_root(name, &reading)?;
        let wanted = parse_reference(reference)?;
        let mut found = None;
        let mut path = Vec::new();
        let mut around = Vec::new();
        self.walk(
            &reading,
            root,
            &mut path,
            &mut around,
            &mut |path, at, around| {
                if path == wanted.as_slice() {
                    found = Some((at, around.to_vec()));
                    Flow::Stop
                } else if wanted.starts_with(path) {
                    Flow::Into
                } else {
                    Flow::Over
                }
            },
        );
        let (at, around) = found.ok_or_else(|| {
            format!("no element `{reference}` in `{name}`'s view; read the tree again")
        })?;
        // The element, then those around it, nearest first.
        let mut chain = around;
        chain.push(at);
        chain.reverse();
        match act {
            ViewAct::Click => click(&chain),
            ViewAct::Fill(text) => fill(at, text),
            ViewAct::Press(key) => self.press(&reading, &chain, key),
            ViewAct::Select(option) => self.select(&reading, at, option),
        }
    }

    /// The view of the mod `name` (or a mounted copy, or the shell).
    fn view_root<'w>(
        &'w self,
        name: &'w str,
        reading: &Reading<'_>,
    ) -> Result<Visited<'w>, String> {
        if !(reading.reaches)(name) {
            return Err(format!("`{name}` is out of reach"));
        }
        let slot = self
            .slot(name)
            .ok_or_else(|| format!("no mod named `{name}`"))?;
        let node = slot
            .view()
            .ok_or_else(|| format!("`{name}` has drawn nothing"))?;
        Ok(Visited { node, owner: name })
    }

    /// Calls `visit` with each element under `at`, `at` first, with its
    /// place and those around it, outermost first; `visit` says where to
    /// go next. It enters only the mods `reading` reaches, none twice on
    /// the way down (a mod that mounts itself), and stops once it visited
    /// as many as it may. False once the walk is stopped.
    fn walk<'w>(
        &'w self,
        reading: &Reading<'_>,
        at: Visited<'w>,
        path: &mut Vec<usize>,
        around: &mut Vec<Visited<'w>>,
        visit: &mut dyn FnMut(&[usize], Visited<'w>, &[Visited<'w>]) -> Flow,
    ) -> bool {
        let Some(left) = reading.left.get().checked_sub(1) else {
            return false;
        };
        reading.left.set(left);
        match visit(path, at, around) {
            Flow::Stop => return false,
            Flow::Over => return true,
            Flow::Into if around.len() >= MAX_DEPTH => return true,
            Flow::Into => {},
        }
        around.push(at);
        let mut go_on = true;
        for (index, child) in self.children(at).into_iter().enumerate() {
            let entered = child.owner != at.owner
                || matches!(
                    at.node,
                    Node::Mount(_) | Node::Region(_) | Node::Instance(_)
                );
            if entered
                && (!(reading.reaches)(child.owner)
                    || around.iter().any(|outer| outer.owner == child.owner))
            {
                continue;
            }
            path.push(index);
            go_on = self.walk(reading, child, path, around, visit);
            path.pop();
            if !go_on {
                break;
            }
        }
        around.pop();
        go_on
    }

    /// What shows inside `at`: components and mounted mods read through.
    fn children<'w>(&'w self, at: Visited<'w>) -> Vec<Visited<'w>> {
        let owner = at.owner;
        let all = |nodes: &'w [Node]| nodes.iter().map(move |node| Visited { node, owner });
        let view = |name: &'w str| {
            self.slot(name)
                .and_then(|slot| slot.view())
                .map(|node| Visited { node, owner: name })
        };
        match at.node {
            Node::Stack(stack) => all(&stack.children).collect(),
            Node::Panel(panel) => all(&panel.actions).chain(all(&panel.children)).collect(),
            Node::Split(split) => split
                .panes
                .iter()
                .filter(|pane| pane.visible)
                .flat_map(|pane| all(&pane.children))
                .collect(),
            Node::Popover(popover) => {
                let mut nodes: Vec<_> = all(&popover.anchor).collect();
                if popover.open {
                    nodes.push(Visited {
                        node: &popover.content,
                        owner,
                    });
                }
                nodes
            },
            Node::Overlay(overlay) if overlay.open => vec![Visited {
                node: &overlay.content,
                owner,
            }],
            Node::List(list) => all(&list.rows).collect(),
            Node::Image(image) => all(&image.overlay).collect(),
            Node::Instance(instance) => self
                .instance(owner, instance)
                .and_then(|(key, slot)| {
                    Some(Visited {
                        node: slot.view()?,
                        owner: key,
                    })
                })
                .into_iter()
                .collect(),
            Node::Mount(name) => view(name).into_iter().collect(),
            Node::Region(region) => self
                .region(&region.name)
                .into_iter()
                .filter_map(view)
                .collect(),
            _ => Vec::new(),
        }
    }

    /// What a person would call it: its label or text, or for a clickable
    /// row the text inside it.
    fn name(&self, reading: &Reading<'_>, at: Visited<'_>) -> String {
        let name = match at.node {
            Node::Text(text) => return cut(&text.text, MAX_TEXT),
            Node::Markdown(markdown) => return cut(&markdown.text, MAX_TEXT),
            Node::Button(button) if !button.label.is_empty() => button.label.clone(),
            Node::Button(button) => button
                .extras
                .tooltip
                .clone()
                .or_else(|| button.icon.as_ref().map(|icon| icon.name().to_owned()))
                .unwrap_or_default(),
            Node::Item(item) => item.label.clone(),
            Node::Badge(badge) => badge.text.clone(),
            Node::Panel(panel) => panel.title.clone().unwrap_or_default(),
            Node::Input(input) => input
                .placeholder
                .clone()
                .or_else(|| input.id.clone())
                .unwrap_or_default(),
            Node::Image(image) => image.src.clone(),
            Node::Browser(browser) => browser.url.clone().unwrap_or_default(),
            Node::View(view) => view.name.clone(),
            Node::Problem(problem) => problem.clone(),
            Node::Stack(stack) => match &stack.extras.tooltip {
                Some(tooltip) => tooltip.clone(),
                None => self.words(reading, at),
            },
            _ => String::new(),
        };
        cut(&name, MAX_NAME)
    }

    /// The words shown inside `at`, joined.
    fn words(&self, reading: &Reading<'_>, at: Visited<'_>) -> String {
        let mut words: Vec<String> = Vec::new();
        let mut path = Vec::new();
        let mut around = Vec::new();
        let mut length = 0;
        self.walk(reading, at, &mut path, &mut around, &mut |_, inside, _| {
            let word = match inside.node {
                Node::Text(text) => text.text.as_str(),
                Node::Badge(badge) => badge.text.as_str(),
                Node::Button(button) => button.label.as_str(),
                Node::Item(item) => item.label.as_str(),
                _ => "",
            };
            let word = word.trim();
            if !word.is_empty() {
                length += word.len() + 1;
                words.push(word.to_owned());
            }
            if length < MAX_NAME * 4 {
                Flow::Into
            } else {
                Flow::Stop
            }
        });
        words.join(" ")
    }

    /// A key on the element or around it: a field's, an open popover's or
    /// dialog's, else the mod's own `keys`.
    fn press(
        &self,
        reading: &Reading<'_>,
        chain: &[Visited<'_>],
        key: &str,
    ) -> Result<Acting, String> {
        let pressed = KeyChord::parse(key).map_err(|problem| format!("`{key}`: {problem}"))?;
        let pressed = Held::of(&pressed);
        // The open popovers and dialogs around it take keys first, nearest
        // first, as the window's layers do.
        for at in chain {
            let (keys, dismiss, to) = match at.node {
                Node::Popover(popover) if popover.open => {
                    (&popover.keys, popover.dismiss.as_ref(), &popover.to)
                },
                Node::Overlay(overlay) if overlay.open => {
                    (&overlay.keys, overlay.dismiss.as_ref(), &overlay.to)
                },
                _ => continue,
            };
            if let Some(msg) = keyed(keys, dismiss, &pressed) {
                return Ok(sent(at.owner, to.clone(), msg));
            }
        }
        // Then the field it is, or is in.
        if let Some((at, input)) = chain.iter().find_map(|at| match at.node {
            Node::Input(input) => Some((at, input)),
            _ => None,
        }) {
            if input.disabled {
                return Err("the field is disabled".to_owned());
            }
            let submits = if input.multiline {
                pressed == Held::of_name("secondary-enter")
            } else {
                pressed == Held::of_name("enter")
            };
            let msg = input
                .keys
                .iter()
                .find(|(chord, _)| Held::of(chord) == pressed)
                .map(|(_, msg)| msg)
                .or(input.submit.as_ref().filter(|_| submits));
            // An address bar goes where it says on Enter, submit or not.
            let browser = input
                .browser
                .clone()
                .filter(|_| !input.multiline && pressed == Held::of_name("enter"));
            // A field recording a shortcut takes it, as it is pressed.
            if msg.is_none()
                && browser.is_none()
                && let Some(capture) = &input.capture
            {
                let keys = pressed.name().ok_or_else(|| {
                    format!("`{key}` is no shortcut: the field records keys held with a modifier")
                })?;
                return Ok(sent(
                    at.owner,
                    input.to.clone(),
                    &crate::Markdown::note(capture, [("keys", Data::String(keys))]),
                ));
            }
            if msg.is_some() || browser.is_some() {
                return Ok(Acting::FieldKey {
                    owner: at.owner.to_owned(),
                    id: input.id.clone(),
                    to: input.to.clone(),
                    msg: msg.cloned(),
                    browser,
                    value: input.value.clone().unwrap_or_default(),
                });
            }
        }
        // The mods whose views it is in, nearest first.
        for at in chain {
            if let Some((owner, keys)) = self.mod_keys().find(|(name, _)| *name == at.owner)
                && (reading.reaches)(owner)
                && let Some(msg) = keyed(keys, None, &pressed)
            {
                return Ok(sent(owner, None, msg));
            }
        }
        Err(format!("nothing there takes `{key}`"))
    }

    /// The option named `option` inside `at`: an item, a button or a
    /// clickable row, by its whole name, else the only one whose name has it.
    fn select(
        &self,
        reading: &Reading<'_>,
        at: Visited<'_>,
        option: &str,
    ) -> Result<Acting, String> {
        let wanted = option.trim().to_lowercase();
        let mut options: Vec<(String, Visited<'_>)> = Vec::new();
        let mut path = Vec::new();
        let mut around = Vec::new();
        self.walk(
            reading,
            at,
            &mut path,
            &mut around,
            &mut |path, inside, _| {
                if options.len() >= MAX_VIEW_NODES {
                    return Flow::Stop;
                }
                if !path.is_empty() && takes_clicks(inside.node) {
                    options.push((self.name(reading, inside), inside));
                    return Flow::Over;
                }
                Flow::Into
            },
        );
        let whole = options
            .iter()
            .find(|(name, _)| name.trim().to_lowercase() == wanted);
        let partly: Vec<_> = options
            .iter()
            .filter(|(name, _)| name.to_lowercase().contains(&wanted))
            .collect();
        let chosen = match (whole, partly.as_slice()) {
            (Some((_, chosen)), _) | (None, [(_, chosen)]) => *chosen,
            _ => {
                let names: Vec<&str> = options
                    .iter()
                    .map(|(name, _)| name.as_str())
                    .take(20)
                    .collect();
                return Err(format!(
                    "no one option named {option:?} there; it has {names:?}"
                ));
            },
        };
        click(&[chosen])
    }
}

/// Its role, if a person would name it; `around` are the elements it is
/// inside, outermost first. Text inside something clickable names it, so
/// is not listed again.
fn role(node: &Node, around: &[Visited<'_>]) -> Option<&'static str> {
    let in_clickable = || around.iter().any(|at| clickable(at.node));
    Some(match node {
        Node::Text(text) if !text.text.trim().is_empty() && !in_clickable() => "text",
        Node::Badge(_) if !in_clickable() => "badge",
        Node::Button(_) => "button",
        Node::Item(_) => "item",
        Node::Stack(_) if takes_clicks(node) => "button",
        Node::Panel(_) => "panel",
        Node::Input(_) => "textbox",
        Node::Markdown(_) | Node::Document(_) => "document",
        Node::Image(_) => "image",
        Node::Popover(popover) if popover.open => "popover",
        Node::Overlay(overlay) if overlay.open => "dialog",
        Node::List(_) => "list",
        Node::Terminal(_) => "terminal",
        Node::Browser(_) => "browser",
        Node::Map(_) => "map",
        Node::Video(_) => "video",
        Node::View(_) => "view",
        Node::Problem(_) => "problem",
        _ => return None,
    })
}

/// Whether a click on it does something of its own.
fn clickable(node: &Node) -> bool {
    matches!(node, Node::Button(_) | Node::Item(_)) || takes_clicks(node)
}

/// Whether a click on it sends something or copies something.
fn takes_clicks(node: &Node) -> bool {
    action(node).is_some() || copied(node).is_some()
}

/// What a click on it puts on the clipboard.
fn copied(node: &Node) -> Option<&String> {
    match node {
        Node::Button(button) => button.extras.copy.as_ref(),
        Node::Item(item) => item.extras.copy.as_ref(),
        Node::Stack(stack) => stack.extras.copy.as_ref(),
        _ => None,
    }
}

/// What a click on it sends.
fn action(node: &Node) -> Option<&Action> {
    match node {
        Node::Button(button) => button.action.as_ref(),
        Node::Item(item) => item.action.as_ref(),
        Node::Stack(stack) => stack.action.as_ref(),
        _ => None,
    }
}

/// A click on the first of `chain` that takes one.
fn click(chain: &[Visited<'_>]) -> Result<Acting, String> {
    for at in chain {
        if let Node::Button(button) = at.node
            && button.disabled
        {
            return Err("it is disabled".to_owned());
        }
        let owner = at.owner.to_owned();
        match (copied(at.node), action(at.node)) {
            (Some(text), then) => {
                return Ok(Acting::Copy {
                    owner,
                    text: text.clone(),
                    then: then.cloned(),
                });
            },
            (None, Some(action)) => {
                return Ok(Acting::Send {
                    owner,
                    action: action.clone(),
                });
            },
            (None, None) => {},
        }
    }
    Err("nothing happens when it is clicked".to_owned())
}

fn fill(at: Visited<'_>, text: &str) -> Result<Acting, String> {
    match at.node {
        Node::Input(input) if input.disabled => Err("the field is disabled".to_owned()),
        // Its editor is found by its `id`; without one only `change` hears.
        Node::Input(input) if input.id.is_none() && input.change.is_none() => {
            Err("the field has no `id` and no `change`, so nothing would take the text".to_owned())
        },
        Node::Input(input) => Ok(Acting::Fill {
            owner: at.owner.to_owned(),
            id: input.id.clone(),
            to: input.to.clone(),
            change: input.change.clone(),
            text: text.to_owned(),
        }),
        _ => Err("only a text field can be filled".to_owned()),
    }
}

/// A message sent as a click would send it.
fn sent(owner: &str, to: Option<String>, msg: &Data) -> Acting {
    Acting::Send {
        owner: owner.to_owned(),
        action: Action::Send {
            to,
            msg: msg.clone(),
        },
    }
}

/// What `keys` (and Escape, `dismiss`) send for `pressed`.
fn keyed<'k>(keys: &'k crate::Keys, dismiss: Option<&'k Data>, pressed: &Held) -> Option<&'k Data> {
    keys.0
        .iter()
        .find(|(chord, _)| Held::of_chord(chord) == *pressed)
        .map(|(_, msg)| msg)
        .or_else(|| {
            (*pressed == Held::of_name("escape"))
                .then_some(dismiss)
                .flatten()
        })
}

/// A key and the modifiers held, `secondary` made the platform's own.
#[derive(Debug, PartialEq)]
struct Held {
    key: String,
    ctrl: bool,
    alt: bool,
    shift: bool,
    cmd: bool,
}

impl Held {
    fn new(key: &str, [ctrl, alt, shift, cmd, secondary]: [bool; 5]) -> Self {
        let mac = cfg!(target_os = "macos");
        Self {
            key: key.to_owned(),
            ctrl: ctrl || (secondary && !mac),
            alt,
            shift,
            cmd: cmd || (secondary && mac),
        }
    }

    fn of(chord: &KeyChord) -> Self {
        Self::new(&chord.key, [
            chord.ctrl,
            chord.alt,
            chord.shift,
            chord.cmd,
            chord.secondary,
        ])
    }

    fn of_chord(chord: &crate::Chord) -> Self {
        Self::new(&chord.key, [
            chord.ctrl,
            chord.alt,
            chord.shift,
            chord.cmd,
            chord.secondary,
        ])
    }

    /// Its name as a field recording shortcuts sends it (`cmd-shift-p`):
    /// none for a lone character, which types rather than being a key.
    fn name(&self) -> Option<String> {
        if !(self.ctrl || self.alt || self.cmd) && self.key.chars().count() == 1 {
            return None;
        }
        let held = [
            (self.ctrl, "ctrl"),
            (self.alt, "alt"),
            (self.shift, "shift"),
            (self.cmd, "cmd"),
        ];
        let mut parts: Vec<&str> = held
            .into_iter()
            .filter_map(|(on, name)| on.then_some(name))
            .collect();
        parts.push(&self.key);
        Some(parts.join("-"))
    }

    fn of_name(name: &str) -> Self {
        KeyChord::parse(name).map_or_else(|_| Self::new(name, [false; 5]), |chord| Self::of(&chord))
    }
}

fn reference(path: &[usize]) -> String {
    let places: Vec<String> = path.iter().map(usize::to_string).collect();
    format!("e{}", places.join("."))
}

fn parse_reference(reference: &str) -> Result<Vec<usize>, String> {
    let bad = || format!("`{reference}` is no element ref; refs look like `e0.2`");
    let places = reference.strip_prefix('e').ok_or_else(bad)?;
    if places.is_empty() {
        return Ok(Vec::new());
    }
    places
        .split('.')
        .map(|place| place.parse::<usize>().map_err(|_| bad()))
        .collect()
}

/// `text` cut to `most` characters.
fn cut(text: &str, most: usize) -> String {
    match text.char_indices().nth(most) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.to_owned(),
    }
}
