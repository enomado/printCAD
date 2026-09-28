//! A datum's references to what it stands on: made from picks or from a
//! command's arguments, filled in from the body's solid where the kernel is
//! at hand, and written back out as the arguments that make them again.

use core_document::{
    Args, BasePlane, BodyId, CommandError, DatumAttachment, DatumFeature, EdgeAnchor, EdgeRef,
    EdgeSpot, FaceAnchor, FaceRef, FeatureId, PlaneAnchor, PointAnchor, WorkbenchFeature,
    WorkbenchRuntimeContext,
};
use serde_json::{Map, Value, json};

use crate::commands::vector3;

/// The modes a datum attaches by, as `part.datum` names them, with what
/// each is.
pub const MODES: &[(&str, &str)] = &[
    ("base_plane", "on a base plane (plane)"),
    (
        "face",
        "on a face, tangent to it where curved (face_point, face_normal)",
    ),
    ("three_points", "through three points (points)"),
    (
        "normal_to_edge",
        "square to an edge (edge_point, edge_direction, spot)",
    ),
    (
        "along_edge",
        "along an edge, or a circle's axis (edge_point, edge_direction)",
    ),
    ("two_points", "through two points (points)"),
    ("plane_intersection", "where two planes meet (planes)"),
    (
        "curve_centre",
        "at a circular edge's centre (edge_point, edge_direction)",
    ),
    (
        "inertia",
        "at the body's centre of mass, on its axes of inertia",
    ),
];

/// A picked face as a datum keeps it; `follows` when it is on the datum's
/// own body.
pub fn face_anchor(face: &FaceRef, follows: bool) -> FaceAnchor {
    FaceAnchor {
        name: face.name,
        point: face.point,
        normal: face.normal,
        surface: face.surface,
        follows,
    }
}

/// A picked edge as a datum keeps it; `follows` when it is on the datum's
/// own body.
pub fn edge_anchor(edge: &EdgeRef, follows: bool) -> EdgeAnchor {
    EdgeAnchor {
        faces: edge.faces,
        point: edge.point,
        direction: edge.direction,
        ends: None,
        middle: None,
        circle: edge.circle.map(|c| core_document::AnchorCircle {
            center: c.center,
            normal: c.normal,
            radius: c.radius,
        }),
        follows,
    }
}

/// Fill in what the body's solid says of the datum's references (a face's
/// surface, an edge's ends, the centre of mass) and check they make a
/// frame. Without a kernel or a solid the references stay as given, and a
/// datum at the centre of mass cannot be made.
pub fn settle(
    ctx: &WorkbenchRuntimeContext,
    body: BodyId,
    datum: &mut DatumFeature,
) -> Result<(), String> {
    let probes = datum.probes();
    if !probes.is_empty() {
        let solid = ctx
            .kernel
            .zip(ctx.document.imported_brep_blob(body))
            .filter(|_| !ctx.document.is_mesh_body(body));
        match solid {
            Some((kernel, blob)) => {
                let answers: Vec<Result<kernel_api::ProbeAnswer, String>> = probes
                    .iter()
                    .map(|probe| kernel.probe(blob, probe).map_err(|e| e.to_string()))
                    .collect();
                if let Some(Err(e)) = answers.iter().find(|a| a.is_err()) {
                    return Err(format!("the body's solid does not answer: {e}"));
                }
                datum.take_answers(&answers);
            }
            None if matches!(datum.attachment, DatumAttachment::Inertia { .. }) => {
                return Err("the centre of mass needs the body's solid".into());
            }
            None => {}
        }
    }
    match datum.attachment.problem() {
        Some(problem) => Err(problem.to_string()),
        None => Ok(()),
    }
}

fn face_from(value: &Value, name: &str) -> Result<FaceAnchor, CommandError> {
    Ok(FaceAnchor {
        name: 0,
        point: vector3(value.get("face_point"), &format!("{name}.face_point"))?,
        normal: vector3(value.get("face_normal"), &format!("{name}.face_normal"))?,
        surface: None,
        follows: true,
    })
}

