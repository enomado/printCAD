//! The kernel's answers to the questions a workbench asks while it runs
//! (`kernel_api::KernelQueries`).

use kernel_api::{
    CentreLine, FaceProbe, KernelError, KernelQueries, KernelResult, MedialPath, MedialRegion,
    Narrowest, Overlap, ProbeAnswer, Profile, ProfilePlane, ProjectedEdge, ShapeProbe,
    TessellationSettings,
};
use ogeom::algo::{MedialGraph, linear_properties, medial_graph, volume_properties};
use ogeom::algo::{ProjectedCurve, distance_between_shapes, project_edge_onto_plane};
use ogeom::core::Tolerances;
use ogeom::geom::Curve2d as _;
use ogeom::geom::Curve3d as _;
use ogeom::math::{Direction, Frame, Plane, Point, Vector};
use ogeom::mesh::{Deflection, discretize};
use ogeom::topo::{EdgeRepr, Model, NodeData, Orientation, Shape, ShapeType, explore_unique};

use crate::ops::dressup::nearest_of;
use crate::tess;

/// The ogeom kernel's answers. It holds nothing, so one serves every caller.
pub struct OgeomQueries;

/// The instance the application hands its workbenches.
pub static QUERIES: OgeomQueries = OgeomQueries;

/// Points taken along a curve with no closed form in a sketch.
const POLYLINE_POINTS: usize = 64;

fn other(message: impl std::fmt::Display) -> KernelError {
    KernelError::Other(anyhow::anyhow!("{message}"))
}

/// A shared solid smaller than this, in mm³, is taken for two faces that
/// touch: what a boolean of flush faces leaves behind.
const TOUCHING_MM3: f64 = 1e-6;

impl KernelQueries for OgeomQueries {
    fn gap(&self, a: &[u8], b: &[u8], b_in_a: &[[f64; 4]; 4]) -> KernelResult<kernel_api::Gap> {
        let tol = tess::tolerances();
        let (mut model, first) = tess::read_blob(a)?;
        let second = crate::chain::absorb_shape(&mut model, b).map_err(other)?;
        let second = crate::ops::pattern::moved(&mut model, &second, b_in_a).map_err(other)?;
        let found = distance_between_shapes(
            &model,
            &first,
            &second,
            ogeom::intersect::ExtremaOptions::default(),
            tol,
        )
        .map_err(other)?;
        let pair = found
            .pairs
            .first()
            .ok_or_else(|| other("the shapes have nothing to measure between"))?;
        let at = |p: Point| [p.x, p.y, p.z];
        Ok(kernel_api::Gap {
            distance_mm: found.distance,
            on_a: at(pair.point_a),
            on_b: at(pair.point_b),
        })
    }

    fn measure(&self, brep: &[u8]) -> KernelResult<kernel_api::PhysicalProperties> {
        crate::health::measure_blob(brep)
    }

    fn overlap(&self, a: &[u8], b: &[u8], b_in_a: &[[f64; 4]; 4]) -> KernelResult<Option<Overlap>> {
        let tol = tess::tolerances();
        let (mut model, first) = tess::read_blob(a)?;
        let second = crate::chain::absorb_shape(&mut model, b).map_err(other)?;
        let second = crate::ops::pattern::moved(&mut model, &second, b_in_a).map_err(other)?;
        let pieces = crate::ops::common_pieces(&mut model, &first, &second).map_err(other)?;
        let (mut volume, mut moment) = (0.0, Vector::new(0.0, 0.0, 0.0));
        for piece in &pieces {
            let measured = volume_properties(&model, piece, Deflection::default(), tol)
                .map_err(|e| other(format!("measuring the shared solid failed: {e}")))?;
            let mass = measured.mass.abs();
            volume += mass;
            moment += Vector::new(measured.centre.x, measured.centre.y, measured.centre.z) * mass;
        }
        if volume <= TOUCHING_MM3 {
            return Ok(None);
        }
        let shared = crate::ops::wrap_pieces(&mut model, pieces).map_err(other)?;
        let mesh = tess::mesh_shape(&model, &shared, &[], &TessellationSettings::default())?;
        let centre = moment / volume;
        Ok(Some(Overlap {
            volume_mm3: volume,
            centre_mm: [centre.x, centre.y, centre.z],
            mesh,
        }))
    }

