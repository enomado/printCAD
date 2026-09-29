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

/// How far apart copies of `body` sit by default: its width along X and
/// a tenth more, in the world.
pub(crate) fn copy_step(document: &core_document::Document, body: BodyId) -> Vec3 {
    let width = document
        .imported_geometry(body)
        .and_then(|g| g.bounds_mm.or_else(|| g.mesh.bounds()))
        .map_or(20.0, |(lo, hi)| hi[0] - lo[0]);
    Vec3::new((width * 1.1).max(1.0), 0.0, 0.0)
}

/// `count` linked copies of `source`, each `step` (world) on from the
/// one before, the first `step` from the source; their ids.
pub(crate) fn insert_copies(
    ctx: &mut WorkbenchRuntimeContext,
    source: BodyId,
    count: usize,
    step: Option<Vec3>,
) -> Option<Vec<BodyId>> {
    let step = step.unwrap_or_else(|| copy_step(ctx.document, source));
    let from = ctx.document.body_placement(source);
    let mut made = Vec::with_capacity(count);
    for i in 1..=count {
        let copy = ctx.document.create_linked_copy(source, None)?;
        let placed = BodyPlacement::new(from.quat(), from.offset() + step * i as f32);
        ctx.document.set_body_placement(copy, placed);
        made.push(copy);
    }
    Some(made)
}

/// `count` linked copies of `source` turned about the axis through
/// `point` along `direction`, spread evenly over `angle` degrees: a whole
/// turn shares it with the source, a part turn ends on its far end.
pub(crate) fn insert_copies_around(
    ctx: &mut WorkbenchRuntimeContext,
    source: BodyId,
    count: usize,
    (point, direction, angle): (Vec3, Vec3, f32),
) -> Option<Vec<BodyId>> {
    let full = (angle.abs() - 360.0).abs() < 1e-3;
    let each = if full {
        angle / (count + 1) as f32
    } else {
        angle / count.max(1) as f32
    };
    let from = ctx.document.body_placement(source);
    let mut made = Vec::with_capacity(count);
    for i in 1..=count {
        let turn = Quat::from_axis_angle(direction, (each * i as f32).to_radians());
        let step = BodyPlacement::new(turn, point - turn * point);
        let copy = ctx.document.create_linked_copy(source, None)?;
        ctx.document.set_body_placement(copy, step.after(&from));
        made.push(copy);
    }
    Some(made)
}

