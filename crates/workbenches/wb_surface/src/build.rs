//! A surface body's history as the kernel's surface steps: each feature in
//! history order, its sketches as chains in the body's own frame, its picks
//! as probes the kernel finds again.

use core_document::{
    BodyId, BodyPlacement, BuildError, BuildPlan, Document, FeatureId, RebuildJob, WorkbenchFeature,
};
use kernel_api::{CurveSource, EdgeProbe, FaceProbe, ProfilePlane, SolidOp, SurfaceOp};
use wb_sketch::SketchFeature;
use wb_sketch::profile;

use crate::feature::{
    Axis, CurveRef, Direction, EdgePick, FacePick, KIND, PlaneRef, SurfaceFeature,
};

/// The body's surface features in history order, as they build: with
/// every formula's value in.
pub fn surface_features_of_body(
    document: &Document,
    body: BodyId,
) -> Vec<(FeatureId, SurfaceFeature)> {
    let mut features: Vec<(u64, FeatureId, SurfaceFeature)> = document
        .feature_tree()
        .all_nodes()
        .filter(|(_, node)| node.workbench_id.as_str() == KIND && node.body == Some(body))
        .filter_map(|(id, node)| {
            SurfaceFeature::from_json(document.feature_values(*id)?)
                .ok()
                .map(|f| (node.seq, *id, f))
        })
        .collect();
    features.sort_by_key(|(seq, id, _)| (*seq, *id));
    features.into_iter().map(|(_, id, f)| (id, f)).collect()
}

/// Whether the body holds a surface feature: the Surface bench builds it.
pub fn is_surface_body(document: &Document, body: BodyId) -> bool {
    document
        .feature_tree()
        .all_nodes()
        .any(|(_, n)| n.workbench_id.as_str() == KIND && n.body == Some(body))
}

/// Bodies with a surface feature to build again.
fn pending(document: &Document) -> Vec<BodyId> {
    let mut bodies: Vec<BodyId> = document
        .feature_tree()
        .all_nodes()
        .filter(|(_, n)| n.workbench_id.as_str() == KIND && n.dirty)
        .filter_map(|(_, n)| n.body)
        .collect();
    bodies.sort_by_key(|b| b.0);
    bodies.dedup();
    bodies
}

/// The bodies to build again, each with its plan. The dirty flags of the
/// body's surface features and of the sketches they read are settled
/// first, so a plan that fails does not come back every frame.
pub fn rebuild_jobs(document: &mut Document) -> Vec<RebuildJob> {
    pending(document)
        .into_iter()
        .map(|body| {
            let features: Vec<FeatureId> = surface_features_of_body(document, body)
                .into_iter()
                .map(|(id, _)| id)
                .collect();
            let inputs: Vec<FeatureId> = features
                .iter()
                .flat_map(|id| document.feature_tree().dependencies(*id))
                .filter(|dep| document.get_feature_meta(*dep).is_some_and(|n| n.dirty))
                .collect();
            for id in features.iter().chain(&inputs) {
                document.clear_feature_dirty(*id);
            }
            RebuildJob {
                body,
                plan: body_plan(document, body),
            }
        })
        .collect()
}

/// The body's history changed shape: build it from the start, or drop its
/// shape when no surface feature is left.
pub fn invalidate_body(document: &mut Document, body: BodyId) {
    match surface_features_of_body(document, body).first() {
        Some((first, _)) => document.mark_feature_dirty(*first),
        None if !document.body_solid_is_imported(body) => document.remove_imported_geometry(body),
        None => {}
    }
}

pub fn invalidate_all(document: &mut Document) {
    let all: Vec<FeatureId> = document
        .feature_tree()
        .all_nodes()
        .filter(|(_, n)| n.workbench_id.as_str() == KIND)
        .map(|(id, _)| *id)
        .collect();
    for id in all {
        document.mark_feature_dirty(id);
    }
}

/// The body's surface history as the kernel's steps, up to its tip, the
/// suppressed left out.
pub fn body_plan(document: &Document, body: BodyId) -> Result<BuildPlan, BuildError> {
    let tip_seq = document
        .bodies()
        .iter()
        .find(|b| b.id == body)
        .and_then(|b| b.tip)
        .and_then(|tip| document.get_feature_meta(tip))
        .map(|n| n.seq);
    let mut plan = BuildPlan {
        ops: Vec::new(),
        op_features: Vec::new(),
        probes: Vec::new(),
    };
    for (id, feature) in surface_features_of_body(document, body) {
        let Some(node) = document.get_feature_meta(id) else {
            continue;
        };
        if tip_seq.is_some_and(|tip| node.seq > tip) || node.suppressed {
            continue;
        }
        let fail = |message: String| BuildError {
            feature: Some(id),
            message,
        };
        if plan.ops.is_empty() && !feature.constructs() {
            return Err(fail(
                "works on the body's surfaces; make one first (an extrusion, a fill…)".into(),
            ));
        }
        let op = op_of(document, body, &feature).map_err(fail)?;
        plan.ops.push(SolidOp::Surface(op));
        plan.op_features.push(id);
    }
    if plan.ops.is_empty() {
        return Err(BuildError {
            feature: None,
            message: "the body has no surface to build".into(),
        });
    }
    Ok(plan)
}

