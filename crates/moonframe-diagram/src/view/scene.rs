//! Where everything is this frame, in diagram pixels: the layout, eased
//! toward a new one, with the gesture under way and edits waiting for the
//! owner on top. Edges touching a node that moved are routed again; the
//! rest keep the layout's routes.

use {
    super::{DiagramView, input::Gesture},
    crate::{
        geometry::{Point, Rect},
        layout::{Route, placed},
    },
    std::collections::{HashMap, HashSet},
};

pub(crate) struct Scene {
    pub rects: HashMap<String, Rect>,
    pub routes: HashMap<String, Route>,
    /// How strongly edges show: they fade in while nodes ease to a new
    /// layout, as their routes are the new layout's.
    pub edge_alpha: f32,
    pub animating: bool,
    /// Lines a dragged node snapped to.
    pub guides: Vec<(Point, Point)>,
    pub marquee: Option<Rect>,
    /// A connector being drawn: from, to, and the node it would end on.
    pub link: Option<(Point, Point, Option<String>)>,
    /// A node being drawn with a shape tool.
    pub creating: Option<Rect>,
}

impl Scene {
    pub(crate) fn build(view: &mut DiagramView) -> Self {
        let (mut rects, t) = view.eased();
        let easing = t < 1.0;
        let mut moved: HashSet<String> = view.pending.keys().cloned().collect();
        let mut scene = Scene {
            rects: HashMap::new(),
            routes: HashMap::new(),
            edge_alpha: t,
            animating: easing,
            guides: Vec::new(),
            marquee: None,
            link: None,
            creating: None,
        };
        match &view.gesture {
            Some(Gesture::Move {
                ids, delta, guides, ..
            }) => {
                for id in ids {
                    if let Some(r) = rects.get_mut(id) {
                        *r = r.translate(delta.x, delta.y);
                        moved.insert(id.clone());
                    }
                }
                scene.guides = guides.clone();
            },
            Some(Gesture::Resize { id, now, .. }) => {
                rects.insert(id.clone(), *now);
                moved.insert(id.clone());
            },
            Some(Gesture::Marquee { start, end, .. }) => {
                scene.marquee = Some(Rect::from_corners(*start, *end))
            },
            Some(Gesture::Connect { from, end, over }) => {
                if let Some(r) = rects.get(from) {
                    scene.link = Some((r.center(), *end, over.clone()));
                }
            },
            Some(Gesture::Create { start, end, .. }) => {
                scene.creating = Some(Rect::from_corners(*start, *end))
            },
            Some(Gesture::Pan { .. }) | None => {},
        }
        let mut routes: HashMap<String, Route> = view
            .layout
            .edges
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        if !moved.is_empty() {
            let edges = view
                .diagram
                .edges
                .iter()
                .map(|e| (e.id.clone(), e.from.clone(), e.to.clone()));
            placed::reroute(edges, &rects, &|id| moved.contains(id), &mut routes);
        }
        scene.rects = rects;
        scene.routes = routes;
        scene
    }
}
