//! Constraint tools: which constraint a toolbar button creates for the
//! current selection. The buttons are Action tools enabled by the shape of
//! the selection; dimensional kinds are created at the measured value so
//! the geometry never jumps.

use std::collections::HashSet;

use uuid::Uuid;

use crate::sketch::{
    self, AxisDirection, ConstraintKind, GeometryElement, ORIGIN_ID, Reference, Sketch, Vec2D,
    X_AXIS_ID, Y_AXIS_ID,
};

/// The selection sorted by element kind, in sketch order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SelectionShape {
    pub all: Vec<Uuid>,
    pub points: Vec<Uuid>,
    pub lines: Vec<Uuid>,
    /// Circles and arcs.
    pub circles: Vec<Uuid>,
    /// The arcs among `circles`.
    pub arcs: Vec<Uuid>,
    pub ellipses: Vec<Uuid>,
}

impl SelectionShape {
    pub fn of(sketch: &Sketch, selected: &HashSet<Uuid>) -> Self {
        let mut shape = Self::default();
        for g in &sketch.geometry {
            let id = g.id();
            if !selected.contains(&id) {
                continue;
            }
            shape.all.push(id);
            match g {
                GeometryElement::Point(_) => shape.points.push(id),
                GeometryElement::Line(_) => shape.lines.push(id),
                GeometryElement::Circle(_) => shape.circles.push(id),
                GeometryElement::Arc(_) => {
                    shape.circles.push(id);
                    shape.arcs.push(id);
                }
                GeometryElement::Ellipse(_) => shape.ellipses.push(id),
                _ => {}
            }
        }
        // The origin and the axes hold no entry in `geometry`, but they take
        // constraints like the geometry that does. They come last and in a
        // fixed order, so a selection reads the same every time.
        for id in [ORIGIN_ID, X_AXIS_ID, Y_AXIS_ID] {
            if !selected.contains(&id) {
                continue;
            }
            shape.all.push(id);
            match Reference::of(id) {
                Some(reference) if reference.is_point() => shape.points.push(id),
                _ => shape.lines.push(id),
            }
        }
        shape
    }

    fn total(&self) -> usize {
        self.all.len()
    }

    fn only(&self, points: usize, lines: usize, circles: usize, ellipses: usize) -> bool {
        self.points.len() == points
            && self.lines.len() == lines
            && self.circles.len() == circles
            && self.ellipses.len() == ellipses
            && self.total() == points + lines + circles + ellipses
    }
}

/// The constraint tool ids, without the `sketch.constrain.` prefix.
#[cfg(test)]
pub const TOOLS: &[&str] = &[
    "coincident",
    "point_on_object",
    "midpoint",
    "vertical",
    "horizontal",
    "parallel",
    "perpendicular",
    "tangent",
    "equal",
    "symmetric",
    "block",
    "lock",
    "distance_x",
    "distance_y",
    "distance",
    "radius",
    "diameter",
    "angle",
    "angle_x",
    "angle_y",
    "arc_length",
    "gap",
    "radius_diameter",
    "angle_at_point",
    "refraction",
];

fn axis_distance(horizontal: bool, a: Uuid, b: Option<Uuid>, value: f32) -> ConstraintKind {
    if horizontal {
        ConstraintKind::DistanceX { a, b, value }
    } else {
        ConstraintKind::DistanceY { a, b, value }
    }
}

