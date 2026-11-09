//! A compact, right-aligned revision ruler with native pointer magnification.

use crate::{Data, Look, with_fields};

tokens! {
    TimelineSide {
        // keep-sorted start
        Left = "left",
        Right = "right",
        // keep-sorted end
    }
}

tokens! {
    TimelineLabels {
        // keep-sorted start
        Active = "active",
        All = "all",
        Hover = "hover",
        None = "none",
        // keep-sorted end
    }
}

/// Pixel dimensions and the continuous lens, shared by layout and painting.
#[derive(Clone, Debug, PartialEq)]
pub struct TimelineMetrics {
    pub tick_width: f32,
    pub tick_height: f32,
    pub spacing: f32,
    pub magnification: f32,
    pub radius: f32,
    pub spread: f32,
}

impl Default for TimelineMetrics {
    fn default() -> Self {
        Self {
            tick_width: 12.0,
            tick_height: 2.0,
            spacing: 9.0,
            magnification: 4.5,
            radius: 4.5,
            spread: 1.8,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RevisionTimeline {
    pub look: Look,
    pub id: Option<String>,
    /// Display order, top to bottom. Adjacent equal days form a group.
    pub entries: Vec<Revision>,
    /// Stable revision id, rather than an index that changes on insertion.
    pub selected: Option<String>,
    pub select: Option<Data>,
    pub to: Option<String>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub grow: bool,
    pub disabled: bool,
    pub metrics: TimelineMetrics,
    pub side: TimelineSide,
    pub labels: TimelineLabels,
    pub dates: bool,
    pub preview: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Revision {
    pub id: String,
    /// Caller-formatted day (for example, `Today` or `8 Nov 2026`).
    pub day: Option<String>,
    /// Time, revision name, or both.
    pub label: String,
    /// Optional body of a hover preview card.
    pub summary: Option<String>,
}

impl RevisionTimeline {
    pub fn selection_message(&self, index: usize) -> Option<Data> {
        let entry = self.entries.get(index)?;
        Some(with_fields(
            self.select.as_ref()?,
            [
                ("index".into(), Data::Integer(index as i64 + 1)),
                ("revision".into(), entry.id.clone().into()),
                (
                    "day".into(),
                    entry.day.clone().map_or(Data::Nil, Data::from),
                ),
                ("label".into(), entry.label.clone().into()),
            ]
            .into(),
        ))
    }
}