/// One feature as the kernel's step.
pub fn op_of(
    document: &Document,
    body: BodyId,
    feature: &SurfaceFeature,
) -> Result<SurfaceOp, String> {
    let curves = |refs: &[CurveRef]| sources(document, body, refs);
    Ok(match feature {
        SurfaceFeature::Extrude {
            curves: refs,
            direction,
            length,
            symmetric,
            reversed,
        } => {
            let mut along = direction_of(document, body, refs, *direction)?;
            if *reversed {
                along = along.map(|c| -c);
            }
            SurfaceOp::Extrude {
                curves: curves(refs)?,
                direction: along,
                length: f64::from(*length),
                symmetric: *symmetric,
            }
        }
        SurfaceFeature::Revolve {
            curves: refs,
            axis,
            angle_deg,
        } => {
            let (origin, axis) = axis_of(document, body, refs, *axis)?;
            SurfaceOp::Revolve {
                curves: curves(refs)?,
                origin,
                axis,
                angle_deg: f64::from(*angle_deg),
            }
        }
        SurfaceFeature::PlanarFill { curves: refs } => SurfaceOp::PlanarFill {
            curves: curves(refs)?,
        },
        SurfaceFeature::Fill {
            boundary,
            continuity,
        } => SurfaceOp::Fill {
            boundary: curves(boundary)?,
            continuity: *continuity,
        },
        SurfaceFeature::Ruled { first, second } => {
            let one = |side: &Option<CurveRef>, which: &str| -> Result<CurveSource, String> {
                let side = side.ok_or_else(|| format!("pick the {which} curve"))?;
                let mut got = sources(document, body, &[side])?;
                match got.len() {
                    1 => Ok(got.remove(0)),
                    n => Err(format!(
                        "the {which} curve must be one chain; its sketch has {n}"
                    )),
                }
            };
            SurfaceOp::Ruled {
                first: one(first, "first")?,
                second: one(second, "second")?,
            }
        }
        SurfaceFeature::Loft { sections, closed } => {
            let mut chains = Vec::with_capacity(sections.len());
            for section in sections {
                let mut got = sources(document, body, std::slice::from_ref(section))?;
                if got.len() != 1 {
                    return Err(format!(
                        "each section must be one chain; one has {}",
                        got.len()
                    ));
                }
                chains.push(got.remove(0));
            }
            SurfaceOp::Loft {
                sections: chains,
                closed: *closed,
            }
        }
        SurfaceFeature::Sweep { profile, path } => SurfaceOp::Sweep {
            profile: curves(profile)?,
            path: curves(path)?,
            frame: Default::default(),
        },
        SurfaceFeature::Offset { faces, distance } => SurfaceOp::Offset {
            faces: faces.iter().map(face_probe).collect(),
            distance: f64::from(*distance),
        },
        SurfaceFeature::Extend {
            edges,
            length,
            continuity,
        } => SurfaceOp::Extend {
            edges: edges.iter().map(edge_probe).collect(),
            length: f64::from(*length),
            continuity: *continuity,
        },
        SurfaceFeature::Blend {
            first,
            second,
            continuity,
        } => SurfaceOp::Blend {
            first: edge_probe(&first.ok_or("pick the first edge")?),
            second: edge_probe(&second.ok_or("pick the second edge")?),
            continuity: *continuity,
        },
        SurfaceFeature::Split {
            faces,
            curves: refs,
        } => SurfaceOp::Split {
            faces: faces.iter().map(face_probe).collect(),
            curves: curves(refs)?,
        },
        SurfaceFeature::Sew { gap } => SurfaceOp::Sew {
            gap: f64::from(*gap),
        },
        SurfaceFeature::Fillet { edges, radius } => SurfaceOp::Fillet {
            edges: edges.iter().map(edge_probe).collect(),
            radius: f64::from(*radius),
        },
        SurfaceFeature::Thicken {
            thickness,
            both_sides,
        } => SurfaceOp::Thicken {
            thickness: f64::from(*thickness),
            both_sides: *both_sides,
        },
        SurfaceFeature::Trim {
            plane,
            offset,
            flip,
        } => {
            let (origin, normal) = plane_of(*plane, *offset);
            SurfaceOp::TrimByPlane {
                origin,
                normal: if *flip { normal.map(|c| -c) } else { normal },
            }
        }
        SurfaceFeature::Mirror { plane, offset } => {
            let (origin, normal) = plane_of(*plane, *offset);
            SurfaceOp::Mirror { origin, normal }
        }
    })
}

fn edge_probe(pick: &EdgePick) -> EdgeProbe {
    EdgeProbe {
        point: pick.point.map(f64::from),
        direction: pick.direction.map(f64::from),
        faces: pick.faces,
    }
}

fn face_probe(pick: &FacePick) -> FaceProbe {
    FaceProbe {
        point: pick.point.map(f64::from),
        normal: pick.normal.map(f64::from),
        name: pick.name,
    }
}

