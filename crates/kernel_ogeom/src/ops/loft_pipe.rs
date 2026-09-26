//! Loft through section profiles and sweep along a sketch spine.

use kernel_api::{PipeCorner, PipeFrame, Profile, ProfileSegment};
use ogeom::algo::transformed;
use ogeom::geom::Curve3d;
use ogeom::math::{Axis, Direction, Point, Transform, Vector};
use ogeom::offset::{make_loft, make_loft_skinned, make_loft_skinned_closed, make_pipe_shell};
use ogeom::topo::{Model, NodeData, Shape};

use super::{fuse_all, tol};
use crate::profile::{self, BuiltProfile};

const SKIN_TOLERANCE: f64 = 1e-3;

struct Section {
    outer: Shape,
    holes: Vec<Shape>,
}

fn single_region(model: &mut Model, prof: &Profile, what: &str) -> Result<Section, String> {
    let built = profile::build_profile(model, prof)?;
    let mut groups = built.groups;
    if groups.len() != 1 {
        return Err(format!("{what} must enclose a single region"));
    }
    let group = groups.remove(0);
    Ok(Section {
        outer: group.outer.wire,
        holes: group.holes.into_iter().map(|h| h.wire).collect(),
    })
}

pub fn loft_tool(
    model: &mut Model,
    sections: &[Profile],
    ruled: bool,
    closed: bool,
) -> Result<Shape, String> {
    if sections.len() < 2 {
        return Err("loft needs at least two sections".into());
    }
    let built: Vec<Section> = sections
        .iter()
        .map(|p| single_region(model, p, "a loft section"))
        .collect::<Result<_, _>>()?;

    let hole_count = built[0].holes.len();
    if built.iter().any(|s| s.holes.len() != hole_count) {
        return Err("loft sections must have matching hole counts".into());
    }

    let outers: Vec<Shape> = built.iter().map(|s| s.outer.clone()).collect();
    let mut solid = loft_wires(model, &outers, ruled, closed)?;

    for hole in 0..hole_count {
        let hole_wires: Vec<Shape> = built.iter().map(|s| s.holes[hole].clone()).collect();
        let hole_solid = loft_wires(model, &hole_wires, ruled, closed)?;
        solid = ogeom::boolean::cut(model, &solid, &hole_solid, tol())
            .map_err(|e| format!("subtracting a hole loft failed: {e}"))?
            .shape;
    }
    Ok(solid)
}

fn loft_wires(
    model: &mut Model,
    wires: &[Shape],
    ruled: bool,
    closed: bool,
) -> Result<Shape, String> {
    if closed {
        return make_loft_skinned_closed(model, wires, SKIN_TOLERANCE, tol())
            .map(|b| b.shape)
            .map_err(|e| format!("closed loft failed: {e}"));
    }
    if ruled {
        // Chain of two-section ruled lofts, fused.
        let mut parts = Vec::with_capacity(wires.len() - 1);
        for pair in wires.windows(2) {
            let part = make_loft(model, &pair[0], &pair[1], tol())
                .map_err(|e| format!("ruled loft failed: {e}"))?;
            parts.push(part.shape);
        }
        fuse_all(model, parts)
    } else {
        make_loft_skinned(model, wires, SKIN_TOLERANCE, tol())
            .map(|b| b.shape)
            .map_err(|e| format!("loft failed: {e}"))
    }
}

