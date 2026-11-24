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

/// How deep mounts nest before the rest is left out, as a mount cycle
/// would never end.
const MAX_DEPTH: usize = 16;
/// The most text read, in bytes, whatever the asker's `max`.
const MOST: usize = 4 * 1024 * 1024;

impl Workspace {
    /// The text `of` draws, line by line; `None` when there is no such mod.
    pub fn drawn_text(&self, of: &TextOf) -> Option<String> {
        let mut lines = Lines::default();
        match of {
            TextOf::Mod(name) => {
                let slot = self.slot(name)?;
                self.read_slot(name, slot, &mut lines, 0);
            },
            TextOf::Region(region) => self.read_region(region, &mut lines, 0),
        }
        Some(lines.finish())
    }

    fn read_slot(&self, name: &str, slot: &Slot, lines: &mut Lines, depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }
        if let Some(view) = slot.view() {
            self.read(name, view, lines, depth);
        }
    }

    fn read_region(&self, region: &str, lines: &mut Lines, depth: usize) {
        for name in self.region(region) {
            if let Some(slot) = self.slot(name) {
                self.read_slot(name, slot, lines, depth + 1);
                lines.end();
            }
        }
    }

    fn read_all(&self, owner: &str, nodes: &[Node], across: bool, lines: &mut Lines, depth: usize) {
        for node in nodes {
            self.read(owner, node, lines, depth);
            if !across {
                lines.end();
            }
        }
        lines.end();
    }

    /// Adds what `node`, drawn by `owner`, shows.
    fn read(&self, owner: &str, node: &Node, lines: &mut Lines, depth: usize) {
        if lines.full() {
            return;
        }
        match node {
            Node::Stack(stack) => {
                let across = stack.direction == Direction::Row;
                self.read_all(owner, &stack.children, across, lines, depth);
            },
            Node::Panel(panel) => {
                lines.add(panel.title.as_deref().unwrap_or_default());
                self.read_all(owner, &panel.actions, true, lines, depth);
                self.read_all(owner, &panel.children, false, lines, depth);
            },
            Node::Split(split) => {
                for pane in split.panes.iter().filter(|pane| pane.visible) {
                    self.read_all(owner, &pane.children, false, lines, depth);
                }
            },
            Node::Text(text) => lines.add(&text.text),
            Node::Button(button) => lines.add(&button.label),
            Node::Item(item) => {
                lines.add(&item.label);
                lines.add(item.detail.as_deref().unwrap_or_default());
                lines.add(item.badge.as_deref().unwrap_or_default());
            },
            Node::Badge(badge) => lines.add(&badge.text),
            Node::Markdown(markdown) => {
                lines.end();
                for line in markdown.text.lines() {
                    lines.add(line);
                    lines.end();
                }
            },
            Node::Popover(popover) => {
                self.read_all(owner, &popover.anchor, true, lines, depth);
                if popover.open {
                    self.read(owner, &popover.content, lines, depth);
                    lines.end();
                }
            },
            Node::Overlay(overlay) if overlay.open => {
                self.read(owner, &overlay.content, lines, depth);
                lines.end();
            },
            Node::List(list) => self.read_all(owner, &list.rows, false, lines, depth),
            Node::Image(image) => self.read_all(owner, &image.overlay, false, lines, depth),
            Node::Region(region) => self.read_region(&region.name, lines, depth),
            Node::Mount(name) => {
                if let Some(slot) = self.slot(name) {
                    self.read_slot(name, slot, lines, depth + 1);
                }
            },
            Node::Instance(instance) => {
                if let Some((key, slot)) = self.instance(owner, instance) {
                    self.read_slot(key, slot, lines, depth + 1);
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
