//! One curve supplies drawing, picking, bounds and the direction of running markers.
//!
//! Strokes are flattened into short segments and emitted as triangle strips, which is
//! cheap enough to rebuild every frame; merging many strokes into one path keeps every
//! connection in a single GPU path pass.
use gpui_kit::{Bounds, Path, Pixels, Point, point, px};

/// The shortest horizontal reach of a connection in world units, so links leave and
/// enter ports horizontally even when their nodes are stacked or reversed.
const MIN_BEND: f32 = 36.;
/// Screen pixels per flattened segment.
const SEGMENT_LENGTH: f32 = 9.;

#[derive(Clone, Copy, PartialEq)]
pub(super) struct Curve {
    pub delta: Point<Pixels>,
    bend: Pixels,
}

impl Curve {
    /// A curve from the origin to `delta`, both in screen pixels at `zoom`. A curve drawn
    /// from an input leaves it leftwards.
    pub fn new(delta: Point<Pixels>, zoom: f32, from_input: bool) -> Self {
        let direction = if from_input { -1. } else { 1. };
        let forward = f32::from(delta.x) / zoom.max(f32::EPSILON) * direction;
        let reach = if forward >= 0. {
            (forward * 0.5).max(MIN_BEND)
        } else {
            // Backward links loop out further the longer they travel back.
            MIN_BEND.max(6.25 * (-forward).sqrt())
        };
        Self {
            delta,
            bend: px(reach * zoom * direction),
        }
    }

    fn controls(self) -> [Point<Pixels>; 4] {
        [
            Point::default(),
            point(self.bend, px(0.)),
            self.delta - point(self.bend, px(0.)),
            self.delta,
        ]
    }

    pub fn bounds(self) -> Bounds<Pixels> {
        let [a, b, c, d] = self.controls();
        Bounds::from_corners(
            point(
                a.x.min(b.x).min(c.x).min(d.x),
                a.y.min(b.y).min(c.y).min(d.y),
            ),
            point(
                a.x.max(b.x).max(c.x).max(d.x),
                a.y.max(b.y).max(c.y).max(d.y),
            ),
        )
    }

    pub fn visible(self, origin: Point<Pixels>, viewport: Bounds<Pixels>) -> bool {
        let mut bounds = self.bounds().dilate(px(8.));
        bounds.origin += origin;
        bounds.intersects(&viewport)
    }

    pub fn position(self, t: f32) -> Point<Pixels> {
        let s = 1. - t;
        let [_, b, c, d] = self.controls();
        b * (3. * s * s * t) + c * (3. * s * t * t) + d * (t * t * t)
    }

    fn segments(self) -> usize {
        let [a, b, c, d] = self.controls();
        let polygon = length(b - a) + length(c - b) + length(d - c);
        let estimate = (polygon + length(d - a)) / 2.;
        ((estimate / SEGMENT_LENGTH).ceil() as usize).clamp(4, 96)
    }
}

fn length(vector: Point<Pixels>) -> f32 {
    f32::from(vector.x).hypot(f32::from(vector.y))
}

/// A dash pattern in screen pixels, shifted along the stroke by `phase`.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct Dash {
    pub on: f32,
    pub off: f32,
    pub phase: f32,
}

/// Scratch space for flattening curves, reused across the strokes of a frame.
#[derive(Default)]
pub(in crate::canvas) struct Tessellator {
    points: Vec<Point<Pixels>>,
    normals: Vec<Point<Pixels>>,
}

impl Tessellator {
    fn flatten(&mut self, curve: Curve, origin: Point<Pixels>) {
        let segments = curve.segments();
        self.points.clear();
        self.points.extend(
            (0..=segments).map(|index| origin + curve.position(index as f32 / segments as f32)),
        );
    }

    /// The distance from `target` to the curve, both relative to the curve's origin.
    pub(super) fn distance(&mut self, curve: Curve, target: Point<Pixels>) -> Pixels {
        self.flatten(curve, Point::default());
        let distance = self
            .points
            .windows(2)
            .map(|segment| segment_distance(target, segment[0], segment[1]))
            .fold(f32::MAX, f32::min);
        px(distance)
    }

    /// Appends a circular arc stroke around `center`, starting at `start` radians and
    /// sweeping `sweep` radians clockwise.
    pub fn arc(
        &mut self,
        center: Point<Pixels>,
        radius: Pixels,
        (start, sweep): (f32, f32),
        width: Pixels,
        path: &mut Option<Path<Pixels>>,
    ) {
        let segments = ((f32::from(radius) * sweep / 3.).ceil() as usize).clamp(8, 48);
        self.points.clear();
        self.points.extend((0..=segments).map(|index| {
            let angle = start + sweep * index as f32 / segments as f32;
            center + point(radius * angle.cos(), radius * angle.sin())
        }));
        self.stroke_points(width, path);
    }

    fn stroke_points(&mut self, width: Pixels, path: &mut Option<Path<Pixels>>) {
        let half = f32::from(width) / 2.;
        let points = &self.points;
        let last = points.len() - 1;
        self.normals.clear();
        self.normals.extend((0..points.len()).map(|index| {
            side(
                points[index.saturating_sub(1)],
                points[(index + 1).min(last)],
                half,
            )
        }));
        for index in 0..last {
            push_quad(
                path,
                points[index],
                points[index + 1],
                self.normals[index],
                self.normals[index + 1],
            );
        }
    }