fn edge_from(value: &Value, name: &str) -> Result<EdgeAnchor, CommandError> {
    Ok(EdgeAnchor {
        faces: [0, 0],
        point: vector3(value.get("edge_point"), &format!("{name}.edge_point"))?,
        direction: match value.get("edge_direction") {
            Some(v) if !v.is_null() => vector3(Some(v), &format!("{name}.edge_direction"))?,
            _ => [0.0; 3],
        },
        ends: None,
        middle: None,
        circle: None,
        follows: true,
    })
}

fn spot_from(value: Option<&Value>, name: &str) -> Result<EdgeSpot, CommandError> {
    match value {
        None | Some(Value::Null) => Ok(EdgeSpot::Picked),
        Some(v) => v
            .as_str()
            .and_then(EdgeSpot::from_key)
            .ok_or_else(|| CommandError::bad(name, "must be picked, start, end, middle or centre")),
    }
}

fn base_plane(name: &str) -> Option<BasePlane> {
    BasePlane::ALL
        .into_iter()
        .find(|p| base_plane_key(*p).eq_ignore_ascii_case(name))
}

fn base_plane_key(plane: BasePlane) -> &'static str {
    match plane {
        BasePlane::XY => "XY",
        BasePlane::XZ => "XZ",
        BasePlane::YZ => "YZ",
    }
}

fn point_from(value: &Value, name: &str) -> Result<PointAnchor, CommandError> {
    if value.get("face_point").is_some() {
        Ok(PointAnchor::Face {
            face: face_from(value, name)?,
        })
    } else if value.get("edge_point").is_some() {
        Ok(PointAnchor::Edge {
            edge: edge_from(value, name)?,
            spot: spot_from(value.get("spot"), &format!("{name}.spot"))?,
        })
    } else {
        Ok(PointAnchor::At {
            point: vector3(Some(value), name)?,
        })
    }
}

fn plane_from(
    value: &Value,
    name: &str,
    ctx: &WorkbenchRuntimeContext,
    body: BodyId,
) -> Result<PlaneAnchor, CommandError> {
    if let Some(text) = value.as_str() {
        if let Some(plane) = base_plane(text) {
            return Ok(PlaneAnchor::Base(plane));
        }
        let datum = uuid::Uuid::parse_str(text)
            .map(FeatureId)
            .map_err(|_| CommandError::bad(name, "must be XY, XZ, YZ or a datum's id"))?;
        let node = ctx
            .document
            .get_feature_meta(datum)
            .filter(|n| n.workbench_id.as_str() == core_document::DATUM_KIND)
            .ok_or_else(|| CommandError::bad(name, "is not a datum of this document"))?;
        if node.body != Some(body) {
            return Err(CommandError::bad(name, "is a datum of another body"));
        }
        let made = ctx
            .document
            .feature_values(datum)
            .and_then(|v| DatumFeature::from_json(v).ok())
            .ok_or_else(|| CommandError::bad(name, "is a datum that does not read"))?;
        if !has_plane(&made) {
            return Err(CommandError::bad(
                name,
                "is a datum line or point, which has no plane",
            ));
        }
        let frame = made.frame();
        return Ok(PlaneAnchor::Datum {
            datum,
            origin: frame.origin,
            normal: frame.normal,
        });
    }
    Ok(PlaneAnchor::Face {
        face: face_from(value, name)?,
    })
}

/// Whether a datum has a plane another can be made from: a plane, or a
/// coordinate system by its XY plane.
pub fn has_plane(datum: &DatumFeature) -> bool {
    matches!(
        datum.shape,
        core_document::DatumShape::Plane { .. }
            | core_document::DatumShape::CoordinateSystem { .. }
    )
}

fn anchors<const N: usize, T>(
    a: &Args,
    name: &str,
    mut each: impl FnMut(&Value, &str) -> Result<T, CommandError>,
) -> Result<[T; N], CommandError> {
    let list = match a.0.get(name) {
        Some(Value::Array(list)) if list.len() == N => list,
        _ => return Err(CommandError::bad(name, format!("must list {N}"))),
    };
    let made: Vec<T> = list
        .iter()
        .enumerate()
        .map(|(i, v)| each(v, &format!("{name}[{}]", i + 1)))
        .collect::<Result<_, _>>()?;
    made.try_into()
        .map_err(|_| CommandError::bad(name, format!("must list {N}")))
}

