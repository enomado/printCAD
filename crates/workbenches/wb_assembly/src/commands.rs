//! Assembly's commands: joints between bodies and moving bodies, by
//! numbers, for scripts and other callers that are not a click.
//!
//! A joint takes faces as `pc.doc.faces` lists them, where the bodies sit
//! now: a flat face as `{point, normal}`, a round one as its `axis`. Each
//! is stored in its body's own frame, as a picked face is, and every
//! joint solves as it is made or changed.

use core_document::{
    Args, BodyId, BodyPlacement, CommandArgs, CommandError, CommandResult, CommandSpec, FeatureId,
    ParamKind, WorkbenchContext, WorkbenchRuntimeContext,
};
use glam::{Quat, Vec3};
use serde_json::{Value, json};

use crate::coupling::{COUPLING_KIND, Coupling, Gearing};
use crate::joint::{Anchor, Drive, JOINT_KIND, JointFeature, JointKind, JointTool, Rigid, Takes};
use crate::solve::joints;

const DRIVE: &str = "A hinge's angle (degrees from where it was made) or a slider's \
    position (mm) to hold it at; false lets it move again";
const GEARING: &str = "gears (hinges turning opposite ways), belt (the same way), \
    rack (a hinge and a slider, by the pinion's pitch radius) or screw (by the lead); \
    the first that suits the two joints when left out";
const RATIO: &str = "Turns of the driven hinge per turn of the driver for gears and a \
    belt, the pitch radius in mm for a rack, the lead in mm a turn for a screw";
const LIMITS: &str = "{low, high}: the range a hinge's angle or a slider's position stays \
    in while not driven; false takes the limits away";

/// Turn a joint's moving body by `degrees` about the joint's direction,
/// or half a turn over with `over`, and carry the joint's settings with
/// it so it holds the body there.
pub(crate) fn turn_joint(
    ctx: &mut WorkbenchRuntimeContext,
    joint: &crate::Joint,
    degrees: f64,
    over: bool,
) -> Result<(), CommandError> {
    let at = |b: BodyId| -> Rigid { ctx.document.body_placement(b).into() };
    let (before, fixed) = (at(joint.body), at(joint.feature.other_body));
    let step = joint.feature.turning_step(&before, &fixed, degrees, over);
    let after = before.then(&step);
    let mut feature = joint.feature.clone();
    feature.carried(&before, &after, &fixed);
    ctx.document
        .update_feature_data(joint.id, core_document::WorkbenchFeature::to_json(&feature))
        .map_err(|e| CommandError::failed(e.to_string()))?;
    ctx.document.clear_feature_dirty(joint.id);
    ctx.document.set_body_placement(joint.body, after.into());
    Ok(())
}

/// Read `drive` and `limits` into a hinge's or a slider's drive.
fn drive_args(a: &Args, drive: &mut Drive) -> Result<(), CommandError> {
    drive_args_named(a, drive, "drive", "limits")
}

/// Read the arguments named `to` and `range` into a drive.
fn drive_args_named(
    a: &Args,
    drive: &mut Drive,
    to_name: &str,
    range: &str,
) -> Result<(), CommandError> {
    match a.0.get(to_name) {
        None => {}
        Some(Value::Bool(false)) | Some(Value::Null) => drive.to = None,
        Some(v) => {
            let to = v
                .as_f64()
                .ok_or_else(|| CommandError::bad(to_name, "must be a number or false"))?;
            drive.to = Some(to as f32);
        }
    }
    match a.0.get(range) {
        None => {}
        Some(Value::Bool(false)) | Some(Value::Null) => drive.limits = None,
        Some(v) => {
            let bad = || CommandError::bad(range, "must be {low, high} or false");
            let pair = v.as_array().filter(|l| l.len() == 2).ok_or_else(bad)?;
            let low = pair[0].as_f64().ok_or_else(bad)? as f32;
            let high = pair[1].as_f64().ok_or_else(bad)? as f32;
            if low > high {
                return Err(CommandError::bad(range, "low must not be above high"));
            }
            drive.limits = Some([low, high]);
        }
    }
    Ok(())
}

/// An alignment's drives: its turn and its slide, each held or limited.
fn align_drives(spec: CommandSpec) -> CommandSpec {
    spec.optional(
        "turn_drive",
        ParamKind::Any,
        "An alignment's turn (degrees from where it was made) to hold it at; false lets it turn",
    )
    .optional(
        "turn_limits",
        ParamKind::Any,
        "{low, high}: the range an alignment's turn stays in; false takes it away",
    )
    .optional(
        "slide_drive",
        ParamKind::Any,
        "How far along the axis (mm) to hold an alignment; false lets it slide",
    )
    .optional(
        "slide_limits",
        ParamKind::Any,
        "{low, high}: the range an alignment's slide stays in, mm; false takes it away",
    )
}