/// Sweep `prof` down `spine`, the section turning as `frame` says and
/// turning the path's sharp corners as `corner` says.
pub fn pipe_tool(
    model: &mut Model,
    prof: &Profile,
    spine: &Profile,
    frame: &PipeFrame,
    corner: PipeCorner,
    sections: &[Profile],
) -> Result<Shape, String> {
    if !sections.is_empty() {
        return Err("the geometry kernel cannot yet sweep a pipe through several sections".into());
    }
    let built = profile::build_profile(model, prof)?;

    let spine_wire_desc = spine
        .wires
        .first()
        .ok_or_else(|| "pipe spine has no wire".to_string())?;
    let spine_built = profile::build_wire_edges(model, &spine.plane, spine_wire_desc, false)
        .map_err(|e| format!("pipe spine: {e}"))?;
    let spine_wire = spine_built.wire.clone();

    if corner != PipeCorner::Transformed && has_corner(model, &spine_built.edges)? {
        let name = match corner {
            PipeCorner::Right => "right",
            _ => "round",
        };
        return Err(format!(
            "the geometry kernel cannot yet turn a pipe's corners {name}; \
             a path with sharp corners sweeps with transformed corners"
        ));
    }
    let frenet = match frame {
        PipeFrame::RotationMinimizing => false,
        PipeFrame::Frenet => true,
        PipeFrame::Binormal { direction } => {
            binormal_holds_by_itself(spine, *direction)?;
            false
        }
        PipeFrame::Auxiliary { path } => {
            let wire = path
                .wires
                .first()
                .ok_or_else(|| "the pipe's auxiliary path has no wire".to_string())?;
            profile::build_wire(model, &path.plane, wire, false)
                .map_err(|e| format!("the pipe's auxiliary path: {e}"))?;
            return Err(
                "the geometry kernel cannot yet orient a pipe's section by an auxiliary path"
                    .into(),
            );
        }
    };

    let (start, tangent) = spine_start(model, &spine_wire)?;

    // The sweep wants the profile sitting at the spine start, square to its
    // tangent. Sketches rarely oblige exactly: rotate the profile's normal
    // onto the tangent and move its centroid onto the start.
    let centroid = profile::profile_centroid(model, &built)?;
    let normal = profile::plane_normal(&prof.plane).map_err(|e| format!("pipe profile: {e}"))?;
    let rotate = rotation_aligning(normal, tangent, centroid);
    let translate = Transform::translation(start - centroid);
    let place = translate * rotate;

    let mut parts = Vec::with_capacity(built.faces.len());
    for face in &built.faces {
        let placed = transformed(model, face, place)
            .map_err(|e| format!("placing the pipe profile failed: {e}"))?
            .shape;
        let part = make_pipe_shell(model, &placed, &spine_wire, frenet, SKIN_TOLERANCE, tol())
            .map_err(|e| format!("pipe sweep failed: {e}"))?;
        parts.push(part.shape);
    }
    let _ = &built as &BuiltProfile;
    fuse_all(model, parts)
}

/// A fixed binormal the rotation-minimizing frame holds exactly by itself:
/// the normal of a path lying in one plane, which that frame never turns
/// away from, or any direction off a straight path, along which the frame
/// never turns at all. Any other binormal is refused.
fn binormal_holds_by_itself(spine: &Profile, direction: [f64; 3]) -> Result<(), String> {
    let d = Vector::new(direction[0], direction[1], direction[2]);
    let len = d.magnitude();
    if !(len.is_finite() && len > 1e-9) {
        return Err("the pipe's binormal direction is zero".into());
    }
    let d = d * (1.0 / len);
    let n = profile::plane_normal(&spine.plane)
        .map_err(|e| format!("pipe spine: {e}"))?
        .vector();
    if d.cross(n).magnitude() <= 1e-9 {
        return Ok(());
    }
    if let Some(along) = straight_run(spine) {
        if d.cross(along).magnitude() <= 1e-9 {
            return Err("the pipe's binormal runs along its path".into());
        }
        return Ok(());
    }
    Err(
        "the geometry kernel cannot yet hold a pipe's binormal leaning off the plane \
         of a path that bends; a binormal square to that plane holds"
            .into(),
    )
}

/// The world direction of a spine made of one straight run of lines.
fn straight_run(spine: &Profile) -> Option<Vector> {
    let [wire] = spine.wires.as_slice() else {
        return None;
    };
    let axis = |a: [f64; 3]| Vector::new(a[0], a[1], a[2]);
    let (x, y) = (axis(spine.plane.x_axis), axis(spine.plane.y_axis));
    let mut along: Option<[f64; 2]> = None;
    for segment in &wire.segments {
        let ProfileSegment::Line { start, end } = segment else {
            return None;
        };
        let d = [end[0] - start[0], end[1] - start[1]];
        let len = d[0].hypot(d[1]);
        if len <= 1e-12 {
            continue;
        }
        let d = [d[0] / len, d[1] / len];
        match along {
            None => along = Some(d),
            Some(a) if (a[0] * d[1] - a[1] * d[0]).abs() <= 1e-9 => {}
            Some(_) => return None,
        }
    }
    along.map(|a| x * a[0] + y * a[1])
}

