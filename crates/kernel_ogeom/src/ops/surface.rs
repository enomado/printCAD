//! Surface steps: sheets made from curves and added to the body beside what
//! is there, and the body's faces sewn, mirrored or trimmed.
//!
//! A body built from surface steps holds a compound of its sheets (and of
//! the solids sewing closes). Curves come from sketches (open chains as
//! well as loops) or from edges of the body's own shape. Extrusions and
//! revolutions are exact; a ruled surface and a loft are B-spline surfaces
//! fitted through the curves to `FIT_TOLERANCE`, as natural faces. A step
//! the kernel has no operation for fails with a message saying so.

use kernel_api::{Continuity, CurveSource, SurfaceOp};
use ogeom::algo::build::trimmed_where_bare;
use ogeom::algo::{
    copied, find_plane, make_face, make_natural_face, make_prism, make_revolution, make_solid,
    make_wire, sew, transformed,
};
use ogeom::geom::{PlaneSurface, SurfaceGeometry};
use ogeom::math::{Axis, Direction, Point, Transform, Vector};
use ogeom::mesh::{Deflection, discretize};
use ogeom::topo::{EdgeRepr, Model, NodeData, Orientation, Shape, ShapeType, explore_unique};

use super::tol;
use crate::profile;

/// How far a fitted surface may stray from the curves it is fitted to (mm).
pub const FIT_TOLERANCE: f64 = 1e-3;

/// Points a curve is sampled at for a fit: enough for a smooth curve to fit
/// within `FIT_TOLERANCE` at part sizes, few enough to solve quickly.
const FIT_SAMPLES: usize = 48;

/// What a surface step made: the body's shape after it, and the sheet it
/// added (whose faces are named afresh), when it added one.
pub struct Made {
    pub shape: Shape,
    pub sheet: Option<Shape>,
}

pub fn apply(model: &mut Model, base: Option<&Shape>, op: &SurfaceOp) -> Result<Made, String> {
    if !op.constructs() {
        let base = base.ok_or("there is no surface in the body to work on yet")?;
        let shape = match op {
            SurfaceOp::Sew => sew_all(model, base)?,
            SurfaceOp::Mirror { origin, normal } => mirror(model, base, *origin, *normal)?,
            SurfaceOp::Thicken { .. } => {
                return Err("the kernel cannot give a sheet thickness yet".into());
            }
            SurfaceOp::TrimByPlane { .. } => {
                return Err("the kernel cannot cut a sheet by a plane yet".into());
            }
            SurfaceOp::Split { .. } => {
                return Err("the kernel cannot split a face along a curve yet".into());
            }
            SurfaceOp::Extend { .. } => {
                return Err("the kernel cannot extend a face past its edge yet".into());
            }
            _ => unreachable!("constructive steps are handled below"),
        };
        return Ok(Made { shape, sheet: None });
    }
    let sheet = match op {
        SurfaceOp::Extrude {
            curves,
            direction,
            length,
            symmetric,
        } => extrude(model, base, curves, *direction, *length, *symmetric)?,
        SurfaceOp::Revolve {
            curves,
            origin,
            axis,
            angle_deg,
        } => revolve(model, base, curves, *origin, *axis, *angle_deg)?,
        SurfaceOp::PlanarFill { curves } => planar_fill(model, base, curves)?,
        SurfaceOp::Fill {
            boundary,
            continuity,
        } => fill(model, base, boundary, *continuity)?,
        SurfaceOp::Ruled { first, second } => {
            let rows = vec![
                chain_samples(model, base, std::slice::from_ref(first))?,
                chain_samples(model, base, std::slice::from_ref(second))?,
            ];
            fitted_face(model, &rows, "ruled surface")?
        }
        SurfaceOp::Loft { sections, closed } => loft(model, base, sections, *closed)?,
        SurfaceOp::Sweep { profile, path, .. } => sweep(model, base, profile, path)?,
        SurfaceOp::Offset { .. } => {
            return Err("the kernel cannot offset a sheet's faces yet".into());
        }
        SurfaceOp::Blend { .. } => {
            return Err("the kernel cannot blend two edges with a surface yet".into());
        }
        _ => unreachable!("only constructive steps reach here"),
    };
    let shape = beside(model, base, sheet.clone())?;
    Ok(Made {
        shape,
        sheet: Some(sheet),
    })
}

