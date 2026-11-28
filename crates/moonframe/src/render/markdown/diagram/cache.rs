//! Fences laid out, kept between frames. A full cache drops the half used
//! longest ago, so a fence still on screen keeps its layout, and a
//! streaming fence the last version of it that parsed (what stands in
//! while a line is half written).

use {
    gpui_kit::SharedString,
    moonframe_diagram::{Diagram, Layout},
    std::{collections::HashMap, sync::Arc},
};

/// Laid-out fences kept: a few long chats' worth.
const KEPT: usize = 256;

/// A fence laid out, or why not.
pub(super) type Laid = Result<(Arc<Diagram>, Arc<Layout>), String>;

/// What a fence's layout hangs on besides its text: the font its labels
/// are measured in (the theme's, which can change), and its language.
pub(super) type Kind = (SharedString, String);

/// Each fence's text laid out, by kind and text, with when it was last
/// used.
#[derive(Default)]
pub(super) struct Cache {
    entries: HashMap<(Kind, String), (Laid, u64)>,
    clock: u64,
}

impl Cache {
    /// What `source` of `kind` laid out to, if kept.
    pub(super) fn get(&mut self, kind: &Kind, source: &str) -> Option<Laid> {
        self.clock += 1;
        let (laid, used) = self.entries.get_mut(&(kind.clone(), source.to_owned()))?;
        *used = self.clock;
        Some(laid.clone())
    }

    /// The longest text of this `kind` laid out before that `source` grew
    /// from, as a streaming fence does.
    pub(super) fn earlier(&mut self, kind: &Kind, source: &str) -> Option<Laid> {
        self.clock += 1;
        let (laid, used) = self
            .entries
            .iter_mut()
            .filter(|((k, text), (laid, _))| {
                k == kind && laid.is_ok() && source.starts_with(text.as_str())
            })
            .max_by_key(|((_, text), _)| text.len())
            .map(|(_, entry)| entry)?;
        *used = self.clock;
        Some(laid.clone())
    }

    pub(super) fn insert(&mut self, kind: Kind, source: String, laid: Laid) {
        if self.entries.len() >= KEPT {
            self.make_room();
        }
        self.clock += 1;
        self.entries.insert((kind, source), (laid, self.clock));
    }

    /// Drops the half of the entries used longest ago.
    fn make_room(&mut self) {
        let mut used: Vec<u64> = self.entries.values().map(|(_, used)| *used).collect();
        used.sort_unstable();
        let cutoff = used[used.len() / 2];
        self.entries.retain(|_, (_, used)| *used >= cutoff);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use {
        super::{Cache, KEPT, Kind, Laid},
        moonframe_diagram::{Diagram, Layout, Node},
        std::sync::Arc,
    };

    fn ok(nodes: usize) -> Laid {
        let mut diagram = Diagram::default();
        for i in 0..nodes {
            diagram.nodes.push(Node::new(format!("n{i}"), "x"));
        }
        Ok((Arc::new(diagram), Arc::new(Layout::default())))
    }

    fn kind(font: &str, language: &str) -> Kind {
        (font.to_owned().into(), language.to_owned())
    }

    #[test]
    fn a_growing_fence_shows_the_longest_version_it_grew_from() {
        let mut cache = Cache::default();
        let mermaid = kind("Inter", "mermaid");
        cache.insert(mermaid.clone(), "flowchart TD\n A-->B".into(), ok(2));
        cache.insert(
            mermaid.clone(),
            "flowchart TD\n A-->B\n B-->C".into(),
            ok(3),
        );
        cache.insert(
            mermaid.clone(),
            "flowchart TD\n A-->B\n B-->C\n C-->[".into(),
            Err("half".into()),
        );
        cache.insert(mermaid.clone(), "flowchart LR\n X-->Y".into(), ok(9));
        let found = cache
            .earlier(&mermaid, "flowchart TD\n A-->B\n B-->C\n C-->[Do")
            .unwrap();
        assert_eq!(found.unwrap().0.nodes.len(), 3);
        assert!(
            cache
                .earlier(&kind("Inter", "dot"), "flowchart TD\n A-->B")
                .is_none()
        );
        assert!(cache.earlier(&mermaid, "sequenceDiagram").is_none());
        // Measured in another font, the boxes would be the wrong size.
        assert!(
            cache
                .earlier(&kind("Menlo", "mermaid"), "flowchart TD\n A-->B")
                .is_none()
        );
    }

    #[test]
    fn a_full_cache_keeps_what_a_half_written_fence_stands_on() {
        let mut cache = Cache::default();
        let mermaid = kind("Inter", "mermaid");
        cache.insert(mermaid.clone(), "flowchart TD\n A-->B".into(), ok(2));
        // Other fences fill the cache many times over while this one,
        // half written, draws each frame from its last good version.
        for i in 0..KEPT * 3 {
            let half = "flowchart TD\n A-->B\n B-->[";
            assert!(cache.earlier(&mermaid, half).is_some(), "lost after {i}");
            cache.insert(mermaid.clone(), format!("flowchart LR\n X{i}-->Y"), ok(1));
        }
        assert!(cache.entries.len() <= KEPT);
        // What nothing used for a long while is gone.
        assert!(cache.get(&mermaid, "flowchart LR\n X0-->Y").is_none());
        assert!(
            cache
                .get(&mermaid, &format!("flowchart LR\n X{}-->Y", KEPT * 3 - 1))
                .is_some()
        );
    }
}