/// Register every command this module runs.
pub fn register(context: &mut WorkbenchContext) {
    let joint = |id: &str, summary: &str, face: &str| {
        CommandSpec::new(id, summary)
            .param("body", ParamKind::Id, "The body that moves")
            .param("face", ParamKind::Any, face)
            .param("other", ParamKind::Id, "The body it is held against")
            .param("other_face", ParamKind::Any, face)
            .optional("name", ParamKind::String, "Its name in the tree")
    };
    for tool in JointTool::ALL {
        let face = match tool.takes() {
            Takes::Flat => "A flat face, {point, normal}, as pc.doc.faces lists it",
            Takes::Round => {
                "A round face, {axis = {point, direction}}, as pc.doc.faces lists it; \
                 an edge's line or circle axis goes the same way"
            }
            Takes::Any => "Any face, as pc.doc.faces lists it; the body's origin when left out",
            Takes::FlatAndRound => {
                "A flat face {point, normal} on one body and a round face {axis, radius} \
                 on the other, either way round"
            }
        };
        let spec = if tool == JointTool::Fixed {
            CommandSpec::new(tool.command(), tool.summary())
                .param("body", ParamKind::Id, "The body that moves")
                .optional("face", ParamKind::Any, face)
                .param("other", ParamKind::Id, "The body it is held against")
                .optional("other_face", ParamKind::Any, face)
                .optional("name", ParamKind::String, "Its name in the tree")
        } else {
            joint(tool.command(), tool.summary(), face)
        };
        let spec = match tool {
            JointTool::Mate => spec
                .optional("offset", ParamKind::Number, "The gap between them, mm")
                .optional(
                    "flip",
                    ParamKind::Bool,
                    "Face the same way instead of at each other",
                ),
            JointTool::Angle => spec.optional(
                "degrees",
                ParamKind::Number,
                "Between their outward normals; the angle they make now when left out",
            ),
            JointTool::Hinge => spec
                .optional(
                    "offset",
                    ParamKind::Number,
                    "How far along the axis the first sits from the second, mm",
                )
                .optional("drive", ParamKind::Any, DRIVE)
                .optional("limits", ParamKind::Any, LIMITS),
            JointTool::Slider => spec.optional("drive", ParamKind::Any, DRIVE).optional(
                "limits",
                ParamKind::Any,
                LIMITS,
            ),
            JointTool::Align => align_drives(spec),
            JointTool::Distance => spec.optional(
                "offset",
                ParamKind::Number,
                "Along the second face's normal, mm; the distance they are now when left out",
            ),
            JointTool::Tangent => spec.optional(
                "radius",
                ParamKind::Number,
                "The round face's radius, mm; the face's own when left out",
            ),
            _ => spec,
        };
        context.register_command(spec.returns("the joint's id"));
    }
    context.register_command(
        CommandSpec::new(
            "asm.couple",
            "Tie two joints' motions together: gears or a belt between two hinges, a \
             rack and pinion or a screw between a hinge and a slider",
        )
        .param("driver", ParamKind::Id, "The hinge or slider that leads")
        .param("driven", ParamKind::Id, "The hinge or slider that follows")
        .optional("gearing", ParamKind::String, GEARING)
        .optional("ratio", ParamKind::Number, RATIO)
        .optional(
            "reverse",
            ParamKind::Bool,
            "The driven joint moves the other way",
        )
        .optional("name", ParamKind::String, "Its name in the tree")
        .returns("the coupling's id"),
    );
    let set = CommandSpec::new(
        "asm.set",
        "Change a joint's gap, side, angle or radius, or a coupling's joints and ratio",
    )
    .param("joint", ParamKind::Id, "A joint or a coupling")
    .optional("gearing", ParamKind::String, GEARING)
    .optional("ratio", ParamKind::Number, RATIO)
    .optional(
        "reverse",
        ParamKind::Bool,
        "A coupling's driven joint moves the other way",
    )
    .optional("driver", ParamKind::Id, "A coupling's leading joint")
    .optional("driven", ParamKind::Id, "A coupling's following joint")
    .optional(
        "offset",
        ParamKind::Number,
        "A mate's gap, a hinge's height or a distance, mm",
    )
    .optional("flip", ParamKind::Bool, "A mate's side")
    .optional("degrees", ParamKind::Number, "An angle joint's angle")
    .optional("radius", ParamKind::Number, "A tangent's radius, mm")
    .optional("drive", ParamKind::Any, DRIVE)
    .optional("limits", ParamKind::Any, LIMITS);
    context.register_command(align_drives(set));
    context.register_command(
        CommandSpec::new(
            "asm.turn",
            "Turn a joint's body about the joint's axis or normal, the joint keeping it there",
        )
        .param("joint", ParamKind::Id, "The joint")
        .param("degrees", ParamKind::Number, "How far, degrees"),
    );
    context.register_command(
        CommandSpec::new(
            "asm.flip",
            "Turn a joint's body over, half a turn across the joint's axis or normal",
        )
        .param("joint", ParamKind::Id, "The joint"),
    );
    context.register_command(
        CommandSpec::new(
            "asm.interference",
            "Where solid bodies share material: each pair that clashes, how much \
             and where",
        )
        .optional(
            "bodies",
            ParamKind::List,
            "Only these bodies; every visible one when left out",
        )
        .optional(
            "clearance",
            ParamKind::Number,
            "Look instead for pairs nearer than this many mm",
        )
        .returns(
            "{checked, skipped, clashes}, each clash {a, b, volume (mm³), centre}; \
             skipped counts visible bodies with no solid. With a clearance, {checked, \
             skipped, near}, each {a, b, distance (mm), on_a, on_b}, nearest first",
        )
        .read_only(),
    );
    context.register_command(
        CommandSpec::new(
            "asm.mass",
            "The mass and centre of mass of the solid bodies at one density",
        )
        .optional(
            "bodies",
            ParamKind::List,
            "Only these bodies; every visible one when left out",
        )
        .optional("density", ParamKind::Number, "g/cm³ (1 when left out)")
        .returns(
            "{mass (g), volume (mm³), centre = {x, y, z} or nil, bodies = {{body, mass, \
             volume, centre}, ...}, skipped}",
        )
        .read_only(),
    );
    context.register_command(
        CommandSpec::new(
            "asm.parts",
            "Every part: bodies of the same shape counted together",
        )
        .returns(
            "a list of {name, quantity, bodies, size = {x, y, z} in mm or nil, mesh}, \
             in name order",
        )
        .read_only(),
    );
    context.register_command(
        CommandSpec::new(
            "asm.travel",
            "Where a hinge or a slider has got to: the hinge's angle in degrees, \
             the slider's position in mm",
        )
        .param("joint", ParamKind::Id, "")
        .returns("a number")
        .read_only(),
    );
    context.register_command(
        CommandSpec::new(
            "asm.ground",
            "Keep a body where it is: the bodies joined to it are placed against it",
        )
        .param("body", ParamKind::Id, "")
        .optional(
            "grounded",
            ParamKind::Bool,
            "false lets it move again (true by default)",
        )
        .returns("the ground joint's id, or nil when it was taken away"),
    );
    context.register_command(
        CommandSpec::new(
            "asm.freedom",
            "What each jointed body may still do: the motions its joints leave open",
        )
        .optional("body", ParamKind::Id, "Only this body")
        .returns(
            "a list of {body, free, motions}, each motion {turn = {axis, through}} \
             or {slide = direction}, with at_limit true where a limit lets it go one \
             way only",
        )
        .read_only(),
    );
    context.register_command(
        CommandSpec::new("asm.solve", "Place every body its joints hold")
            .returns("what moved, in words"),
    );
    context.register_command(
        CommandSpec::new("asm.placement", "Where a body sits")
            .param("body", ParamKind::Id, "")
            .returns("{translation, rotation}, rotation a quaternion {x, y, z, w}")
            .read_only(),
    );
    context.register_command(
        CommandSpec::new("asm.place", "Put a body at a placement")
            .param("body", ParamKind::Id, "")
            .optional("translation", ParamKind::List, "{x, y, z} in mm")
            .optional("rotation", ParamKind::List, "A quaternion {x, y, z, w}"),
    );
    context.register_command(
        CommandSpec::new("asm.move", "Move a body by a step and a turn")
            .param("body", ParamKind::Id, "")
            .optional("by", ParamKind::List, "{x, y, z} in mm")
            .optional("turn", ParamKind::Number, "Degrees about `axis`")
            .optional("axis", ParamKind::List, "{x, y, z}; Z when left out")
            .optional(
                "about",
                ParamKind::List,
                "The point the turn is about, {x, y, z}; the origin when left out",
            ),
    );
}