    /// Appends the stroke of `curve` at `origin` to `path`, leaving out dash gaps and
    /// every stretch for which `hidden` holds.
    pub(super) fn stroke(
        &mut self,
        curve: Curve,
        origin: Point<Pixels>,
        width: Pixels,
        dash: Option<Dash>,
        hidden: &dyn Fn(Point<Pixels>) -> bool,
        path: &mut Option<Path<Pixels>>,
    ) {
        self.flatten(curve, origin);
        let half = f32::from(width) / 2.;
        let points = &self.points;
        let last = points.len() - 1;
        let Some(dash) = dash else {
            self.normals.clear();
            self.normals.extend((0..points.len()).map(|index| {
                side(
                    points[index.saturating_sub(1)],
                    points[(index + 1).min(last)],
                    half,
                )
            }));
            for index in 0..last {
                let (a, b) = (points[index], points[index + 1]);
                if !hidden((a + b) / 2.) {
                    push_quad(path, a, b, self.normals[index], self.normals[index + 1]);
                }
            }
            return;
        };
        let period = dash.on + dash.off;
        let mut travelled = dash.phase.rem_euclid(period);
        for segment in points.windows(2) {
            let (a, b) = (segment[0], segment[1]);
            let span = length(b - a);
            if span <= f32::EPSILON {
                continue;
            }
            let normal = side(a, b, half);
            let mut start = 0.;
            while start < span {
                let within = travelled % period;
                let (on, run) = if within < dash.on {
                    (true, dash.on - within)
                } else {
                    (false, period - within)
                };
                let end = (start + run).min(span);
                if on {
                    let from = a + (b - a) * (start / span);
                    let to = a + (b - a) * (end / span);
                    if !hidden((from + to) / 2.) {
                        push_quad(path, from, to, normal, normal);
                    }
                }
                travelled += end - start;
                start = end;
            }
        }
    }
}

fn side(from: Point<Pixels>, to: Point<Pixels>, half: f32) -> Point<Pixels> {
    let direction = to - from;
    let span = length(direction);
    if span <= f32::EPSILON {
        Point::default()
    } else {
        point(-direction.y, direction.x) * (half / span)
    }
}

fn segment_distance(target: Point<Pixels>, a: Point<Pixels>, b: Point<Pixels>) -> f32 {
    let edge = b - a;
    let squared = f32::from(edge.x).powi(2) + f32::from(edge.y).powi(2);
    let t = if squared <= f32::EPSILON {
        0.
    } else {
        let relative = target - a;
        ((f32::from(relative.x) * f32::from(edge.x) + f32::from(relative.y) * f32::from(edge.y))
            / squared)
            .clamp(0., 1.)
    };
    length(target - (a + edge * t))
}

fn push_quad(
    path: &mut Option<Path<Pixels>>,
    a: Point<Pixels>,
    b: Point<Pixels>,
    side_a: Point<Pixels>,
    side_b: Point<Pixels>,
) {
    let path = path.get_or_insert_with(|| Path::new(a));
    let solid = (point(0., 1.), point(0., 1.), point(0., 1.));
    path.push_triangle((a + side_a, a - side_a, b + side_b), solid);
    path.push_triangle((a - side_a, b - side_b, b + side_b), solid);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curves_pass_through_their_midpoint_at_every_zoom_and_direction() {
        let mut tessellator = Tessellator::default();
        for zoom in [0.25, 1., 2.5] {
            for from_input in [false, true] {
                for delta in [
                    point(px(320.), px(140.)),
                    point(px(-280.), px(60.)),
                    point(px(4.), px(-300.)),
                ] {
                    let curve = Curve::new(delta, zoom, from_input);
                    let middle = curve.position(0.5);
                    assert!(length(middle - delta / 2.) < 1e-3);
                    assert!(curve.bounds().dilate(px(1.)).contains(&middle));
                    assert!(tessellator.distance(curve, middle) < px(0.5));
                    let tangent = curve.position(0.51) - curve.position(0.49);
                    let away = middle + side(Point::default(), tangent, 40.);
                    assert!(tessellator.distance(curve, away) > px(6.));
                }
            }
        }
    }

    #[test]
    fn backward_links_loop_out_without_growing_linearly() {
        let reach = |dx: f32| f32::from(Curve::new(point(px(dx), px(0.)), 1., false).bend);
        assert_eq!(reach(10.), MIN_BEND);
        assert_eq!(reach(400.), 200.);
        assert!(reach(-100.) >= MIN_BEND);
        assert!(reach(-1600.) < 1600. / 2.);
        assert!(reach(-1600.) > reach(-100.));
    }

    #[test]
    fn dashes_and_hidden_stretches_leave_gaps() {
        let curve = Curve::new(point(px(300.), px(0.)), 1., false);
        let mut tessellator = Tessellator::default();
        let count = |dash, hidden: &dyn Fn(Point<Pixels>) -> bool| {
            let mut path = None;
            Tessellator::default().stroke(curve, Point::default(), px(2.), dash, hidden, &mut path);
            path.map_or(0, |path: Path<Pixels>| path.vertices.len())
        };
        let solid = count(None, &|_| false);
        let dashed = count(
            Some(Dash {
                on: 4.,
                off: 8.,
                phase: 0.,
            }),
            &|_| false,
        );
        assert!(solid > 0 && dashed > 0);
        assert_eq!(count(None, &|_| true), 0);
        let half = count(None, &|point| point.x > px(150.));
        assert!(half > 0 && half < solid);
        let mut covered = None;
        tessellator.stroke(
            curve,
            Point::default(),
            px(2.),
            None,
            &|_| false,
            &mut covered,
        );
        let bounds = covered.map(|path| path.bounds).unwrap_or_default();
        assert!(bounds.size.width >= px(299.));
    }
}