/// The attachment `part.datum`'s arguments describe, in `body`'s own
/// frame.
pub fn attachment_from_args(
    a: &Args,
    ctx: &WorkbenchRuntimeContext,
    body: BodyId,
) -> Result<DatumAttachment, CommandError> {
    let edge = || edge_from(&Value::Object(a.0.clone()), "edge");
    let mode = match a.opt_string("mode")? {
        Some(mode) => mode,
        None if a.has("face_point") => "flat_face",
        None => "base_plane",
    };
    Ok(match mode {
        "base_plane" => {
            let name = a.opt_string("plane")?.unwrap_or("XY");
            DatumAttachment::BasePlane(
                base_plane(name)
                    .ok_or_else(|| CommandError::bad("plane", "must be XY, XZ or YZ"))?,
            )
        }
        "flat_face" => DatumAttachment::FlatFace {
            point: vector3(a.0.get("face_point"), "face_point")?,
            normal: vector3(a.0.get("face_normal"), "face_normal")?,
        },
        "face" => DatumAttachment::Face {
            face: FaceAnchor {
                name: 0,
                point: vector3(a.0.get("face_point"), "face_point")?,
                normal: vector3(a.0.get("face_normal"), "face_normal")?,
                surface: None,
                follows: true,
            },
        },
        "three_points" => DatumAttachment::ThreePoints {
            points: anchors(a, "points", point_from)?,
        },
        "two_points" => DatumAttachment::TwoPoints {
            points: anchors(a, "points", point_from)?,
        },
        "normal_to_edge" => DatumAttachment::NormalToEdge {
            edge: edge()?,
            spot: spot_from(a.0.get("spot"), "spot")?,
        },
        "along_edge" => DatumAttachment::AlongEdge { edge: edge()? },
        "curve_centre" => DatumAttachment::CurveCentre { edge: edge()? },
        "plane_intersection" => DatumAttachment::PlaneIntersection {
            planes: anchors(a, "planes", |v, name| plane_from(v, name, ctx, body))?,
        },
        "inertia" => DatumAttachment::Inertia {
            centre: [0.0; 3],
            axes: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        },
        _ => {
            let modes: Vec<&str> = MODES.iter().map(|(m, _)| *m).collect();
            return Err(CommandError::bad(
                "mode",
                format!("must be one of {}", modes.join(", ")),
            ));
        }
    })
}

fn face_args(face: &FaceAnchor) -> Map<String, Value> {
    let mut args = Map::new();
    args.insert("face_point".into(), json!(face.point));
    args.insert("face_normal".into(), json!(face.normal));
    args
}

fn edge_args(edge: &EdgeAnchor) -> Map<String, Value> {
    let mut args = Map::new();
    args.insert("edge_point".into(), json!(edge.point));
    args.insert("edge_direction".into(), json!(edge.direction));
    args
}

fn point_args(point: &PointAnchor) -> Value {
    match point {
        PointAnchor::At { point } => json!(point),
        PointAnchor::Face { face } => Value::Object(face_args(face)),
        PointAnchor::Edge { edge, spot } => {
            let mut args = edge_args(edge);
            args.insert("spot".into(), json!(spot.key()));
            Value::Object(args)
        }
    }
}

fn plane_args(plane: &PlaneAnchor) -> Value {
    match plane {
        PlaneAnchor::Base(plane) => json!(base_plane_key(*plane)),
        PlaneAnchor::Datum { datum, .. } => json!(datum.0.to_string()),
        PlaneAnchor::Face { face } => Value::Object(face_args(face)),
    }
}