/// Run command `id`.
pub fn run(id: &str, args: &CommandArgs, ctx: &mut WorkbenchRuntimeContext) -> CommandResult {
    let a = Args(args);
    match id {
        id if JointTool::of_command(id).is_some() => make_joint(id, &a, ctx),
        "asm.couple" => couple(&a, ctx),
        "asm.set"
            if ctx
                .document
                .get_feature_meta(FeatureId(a.id("joint")?))
                .is_some_and(|n| n.workbench_id.as_str() == COUPLING_KIND) =>
        {
            set_coupling(&a, ctx)
        }
        "asm.set" => {
            let joint = FeatureId(a.id("joint")?);
            let not_a_joint = || CommandError::bad("joint", "is not a joint");
            let node = ctx
                .document
                .get_feature_meta(joint)
                .filter(|n| n.workbench_id.as_str() == JOINT_KIND)
                .ok_or_else(not_a_joint)?;
            let mut feature: JointFeature =
                serde_json::from_value(node.data.clone()).map_err(|_| not_a_joint())?;
            match &mut feature.kind {
                JointKind::Mate { flip, offset } => {
                    if let Some(v) = a.opt_number("offset")? {
                        *offset = v as f32;
                    }
                    if let Some(v) = a.opt_bool("flip")? {
                        *flip = v;
                    }
                }
                JointKind::Angle { degrees } => {
                    if let Some(v) = a.opt_number("degrees")? {
                        *degrees = v as f32;
                    }
                }
                JointKind::Hinge { offset, drive, .. } => {
                    if let Some(v) = a.opt_number("offset")? {
                        *offset = v as f32;
                    }
                    drive_args(&a, drive)?;
                }
                JointKind::Slider { drive, .. } => drive_args(&a, drive)?,
                JointKind::Distance { offset } => {
                    if let Some(v) = a.opt_number("offset")? {
                        *offset = v as f32;
                    }
                }
                JointKind::Tangent { radius } => {
                    if let Some(v) = a.opt_number("radius")? {
                        *radius = v as f32;
                    }
                }
                JointKind::Align { turn, slide, .. } => {
                    drive_args_named(&a, turn, "turn_drive", "turn_limits")?;
                    drive_args_named(&a, slide, "slide_drive", "slide_limits")?;
                }
                JointKind::Ground
                | JointKind::Fixed { .. }
                | JointKind::Parallel
                | JointKind::Perpendicular => {}
            }
            let data =
                serde_json::to_value(&feature).map_err(|e| CommandError::failed(e.to_string()))?;
            ctx.document
                .update_feature_data(joint, data)
                .map_err(|e| CommandError::failed(e.to_string()))?;
            ctx.document.clear_feature_dirty(joint);
            solved(ctx, Value::Null)
        }
        "asm.mass" => {
            let kernel = ctx
                .kernel
                .ok_or_else(|| CommandError::failed("no kernel to measure with"))?;
            let among = body_list(&a)?;
            let density = a
                .opt_number("density")?
                .unwrap_or(f64::from(DEFAULT_DENSITY));
            let report = crate::mass::plan(ctx.document, among.as_deref())
                .run(
                    kernel,
                    &std::sync::atomic::AtomicUsize::new(0),
                    &std::sync::atomic::AtomicBool::new(false),
                )
                .map_err(CommandError::failed)?;
            let bodies: Vec<Value> = report
                .bodies
                .iter()
                .map(|b| {
                    json!({
                        "body": b.body.0.to_string(),
                        "mass": b.volume_mm3 * density / 1000.0,
                        "volume": b.volume_mm3,
                        "centre": b.centre,
                    })
                })
                .collect();
            Ok(json!({
                "mass": report.mass_g(density),
                "volume": report.volume_mm3(),
                "centre": report.centre(),
                "bodies": bodies,
                "skipped": report.skipped,
            }))
        }
        "asm.interference" => {
            let kernel = ctx
                .kernel
                .ok_or_else(|| CommandError::failed("no kernel to check with"))?;
            let among = body_list(&a)?;
            if let Some(gap) = a.opt_number("clearance")? {
                let found =
                    crate::interference::plan_clearance(ctx.document, among.as_deref(), gap)
                        .run(
                            kernel,
                            &std::sync::atomic::AtomicUsize::new(0),
                            &std::sync::atomic::AtomicBool::new(false),
                        )
                        .map_err(CommandError::failed)?;
                let near: Vec<Value> = found
                    .near
                    .iter()
                    .map(|n| {
                        json!({
                            "a": n.a.0.to_string(),
                            "b": n.b.0.to_string(),
                            "distance": n.distance_mm,
                            "on_a": n.on_a,
                            "on_b": n.on_b,
                        })
                    })
                    .collect();
                return Ok(json!({
                    "checked": found.checked,
                    "skipped": found.skipped,
                    "near": near,
                }));
            }
            let found = crate::interference(ctx.document, kernel, among.as_deref())
                .map_err(CommandError::failed)?;
            let clashes: Vec<Value> = found
                .clashes
                .iter()
                .map(|c| {
                    json!({
                        "a": c.a.0.to_string(),
                        "b": c.b.0.to_string(),
                        "volume": c.volume_mm3,
                        "centre": c.centre,
                    })
                })
                .collect();
            Ok(json!({
                "checked": found.checked,
                "skipped": found.skipped,
                "clashes": clashes,
            }))
        }
        "asm.parts" => Ok(Value::Array(
            crate::parts_list(ctx.document)
                .into_iter()
                .map(|part| {
                    json!({
                        "name": part.name,
                        "quantity": part.bodies.len(),
                        "bodies": part.bodies.iter().map(|b| b.0.to_string()).collect::<Vec<_>>(),
                        "size": part.size_mm,
                        "mesh": part.mesh,
                    })
                })
                .collect(),
        )),
        "asm.travel" => {
            let joint = FeatureId(a.id("joint")?);
            let found = joints(ctx.document).into_iter().find(|j| j.id == joint);
            let found = found.ok_or_else(|| CommandError::bad("joint", "is not a joint"))?;
            let at = |b: BodyId| -> crate::Rigid { ctx.document.body_placement(b).into() };
            let travel = found
                .feature
                .travel(&at(found.body), &at(found.feature.other_body))
                .ok_or_else(|| CommandError::bad("joint", "is not a hinge or a slider"))?;
            Ok(json!(travel))
        }
        "asm.ground" => {
            let body = BodyId(a.id("body")?);
            if !ctx.document.bodies().iter().any(|b| b.id == body) {
                return Err(CommandError::bad("body", "is not a body"));
            }
            let grounded = a.opt_bool("grounded")?.unwrap_or(true);
            let id = set_grounded(ctx, body, grounded);
            solved(ctx, id.map_or(Value::Null, |id| json!(id.0.to_string())))
        }
        "asm.freedom" => {
            let only = a.opt_id("body")?.map(BodyId);
            Ok(Value::Array(
                crate::freedom(ctx.document)
                    .into_iter()
                    .filter(|(body, _)| only.is_none_or(|o| o == *body))
                    .map(|(body, motions)| {
                        json!({
                            "body": body.0.to_string(),
                            "free": motions.len(),
                            "motions": motions.iter().map(|m| match m {
                                crate::Motion::Turn { axis, through, at_limit } => json!({
                                    "turn": {"axis": axis, "through": through},
                                    "at_limit": at_limit,
                                }),
                                crate::Motion::Slide { direction, at_limit } => {
                                    json!({"slide": direction, "at_limit": at_limit})
                                }
                            }).collect::<Vec<_>>(),
                        })
                    })
                    .collect(),
            ))
        }
        "asm.solve" => crate::apply_solve(ctx)
            .map(Value::String)
            .map_err(CommandError::failed),
        "asm.placement" => {
            let placement = ctx.document.body_placement(body(&a, ctx)?);
            Ok(json!({
                "translation": placement.translation,
                "rotation": placement.rotation,
            }))
        }
        "asm.place" => {
            let body = body(&a, ctx)?;
            let now = ctx.document.body_placement(body);
            let translation = match a.0.get("translation") {
                Some(v) if !v.is_null() => vector(v, "translation")?,
                _ => Vec3::from(now.translation),
            };
            let rotation = match a.0.get("rotation") {
                Some(v) if !v.is_null() => quaternion(v)?,
                _ => now.quat(),
            };
            ctx.document
                .set_body_placement(body, BodyPlacement::new(rotation, translation));
            Ok(Value::Null)
        }
        "asm.turn" | "asm.flip" => {
            let joint = FeatureId(a.id("joint")?);
            let found = joints(ctx.document)
                .into_iter()
                .find(|j| j.id == joint && j.feature.kind != JointKind::Ground)
                .ok_or_else(|| CommandError::bad("joint", "is not a joint between two bodies"))?;
            let degrees = if id == "asm.turn" {
                a.number("degrees")?
            } else {
                0.0
            };
            turn_joint(ctx, &found, degrees, id == "asm.flip")?;
            solved(ctx, Value::Null)
        }
        "asm.move" => {
            let body = body(&a, ctx)?;
            let by = match a.0.get("by") {
                Some(v) if !v.is_null() => vector(v, "by")?,
                _ => Vec3::ZERO,
            };
            let axis = match a.0.get("axis") {
                Some(v) if !v.is_null() => vector(v, "axis")?,
                _ => Vec3::Z,
            };
            if axis.length() < 1e-9 {
                return Err(CommandError::bad("axis", "must not be zero"));
            }
            let about = match a.0.get("about") {
                Some(v) if !v.is_null() => vector(v, "about")?,
                _ => Vec3::ZERO,
            };
            let turn = Quat::from_axis_angle(
                axis.normalize(),
                (a.opt_number("turn")?.unwrap_or(0.0) as f32).to_radians(),
            );
            // Turn about `about`, then step: p -> turn (p - about) + about + by.
            let step = BodyPlacement::new(turn, about - turn * about + by);
            let now = ctx.document.body_placement(body);
            ctx.document.set_body_placement(body, step.after(&now));
            Ok(Value::Null)
        }
        _ => Err(CommandError::Unknown(id.to_string())),
    }
}