    fn project_edge(
        &self,
        brep: &[u8],
        near: [f64; 3],
        plane: &ProfilePlane,
    ) -> KernelResult<ProjectedEdge> {
        let tol = tess::tolerances();
        let (mut model, root) = tess::read_blob(brep)?;
        let edge = nearest_of(
            &mut model,
            &root,
            ShapeType::Edge,
            Point::new(near[0], near[1], near[2]),
        )
        .map_err(other)?;
        let plane = plane_of(plane)?;
        let projected = project_edge_onto_plane(&model, &edge, &plane, tol).map_err(other)?;
        in_plane(projected)
    }

    fn section_face(
        &self,
        brep: &[u8],
        near: [f64; 3],
        plane: &ProfilePlane,
    ) -> KernelResult<Vec<ProjectedEdge>> {
        let tol = tess::tolerances();
        let (mut model, root) = tess::read_blob(brep)?;
        let face = nearest_of(
            &mut model,
            &root,
            ShapeType::Face,
            Point::new(near[0], near[1], near[2]),
        )
        .map_err(other)?;
        let plane = plane_of(plane)?;
        face_section(&mut model, &root, &face, &plane)?
            .iter()
            .map(|edge| {
                // A section edge lies in the plane: projecting it onto the
                // plane names the curve it is there.
                in_plane(project_edge_onto_plane(&model, edge, &plane, tol).map_err(other)?)
            })
            .collect()
    }

    fn face_of(&self, brep: &[u8], near: [f64; 3]) -> KernelResult<Vec<u8>> {
        let (mut model, root) = tess::read_blob(brep)?;
        let at = Point::new(near[0], near[1], near[2]);
        let face = crate::ops::sweep::face_at(&mut model, &root, at)
            .map_err(other)?
            .ok_or_else(|| {
                other(format!(
                    "no face of the solid lies at ({:.1}, {:.1}, {:.1})",
                    at.x, at.y, at.z
                ))
            })?;
        let copy = ogeom::algo::copied(&mut model, &face)
            .map_err(|e| other(format!("copying the face failed: {e}")))?
            .shape;
        tess::write_blob(&model, &copy)
    }

    fn read_dxf(&self, text: &str) -> KernelResult<kernel_api::Drawing2d> {
        crate::dxf::read_dxf(text)
    }

    fn medial_axis(&self, profile: &Profile, tolerance: f64) -> KernelResult<Vec<MedialRegion>> {
        let tol = tess::tolerances();
        let mut model = Model::new();
        let built = crate::profile::build_profile(&mut model, profile).map_err(other)?;
        built
            .faces
            .iter()
            .zip(&built.groups)
            .map(|(face, group)| {
                let graph = medial_graph(&model, face, tolerance, tol)
                    .map_err(|e| other(format!("the medial axis failed: {e}")))?;
                medial_region(
                    &graph,
                    &profile.plane,
                    group.wire_indices.clone(),
                    tolerance,
                    tol,
                )
            })
            .collect()
    }

    fn centre_line(
        &self,
        brep: &[u8],
        from: &FaceProbe,
        to: &FaceProbe,
        tolerance: f64,
    ) -> KernelResult<CentreLine> {
        let tol = tess::tolerances();
        let (mut model, root) = tess::read_blob(brep)?;
        let start = face_named(&mut model, &root, from)?;
        let end = face_named(&mut model, &root, to)?;
        let solid = crate::ops::solids_of(&model, &root)
            .into_iter()
            .find(|solid| {
                explore_unique(&model, solid, ShapeType::Face)
                    .is_ok_and(|faces| faces.iter().any(|f| f.is_same(&start)))
            })
            .ok_or_else(|| other("the first face is on no solid"))?;
        let path = ogeom::offset::middle_path(&mut model, &solid, &start, &end, tolerance, tol)
            .map_err(|e| other(format!("no centre line between those faces: {e}")))?;
        let wire = path.built.shape;
        let deflection = Deflection {
            chord: tolerance * 0.25,
            ..Deflection::default()
        };
        let length = linear_properties(&model, &wire, deflection, tol)
            .map_err(|e| other(format!("measuring the centre line failed: {e}")))?
            .mass;
        let edges = explore_unique(&model, &wire, ShapeType::Edge).map_err(other)?;
        let mut points: Vec<[f64; 3]> = Vec::new();
        for edge in &edges {
            let mut run = edge_points(&model, edge, deflection, tol)?;
            // Each edge runs on from where the last one stopped.
            if let (Some(last), Some(first), Some(tail)) = (points.last(), run.first(), run.last())
                && distance(*last, *tail) < distance(*last, *first)
            {
                run.reverse();
            }
            if !points.is_empty() && !run.is_empty() {
                run.remove(0);
            }
            points.extend(run);
        }
        if let (Some(first), Some(last)) = (points.first(), points.last())
            && distance(*last, from.point) < distance(*first, from.point)
        {
            points.reverse();
        }
        let straight = edges.len() == 1 && is_line(&model, &edges[0]);
        Ok(CentreLine {
            points,
            length,
            deviation: path.deviation,
            straight,
        })
    }