/// `sheet` added to the body's shape: the pieces of both in one compound.
fn beside(model: &mut Model, base: Option<&Shape>, sheet: Shape) -> Result<Shape, String> {
    let mut pieces = match base {
        Some(base) => pieces_of(model, base),
        None => Vec::new(),
    };
    pieces.extend(pieces_of(model, &sheet));
    one_or_compound(model, pieces)
}

/// A compound's members, or the shape itself.
fn pieces_of(model: &Model, shape: &Shape) -> Vec<Shape> {
    if model.kind_of(shape) == Ok(ShapeType::Compound)
        && let Ok(children) = model.children_of(shape)
    {
        return children;
    }
    vec![shape.clone()]
}

fn one_or_compound(model: &mut Model, mut pieces: Vec<Shape>) -> Result<Shape, String> {
    match pieces.len() {
        0 => Err("the step made nothing".into()),
        1 => Ok(pieces.remove(0)),
        _ => model
            .add_compound(&pieces)
            .map_err(|e| format!("gathering the sheets failed: {e}")),
    }
}

fn point(p: [f64; 3]) -> Point {
    Point::new(p[0], p[1], p[2])
}

fn direction(v: [f64; 3], what: &str) -> Result<Direction, String> {
    Direction::new(Vector::new(v[0], v[1], v[2]), tol())
        .map_err(|_| format!("{what} has no direction"))
}

/// Each source as a kernel shape: a sketch chain as a wire (open or
/// closed), a picked edge as the edge of the body it names.
fn shapes_of(
    model: &mut Model,
    base: Option<&Shape>,
    curves: &[CurveSource],
) -> Result<Vec<Shape>, String> {
    if curves.is_empty() {
        return Err("the step has no curves to build from".into());
    }
    curves
        .iter()
        .map(|curve| match curve {
            CurveSource::Sketch { plane, wire } => profile::build_wire(model, plane, wire, false),
            CurveSource::Edge(probe) => {
                let base = base.ok_or("a picked edge needs a surface in the body to be on")?;
                super::dressup::picked_edges(model, base, std::slice::from_ref(probe))?
                    .into_iter()
                    .next()
                    .ok_or_else(|| "a picked edge is no longer in the body".to_string())
            }
        })
        .collect()
}

/// The edges of the sources, in their order: a wire's in its own order.
fn edges_of(
    model: &mut Model,
    base: Option<&Shape>,
    curves: &[CurveSource],
) -> Result<Vec<Shape>, String> {
    let mut edges = Vec::new();
    for shape in shapes_of(model, base, curves)? {
        match model.kind_of(&shape) {
            Ok(ShapeType::Edge) => edges.push(shape),
            _ => edges.extend(
                explore_unique(model, &shape, ShapeType::Edge)
                    .map_err(|e| format!("reading a curve's edges failed: {e}"))?,
            ),
        }
    }
    Ok(edges)
}

fn extrude(
    model: &mut Model,
    base: Option<&Shape>,
    curves: &[CurveSource],
    along: [f64; 3],
    length: f64,
    symmetric: bool,
) -> Result<Shape, String> {
    if !(length.is_finite() && length > 0.0) {
        return Err("an extrusion needs a length above zero".into());
    }
    let dir = direction(along, "the extrusion")?;
    let vector = Vector::from(dir) * length;
    let mut sheets = Vec::new();
    for curve in shapes_of(model, base, curves)? {
        let start = if symmetric {
            let fresh = copied(model, &curve)
                .map_err(|e| format!("copying a curve failed: {e}"))?
                .shape;
            transformed(model, &fresh, Transform::translation(vector * -0.5))
                .map_err(|e| format!("moving a curve failed: {e}"))?
                .shape
        } else {
            curve
        };
        let built = make_prism(model, &start, vector, tol())
            .map_err(|e| format!("extruding a curve failed: {e}"))?;
        sheets.push(built.shape);
    }
    one_or_compound(model, sheets)
}

