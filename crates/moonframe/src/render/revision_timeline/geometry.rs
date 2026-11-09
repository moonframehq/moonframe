//! The dock's lens expands adjacent slots as well as the ticks themselves.
//! Hit testing uses these same painted centers, never the unexpanded ruler.

use moonframe_core::TimelineMetrics;

#[derive(Clone, Copy, Debug)]
pub(super) struct Tick {
    pub index: usize,
    pub y: f32,
    pub influence: f32,
    pub slot: f32,
}

pub(super) fn capacity(height: f32, metrics: &TimelineMetrics) -> usize {
    ((height - 40.0).max(0.0) / metrics.spacing)
        .floor()
        .max(1.0) as usize
}

pub(super) fn ticks(
    len: usize,
    start: usize,
    height: f32,
    focus: f32,
    strength: f32,
    metrics: &TimelineMetrics,
) -> Vec<Tick> {
    let count = capacity(height, metrics).min(len.saturating_sub(start));
    let mut ticks: Vec<_> = (start..start + count)
        .map(|index| {
            let distance = ((index as f32 - focus).abs() / metrics.radius).min(1.0);
            Tick {
                index,
                y: 0.0,
                slot: 0.0,
                influence: (1.0 + (distance * std::f32::consts::PI).cos()) * 0.5 * strength,
            }
        })
        .collect();
    let spread = metrics.spread * (metrics.magnification - 1.0) / 3.5;
    let total: f32 = ticks.iter().map(|tick| 1.0 + spread * tick.influence).sum();
    let margin = 20.0_f32.min(height.max(0.0) / 2.0);
    let space = (height - 2.0 * margin).max(0.0);
    let mut cursor = margin;
    for tick in &mut ticks {
        let slot = space * (1.0 + spread * tick.influence) / total;
        tick.slot = slot;
        tick.y = cursor + slot / 2.0;
        cursor += slot;
    }
    ticks
}

pub(super) fn nearest(ticks: &[Tick], y: f32) -> Option<usize> {
    ticks
        .iter()
        .min_by(|a, b| (a.y - y).abs().total_cmp(&(b.y - y).abs()))
        .map(|tick| tick.index)
}

/// Continuous position under the pointer; no snapping as it crosses a tick.
pub(super) fn position(ticks: &[Tick], y: f32) -> Option<f32> {
    let first = ticks.first()?;
    for pair in ticks.windows(2) {
        if y <= pair[1].y {
            let fraction = ((y - pair[0].y) / (pair[1].y - pair[0].y).max(0.001)).clamp(0.0, 1.0);
            return Some(pair[0].index as f32 + fraction);
        }
    }
    Some(ticks.last().unwrap_or(first).index as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dock_lens_keeps_order_bounds_and_hit_targets() {
        for height in [0.0, 32.0, 420.0, 900.0] {
            for focus in [0.0, 4.5, 35.0, 99.0] {
                let ticks = ticks(100, 0, height, focus, 1.0, &TimelineMetrics::default());
                assert!(ticks.windows(2).all(|pair| pair[0].y < pair[1].y));
                for tick in &ticks {
                    assert!((0.0..=height).contains(&tick.y));
                    assert_eq!(nearest(&ticks, tick.y), Some(tick.index));
                }
            }
        }
    }

    #[test]
    fn empty_single_and_scrolled_histories() {
        let metrics = TimelineMetrics::default();
        assert!(ticks(0, 0, 420.0, 0.0, 0.0, &metrics).is_empty());
        assert_eq!(nearest(&[], 10.0), None);
        let single = ticks(1, 0, 420.0, 0.0, 1.0, &metrics);
        assert_eq!(single[0].y, 210.0);
        let page = ticks(100, 90, 420.0, 95.0, 1.0, &metrics);
        assert_eq!(nearest(&page, 0.0), Some(90));
        assert_eq!(nearest(&page, 500.0), Some(99));
        assert_eq!(position(&page, (page[2].y + page[3].y) / 2.0), Some(92.5));
    }

    #[test]
    fn configured_lenses_keep_targets_ordered_and_can_disable_expansion() {
        for radius in [1.0, 20.0] {
            for magnification in [1.0, 10.0] {
                for spread in [0.0, 5.0] {
                    let metrics = TimelineMetrics {
                        radius,
                        magnification,
                        spread,
                        spacing: 4.0,
                        ..Default::default()
                    };
                    let rest = ticks(100, 0, 400.0, 8.0, 0.0, &metrics);
                    let expanded = ticks(100, 0, 400.0, 8.0, 1.0, &metrics);
                    assert!(expanded.windows(2).all(|pair| pair[0].y < pair[1].y));
                    if spread == 0.0 || magnification == 1.0 {
                        assert_eq!(
                            rest.iter().map(|tick| tick.y).collect::<Vec<_>>(),
                            expanded.iter().map(|tick| tick.y).collect::<Vec<_>>()
                        );
                    }
                }
            }
        }
        assert!(
            capacity(400.0, &TimelineMetrics {
                spacing: 20.0,
                ..Default::default()
            }) < capacity(400.0, &TimelineMetrics::default())
        );
    }
}
