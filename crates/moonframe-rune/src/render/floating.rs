//! Popovers and overlays. Each draws its content at the window's root, after
//! everything else, and sends `dismiss` on a click outside or Escape.

use {
    super::{Painter, Place},
    crate::{Ui, keys::Layer, style::looked},
    moonframe_core::{Anchor, DEMO, Data, Keys, Overlay, Part, Placement, Popover, Side, States},
    rune::{Align, Color, El, Side as RuneSide, col, row},
};

/// Dims the window behind an overlay that asks for it.
const BACKDROP: f32 = 0.4;

/// `dismiss` for the mod `to`, as Rune's modal calls it.
fn dismissed(owner: &str, to: Option<&str>, dismiss: Option<&Data>) -> impl Fn(&mut Ui) + 'static {
    let (owner, to) = (owner.to_owned(), to.unwrap_or(owner).to_owned());
    let dismiss = dismiss.cloned();
    move |ui: &mut Ui| {
        if let Some(msg) = &dismiss {
            ui.send(&owner, &to, msg.clone());
        }
    }
}

/// Fills what it is in, and sends `dismiss` on a click there.
fn click_outside(dismiss: impl Fn(&mut Ui) + 'static) -> El {
    rune::el()
        .pin(Some(0.), Some(0.), Some(0.), Some(0.))
        .on_press(dismiss)
}

impl<'w> Painter<'w> {
    /// Notes an open layer's keys: the last drawn is the topmost.
    fn layer(&self, place: &Place<'w>, to: Option<&str>, keys: &Keys, dismiss: Option<&Data>) {
        self.ui.layers.borrow_mut().push(Layer {
            owner: place.owner.to_owned(),
            to: to.unwrap_or(place.owner).to_owned(),
            keys: keys.clone(),
            dismiss: dismiss.cloned(),
        });
    }

    pub(super) fn popover(&self, popover: &'w Popover, place: &Place<'w>) -> El {
        let key = format!("{}-anchor", place.path);
        let anchor = row()
            .key(key.clone())
            .children(self.nodes(&popover.anchor, &place.child(0)));
        if !popover.open {
            return anchor;
        }
        let look = self.look(Part::Popover, &popover.look, States::default(), place);
        let side = match popover.side {
            Side::Top => RuneSide::Top,
            Side::Bottom => RuneSide::Bottom,
            Side::Left => RuneSide::Left,
            Side::Right => RuneSide::Right,
        };
        let align = match popover.align {
            Anchor::Start => Align::Start,
            Anchor::End => Align::End,
        };
        let content = col()
            .key(format!("{}-content", place.path))
            .child(self.node(&popover.content, &place.child(1)));
        let content = looked(content, &look, None, self.palette);
        let content = match popover.width {
            Some(width) => content.w(width),
            None => content,
        };
        self.layer(
            place,
            popover.to.as_deref(),
            &popover.keys,
            popover.dismiss.as_ref(),
        );
        let content = content.anchored(key, side, align, popover.gap);
        let dismiss = dismissed(place.owner, popover.to.as_deref(), popover.dismiss.as_ref());
        // Suggestions under a field leave the keyboard with it: a modal would
        // take the focus the field is typing with.
        let content = if popover.anchor_clicks {
            content
        } else if place.owner.starts_with(DEMO) {
            self.stage.borrow_mut().push(click_outside(dismiss));
            content.on_press(|_: &mut Ui| {})
        } else {
            content.modal(true, dismiss)
        };
        self.floating.borrow_mut().push(content);
        anchor
    }

    pub(super) fn overlay(&self, overlay: &'w Overlay, place: &Place<'w>) -> El {
        if !overlay.open {
            return rune::el();
        }
        let look = self.look(Part::Overlay, &overlay.look, States::default(), place);
        let content = col()
            .key(format!("{}-content", place.path))
            .child(self.node(&overlay.content, &place.child(0)));
        let mut content = looked(content, &look, None, self.palette);
        if let Some(width) = overlay.width {
            content = content.w(width);
        }
        if let Some(height) = overlay.height {
            content = content.h(height);
        }
        if overlay.place == Placement::Bottom {
            content = content.w_full();
        }
        let layer = row()
            .key(format!("{}-layer", place.path))
            .pin(Some(0.), Some(0.), Some(0.), Some(0.))
            .padding(16.);
        let layer = if overlay.backdrop {
            layer.fill(Color::BLACK.alpha(BACKDROP))
        } else {
            layer
        };
        let layer = match overlay.place {
            Placement::Center => layer.items_center().justify_center(),
            Placement::Top => layer.items_start().justify_center(),
            // Sheets along an edge are as tall or as wide as the window.
            Placement::Left => layer.items_stretch().justify_start(),
            Placement::Right => layer.items_stretch().justify_end(),
            Placement::Bottom => layer.items_end().justify_center(),
            Placement::TopStart => layer.items_start().justify_start(),
            Placement::TopEnd => layer.items_start().justify_end(),
            Placement::BottomStart => layer.items_end().justify_start(),
            Placement::BottomEnd => layer.items_end().justify_end(),
        };
        self.layer(
            place,
            overlay.to.as_deref(),
            &overlay.keys,
            overlay.dismiss.as_ref(),
        );
        let dismiss = dismissed(place.owner, overlay.to.as_deref(), overlay.dismiss.as_ref());
        // A demo's dialog is a preview: it stays on the stage, and the
        // browser's list beside it goes on answering.
        if place.owner.starts_with(DEMO) {
            let layer = layer
                .child(click_outside(dismiss))
                .child(content.on_press(|_: &mut Ui| {}));
            self.stage.borrow_mut().push(layer);
        } else {
            self.floating
                .borrow_mut()
                .push(layer.child(content).modal(true, dismiss));
        }
        rune::el()
    }
}