/// The arguments of `part.datum` that make `attachment` again, as the
/// references were picked.
pub fn attachment_args(attachment: &DatumAttachment) -> Map<String, Value> {
    let mut args = match attachment {
        DatumAttachment::BasePlane(plane) => {
            let mut args = Map::new();
            args.insert("plane".into(), json!(base_plane_key(*plane)));
            args
        }
        DatumAttachment::FlatFace { point, normal } => {
            let mut args = Map::new();
            args.insert("face_point".into(), json!(point));
            args.insert("face_normal".into(), json!(normal));
            args
        }
        DatumAttachment::Face { face } => face_args(face),
        DatumAttachment::ThreePoints { points } => {
            let mut args = Map::new();
            args.insert("points".into(), points.iter().map(point_args).collect());
            args
        }
        DatumAttachment::TwoPoints { points } => {
            let mut args = Map::new();
            args.insert("points".into(), points.iter().map(point_args).collect());
            args
        }
        DatumAttachment::NormalToEdge { edge, spot } => {
            let mut args = edge_args(edge);
            args.insert("spot".into(), json!(spot.key()));
            args
        }
        DatumAttachment::AlongEdge { edge } | DatumAttachment::CurveCentre { edge } => {
            edge_args(edge)
        }
        DatumAttachment::PlaneIntersection { planes } => {
            let mut args = Map::new();
            args.insert("planes".into(), planes.iter().map(plane_args).collect());
            args
        }
        DatumAttachment::Inertia { .. } => Map::new(),
    };
    args.insert("mode".into(), json!(attachment.mode()));
    args
}