fn revolve(
    model: &mut Model,
    base: Option<&Shape>,
    curves: &[CurveSource],
    origin: [f64; 3],
    axis: [f64; 3],
    angle_deg: f64,
) -> Result<Shape, String> {
    if !(angle_deg.is_finite() && angle_deg > 0.0 && angle_deg <= 360.0) {
        return Err("a revolution turns by more than 0° and at most 360°".into());
    }
    let axis = Axis::new(point(origin), direction(axis, "the axis")?);
    let mut sheets = Vec::new();
    for curve in shapes_of(model, base, curves)? {
        let built = make_revolution(model, &curve, axis, angle_deg.to_radians(), tol())
            .map_err(|e| format!("revolving a curve failed: {e}"))?;
        sheets.push(built.shape);
    }
    one_or_compound(model, sheets)
}

/// Closed sketch loops become faces as a profile's do, a loop inside
/// another a hole in it. Picked edges closing a planar loop become the face
/// on its plane.
fn planar_fill(
    model: &mut Model,
    base: Option<&Shape>,
    curves: &[CurveSource],
) -> Result<Shape, String> {
    let sketched: Vec<_> = curves
        .iter()
        .filter_map(|c| match c {
            CurveSource::Sketch { plane, wire } => Some((plane, wire)),
            CurveSource::Edge(_) => None,
        })
        .collect();
    if sketched.len() == curves.len()
        && let Some((plane, _)) = sketched.first()
    {
        let profile = kernel_api::Profile {
            plane: **plane,
            wires: sketched.iter().map(|(_, wire)| (*wire).clone()).collect(),
        };
        let built = profile::build_profile(model, &profile)?;
        return one_or_compound(model, built.faces);
    }
    let edges = edges_of(model, base, curves)?;
    let ordered = ogeom::algo::order_edges(model, &edges, tol())
        .map_err(|e| format!("the edges do not run end to end: {e}"))?;
    let wire = make_wire(model, &ordered, tol())
        .map_err(|e| format!("the edges do not close a loop: {e}"))?
        .shape;
    let plane = find_plane(model, &wire, tol())
        .map_err(|e| format!("finding the loop's plane failed: {e}"))?
        .ok_or("the loop does not lie in one plane")?;
    // The face's own bounds are its wire; the surface only needs to span it.
    let surface = PlaneSurface::over(plane, (-1.0e6, 1.0e6), (-1.0e6, 1.0e6))
        .map_err(|e| format!("the loop's plane: {e}"))?;
    let face = make_face(model, SurfaceGeometry::Plane(surface), &[wire], tol())
        .map_err(|e| format!("filling the loop failed: {e}"))?
        .shape;
    trimmed_where_bare(model, &face, tol()).map_err(|e| format!("filling the loop failed: {e}"))?;
    Ok(face)
}

fn fill(
    model: &mut Model,
    base: Option<&Shape>,
    boundary: &[CurveSource],
    continuity: Continuity,
) -> Result<Shape, String> {
    if continuity != Continuity::G0 {
        return Err(
            "the kernel can only fill touching its boundary (G0) yet; tangent and curvature \
             continuous fills wait on it"
                .into(),
        );
    }
    let edges = edges_of(model, base, boundary)?;
    let four: [Shape; 4] = edges.try_into().map_err(|edges: Vec<Shape>| {
        format!(
            "the kernel fills a boundary of four curves; this one has {}",
            edges.len()
        )
    })?;
    ogeom::offset::make_filling(model, &four, FIT_SAMPLES, FIT_TOLERANCE, tol())
        .map(|b| b.shape)
        .map_err(|e| format!("filling the boundary failed: {e}"))
}

