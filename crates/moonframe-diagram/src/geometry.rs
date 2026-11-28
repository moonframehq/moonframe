//! Points, sizes and boxes in diagram pixels: plain `f32`, so the layout
//! needs no GUI.

/// A point, in diagram pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn distance(self, other: Self) -> f32 {
        (other.x - self.x).hypot(other.y - self.y)
    }
}

/// A width and a height, in diagram pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub w: f32,
    pub h: f32,
}

impl Size {
    pub const fn new(w: f32, h: f32) -> Self {
        Self { w, h }
    }
}

/// A box: its top-left corner and its size.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    /// The box of `size` centred on `center`.
    pub fn centered(center: Point, size: Size) -> Self {
        Self::new(center.x - size.w / 2.0, center.y - size.h / 2.0, size.w, size.h)
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    pub fn center(&self) -> Point {
        Point::new(self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    pub fn size(&self) -> Size {
        Size::new(self.w, self.h)
    }

    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.x && p.x <= self.right() && p.y >= self.y && p.y <= self.bottom()
    }

    /// Whether the two boxes share any area.
    pub fn overlaps(&self, other: &Rect) -> bool {
        self.x < other.right() && other.x < self.right() && self.y < other.bottom() && other.y < self.bottom()
    }

    /// This box grown by `by` on every side.
    pub fn inflate(&self, by: f32) -> Self {
        Self::new(self.x - by, self.y - by, self.w + by * 2.0, self.h + by * 2.0)
    }

    /// The smallest box holding both.
    pub fn union(&self, other: &Rect) -> Self {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Self::new(x, y, self.right().max(other.right()) - x, self.bottom().max(other.bottom()) - y)
    }

    /// The box with its corners in any order: a marquee dragged up or left.
    pub fn from_corners(a: Point, b: Point) -> Self {
        Self::new(a.x.min(b.x), a.y.min(b.y), (a.x - b.x).abs(), (a.y - b.y).abs())
    }

    pub fn translate(&self, dx: f32, dy: f32) -> Self {
        Self::new(self.x + dx, self.y + dy, self.w, self.h)
    }

    /// Whether the segment from `a` to `b`, level or upright, passes
    /// through this box's inside (touching its edge does not count).
    pub fn crossed_by(&self, a: Point, b: Point) -> bool {
        let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
        let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
        x1 > self.x + 0.5 && x0 < self.right() - 0.5 && y1 > self.y + 0.5 && y0 < self.bottom() - 0.5
    }
}

/// The box holding every one of `rects`, or `None` for none.
pub fn bounds<'a>(rects: impl IntoIterator<Item = &'a Rect>) -> Option<Rect> {
    rects.into_iter().fold(None, |all: Option<Rect>, r| Some(all.map_or(*r, |a| a.union(r))))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn a_segment_crosses_a_box_only_through_its_inside() {
        let r = Rect::new(10.0, 10.0, 20.0, 20.0);
        assert!(r.crossed_by(Point::new(0.0, 20.0), Point::new(40.0, 20.0)));
        assert!(!r.crossed_by(Point::new(0.0, 10.0), Point::new(40.0, 10.0)));
        assert!(!r.crossed_by(Point::new(0.0, 50.0), Point::new(40.0, 50.0)));
        assert!(r.crossed_by(Point::new(20.0, 0.0), Point::new(20.0, 15.0)));
    }

    #[test]
    fn bounds_hold_every_box() {
        let all = bounds(&[Rect::new(0.0, 0.0, 10.0, 10.0), Rect::new(20.0, -5.0, 5.0, 5.0)]).unwrap();
        assert_eq!(all, Rect::new(0.0, -5.0, 25.0, 15.0));
        assert_eq!(bounds(&[]), None);
    }
}
