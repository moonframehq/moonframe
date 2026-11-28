//! What part of the diagram the view shows: the world point at its
//! top-left corner and how many screen pixels a diagram pixel takes.

use crate::geometry::{Point, Rect};

pub const MIN_ZOOM: f32 = 0.1;
pub const MAX_ZOOM: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self { x: -40.0, y: -40.0, zoom: 1.0 }
    }
}

impl Camera {
    /// A diagram point on screen, from the view's corner.
    pub fn to_screen(self, p: Point) -> Point {
        Point::new((p.x - self.x) * self.zoom, (p.y - self.y) * self.zoom)
    }

    /// A screen point, from the view's corner, in the diagram.
    pub fn to_world(self, p: Point) -> Point {
        Point::new(self.x + p.x / self.zoom, self.y + p.y / self.zoom)
    }

    pub fn rect_to_screen(self, r: Rect) -> Rect {
        let at = self.to_screen(Point::new(r.x, r.y));
        Rect::new(at.x, at.y, r.w * self.zoom, r.h * self.zoom)
    }

    /// What of the diagram a view of `size` shows.
    pub fn visible(self, w: f32, h: f32) -> Rect {
        Rect::new(self.x, self.y, w / self.zoom, h / self.zoom)
    }

    /// Zoomed to `zoom`, keeping the diagram point under `screen` still.
    pub fn zoom_at(&mut self, zoom: f32, screen: Point) {
        let zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        let world = self.to_world(screen);
        self.zoom = zoom;
        self.x = world.x - screen.x / zoom;
        self.y = world.y - screen.y / zoom;
    }

    /// Showing all of `content` in a view of `w` by `h`, with a margin, and
    /// never past 100 %.
    pub fn fit(content: Rect, w: f32, h: f32, margin: f32) -> Self {
        if content.w <= 0.0 || content.h <= 0.0 || w <= 0.0 || h <= 0.0 {
            return Self { x: content.x - margin, y: content.y - margin, zoom: 1.0 };
        }
        let zoom = ((w - margin * 2.0) / content.w)
            .min((h - margin * 2.0) / content.h)
            .clamp(MIN_ZOOM, 1.0);
        let center = content.center();
        Self { x: center.x - w / 2.0 / zoom, y: center.y - h / 2.0 / zoom, zoom }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn zooming_keeps_the_point_under_the_pointer() {
        let mut camera = Camera::default();
        let screen = Point::new(200.0, 120.0);
        let before = camera.to_world(screen);
        camera.zoom_at(2.5, screen);
        let after = camera.to_world(screen);
        assert!((before.x - after.x).abs() < 1e-3 && (before.y - after.y).abs() < 1e-3);
    }

    #[test]
    fn fit_shows_everything_and_never_magnifies() {
        let camera = Camera::fit(Rect::new(0.0, 0.0, 2000.0, 1000.0), 800.0, 600.0, 40.0);
        let shown = camera.visible(800.0, 600.0);
        assert!(shown.x <= 0.0 && shown.right() >= 2000.0, "{shown:?}");
        assert_eq!(Camera::fit(Rect::new(0.0, 0.0, 50.0, 50.0), 800.0, 600.0, 40.0).zoom, 1.0);
    }
}