    fn probe(&self, brep: &[u8], probe: &ShapeProbe) -> KernelResult<ProbeAnswer> {
        let (mut model, root) = tess::read_blob(brep)?;
        crate::probe::answer(&mut model, &root, probe).map_err(other)
    }
}

/// A profile plane as the kernel's.
fn plane_of(plane: &ProfilePlane) -> KernelResult<Plane> {
    let tol = tess::tolerances();
    let direction = |v: [f64; 3]| Direction::new(Vector::new(v[0], v[1], v[2]), tol);
    let frame = Frame::from_axes(
        Point::new(plane.origin[0], plane.origin[1], plane.origin[2]),
        direction(plane.x_axis).map_err(other)?,
        direction(plane.y_axis).map_err(other)?,
        direction(plane.normal).map_err(other)?,
        tol,
    )
    .map_err(other)?;
    Ok(Plane::new(frame))
}

/// How far off the face a section edge may run and still be its, in mm.
const ON_FACE_MM: f64 = 1e-4;

/// The edges where `face` crosses `plane`: its solid sectioned by the half
/// space the plane bounds, and of the section the edges that run on the
/// face.
fn face_section(
    model: &mut Model,
    root: &Shape,
    face: &Shape,
    plane: &Plane,
) -> KernelResult<Vec<Shape>> {
    let tol = tess::tolerances();
    let solid = crate::ops::solids_of(model, root)
        .into_iter()
        .find(|solid| {
            explore_unique(model, solid, ShapeType::Face)
                .is_ok_and(|faces| faces.iter().any(|f| f.is_same(face)))
        })
        .ok_or_else(|| other("the face is on no solid"))?;
    // A plane clear of the face's box crosses none of it.
    let frame = plane.frame();
    let (origin, normal) = (frame.origin(), frame.z().vector());
    let sides: Vec<f64> = ogeom::algo::shape_bounds(model, face, tol)
        .map_err(other)?
        .corners()
        .into_iter()
        .map(|corner| normal.dot(corner - origin))
        .collect();
    let reach = tol.confusion();
    if sides.iter().all(|d| *d > reach) || sides.iter().all(|d| *d < -reach) {
        return Ok(Vec::new());
    }
    let surface = ogeom::geom::PlaneSurface::over(*plane, (-1.0e6, 1.0e6), (-1.0e6, 1.0e6))
        .map_err(|e| other(format!("the section plane: {e}")))?;
    let cutter =
        ogeom::algo::make_natural_face(model, ogeom::geom::SurfaceGeometry::Plane(surface))
            .map_err(|e| other(format!("the section plane: {e}")))?
            .shape;
    let below = origin - normal;
    let half = ogeom::algo::make_half_space(model, &cutter, below, tol)
        .map_err(|e| other(format!("the section plane: {e}")))?
        .shape;
    let section = ogeom::boolean::section(model, &solid, &half, tol)
        .map_err(|e| other(format!("the solid could not be cut by the plane: {e}")))?
        .shape;
    let edges = explore_unique(model, &section, ShapeType::Edge).map_err(other)?;
    let mut on_face = Vec::new();
    for edge in edges {
        // Along the edge, away from its ends, where an edge of the next
        // face would meet this one.
        let mut runs_on = true;
        for p in edge_samples(model, &edge, &[0.25, 0.5, 0.75])? {
            let vertex = model.add_vertex(ogeom::topo::VertexData::new(p));
            let gap = distance_between_shapes(
                model,
                &vertex,
                face,
                ogeom::intersect::ExtremaOptions::default(),
                tol,
            )
            .map_err(other)?
            .distance;
            runs_on &= gap <= ON_FACE_MM;
        }
        if runs_on {
            on_face.push(edge);
        }
    }
    Ok(on_face)
}

/// Points of `edge` at fractions of its parameter range, in the model's
/// frame.
fn edge_samples(model: &Model, edge: &Shape, fractions: &[f64]) -> KernelResult<Vec<Point>> {
    let tol = tess::tolerances();
    let Some(NodeData::Edge(data)) = model.node(edge).map(|n| n.data()) else {
        return Err(other("not an edge"));
    };
    let Some(EdgeRepr::Curve3d { curve, range, .. }) = data.curve3d() else {
        return Err(other("an edge has no curve"));
    };
    let geometry = model
        .geometry()
        .curve(*curve)
        .ok_or_else(|| other("an edge's curve is missing"))?;
    let placement = edge.transform(model.datums()).map_err(other)?;
    fractions
        .iter()
        .map(|f| {
            let t = range.0 + (range.1 - range.0) * f;
            let p = geometry.point_at(t, tol).map_err(other)?;
            Ok(placement.apply(p))
        })
        .collect()
}