/// The datum's dependencies changed with its attachment: the document's
/// record of them follows.
pub fn sync_dependencies(ctx: &mut WorkbenchRuntimeContext, id: FeatureId, datum: &DatumFeature) {
    let wanted = datum.dependencies();
    let current = ctx.document.feature_tree().dependencies(id);
    if current != wanted {
        ctx.document.set_feature_dependencies(id, wanted);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_document::{Document, Workbench};
    use kernel_api::{KernelQueries, KernelResult, ProbeAnswer, ProbedCircle, ShapeProbe};

    use crate::PartDesignWorkbench;

    /// A kernel that knows one solid: a cylinder of radius 5 on the XY
    /// plane, 12 high, its rim at the top, and a straight edge from the
    /// origin to (10, 0, 0).
    struct Cylinder;

    impl KernelQueries for Cylinder {
        fn project_edge(
            &self,
            _brep: &[u8],
            _near: [f64; 3],
            _plane: &kernel_api::ProfilePlane,
        ) -> KernelResult<kernel_api::ProjectedEdge> {
            Err(kernel_api::KernelError::Unsupported("projecting".into()))
        }

        fn probe(&self, _brep: &[u8], probe: &ShapeProbe) -> KernelResult<ProbeAnswer> {
            Ok(match *probe {
                ShapeProbe::Face { point, .. } => {
                    let r = (point[0] * point[0] + point[1] * point[1]).sqrt();
                    let n = [point[0] / r, point[1] / r, 0.0];
                    ProbeAnswer::Face {
                        point: [n[0] * 5.0, n[1] * 5.0, point[2]],
                        normal: n,
                        surface: kernel_api::FaceSurface::Cylinder {
                            origin: [0.0; 3],
                            axis: [0.0, 0.0, 1.0],
                            radius: 5.0,
                        },
                    }
                }
                ShapeProbe::Edge { point, .. } if point[2] > 6.0 => ProbeAnswer::Edge {
                    point: [5.0, 0.0, 12.0],
                    direction: [0.0, 1.0, 0.0],
                    start: [5.0, 0.0, 12.0],
                    end: [5.0, 0.0, 12.0],
                    middle: [-5.0, 0.0, 12.0],
                    circle: Some(ProbedCircle {
                        centre: [0.0, 0.0, 12.0],
                        normal: [0.0, 0.0, 1.0],
                        radius: 5.0,
                    }),
                },
                ShapeProbe::Edge { point, .. } => ProbeAnswer::Edge {
                    point: [point[0], 0.0, 0.0],
                    direction: [1.0, 0.0, 0.0],
                    start: [0.0; 3],
                    end: [10.0, 0.0, 0.0],
                    middle: [5.0, 0.0, 0.0],
                    circle: None,
                },
                ShapeProbe::Mass => ProbeAnswer::Mass {
                    centre: [0.0, 0.0, 6.0],
                    axes: [[0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                },
            })
        }
    }

    static CYLINDER: Cylinder = Cylinder;

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-4)
    }

    fn parallel(a: [f32; 3], b: [f32; 3]) -> bool {
        (a[0] * b[0] + a[1] * b[1] + a[2] * b[2]).abs() > 1.0 - 1e-5
    }

    /// A document with one body whose solid the stand-in kernel answers
    /// for.
    fn with_solid() -> (Document, BodyId) {
        let mut doc = Document::new("t");
        let body = doc.create_body(None);
        doc.set_imported_brep_data(body, b"cylinder".to_vec(), Vec::new());
        (doc, body)
    }

    fn datum(
        doc: &mut Document,
        body: BodyId,
        kernel: bool,
        args: Value,
    ) -> Result<(FeatureId, DatumFeature), core_document::CommandError> {
        let mut bench = PartDesignWorkbench::default();
        let mut args = args.as_object().unwrap().clone();
        args.insert("body".into(), json!(body.0.to_string()));
        let made = {
            let mut ctx = WorkbenchRuntimeContext::new(doc, [0.0; 3], [0.0; 3], (0, 0, 1, 1));
            if kernel {
                ctx.kernel = Some(&CYLINDER);
            }
            bench.run_command("part.datum", &args, &mut ctx)?
        };
        let id = FeatureId(uuid::Uuid::parse_str(made.as_str().unwrap()).unwrap());
        let datum = DatumFeature::from_json(doc.get_feature_data(id).unwrap()).unwrap();
        Ok((id, datum))
    }

    #[test]
    fn a_plane_on_a_face_is_tangent_to_the_cylinder_where_picked() {
        let (mut doc, body) = with_solid();
        let (_, made) = datum(
            &mut doc,
            body,
            true,
            json!({"kind": "plane", "mode": "face",
                   "face_point": [0, 5.2, 4], "face_normal": [0, 1, 0]}),
        )
        .unwrap();
        let frame = made.frame();
        assert!(close(frame.origin, [0.0, 5.0, 4.0]), "{frame:?}");
        assert!(close(frame.normal, [0.0, 1.0, 0.0]));
        assert!(parallel(frame.x_axis, [0.0, 0.0, 1.0]), "along the axis");
        assert_eq!(made.probes().len(), 1, "it follows the solid");
    }

    #[test]
    fn three_points_and_two_points_take_given_and_picked_points() {
        let (mut doc, body) = with_solid();
        let (_, plane) = datum(
            &mut doc,
            body,
            false,
            json!({"kind": "plane", "mode": "three_points",
                   "points": [[0, 0, 2], [1, 0, 2], {"x": 0, "y": 1, "z": 2}]}),
        )
        .unwrap();
        assert!(close(plane.frame().normal, [0.0, 0.0, 1.0]));
        assert!(close(plane.frame().origin, [0.0, 0.0, 2.0]));

        let (_, line) = datum(
            &mut doc,
            body,
            true,
            json!({"kind": "line", "mode": "two_points", "points": [
                {"edge_point": [3, 0, 0], "edge_direction": [1, 0, 0], "spot": "end"},
                [10, 0, 8],
            ]}),
        )
        .unwrap();
        let frame = line.frame();
        assert!(close(frame.origin, [10.0, 0.0, 4.0]), "{frame:?}");
        assert!(parallel(frame.x_axis, [0.0, 0.0, 1.0]));
    }

    #[test]
    fn points_in_a_line_make_no_plane() {
        let (mut doc, body) = with_solid();
        let refused = datum(
            &mut doc,
            body,
            false,
            json!({"kind": "plane", "mode": "three_points",
                   "points": [[0, 0, 0], [1, 0, 0], [2, 0, 0]]}),
        );
        assert!(refused.is_err());
    }

    #[test]
    fn edge_modes_stand_on_the_edge_the_solid_names() {
        let (mut doc, body) = with_solid();
        let (_, square) = datum(
            &mut doc,
            body,
            true,
            json!({"kind": "plane", "mode": "normal_to_edge",
                   "edge_point": [3, 0, 0], "edge_direction": [1, 0, 0], "spot": "end"}),
        )
        .unwrap();
        assert!(close(square.frame().origin, [10.0, 0.0, 0.0]));
        assert!(parallel(square.frame().normal, [1.0, 0.0, 0.0]));

        let (_, axis) = datum(
            &mut doc,
            body,
            true,
            json!({"kind": "line", "mode": "along_edge",
                   "edge_point": [5, 0, 12], "edge_direction": [0, 1, 0]}),
        )
        .unwrap();
        assert!(close(axis.frame().origin, [0.0, 0.0, 12.0]));
        assert!(parallel(axis.frame().x_axis, [0.0, 0.0, 1.0]));

        let (_, centre) = datum(
            &mut doc,
            body,
            true,
            json!({"kind": "point", "mode": "curve_centre",
                   "edge_point": [5, 0, 12], "edge_direction": [0, 1, 0]}),
        )
        .unwrap();
        assert!(close(centre.frame().origin, [0.0, 0.0, 12.0]));

        // Without the solid to ask, a straight pick has no centre.
        let refused = datum(
            &mut doc,
            body,
            false,
            json!({"kind": "point", "mode": "curve_centre",
                   "edge_point": [5, 0, 12], "edge_direction": [0, 1, 0]}),
        );
        assert!(refused.is_err());
    }

    #[test]
    fn two_planes_meet_in_a_line_that_follows_a_datum_it_is_made_from() {
        let (mut doc, body) = with_solid();
        let (base, _) = datum(
            &mut doc,
            body,
            false,
            json!({"kind": "plane", "offset": [0, 0, 4]}),
        )
        .unwrap();
        let (line_id, line) = datum(
            &mut doc,
            body,
            false,
            json!({"kind": "line", "mode": "plane_intersection",
                   "planes": [base.0.to_string(), "YZ"]}),
        )
        .unwrap();
        let frame = line.frame();
        assert!(parallel(frame.x_axis, [0.0, 1.0, 0.0]));
        assert!((frame.origin[2] - 4.0).abs() < 1e-4 && frame.origin[0].abs() < 1e-4);
        assert_eq!(doc.feature_tree().dependencies(line_id), vec![base]);

        let other = doc.create_body(None);
        let refused = datum(
            &mut doc,
            other,
            false,
            json!({"kind": "line", "mode": "plane_intersection",
                   "planes": [base.0.to_string(), "YZ"]}),
        );
        assert!(refused.is_err(), "a datum of another body");
        let parallel_planes = datum(
            &mut doc,
            body,
            false,
            json!({"kind": "line", "mode": "plane_intersection", "planes": ["XY", "XY"]}),
        );
        assert!(parallel_planes.is_err());
        let from_a_line = datum(
            &mut doc,
            body,
            false,
            json!({"kind": "line", "mode": "plane_intersection",
                   "planes": [line_id.0.to_string(), "XY"]}),
        );
        assert!(from_a_line.is_err(), "a datum line has no plane");
    }

    #[test]
    fn a_coordinate_system_at_the_centre_of_mass_takes_the_axes_of_inertia() {
        let (mut doc, body) = with_solid();
        let (_, made) = datum(
            &mut doc,
            body,
            true,
            json!({"kind": "coordinate_system", "mode": "inertia"}),
        )
        .unwrap();
        let frame = made.frame();
        assert!(close(frame.origin, [0.0, 0.0, 6.0]));
        assert!(close(frame.x_axis, [0.0, 0.0, 1.0]));
        assert!(close(frame.normal, [0.0, 1.0, 0.0]));

        let refused = datum(
            &mut doc,
            body,
            false,
            json!({"kind": "point", "mode": "inertia"}),
        );
        assert!(refused.is_err(), "no kernel to weigh the solid with");
    }

    /// What a recording writes of a datum makes the same datum again.
    #[test]
    fn every_mode_s_arguments_make_it_again() {
        let (mut doc, body) = with_solid();
        let (base, _) = datum(&mut doc, body, false, json!({"kind": "plane"})).unwrap();
        let made = [
            json!({"mode": "base_plane", "plane": "XZ"}),
            json!({"face_point": [0, 0, 3], "face_normal": [0, 0, 1]}),
            json!({"mode": "face", "face_point": [0, 5, 4], "face_normal": [0, 1, 0]}),
            json!({"mode": "three_points", "points": [
                [0, 0, 2],
                {"face_point": [1, 0, 2], "face_normal": [0, 0, 1]},
                {"edge_point": [0, 1, 2], "edge_direction": [1, 0, 0], "spot": "middle"},
            ]}),
            json!({"mode": "normal_to_edge", "edge_point": [1, 0, 0],
                   "edge_direction": [1, 0, 0], "spot": "start"}),
            json!({"mode": "along_edge", "edge_point": [1, 0, 0], "edge_direction": [1, 0, 0]}),
            json!({"mode": "two_points", "points": [[0, 0, 0], [0, 0, 1]]}),
            json!({"mode": "plane_intersection", "planes": [
                base.0.to_string(),
                {"face_point": [0, 5, 0], "face_normal": [0, 1, 0]},
            ]}),
        ];
        for args in made {
            let mut with_kind = args.as_object().unwrap().clone();
            with_kind.insert("kind".into(), json!("plane"));
            let (_, first) = datum(&mut doc, body, false, Value::Object(with_kind.clone()))
                .unwrap_or_else(|e| panic!("{args}: {e}"));
            let mut again = attachment_args(&first.attachment);
            again.insert("kind".into(), json!("plane"));
            let (_, second) = datum(&mut doc, body, false, Value::Object(again)).unwrap();
            assert_eq!(first.attachment, second.attachment, "{args}");
        }
    }

    #[test]
    fn a_build_asks_each_datum_s_questions_where_it_stands() {
        let mut doc = Document::new("t");
        let body = doc.create_body(None);
        let mut bench = PartDesignWorkbench::default();
        let early = datum(
            &mut doc,
            body,
            false,
            json!({"kind": "plane", "mode": "face",
                   "face_point": [0, 5, 4], "face_normal": [0, 1, 0]}),
        )
        .unwrap()
        .0;
        {
            let mut ctx = WorkbenchRuntimeContext::new(&mut doc, [0.0; 3], [0.0; 3], (0, 0, 1, 1));
            let args = json!({"body": body.0.to_string(), "variant": "cylinder"});
            bench
                .run_command("part.primitive", args.as_object().unwrap(), &mut ctx)
                .unwrap();
        }
        let late = datum(
            &mut doc,
            body,
            false,
            json!({"kind": "point", "mode": "inertia_free_face",
                   "face_point": [0, 5, 4], "face_normal": [0, 1, 0]}),
        );
        assert!(late.is_err(), "an unknown mode is refused");
        let late = datum(
            &mut doc,
            body,
            false,
            json!({"kind": "point", "mode": "face",
                   "face_point": [0, 5, 4], "face_normal": [0, 1, 0]}),
        )
        .unwrap()
        .0;

        let jobs = crate::build::rebuild_jobs(&mut doc);
        let plan = jobs[0].plan.as_ref().unwrap();
        let after = |feature: FeatureId| {
            plan.probes
                .iter()
                .find(|p| p.feature == feature)
                .map(|p| p.probe.after_op)
        };
        assert_eq!(after(early), Some(0), "before the cylinder: nothing to ask");
        assert_eq!(
            after(late),
            Some(plan.ops.len()),
            "after all the cylinder's ops"
        );
        assert!(crate::build::rebuild_jobs(&mut doc).is_empty());

        // A datum stood on something else: the body is built again to ask.
        let mut data = doc.get_feature_data(late).unwrap().clone();
        data["attachment"]["Face"]["face"]["point"] = json!([5.0, 0.0, 4.0]);
        doc.update_feature_data(late, data).unwrap();
        assert_eq!(crate::build::rebuild_jobs(&mut doc).len(), 1);
    }
}