/// The constraints `tool` creates for `shape`, or `None` when the selection
/// does not fit the tool. Dimensional kinds carry the value measured on
/// `sketch`.
pub fn kinds_for(
    tool: &str,
    shape: &SelectionShape,
    sketch: &Sketch,
) -> Option<Vec<ConstraintKind>> {
    let measured = |kind: &ConstraintKind| sketch::measured_value(sketch, kind).unwrap_or(0.0);
    let (p, l, c, e) = (&shape.points, &shape.lines, &shape.circles, &shape.ellipses);
    let kinds = match tool {
        "coincident" if shape.only(2, 0, 0, 0) => vec![ConstraintKind::Coincident {
            point1: p[0],
            point2: p[1],
        }],
        "point_on_object" if shape.only(1, 1, 0, 0) => vec![ConstraintKind::PointOnLine {
            point: p[0],
            line: l[0],
        }],
        "point_on_object" if shape.only(1, 0, 1, 0) => vec![ConstraintKind::PointOnCircle {
            point: p[0],
            circle: c[0],
        }],
        "point_on_object" if shape.only(1, 0, 0, 1) => vec![ConstraintKind::PointOnEllipse {
            point: p[0],
            ellipse: e[0],
        }],
        "midpoint" if shape.only(1, 1, 0, 0) => vec![ConstraintKind::Midpoint {
            point: p[0],
            line: l[0],
        }],
        "horizontal" if !l.is_empty() && shape.only(0, l.len(), 0, 0) => l
            .iter()
            .map(|line| ConstraintKind::Horizontal { element: *line })
            .collect(),
        "vertical" if !l.is_empty() && shape.only(0, l.len(), 0, 0) => l
            .iter()
            .map(|line| ConstraintKind::Vertical { element: *line })
            .collect(),
        "parallel" if shape.only(0, 2, 0, 0) => vec![ConstraintKind::Parallel {
            line1: l[0],
            line2: l[1],
        }],
        "perpendicular" if shape.only(0, 2, 0, 0) => vec![ConstraintKind::Perpendicular {
            line1: l[0],
            line2: l[1],
        }],
        "tangent" if shape.only(0, 0, 2, 0) => vec![ConstraintKind::Tangent {
            line_or_circle1: c[0],
            item2: c[1],
        }],
        "tangent" if shape.only(0, 1, 1, 0) => vec![ConstraintKind::Tangent {
            line_or_circle1: l[0],
            item2: c[0],
        }],
        "equal" if shape.only(0, 2, 0, 0) => vec![ConstraintKind::EqualLength {
            line1: l[0],
            line2: l[1],
        }],
        "equal" if shape.only(0, 0, 2, 0) => vec![ConstraintKind::EqualRadius {
            circle1: c[0],
            circle2: c[1],
        }],
        "symmetric" if shape.only(2, 1, 0, 0) => vec![ConstraintKind::Symmetric {
            point1: p[0],
            point2: p[1],
            line: l[0],
        }],
        "symmetric" if shape.only(3, 0, 0, 0) => vec![ConstraintKind::SymmetricAboutPoint {
            point1: p[0],
            point2: p[1],
            center: p[2],
        }],
        "block" if shape.total() >= 1 => shape
            .all
            .iter()
            .map(|id| ConstraintKind::Block { element: *id })
            .collect(),
        "lock" if shape.only(1, 0, 0, 0) => vec![ConstraintKind::FixedPoint {
            point: p[0],
            position: sketch.point_position(p[0]).unwrap_or(Vec2D::new(0.0, 0.0)),
        }],
        "distance_x" | "distance_y" if shape.only(1, 0, 0, 0) || shape.only(2, 0, 0, 0) => {
            let horizontal = tool == "distance_x";
            let b = p.get(1).copied();
            let kind = axis_distance(horizontal, p[0], b, 0.0);
            vec![axis_distance(horizontal, p[0], b, measured(&kind))]
        }
        "distance" if shape.only(2, 0, 0, 0) => {
            let kind = ConstraintKind::Distance {
                point1: p[0],
                point2: p[1],
                distance: 0.0,
            };
            vec![ConstraintKind::Distance {
                point1: p[0],
                point2: p[1],
                distance: measured(&kind),
            }]
        }
        "distance" if shape.only(0, 1, 0, 0) => {
            let kind = ConstraintKind::Length {
                line: l[0],
                length: 0.0,
            };
            vec![ConstraintKind::Length {
                line: l[0],
                length: measured(&kind),
            }]
        }
        "radius" if shape.only(0, 0, 1, 0) => {
            let kind = ConstraintKind::Radius {
                circle: c[0],
                radius: 0.0,
            };
            vec![ConstraintKind::Radius {
                circle: c[0],
                radius: measured(&kind),
            }]
        }
        "diameter" if shape.only(0, 0, 1, 0) => {
            let kind = ConstraintKind::Diameter {
                circle: c[0],
                diameter: 0.0,
            };
            vec![ConstraintKind::Diameter {
                circle: c[0],
                diameter: measured(&kind),
            }]
        }
        "angle" if shape.only(0, 2, 0, 0) => {
            let kind = ConstraintKind::Angle {
                line1: l[0],
                line2: l[1],
                angle_rad: 0.0,
            };
            vec![ConstraintKind::Angle {
                line1: l[0],
                line2: l[1],
                angle_rad: measured(&kind).to_radians(),
            }]
        }
        "angle" | "angle_at_point"
            if p.len() == 1 && l.len() + c.len() == 2 && shape.only(1, l.len(), c.len(), 0) =>
        {
            let point = p[0];
            let curves: Vec<Uuid> = shape
                .all
                .iter()
                .copied()
                .filter(|id| *id != point)
                .collect();
            let mut kinds: Vec<ConstraintKind> = curves
                .iter()
                .filter(|curve| !held_on(sketch, point, **curve))
                .map(|curve| on_curve(shape, point, *curve))
                .collect();
            let kind = ConstraintKind::AngleAtPoint {
                curve1: curves[0],
                curve2: curves[1],
                point,
                angle_rad: 0.0,
            };
            kinds.push(ConstraintKind::AngleAtPoint {
                curve1: curves[0],
                curve2: curves[1],
                point,
                angle_rad: measured(&kind).to_radians(),
            });
            kinds
        }
        "arc_length" if !shape.arcs.is_empty() && shape.only(0, 0, shape.arcs.len(), 0) => shape
            .arcs
            .iter()
            .map(|arc| {
                let kind = ConstraintKind::ArcLength {
                    arc: *arc,
                    length: 0.0,
                };
                ConstraintKind::ArcLength {
                    arc: *arc,
                    length: measured(&kind),
                }
            })
            .collect(),
        "distance" | "gap"
            if shape.only(1, 1, 0, 0)
                || shape.only(1, 0, 1, 0)
                || shape.only(0, 1, 1, 0)
                || shape.only(0, 0, 2, 0) =>
        {
            let (item1, item2) = (shape.all[0], shape.all[1]);
            let kind = ConstraintKind::Gap {
                item1,
                item2,
                distance: 0.0,
            };
            vec![ConstraintKind::Gap {
                item1,
                item2,
                distance: measured(&kind),
            }]
        }
        "radius_diameter" if !c.is_empty() && shape.only(0, 0, c.len(), 0) => {
            // The first takes the dimension its kind is given, the rest
            // are made equal to it.
            let first = c[0];
            let dimension = if shape.arcs.contains(&first) {
                let kind = ConstraintKind::Radius {
                    circle: first,
                    radius: 0.0,
                };
                ConstraintKind::Radius {
                    circle: first,
                    radius: measured(&kind),
                }
            } else {
                let kind = ConstraintKind::Diameter {
                    circle: first,
                    diameter: 0.0,
                };
                ConstraintKind::Diameter {
                    circle: first,
                    diameter: measured(&kind),
                }
            };
            std::iter::once(dimension)
                .chain(c[1..].iter().map(|other| ConstraintKind::EqualRadius {
                    circle1: first,
                    circle2: *other,
                }))
                .collect()
        }
        "refraction" if shape.only(1, 2, 1, 0) || shape.only(1, 3, 0, 0) => {
            let point = p[0];
            let (rays, interface) = if c.is_empty() {
                // The rays are the lines that end at the point; the
                // interface is the one it lies on.
                let ending: Vec<Uuid> = l
                    .iter()
                    .copied()
                    .filter(|line| ends_at(sketch, *line, point))
                    .collect();
                match ending.as_slice() {
                    [a, b] => {
                        let rest = l.iter().copied().find(|line| !ending.contains(line));
                        ([*a, *b], rest.unwrap_or(l[2]))
                    }
                    _ => ([l[0], l[1]], l[2]),
                }
            } else {
                ([l[0], l[1]], c[0])
            };
            let mut kinds = Vec::new();
            if !held_on(sketch, point, interface) {
                kinds.push(on_curve(shape, point, interface));
            }
            for ray in rays {
                if !ends_at(sketch, ray, point)
                    && let Some(end) = nearer_end(sketch, ray, point)
                {
                    kinds.push(ConstraintKind::Coincident {
                        point1: end,
                        point2: point,
                    });
                }
            }
            let ratio =
                crate::measure::refraction_ratio(sketch, rays[0], rays[1], interface, point)
                    .filter(|r| r.is_finite() && r.abs() > 1e-6)
                    .unwrap_or(1.0);
            kinds.push(ConstraintKind::Refraction {
                ray1: rays[0],
                ray2: rays[1],
                interface,
                point,
                ratio,
            });
            kinds
        }
        "angle_x" | "angle_y" if shape.only(0, 1, 0, 0) => {
            let axis = if tool == "angle_x" {
                AxisDirection::Horizontal
            } else {
                AxisDirection::Vertical
            };
            let kind = ConstraintKind::AngleToAxis {
                line: l[0],
                axis,
                angle_rad: 0.0,
            };
            vec![ConstraintKind::AngleToAxis {
                line: l[0],
                axis,
                angle_rad: measured(&kind).to_radians(),
            }]
        }
        _ => return None,
    };
    Some(kinds)
}

