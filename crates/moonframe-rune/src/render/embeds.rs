//! Elements whose content comes from outside the view: web pages, video and
//! street maps. Their workings are in `crate::{browser, video, map}`.

use {
    super::{Painter, Place},
    crate::{
        Ui,
        browser::Wanted,
        map::{self, Camera},
        style::looked,
    },
    moonframe_core::{Browser, Map, Part, States, Token, Video, world},
    rune::{Drag, El, LineCap, Shape, SurfaceFit, Tx, WheelEvent, canvas, el, text},
};

const TILE: f64 = 256.;
/// Where a map opens before it has been laid out.
const FIRST_BOX: (f32, f32) = (800., 500.);
const PIN: f32 = 12.;

impl<'w> Painter<'w> {
    pub(super) fn browser(&self, node: &'w Browser, place: &Place<'w>) -> El {
        let look = self.look(Part::Browser, &node.look, States::default(), place);
        let key = format!("{}/{}", place.owner, node.id);
        // Noted first: a page can be built as the window opens, before any
        // view runs again, and a window not on screen runs none.
        self.ui
            .browsers
            .wanted
            .borrow_mut()
            .insert(key.clone(), Wanted {
                owner: place.owner.to_owned(),
                url: node.url.clone(),
                navigate: node.navigate.clone(),
                loading: node.loading.clone(),
            });
        if !self.ui.browsers.can_show() {
            return self.not_here_named("ui.browser", place);
        }
        // Pages are built between frames: an idle window needs asking for one.
        if !self.ui.browsers.built(&key)
            && let Some(wake) = &self.ui.waker
        {
            wake();
        }
        let element = el()
            .key(place.path.clone())
            .grow(1.)
            .min_w(0.)
            .min_h(0.)
            .on_screen_layout(move |ui: &mut Ui, rect: [f32; 4]| ui.browsers.place(&key, rect));
        looked(element, &look, None, self.palette)
    }

    pub(super) fn video(&self, node: &'w Video, place: &Place<'w>) -> El {
        let look = self.look(Part::Video, &node.look, States::default(), place);
        let pixels = self.ui.videos.pixels(&node.source);
        let element = el()
            .key(place.path.clone())
            .pixels_fit(&pixels, SurfaceFit::Fit);
        let element = match node.width {
            Some(width) => element.w(width),
            None => element.w_full(),
        };
        let element = match node.height {
            Some(height) => element.h(height),
            None => element.grow(1.).min_h(0.),
        };
        let element = looked(element, &look, None, self.palette);
        self.extras(element, &node.extras, place)
    }

    pub(super) fn map(&self, node: &'w Map, place: &Place<'w>) -> El {
        let look = self.look(Part::Map, &node.look, States::default(), place);
        let key = format!("{}/{}", place.owner, place.path);
        let size = self
            .ui
            .map_tiles
            .boxes
            .borrow()
            .get(&key)
            .copied()
            .unwrap_or((
                node.width.unwrap_or(FIRST_BOX.0),
                node.height.unwrap_or(FIRST_BOX.1),
            ));
        let moved = self.ui.map_tiles.cameras.borrow().get(&key).copied();
        let camera = map::camera(node, moved, size);
        let mut element = el()
            .key(place.path.clone())
            .clip_rounded(look.base.radius.map_or(0., |radius| radius.top_left));
        element = element.fill(self.palette.token(Token::Muted));
        element = self.tiles(element, node, camera, size);
        element = element.child(self.overlays(node, camera, size, place));
        let source = node.provider.source();
        element = element.child(
            text(source.attribution)
                .font(place.ink.text_style())
                .color(self.palette.token(Token::MutedForeground))
                .pin(None, None, Some(6.), Some(4.)),
        );
        let (laid, panned, zoomed, told) = (key.clone(), key.clone(), key.clone(), key);
        let owner = place.owner.to_owned();
        let to = node.to.clone().unwrap_or_else(|| owner.clone());
        let moved_msg = node.moved.clone();
        element = element
            .on_layout(move |ui: &mut Ui, rect: [f32; 4]| {
                ui.map_tiles
                    .boxes
                    .borrow_mut()
                    .insert(laid.clone(), (rect[2], rect[3]));
            })
            .on_drag(move |ui: &mut Ui, drag: &mut Drag, tx: &mut Tx| {
                ui.pan_map(&panned, camera, drag);
                if Ui::drag_ended(drag)
                    && let Some(msg) = &moved_msg
                {
                    ui.map_moved(&told, &owner, &to, msg, tx);
                }
            })
            .on_wheel(move |ui: &mut Ui, event: WheelEvent, _: &mut Tx| {
                ui.zoom_map(&zoomed, camera, &event);
                true
            });
        let element = match node.width {
            Some(width) => element.w(width),
            None => element.w_full(),
        };
        let element = match node.height {
            Some(height) => element.h(height),
            None => element,
        };
        let element = if node.grow {
            element.grow(1.).min_h(0.).min_w(0.)
        } else {
            element
        };
        looked(element, &look, None, self.palette)
    }

