//! What a mod draws, as plain text: what `app:text` reads for an assistant
//! to send with a question. Rows read across, columns down; what a mod
//! mounts, an instance or a region reads where it is drawn; hidden panes,
//! closed popovers and overlays, and the rows a list has not built are
//! left out, as they do not show. Fields are left out too: their text is
//! the user's, still being written.

use {
    super::*,
    crate::{Direction, TextOf},
};

/// How deep mounts nest before the rest is left out.
const MAX_DEPTH: usize = 16;
/// The most elements read, however mods mount each other: as many as
/// views may draw in all.
const MAX_VISITS: usize = 100_000;
/// The most text read, in bytes, whatever the asker's `max`.
const MOST: usize = 4 * 1024 * 1024;

impl Workspace {
    /// The text `of` draws, line by line; `None` when there is no such mod.
    pub fn drawn_text(&self, of: &TextOf) -> Option<String> {
        let mut reader = Reader {
            workspace: self,
            lines: Lines::default(),
            reading: Vec::new(),
            visits: 0,
        };
        match of {
            TextOf::Mod(name) => {
                let slot = self.slot(name)?;
                reader.slot(name, slot);
            },
            TextOf::Region(region) => reader.region(region),
        }
        Some(reader.lines.finish())
    }
}

/// A walk through what mods draw.
struct Reader<'w> {
    workspace: &'w Workspace,
    lines: Lines,
    /// The mods being read, outermost first: one mounted inside itself is
    /// read once.
    reading: Vec<&'w str>,
    /// Elements read so far.
    visits: usize,
}

impl<'w> Reader<'w> {
    fn slot(&mut self, name: &'w str, slot: &'w Slot) {
        if self.reading.len() >= MAX_DEPTH || self.reading.contains(&name) {
            return;
        }
        if let Some(view) = slot.view() {
            self.reading.push(name);
            self.node(name, view);
            self.reading.pop();
        }
    }

    fn region(&mut self, region: &str) {
        let workspace = self.workspace;
        for name in workspace.region(region) {
            if let Some(slot) = workspace.slot(name) {
                self.lines.end();
                self.slot(name, slot);
                self.lines.end();
            }
        }
    }

    /// `nodes` read across, as a row reads, or each on lines of its own.
    fn all(&mut self, owner: &'w str, nodes: &'w [Node], across: bool) {
        for node in nodes {
            if !across {
                self.lines.end();
            }
            self.node(owner, node);
        }
        if !across {
            self.lines.end();
        }
    }

    /// Adds what `node`, drawn by `owner`, shows.
    fn node(&mut self, owner: &'w str, node: &'w Node) {
        if self.lines.full() || self.visits >= MAX_VISITS {
            return;
        }
        self.visits += 1;
        let workspace = self.workspace;
        match node {
            Node::Stack(stack) => {
                self.all(owner, &stack.children, stack.direction == Direction::Row);
            },
            Node::Panel(panel) => {
                self.lines.end();
                self.lines.add(panel.title.as_deref().unwrap_or_default());
                self.all(owner, &panel.actions, true);
                self.all(owner, &panel.children, false);
            },
            Node::Split(split) => {
                for pane in split.panes.iter().filter(|pane| pane.visible) {
                    self.all(owner, &pane.children, false);
                }
            },
            Node::Text(text) => self.lines.add(&text.text),
            Node::Button(button) => self.lines.add(&button.label),
            Node::Item(item) => {
                self.lines.add(&item.label);
                self.lines.add(item.detail.as_deref().unwrap_or_default());
                self.lines.add(item.badge.as_deref().unwrap_or_default());
            },
            Node::Badge(badge) => self.lines.add(&badge.text),
            Node::Markdown(markdown) => {
                for line in markdown.text.lines() {
                    self.lines.end();
                    self.lines.add(line);
                }
                self.lines.end();
            },
            Node::Popover(popover) => {
                self.all(owner, &popover.anchor, true);
                if popover.open {
                    self.lines.end();
                    self.node(owner, &popover.content);
                    self.lines.end();
                }
            },
            Node::Overlay(overlay) if overlay.open => {
                self.lines.end();
                self.node(owner, &overlay.content);
                self.lines.end();
            },
            Node::List(list) => self.all(owner, &list.rows, false),
            Node::Image(image) => self.all(owner, &image.overlay, false),
            Node::Region(region) => self.region(&region.name),
            Node::Mount(name) => {
                if let Some(slot) = workspace.slot(name) {
                    self.slot(name, slot);
                }
            },
            Node::Instance(instance) => {
                if let Some((key, slot)) = workspace.instance(owner, instance) {
                    self.slot(key, slot);
                }
            },
            _ => {},
        }
    }
}

/// Text as it is read: words of a line joined by a space, lines by a
/// newline, with no blank ones.
#[derive(Default)]
struct Lines {
    text: String,
    /// Whether the last line has words and is not ended.
    open: bool,
}

impl Lines {
    fn add(&mut self, words: &str) {
        let words = words.trim();
        if words.is_empty() || self.full() {
            return;
        }
        if self.open {
            self.text.push(' ');
        }
        self.text.push_str(words);
        self.open = true;
    }

    fn end(&mut self) {
        if self.open {
            self.text.push('\n');
            self.open = false;
        }
    }

    fn full(&self) -> bool {
        self.text.len() >= MOST
    }

    fn finish(self) -> String {
        self.text.trim_end().to_owned()
    }
}