fn make_joint(id: &str, a: &Args, ctx: &mut WorkbenchRuntimeContext) -> CommandResult {
    let moving_body = body(a, ctx)?;
    let other = BodyId(a.id("other")?);
    if !ctx.document.bodies().iter().any(|b| b.id == other) {
        return Err(CommandError::bad("other", "is not a body of this document"));
    }
    if other == moving_body {
        return Err(CommandError::bad("other", "must be another body"));
    }
    let tool = JointTool::of_command(id).unwrap_or(JointTool::Mate);
    let at = |body: BodyId| -> crate::Rigid { ctx.document.body_placement(body).into() };
    let anchor = |name: &str, body: BodyId| {
        if tool == JointTool::Fixed && a.0.get(name).is_none() {
            // The body's own origin: a fixed joint holds the body, not a face.
            return Ok(Anchor::Plane {
                point: [0.0; 3],
                normal: [0.0, 0.0, 1.0],
            });
        }
        let world = anchor_of(a.0.get(name), name, tool.takes())?;
        // Stored in the body's own frame, as a picked face is.
        Ok::<_, CommandError>(world.moved(&ctx.document.body_placement(body).inverse()))
    };
    let moving = anchor("face", moving_body)?;
    let fixed = anchor("other_face", other)?;
    if tool.takes() == Takes::FlatAndRound
        && matches!(moving, Anchor::Plane { .. }) == matches!(fixed, Anchor::Plane { .. })
    {
        return Err(CommandError::bad(
            "other_face",
            "must be round where `face` is flat, or flat where it is round",
        ));
    }
    let radius = match a.opt_number("radius")? {
        Some(r) => r as f32,
        None => ["face", "other_face"]
            .iter()
            .find_map(|name| a.0.get(*name)?.get("radius")?.as_f64())
            .unwrap_or(0.0) as f32,
    };
    if tool == JointTool::Tangent && radius <= 0.0 {
        return Err(CommandError::bad(
            "radius",
            "must be given where the round face has none",
        ));
    }
    let mut kind = tool.joint(&moving, &at(moving_body), &fixed, &at(other), radius);
    match &mut kind {
        JointKind::Mate { flip, offset } => {
            *flip = a.opt_bool("flip")?.unwrap_or(false);
            *offset = a.opt_number("offset")?.unwrap_or(0.0) as f32;
        }
        JointKind::Angle { degrees } => {
            if let Some(d) = a.opt_number("degrees")? {
                *degrees = d as f32;
            }
        }
        JointKind::Hinge { offset, drive, .. } => {
            if let Some(v) = a.opt_number("offset")? {
                *offset = v as f32;
            }
            drive_args(a, drive)?;
        }
        JointKind::Slider { drive, .. } => drive_args(a, drive)?,
        JointKind::Align { turn, slide, .. } => {
            drive_args_named(a, turn, "turn_drive", "turn_limits")?;
            drive_args_named(a, slide, "slide_drive", "slide_limits")?;
        }
        JointKind::Distance { offset } => {
            if let Some(v) = a.opt_number("offset")? {
                *offset = v as f32;
            }
        }
        _ => {}
    }
    let label = kind.label();
    let name = match a.opt_string("name")? {
        Some(name) => name.to_string(),
        None => {
            let number = joints(ctx.document)
                .iter()
                .filter(|j| j.feature.kind.label() == label)
                .count()
                + 1;
            format!("{label} {number}")
        }
    };
    let joint = JointFeature {
        kind,
        moving,
        other_body: other,
        fixed,
    };
    ground_first(ctx, other);
    let feature = ctx
        .document
        .add_feature_in_body(joint, name, Some(moving_body))
        .map_err(|e| CommandError::failed(e.to_string()))?;
    // A joint has no solid to rebuild.
    ctx.document.clear_feature_dirty(feature);
    solved(ctx, json!(feature.0.to_string()))
}

