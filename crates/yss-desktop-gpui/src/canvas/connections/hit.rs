//! Picking uses GPUI's tessellation of the same curve at a 12px interaction width.
use super::curve::Curve;
use gpui_kit::{Path, Pixels, Point, px};

#[derive(Default)]
pub(super) struct HitPath {
    curve: Option<Curve>,
    path: Option<Path<Pixels>>,
}

impl HitPath {
    pub fn contains(&mut self, curve: Curve, position: Point<Pixels>) -> bool {
        let bounds = curve.bounds().dilate(px(6.));
        if !bounds.contains(&position) {
            return false;
        }
        if self.curve != Some(curve) {
            self.curve = Some(curve);
            self.path = curve.build(px(12.), &[]);
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
