//! A `ui.image` with `zoom`, `overlay` or `measure`: the picture placed
//! by its own size in a box that clips it, things drawn over the picture
//! wherever it lands, and its size told to the mod. The box's size comes
//! from the last frame, so the first frame of a new box shows it fitted.

use {
    super::{
        Cx, Painter, Place,
        pointer::Sender,
        styling::{LookExt as _, StyleExt as _},
    },
    gpui_kit::{component::ActiveTheme, *},
    moonframe_core::{Direction, Fit, Image, Part, with_fields},
    std::{cell::RefCell, collections::HashMap, path::Path},
};

thread_local! {
    /// Each viewer's box as last laid out, by its place.
    static BOXES: RefCell<HashMap<String, Size<Pixels>>> = RefCell::new(HashMap::new());
    /// What each viewer last told its mod, so it is told only of changes.
    static TOLD: RefCell<HashMap<String, [u32; 5]>> = RefCell::new(HashMap::new());
}

/// Whether `image` needs the viewer rather than a plain picture.
pub(super) fn wanted(image: &Image) -> bool {
    image.zoom.is_some() || image.measure.is_some() || !image.overlay.is_empty()
}

pub(super) fn draw<'w>(
    image: &'w Image,
    path: &Path,
    painter: &mut Painter<'w, '_>,
    place: &Place<'w>,
    cx: Cx,
) -> AnyElement {
    let look = painter.look(Part::Image, &image.look, false, place);
    let theme = cx.theme();
    let natural = super::images::natural(path);
    let laid = BOXES.with_borrow(|boxes| boxes.get(&place.path).copied());
    let mut element = div()
        .id(place.id())
        .relative()
        .overflow_hidden()
        .flex_shrink_0()
        .look(&look, theme)
        .edges(look.base.border, &look.base, theme);
    element = match image.width {
        Some(width) => element.w(px(width)),
        None => element.w_full(),
    };
    element = match image.height {
        Some(height) => element.h(px(height)),
        None => element.h_full(),
    };
    element = element.child(measure(image, natural, place, cx));
    let shown = match (natural, laid) {
        (Some(natural), Some(laid)) => Some(rect(image, natural, laid)),
        _ => None,
    };
    let Some(shown) = shown else {
        // Not laid out yet, or not a picture: fitted, as a plain image.
        return element
            .child(
                img(path.to_owned())
                    .object_fit(ObjectFit::Contain)
                    .size_full(),
            )
            .into_any_element();
    };
    let picture = img(path.to_owned())
        .object_fit(ObjectFit::Fill)
        .absolute()
        .left(shown.origin.x)
        .top(shown.origin.y)
        .w(shown.size.width)
        .h(shown.size.height);
    let overlay = (!image.overlay.is_empty()).then(|| {
        let children = painter.children(
            &image.overlay,
            &place.part("overlay"),
            Direction::Column,
            cx,
        );
        div()
            .absolute()
            .left(shown.origin.x)
            .top(shown.origin.y)
            .w(shown.size.width)
            .h(shown.size.height)
            .flex()
            .flex_col()
            .children(children)
    });
    element.child(picture).children(overlay).into_any_element()
}

/// The zoom at which the picture fits a box, as `fit` fits it.
fn fitting(fit: Fit, natural: (f32, f32), laid: Size<Pixels>) -> f32 {
    let across = f32::from(laid.width) / natural.0;
    let down = f32::from(laid.height) / natural.1;
    match fit {
        Fit::Cover => across.max(down),
        Fit::ScaleDown => across.min(down).min(1.0),
        Fit::None => 1.0,
        Fit::Contain | Fit::Fill => across.min(down),
    }
}

/// Where the picture goes in its box, in pixels from the box's corner.
fn rect(image: &Image, natural: (f32, f32), laid: Size<Pixels>) -> Bounds<Pixels> {
    let scale = image
        .zoom
        .unwrap_or_else(|| fitting(image.fit, natural, laid));
    let (width, height) = (natural.0 * scale, natural.1 * scale);
    let [x, y] = match image.zoom {
        Some(_) => image.center.unwrap_or([0.5, 0.5]),
        None => [0.5, 0.5],
    };
    let left = f32::from(laid.width) / 2.0 - x * width;
    let top = f32::from(laid.height) / 2.0 - y * height;
    Bounds::new(point(px(left), px(top)), size(px(width), px(height)))
}

/// A canvas over the box that keeps its size, draws again when it
/// changes, and tells `measure` the picture's size and fitting zoom.
fn measure(
    image: &Image,
    natural: Option<(f32, f32)>,
    place: &Place<'_>,
    cx: Cx,
) -> impl IntoElement {
    let key = place.path.clone();
    let fit = image.fit;
    let told = image.measure.clone().map(|msg| {
        let sender = Sender {
            app: cx.entity().downgrade(),
            owner: place.owner.to_owned(),
            to: image.to.clone(),
        };
        (msg, sender)
    });
    canvas(
        move |bounds, window, cx| {
            let laid = bounds.size;
            let changed =
                BOXES.with_borrow_mut(|boxes| boxes.insert(key.clone(), laid) != Some(laid));
            if changed {
                window.refresh();
            }
            let (Some((msg, sender)), Some(natural)) = (told, natural) else {
                return;
            };
            let zoom = fitting(fit, natural, laid);
            // Whole pixels, and the zoom to a ten-thousandth, decide a change.
            let reading = [
                natural.0 as u32,
                natural.1 as u32,
                f32::from(laid.width).round() as u32,
                f32::from(laid.height).round() as u32,
                (zoom * 10_000.0).round() as u32,
            ];
            if TOLD.with_borrow_mut(|told| told.insert(key, reading) == Some(reading)) {
                return;
            }
            let fields = [
                ("width", natural.0),
                ("height", natural.1),
                ("box_width", f32::from(laid.width)),
                ("box_height", f32::from(laid.height)),
                ("fit", zoom),
            ]
            .into_iter()
            .map(|(name, value)| {
                let value = (f64::from(value) * 10_000.0).round() / 10_000.0;
                (name.to_owned(), moonframe_core::Data::Number(value))
            })
            .collect();
            let msg = with_fields(&msg, fields);
            window.defer(cx, move |window, cx| sender.send(msg, window, cx));
        },
        |_, (), _, _| {},
    )
    .absolute()
    .size_full()
}
