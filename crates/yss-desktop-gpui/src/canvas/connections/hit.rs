//! Picking uses GPUI's tessellation of the same curve at a 12px interaction width.
use gpui::{Bounds, Path, Pixels, Point, point, px};

#[derive(Default)]
pub(super) struct HitPath {
    delta: Option<Point<Pixels>>,
    path: Option<Path<Pixels>>,
}

impl HitPath {
    pub fn contains(&mut self, delta: Point<Pixels>, position: Point<Pixels>) -> bool {
        let bend = super::connection_bend(delta);
        let bounds = Bounds::from_corners(
            point(px(0.).min(delta.x - bend), px(0.).min(delta.y)),
            point(bend.max(delta.x), px(0.).max(delta.y)),
        )
        .dilate(px(6.));
        if !bounds.contains(&position) {
            return false;
        }
        if self.delta != Some(delta) {
            self.delta = Some(delta);
            self.path = super::build_connection(delta, px(12.));
        }
        self.path.as_ref().is_some_and(|path| {
            path.vertices.chunks_exact(3).any(|triangle| {
                let [a, b, c] = [
                    triangle[0].xy_position,
                    triangle[1].xy_position,
                    triangle[2].xy_position,
                ];
                let cross = |a: Point<Pixels>, b: Point<Pixels>, c: Point<Pixels>| {
                    let b = b - a;
                    let c = c - a;
                    f32::from(b.x) * f32::from(c.y) - f32::from(b.y) * f32::from(c.x)
                };
                if cross(a, b, c).abs() <= f32::EPSILON {
                    return false;
                }
                let signs = [
                    cross(a, b, position),
                    cross(b, c, position),
                    cross(c, a, position),
                ];
                signs.iter().all(|s| *s >= 0.) || signs.iter().all(|s| *s <= 0.)
            })
        })
    }
}