/// The joint `tool` makes between these two anchors (each in its own
/// body's frame) where the bodies stand.
pub(crate) fn rejoined(
    ctx: &WorkbenchRuntimeContext,
    tool: JointTool,
    (moving_body, moving): (BodyId, Anchor),
    (other, fixed): (BodyId, Anchor),
    radius: f32,
) -> Result<JointFeature, CommandError> {
    if !tool.fits(&moving, &fixed) {
        return Err(CommandError::bad("kind", tool.refusal()));
    }
    if tool == JointTool::Tangent && radius <= 0.0 {
        return Err(CommandError::bad(
            "radius",
            "must be given where the round face has none",
        ));
    }
    let at = |b: BodyId| -> Rigid { ctx.document.body_placement(b).into() };
    Ok(JointFeature {
        second: None,
        shape: Vec::new(),
        ends: [0.0; 2],
        names: [0; 2],
        kind: tool.joint(&moving, &at(moving_body), &fixed, &at(other), radius),
        moving,
        other_body: other,
        fixed,
    })
}

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
            .param(
                "other",
                ParamKind::Id,
                "The body it is held against; the nil id (all zeros) for the world origin, \
                 its faces then in world space",
            )
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
            Takes::Point => {
                "A point: a ball's {centre}, or {point} alone, as pc.doc.faces lists them"
            }
            Takes::Directed => {
                "A flat face {point, normal} or a round face or edge {axis = {point, direction}}"
            }
            Takes::Anything => {
                "A flat face {point, normal}, a round face or edge {axis}, or a point \
                 ({centre} of a ball, or {point} alone)"
            }
            Takes::PointAndLine => {
                "The pin: a point ({centre} or {point}) on the moving body; the slot: a line \
                 {axis = {point, direction}} on the other"
            }
            Takes::PointAndEdge => {
                "The point that runs ({centre} or {point}); on the other body, a {point} on \
                 the edge it runs along"
            }
            Takes::PointAndFace => {
                "The follower ({centre} or {point}); on the other body, a {point} on the cam's face"
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
            JointTool::Cam => spec.optional(
                "radius",
                ParamKind::Number,
                "The follower's roller radius, mm; 0 for a point follower",
            ),
            JointTool::Width => spec
                .param("face2", ParamKind::Any, "The tab's other flat face")
                .param("other_face2", ParamKind::Any, "The slot's other wall"),
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
            "asm.copy",
            "Insert linked copies of a body: each takes its shape and follows it, placed on its own",
        )
        .param("body", ParamKind::Id, "The body to copy")
        .optional("count", ParamKind::Number, "How many (1 when left out)")
        .optional(
            "step",
            ParamKind::List,
            "{x, y, z}: how far each copy sits from the one before, mm; beside it along X \
             when left out",
        )
        .optional(
            "around",
            ParamKind::Any,
            "{point = {x, y, z}, direction = {x, y, z}, angle}: the copies turned about this \
             axis instead, spread evenly over `angle` degrees (360 when left out)",
        )
        .returns("the copies' ids"),
    );
    context.register_command(
        CommandSpec::new(
            "asm.mirror",
            "Insert a linked copy that is a body's mirror image, following every change to it",
        )
        .param("body", ParamKind::Id, "The body to mirror")
        .param(
            "point",
            ParamKind::List,
            "A point of the mirror plane, {x, y, z}, in the world",
        )
        .param("normal", ParamKind::List, "The plane's normal, {x, y, z}")
        .returns("the mirrored copy's id"),
    );
    context.register_command(
        CommandSpec::new(
            "asm.replace",
            "Put another body in a body's place, with its joints found again on the new body's faces",
        )
        .param("body", ParamKind::Id, "The body to replace; it is hidden")
        .param("with", ParamKind::Id, "The body that takes its place")
        .returns(
            "{kept, unmatched}: the joints whose ends were found on the new body, and those \
             that were not",
        ),
    );
    context.register_command(
        CommandSpec::new(
            "asm.group",
            "Lock bodies together where they sit, in one rigid group",
        )
        .param(
            "bodies",
            ParamKind::List,
            "Two bodies or more; the first the one the rest hold to",
        )
        .optional(
            "group",
            ParamKind::Id,
            "A group to change to these bodies, rather than a new one",
        )
        .optional("name", ParamKind::String, "A new group's name in the tree")
        .returns("the group's id"),
    );
    context.register_command(
        CommandSpec::new(
            "asm.motion",
            "Keep a motion over time: hinges and sliders each driven by a formula of t, seconds",
        )
        .param(
            "drives",
            ParamKind::List,
            "{{joint = id, formula = \"90 * t\"}, ...}: a hinge's angle in degrees, a slider's \
             position in mm",
        )
        .optional(
            "start",
            ParamKind::Number,
            "When it starts, s (0 when left out)",
        )
        .optional(
            "end",
            ParamKind::Number,
            "When it ends, s (2 when left out)",
        )
        .optional(
            "step",
            ParamKind::Number,
            "The time between frames, s (0.05 when left out)",
        )
        .optional(
            "study",
            ParamKind::Id,
            "A motion to change, rather than a new one",
        )
        .optional("name", ParamKind::String, "A new motion's name in the tree")
        .returns("the motion's id"),
    );
    context.register_command(
        CommandSpec::new(
            "asm.motion_frames",
            "Every body's placement at each frame of a motion; nothing is moved",
        )
        .param("study", ParamKind::Id, "The motion")
        .returns("a list of {t, bodies = {{body, translation, rotation}, ...}}")
        .read_only(),
    );
    context.register_command(
        CommandSpec::new(
            "asm.exploded_view",
            "Keep an exploded view: steps, each moving some bodies by a shift, played in order",
        )
        .param(
            "steps",
            ParamKind::List,
            "{{bodies = {ids}, shift = {x, y, z}}, ...}, in the order they play",
        )
        .optional(
            "view",
            ParamKind::Id,
            "A view to change, rather than a new one",
        )
        .optional("name", ParamKind::String, "A new view's name in the tree")
        .returns("the view's id"),
    );
    context.register_command(
        CommandSpec::new(
            "asm.explode_at",
            "Where an exploded view puts every body, part way through its steps",
        )
        .param("view", ParamKind::Id, "The exploded view")
        .param(
            "at",
            ParamKind::Number,
            "How many steps in: 1.5 is half way through the second",
        )
        .returns("a list of {body, translation, rotation}; nothing is moved")
        .read_only(),
    );
    context.register_command(
        CommandSpec::new(
            "asm.save_state",
            "Save where every body sits, which are hidden and where drives hold, under a name",
        )
        .optional("name", ParamKind::String, "A new state's name in the tree")
        .optional(
            "state",
            ParamKind::Id,
            "A saved state to keep the assembly in instead",
        )
        .returns("the state's id"),
    );
    context.register_command(
        CommandSpec::new(
            "asm.restore_state",
            "Put the assembly back as a saved state has it",
        )
        .param("state", ParamKind::Id, "The saved state"),
    );
    context.register_command(
        CommandSpec::new(
            "asm.redundant",
            "The joints that hold nothing a body's other joints do not",
        )
        .returns("a list of {joint, name}")
        .read_only(),
    );
    context.register_command(
        CommandSpec::new(
            "asm.motion_clashes",
            "Step a hinge's or a slider's drive through a range and find where bodies collide",
        )
        .param("joint", ParamKind::Id, "The hinge or slider")
        .param(
            "low",
            ParamKind::Number,
            "Where the steps start: degrees or mm",
        )
        .param("high", ParamKind::Number, "Where they end")
        .optional(
            "steps",
            ParamKind::Number,
            "How many steps (24 when left out)",
        )
        .returns(
            "a list of {at, a, b, volume (mm³)}: each step and pair sharing more material \
             than where the joint stands",
        )
        .read_only(),
    );
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
            "a list of {name, quantity, bodies, size = {x, y, z} in mm or nil, mesh, number \
             or nil, bought, values = {column = text}}, numbered parts first by number, \
             then by name",
        )
        .read_only(),
    );
    context.register_command(
        CommandSpec::new(
            "asm.part",
            "Set what the parts list keeps for a part: its number, whether it is bought, \
             its values in the added columns",
        )
        .param("body", ParamKind::Id, "Any body of the part")
        .optional("number", ParamKind::Number, "Its item number")
        .optional(
            "bought",
            ParamKind::Bool,
            "Bought rather than made: left out of exports and the slicer",
        )
        .optional(
            "values",
            ParamKind::Any,
            "{column = text}: its values, a column not yet in the list added to it",
        ),
    );
    context.register_command(
        CommandSpec::new(
            "asm.parts_table",
            "Replace what the parts list keeps, whole",
        )
        .param(
            "table",
            ParamKind::Any,
            "{columns = {...}, entries = {[body id] = {number, bought, values}}}",
        ),
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
            // Other faces, another body or another kind first: the settings
            // below apply to the joint they make.
            let tool = match a.opt_string("kind")? {
                Some(word) => Some(JointTool::of_word(word).ok_or_else(|| {
                    CommandError::bad("kind", "must be a joint's word: mate, align, hinge, ...")
                })?),
                None => None,
            };
            if tool.is_some()
                || ["face", "other", "other_face"]
                    .iter()
                    .any(|k| a.0.contains_key(*k))
            {
                let moving_body = node.body.ok_or_else(not_a_joint)?;
                let tool = tool
                    .or_else(|| JointTool::of_kind(&feature.kind))
                    .ok_or_else(|| CommandError::bad("joint", "a ground takes no faces"))?;
                let other = match a.opt_id("other")? {
                    Some(id) => BodyId(id),
                    None => feature.other_body,
                };
                if !(other == crate::WORLD || ctx.document.bodies().iter().any(|b| b.id == other))
                    || other == moving_body
                {
                    return Err(CommandError::bad(
                        "other",
                        "must be another body of the document",
                    ));
                }
                let local = |name: &str, body: BodyId, kept: Anchor| match a.0.get(name) {
                    Some(v) => Ok::<_, CommandError>(
                        anchor_of(Some(v), name, tool.takes())?
                            .moved(&ctx.document.body_placement(body).inverse()),
                    ),
                    None => Ok(kept),
                };
                let moving = local("face", moving_body, feature.moving)?;
                let fixed = local("other_face", other, feature.fixed)?;
                let radius = a
                    .opt_number("radius")?
                    .map(|r| r as f32)
                    .or_else(|| {
                        ["face", "other_face"]
                            .iter()
                            .find_map(|n| a.0.get(*n)?.get("radius")?.as_f64())
                            .map(|r| r as f32)
                    })
                    .or(match feature.kind {
                        JointKind::Tangent { radius } => Some(radius),
                        _ => None,
                    })
                    .unwrap_or(0.0);
                let kept = feature.names;
                let name_of = |key: &str, keep: u64| match a.0.get(key) {
                    Some(face) => face.get("name").and_then(Value::as_u64).unwrap_or(0),
                    None => keep,
                };
                feature = rejoined(ctx, tool, (moving_body, moving), (other, fixed), radius)?;
                feature.names = [name_of("face", kept[0]), name_of("other_face", kept[1])];
            }
            for (end, key) in ["moving_end", "fixed_end"].into_iter().enumerate() {
                if let Some(v) = a.opt_number(key)? {
                    feature.ends[end] = v as f32;
                }
            }
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
                | JointKind::Perpendicular
                | JointKind::Ball
                | JointKind::Universal
                | JointKind::Slot
                | JointKind::Path
                | JointKind::Width => {}
                JointKind::Cam { radius } => {
                    if let Some(v) = a.opt_number("radius")? {
                        *radius = v as f32;
                    }
                }
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
                        "number": part.number,
                        "bought": part.bought,
                        "values": part.values,
                    })
                })
                .collect(),
        )),
        "asm.parts_table" => {
            let table: crate::parts::PartsTable =
                serde_json::from_value(a.0.get("table").cloned().unwrap_or(Value::Null))
                    .map_err(|e| CommandError::bad("table", e.to_string()))?;
            crate::parts::store_table(ctx.document, &table)
                .map_err(|e| CommandError::failed(e.to_string()))?;
            Ok(Value::Null)
        }
        "asm.part" => {
            let body = body(&a, ctx)?;
            let bodies = crate::parts_list(ctx.document)
                .into_iter()
                .find(|p| p.bodies.contains(&body))
                .map(|p| p.bodies)
                .unwrap_or_else(|| vec![body]);
            let mut table = crate::parts::table_of(ctx.document)
                .map(|(_, t)| t)
                .unwrap_or_default();
            let entry = table.entry_mut(&bodies);
            if let Some(n) = a.opt_number("number")? {
                entry.number = n.max(0.0) as u32;
            }
            if let Some(bought) = a.opt_bool("bought")? {
                entry.bought = bought;
            }
            if let Some(values) = a.0.get("values").and_then(Value::as_object) {
                for (column, value) in values {
                    let text = value
                        .as_str()
                        .map_or_else(|| value.to_string(), str::to_string);
                    entry.values.insert(column.clone(), text);
                }
                for column in values.keys() {
                    if !table.columns.contains(column) {
                        table.columns.push(column.clone());
                    }
                }
            }
            crate::parts::store_table(ctx.document, &table)
                .map_err(|e| CommandError::failed(e.to_string()))?;
            Ok(Value::Null)
        }
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
        "asm.copy" => {
            let source = body(&a, ctx)?;
            let count = a.opt_number("count")?.unwrap_or(1.0).clamp(1.0, 500.0) as usize;
            let step = match a.0.get("step") {
                Some(v) if !v.is_null() => Some(vector(v, "step")?),
                _ => None,
            };
            let made = match a.0.get("around").filter(|v| !v.is_null()) {
                Some(around) => {
                    let point = vector(around.get("point").unwrap_or(&json!([0, 0, 0])), "around")?;
                    let direction =
                        vector(around.get("direction").unwrap_or(&Value::Null), "around")?;
                    if direction.length() < 1e-9 {
                        return Err(CommandError::bad("around", "needs a direction"));
                    }
                    let angle = around.get("angle").and_then(Value::as_f64).unwrap_or(360.0) as f32;
                    insert_copies_around(ctx, source, count, (point, direction.normalize(), angle))
                }
                None => insert_copies(ctx, source, count, step),
            }
            .ok_or_else(|| CommandError::bad("body", "is not a body of this document"))?;
            Ok(Value::from(
                made.iter().map(|b| b.0.to_string()).collect::<Vec<_>>(),
            ))
        }
        "asm.replace" => {
            let old = body(&a, ctx)?;
            let new = BodyId(a.id("with")?);
            let report =
                crate::replace::replace(ctx.document, old, new).map_err(CommandError::failed)?;
            solved(
                ctx,
                json!({"kept": report.kept, "unmatched": report.unmatched}),
            )
        }
        "asm.mirror" => {
            let source = body(&a, ctx)?;
            let point = vector(a.0.get("point").unwrap_or(&Value::Null), "point")?;
            let normal = vector(a.0.get("normal").unwrap_or(&Value::Null), "normal")?;
            if normal.length() < 1e-9 {
                return Err(CommandError::bad("normal", "must not be zero"));
            }
            let plane = core_document::MirrorPlane {
                point: point.to_array(),
                normal: normal.normalize().to_array(),
            };
            let copy = ctx
                .document
                .create_mirrored_copy(source, plane, None)
                .ok_or_else(|| {
                    CommandError::bad("body", "cannot be mirrored: a mirror of a mirror")
                })?;
            Ok(json!(copy.0.to_string()))
        }
        "asm.group" => {
            let bodies = body_list(&a)?.unwrap_or_default();
            if bodies.len() < 2 {
                return Err(CommandError::bad("bodies", "must name two bodies or more"));
            }
            if let Some(b) = bodies
                .iter()
                .find(|b| !ctx.document.bodies().iter().any(|x| x.id == **b))
            {
                return Err(CommandError::bad(
                    "bodies",
                    format!("{} is not a body of this document", b.0),
                ));
            }
            let group = crate::RigidGroup::of(ctx.document, &bodies);
            let id = match a.opt_id("group")? {
                Some(id) => {
                    let id = FeatureId(id);
                    ctx.document
                        .update_feature_data(id, core_document::WorkbenchFeature::to_json(&group))
                        .map_err(|e| CommandError::failed(e.to_string()))?;
                    id
                }
                None => {
                    let name = match a.opt_string("name")? {
                        Some(n) => n.to_string(),
                        None => next_name(ctx.document, "Group"),
                    };
                    ctx.document
                        .add_feature_in_body(group, name, Some(bodies[0]))
                        .map_err(|e| CommandError::failed(e.to_string()))?
                }
            };
            ctx.document.clear_feature_dirty(id);
            solved(ctx, json!(id.0.to_string()))
        }
        "asm.motion" => {
            let defaults = crate::MotionStudy::default();
            let study = crate::MotionStudy {
                start: a.opt_number("start")?.map_or(defaults.start, |v| v as f32),
                end: a.opt_number("end")?.map_or(defaults.end, |v| v as f32),
                step: a.opt_number("step")?.map_or(defaults.step, |v| v as f32),
                drives: serde_json::from_value(a.0.get("drives").cloned().unwrap_or(Value::Null))
                    .map_err(|e| CommandError::bad("drives", e.to_string()))?,
            };
            for drive in &study.drives {
                crate::motion::value_at(&drive.formula, 0.0)
                    .map_err(|e| CommandError::bad("drives", e))?;
            }
            let data = core_document::WorkbenchFeature::to_json(&study);
            let id = match a.opt_id("study")? {
                Some(id) => {
                    let id = FeatureId(id);
                    ctx.document
                        .update_feature_data(id, data)
                        .map_err(|e| CommandError::failed(e.to_string()))?;
                    id
                }
                None => {
                    let name = match a.opt_string("name")? {
                        Some(n) => n.to_string(),
                        None => next_name(ctx.document, "Motion"),
                    };
                    ctx.document
                        .add_feature_in_body(study, name, None)
                        .map_err(|e| CommandError::failed(e.to_string()))?
                }
            };
            ctx.document.clear_feature_dirty(id);
            Ok(json!(id.0.to_string()))
        }
        "asm.motion_frames" => {
            let id = FeatureId(a.id("study")?);
            let study = ctx
                .document
                .get_feature_data(id)
                .and_then(|d| {
                    <crate::MotionStudy as core_document::WorkbenchFeature>::from_json(d).ok()
                })
                .ok_or_else(|| CommandError::bad("study", "is not a motion"))?;
            let frames = study.frames(ctx.document).map_err(CommandError::failed)?;
            Ok(Value::Array(
                frames
                    .into_iter()
                    .map(|(t, bodies)| {
                        let bodies: Vec<Value> = bodies
                            .into_iter()
                            .map(|(body, p)| {
                                json!({
                                    "body": body.0.to_string(),
                                    "translation": p.translation,
                                    "rotation": p.rotation,
                                })
                            })
                            .collect();
                        json!({"t": t, "bodies": bodies})
                    })
                    .collect(),
            ))
        }
        "asm.exploded_view" => {
            let steps: Vec<crate::ExplodeStep> =
                serde_json::from_value(a.0.get("steps").cloned().unwrap_or(Value::Null))
                    .map_err(|e| CommandError::bad("steps", e.to_string()))?;
            let view = crate::ExplodedView { steps };
            let data = core_document::WorkbenchFeature::to_json(&view);
            let id = match a.opt_id("view")? {
                Some(id) => {
                    let id = FeatureId(id);
                    ctx.document
                        .update_feature_data(id, data)
                        .map_err(|e| CommandError::failed(e.to_string()))?;
                    id
                }
                None => {
                    let name = match a.opt_string("name")? {
                        Some(n) => n.to_string(),
                        None => next_name(ctx.document, "Exploded view"),
                    };
                    ctx.document
                        .add_feature_in_body(view, name, None)
                        .map_err(|e| CommandError::failed(e.to_string()))?
                }
            };
            ctx.document.clear_feature_dirty(id);
            Ok(json!(id.0.to_string()))
        }
        "asm.explode_at" => {
            let id = FeatureId(a.id("view")?);
            let view = crate::exploded::view_of(ctx.document, id)
                .ok_or_else(|| CommandError::bad("view", "is not an exploded view"))?;
            let start: Vec<(BodyId, BodyPlacement)> = ctx
                .document
                .bodies()
                .iter()
                .map(|b| (b.id, b.placement))
                .collect();
            Ok(Value::Array(
                view.placed_at(&start, a.number("at")? as f32)
                    .into_iter()
                    .map(|(body, p)| {
                        json!({
                            "body": body.0.to_string(),
                            "translation": p.translation,
                            "rotation": p.rotation,
                        })
                    })
                    .collect(),
            ))
        }
        "asm.save_state" => {
            if let Some(state) = a.opt_id("state")? {
                let state = FeatureId(state);
                let now = crate::states::capture(ctx.document);
                ctx.document
                    .update_feature_data(state, core_document::WorkbenchFeature::to_json(&now))
                    .map_err(|e| CommandError::failed(e.to_string()))?;
                ctx.document.clear_feature_dirty(state);
                return Ok(json!(state.0.to_string()));
            }
            let name = match a.opt_string("name")? {
                Some(n) => n.to_string(),
                None => next_name(ctx.document, "State"),
            };
            let id = crate::states::save(ctx.document, name)
                .map_err(|e| CommandError::failed(e.to_string()))?;
            Ok(json!(id.0.to_string()))
        }
        "asm.restore_state" => {
            let id = FeatureId(a.id("state")?);
            let state = ctx
                .document
                .get_feature_data(id)
                .and_then(|d| {
                    <crate::AssemblyState as core_document::WorkbenchFeature>::from_json(d).ok()
                })
                .ok_or_else(|| CommandError::bad("state", "is not a saved assembly state"))?;
            crate::states::restore(ctx.document, &state);
            solved(ctx, Value::Null)
        }
        "asm.redundant" => Ok(Value::Array(
            crate::redundant(ctx.document)
                .into_iter()
                .map(|(id, name)| json!({"joint": id.0.to_string(), "name": name}))
                .collect(),
        )),
        "asm.motion_clashes" => {
            let kernel = ctx
                .kernel
                .ok_or_else(|| CommandError::failed("no kernel to check with"))?;
            let joint = FeatureId(a.id("joint")?);
            let steps = a.opt_number("steps")?.unwrap_or(24.0).clamp(2.0, 1000.0) as usize;
            let (low, high) = (a.number("low")? as f32, a.number("high")? as f32);
            let Some(check) = crate::sweep_check::plan(ctx.document, joint, low, high, steps)
            else {
                return Ok(json!([]));
            };
            let found = check
                .run(
                    kernel,
                    &std::sync::atomic::AtomicUsize::new(0),
                    &std::sync::atomic::AtomicBool::new(false),
                )
                .map_err(CommandError::failed)?;
            Ok(Value::Array(
                found
                    .iter()
                    .map(|c| {
                        json!({
                            "at": c.at,
                            "a": c.a.0.to_string(),
                            "b": c.b.0.to_string(),
                            "volume": c.volume_mm3,
                        })
                    })
                    .collect(),
            ))
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
    if other != crate::WORLD && !ctx.document.bodies().iter().any(|b| b.id == other) {
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
    let name_of = |key: &str| {
        a.0.get(key)
            .and_then(|f| f.get("name"))
            .and_then(Value::as_u64)
            .unwrap_or(0)
    };
    let joint = JointFeature {
        second: None,
        shape: Vec::new(),
        ends: [0.0; 2],
        names: [name_of("face"), name_of("other_face")],
        kind,
        moving,
        other_body: other,
        fixed,
    };
    let mut joint = joint;
    if tool == JointTool::Width {
        let second = anchor("face2", moving_body)?;
        let other_second = anchor("other_face2", other)?;
        joint.second = Some([second, other_second]);
    }
    crate::shapes::take_shape(ctx.document, &mut joint);
    if matches!(joint.kind, JointKind::Path | JointKind::Cam { .. }) && joint.shape.is_empty() {
        return Err(CommandError::bad(
            "other_face",
            "finds no edge or face of the other body there",
        ));
    }
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
        | JointKind::Perpendicular
        | JointKind::Ball
        | JointKind::Universal
        | JointKind::Slot
        | JointKind::Path
        | JointKind::Width => json!({}),
        JointKind::Cam { radius } => json!({"radius": radius}),
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
            let face = |anchor: &Anchor, b: BodyId, name: u64| {
                let mut face = match anchor.moved(&at(b)) {
                    Anchor::Plane { point, normal } => json!({"point": point, "normal": normal}),
                    Anchor::Axis { point, direction } => {
                        json!({"axis": {"point": point, "direction": direction}})
                    }
                    Anchor::Point { point } => json!({"point": point}),
                };
                if name != 0 {
                    face["name"] = json!(name);
                }
                face
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
                "face": face(&joint.moving, body, joint.names[0]),
                "other": joint.other_body.0.to_string(),
                "other_face": face(&joint.fixed, joint.other_body, joint.names[1]),
                "name": node.name,
            }));
            args.extend(object(settings(&joint.kind)));
            if let Some([tab, wall]) = joint.second {
                args.insert("face2".into(), face(&tab, body, 0));
                args.insert("other_face2".into(), face(&wall, joint.other_body, 0));
            }
            ctx.record(command, args, json!(id.0.to_string()));
            if joint.ends != [0.0, 0.0] {
                ctx.record(
                    "asm.set",
                    object(json!({
                        "joint": id.0.to_string(),
                        "moving_end": joint.ends[0],
                        "fixed_end": joint.ends[1],
                    })),
                    Value::Null,
                );
            }
        }
        Some(before) => {
            let Ok(old) = serde_json::from_value::<JointFeature>(before.clone()) else {
                return;
            };
            let with_ends = |j: &JointFeature| {
                let mut all = object(settings(&j.kind));
                all.insert("moving_end".into(), json!(j.ends[0]));
                all.insert("fixed_end".into(), json!(j.ends[1]));
                all
            };
            let (was, now) = (with_ends(&old), with_ends(&joint));
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
    crate::solve::rigid_placements(document)
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
    let point = || -> Result<Anchor, CommandError> {
        let at = face
            .get("centre")
            .or_else(|| face.get("point"))
            .unwrap_or(&Value::Null);
        Ok(Anchor::Point {
            point: vector(at, name)?.to_array(),
        })
    };
    match takes {
        Takes::Flat => flat(),
        Takes::Round => round(),
        Takes::Point => point(),
        Takes::Any | Takes::FlatAndRound | Takes::Directed if face.contains_key("normal") => flat(),
        Takes::Any | Takes::FlatAndRound | Takes::Directed => round(),
        Takes::Anything if face.contains_key("normal") => flat(),
        Takes::Anything if face.contains_key("axis") => round(),
        Takes::Anything => point(),
        // The pin is a point, the slot a line.
        Takes::PointAndLine if name == "face" => point(),
        Takes::PointAndLine => round(),
        Takes::PointAndEdge | Takes::PointAndFace => point(),
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

/// The first joint of an assembly grounds the body it holds against (the
/// world needs none), when nothing is grounded yet: the assembly then stands on it, and what its
/// joints leave free reads true from the start.
pub(crate) fn ground_first(ctx: &mut WorkbenchRuntimeContext, other: BodyId) {
    let all = crate::joints(ctx.document);
    if all.is_empty() && other != crate::WORLD {
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
                second: None,
                shape: Vec::new(),
                ends: [0.0; 2],
                names: [0; 2],
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

    /// A part set bought is bought for every body of its shape, the
    /// bench leaves them out of what is printed, and the list says so.
    #[test]
    fn a_part_is_marked_bought_and_given_a_column() {
        use core_document::Workbench;
        let mut doc = Document::new("t");
        let (a, b) = (doc.create_body(Some("Screw".into())), doc.create_body(None));
        doc.set_imported_brep_data(a, b"screw".to_vec(), Vec::new());
        doc.set_imported_brep_data(b, b"screw".to_vec(), Vec::new());
        call(
            &mut doc,
            "asm.part",
            json!({"body": b.0.to_string(), "bought": true, "number": 4,
                   "values": {"Supplier": "Fasteners Ltd"}}),
        )
        .unwrap();
        let listed = call(&mut doc, "asm.parts", json!({})).unwrap();
        assert_eq!(listed[0]["bought"], json!(true));
        assert_eq!(listed[0]["number"], json!(4));
        assert_eq!(listed[0]["values"]["Supplier"], json!("Fasteners Ltd"));
        let mut not_made = crate::AssemblyWorkbench::default().not_printed(&doc);
        not_made.sort();
        let mut both = vec![a, b];
        both.sort();
        assert_eq!(not_made, both);
    }

    /// Copies turned about an axis spread over a whole turn with the
    /// original: three about Z make four in all, a quarter turn apart.
    #[test]
    fn copies_spread_round_an_axis() {
        let mut doc = Document::new("t");
        let arm = doc.create_body(Some("Arm".into()));
        doc.set_body_placement(
            arm,
            BodyPlacement::new(glam::Quat::IDENTITY, glam::Vec3::new(10.0, 0.0, 0.0)),
        );
        let made = call(
            &mut doc,
            "asm.copy",
            json!({"body": arm.0.to_string(), "count": 3,
                   "around": {"point": [0, 0, 0], "direction": [0, 0, 1]}}),
        )
        .unwrap();
        let at: Vec<[f32; 3]> = made
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                let id = BodyId(uuid::Uuid::parse_str(v.as_str().unwrap()).unwrap());
                doc.body_placement(id).translation
            })
            .collect();
        let want = [[0.0, 10.0], [-10.0, 0.0], [0.0, -10.0]];
        for (got, want) in at.iter().zip(want) {
            assert!(
                (got[0] - want[0]).abs() < 1e-3 && (got[1] - want[1]).abs() < 1e-3,
                "{at:?}"
            );
        }
    }

    /// A body replaced by another: the mate on the old body's underside
    /// goes to the new body's, which sits on the base; the old is hidden.
    #[test]
    fn a_replacement_takes_the_joints_on_its_own_faces() {
        use std::sync::Arc;
        let mut doc = Document::new("t");
        let [base, old, new] = [
            doc.create_body(Some("Base".into())),
            doc.create_body(Some("Old".into())),
            doc.create_body(Some("New".into())),
        ];
        let with_bottom = |z: f32| core_document::ImportedGeometry {
            mesh: Arc::new(kernel_api::TriMesh {
                positions: vec![[0.0, 0.0, z], [10.0, 0.0, z], [0.0, 10.0, z]],
                normals: vec![[0.0, 0.0, -1.0]; 3],
                indices: vec![0, 2, 1],
                faces: vec![0],
                face_surfaces: vec![kernel_api::FaceSurface::Plane {
                    origin: [0.0, 0.0, z],
                    normal: [0.0, 0.0, -1.0],
                }],
                ..kernel_api::TriMesh::default()
            }),
            source_asset: None,
            revision: 0,
            bounds_mm: None,
            brep_blob_path: None,
            face_colors_path: None,
            health: None,
        };
        doc.set_imported_geometry(old, with_bottom(0.0));
        doc.set_imported_geometry(new, with_bottom(2.0));
        doc.set_body_placement(
            old,
            BodyPlacement::new(glam::Quat::IDENTITY, glam::Vec3::new(0.0, 0.0, 30.0)),
        );
        call(
            &mut doc,
            "asm.distance",
            json!({"body": old.0.to_string(), "face": {"point": [0, 0, 30], "normal": [0, 0, -1]},
                   "other": base.0.to_string(), "other_face": {"point": [0, 0, 10], "normal": [0, 0, 1]},
                   "offset": 0}),
        )
        .unwrap();
        assert!((doc.body_placement(old).translation[2] - 10.0).abs() < 1e-3);
        let report = call(
            &mut doc,
            "asm.replace",
            json!({"body": old.0.to_string(), "with": new.0.to_string()}),
        )
        .unwrap();
        assert_eq!(report["unmatched"], json!([]), "{report}");
        let joint = crate::joints(&doc)
            .into_iter()
            .find(|j| j.feature.kind != JointKind::Ground)
            .unwrap();
        assert_eq!(joint.body, new);
        // Its underside, 2 up in its own frame, on the base's top at 10.
        assert!((doc.body_placement(new).translation[2] - 8.0).abs() < 1e-3);
        assert!(doc.bodies().iter().any(|b| b.id == old && b.hidden));
    }

    /// A motion drives a hinge by a formula of time, frame by frame, and
    /// moves nothing in the document.
    #[test]
    fn a_motion_drives_its_joints_over_time() {
        let mut doc = Document::new("t");
        let (a, b) = (doc.create_body(None), doc.create_body(None));
        let pin = json!({"axis": {"point": [0, 0, 0], "direction": [0, 0, 1]}});
        let hinge = call(
            &mut doc,
            "asm.hinge",
            json!({"body": a.0.to_string(), "face": pin, "other": b.0.to_string(), "other_face": pin,
                   "drive": 0}),
        )
        .unwrap();
        let study = call(
            &mut doc,
            "asm.motion",
            json!({"drives": [{"joint": hinge, "formula": "90 * t"}], "start": 0, "end": 1, "step": 0.5}),
        )
        .unwrap();
        let frames = call(&mut doc, "asm.motion_frames", json!({"study": study})).unwrap();
        let frames = frames.as_array().unwrap();
        assert_eq!(frames.len(), 3);
        let turned = |frame: &Value| {
            let row = frame["bodies"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["body"] == json!(a.0.to_string()))
                .unwrap()
                .clone();
            let q: [f32; 4] = serde_json::from_value(row["rotation"].clone()).unwrap();
            let x = glam::Quat::from_array(q) * glam::Vec3::X;
            x.y.atan2(x.x).to_degrees()
        };
        assert!((turned(&frames[1]) - 45.0).abs() < 1e-2);
        assert!((turned(&frames[2]) - 90.0).abs() < 1e-2);
        let x = doc.body_placement(a).direction([1.0, 0.0, 0.0]);
        assert!(x[1].abs() < 1e-4, "the document is not moved");
        let bad = call(
            &mut doc,
            "asm.motion",
            json!({"drives": [{"joint": hinge, "formula": "90 * "}]}),
        );
        assert!(bad.is_err());
    }

    /// An exploded view kept by a script plays its steps in order.
    #[test]
    fn an_exploded_view_is_kept_and_played() {
        let mut doc = Document::new("t");
        let (a, b) = (doc.create_body(None), doc.create_body(None));
        let view = call(
            &mut doc,
            "asm.exploded_view",
            json!({"steps": [
                {"bodies": [a.0.to_string()], "shift": [0, 0, 10]},
                {"bodies": [a.0.to_string(), b.0.to_string()], "shift": [4, 0, 0]},
            ]}),
        )
        .unwrap();
        let at = call(&mut doc, "asm.explode_at", json!({"view": view, "at": 1.5})).unwrap();
        let of = |body: BodyId| {
            at.as_array()
                .unwrap()
                .iter()
                .find(|r| r["body"] == json!(body.0.to_string()))
                .unwrap()["translation"]
                .clone()
        };
        assert_eq!(of(a), json!([2.0, 0.0, 10.0]));
        assert_eq!(of(b), json!([2.0, 0.0, 0.0]));
        assert_eq!(doc.body_placement(a).translation, [0.0; 3], "nothing moved");
    }

    /// A saved state brings back a driven hinge's angle, where the bodies
    /// sat and which were hidden.
    #[test]
    fn a_saved_state_is_returned_to() {
        let mut doc = Document::new("t");
        let (a, b) = (doc.create_body(None), doc.create_body(None));
        let pin = json!({"axis": {"point": [0, 0, 0], "direction": [0, 0, 1]}});
        let hinge = call(
            &mut doc,
            "asm.hinge",
            json!({"body": a.0.to_string(), "face": pin, "other": b.0.to_string(), "other_face": pin,
                   "drive": 0}),
        )
        .unwrap();
        let folded = call(&mut doc, "asm.save_state", json!({"name": "Folded"})).unwrap();
        call(&mut doc, "asm.set", json!({"joint": hinge, "drive": 90})).unwrap();
        doc.set_body_visible(b, false);
        let turned = |doc: &Document| {
            let x = doc.body_placement(a).direction([1.0, 0.0, 0.0]);
            x[1].atan2(x[0]).to_degrees()
        };
        assert!((turned(&doc) - 90.0).abs() < 1e-2);
        call(&mut doc, "asm.restore_state", json!({"state": folded})).unwrap();
        assert!(turned(&doc).abs() < 1e-2, "{}", turned(&doc));
        assert!(doc.bodies().iter().all(|x| !x.hidden));
    }

    /// A parallel joint added to a mate holds nothing new: it is reported
    /// redundant; the mate alone is not.
    #[test]
    fn a_joint_holding_nothing_new_is_redundant() {
        let mut doc = Document::new("t");
        let (a, b) = (doc.create_body(None), doc.create_body(None));
        let top = json!({"point": [0, 0, 0], "normal": [0, 0, 1]});
        let under = json!({"point": [0, 0, 0], "normal": [0, 0, -1]});
        call(
            &mut doc,
            "asm.mate",
            json!({"body": a.0.to_string(), "face": under, "other": b.0.to_string(), "other_face": top}),
        )
        .unwrap();
        assert_eq!(
            call(&mut doc, "asm.redundant", json!({})).unwrap(),
            json!([])
        );
        call(
            &mut doc,
            "asm.parallel",
            json!({"body": a.0.to_string(), "face": under, "other": b.0.to_string(), "other_face": top,
                   "name": "Extra"}),
        )
        .unwrap();
        let found = call(&mut doc, "asm.redundant", json!({})).unwrap();
        let names: Vec<&str> = found
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"Extra"), "{names:?}");
    }

    /// A tab centred in a slot: its middle on the slot's middle.
    #[test]
    fn a_tab_is_centred_in_its_slot() {
        let mut doc = Document::new("t");
        let (tab, slot) = (doc.create_body(None), doc.create_body(None));
        let face = |x: f32, n: f32| json!({"point": [x, 0, 0], "normal": [n, 0, 0]});
        call(
            &mut doc,
            "asm.width",
            json!({"body": tab.0.to_string(), "face": face(0.0, -1.0), "face2": face(4.0, 1.0),
                   "other": slot.0.to_string(), "other_face": face(10.0, 1.0),
                   "other_face2": face(20.0, -1.0)}),
        )
        .unwrap();
        let middle = doc.body_placement(tab).point([2.0, 0.0, 0.0]);
        assert!((middle[0] - 15.0).abs() < 1e-3, "{middle:?}");
    }

    /// A point on a path runs along the edge it was put on; a follower on
    /// a cam sits a roller's radius off the cam's face.
    #[test]
    fn a_path_and_a_cam_hold_to_the_other_body_s_edge_and_face() {
        use std::sync::Arc;
        let mut doc = Document::new("t");
        let (a, b) = (doc.create_body(None), doc.create_body(None));
        // An L-shaped edge along X then Y at height 10, and a floor face at
        // z = 10 facing up.
        let mesh = kernel_api::TriMesh {
            positions: vec![
                [0.0, 0.0, 10.0],
                [20.0, 0.0, 10.0],
                [20.0, 20.0, 10.0],
                [0.0, 20.0, 10.0],
            ],
            normals: vec![[0.0, 0.0, 1.0]; 4],
            indices: vec![0, 1, 2, 0, 2, 3],
            faces: vec![0, 0],
            edges: vec![0, 1, 1, 2],
            edge_ids: vec![4, 4],
            ..kernel_api::TriMesh::default()
        };
        doc.set_imported_geometry(
            b,
            core_document::ImportedGeometry {
                mesh: Arc::new(mesh),
                source_asset: None,
                revision: 0,
                bounds_mm: None,
                brep_blob_path: None,
                face_colors_path: None,
                health: None,
            },
        );
        doc.set_body_placement(
            a,
            BodyPlacement::new(glam::Quat::IDENTITY, glam::Vec3::new(18.0, 6.0, 15.0)),
        );
        call(
            &mut doc,
            "asm.path",
            json!({"body": a.0.to_string(), "face": {"point": [18, 6, 15]}, "other": b.0.to_string(),
                   "other_face": {"point": [20, 5, 10]}}),
        )
        .unwrap();
        let at = doc.body_placement(a).point([0.0; 3]);
        let joint = crate::joints(&doc)
            .into_iter()
            .find(|j| j.body == a)
            .unwrap();
        assert!(
            (at[0] - 20.0).abs() < 1e-3 && (at[2] - 10.0).abs() < 1e-3,
            "{at:?}"
        );
        assert_eq!(joint.feature.shape.len(), 3, "the whole L");

        let mut doc2 = doc.clone();
        let doc = &mut doc2;
        for id in crate::joints(doc).iter().map(|j| j.id).collect::<Vec<_>>() {
            doc.remove_feature(id).unwrap();
        }
        call(
            doc,
            "asm.cam",
            json!({"body": a.0.to_string(), "face": {"point": [5, 5, 30]}, "other": b.0.to_string(),
                   "other_face": {"point": [5, 5, 10]}, "radius": 2}),
        )
        .unwrap();
        // The follower was picked at (5, 5, 30) where the body stood.
        let cam = crate::joints(doc)
            .into_iter()
            .find(|j| j.body == a)
            .unwrap();
        let Anchor::Point { point } = cam.feature.moving else {
            panic!()
        };
        let follower = doc.body_placement(a).point(point);
        assert!((follower[2] - 12.0).abs() < 1e-3, "{follower:?}");
    }

    /// A pin in a slot rides on the slot's line with four motions left; a
    /// universal joint crosses two pins at a point with two.
    #[test]
    fn a_slot_and_a_universal_joint_leave_what_they_should() {
        let motions = |doc: &Document, body: BodyId| {
            crate::freedom(doc)
                .into_iter()
                .find(|(b, _)| *b == body)
                .map(|(_, m)| m.len())
                .unwrap()
        };
        let mut doc = Document::new("t");
        let (a, b) = (doc.create_body(None), doc.create_body(None));
        call(
            &mut doc,
            "asm.slot",
            json!({"body": a.0.to_string(), "face": {"point": [0, 0, 5]}, "other": b.0.to_string(),
                   "other_face": {"axis": {"point": [10, 0, 0], "direction": [1, 0, 0]}}}),
        )
        .unwrap();
        let pin = doc.body_placement(a).point([0.0, 0.0, 5.0]);
        assert!(pin[1].abs() < 1e-3 && pin[2].abs() < 1e-3, "{pin:?}");
        assert_eq!(motions(&doc, a), 4);

        let mut doc = Document::new("t");
        let (a, b) = (doc.create_body(None), doc.create_body(None));
        doc.set_body_placement(
            a,
            BodyPlacement::new(glam::Quat::IDENTITY, glam::Vec3::new(3.0, 2.0, 1.0)),
        );
        call(
            &mut doc,
            "asm.universal",
            json!({"body": a.0.to_string(), "face": {"axis": {"point": [3, 2, 1], "direction": [1, 0, 0]}},
                   "other": b.0.to_string(), "other_face": {"axis": {"point": [0, 0, 0], "direction": [0, 1, 0]}}}),
        )
        .unwrap();
        let centre = doc.body_placement(a).point([0.0; 3]);
        assert!(glam::Vec3::from_array(centre).length() < 1e-3, "{centre:?}");
        assert_eq!(motions(&doc, a), 2);
    }

    /// An end moved along its normal moves the body with it: a mate with
    /// the other end raised 3 sits 3 higher.
    #[test]
    fn an_end_offset_moves_the_body() {
        let mut doc = Document::new("t");
        let (a, b) = (doc.create_body(None), doc.create_body(None));
        let top = json!({"point": [0, 0, 0], "normal": [0, 0, 1]});
        let mate = call(
            &mut doc,
            "asm.mate",
            json!({"body": a.0.to_string(), "face": {"point": [0, 0, 0], "normal": [0, 0, -1]},
                   "other": b.0.to_string(), "other_face": top}),
        )
        .unwrap();
        call(&mut doc, "asm.set", json!({"joint": mate, "fixed_end": 3})).unwrap();
        assert!((doc.body_placement(a).translation[2] - 3.0).abs() < 1e-3);
        call(&mut doc, "asm.set", json!({"joint": mate, "moving_end": 1})).unwrap();
        // The moving end, its underside, pushed 1 down its own normal.
        assert!((doc.body_placement(a).translation[2] - 4.0).abs() < 1e-3);
    }

    /// A ball joint puts two points together and leaves three turns free;
    /// a distance holds between points or parallel axes as it does between
    /// faces, and parallel takes axes.
    #[test]
    fn points_and_axes_are_held_as_faces_are() {
        let mut doc = Document::new("t");
        let (a, b) = (doc.create_body(None), doc.create_body(None));
        let ball = json!({"centre": [0, 0, 20], "point": [0, 0, 25]});
        let socket = json!({"point": [5, 5, 5]});
        call(
            &mut doc,
            "asm.ball",
            json!({"body": a.0.to_string(), "face": ball, "other": b.0.to_string(), "other_face": socket}),
        )
        .unwrap();
        let centre = doc.body_placement(a).point([0.0, 0.0, 20.0]);
        assert!(
            (centre[0] - 5.0).abs() < 1e-3
                && (centre[1] - 5.0).abs() < 1e-3
                && (centre[2] - 5.0).abs() < 1e-3,
            "{centre:?}"
        );
        let free = crate::freedom(&doc);
        let motions = &free.iter().find(|(body, _)| *body == a).unwrap().1;
        assert_eq!(motions.len(), 3, "three turns: {motions:?}");

        let mut doc = Document::new("t");
        let (a, b) = (doc.create_body(None), doc.create_body(None));
        let axis = |x: f32| json!({"axis": {"point": [x, 0, 0], "direction": [0, 0, 1]}});
        call(
            &mut doc,
            "asm.parallel",
            json!({"body": a.0.to_string(), "face": axis(0.0), "other": b.0.to_string(), "other_face": axis(0.0)}),
        )
        .unwrap();
        call(
            &mut doc,
            "asm.distance",
            json!({"body": a.0.to_string(), "face": axis(0.0), "other": b.0.to_string(),
                   "other_face": axis(0.0), "offset": 30}),
        )
        .unwrap();
        let at = doc.body_placement(a).point([0.0, 0.0, 0.0]);
        assert!(
            ((at[0] * at[0] + at[1] * at[1]).sqrt() - 30.0).abs() < 1e-3,
            "{at:?}"
        );

        let mut doc = Document::new("t");
        let (a, b) = (doc.create_body(None), doc.create_body(None));
        call(
            &mut doc,
            "asm.distance",
            json!({"body": a.0.to_string(), "face": {"point": [0, 0, 0]}, "other": b.0.to_string(),
                   "other_face": {"point": [0, 0, 0]}, "offset": 12}),
        )
        .unwrap();
        let at = doc.body_placement(a).point([0.0, 0.0, 0.0]);
        assert!(
            (glam::Vec3::from_array(at).length() - 12.0).abs() < 1e-3,
            "{at:?}"
        );
    }

    /// A group moves as one: the body held to its first member follows it
    /// when a mate moves that one.
    #[test]
    fn a_rigid_group_moves_as_one() {
        let mut doc = Document::new("t");
        let [base, lid, knob] = [
            doc.create_body(None),
            doc.create_body(None),
            doc.create_body(None),
        ];
        doc.set_body_placement(
            lid,
            BodyPlacement::new(glam::Quat::IDENTITY, glam::Vec3::new(0.0, 0.0, 30.0)),
        );
        doc.set_body_placement(
            knob,
            BodyPlacement::new(glam::Quat::IDENTITY, glam::Vec3::new(5.0, 0.0, 40.0)),
        );
        let group = call(
            &mut doc,
            "asm.group",
            json!({"bodies": [lid.0.to_string(), knob.0.to_string()]}),
        )
        .unwrap();
        assert!(group.is_string());
        // The lid's underside, 30 up, onto the base's top at 10.
        call(
            &mut doc,
            "asm.distance",
            json!({"body": lid.0.to_string(), "face": {"point": [0, 0, 30], "normal": [0, 0, -1]},
                   "other": base.0.to_string(), "other_face": {"point": [0, 0, 10], "normal": [0, 0, 1]},
                   "offset": 0}),
        )
        .unwrap();
        let lid_z = doc.body_placement(lid).translation[2];
        let knob_z = doc.body_placement(knob).translation[2];
        assert!((lid_z - 10.0).abs() < 1e-3, "{lid_z}");
        assert!(
            (knob_z - 20.0).abs() < 1e-3,
            "the knob keeps its 10 above: {knob_z}"
        );
        assert!(
            call(
                &mut doc,
                "asm.group",
                json!({"bodies": [lid.0.to_string()]})
            )
            .is_err()
        );
    }

    /// A joint to the world holds the body to the origin's planes, and
    /// grounds nothing.
    #[test]
    fn a_body_mates_to_the_origin() {
        let mut doc = Document::new("t");
        let a = doc.create_body(None);
        doc.set_body_placement(
            a,
            BodyPlacement::new(glam::Quat::IDENTITY, glam::Vec3::new(3.0, 4.0, 25.0)),
        );
        let bottom = json!({"point": [3, 4, 25], "normal": [0, 0, -1]});
        let xy = json!({"point": [0, 0, 0], "normal": [0, 0, 1]});
        call(
            &mut doc,
            "asm.mate",
            json!({"body": a.0.to_string(), "face": bottom, "other": crate::WORLD.0.to_string(),
                   "other_face": xy}),
        )
        .unwrap();
        let t = doc.body_placement(a).translation;
        assert!(t[2].abs() < 1e-3, "on the XY plane: {t:?}");
        assert!(
            crate::joints(&doc)
                .iter()
                .all(|j| j.feature.kind != JointKind::Ground),
            "the origin needs no ground"
        );
    }

    /// A joint made another kind keeps its faces and name; faces of the
    /// wrong sort for the kind asked are refused, and new faces go in.
    #[test]
    fn a_joint_changes_kind_and_faces() {
        let mut doc = Document::new("t");
        let (a, b) = (doc.create_body(None), doc.create_body(None));
        let top = json!({"point": [0, 0, 0], "normal": [0, 0, 1]});
        let mate = call(
            &mut doc,
            "asm.mate",
            json!({"body": a.0.to_string(), "face": top, "other": b.0.to_string(), "other_face": top}),
        )
        .unwrap();
        let id = FeatureId(uuid::Uuid::parse_str(mate.as_str().unwrap()).unwrap());
        let kind = |doc: &Document| {
            serde_json::from_value::<JointFeature>(doc.get_feature_data(id).unwrap().clone())
                .unwrap()
                .kind
        };
        call(
            &mut doc,
            "asm.set",
            json!({"joint": mate, "kind": "parallel"}),
        )
        .unwrap();
        assert_eq!(kind(&doc), JointKind::Parallel);
        assert!(call(&mut doc, "asm.set", json!({"joint": mate, "kind": "hinge"})).is_err());
        let pin = json!({"axis": {"point": [0, 0, 0], "direction": [0, 0, 1]}});
        call(
            &mut doc,
            "asm.set",
            json!({"joint": mate, "kind": "hinge", "face": pin, "other_face": pin}),
        )
        .unwrap();
        assert!(matches!(kind(&doc), JointKind::Hinge { .. }));
        assert_eq!(doc.get_feature_meta(id).unwrap().name, "Mate 1");
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
