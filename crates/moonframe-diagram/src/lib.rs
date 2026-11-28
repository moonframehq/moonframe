//! Diagrams drawn in an app's own style.
//!
//! - [`model`]: what a diagram says: nodes, edges, groups, and for a
//!   sequence diagram its messages and notes. Plain data.
//! - [`layout`]: where everything goes. A diagram whose nodes carry no
//!   position is laid out in layers (ranks, crossing reduction, coordinate
//!   assignment) with orthogonal edges; one whose nodes are placed keeps
//!   them and only routes its edges. A new layout keeps what the previous
//!   one decided where it can, so adding a node does not move the rest.
//! - `view` (the `view` feature): a GPUI view that draws a diagram and its
//!   layout with the theme's colors and fonts, pans, zooms and, in edit
//!   mode, selects, drags, resizes and connects, telling its owner each
//!   edit as a [`view::DiagramEvent`] rather than changing the diagram
//!   itself.
//!
//! Nothing here knows about Moonframe: any gpui-kit app can embed the view,
//! and a program with no GUI can lay diagrams out.

// keep-sorted start
pub mod geometry;
pub mod layout;
pub mod model;
#[cfg(feature = "view")]
pub mod view;
// keep-sorted end

pub use {
    geometry::{Point, Rect, Size},
    layout::{Layout, Measure, Options, Route, layout},
    model::{
        Diagram, DiagramKind, Direction, Edge, EdgeKind, Group, Head, Message, MessageKind, Node,
        NodeKind, Note, Tone,
    },
};
