//! `ui.video`: a box the host fills with frames, or a line saying why not.

use {
    super::{
        Cx, Draw, Painter, Place,
        elements::with_extras,
        styling::{LookExt as _, StyleExt as _},
    },
    gpui_kit::{component::ActiveTheme, *},
    moonframe_core::{Part, Video},
};

/// What a preview, a build without it, or another system shows.
fn note(frame: Stateful<Div>, text: String, cx: Cx) -> Stateful<Div> {
    frame
        .items_center()
        .justify_center()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(text)
}

impl<'w> Draw<'w> for Video {
    fn draw(&'w self, painter: &mut Painter<'w, '_>, place: &Place<'w>, cx: Cx) -> AnyElement {
        let look = painter.look(Part::Video, &self.look, false, place);
        let theme = cx.theme();
        let mut frame = div()
            .id(place.id())
            .relative()
            .flex()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .look(&look, theme)
            .edges(look.base.border, &look.base, theme)
            .padded(&look.base.padding);
        match (self.width, self.height) {
            (None, None) => frame = frame.size_full().flex_1(),
            (width, height) => {
                if let Some(width) = width {
                    frame = frame.w(px(width));
                }
                if let Some(height) = height {
                    frame = frame.h(px(height));
                }
            },
        }
        let frame = if painter.preview {
            note(frame, format!("video from `{}`", self.source), cx)
        } else if cfg!(all(feature = "video", target_os = "macos")) {
            frame.child(surface(self, place))
        } else {
            note(
                frame,
                "video needs macOS and the `video` feature".to_owned(),
                cx,
            )
        };
        with_extras(frame, &self.extras, painter, place, cx).into_any_element()
    }
}

#[cfg(all(feature = "video", target_os = "macos"))]
fn surface(video: &Video, place: &Place<'_>) -> impl IntoElement {
    let (video, owner) = (video.clone(), place.owner.to_owned());
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            if let Err(problem) = crate::video::paint(&owner, &video, bounds, window) {
                tracing::warn!("{owner}: ui.video: {problem}");
            }
        },
    )
    .size_full()
}

#[cfg(not(all(feature = "video", target_os = "macos")))]
fn surface(_: &Video, _: &Place<'_>) -> impl IntoElement {
    div()
}
