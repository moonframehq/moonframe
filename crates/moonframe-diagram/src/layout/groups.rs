//! Group boxes: around their nodes and the groups inside them, with room
//! for the title on top. Innermost first, so a group holds its children's
//! boxes.

use {
    super::{Layout, Options},
    crate::{
        geometry::{Rect, bounds},
        model::Diagram,
    },
};

pub(super) fn place(diagram: &Diagram, options: &Options, out: &mut Layout) {
    let depth = |id: &str| {
        let mut depth = 0;
        let mut at = diagram.groups.iter().find(|g| g.id == id);
        while let Some(group) = at {
            depth += 1;
            // A loop of parents would never end: stop at the group count.
            if depth > diagram.groups.len() {
                break;
            }
            at = group.parent.as_deref().and_then(|p| diagram.groups.iter().find(|g| g.id == p));
        }
        depth
    };
    let mut groups: Vec<_> = diagram.groups.iter().collect();
    groups.sort_by_key(|g| std::cmp::Reverse(depth(&g.id)));
    for group in groups {
        let members: Vec<Rect> = diagram
            .nodes
            .iter()
            .filter(|n| n.group.as_deref() == Some(group.id.as_str()))
            .filter_map(|n| out.nodes.get(&n.id).copied())
            .chain(
                diagram
                    .groups
                    .iter()
                    .filter(|g| g.parent.as_deref() == Some(group.id.as_str()))
                    .filter_map(|g| out.groups.get(&g.id).copied()),
            )
            .collect();
        let Some(inside) = bounds(&members) else {
            continue;
        };
        let mut rect = inside.inflate(options.group_padding);
        rect.y -= options.group_header;
        rect.h += options.group_header;
        out.groups.insert(group.id.clone(), rect);
    }
}