/// The points that stand for `point`: itself and those a coincidence joins
/// it to.
fn same_points(sketch: &Sketch, point: Uuid) -> Vec<Uuid> {
    let mut out = vec![point];
    for c in &sketch.constraints {
        if let ConstraintKind::Coincident { point1, point2 } = c.kind {
            if point1 == point {
                out.push(point2);
            } else if point2 == point {
                out.push(point1);
            }
        }
    }
    out
}

/// Whether `line` (a line or an arc) has an end at `point`.
fn ends_at(sketch: &Sketch, line: Uuid, point: Uuid) -> bool {
    let ends = match sketch.get_geometry(line) {
        Some(GeometryElement::Line(l)) => [l.start, l.end],
        Some(GeometryElement::Arc(a)) => [a.start, a.end],
        _ => return false,
    };
    same_points(sketch, point).iter().any(|p| ends.contains(p))
}

/// The end of a line nearer `point`.
fn nearer_end(sketch: &Sketch, line: Uuid, point: Uuid) -> Option<Uuid> {
    let Some(GeometryElement::Line(l)) = sketch.get_geometry(line) else {
        return None;
    };
    let at = sketch.point_position(point)?.to_glam();
    let d = |id: Uuid| {
        sketch
            .point_position(id)
            .map_or(f32::INFINITY, |p| (p.to_glam() - at).length())
    };
    Some(if d(l.start) <= d(l.end) {
        l.start
    } else {
        l.end
    })
}