/// A sketch as its body holds it, with every formula's value in, solved.
pub fn load_sketch(document: &Document, id: FeatureId) -> Result<SketchFeature, String> {
    let data = document
        .feature_values(id)
        .ok_or("reads a sketch that is gone")?;
    SketchFeature::from_json(data).map_err(|e| format!("reads a sketch it cannot: {e}"))
}

/// Where a sketch of another body sits in `body`'s frame: from the
/// sketch's body to the world, then into `body`.
fn into_body(document: &Document, body: BodyId, sketch: FeatureId) -> BodyPlacement {
    let from = document
        .get_feature_meta(sketch)
        .and_then(|n| n.body)
        .unwrap_or(body);
    if from == body {
        return BodyPlacement::default();
    }
    document
        .body_placement(body)
        .inverse()
        .after(&document.body_placement(from))
}

/// A sketch's plane in `body`'s frame.
fn sketch_plane(document: &Document, body: BodyId, id: FeatureId) -> Result<ProfilePlane, String> {
    let sketch = load_sketch(document, id)?;
    let moved = into_body(document, body, id);
    let p = &sketch.plane;
    Ok(profile::plane_of(&wb_sketch::sketch::SketchPlane {
        origin: moved.point(p.origin),
        normal: moved.direction(p.normal),
        x_axis: moved.direction(p.x_axis),
        y_axis: moved.direction(p.y_axis),
    }))
}

/// The curves as the kernel takes them: each sketch's chains, open or
/// closed, on its plane; each edge as a probe.
pub fn sources(
    document: &Document,
    body: BodyId,
    refs: &[CurveRef],
) -> Result<Vec<CurveSource>, String> {
    if refs.is_empty() {
        return Err("pick the curves it is built from".into());
    }
    let mut out = Vec::new();
    for curve in refs {
        match curve {
            CurveRef::Sketch(id) => {
                let sketch = load_sketch(document, *id)?;
                let plane = sketch_plane(document, body, *id)?;
                let chains = profile::extract_chains(&sketch.sketch).map_err(|e| e.to_string())?;
                out.extend(
                    chains
                        .into_iter()
                        .map(|wire| CurveSource::Sketch { plane, wire }),
                );
            }
            CurveRef::Edge(pick) => out.push(CurveSource::Edge(edge_probe(pick))),
        }
    }
    Ok(out)
}

/// The first sketch's plane among the curves, when there is one.
fn first_plane(document: &Document, body: BodyId, refs: &[CurveRef]) -> Option<ProfilePlane> {
    refs.iter().find_map(|r| match r {
        CurveRef::Sketch(id) => sketch_plane(document, body, *id).ok(),
        CurveRef::Edge(_) => None,
    })
}

fn direction_of(
    document: &Document,
    body: BodyId,
    refs: &[CurveRef],
    direction: Direction,
) -> Result<[f64; 3], String> {
    Ok(match direction {
        Direction::SketchNormal => first_plane(document, body, refs)
            .map(|p| p.normal)
            .ok_or("the direction follows a sketch's plane; pick a sketch, or another direction")?,
        Direction::X => [1.0, 0.0, 0.0],
        Direction::Y => [0.0, 1.0, 0.0],
        Direction::Z => [0.0, 0.0, 1.0],
        Direction::Custom(d) => d.map(f64::from),
    })
}

fn axis_of(
    document: &Document,
    body: BodyId,
    refs: &[CurveRef],
    axis: Axis,
) -> Result<([f64; 3], [f64; 3]), String> {
    let plane = || {
        first_plane(document, body, refs)
            .ok_or("the axis is a sketch's; pick a sketch, or another axis")
    };
    Ok(match axis {
        Axis::SketchVertical => {
            let p = plane()?;
            (p.origin, p.y_axis)
        }
        Axis::SketchHorizontal => {
            let p = plane()?;
            (p.origin, p.x_axis)
        }
        Axis::X => ([0.0; 3], [1.0, 0.0, 0.0]),
        Axis::Y => ([0.0; 3], [0.0, 1.0, 0.0]),
        Axis::Z => ([0.0; 3], [0.0, 0.0, 1.0]),
        Axis::Custom { origin, direction } => (origin.map(f64::from), direction.map(f64::from)),
    })
}

/// A plane's point and normal, moved `offset` along the normal.
fn plane_of(plane: PlaneRef, offset: f32) -> ([f64; 3], [f64; 3]) {
    let (origin, normal) = match plane {
        PlaneRef::YZ => ([0.0; 3], [1.0, 0.0, 0.0]),
        PlaneRef::XZ => ([0.0; 3], [0.0, 1.0, 0.0]),
        PlaneRef::XY => ([0.0; 3], [0.0, 0.0, 1.0]),
        PlaneRef::Custom { origin, normal } => (origin.map(f64::from), normal.map(f64::from)),
    };
    let length = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
    let unit = if length > 0.0 {
        normal.map(|c| c / length)
    } else {
        normal
    };
    let shift = f64::from(offset);
    ([0, 1, 2].map(|k| origin[k] + unit[k] * shift), normal)
}
