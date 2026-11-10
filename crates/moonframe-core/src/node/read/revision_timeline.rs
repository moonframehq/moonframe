use super::*;

impl Build for RevisionTimeline {
    const KEYS: &'static [&'static [&'static str]] = &[TYPE, LOOK, &[
        "id",
        "entries",
        "selected",
        "select",
        "to",
        "width",
        "height",
        "grow",
        "disabled",
        "tick_width",
        "tick_height",
        "spacing",
        "magnification",
        "radius",
        "spread",
        "side",
        "labels",
        "dates",
        "preview",
    ]];

    fn build(_: Element, fields: &Fields, _: &mut Tree<'_>) -> Result<Self, Error> {
        let entries: Vec<Revision> = fields.get("entries")?.unwrap_or_default();
        if entries.len() > 10_000 {
            return Err(fields.cx().key("entries").fail("at most 10000 revisions"));
        }
        let mut ids = BTreeSet::new();
        for entry in &entries {
            if !ids.insert(&entry.id) {
                return Err(fields
                    .cx()
                    .key("entries")
                    .fail("revision ids must be unique"));
            }
        }
        Ok(Self {
            look: Look::from_fields(fields)?,
            id: fields.get::<PlainId>("id")?.map(|id| id.0),
            entries,
            selected: fields.get("selected")?,
            select: fields.get("select")?,
            to: fields.get("to")?,
            width: fields.number("width", PIXELS)?,
            height: fields.number("height", PIXELS)?,
            grow: fields.get("grow")?.unwrap_or(false),
            disabled: fields.get("disabled")?.unwrap_or(false),
            metrics: TimelineMetrics {
                tick_width: fields.number("tick_width", 1.0..=128.0)?.unwrap_or(12.0),
                tick_height: fields.number("tick_height", 0.5..=16.0)?.unwrap_or(2.0),
                spacing: fields.number("spacing", 4.0..=64.0)?.unwrap_or(9.0),
                magnification: fields.number("magnification", 1.0..=10.0)?.unwrap_or(4.5),
                radius: fields.number("radius", 1.0..=20.0)?.unwrap_or(4.5),
                spread: fields.number("spread", 0.0..=5.0)?.unwrap_or(1.8),
            },
            side: fields.get("side")?.unwrap_or(TimelineSide::Right),
            labels: fields.get("labels")?.unwrap_or(TimelineLabels::All),
            dates: fields.get("dates")?.unwrap_or(true),
            preview: fields.get("preview")?.unwrap_or(false),
        })
    }
}

impl Read for Revision {
    fn read(value: &Value, cx: &Cx) -> Result<Self, Error> {
        let fields = Fields::read(value, cx, &[&["id", "day", "label", "summary"]])?;
        let id: String = fields.need("id")?;
        let day: Option<String> = fields.get("day")?;
        let label: String = fields.need("label")?;
        let summary: Option<String> = fields.get("summary")?;
        for (key, text) in [
            ("id", Some(&id)),
            ("day", day.as_ref()),
            ("label", Some(&label)),
            ("summary", summary.as_ref()),
        ] {
            let Some(text) = text else {
                continue;
            };
            if text.trim().is_empty() || text.len() > 512 {
                return Err(cx
                    .key(key)
                    .fail("expected nonempty text, at most 512 bytes"));
            }
        }
        Ok(Self {
            id,
            day,
            label,
            summary,
        })
    }
}
