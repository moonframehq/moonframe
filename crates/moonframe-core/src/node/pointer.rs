//! What a row or column hears from the pointer besides clicks: presses and
//! drags (`pointer`), the wheel and trackpad pinches (`wheel`), files
//! dropped on it (`drop`), things dragged inside the app (`carry` and
//! `accept`), and the cursor it shows (`cursor`). Each message is the
//! mod's own `msg` with the event's fields added, as [`with_fields`]
//! makes it.

use {super::*, std::collections::BTreeMap};

/// A row's or column's pointer listeners. All unset: it hears only clicks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pointer {
    /// Sent as a button goes down on it, as the pointer moves while it is
    /// held (even outside it), and as it comes up: see [`PointerPhase`].
    pub press: Option<Data>,
    /// Sent as the wheel turns or the trackpad scrolls or pinches over it.
    pub wheel: Option<Data>,
    /// Sent when files are dropped on it from another app, with their paths.
    pub drop: Option<Data>,
    /// Dragging it carries this to a row or column that `accept`s it: a
    /// card to another column, a row to another place in its list.
    pub carry: Option<Data>,
    /// Sent as something carried moves over it, leaves it, and is dropped
    /// on it: see [`Pointer::accept_message`].
    pub accept: Option<Data>,
    /// The mod these go to; the drawing mod when unset.
    pub to: Option<String>,
    /// Also send `hover` as the pointer moves over it with no button held,
    /// and `leave` as it goes.
    pub hover: bool,
    /// Turns a drag into a value, sent as `value` with each press message.
    pub range: Option<Range>,
    /// The cursor shown over it.
    pub cursor: Option<Cursor>,
}

impl Pointer {
    pub fn is_empty(&self) -> bool {
        self.press.is_none()
            && self.wheel.is_none()
            && self.drop.is_none()
            && self.carry.is_none()
            && self.accept.is_none()
            && self.cursor.is_none()
    }
}

tokens! {
    /// What an `accept` message reports.
    CarryPhase {
        /// Something carried moved over it.
        Over = "over",
        /// It moved off it again.
        Leave = "leave",
        /// It was dropped on it: nothing follows, so the mod clears what
        /// `over` showed.
        Drop = "drop",
    }
}

tokens! {
    /// What a `pointer` message reports.
    PointerPhase {
        /// A button went down over the element.
        Down = "down",
        /// The pointer moved while that button is held, wherever it is.
        Move = "move",
        /// The button came up, wherever the pointer is.
        Up = "up",
        /// The pointer moved over it with no button held (`hover = true`).
        Hover = "hover",
        /// The pointer left it (`hover = true`).
        Leave = "leave",
    }
}

/// A value a drag sets, so a control need not do the arithmetic: a
/// slider's thumb, a number scrubbed by dragging its label.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Range {
    pub min: f32,
    pub max: f32,
    /// The value snaps to multiples of it, from `min`.
    pub step: Option<f32>,
    /// The value when the press starts, for a relative drag.
    pub value: f32,
    /// Value per pixel dragged: a relative drag from `value`, as a number
    /// is scrubbed. Without it the value is where the pointer is along
    /// `axis`, `min` at the start and `max` at the end, as on a slider.
    pub speed: Option<f32>,
    pub axis: Axis,
}

tokens! {
    /// Which way a drag sets a [`Range`]'s value.
    Axis {
        /// Left to right.
        X = "x",
        /// Bottom to top.
        Y = "y",
    }
}

/// Where a press sits in its element, and how far it went since it began.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Travel {
    /// The fraction across and down the element's box as it was when the
    /// button went down (0 to 1 inside it, beyond while dragging outside).
    pub x: f32,
    pub y: f32,
    /// Pixels moved since the button went down.
    pub dx: f32,
    pub dy: f32,
    /// The box's size when the button went down, in pixels.
    pub width: f32,
    pub height: f32,
}

impl Range {
    /// The value a drag that went `travel` gives. Worked out in f64: an
    /// open-ended range (`min = -1e12`) would lose a small value to f32's
    /// rounding when snapping from `min`.
    pub fn value(&self, travel: &Travel) -> f32 {
        let (min, max, value) = (
            f64::from(self.min),
            f64::from(self.max),
            f64::from(self.value),
        );
        let (low, high) = if min <= max {
            (min, max)
        } else {
            (max, min)
        };
        let raw = match (self.speed.map(f64::from), self.axis) {
            (Some(speed), Axis::X) => value + f64::from(travel.dx) * speed,
            // Up is more.
            (Some(speed), Axis::Y) => value - f64::from(travel.dy) * speed,
            (None, Axis::X) => min + f64::from(travel.x.clamp(0.0, 1.0)) * (max - min),
            (None, Axis::Y) => min + f64::from(1.0 - travel.y.clamp(0.0, 1.0)) * (max - min),
        };
        let stepped = match self.step.map(f64::from) {
            // Snapped from the start of the range, or from zero when it has
            // no real start, so steps of 1 land on whole numbers.
            Some(step) if step > 0.0 => {
                let origin = if min.abs() >= 1e9 {
                    0.0
                } else {
                    min
                };
                origin + ((raw - origin) / step).round() * step
            },
            _ => raw,
        };
        // Steps leave float dust: 0.1 * 3 is 0.30000000000000004.
        let stepped = (stepped * 1e6).round() / 1e6;
        stepped.clamp(low, high) as f32
    }
}

tokens! {
    /// The pointer's shape over an element.
    Cursor {
        Default = "default",
        Pointer = "pointer",
        Text = "text",
        Crosshair = "crosshair",
        Grab = "grab",
        Grabbing = "grabbing",
        Move = "move",
        EwResize = "ew-resize",
        NsResize = "ns-resize",
        NwseResize = "nwse-resize",
        NeswResize = "nesw-resize",
        ColResize = "col-resize",
        RowResize = "row-resize",
        NotAllowed = "not-allowed",
    }
}