/// A curve projected onto a plane, in the plane's own coordinates: the
/// exact curve it is there, or points along one with no closed form.
fn in_plane(projected: ProjectedCurve) -> KernelResult<ProjectedEdge> {
    let tol = tess::tolerances();
    Ok(match projected {
        ProjectedCurve::Point(p) => ProjectedEdge::Point([p.x, p.y]),
        ProjectedCurve::Line { start, end } => ProjectedEdge::Line {
            start: [start.x, start.y],
            end: [end.x, end.y],
        },
        ProjectedCurve::Circle {
            centre,
            radius,
            range,
        } => ProjectedEdge::Circle {
            centre: [centre.x, centre.y],
            radius,
            range,
        },
        ProjectedCurve::Ellipse {
            centre,
            major,
            ratio,
            range,
        } => ProjectedEdge::Ellipse {
            centre: [centre.x, centre.y],
            major: [major.x, major.y],
            ratio,
            range,
        },
        ProjectedCurve::BSpline { curve, .. } => {
            let (a, b) = curve.domain();
            let points = (0..=POLYLINE_POINTS)
                .map(|i| {
                    let t = a + (b - a) * i as f64 / POLYLINE_POINTS as f64;
                    curve.point_at(t, tol).map(|p| [p.x, p.y]).map_err(other)
                })
                .collect::<KernelResult<Vec<_>>>()?;
            ProjectedEdge::Polyline(points)
        }
    })
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// The face of `root` a probe names: the nearest to its point, and among
/// faces as near (a point on the edge between them), the one whose outward
/// normal agrees best with the probe's.
pub(crate) fn face_named(
    model: &mut Model,
    root: &Shape,
    probe: &FaceProbe,
) -> KernelResult<Shape> {
    let tol = tess::tolerances();
    let point = Point::new(probe.point[0], probe.point[1], probe.point[2]);
    if let Some(face) = crate::naming::find_face(model, root, probe.name, point) {
        return Ok(face);
    }
    let normal = Vector::new(probe.normal[0], probe.normal[1], probe.normal[2]);
    let vertex = model.add_vertex(ogeom::topo::VertexData::new(point));
    let faces = explore_unique(model, root, ShapeType::Face).map_err(other)?;
    let mut near: Vec<(f64, Shape)> = Vec::new();
    for face in faces {
        let Ok(d) = distance_between_shapes(
            model,
            &vertex,
            &face,
            ogeom::intersect::ExtremaOptions::default(),
            tol,
        ) else {
            continue;
        };
        near.push((d.distance, face));
    }
    let best = near.iter().map(|(d, _)| *d).fold(f64::INFINITY, f64::min);
    let agreement = |face: &Shape| {
        crate::ops::sweep::face_plane(model, face).map_or(0.0, |(_, n)| {
            let outward = if face.orientation() == Orientation::Reversed {
                -n.vector()
            } else {
                n.vector()
            };
            outward.dot(normal)
        })
    };
    near.into_iter()
        .filter(|(d, _)| *d <= best + 1e-6)
        .map(|(_, face)| (agreement(&face), face))
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, face)| face)
        .ok_or_else(|| other("no face near the picked point"))
}

/// Points along an edge, placed, from its start to its end as the edge
/// runs.
fn edge_points(
    model: &Model,
    edge: &Shape,
    deflection: Deflection,
    tol: Tolerances,
) -> KernelResult<Vec<[f64; 3]>> {
    let Some(NodeData::Edge(data)) = model.node(edge).map(|n| n.data()) else {
        return Err(other("the centre line holds no edge"));
    };
    let Some(EdgeRepr::Curve3d { curve, range, .. }) = data.curve3d() else {
        return Err(other("the centre line's edge has no curve"));
    };
    let geometry = model
        .geometry()
        .curve(*curve)
        .ok_or_else(|| other("the centre line's curve is missing"))?;
    let placement = edge.transform(model.datums()).map_err(other)?;
    let line = discretize(geometry, *range, deflection, tol).map_err(other)?;
    let mut points: Vec<[f64; 3]> = line
        .points
        .iter()
        .map(|p| {
            let p = placement.apply(*p);
            [p.x, p.y, p.z]
        })
        .collect();
    if edge.orientation() == Orientation::Reversed {
        points.reverse();
    }
    Ok(points)
}

