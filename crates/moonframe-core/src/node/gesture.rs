//! What a row or column hears from a trackpad beyond scrolls and pinches:
//! two fingers turning (`rotate`), a two-finger double tap (`smart_zoom`),
//! a swipe (`swipe`) and a firm press (`force`). Each message is the mod's
//! own `msg` with the event's fields added, as [`with_fields`] makes it.
//! Hosts without a trackpad's gestures simply never send them.

use {
    super::{
        pointer::{Held, Travel, number, travel_fields},
        *,
    },
    std::collections::BTreeMap,
};

/// A row's or column's gesture listeners. All unset: it hears none.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Gestures {
    /// Sent as two fingers turn over it.
    pub rotate: Option<Data>,
    /// Sent on a two-finger double tap over it.
    pub smart_zoom: Option<Data>,
    /// Sent when a swipe over it ends.
    pub swipe: Option<Data>,
    /// Sent when a press over it goes on to a force click.
    pub force: Option<Data>,
}

impl Gestures {
    pub fn is_empty(&self) -> bool {
        self.rotate.is_none()
            && self.smart_zoom.is_none()
            && self.swipe.is_none()
            && self.force.is_none()
    }

    /// What `rotate` sends: `{ x, y, width, height, angle, phase, ... }`,
    /// `angle` the degrees turned since the last message, clockwise.
    pub fn rotate_message(
        &self,
        at: &Travel,
        angle: f32,
        phase: GesturePhase,
        held: Held,
    ) -> Option<Data> {
        let mut fields = fields(at, held);
        fields.insert("angle".to_owned(), number((angle * 100.0).round() / 100.0));
        fields.insert("phase".to_owned(), Data::String(phase.name().to_owned()));
        Some(with_fields(self.rotate.as_ref()?, fields))
    }

    /// What `smart_zoom` sends: `{ x, y, width, height, ... }`.
    pub fn smart_zoom_message(&self, at: &Travel, held: Held) -> Option<Data> {
        Some(with_fields(self.smart_zoom.as_ref()?, fields(at, held)))
    }

    /// What `swipe` sends: `{ x, y, width, height, direction, ... }`.
    pub fn swipe_message(&self, at: &Travel, direction: Swipe, held: Held) -> Option<Data> {
        let mut fields = fields(at, held);
        fields.insert(
            "direction".to_owned(),
            Data::String(direction.name().to_owned()),
        );
        Some(with_fields(self.swipe.as_ref()?, fields))
    }

    /// What `force` sends: `{ x, y, width, height, pressure, ... }`, how
    /// hard the press is, from 0 to 1.
    pub fn force_message(&self, at: &Travel, pressure: f32, held: Held) -> Option<Data> {
        let mut fields = fields(at, held);
        fields.insert(
            "pressure".to_owned(),
            number((pressure * 100.0).round() / 100.0),
        );
        Some(with_fields(self.force.as_ref()?, fields))
    }
}

/// Where a gesture is over the box, and the keys held. A gesture travels
/// nowhere, so the press-only `dx` and `dy` stay out.
fn fields(at: &Travel, held: Held) -> BTreeMap<String, Data> {
    let mut fields = travel_fields(at);
    fields.remove("dx");
    fields.remove("dy");
    held.fields(&mut fields);
    fields
}

tokens! {
    /// How far through a gesture a message is.
    GesturePhase {
        /// The fingers began.
        Start = "start",
        /// They moved on.
        Move = "move",
        /// They lifted, or the system took the gesture over.
        End = "end",
    }
}

tokens! {
    /// Which way a `swipe` went, as a browser reads it.
    Swipe {
        /// Fingers to the right: to what came before.
        Back = "back",
        /// Fingers to the left: to what comes next.
        Forward = "forward",
        Up = "up",
        Down = "down",
    }
}