/// Whether consecutive edges of a spine meet at an angle (the closing join
/// of a closed spine included), by the test the sweep itself makes: the
/// tangents arriving at and leaving the join differ in direction.
fn has_corner(model: &Model, edges: &[Shape]) -> Result<bool, String> {
    let ends = edges
        .iter()
        .map(|e| edge_ends(model, e))
        .collect::<Result<Vec<_>, _>>()?;
    let mut pairs: Vec<(usize, usize)> = (1..ends.len()).map(|i| (i - 1, i)).collect();
    if ends.len() > 2 {
        pairs.push((ends.len() - 1, 0));
    }
    let angular = tol().angular();
    let reach = tol().confusion() * 10.0;
    for (a, b) in pairs {
        // The ends that meet: the arriving tangent runs into the join, the
        // leaving one out of it.
        let mut best: Option<(f64, Vector, Vector)> = None;
        for (ia, (pa, ta)) in ends[a].iter().enumerate() {
            for (ib, (pb, tb)) in ends[b].iter().enumerate() {
                let gap = pa.distance(*pb);
                let arriving = if ia == 1 { *ta } else { -*ta };
                let leaving = if ib == 0 { *tb } else { -*tb };
                if best.as_ref().is_none_or(|(g, ..)| gap < *g) {
                    best = Some((gap, arriving, leaving));
                }
            }
        }
        let Some((gap, arriving, leaving)) = best else {
            continue;
        };
        if gap > reach {
            continue;
        }
        if arriving.cross(leaving).magnitude() > angular || arriving.dot(leaving) < 0.0 {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Each end of an edge with its unit tangent in the edge's own sense.
fn edge_ends(model: &Model, edge: &Shape) -> Result<[(Point, Vector); 2], String> {
    let node = model
        .node(edge)
        .ok_or_else(|| "pipe spine edge is not in the model".to_string())?;
    let NodeData::Edge(data) = node.data() else {
        return Err("pipe spine child is not an edge".into());
    };
    for repr in &data.representations {
        if let ogeom::topo::EdgeRepr::Curve3d {
            curve,
            location,
            range,
        } = repr
        {
            let Some(geometry) = model.geometry().curve(*curve) else {
                continue;
            };
            let placement = location
                .composed(model.datums())
                .map_err(|e| format!("pipe spine placement: {e}"))?;
            let mut out = Vec::with_capacity(2);
            for t in [range.0, range.1] {
                let p = geometry
                    .point_at(t, tol())
                    .map_err(|e| format!("pipe spine end: {e}"))?;
                let d1 = geometry
                    .d1_at(t, tol())
                    .map_err(|e| format!("pipe spine tangent: {e}"))?;
                let at = placement.apply(p);
                let toward = placement.apply(p + d1) - at;
                let len = toward.magnitude();
                if len <= 1e-12 {
                    return Err("pipe spine tangent is degenerate".into());
                }
                out.push((at, toward * (1.0 / len)));
            }
            return Ok([out[0], out[1]]);
        }
    }
    Err("pipe spine edge carries no 3D curve".into())
}

/// Start point and unit tangent of a wire's first edge.
fn spine_start(model: &Model, wire: &Shape) -> Result<(Point, Vector), String> {
    let edges = model
        .children_of(wire)
        .map_err(|e| format!("pipe spine wire: {e}"))?;
    let first = edges
        .first()
        .ok_or_else(|| "pipe spine wire has no edges".to_string())?;
    let node = model
        .node(first)
        .ok_or_else(|| "pipe spine edge is not in the model".to_string())?;
    let NodeData::Edge(data) = node.data() else {
        return Err("pipe spine child is not an edge".into());
    };
    for repr in &data.representations {
        if let ogeom::topo::EdgeRepr::Curve3d {
            curve,
            location,
            range,
        } = repr
        {
            let Some(geometry) = model.geometry().curve(*curve) else {
                continue;
            };
            // The exact tangent: the sweep reads the profile's lean
            // against it, and a chord's direction is off on a curve.
            let t0 = range.0;
            let p0 = geometry
                .point_at(t0, tol())
                .map_err(|e| format!("pipe spine start: {e}"))?;
            let d1 = geometry
                .d1_at(t0, tol())
                .map_err(|e| format!("pipe spine tangent: {e}"))?;
            let placement = location
                .composed(model.datums())
                .map_err(|e| format!("pipe spine placement: {e}"))?;
            let start = placement.apply(p0);
            let toward = placement.apply(p0 + d1) - start;
            let len = toward.magnitude();
            if len <= 1e-12 {
                return Err("pipe spine tangent is degenerate".into());
            }
            return Ok((start, toward * (1.0 / len)));
        }
    }
    Err("pipe spine edge carries no 3D curve".into())
}

/// Rotation about the profile centroid taking `normal` onto whichever of
/// ±`tangent` it is closer to. Identity when already aligned.
fn rotation_aligning(normal: Direction, tangent: Vector, about: Point) -> Transform {
    let n = normal.vector();
    let t = if n.dot(tangent) >= 0.0 {
        tangent
    } else {
        -tangent
    };
    let cross = n.cross(t);
    let sin = cross.magnitude();
    let cos = n.dot(t);
    let angle = sin.atan2(cos);
    if angle.abs() < 1e-9 {
        return Transform::IDENTITY;
    }
    let Ok(axis_dir) = Direction::new(cross, tol()) else {
        return Transform::IDENTITY;
    };
    Transform::rotation(Axis::new(about, axis_dir), angle)
}