fn is_line(model: &Model, edge: &Shape) -> bool {
    let Some(NodeData::Edge(data)) = model.node(edge).map(|n| n.data()) else {
        return false;
    };
    let Some(EdgeRepr::Curve3d { curve, .. }) = data.curve3d() else {
        return false;
    };
    matches!(
        model.geometry().curve(*curve),
        Some(ogeom::geom::Curve::Line(_))
    )
}

/// Points taken along each medial branch.
const MEDIAL_SAMPLES: usize = 48;

/// A medial graph read into the profile plane's coordinates, with the
/// narrowest place found.
///
/// The clearance runs down to nothing where a branch ends in a convex
/// corner, and down to a fillet's radius where it ends at a rounded one,
/// without the region getting thinner there, so the branch ends that meet
/// the boundary never count. What counts is a branch point (where three
/// or more branches meet, or a full circle's centre, where none do) and
/// every local minimum inside a branch: a neck, or a strip of constant
/// width.
fn medial_region(
    graph: &MedialGraph,
    plane: &ProfilePlane,
    wires: Vec<usize>,
    tolerance: f64,
    tol: Tolerances,
) -> KernelResult<MedialRegion> {
    let flat = |p: Point| {
        let d = [
            p.x - plane.origin[0],
            p.y - plane.origin[1],
            p.z - plane.origin[2],
        ];
        let dot = |a: [f64; 3]| d[0] * a[0] + d[1] * a[1] + d[2] * a[2];
        [dot(plane.x_axis), dot(plane.y_axis)]
    };
    let mut degree = vec![0usize; graph.vertices.len()];
    for branch in &graph.branches {
        for &end in &branch.ends {
            if let Some(d) = degree.get_mut(end) {
                *d += 1;
            }
        }
    }
    let mut narrowest: Option<Narrowest> = None;
    let mut consider = |at: [f64; 2], clearance: f64| {
        if clearance > tolerance && narrowest.is_none_or(|n| clearance < n.clearance) {
            narrowest = Some(Narrowest { at, clearance });
        }
    };
    for (vertex, &d) in graph.vertices.iter().zip(&degree) {
        if d != 1 {
            consider(flat(vertex.point), vertex.clearance);
        }
    }
    let mut paths = Vec::with_capacity(graph.branches.len());
    for (index, branch) in graph.branches.iter().enumerate() {
        let (a, b) = branch.range;
        let at = |i: usize| a + (b - a) * i as f64 / MEDIAL_SAMPLES as f64;
        let mut points = Vec::with_capacity(MEDIAL_SAMPLES + 1);
        let mut clearance = Vec::with_capacity(MEDIAL_SAMPLES + 1);
        for i in 0..=MEDIAL_SAMPLES {
            let t = at(i);
            let p = branch.curve.point_at(t, tol).map_err(other)?;
            points.push(flat(p));
            clearance.push(graph.clearance_at(index, t, tol).map_err(other)?);
        }
        for i in 1..MEDIAL_SAMPLES {
            if clearance[i] <= clearance[i - 1] && clearance[i] <= clearance[i + 1] {
                let t = least_between(graph, index, at(i - 1), at(i + 1), tol);
                let p = branch.curve.point_at(t, tol).map_err(other)?;
                let c = graph.clearance_at(index, t, tol).map_err(other)?;
                if c <= clearance[i] {
                    consider(flat(p), c);
                } else {
                    consider(points[i], clearance[i]);
                }
            }
        }
        let boundary = |end: usize| degree.get(end) == Some(&1);
        paths.push(MedialPath {
            points,
            clearance,
            boundary_ends: [boundary(branch.ends[0]), boundary(branch.ends[1])],
        });
    }
    Ok(MedialRegion {
        wires,
        paths,
        narrowest,
    })
}

/// Where the clearance along a branch is least between two parameters
/// that bracket a local minimum, by golden-section search.
fn least_between(
    graph: &MedialGraph,
    branch: usize,
    mut lo: f64,
    mut hi: f64,
    tol: Tolerances,
) -> f64 {
    let ratio = (5f64.sqrt() - 1.0) / 2.0;
    let f = |t: f64| graph.clearance_at(branch, t, tol).unwrap_or(f64::INFINITY);
    for _ in 0..40 {
        let m1 = hi - (hi - lo) * ratio;
        let m2 = lo + (hi - lo) * ratio;
        if f(m1) <= f(m2) {
            hi = m2;
        } else {
            lo = m1;
        }
    }
    (lo + hi) / 2.0
}
