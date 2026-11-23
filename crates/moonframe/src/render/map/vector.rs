//! The `vector-*` providers: vector tiles drawn on the GPU by
//! [gpui-maps](https://github.com/swatmobility/gpui-maps), in place of the
//! raster tile layer. gpui-maps takes the gestures, with its glide after a
//! drag; the map follows its camera ([`view::adopt::adopt`]) and keeps its own
//! pins, routes, credit and zoom buttons, so a view writes the same `ui.map`
//! whichever provider it picks. A move from outside -- the view's `center`,
//! the zoom buttons -- is handed to gpui-maps. Tilted, the pins and routes
//! are placed with gpui-maps' camera ([`tilt`]).

use {
    super::{
        Sender, redraw, tell,
        tilt::Tilt,
        view::{self, Frame},
    },
    gpui_kit::*,
    gpui_maps::{
        basemaps::{self, Basemap},
        camera_control::CameraTarget,
        map_view::MapView,
    },
    moonframe_core::{Data, MapProvider},
    std::{cell::RefCell, collections::HashMap, sync::OnceLock},
};

/// Where a map looks, in the map's own terms: `(lat, lon, zoom, bearing)`,
/// zoom counting 256-pixel tiles and bearing in degrees.
type Pose = (f64, f64, f64, f64);

/// What the map `key` knows of its gpui-maps view. The view itself is held
/// by the map's layer element ([`Held`]), so it goes once the map is no
/// longer drawn -- its window closed, or the map gone from it.
struct Kept {
    view: WeakEntity<MapView>,
    /// Where gpui-maps' camera was, last seen: a camera found elsewhere
    /// moved under the user's hand.
    seen: Option<Pose>,
    /// Its tilt, last seen, radians.
    seen_pitch: f32,
    /// The frame last handed to gpui-maps, so an unchanged one is not
    /// handed again -- that would stop a glide gpui-maps has under way.
    asked: Option<Pose>,
    /// What the map tells its view when it moves, and how.
    moved: Option<Data>,
    sender: Sender,
}

/// A map's gpui-maps view and the watch on its camera, kept in its layer's
/// element state for as long as the map is drawn frame after frame.
struct Held {
    provider: MapProvider,
    view: Entity<MapView>,
    _watch: Subscription,
}

thread_local! {
    static VIEWS: RefCell<HashMap<String, Kept>> = RefCell::new(HashMap::new());
}

/// The runtime gpui-maps fetches tiles, fonts and sprites on; `None` if the
/// system would not give it threads.
fn runtime() -> Option<tokio_maps::runtime::Handle> {
    static RUNTIME: OnceLock<Option<tokio_maps::runtime::Runtime>> = OnceLock::new();
    RUNTIME
        .get_or_init(|| {
            tokio_maps::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .thread_name("moonframe-map")
                .enable_all()
                .build()
                .inspect_err(|error| tracing::warn!("ui.map: no vector map runtime: {error}"))
                .ok()
        })
        .as_ref()
        .map(|runtime| runtime.handle().clone())
}

fn basemap(provider: MapProvider) -> Basemap {
    match provider {
        MapProvider::VectorDark => Basemap::Dark,
        MapProvider::VectorPositron => Basemap::Positron,
        MapProvider::VectorGraphite => Basemap::SwatDark,
        MapProvider::VectorStreets => Basemap::StreetsLight,
        _ => Basemap::Liberty,
    }
}

/// Where `frame` looks.
fn pose_of(frame: Frame) -> Pose {
    let [lat, lon] = frame.middle();
    (lat, lon, frame.zoom, frame.bearing)
}

/// Where gpui-maps' camera looks. Its zoom counts 512-pixel tiles, as
/// MapLibre's does, so it is one less; both bearings are the compass
/// direction up the box, its in radians.
fn camera_pose(view: &MapView) -> Pose {
    let camera = view.camera();
    (
        camera.center_lat,
        camera.center_lon,
        camera.zoom + 1.0,
        f64::from(camera.bearing).to_degrees().rem_euclid(360.0),
    )
}