/// Whether `point` already lies on `curve`: at one of its ends, or held
/// there by a point-on constraint.
fn held_on(sketch: &Sketch, point: Uuid, curve: Uuid) -> bool {
    if ends_at(sketch, curve, point) {
        return true;
    }
    let same = same_points(sketch, point);
    sketch.constraints.iter().any(|c| match c.kind {
        ConstraintKind::PointOnLine { point, line } => line == curve && same.contains(&point),
        ConstraintKind::PointOnCircle { point, circle } => circle == curve && same.contains(&point),
        _ => false,
    })
}

/// The constraint that keeps `point` on `curve`.
fn on_curve(shape: &SelectionShape, point: Uuid, curve: Uuid) -> ConstraintKind {
    if shape.lines.contains(&curve) {
        ConstraintKind::PointOnLine { point, line: curve }
    } else {
        ConstraintKind::PointOnCircle {
            point,
            circle: curve,
        }
    }
}

/// The dimensional tool the Dimension button takes for a selection: an
/// angle for two lines or two curves and the point they meet at, a
/// distance or a gap for two items, a radius or diameter (by kind) for
/// circles and arcs, a length for a line. An arc's length is never
/// guessed: it has its own tool.
pub fn dimension_for(shape: &SelectionShape) -> Option<&'static str> {
    [
        "angle",
        "distance",
        "radius_diameter",
        "distance_x",
        "distance_y",
    ]
    .into_iter()
    .find(|tool| fits(tool, shape))
}