/// The keys held during a pointer event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Held {
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
    /// ⌘ on macOS, Super or the Windows key elsewhere.
    pub cmd: bool,
    /// ⌘ on macOS, Ctrl elsewhere.
    pub secondary: bool,
}

impl Held {
    fn fields(&self, fields: &mut BTreeMap<String, Data>) {
        for (name, on) in [
            ("shift", self.shift),
            ("alt", self.alt),
            ("ctrl", self.ctrl),
            ("cmd", self.cmd),
            ("secondary", self.secondary),
        ] {
            if on {
                fields.insert(name.to_owned(), Data::Bool(true));
            }
        }
    }
}

/// `msg` with `fields` added: a table `msg` gets them (its own keys give
/// way); anything else becomes `{ msg = msg, ... }`.
pub fn with_fields(msg: &Data, fields: BTreeMap<String, Data>) -> Data {
    match msg {
        Data::Map(entries) => {
            let mut entries = entries.clone();
            entries.extend(fields);
            Data::Map(entries)
        },
        other => {
            let mut fields = fields;
            fields.insert("msg".to_owned(), other.clone());
            Data::Map(fields)
        },
    }
}

fn number(value: f32) -> Data {
    // Whole numbers arrive as integers, so `value == 3` holds in Luau; the
    // rest are rounded in f64, so 0.1 arrives as 0.1, not 0.10000000149.
    if value.fract() == 0.0 && value.abs() < 1e15 {
        Data::Integer(value as i64)
    } else {
        Data::Number((f64::from(value) * 1e6).round() / 1e6)
    }
}

impl Pointer {
    /// What `press` sends for a press event: `{ phase, x, y, dx, dy, width,
    /// height, button, clicks, value?, shift?, ... }`.
    pub fn press_message(
        &self,
        phase: PointerPhase,
        travel: &Travel,
        button: &str,
        clicks: usize,
        held: Held,
    ) -> Option<Data> {
        let msg = self.press.as_ref()?;
        let mut fields = travel_fields(travel);
        fields.insert("phase".to_owned(), Data::String(phase.name().to_owned()));
        if !matches!(phase, PointerPhase::Hover | PointerPhase::Leave) {
            fields.insert("button".to_owned(), Data::String(button.to_owned()));
            fields.insert("clicks".to_owned(), Data::Integer(clicks as i64));
            if let Some(range) = &self.range {
                fields.insert("value".to_owned(), number(range.value(travel)));
            }
        }
        held.fields(&mut fields);
        Some(with_fields(msg, fields))
    }

    /// What `wheel` sends: `{ x, y, width, height, dx, dy, zoom, ... }`,
    /// `dx` and `dy` the pixels scrolled, `zoom` a pinch's change (0.1 is
    /// 10 % larger), 0 for a scroll.
    pub fn wheel_message(&self, at: &Travel, zoom: f32, held: Held) -> Option<Data> {
        let msg = self.wheel.as_ref()?;
        let mut fields = travel_fields(at);
        fields.insert("zoom".to_owned(), number(zoom));
        held.fields(&mut fields);
        Some(with_fields(msg, fields))
    }

    /// What `drop` sends: `{ paths = { ... }, x, y }`.
    pub fn drop_message(&self, paths: &[PathBuf], at: &Travel) -> Option<Data> {
        let msg = self.drop.as_ref()?;
        let mut fields = BTreeMap::new();
        fields.insert("x".to_owned(), number(at.x));
        fields.insert("y".to_owned(), number(at.y));
        let paths = paths
            .iter()
            .map(|path| Data::String(path.to_string_lossy().into_owned()))
            .collect();
        fields.insert("paths".to_owned(), Data::List(paths));
        Some(with_fields(msg, fields))
    }
}

impl Pointer {
    /// What `accept` sends as `carry`, drawn by the mod `from`, moves over
    /// it, leaves it or is dropped on it: `{ phase, carry, from, x, y,
    /// width, height }`, `x` and `y` where the pointer is in its box.
    pub fn accept_message(
        &self,
        phase: CarryPhase,
        carry: &Data,
        from: &str,
        at: &Travel,
    ) -> Option<Data> {
        let msg = self.accept.as_ref()?;
        let mut fields = travel_fields(at);
        fields.remove("dx");
        fields.remove("dy");
        fields.insert("phase".to_owned(), Data::String(phase.name().to_owned()));
        fields.insert("carry".to_owned(), carry.clone());
        fields.insert("from".to_owned(), Data::String(from.to_owned()));
        Some(with_fields(msg, fields))
    }
}

fn travel_fields(travel: &Travel) -> BTreeMap<String, Data> {
    // Fractions to four places and pixels to two keep messages short and
    // comparable without losing anything a view can show.
    let fraction = |value: f32| number((value * 10_000.0).round() / 10_000.0);
    let pixels = |value: f32| number((value * 100.0).round() / 100.0);
    BTreeMap::from([
        ("x".to_owned(), fraction(travel.x)),
        ("y".to_owned(), fraction(travel.y)),
        ("dx".to_owned(), pixels(travel.dx)),
        ("dy".to_owned(), pixels(travel.dy)),
        ("width".to_owned(), pixels(travel.width)),
        ("height".to_owned(), pixels(travel.height)),
    ])
}

/// A box at fractions of its parent, out of the flow: `area = { left =
/// 0.1, top = 0.2, width = 0.5, height = 0.3 }`. A crop frame over a
/// picture, a marker on a track; each side optional.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Area {
    pub left: Option<f32>,
    pub top: Option<f32>,
    pub width: Option<f32>,
    pub height: Option<f32>,
}