fn same(a: Pose, b: Pose) -> bool {
    let turn = (a.3 - b.3).rem_euclid(360.0);
    (a.0 - b.0).abs() < 1e-9
        && (a.1 - b.1).abs() < 1e-9
        && (a.2 - b.2).abs() < 1e-6
        && turn.min(360.0 - turn) < 1e-4
}

/// The map `key`'s base layer for `frame`: a gpui-maps view that takes the
/// map's gestures -- drag with its glide, pinch, wheel, turn -- and looks
/// where `frame` does whenever the map is moved from outside (its view, its
/// zoom buttons). `Draw` has no `&mut App`, so the view is made and moved
/// when the layer is laid out.
pub(super) fn layer(
    provider: MapProvider,
    key: &str,
    frame: Frame,
    moved: Option<Data>,
    sender: Sender,
) -> Div {
    div().absolute().top_0().left_0().size_full().child(Layer {
        provider,
        key: key.to_owned(),
        frame,
        moved,
        sender,
        child: None,
    })
}

/// gpui-maps' view of the map `key` changed: if its camera moved, the user
/// moved it, and the map follows -- kept where they left it, its view told,
/// its pins and routes drawn again.
fn camera_moved(key: &str, map: &Entity<MapView>, window: AnyWindowHandle, cx: &mut App) {
    let pose = camera_pose(map.read(cx));
    let pitch = map.read(cx).camera().pitch;
    let change = VIEWS.with_borrow_mut(|views| {
        let kept = views.get_mut(key)?;
        let tilted = (kept.seen_pitch - pitch).abs() > 1e-5;
        kept.seen_pitch = pitch;
        if kept.seen.is_some_and(|seen| same(seen, pose)) {
            // Only tilted: its pins and routes move, the place it looks at
            // does not.
            return tilted.then_some(None);
        }
        kept.seen = Some(pose);
        let frame = view::adopt::adopt(key, pose.0, pose.1, pose.2, pose.3)?;
        kept.asked = Some(pose_of(frame));
        Some(Some((frame, kept.moved.clone(), kept.sender.clone())))
    });
    let Some(told) = change else {
        return;
    };
    let key = key.to_owned();
    let _ = window.update(cx, |_, window, cx| {
        if let Some((frame, moved, sender)) = told {
            tell(&key, moved.as_ref(), frame, &sender, window, cx);
        }
        redraw(&key, window, cx);
    });
}

/// gpui-maps' camera for the map `key`, while it is tilted: its pins and
/// routes are placed with it. Flat, the map's own arithmetic places them.
pub(super) fn tilt(key: &str, cx: &App) -> Tilt {
    VIEWS
        .with_borrow(|views| {
            let camera = *views.get(key)?.view.upgrade()?.read(cx).camera();
            (camera.pitch.abs() > 1e-4).then_some(camera)
        })
        .map_or_else(Tilt::default, |camera| Tilt(Some(camera)))
}

/// A new gpui-maps view for `layer`, looking where its frame does. `None`,
/// and a warning logged, when it cannot be made.
fn make(layer: &Layer, window: &Window, cx: &mut App) -> Option<Held> {
    let (provider, key, frame) = (layer.provider, layer.key.as_str(), layer.frame);
    let loaded = basemaps::load(basemap(provider))
        .inspect_err(|error| tracing::warn!("ui.map: {}: {error}", provider.name()))
        .ok()?;
    let [lat, lon] = frame.middle();
    let rt = runtime()?;
    let map = cx.new(|_| {
        let mut map = MapView::new(
            lon,
            lat,
            frame.zoom - 1.0,
            frame.size.0,
            frame.size.1,
            loaded.style,
            loaded.source,
            rt.clone(),
        );
        // Under the map's own pins, lines and buttons, not over them.
        map.set_present_in_scene(true);
        // The map draws its provider's credit itself.
        map.set_credits_by_host(true);
        map
    });
    if let Some(sprite) = loaded.sprite {
        basemaps::spawn_sprites(map.downgrade(), sprite, None, rt, cx);
    }
    let (watched, handle) = (key.to_owned(), window.window_handle());
    let watch = cx.observe(&map, move |map, cx| {
        camera_moved(&watched, &map, handle, cx)
    });
    VIEWS.with_borrow_mut(|views| {
        // What is known of views since dropped goes with them.
        views.retain(|_, kept| kept.view.upgrade().is_some());
        views.insert(key.to_owned(), Kept {
            view: map.downgrade(),
            seen: None,
            seen_pitch: 0.0,
            asked: None,
            moved: None,
            sender: layer.sender.clone(),
        })
    });
    Some(Held {
        provider,
        view: map,
        _watch: watch,
    })
}

