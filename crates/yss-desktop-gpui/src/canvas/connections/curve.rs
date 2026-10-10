//! One curve supplies drawing, picking, bounds and the direction of running markers.
use gpui_kit::{Bounds, Path, PathBuilder, Pixels, Point, point, px};

#[derive(Clone, Copy, PartialEq)]
pub(super) struct Curve {
    pub delta: Point<Pixels>,
    pub from_input: bool,
}

impl Curve {
    fn bend(self) -> Pixels {
        (self.delta.x.abs() * 0.5).max(px(40.)) * if self.from_input { -1. } else { 1. }
    }

    pub fn bounds(self) -> Bounds<Pixels> {
        let bend = self.bend();
        let opposite = self.delta.x - bend;
        Bounds::from_corners(
            point(
                px(0.).min(self.delta.x).min(bend).min(opposite),
                px(0.).min(self.delta.y),
            ),
            point(
                px(0.).max(self.delta.x).max(bend).max(opposite),
                px(0.).max(self.delta.y),
            ),
        )
    }

    pub fn build(self, width: Pixels, dashes: &[f32]) -> Option<Path<Pixels>> {
        let mut path = PathBuilder::stroke(width);
        if !dashes.is_empty() {
            path = path.dash_array(&dashes.iter().copied().map(px).collect::<Vec<_>>());
        }
        let bend = self.bend();
        path.move_to(Point::default());
        path.cubic_bezier_to(
            self.delta,
            point(bend, px(0.)),
            self.delta - point(bend, px(0.)),
        );
        path.build().ok()
    }

    pub fn position(self, t: f32) -> Point<Pixels> {
        let s = 1. - t;
        let bend = point(self.bend(), px(0.));
        bend * (3. * s * s * t) + (self.delta - bend) * (3. * s * t * t) + self.delta * (t * t * t)
    }
}