    /// The tiles that show, each placed from the map's middle.
    fn tiles(
        &self,
        mut element: El,
        node: &Map,
        camera: Camera,
        (width, height): (f32, f32),
    ) -> El {
        let (Some(wake), source) = (self.ui.waker.as_ref(), node.provider.source()) else {
            return element;
        };
        let level = camera.zoom.floor().clamp(0., f64::from(source.max_zoom));
        let z = level as u32;
        let count = 2f64.powf(level);
        let tile = TILE * 2f64.powf(camera.zoom - level);
        let (cx, cy) = world(camera.center[0], camera.center[1]);
        let left = cx * count * tile - f64::from(width) / 2.;
        let top = cy * count * tile - f64::from(height) / 2.;
        let (first_x, first_y) = ((left / tile).floor() as i64, (top / tile).floor() as i64);
        let (last_x, last_y) = (
            ((left + f64::from(width)) / tile).floor() as i64,
            ((top + f64::from(height)) / tile).floor() as i64,
        );
        let wrap = count as i64;
        for y in first_y.max(0)..=last_y.min(wrap - 1) {
            for x in first_x..=last_x {
                let column = x.rem_euclid(wrap) as u32;
                let url = map::address(source.url, node.key.as_deref(), (z, column, y as u32));
                let Some(image) = self.ui.map_tiles.tile(&url, wake) else {
                    continue;
                };
                element = element.child(
                    el().absolute(
                        (x as f64 * tile - left) as f32,
                        (y as f64 * tile - top) as f32,
                    )
                    .size(tile as f32, tile as f32)
                    .pixels(image.pixels()),
                );
            }
        }
        element
    }

    /// Routes and pins, over the tiles.
    fn overlays(
        &self,
        node: &'w Map,
        camera: Camera,
        (width, height): (f32, f32),
        place: &Place<'w>,
    ) -> El {
        let world_size = TILE * 2f64.powf(camera.zoom);
        let (cx, cy) = world(camera.center[0], camera.center[1]);
        let at = |lat: f64, lon: f64| {
            let (x, y) = world(lat, lon);
            [
                ((x - cx) * world_size) as f32 + width / 2.,
                ((y - cy) * world_size) as f32 + height / 2.,
            ]
        };
        let accent = self.palette.token(Token::Accent);
        let mut shape = Shape::new();
        for route in &node.routes {
            let color = route
                .color
                .as_ref()
                .map_or(accent, |color| self.palette.color(color));
            let points: Vec<[f32; 2]> = route
                .places()
                .iter()
                .map(|place| at(place[0], place[1]))
                .collect();
            shape = shape.stroke(points, route.stroke, color);
        }
        let mut layer = canvas(shape).pin(Some(0.), Some(0.), Some(0.), Some(0.));
        for (index, pin) in node.pins.iter().enumerate() {
            let [x, y] = at(pin.lat, pin.lon);
            let color = pin
                .color
                .as_ref()
                .map_or(accent, |color| self.palette.color(color));
            let dot = canvas(Shape::new().line(
                [PIN / 2., PIN / 2.],
                [PIN / 2., PIN / 2.],
                PIN,
                LineCap::Round,
                color,
            ))
            .absolute(x - PIN / 2., y - PIN / 2.)
            .size(PIN, PIN);
            let dot = match node.select.clone() {
                Some(msg) => {
                    let owner = place.owner.to_owned();
                    let to = node.to.clone().unwrap_or_else(|| owner.clone());
                    dot.on_press(move |ui: &mut Ui| {
                        let number = i64::try_from(index + 1).unwrap_or(i64::MAX);
                        let note = moonframe_core::Markdown::note(&msg, [("pin", number.into())]);
                        ui.send(&owner, &to, note);
                    })
                },
                None => dot,
            };
            layer = layer.child(dot);
        }
        layer
    }
}