/// The view for `layer`, held in its element state `id`: made on first use
/// or when the provider changes, its camera put where the frame looks when
/// the map was moved from outside. `None` when it cannot be made: the map
/// shows no tiles, only its pins.
fn view(
    layer: &Layer,
    id: &GlobalElementId,
    window: &mut Window,
    cx: &mut App,
) -> Option<Entity<MapView>> {
    let (provider, key, frame) = (layer.provider, layer.key.as_str(), layer.frame);
    let map = window.with_element_state(id, |held: Option<Option<Held>>, window| {
        let held = match held.flatten() {
            Some(held) if held.provider == provider => Some(held),
            _ => make(layer, window, cx),
        };
        (held.as_ref().map(|held| held.view.clone()), held)
    })?;
    let ask = VIEWS.with_borrow_mut(|views| {
        let kept = views.get_mut(key)?;
        kept.moved = layer.moved.clone();
        kept.sender = layer.sender.clone();
        let pose = pose_of(frame);
        let ask = !kept.asked.is_some_and(|asked| same(asked, pose));
        if ask {
            kept.asked = Some(pose);
        }
        ask.then_some(pose)
    });
    if let Some((lat, lon, zoom, bearing)) = ask {
        map.update(cx, |map, cx| {
            map.jump_to(CameraTarget {
                center: Some((lon, lat)),
                zoom: Some(zoom - 1.0),
                bearing: Some(bearing.to_radians() as f32),
                ..Default::default()
            });
            cx.notify();
        });
        // Where it went is no move of the user's.
        let seen = camera_pose(map.read(cx));
        VIEWS.with_borrow_mut(|views| {
            if let Some(kept) = views.get_mut(key) {
                kept.seen = Some(seen);
            }
        });
    }
    Some(map)
}

/// Lays out the gpui-maps view, filling the box.
struct Layer {
    provider: MapProvider,
    key: String,
    frame: Frame,
    moved: Option<Data>,
    sender: Sender,
    child: Option<AnyElement>,
}

impl IntoElement for Layer {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Layer {
    type PrepaintState = ();
    type RequestLayoutState = ();

    fn id(&self) -> Option<ElementId> {
        Some(ElementId::Name(self.key.clone().into()))
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let map = id.and_then(|id| view(self, id, window, cx));
        let mut child = div().size_full().children(map).into_any_element();
        let child_layout = child.request_layout(window, cx);
        self.child = Some(child);
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [child_layout], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        // A map laid out wider than its view shows (clipped by a parent):
        // gpui-maps' mark stands in the part on screen.
        let shown = bounds.intersect(&window.content_mask().bounds);
        let left = f32::from(shown.origin.x - bounds.origin.x).max(0.0);
        let map = VIEWS.with_borrow(|views| views.get(&self.key)?.view.upgrade());
        if let Some(map) = map {
            map.update(cx, |map, _| map.set_chrome_left_inset(left));
        }
        if let Some(child) = &mut self.child {
            child.prepaint(window, cx);
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(child) = &mut self.child {
            child.paint(window, cx);
        }
    }
}