/// A joint's task accepted, as a recording says it: a new joint as the
/// command that makes it, its faces where the bodies sat before it moved
/// them (where a replay finds them); an edited one as `asm.set` with what
/// changed.
#[cfg(feature = "egui")]
pub(crate) fn record_joint(
    ctx: &mut WorkbenchRuntimeContext,
    id: FeatureId,
    before: Option<&Value>,
    placements: &[(BodyId, BodyPlacement)],
) {
    let Some(node) = ctx.document.get_feature_meta(id).cloned() else {
        return;
    };
    let Ok(joint) = serde_json::from_value::<JointFeature>(node.data.clone()) else {
        return;
    };
    let settings = |kind: &JointKind| match *kind {
        JointKind::Mate { flip, offset } => json!({"offset": offset, "flip": flip}),
        JointKind::Angle { degrees } => json!({"degrees": degrees}),
        JointKind::Hinge { offset, drive, .. } => json!({
            "offset": offset,
            "drive": drive.to.map_or(json!(false), |v| json!(v)),
            "limits": drive.limits.map_or(json!(false), |l| json!(l)),
        }),
        JointKind::Slider { drive, .. } => json!({
            "drive": drive.to.map_or(json!(false), |v| json!(v)),
            "limits": drive.limits.map_or(json!(false), |l| json!(l)),
        }),
        JointKind::Distance { offset } => json!({"offset": offset}),
        JointKind::Tangent { radius } => json!({"radius": radius}),
        JointKind::Align { turn, slide, .. } => json!({
            "turn_drive": turn.to.map_or(json!(false), |v| json!(v)),
            "turn_limits": turn.limits.map_or(json!(false), |l| json!(l)),
            "slide_drive": slide.to.map_or(json!(false), |v| json!(v)),
            "slide_limits": slide.limits.map_or(json!(false), |l| json!(l)),
        }),
        JointKind::Ground
        | JointKind::Fixed { .. }
        | JointKind::Parallel
        | JointKind::Perpendicular => json!({}),
    };
    match before {
        None => {
            let Some(body) = node.body else {
                return;
            };
            let at = |b: BodyId| {
                placements
                    .iter()
                    .find(|(p, _)| *p == b)
                    .map(|(_, placement)| *placement)
                    .unwrap_or_default()
            };
            let face = |anchor: &Anchor, b: BodyId| match anchor.moved(&at(b)) {
                Anchor::Plane { point, normal } => json!({"point": point, "normal": normal}),
                Anchor::Axis { point, direction } => {
                    json!({"axis": {"point": point, "direction": direction}})
                }
            };
            let command = match JointTool::of_kind(&joint.kind) {
                Some(tool) => tool.command(),
                None => {
                    ctx.record(
                        "asm.ground",
                        object(json!({"body": body.0.to_string()})),
                        json!(id.0.to_string()),
                    );
                    return;
                }
            };
            let mut args = object(json!({
                "body": body.0.to_string(),
                "face": face(&joint.moving, body),
                "other": joint.other_body.0.to_string(),
                "other_face": face(&joint.fixed, joint.other_body),
                "name": node.name,
            }));
            args.extend(object(settings(&joint.kind)));
            ctx.record(command, args, json!(id.0.to_string()));
        }
        Some(before) => {
            let Ok(old) = serde_json::from_value::<JointFeature>(before.clone()) else {
                return;
            };
            let (was, now) = (object(settings(&old.kind)), object(settings(&joint.kind)));
            let mut args = object(json!({"joint": id.0.to_string()}));
            for (name, value) in now {
                if was.get(&name) != Some(&value) {
                    args.insert(name, value);
                }
            }
            if args.len() > 1 {
                ctx.record("asm.set", args, Value::Null);
            }
        }
    }
}

/// Every body's placement, in double precision.
fn placements(document: &core_document::Document) -> std::collections::HashMap<BodyId, Rigid> {
    document
        .bodies()
        .iter()
        .map(|b| (b.id, Rigid::from(b.placement)))
        .collect()
}

/// A coupling of two joints where they stand, from a command's
/// arguments; `keep` supplies what the arguments leave out.
fn build_coupling(
    a: &Args,
    ctx: &WorkbenchRuntimeContext,
    keep: Option<&Coupling>,
) -> Result<Coupling, CommandError> {
    let all = joints(ctx.document);
    let joint = |name: &str, kept: Option<FeatureId>| -> Result<crate::Joint, CommandError> {
        let id = match a.opt_id(name)? {
            Some(id) => FeatureId(id),
            None => kept.ok_or_else(|| CommandError::bad(name, "is needed"))?,
        };
        all.iter()
            .find(|j| j.id == id)
            .cloned()
            .filter(|j| {
                matches!(
                    j.feature.kind,
                    JointKind::Hinge { .. } | JointKind::Slider { .. }
                )
            })
            .ok_or_else(|| CommandError::bad(name, "must be a hinge or a slider"))
    };
    let driver = joint("driver", keep.map(|c| c.driver))?;
    let driven = joint("driven", keep.map(|c| c.driven))?;
    if driver.id == driven.id {
        return Err(CommandError::bad("driven", "must be another joint"));
    }
    let gearing = match a.opt_string("gearing")? {
        Some(word) => Gearing::of_word(word)
            .ok_or_else(|| CommandError::bad("gearing", "must be gears, belt, rack or screw"))?,
        None => keep
            .map(|c| c.gearing)
            .filter(|g| g.fits(&driver.feature.kind, &driven.feature.kind))
            .or_else(|| Gearing::suiting(&driver.feature.kind, &driven.feature.kind))
            .ok_or_else(|| CommandError::bad("driven", "cannot be tied to the driver"))?,
    };
    if !gearing.fits(&driver.feature.kind, &driven.feature.kind) {
        return Err(CommandError::bad(
            "gearing",
            "does not suit these joints: gears and a belt tie two hinges, a rack and a \
             screw a hinge and a slider",
        ));
    }
    let ratio = match a.opt_number("ratio")? {
        Some(r) => r as f32,
        // A kept ratio reads the same way only in a kind of the same sort.
        None => keep
            .filter(|c| c.gearing.ratio_label().1 == gearing.ratio_label().1)
            .map_or(gearing.default_ratio(), |c| c.ratio),
    };
    if !(ratio.is_finite() && ratio > 0.0) {
        return Err(CommandError::bad("ratio", "must be above zero"));
    }
    let reverse = match a.opt_bool("reverse")? {
        Some(r) => r,
        None => keep.is_some_and(|c| c.reverse),
    };
    let same_joints = keep.is_some_and(|c| c.driver == driver.id && c.driven == driven.id);
    match keep {
        // The same two joints keep where they were tied, so a new ratio
        // moves nothing at that place.
        Some(kept) if same_joints => Ok(Coupling {
            gearing,
            ratio,
            reverse,
            ..kept.clone()
        }),
        _ => Coupling::new(
            gearing,
            &driver,
            &driven,
            ratio,
            reverse,
            &placements(ctx.document),
        )
        .ok_or_else(|| CommandError::failed("the joints' bodies are not there")),
    }
}

fn couple(a: &Args, ctx: &mut WorkbenchRuntimeContext) -> CommandResult {
    let coupling = build_coupling(a, ctx, None)?;
    let body = joints(ctx.document)
        .into_iter()
        .find(|j| j.id == coupling.driven)
        .map(|j| j.body)
        .ok_or_else(|| CommandError::bad("driven", "is not a joint"))?;
    let name = match a.opt_string("name")? {
        Some(name) => name.to_string(),
        None => next_name(ctx.document, coupling.gearing.label()),
    };
    let id = ctx
        .document
        .add_feature_in_body(coupling, name, Some(body))
        .map_err(|e| CommandError::failed(e.to_string()))?;
    ctx.document.clear_feature_dirty(id);
    solved(ctx, json!(id.0.to_string()))
}