fn loft(
    model: &mut Model,
    base: Option<&Shape>,
    sections: &[CurveSource],
    closed: bool,
) -> Result<Shape, String> {
    if sections.len() < 2 {
        return Err("a loft needs two sections or more".into());
    }
    let mut rows = Vec::with_capacity(sections.len() + 1);
    for section in sections {
        rows.push(chain_samples(model, base, std::slice::from_ref(section))?);
    }
    if closed {
        return Err("the kernel cannot close a lofted surface on itself yet".into());
    }
    fitted_face(model, &rows, "loft")
}

/// A profile swept along a straight path is an extrusion, and exact; along
/// any other path the kernel sweeps closed profiles into solids only.
fn sweep(
    model: &mut Model,
    base: Option<&Shape>,
    profile: &[CurveSource],
    path: &[CurveSource],
) -> Result<Shape, String> {
    let path_edges = edges_of(model, base, path)?;
    let [edge] = path_edges.as_slice() else {
        return Err("the kernel sweeps a surface along a single straight line yet".into());
    };
    let points = edge_points(model, edge)?;
    let (Some(a), Some(b)) = (points.first(), points.last()) else {
        return Err("the path has no length".into());
    };
    let along = Vector::new(b.x - a.x, b.y - a.y, b.z - a.z);
    let straight = points.iter().all(|p| {
        let off = Vector::new(p.x - a.x, p.y - a.y, p.z - a.z);
        off.cross(along).magnitude() <= along.magnitude() * FIT_TOLERANCE
    });
    if !straight {
        return Err("the kernel sweeps a surface along a single straight line yet".into());
    }
    let mut sheets = Vec::new();
    for curve in shapes_of(model, base, profile)? {
        let built = make_prism(model, &curve, along, tol())
            .map_err(|e| format!("sweeping the profile failed: {e}"))?;
        sheets.push(built.shape);
    }
    one_or_compound(model, sheets)
}

/// A B-spline surface through rows of points, as a natural face.
fn fitted_face(model: &mut Model, rows: &[Vec<Point>], what: &str) -> Result<Shape, String> {
    let fitted = ogeom::geom::fit::fit_surface_grid(rows, 3, FIT_TOLERANCE, tol())
        .map_err(|e| format!("fitting the {what} failed: {e}"))?;
    if !fitted.met {
        return Err(format!(
            "the {what} strays {:.4} mm from its curves, more than {FIT_TOLERANCE} mm",
            fitted.error
        ));
    }
    make_natural_face(model, SurfaceGeometry::BSpline(fitted.curve))
        .map(|b| b.shape)
        .map_err(|e| format!("making the {what}'s face failed: {e}"))
}

/// `FIT_SAMPLES` points spread evenly along a chain of curves, from its
/// start to its end.
fn chain_samples(
    model: &mut Model,
    base: Option<&Shape>,
    curves: &[CurveSource],
) -> Result<Vec<Point>, String> {
    let edges = edges_of(model, base, curves)?;
    let mut line: Vec<Point> = Vec::new();
    for edge in &edges {
        let points = edge_points(model, edge)?;
        let skip = usize::from(
            line.last()
                .zip(points.first())
                .is_some_and(|(a, b)| a.distance(*b) <= FIT_TOLERANCE),
        );
        line.extend(points.into_iter().skip(skip));
    }
    resample(&line, FIT_SAMPLES).ok_or_else(|| "a curve has no length".to_string())
}

