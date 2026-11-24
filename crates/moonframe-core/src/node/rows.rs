//! A list's row heights: one for every row, or each row its own.

use std::{ops::Range, sync::Arc};

/// How tall a list's rows are.
#[derive(Clone, Debug, PartialEq)]
pub enum RowHeights {
    /// Every row this many pixels.
    Same(f32),
    /// Each row its own.
    Each(Arc<OwnHeights>),
}

/// Rows of their own heights: each as given, and where each starts, from
/// 0, then where the last ends (`count + 1` offsets). The offsets add up
/// in double precision, so a short row far down keeps its height.
#[derive(Debug, PartialEq)]
pub struct OwnHeights {
    heights: Vec<f32>,
    tops: Vec<f64>,
}

impl RowHeights {
    /// Row heights from each row's own height.
    pub fn each(heights: impl IntoIterator<Item = f32>) -> Self {
        let heights: Vec<f32> = heights.into_iter().collect();
        let mut top = 0.0;
        let mut tops = Vec::with_capacity(heights.len() + 1);
        tops.push(top);
        for height in &heights {
            top += f64::from(*height);
            tops.push(top);
        }
        Self::Each(Arc::new(OwnHeights { heights, tops }))
    }

    /// Where row `index` (from 0) starts.
    pub fn top(&self, index: usize) -> f32 {
        match self {
            Self::Same(row) => index as f32 * row,
            Self::Each(rows) => rows.tops[index.min(rows.tops.len() - 1)] as f32,
        }
    }

    /// How tall row `index` (from 0) is.
    pub fn height(&self, index: usize) -> f32 {
        match self {
            Self::Same(row) => *row,
            Self::Each(rows) => rows.heights.get(index).copied().unwrap_or_default(),
        }
    }

    /// How tall `count` rows are together.
    pub fn total(&self, count: usize) -> f32 {
        self.top(count)
    }

    /// The rows of `count` that a box `height` tall, scrolled `scrolled`
    /// down, shows: from the one at its top to the one at its bottom.
    pub fn shown(&self, scrolled: f32, height: f32, count: usize) -> Range<usize> {
        let first = self.at(scrolled, count);
        match self {
            Self::Same(row) => first..(first + (height / row).ceil() as usize + 1).min(count),
            Self::Each(_) => first..(self.at(scrolled + height, count) + 1).min(count),
        }
    }

    /// The row of `count` at `y`, from the top.
    fn at(&self, y: f32, count: usize) -> usize {
        let y = y.max(0.0);
        match self {
            Self::Same(row) => ((y / row).floor() as usize).min(count),
            Self::Each(rows) => {
                let y = f64::from(y);
                let tops = &rows.tops[..=count.min(rows.tops.len() - 1)];
                // The last offset that is not below `y` starts its row.
                tops.partition_point(|top| *top <= y)
                    .saturating_sub(1)
                    .min(count)
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::RowHeights;

    #[test]
    fn rows_of_one_height_sit_one_after_another() {
        let rows = RowHeights::Same(20.0);
        assert_eq!(
            (rows.top(3), rows.height(3), rows.total(10)),
            (60.0, 20.0, 200.0)
        );
        assert_eq!(rows.shown(0.0, 50.0, 100), 0..4);
        assert_eq!(rows.shown(45.0, 50.0, 100), 2..6);
        assert_eq!(rows.shown(190.0, 50.0, 10), 9..10);
    }

    #[test]
    fn rows_of_their_own_height_start_where_the_last_ended() {
        let rows = RowHeights::each([44.0, 60.0, 44.0, 44.0, 60.0]);
        assert_eq!(
            (rows.top(2), rows.height(1), rows.total(5)),
            (104.0, 60.0, 252.0)
        );
        assert_eq!(rows.shown(0.0, 50.0, 5), 0..2);
        assert_eq!(rows.shown(104.0, 44.0, 5), 2..4);
        assert_eq!(rows.shown(103.0, 10.0, 5), 1..3);
        // Scrolled past the end, no row shows.
        assert!(rows.shown(1000.0, 50.0, 5).is_empty());
        assert_eq!(RowHeights::each([]).shown(0.0, 50.0, 0), 0..0);
    }

    #[test]
    fn a_short_row_far_down_keeps_its_height() {
        let tall = std::iter::repeat_n(1000.0, 20_000);
        let rows = RowHeights::each(tall.chain([1.0, 1000.0]));
        assert_eq!(rows.height(20_000), 1.0);
        assert_eq!(rows.shown(20_000_000.0, 0.0, 20_002), 20_000..20_001);
    }
}