fn set_coupling(a: &Args, ctx: &mut WorkbenchRuntimeContext) -> CommandResult {
    let id = FeatureId(a.id("joint")?);
    let kept = ctx
        .document
        .get_feature_data(id)
        .and_then(|d| serde_json::from_value::<Coupling>(d.clone()).ok())
        .ok_or_else(|| CommandError::bad("joint", "is not a coupling"))?;
    let coupling = build_coupling(a, ctx, Some(&kept))?;
    let data = serde_json::to_value(&coupling).map_err(|e| CommandError::failed(e.to_string()))?;
    ctx.document
        .update_feature_data(id, data)
        .map_err(|e| CommandError::failed(e.to_string()))?;
    ctx.document.clear_feature_dirty(id);
    solved(ctx, Value::Null)
}

/// `label` numbered past every coupling already named so.
pub(crate) fn next_name(document: &core_document::Document, label: &str) -> String {
    let taken = document
        .feature_tree()
        .all_nodes()
        .filter(|(_, n)| n.workbench_id.as_str() == COUPLING_KIND && n.name.starts_with(label))
        .count();
    format!("{label} {}", taken + 1)
}

/// A coupling's settings, as `asm.couple` and `asm.set` take them.
fn coupling_settings(c: &Coupling) -> CommandArgs {
    object(json!({
        "driver": c.driver.0.to_string(),
        "driven": c.driven.0.to_string(),
        "gearing": c.gearing.word(),
        "ratio": c.ratio,
        "reverse": c.reverse,
    }))
}

/// A coupling's task accepted, as a recording says it: a new one as the
/// command that makes it, an edit as the settings it changed.
pub(crate) fn record_coupling(
    ctx: &mut WorkbenchRuntimeContext,
    id: FeatureId,
    before: Option<&Value>,
) {
    let Some(node) = ctx.document.get_feature_meta(id).cloned() else {
        return;
    };
    let Ok(coupling) = serde_json::from_value::<Coupling>(node.data.clone()) else {
        return;
    };
    let now = coupling_settings(&coupling);
    match before.and_then(|b| serde_json::from_value::<Coupling>(b.clone()).ok()) {
        None => {
            let mut args = now;
            args.insert("name".into(), json!(node.name));
            ctx.record("asm.couple", args, json!(id.0.to_string()));
        }
        Some(old) => {
            let was = coupling_settings(&old);
            let mut args = object(json!({"joint": id.0.to_string()}));
            for (name, value) in now {
                if was.get(&name) != Some(&value) {
                    args.insert(name, value);
                }
            }
            if args.len() > 1 {
                ctx.record("asm.set", args, Value::Null);
            }
        }
    }
}