/// Whether `tool` applies to `shape` at all.
pub fn fits(tool: &str, shape: &SelectionShape) -> bool {
    kinds_for(tool, shape, &Sketch::new("")).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sketch::{Arc, Circle, Line, Point};

    fn sketch_with_line_and_circle() -> (Sketch, Uuid, Uuid, Uuid, Uuid) {
        let mut sketch = Sketch::new("t");
        let a = sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(0.0, 0.0))));
        let b = sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(10.0, 0.0))));
        let line = sketch.add_geometry(GeometryElement::Line(Line::new(a, b)));
        let center = sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(5.0, 5.0))));
        let circle = sketch.add_geometry(GeometryElement::Circle(Circle::new(center, 2.0)));
        (sketch, a, b, line, circle)
    }

    #[test]
    fn a_line_alone_takes_length_and_orientation_tools() {
        let (sketch, _, _, line, _) = sketch_with_line_and_circle();
        let shape = SelectionShape::of(&sketch, &HashSet::from([line]));
        assert!(fits("horizontal", &shape));
        assert!(fits("distance", &shape));
        assert!(fits("angle_x", &shape));
        assert!(!fits("radius", &shape));
        assert!(!fits("coincident", &shape));
        let kinds = kinds_for("distance", &shape, &sketch).unwrap();
        assert!(
            matches!(kinds[0], ConstraintKind::Length { length, .. } if (length - 10.0).abs() < 1e-4)
        );
    }

    #[test]
    fn point_on_object_dispatches_by_the_other_element() {
        let (sketch, a, _, line, circle) = sketch_with_line_and_circle();
        let on_line = SelectionShape::of(&sketch, &HashSet::from([a, line]));
        assert!(matches!(
            kinds_for("point_on_object", &on_line, &sketch).unwrap()[0],
            ConstraintKind::PointOnLine { .. }
        ));
        let on_circle = SelectionShape::of(&sketch, &HashSet::from([a, circle]));
        assert!(matches!(
            kinds_for("point_on_object", &on_circle, &sketch).unwrap()[0],
            ConstraintKind::PointOnCircle { .. }
        ));
    }

    #[test]
    fn block_takes_any_selection_and_nothing_takes_an_empty_one() {
        let (sketch, a, _, line, _) = sketch_with_line_and_circle();
        let shape = SelectionShape::of(&sketch, &HashSet::from([a, line]));
        assert_eq!(kinds_for("block", &shape, &sketch).unwrap().len(), 2);
        let empty = SelectionShape::default();
        assert!(TOOLS.iter().all(|t| !fits(t, &empty)));
    }

    /// A line, a circle, and an arc whose start the line ends at.
    fn sketch_with_an_arc() -> (Sketch, Uuid, Uuid, Uuid, Uuid) {
        let (mut sketch, _, b, line, circle) = sketch_with_line_and_circle();
        let center = sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(10.0, 5.0))));
        let end = sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(15.0, 5.0))));
        let arc = sketch.add_geometry(GeometryElement::Arc(Arc::new(center, b, end, 5.0)));
        (sketch, b, line, circle, arc)
    }

    #[test]
    fn an_arc_takes_its_length_and_a_circle_does_not() {
        let (sketch, _, _, circle, arc) = sketch_with_an_arc();
        let shape = SelectionShape::of(&sketch, &HashSet::from([arc]));
        let kinds = kinds_for("arc_length", &shape, &sketch).unwrap();
        let quarter = 5.0 * std::f32::consts::FRAC_PI_2;
        assert!(
            matches!(kinds[0], ConstraintKind::ArcLength { length, .. } if (length - quarter).abs() < 1e-4),
            "{kinds:?}"
        );
        let circle_only = SelectionShape::of(&sketch, &HashSet::from([circle]));
        assert!(!fits("arc_length", &circle_only));
        let both = SelectionShape::of(&sketch, &HashSet::from([circle, arc]));
        assert!(!fits("arc_length", &both));
    }

    #[test]
    fn two_curves_or_a_point_and_a_curve_take_a_gap() {
        let (sketch, b, line, circle, arc) = sketch_with_an_arc();
        for pair in [[line, circle], [circle, arc], [b, circle], [b, line]] {
            let shape = SelectionShape::of(&sketch, &HashSet::from(pair));
            assert!(fits("gap", &shape), "{pair:?}");
            let kinds = kinds_for("distance", &shape, &sketch).unwrap();
            assert!(matches!(kinds[0], ConstraintKind::Gap { .. }), "{kinds:?}");
        }
        let shape = SelectionShape::of(&sketch, &HashSet::from([line, circle]));
        let kinds = kinds_for("gap", &shape, &sketch).unwrap();
        assert!(
            matches!(kinds[0], ConstraintKind::Gap { distance, .. } if (distance - 3.0).abs() < 1e-4),
            "the circle at (5, 5) with radius 2 stands 3 off the line: {kinds:?}"
        );
        let two_lines = {
            let mut sketch = sketch.clone();
            let p = sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(0.0, 9.0))));
            let other = sketch.add_geometry(GeometryElement::Line(Line::new(b, p)));
            SelectionShape::of(&sketch, &HashSet::from([line, other]))
        };
        assert!(
            !fits("gap", &two_lines),
            "two lines take an angle, not a gap"
        );
    }

    #[test]
    fn radius_or_diameter_goes_by_kind_and_the_rest_are_equal() {
        let (sketch, _, _, circle, arc) = sketch_with_an_arc();
        let circle_only = SelectionShape::of(&sketch, &HashSet::from([circle]));
        assert!(matches!(
            kinds_for("radius_diameter", &circle_only, &sketch).unwrap()[..],
            [ConstraintKind::Diameter { diameter, .. }] if (diameter - 4.0).abs() < 1e-4
        ));
        let arc_only = SelectionShape::of(&sketch, &HashSet::from([arc]));
        assert!(matches!(
            kinds_for("radius_diameter", &arc_only, &sketch).unwrap()[..],
            [ConstraintKind::Radius { radius, .. }] if (radius - 5.0).abs() < 1e-4
        ));
        let both = SelectionShape::of(&sketch, &HashSet::from([circle, arc]));
        let kinds = kinds_for("radius_diameter", &both, &sketch).unwrap();
        assert!(
            matches!(
                kinds[..],
                [
                    ConstraintKind::Diameter { circle: first, .. },
                    ConstraintKind::EqualRadius { circle1, circle2 },
                ] if first == circle && circle1 == circle && circle2 == arc
            ),
            "{kinds:?}"
        );
    }

    #[test]
    fn an_angle_at_a_point_keeps_the_point_on_the_curve_it_is_not_an_end_of() {
        let (sketch, b, line, circle, arc) = sketch_with_an_arc();
        let shape = SelectionShape::of(&sketch, &HashSet::from([b, line, arc]));
        let kinds = kinds_for("angle", &shape, &sketch).unwrap();
        assert!(
            matches!(kinds[..], [ConstraintKind::AngleAtPoint { point, .. }] if point == b),
            "the point ends both curves, so nothing else is needed: {kinds:?}"
        );
        let shape = SelectionShape::of(&sketch, &HashSet::from([b, line, circle]));
        let kinds = kinds_for("angle_at_point", &shape, &sketch).unwrap();
        assert!(
            matches!(
                kinds[..],
                [
                    ConstraintKind::PointOnCircle { point, circle: on },
                    ConstraintKind::AngleAtPoint { .. },
                ] if point == b && on == circle
            ),
            "{kinds:?}"
        );
    }

    #[test]
    fn a_refraction_tells_its_rays_from_the_interface() {
        let mut sketch = Sketch::new("t");
        let point = |sketch: &mut Sketch, x: f32, y: f32| {
            sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(x, y))))
        };
        let l = point(&mut sketch, -10.0, 0.0);
        let r = point(&mut sketch, 10.0, 0.0);
        let source = point(&mut sketch, -4.0, 4.0);
        let hit = point(&mut sketch, 0.0, 0.0);
        let exit = point(&mut sketch, 3.0, -4.0);
        let ray1 = sketch.add_geometry(GeometryElement::Line(Line::new(source, hit)));
        // The interface drawn between the two rays, so sketch order alone
        // would take it for a ray.
        let interface = sketch.add_geometry(GeometryElement::Line(Line::new(l, r)));
        let ray2 = sketch.add_geometry(GeometryElement::Line(Line::new(hit, exit)));
        let shape = SelectionShape::of(&sketch, &HashSet::from([hit, ray1, ray2, interface]));
        let kinds = kinds_for("refraction", &shape, &sketch).unwrap();
        let [
            ConstraintKind::PointOnLine { point: on, line },
            ConstraintKind::Refraction {
                ray1: r1,
                ray2: r2,
                interface: i,
                ratio,
                ..
            },
        ] = kinds[..]
        else {
            panic!("{kinds:?}");
        };
        assert_eq!((on, line), (hit, interface));
        assert_eq!((r1, r2, i), (ray1, ray2, interface));
        let expected = 45f32.to_radians().sin() / (3.0f32 / 5.0);
        assert!((ratio - expected).abs() < 1e-4, "{ratio}");
    }

    #[test]
    fn the_dimension_tool_never_guesses_an_arc_length() {
        let (sketch, _, _, _, arc) = sketch_with_an_arc();
        let shape = SelectionShape::of(&sketch, &HashSet::from([arc]));
        assert_eq!(dimension_for(&shape), Some("radius_diameter"));
    }
}