/// Points along an edge, placed, from its start to its end as it runs.
fn edge_points(model: &Model, edge: &Shape) -> Result<Vec<Point>, String> {
    let Some(NodeData::Edge(data)) = model.node(edge).map(|n| n.data()) else {
        return Err("a curve is not an edge".into());
    };
    let Some(EdgeRepr::Curve3d { curve, range, .. }) = data.curve3d() else {
        return Err("an edge has no curve".into());
    };
    let geometry = model
        .geometry()
        .curve(*curve)
        .ok_or("an edge's curve is missing")?;
    let placement = edge
        .transform(model.datums())
        .map_err(|e| format!("placing an edge failed: {e}"))?;
    let deflection = Deflection::with_chord(FIT_TOLERANCE * 0.25)
        .map_err(|e| format!("sampling an edge: {e}"))?;
    let line = discretize(geometry, *range, deflection, tol())
        .map_err(|e| format!("sampling an edge failed: {e}"))?;
    let mut points: Vec<Point> = line.points.iter().map(|p| placement.apply(*p)).collect();
    if edge.orientation() == Orientation::Reversed {
        points.reverse();
    }
    Ok(points)
}

/// `count` points spread evenly by length along a polyline.
fn resample(line: &[Point], count: usize) -> Option<Vec<Point>> {
    let lengths: Vec<f64> = std::iter::once(0.0)
        .chain(line.windows(2).scan(0.0, |run, pair| {
            *run += pair[0].distance(pair[1]);
            Some(*run)
        }))
        .collect();
    let total = *lengths.last()?;
    if total <= FIT_TOLERANCE {
        return None;
    }
    let mut out = Vec::with_capacity(count);
    let mut segment = 0;
    for i in 0..count {
        #[allow(clippy::cast_precision_loss)]
        let at = total * i as f64 / (count - 1) as f64;
        while segment + 2 < lengths.len() && lengths[segment + 1] < at {
            segment += 1;
        }
        let (a, b) = (line[segment], line[segment + 1]);
        let span = lengths[segment + 1] - lengths[segment];
        let t = if span > 0.0 {
            ((at - lengths[segment]) / span).clamp(0.0, 1.0)
        } else {
            0.0
        };
        out.push(Point::new(
            a.x + (b.x - a.x) * t,
            a.y + (b.y - a.y) * t,
            a.z + (b.z - a.z) * t,
        ));
    }
    Some(out)
}

/// Every face of the body joined where their edges meet; a shell that
/// closes is made a solid.
fn sew_all(model: &mut Model, base: &Shape) -> Result<Shape, String> {
    let faces = explore_unique(model, base, ShapeType::Face)
        .map_err(|e| format!("reading the body's faces failed: {e}"))?;
    let sewn = sew(model, &faces, tol()).map_err(|e| format!("sewing failed: {e}"))?;
    crate::naming::record(&sewn.history);
    let mut pieces = Vec::with_capacity(sewn.shells.len());
    for shell in sewn.shells {
        let closed = ogeom::algo::is_shell_closed(model, &shell).unwrap_or(false);
        pieces.push(if closed {
            make_solid(model, std::slice::from_ref(&shell))
                .map(|b| b.shape)
                .map_err(|e| format!("making the closed shell a solid failed: {e}"))?
        } else {
            shell
        });
    }
    one_or_compound(model, pieces)
}

/// The body's shape with its reflection in the plane beside it.
fn mirror(
    model: &mut Model,
    base: &Shape,
    origin: [f64; 3],
    normal: [f64; 3],
) -> Result<Shape, String> {
    let n = direction(normal, "the mirror plane")?;
    let n = Vector::from(n);
    let o = Vector::new(origin[0], origin[1], origin[2]);
    // x' = x - 2 n (n·(x - o))
    let d = 2.0 * n.dot(o);
    let mut m = [[0.0; 4]; 4];
    let nn = [n.x, n.y, n.z];
    for (i, row) in m.iter_mut().enumerate().take(3) {
        for (j, cell) in row.iter_mut().enumerate().take(3) {
            *cell = f64::from(u8::from(i == j)) - 2.0 * nn[i] * nn[j];
        }
        row[3] = d * nn[i];
    }
    m[3][3] = 1.0;
    let image = super::pattern::mirrored(model, base, &m)?;
    let mut pieces = pieces_of(model, base);
    pieces.extend(pieces_of(model, &image));
    one_or_compound(model, pieces)
}