/// A JSON object as named arguments.
/// The `bodies` a command is limited to, when it names any.
fn body_list(a: &Args) -> Result<Option<Vec<BodyId>>, CommandError> {
    let bad = || CommandError::bad("bodies", "must be a list of ids");
    match a.0.get("bodies") {
        None | Some(Value::Null) => Ok(None),
        Some(list) => list
            .as_array()
            .ok_or_else(bad)?
            .iter()
            .map(|v| {
                v.as_str()
                    .and_then(|t| uuid::Uuid::parse_str(t).ok())
                    .map(BodyId)
                    .ok_or_else(bad)
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
    }
}

pub(crate) fn object(value: Value) -> CommandArgs {
    match value {
        Value::Object(map) => map,
        _ => CommandArgs::new(),
    }
}

/// Solve, and answer `value` when every joint holds.
fn solved(ctx: &mut WorkbenchRuntimeContext, value: Value) -> CommandResult {
    crate::apply_solve(ctx).map_err(CommandError::failed)?;
    Ok(value)
}

fn body(a: &Args, ctx: &WorkbenchRuntimeContext) -> Result<BodyId, CommandError> {
    let body = BodyId(a.id("body")?);
    if ctx.document.bodies().iter().any(|b| b.id == body) {
        Ok(body)
    } else {
        Err(CommandError::bad("body", "is not a body of this document"))
    }
}

/// A face as a joint anchors to it: a flat face's `point` and `normal`, or
/// a round face's `axis`.
fn anchor_of(value: Option<&Value>, name: &str, takes: Takes) -> Result<Anchor, CommandError> {
    let face = value
        .and_then(Value::as_object)
        .ok_or_else(|| CommandError::bad(name, "must be a face, as pc.doc.faces lists it"))?;
    let flat = || -> Result<Anchor, CommandError> {
        let point = vector(face.get("point").unwrap_or(&Value::Null), name)?;
        let normal = face
            .get("normal")
            .ok_or_else(|| CommandError::bad(name, "must be a flat face, with a normal"))?;
        let normal = vector(normal, name)?;
        Ok(Anchor::Plane {
            point: point.to_array(),
            normal: normal.normalize_or_zero().to_array(),
        })
    };
    let round = || -> Result<Anchor, CommandError> {
        let axis = face
            .get("axis")
            .and_then(Value::as_object)
            .ok_or_else(|| CommandError::bad(name, "must be a round face, with an axis"))?;
        let point = vector(axis.get("point").unwrap_or(&Value::Null), name)?;
        let direction = vector(axis.get("direction").unwrap_or(&Value::Null), name)?;
        Ok(Anchor::Axis {
            point: point.to_array(),
            direction: direction.normalize_or_zero().to_array(),
        })
    };
    match takes {
        Takes::Flat => flat(),
        Takes::Round => round(),
        Takes::Any | Takes::FlatAndRound if face.contains_key("normal") => flat(),
        Takes::Any | Takes::FlatAndRound => round(),
    }
}

fn vector(value: &Value, name: &str) -> Result<Vec3, CommandError> {
    let bad = || CommandError::bad(name, "must be {x, y, z}");
    let v = match value {
        Value::Array(v) if v.len() == 3 => [v[0].as_f64(), v[1].as_f64(), v[2].as_f64()],
        Value::Object(m) => ["x", "y", "z"].map(|k| m.get(k).and_then(Value::as_f64)),
        _ => return Err(bad()),
    };
    Ok(Vec3::new(
        v[0].ok_or_else(bad)? as f32,
        v[1].ok_or_else(bad)? as f32,
        v[2].ok_or_else(bad)? as f32,
    ))
}

fn quaternion(value: &Value) -> Result<Quat, CommandError> {
    let bad = || CommandError::bad("rotation", "must be a quaternion {x, y, z, w}");
    let q = match value {
        Value::Array(v) if v.len() == 4 => {
            [v[0].as_f64(), v[1].as_f64(), v[2].as_f64(), v[3].as_f64()]
        }
        Value::Object(m) => ["x", "y", "z", "w"].map(|k| m.get(k).and_then(Value::as_f64)),
        _ => return Err(bad()),
    };
    let q = Quat::from_xyzw(
        q[0].ok_or_else(bad)? as f32,
        q[1].ok_or_else(bad)? as f32,
        q[2].ok_or_else(bad)? as f32,
        q[3].ok_or_else(bad)? as f32,
    );
    if q.length() < 1e-6 {
        return Err(bad());
    }
    Ok(q.normalize())
}

/// The density the mass tool starts at, g/cm³.
pub(crate) const DEFAULT_DENSITY: f32 = 1.0;

/// The first joint of an assembly grounds the body it holds against, when
/// nothing is grounded yet: the assembly then stands on it, and what its
/// joints leave free reads true from the start.
pub(crate) fn ground_first(ctx: &mut WorkbenchRuntimeContext, other: BodyId) {
    let all = crate::joints(ctx.document);
    if all.is_empty() {
        set_grounded(ctx, other, true);
    }
}

/// Ground `body`, or let it move again: a ground joint on it, or none.
/// Answers the ground joint made.
pub(crate) fn set_grounded(
    ctx: &mut WorkbenchRuntimeContext,
    body: BodyId,
    grounded: bool,
) -> Option<FeatureId> {
    let existing: Vec<FeatureId> = crate::joints(ctx.document)
        .into_iter()
        .filter(|j| j.body == body && j.feature.kind == JointKind::Ground)
        .map(|j| j.id)
        .collect();
    if !grounded {
        for id in existing {
            let _ = ctx.document.remove_feature(id);
        }
        return None;
    }
    if let Some(id) = existing.first() {
        return Some(*id);
    }
    let anchor = Anchor::Plane {
        point: [0.0; 3],
        normal: [0.0, 0.0, 1.0],
    };
    ctx.document
        .add_feature_in_body(
            JointFeature {
                kind: JointKind::Ground,
                moving: anchor,
                other_body: body,
                fixed: anchor,
            },
            "Ground".into(),
            Some(body),
        )
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_document::Document;

    fn call(doc: &mut Document, id: &str, args: Value) -> CommandResult {
        let mut ctx = WorkbenchRuntimeContext::new(doc, [0.0; 3], [0.0; 3], (0, 0, 1, 1));
        run(id, args.as_object().unwrap(), &mut ctx)
    }

    #[test]
    fn a_mate_puts_one_body_s_face_on_the_other_s() {
        let mut doc = Document::new("t");
        let (a, b) = (doc.create_body(None), doc.create_body(None));
        call(
            &mut doc,
            "asm.move",
            json!({"body": a.0.to_string(), "by": [0, 0, 50]}),
        )
        .unwrap();
        // a's bottom (at z = 50 where it sits) onto b's top at z = 10.
        let joint = call(
            &mut doc,
            "asm.mate",
            json!({
                "body": a.0.to_string(),
                "face": {"point": [0, 0, 50], "normal": [0, 0, -1]},
                "other": b.0.to_string(),
                "other_face": {"point": [0, 0, 10], "normal": [0, 0, 1]},
                "offset": 2,
            }),
        )
        .unwrap();
        let z = doc.body_placement(a).translation[2];
        assert!((z - 12.0).abs() < 1e-3, "{z}");
        call(&mut doc, "asm.set", json!({"joint": joint, "offset": 5})).unwrap();
        assert!((doc.body_placement(a).translation[2] - 15.0).abs() < 1e-3);
        let err = call(
            &mut doc,
            "asm.align",
            json!({
                "body": a.0.to_string(),
                "face": {"point": [0, 0, 0], "normal": [0, 0, 1]},
                "other": b.0.to_string(),
                "other_face": {"point": [0, 0, 0], "normal": [0, 0, 1]},
            }),
        );
        assert!(err.is_err(), "an alignment takes round faces");
    }

    #[test]
    fn a_tangent_takes_its_radius_from_the_round_face_and_either_order() {
        let mut doc = Document::new("t");
        let (a, b) = (doc.create_body(None), doc.create_body(None));
        let roller = json!({"axis": {"point": [0, 0, 0], "direction": [1, 0, 0]}, "radius": 4});
        let top = json!({"point": [0, 0, 10], "normal": [0, 0, 1]});
        let joint = call(
            &mut doc,
            "asm.tangent",
            json!({"body": a.0.to_string(), "face": roller, "other": b.0.to_string(), "other_face": top}),
        )
        .unwrap();
        assert!((doc.body_placement(a).translation[2] - 14.0).abs() < 1e-3);
        call(&mut doc, "asm.set", json!({"joint": joint, "radius": 6})).unwrap();
        assert!((doc.body_placement(a).translation[2] - 16.0).abs() < 1e-3);
        let both_flat = call(
            &mut doc,
            "asm.tangent",
            json!({"body": a.0.to_string(), "face": top, "other": b.0.to_string(), "other_face": top}),
        );
        assert!(both_flat.is_err(), "one face of each");
    }

    /// Turning a joint's body keeps it there: a driven hinge keeps its
    /// angle with the body turned further, a slider turned over runs the
    /// other way round, a mate turned over faces the same way.
    #[test]
    fn a_joint_s_body_turns_and_turns_over_and_stays() {
        let mut doc = Document::new("t");
        let (a, b) = (doc.create_body(None), doc.create_body(None));
        let pin = json!({"axis": {"point": [0, 0, 0], "direction": [0, 0, 1]}});
        let hinge = call(
            &mut doc,
            "asm.hinge",
            json!({"body": a.0.to_string(), "face": pin, "other": b.0.to_string(), "other_face": pin,
                   "drive": 30}),
        )
        .unwrap();
        call(&mut doc, "asm.turn", json!({"joint": hinge, "degrees": 20})).unwrap();
        let x = doc.body_placement(a).direction([1.0, 0.0, 0.0]);
        assert!((x[1].atan2(x[0]).to_degrees() - 50.0).abs() < 1e-2, "{x:?}");
        let travel = call(&mut doc, "asm.travel", json!({"joint": hinge})).unwrap();
        assert!((travel.as_f64().unwrap() - 30.0).abs() < 1e-2, "{travel}");
        call(&mut doc, "asm.solve", json!({})).unwrap();
        let x = doc.body_placement(a).direction([1.0, 0.0, 0.0]);
        assert!(
            (x[1].atan2(x[0]).to_degrees() - 50.0).abs() < 1e-2,
            "stays: {x:?}"
        );

        let mut doc = Document::new("t");
        let (a, b) = (doc.create_body(None), doc.create_body(None));
        let top = json!({"point": [0, 0, 0], "normal": [0, 0, 1]});
        let mate = call(
            &mut doc,
            "asm.mate",
            json!({"body": a.0.to_string(), "face": top, "other": b.0.to_string(), "other_face": top}),
        )
        .unwrap();
        let up = doc.body_placement(a).direction([0.0, 0.0, 1.0]);
        assert!(up[2] < -0.99, "faces the other: {up:?}");
        call(&mut doc, "asm.flip", json!({"joint": mate})).unwrap();
        call(&mut doc, "asm.solve", json!({})).unwrap();
        let up = doc.body_placement(a).direction([0.0, 0.0, 1.0]);
        assert!(up[2] > 0.99, "the same way now: {up:?}");
        let id = FeatureId(uuid::Uuid::parse_str(mate.as_str().unwrap()).unwrap());
        let data: JointFeature =
            serde_json::from_value(doc.get_feature_data(id).unwrap().clone()).unwrap();
        assert!(matches!(data.kind, JointKind::Mate { flip: true, .. }));
    }

    /// An alignment's turn and slide are each driven, and an alignment
    /// stored as a plain word still loads, both free.
    #[test]
    fn an_alignment_drives_its_turn_and_its_slide() {
        let mut doc = Document::new("t");
        let (a, b) = (doc.create_body(None), doc.create_body(None));
        let pin = json!({"axis": {"point": [0, 0, 0], "direction": [0, 0, 1]}});
        let joint = call(
            &mut doc,
            "asm.align",
            json!({"body": a.0.to_string(), "face": pin, "other": b.0.to_string(), "other_face": pin,
                   "slide_drive": 5}),
        )
        .unwrap();
        assert!((doc.body_placement(a).translation[2] - 5.0).abs() < 1e-3);
        call(
            &mut doc,
            "asm.set",
            json!({"joint": joint, "turn_drive": 30}),
        )
        .unwrap();
        let x = doc.body_placement(a).direction([1.0, 0.0, 0.0]);
        assert!((x[1].atan2(x[0]).to_degrees() - 30.0).abs() < 1e-2, "{x:?}");
        assert!((doc.body_placement(a).translation[2] - 5.0).abs() < 1e-3);
        let free = crate::freedom(&doc);
        let motions = &free.iter().find(|(body, _)| *body == a).unwrap().1;
        assert!(motions.is_empty(), "both held: {motions:?}");

        let old = json!({
            "kind": "Align",
            "moving": {"Axis": {"point": [0.0, 0.0, 0.0], "direction": [0.0, 0.0, 1.0]}},
            "other_body": b.0.to_string(),
            "fixed": {"Axis": {"point": [0.0, 0.0, 0.0], "direction": [0.0, 0.0, 1.0]}},
        });
        let read: JointFeature = serde_json::from_value(old).expect("an old alignment loads");
        assert_eq!(read.kind, JointKind::align());
    }

    #[test]
    fn a_hinge_is_driven_to_an_angle_and_kept_within_its_limits() {
        let mut doc = Document::new("t");
        let (a, b) = (doc.create_body(None), doc.create_body(None));
        let pin = json!({"axis": {"point": [0, 0, 0], "direction": [0, 0, 1]}});
        let joint = call(
            &mut doc,
            "asm.hinge",
            json!({"body": a.0.to_string(), "face": pin, "other": b.0.to_string(), "other_face": pin}),
        )
        .unwrap();
        let travel = |doc: &mut Document| {
            call(doc, "asm.travel", json!({"joint": joint}))
                .unwrap()
                .as_f64()
                .unwrap()
        };
        assert!(travel(&mut doc).abs() < 1e-6, "made where it sits");
        call(&mut doc, "asm.set", json!({"joint": joint, "drive": 30})).unwrap();
        assert!((travel(&mut doc) - 30.0).abs() < 1e-3);
        let x = doc.body_placement(a).direction([1.0, 0.0, 0.0]);
        assert!((x[1].atan2(x[0]).to_degrees() - 30.0).abs() < 1e-2, "{x:?}");
        // Let go and limited below where it is: it turns back to the limit.
        call(
            &mut doc,
            "asm.set",
            json!({"joint": joint, "drive": false, "limits": [-10, 10]}),
        )
        .unwrap();
        assert!((travel(&mut doc) - 10.0).abs() < 1e-2);
        // At its limit it may still turn, back the other way.
        let motions = &crate::freedom(&doc)[0].1;
        assert_eq!(motions.len(), 1);
        assert!(
            matches!(motions[0], crate::Motion::Turn { at_limit: true, .. }),
            "{motions:?}"
        );
        assert!(motions[0].describe().ends_with("(one way, at its limit)"));
        call(
            &mut doc,
            "asm.set",
            json!({"joint": joint, "limits": [-45, 45]}),
        )
        .unwrap();
        assert!(
            matches!(
                crate::freedom(&doc)[0].1[0],
                crate::Motion::Turn {
                    at_limit: false,
                    ..
                }
            ),
            "well within its limits"
        );
        let bad = call(
            &mut doc,
            "asm.set",
            json!({"joint": joint, "limits": [5, -5]}),
        );
        assert!(bad.is_err());
    }

    #[test]
    fn a_slider_is_driven_along_its_axis() {
        let mut doc = Document::new("t");
        let (a, b) = (doc.create_body(None), doc.create_body(None));
        let rail = json!({"axis": {"point": [0, 0, 0], "direction": [1, 0, 0]}});
        let joint = call(
            &mut doc,
            "asm.slider",
            json!({"body": a.0.to_string(), "face": rail, "other": b.0.to_string(),
                   "other_face": rail, "drive": 12.5}),
        )
        .unwrap();
        assert!((doc.body_placement(a).translation[0] - 12.5).abs() < 1e-3);
        assert!(crate::freedom(&doc)[0].1.is_empty(), "driven: nothing left");
        let travel = call(&mut doc, "asm.travel", json!({"joint": joint})).unwrap();
        assert!((travel.as_f64().unwrap() - 12.5).abs() < 1e-3);
    }

    #[test]
    fn a_fixed_joint_needs_no_faces_and_carries_the_body_along() {
        let mut doc = Document::new("t");
        let (a, b) = (doc.create_body(None), doc.create_body(None));
        call(
            &mut doc,
            "asm.move",
            json!({"body": a.0.to_string(), "by": [5, 6, 7]}),
        )
        .unwrap();
        call(
            &mut doc,
            "asm.fix",
            json!({"body": a.0.to_string(), "other": b.0.to_string()}),
        )
        .unwrap();
        call(
            &mut doc,
            "asm.move",
            json!({"body": b.0.to_string(), "by": [100, 0, 0]}),
        )
        .unwrap();
        call(&mut doc, "asm.solve", json!({})).unwrap();
        let at = doc.body_placement(a).translation;
        assert!(
            (at[0] - 105.0).abs() < 1e-3 && (at[1] - 6.0).abs() < 1e-3,
            "{at:?}"
        );
    }

    #[test]
    fn a_move_turns_about_a_point_and_steps() {
        let mut doc = Document::new("t");
        let a = doc.create_body(None);
        call(
            &mut doc,
            "asm.move",
            json!({"body": a.0.to_string(), "turn": 90, "about": [10, 0, 0], "by": [0, 0, 5]}),
        )
        .unwrap();
        let placement = doc.body_placement(a);
        // The origin turned a quarter about (10, 0, 0) lands on (10, -10, 0).
        let p = placement.point([0.0, 0.0, 0.0]);
        assert!(
            (p[0] - 10.0).abs() < 1e-4 && (p[1] + 10.0).abs() < 1e-4 && (p[2] - 5.0).abs() < 1e-4,
            "{p:?}"
        );
        let listed = call(&mut doc, "asm.placement", json!({"body": a.0.to_string()})).unwrap();
        assert_eq!(listed["translation"][2], json!(5.0));
    }
}
