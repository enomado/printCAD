//! Design's commands: add a feature, or change one, by numbers, for
//! scripts and other callers that are not a click.
//!
//! A feature is made the way its toolbar button makes it, from the sketch
//! and body the call names instead of the selection, with the defaults the
//! button gives; any field of the feature named in the call replaces its
//! default. `pc.doc.feature{id = ...}` shows a feature's fields.

use core_document::{
    Args, BodyId, CommandArgs, CommandError, CommandResult, CommandSpec, FeatureId, ParamKind,
    WorkbenchContext, WorkbenchFeature, WorkbenchRuntimeContext,
};
use serde_json::{Map, Value, json};

use core_document::{AttachmentOffset, DatumFeature, DatumShape};

use crate::DesignWorkbench;
use crate::feature::DesignFeature;

/// The features a command makes, by tool id, and what each is.
const FEATURES: &[(&str, &str)] = &[
    ("design.pad", "Pad a sketch"),
    ("design.pocket", "Cut a sketch into the body"),
    ("design.revolve", "Turn a sketch about an axis"),
    ("design.groove", "Cut a sketch turned about an axis"),
    ("design.loft", "Loft through sketches"),
    ("design.subtractive_loft", "Cut a loft through sketches"),
    ("design.pipe", "Sweep a sketch along a path"),
    ("design.subtractive_pipe", "Cut a sketch swept along a path"),
    ("design.helix", "Sweep a sketch along a helix"),
    (
        "design.subtractive_helix",
        "Cut a sketch swept along a helix",
    ),
    (
        "design.primitive",
        "Add a box, cylinder, sphere, cone, torus or wedge",
    ),
    (
        "design.subtractive_primitive",
        "Cut a box, cylinder, sphere, cone, torus or wedge",
    ),
    (
        "design.hole",
        "Drill holes at a sketch's circles and points",
    ),
    ("design.fillet", "Round edges"),
    ("design.chamfer", "Bevel edges"),
    ("design.draft", "Tilt faces"),
    ("design.thickness", "Hollow the solid"),
    (
        "design.delete_faces",
        "Delete faces and close the openings from their neighbours",
    ),
    (
        "design.offset_faces",
        "Push or pull faces along their normals, their neighbours following",
    ),
    (
        "design.move_faces",
        "Move or turn faces, their neighbours following",
    ),
    ("design.mirror", "Mirror the last feature"),
    (
        "design.linear_pattern",
        "Repeat the last feature along a line",
    ),
    (
        "design.polar_pattern",
        "Repeat the last feature about an axis",
    ),
    ("design.scaled", "Scale the last feature"),
    ("design.boolean", "Combine with another body"),
];

/// Arguments every feature command reads itself rather than as a field.
const OWN_ARGS: &[&str] = &[
    "sketch",
    "body",
    "name",
    "variant",
    "face_point",
    "face_normal",
];

/// Feature command `id`'s notes and examples, beyond what every feature
/// command says.
fn explained(id: &str, spec: CommandSpec) -> CommandSpec {
    match id {
        "design.pad" => spec
            .note(
                "`length` is 10 mm when left out. The pad grows along the sketch's normal; \
                 `reversed = true` grows it the other way, `symmetric = true` half each way.",
            )
            .note(
                "It goes in the sketch's body. With no sketch, `face_point` and `face_normal` \
                 name a flat face of the solid to extrude instead.",
            )
            .see_also("sketch.new")
            .see_also("design.pocket")
            .example(
                "A plate padded from a rectangle",
                r#"
                local s = pc.sketch.new{plane = "XY"}
                pc.sketch.rect{sketch = s, x = 0, y = 0, width = 40, height = 30}
                local pad = pc.design.pad{sketch = s, length = 5}
                assert(#pc.doc.rebuild() == 0, "the pad builds")
                local body = pc.doc.feature{id = pad}.body
                assert(math.abs(pc.doc.measure{body = body}.volume - 40 * 30 * 5) < 1e-3)
                assert(#pc.doc.faces{body = body} == 6, "a box has six faces")
                "#,
            ),
        "design.pocket" => spec
            .note(
                "It cuts against its sketch's normal: from a sketch at the top face's height \
                 it digs down into the solid. From a sketch on the bottom (XY at 0 under a \
                 pad) it cuts away from the material and removes nothing, without an \
                 error; `reversed = true` turns it.",
            )
            .note(
                "`through_all = true` cuts through everything; else `depth`, 5 mm when left \
                 out.",
            )
            .note(
                "It is refused in a body with no solid feature yet. Its sketch must be in \
                 the padded body: `pc.sketch.new{body = ...}`, since a sketch made without \
                 `body` starts a new one.",
            )
            .see_also("sketch.new")
            .see_also("design.hole")
            .example(
                "A square window cut through a plate",
                r#"
                local s = pc.sketch.new{plane = "XY"}
                pc.sketch.rect{sketch = s, x = 0, y = 0, width = 40, height = 30}
                local pad = pc.design.pad{sketch = s, length = 5}
                local body = pc.doc.feature{id = pad}.body
                local top = pc.sketch.new{body = body, plane = "XY", offset = 5}
                pc.sketch.rect{sketch = top, x = 15, y = 10, width = 10, height = 10}
                pc.design.pocket{sketch = top, through_all = true}
                assert(#pc.doc.rebuild() == 0, "the pocket builds")
                local volume = pc.doc.measure{body = body}.volume
                assert(math.abs(volume - (40 * 30 - 10 * 10) * 5) < 1e-3, volume)
                assert(#pc.doc.faces{body = body} == 10, "six faces and four walls")
                "#,
            ),
        "design.hole" => spec
            .note(
                "It drills at every circle's centre and every point of its sketch; the \
                 circles' sizes are ignored, the hole's own `diameter` (5 mm when left out) \
                 is what it drills.",
            )
            .note(
                "It drills against the sketch's normal, `depth` deep (10 mm) or \
                 `through_all = true`. A sketch on the bottom of a pad drills away from the \
                 material and removes nothing, without an error; `reversed = true` turns it.",
            )
            .note(
                "Counterbores, countersinks and threads are fields too (`cut`, `threaded`, \
                 `thread`); docs/HOLES.md describes them, and `pc.doc.feature{id = ...}` \
                 shows a hole's fields.",
            )
            .see_also("sketch.circle")
            .see_also("design.pocket")
            .example(
                "Two holes through a plate, one at a circle and one at a point",
                r#"
                local s = pc.sketch.new{plane = "XY"}
                pc.sketch.rect{sketch = s, x = 0, y = 0, width = 40, height = 20}
                local pad = pc.design.pad{sketch = s, length = 4}
                local body = pc.doc.feature{id = pad}.body
                local at = pc.sketch.new{body = body, plane = "XY", offset = 4}
                pc.sketch.circle{sketch = at, x = 10, y = 10, radius = 1}
                pc.sketch.point{sketch = at, x = 30, y = 10}
                pc.design.hole{sketch = at, diameter = 6, through_all = true}
                assert(#pc.doc.rebuild() == 0, "the holes build")
                local bores = 0
                for _, face in ipairs(pc.doc.faces{body = body}) do
                  if face.kind == "cylinder" then
                    bores = bores + 1
                    assert(math.abs(face.radius - 3) < 1e-6, "the hole's diameter, not the circle's")
                  end
                end
                assert(bores == 2)
                local volume = pc.doc.measure{body = body}.volume
                assert(math.abs(volume - (40 * 20 - 2 * math.pi * 9) * 4) < 1e-3, volume)
                "#,
            ),
        "design.fillet" | "design.chamfer" => {
            let (size, round) = if id == "design.fillet" {
                ("`radius`", "rounds")
            } else {
                ("`size`", "bevels")
            };
            let spec = spec
                .note(&format!(
                    "Without `edges` it {round} every edge of the solid (`edges = \"All\"`); \
                     {size} is 1 mm when left out. In the app, edges selected in the view \
                     are taken instead."
                ))
                .note(
                    "`edges = {Edges = {{point = {x, y, z}, direction = {x, y, z}}, ...}}` \
                     picks edges by a point on each and its direction there, and \
                     `edges = {Faces = {{point = .., normal = ..}}}` every edge around those \
                     faces, in the body's own frame. A bare list of picks is refused.",
                )
                .note(
                    "Edges running on tangentially are taken too (`follow_tangent`, on by \
                     default).",
                )
                .see_also("doc.faces");
            if id == "design.fillet" {
                spec.note(
                    "A radius larger than the faces beside an edge can take fails at \
                     `pc.doc.rebuild()`.",
                )
                .see_also("design.chamfer")
                .example(
                    "One top edge of a block rounded",
                    r#"
                    local s = pc.sketch.new{plane = "XY"}
                    pc.sketch.rect{sketch = s, x = 0, y = 0, width = 40, height = 30}
                    local pad = pc.design.pad{sketch = s, length = 10}
                    local body = pc.doc.feature{id = pad}.body
                    pc.design.fillet{body = body, radius = 3,
                      edges = {Edges = {{point = {20, 0, 10}, direction = {1, 0, 0}}}}}
                    assert(#pc.doc.rebuild() == 0, "the fillet builds")
                    assert(#pc.doc.faces{body = body} == 7, "six faces and the round")
                    local taken = (9 - math.pi * 9 / 4) * 40
                    local volume = pc.doc.measure{body = body}.volume
                    assert(math.abs(volume - (12000 - taken)) < 0.01, volume)
                    "#,
                )
            } else {
                spec.note(
                    "`mode` is EqualDistance, TwoDistances (with `size2`) or DistanceAngle \
                     (with `angle_deg`).",
                )
                .see_also("design.fillet")
                .example(
                    "The top face's edges bevelled",
                    r#"
                    local s = pc.sketch.new{plane = "XY"}
                    pc.sketch.rect{sketch = s, x = 0, y = 0, width = 40, height = 30}
                    local pad = pc.design.pad{sketch = s, length = 10}
                    local body = pc.doc.feature{id = pad}.body
                    pc.design.chamfer{body = body, size = 2,
                      edges = {Faces = {{point = {20, 15, 10}, normal = {0, 0, 1}}}}}
                    assert(#pc.doc.rebuild() == 0, "the chamfer builds")
                    assert(#pc.doc.faces{body = body} == 10, "six faces and four bevels")
                    -- A 2 x 2 prism along each edge, less what two share at a corner.
                    local taken = 2 * (40 + 30) * 2 - 4 * 8 / 3
                    local volume = pc.doc.measure{body = body}.volume
                    assert(math.abs(volume - (12000 - taken)) < 0.01, volume)
                    "#,
                )
            }
        }
        _ => spec,
    }
}

/// Register every command this module runs.
pub fn register(context: &mut WorkbenchContext) {
    for (id, summary) in FEATURES {
        let mut spec = CommandSpec::new(*id, *summary)
            .optional("sketch", ParamKind::Id, "The sketch it uses")
            .optional(
                "body",
                ParamKind::Id,
                "The body it goes in; the sketch's body when left out",
            )
            .optional("name", ParamKind::String, "Its name in the tree")
            .optional(
                "face_point",
                ParamKind::List,
                "A face it takes as the viewport's picked face (a thickness's \
                 opening, a draft's neutral plane, a mirror's plane, the profile of a pad or a \
                 pocket given no sketch): a point of it, {x, y, z}, in the body's own frame",
            )
            .optional(
                "face_normal",
                ParamKind::List,
                "With face_point: the face's outward normal, {x, y, z}",
            );
        if id.ends_with("primitive") {
            spec = spec.optional(
                "variant",
                ParamKind::String,
                "box (the default), cylinder, sphere, cone, torus or wedge",
            );
        }
        context.register_command(explained(
            id,
            spec.extra_args("Any field of the feature, such as length = 20 or reversed = true")
                .returns("the feature's id")
                .note(
                    "It makes the feature and builds nothing: a feature that cannot build \
                     is told by `pc.doc.rebuild()`, in the list it returns, and its body \
                     stays the solid before it. A misspelt field is refused here, naming the \
                     fields the feature has.",
                )
                .see_also("doc.rebuild")
                .see_also("design.set"),
        ));
    }
    context.register_command(
        CommandSpec::new("design.set", "Change fields of a Design feature or a datum")
            .param("feature", ParamKind::Id, "The feature to change")
            .extra_args(
                "The fields to change, such as length = 25; a datum takes offset {x, y, z}, \
             rotation and flip as design.datum does",
            )
            .returns("nothing"),
    );
    context.register_command(
        CommandSpec::new(
            "design.datum",
            "Add a datum plane, line, point or coordinate system",
        )
        .param(
            "kind",
            ParamKind::String,
            "plane, line, point or coordinate_system",
        )
        .param("body", ParamKind::Id, "The body it belongs to")
        .optional(
            "mode",
            ParamKind::String,
            "What it attaches to: base_plane (the default), face, three_points, \
             normal_to_edge, along_edge, two_points, plane_intersection, curve_centre \
             or inertia; references are in the body's own frame and follow its solid",
        )
        .optional(
            "plane",
            ParamKind::String,
            "base_plane: the base plane it sits on, XY (the default), XZ or YZ",
        )
        .optional(
            "face_point",
            ParamKind::List,
            "face: a point of the face, {x, y, z}; without a mode, a flat face kept as given",
        )
        .optional(
            "face_normal",
            ParamKind::List,
            "With face_point: the face's outward normal, {x, y, z}",
        )
        .optional(
            "edge_point",
            ParamKind::List,
            "normal_to_edge, along_edge, curve_centre: a point of the edge, {x, y, z}",
        )
        .optional(
            "edge_direction",
            ParamKind::List,
            "With edge_point: the way the edge runs there, {x, y, z}",
        )
        .optional(
            "spot",
            ParamKind::String,
            "normal_to_edge: where on the edge, picked (the default), start, end, middle \
             or centre",
        )
        .optional(
            "points",
            ParamKind::List,
            "three_points, two_points: each {x, y, z}, or {face_point, face_normal}, or \
             {edge_point, edge_direction, spot}",
        )
        .optional(
            "planes",
            ParamKind::List,
            "plane_intersection: two of XY, XZ, YZ, a datum's id, or {face_point, face_normal}",
        )
        .optional(
            "offset",
            ParamKind::List,
            "Moved along its own x, y and normal, {x, y, z} in mm",
        )
        .optional(
            "rotation",
            ParamKind::Number,
            "Turned about its normal, degrees",
        )
        .optional("flip", ParamKind::Bool, "Turned to face the other way")
        .optional(
            "tilt",
            ParamKind::List,
            "Tilted about its own x-axis, then its y-axis, {x, y} in degrees",
        )
        .optional("size", ParamKind::Number, "How large it draws, mm")
        .optional("name", ParamKind::String, "Its name in the tree")
        .returns("the datum's id"),
    );
    context.register_command(
        CommandSpec::new(
            "design.borrow",
            "Borrow another body's sketch, or faces and edges of its solid",
        )
        .param("body", ParamKind::Id, "The body that borrows")
        .optional(
            "sketch",
            ParamKind::Id,
            "A sketch of another body: its profile, for this body's features",
        )
        .optional(
            "from",
            ParamKind::Id,
            "Or the body whose solid lends faces and edges",
        )
        .optional(
            "faces",
            ParamKind::List,
            "With from: faces it lends, each {point, normal} in that body's own frame",
        )
        .optional(
            "edges",
            ParamKind::List,
            "With from: edges it lends, each {point, direction} in that body's own frame",
        )
        .optional(
            "frozen",
            ParamKind::Bool,
            "Keep the geometry as it is now rather than follow the source",
        )
        .optional(
            "options",
            ParamKind::Any,
            "How it lends: {offset = {translation = {x, y, z}, rotation_deg, tilt = {x, y}, \
             flip}} moves and turns it along and about this body's axes; fill = true makes \
             closed borrowed edges a face features take as a profile; whole = true lends the \
             whole solid's edges as reference",
        )
        .optional("name", ParamKind::String, "Its name in the tree")
        .returns("the borrow's id"),
    );
    context.register_command(
        CommandSpec::new(
            "design.recognize_holes",
            "Make the round holes of a body's solid Hole features: their faces deleted, \
             and each set of alike holes drilled again from a sketch of their centres",
        )
        .param("body", ParamKind::Id, "The body")
        .returns(
            "{holes, left, features}: the holes made features, the bores left as they are \
             (counterbores, slots) and the features added",
        ),
    );
    context.register_command(
        CommandSpec::new(
            "design.freeze",
            "Freeze borrowed geometry as it is now, or let it follow its source again",
        )
        .param("feature", ParamKind::Id, "The borrow")
        .optional(
            "frozen",
            ParamKind::Bool,
            "true (the default) takes the source as it is now; false follows it again",
        )
        .returns("nothing"),
    );
    context.register_command(
        CommandSpec::new(
            "design.move_to_body",
            "Move a feature into another body's history, with the sketch and datums only it uses",
        )
        .param("feature", ParamKind::Id, "The feature")
        .param("body", ParamKind::Id, "The body it goes to, in at its tip")
        .returns("the ids of the features moved, the given one last"),
    );
    context.register_command(
        CommandSpec::new(
            "design.duplicate",
            "Make a copy of a feature, with its own copies of the sketches and datums it reads",
        )
        .param("feature", ParamKind::Id, "The feature")
        .optional(
            "body",
            ParamKind::Id,
            "The body the copy goes in, at its tip (the feature's own when left out)",
        )
        .returns("the ids of the features made, the copy of the given one last"),
    );
    context.register_command(
        CommandSpec::new(
            "design.centre_line",
            "Measure the centre line of a tube-like solid between two of its faces",
        )
        .param(
            "body",
            ParamKind::Id,
            "The body whose solid it runs through",
        )
        .param(
            "from_point",
            ParamKind::List,
            "A point of the face it starts at, {x, y, z}, in the body's own frame",
        )
        .param(
            "from_normal",
            ParamKind::List,
            "That face's outward normal, {x, y, z}",
        )
        .param(
            "to_point",
            ParamKind::List,
            "A point of the face it ends at, {x, y, z}",
        )
        .param(
            "to_normal",
            ParamKind::List,
            "That face's outward normal, {x, y, z}",
        )
        .optional(
            "tolerance",
            ParamKind::Number,
            "How closely it follows the sections' centres, mm (0.02 when left out)",
        )
        .returns(
            "{length, points, deviation, straight}: its length in mm, points along it \
             in the body's frame, the largest distance measured from a section's centre \
             to it, and whether it is one straight segment",
        )
        .read_only(),
    );
}

/// Run command `id` with `bench` making the features.
pub fn run(
    bench: &DesignWorkbench,
    id: &str,
    args: &CommandArgs,
    ctx: &mut WorkbenchRuntimeContext,
) -> CommandResult {
    let a = Args(args);
    if id == "design.set" {
        return set(&a, args, ctx);
    }
    if id == "design.datum" {
        return datum(&a, ctx);
    }
    if id == "design.centre_line" {
        return crate::centre::command(&a, ctx);
    }
    if id == "design.borrow" {
        return borrow(&a, ctx);
    }
    if id == "design.freeze" {
        return freeze(&a, ctx);
    }
    if id == "design.recognize_holes" {
        let body = BodyId(a.id("body")?);
        if !ctx.document.bodies().iter().any(|b| b.id == body) {
            return Err(CommandError::bad("body", "is not a body of this document"));
        }
        let made = crate::recognize::recognize_holes(ctx, body).map_err(CommandError::failed)?;
        return Ok(json!({
            "holes": made.holes,
            "left": made.left,
            "features": made.features.iter().map(|f| f.0.to_string()).collect::<Vec<_>>(),
        }));
    }
    if id == "design.move_to_body" {
        return move_to_body(&a, ctx);
    }
    if id == "design.duplicate" {
        let feature = FeatureId(a.id("feature")?);
        let body = a.opt_id("body")?.map(BodyId);
        return duplicate(ctx, feature, body)
            .map(|made| Value::from(made.iter().map(|f| f.0.to_string()).collect::<Vec<_>>()))
            .map_err(CommandError::failed);
    }
    if !FEATURES.iter().any(|(f, _)| *f == id) {
        return Err(CommandError::Unknown(id.to_string()));
    }
    let mut sketch = a.opt_id("sketch")?.map(FeatureId);
    // A loft's sketch is its first section: given only sections, the first
    // is that sketch.
    let lofting = matches!(id, "design.loft" | "design.subtractive_loft");
    let listed: Vec<FeatureId> = match args.get("sections") {
        Some(Value::Array(list)) if lofting => list
            .iter()
            .filter_map(|v| v.as_str().and_then(|s| uuid::Uuid::parse_str(s).ok()))
            .map(FeatureId)
            .collect(),
        _ => Vec::new(),
    };
    if sketch.is_none() {
        sketch = listed.first().copied();
    }
    // A borrowed face as the profile: its borrow stands selected, as the
    // toolbar reads it.
    if let Some(borrow) = args
        .get("profile_borrowed")
        .and_then(|r| r.get("borrow"))
        .and_then(Value::as_str)
        .and_then(|s| uuid::Uuid::parse_str(s).ok())
    {
        ctx.active_document_object = Some(FeatureId(borrow));
    }
    if let Some(sketch) = sketch {
        let node = ctx
            .document
            .get_feature_meta(sketch)
            .ok_or_else(|| CommandError::bad("sketch", "is not a feature of this document"))?;
        if node.workbench_id.as_str() != "wb.sketch"
            && !crate::borrow::lends_sketch(ctx.document, sketch)
        {
            return Err(CommandError::bad("sketch", "is not a sketch"));
        }
        // The toolbar's path reads the sketch from the selection.
        ctx.active_document_object = Some(sketch);
    }
    let body = match a.opt_id("body")? {
        Some(id) => {
            let body = BodyId(id);
            if !ctx.document.bodies().iter().any(|b| b.id == body) {
                return Err(CommandError::bad("body", "is not a body of this document"));
            }
            body
        }
        None => sketch
            .and_then(|s| ctx.document.get_feature_meta(s).and_then(|n| n.body))
            .or_else(|| ctx.selected_body_id.map(BodyId))
            .ok_or_else(|| CommandError::bad("body", "is required when there is no sketch"))?,
    };
    // A face given here, in the body's own frame, stands in for one picked
    // in the viewport: placed in world space, where the toolbar's path
    // reads it.
    if a.has("face_point") {
        let placement = ctx.document.body_placement(body);
        let point = vector3(a.0.get("face_point"), "face_point")?;
        let normal = vector3(a.0.get("face_normal"), "face_normal")?;
        ctx.selected_face = Some(core_document::FaceRef {
            name: 0,
            point: placement.point(point),
            normal: placement.direction(normal),
            surface: None,
        });
    }
    // A thickness opens a face, a draft turns about one, and a deletion,
    // offset or move works on one: in a script that face is an argument.
    if matches!(
        id,
        "design.thickness"
            | "design.draft"
            | "design.delete_faces"
            | "design.offset_faces"
            | "design.move_faces"
    ) && ctx.selected_face.is_none()
    {
        return Err(CommandError::bad(
            "face_point",
            "is required with face_normal: a point on the face (the one to open or to \
             delete, or the neutral plane) and its outward normal, in the body's own frame",
        ));
    }
    let tool = match a.opt_string("variant")? {
        Some(variant) => format!("{id}:{variant}"),
        None => id.to_string(),
    };
    let mut fields: Map<String, Value> = args
        .iter()
        .filter(|(k, _)| !OWN_ARGS.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if lofting
        && let (Some(sketch), Some(Value::Array(list))) = (sketch, fields.get_mut("sections"))
        && !listed.contains(&sketch)
    {
        list.insert(0, json!(sketch.0.to_string()));
    }
    let made = bench
        .create_feature(ctx, &tool, body, |feature| apply_fields(feature, &fields))
        .map_err(CommandError::failed)?;
    if let Some(name) = a.opt_string("name")? {
        ctx.document.rename_feature(made.id, name);
    }
    ctx.active_document_object = Some(made.id);
    Ok(json!(made.id.0.to_string()))
}

fn set(a: &Args, args: &CommandArgs, ctx: &mut WorkbenchRuntimeContext) -> CommandResult {
    let id = FeatureId(a.id("feature")?);
    let not_ours = || CommandError::bad("feature", "is not a Design feature or a datum");
    let data = ctx.document.get_feature_data(id).ok_or_else(not_ours)?;
    let fields: Map<String, Value> = args
        .iter()
        .filter(|(k, _)| k.as_str() != "feature")
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let data = if let Ok(mut feature) = DesignFeature::from_json(data) {
        apply_fields(&mut feature, &fields).map_err(CommandError::failed)?;
        feature.to_json()
    } else if let Ok(datum) = DatumFeature::from_json(data) {
        let fields = datum_fields(&datum, fields)?;
        let mut value = datum.to_json();
        merge_fields("Datum", &mut value, &fields).map_err(CommandError::failed)?;
        DatumFeature::from_json(&value)
            .map_err(|e| CommandError::failed(format!("Datum: {e}")))?
            .to_json()
    } else {
        return Err(not_ours());
    };
    ctx.document
        .update_feature_data(id, data)
        .map_err(|e| CommandError::failed(e.to_string()))?;
    ctx.document.mark_feature_dirty(id);
    Ok(Value::Null)
}

/// A tilt, {x, y} degrees, where one is given.
fn tilt_of(value: Option<&Value>) -> Result<Option<[f32; 2]>, CommandError> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_array()
            .filter(|p| p.len() == 2)
            .and_then(|p| Some([p[0].as_f64()? as f32, p[1].as_f64()? as f32]))
            .map(Some)
            .ok_or_else(|| CommandError::bad("tilt", "must be {x, y} degrees")),
    }
}

/// A datum's fields as `design.set` takes them: `offset` as `design.datum`
/// gives it, {x, y, z}, with `rotation` and `flip` beside it, or whole as
/// the datum keeps it, {translation, rotation_deg, flip}.
fn datum_fields(
    datum: &DatumFeature,
    mut fields: Map<String, Value>,
) -> Result<Map<String, Value>, CommandError> {
    let mut offset = datum.offset;
    let mut moved = false;
    if let Some(given) = fields.get("offset")
        && given.get("translation").is_none()
    {
        offset.translation = vector3(Some(given), "offset")?;
        moved = true;
    }
    if let Some(rotation) = fields.remove("rotation") {
        offset.rotation_deg = rotation
            .as_f64()
            .ok_or_else(|| CommandError::bad("rotation", "must be a number"))?
            as f32;
        moved = true;
    }
    if let Some(flip) = fields.remove("flip") {
        offset.flip = flip
            .as_bool()
            .ok_or_else(|| CommandError::bad("flip", "must be true or false"))?;
        moved = true;
    }
    if let Some(tilt) = tilt_of(fields.remove("tilt").as_ref())? {
        offset.tilt = tilt;
        moved = true;
    }
    if moved {
        fields.insert("offset".into(), json!(offset));
    }
    Ok(fields)
}

fn datum(a: &Args, ctx: &mut WorkbenchRuntimeContext) -> CommandResult {
    let body = BodyId(a.id("body")?);
    if !ctx.document.bodies().iter().any(|b| b.id == body) {
        return Err(CommandError::bad("body", "is not a body of this document"));
    }
    let size = a.opt_number("size")?.map(|s| s as f32);
    let shape = match a.string("kind")? {
        "plane" => DatumShape::Plane {
            size: size.unwrap_or(30.0),
        },
        "line" => DatumShape::Line {
            length: size.unwrap_or(40.0),
        },
        "point" => DatumShape::Point,
        "coordinate_system" => DatumShape::CoordinateSystem {
            size: size.unwrap_or(20.0),
        },
        _ => {
            return Err(CommandError::bad(
                "kind",
                "must be plane, line, point or coordinate_system",
            ));
        }
    };
    let attachment = crate::datum_refs::attachment_from_args(a, ctx, body)?;
    let offset = AttachmentOffset {
        tilt: tilt_of(a.0.get("tilt"))?.unwrap_or_default(),
        translation: match a.0.get("offset") {
            Some(v) if !v.is_null() => vector3(Some(v), "offset")?,
            _ => [0.0; 3],
        },
        rotation_deg: a.opt_number("rotation")?.unwrap_or(0.0) as f32,
        flip: a.opt_bool("flip")?.unwrap_or(false),
    };
    let name = a
        .opt_string("name")?
        .map(str::to_string)
        .unwrap_or_else(|| DesignWorkbench::next_feature_name(ctx, shape.label()));
    let mut datum = DatumFeature {
        shape,
        attachment,
        offset,
    };
    crate::datum_refs::settle(ctx, body, &mut datum).map_err(CommandError::failed)?;
    let id = ctx
        .document
        .add_feature_in_body(datum, name, Some(body))
        .map_err(|e| CommandError::failed(e.to_string()))?;
    Ok(json!(id.0.to_string()))
}

/// `design.borrow`: a borrow of a sketch, or of faces and edges, added to a
/// body as a feature of its history.
fn borrow(a: &Args, ctx: &mut WorkbenchRuntimeContext) -> CommandResult {
    use crate::feature::{BorrowSource, EdgePick, FacePick};
    let body = BodyId(a.id("body")?);
    if !ctx.document.bodies().iter().any(|b| b.id == body) {
        return Err(CommandError::bad("body", "is not a body of this document"));
    }
    // An imported solid takes a base shape first, and builds on it.
    if ctx.document.body_solid_is_imported(body) && crate::take_base(ctx, body).is_none() {
        return Err(CommandError::bad(
            "body",
            "takes its shape from elsewhere (a mesh, a copy or a linked part) and takes no \
             features; borrow into a body of its own",
        ));
    }
    let source = match (a.opt_id("sketch")?, a.opt_id("from")?) {
        (Some(sketch), None) => BorrowSource::Sketch(FeatureId(sketch)),
        (None, Some(from)) => {
            let list = |name: &str| match a.0.get(name) {
                None | Some(Value::Null) => Ok(Vec::new()),
                Some(Value::Array(items)) => Ok(items.clone()),
                // A script's empty table.
                Some(Value::Object(map)) if map.is_empty() => Ok(Vec::new()),
                Some(_) => Err(CommandError::bad(name, "must be a list")),
            };
            let faces = list("faces")?
                .iter()
                .map(|f| {
                    Ok(FacePick {
                        name: 0,
                        point: vector3(f.get("point"), "faces")?,
                        normal: vector3(f.get("normal"), "faces")?,
                    })
                })
                .collect::<Result<Vec<_>, CommandError>>()?;
            let edges = list("edges")?
                .iter()
                .map(|e| {
                    Ok(EdgePick {
                        faces: [0, 0],
                        point: vector3(e.get("point"), "edges")?,
                        direction: vector3(e.get("direction"), "edges")?,
                    })
                })
                .collect::<Result<Vec<_>, CommandError>>()?;
            BorrowSource::Solid {
                body: BodyId(from),
                faces,
                edges,
            }
        }
        _ => {
            return Err(CommandError::bad(
                "sketch",
                "give a sketch, or from with the faces and edges it lends, not both",
            ));
        }
    };
    if let Some(why) = crate::borrow::refusal(ctx.document, body, &source) {
        let arg = if matches!(source, BorrowSource::Sketch(_)) {
            "sketch"
        } else {
            "from"
        };
        return Err(CommandError::bad(arg, why));
    }
    let options: crate::feature::BorrowOptions = match a.0.get("options") {
        None | Some(Value::Null) => Default::default(),
        Some(v) => serde_json::from_value(v.clone())
            .map_err(|e| CommandError::bad("options", e.to_string()))?,
    };
    if (options.fill || options.whole) && matches!(source, BorrowSource::Sketch(_)) {
        return Err(CommandError::bad(
            "options",
            "fill and whole take from a solid, not a sketch",
        ));
    }
    let frozen = if a.opt_bool("frozen")?.unwrap_or(false) {
        Some(
            crate::borrow::freeze(ctx.document, ctx.kernel, body, &source, &options)
                .map_err(CommandError::failed)?,
        )
    } else {
        None
    };
    let name = a
        .opt_string("name")?
        .map(str::to_string)
        .unwrap_or_else(|| DesignWorkbench::next_feature_name(ctx, "Borrowed"));
    let feature = DesignFeature::Borrow {
        source,
        frozen,
        options,
    };
    let id = ctx
        .document
        .add_feature_in_body(feature, name, Some(body))
        .map_err(|e| CommandError::failed(e.to_string()))?;
    ctx.document.mark_feature_dirty(id);
    ctx.active_document_object = Some(id);
    Ok(json!(id.0.to_string()))
}

/// `design.freeze`: a borrow takes its source as it is now, or follows it
/// again.
fn freeze(a: &Args, ctx: &mut WorkbenchRuntimeContext) -> CommandResult {
    let id = FeatureId(a.id("feature")?);
    let not_a_borrow = || CommandError::bad("feature", "is not borrowed geometry");
    let body = ctx
        .document
        .get_feature_meta(id)
        .and_then(|n| n.body)
        .ok_or_else(not_a_borrow)?;
    let Some(DesignFeature::Borrow {
        source, options, ..
    }) = ctx
        .document
        .get_feature_data(id)
        .and_then(|d| DesignFeature::from_json(d).ok())
    else {
        return Err(not_a_borrow());
    };
    let frozen = if a.opt_bool("frozen")?.unwrap_or(true) {
        Some(
            crate::borrow::freeze(ctx.document, ctx.kernel, body, &source, &options)
                .map_err(CommandError::failed)?,
        )
    } else {
        None
    };
    let feature = DesignFeature::Borrow {
        source,
        frozen,
        options,
    };
    ctx.document
        .update_feature_data(id, feature.to_json())
        .map_err(|e| CommandError::failed(e.to_string()))?;
    ctx.document
        .set_feature_dependencies(id, feature.dependencies());
    ctx.document.mark_feature_dirty(id);
    Ok(Value::Null)
}

fn move_to_body(a: &Args, ctx: &mut WorkbenchRuntimeContext) -> CommandResult {
    let id = FeatureId(a.id("feature")?);
    let body = BodyId(a.id("body")?);
    move_feature(ctx, id, body)
        .map(|moved| Value::from(moved.iter().map(|f| f.0.to_string()).collect::<Vec<_>>()))
        .map_err(CommandError::failed)
}

/// Copy `id` into `body`, its own when `None`.
pub(crate) fn duplicate(
    ctx: &mut WorkbenchRuntimeContext,
    id: FeatureId,
    body: Option<BodyId>,
) -> Result<Vec<FeatureId>, String> {
    let copied = crate::clipboard::Clipboard::copy(ctx.document, id)
        .ok_or("the feature belongs to no body")?;
    let body = match body {
        Some(body) if ctx.document.bodies().iter().any(|b| b.id == body) => body,
        Some(_) => return Err("no such body".into()),
        None => ctx
            .document
            .get_feature_meta(id)
            .and_then(|n| n.body)
            .ok_or("the feature belongs to no body")?,
    };
    Ok(copied.paste(ctx.document, body))
}

/// Move `id` into `body` and rebuild both bodies.
pub(crate) fn move_feature(
    ctx: &mut WorkbenchRuntimeContext,
    id: FeatureId,
    body: BodyId,
) -> Result<Vec<FeatureId>, String> {
    let from = ctx
        .document
        .get_feature_meta(id)
        .and_then(|n| n.body)
        .ok_or("the feature belongs to no body")?;
    let moved = ctx.document.move_feature_to_body(id, body)?;
    crate::invalidate_body(ctx.document, from);
    crate::invalidate_body(ctx.document, body);
    Ok(moved)
}

pub(crate) fn vector3(value: Option<&Value>, name: &str) -> Result<[f32; 3], CommandError> {
    let bad = || CommandError::bad(name, "must be {x, y, z}");
    let v = match value {
        Some(Value::Array(v)) if v.len() == 3 => [v[0].as_f64(), v[1].as_f64(), v[2].as_f64()],
        Some(Value::Object(m)) => ["x", "y", "z"].map(|k| m.get(k).and_then(Value::as_f64)),
        _ => return Err(bad()),
    };
    Ok([
        v[0].ok_or_else(bad)? as f32,
        v[1].ok_or_else(bad)? as f32,
        v[2].ok_or_else(bad)? as f32,
    ])
}

/// What a task accepted, as a recording says it: a feature a tool made as
/// the command that makes it, with the fields that differ from what that
/// command would make alone; a datum a tool made as `design.datum`; an edit
/// of an existing one as `design.set` with the fields it changed.
#[cfg(feature = "egui")]
pub(crate) fn record_task(
    bench: &DesignWorkbench,
    ctx: &mut WorkbenchRuntimeContext,
    task: &crate::task::TaskState,
) {
    let Some(node) = ctx.document.get_feature_meta(task.feature).cloned() else {
        return;
    };
    let id = json!(task.feature.0.to_string());
    if let Ok(DesignFeature::Borrow {
        source,
        frozen,
        options,
    }) = DesignFeature::from_json(&node.data)
    {
        record_borrow(ctx, task, &node.name, source, frozen.is_some(), options);
        return;
    }
    match (&task.made_by, &task.kind) {
        (Some((tool, body)), crate::task::TaskKind::Part) => {
            let command = core_document::base_tool_id(tool);
            if !FEATURES.iter().any(|(f, _)| *f == command) {
                return;
            }
            let fields = inner(&node.data);
            let sketch = fields
                .get("sketch")
                .and_then(Value::as_str)
                .and_then(|s| uuid::Uuid::parse_str(s).ok())
                .map(FeatureId);
            // A profile face goes as the face the command takes for one.
            let profile_face = fields
                .get("profile_face")
                .and_then(|v| serde_json::from_value::<crate::FacePick>(v.clone()).ok());
            let placement = ctx.document.body_placement(*body);
            let picked = profile_face.map(|face| core_document::FaceRef {
                name: face.name,
                point: placement.point(face.point),
                normal: placement.direction(face.normal),
                surface: None,
            });
            let default = default_feature(bench, ctx, tool, *body, sketch, picked);
            let mut args = Map::new();
            if let Some(sketch) = sketch {
                args.insert("sketch".into(), json!(sketch.0.to_string()));
            }
            if let Some(face) = profile_face {
                args.insert("face_point".into(), json!(face.point));
                args.insert("face_normal".into(), json!(face.normal));
            }
            args.insert("body".into(), json!(body.0.to_string()));
            args.insert("name".into(), json!(node.name));
            if let Some(variant) = core_document::tool_variant(tool) {
                args.insert("variant".into(), json!(variant));
            }
            let default_fields = default.as_ref().map(inner).unwrap_or_default();
            for (name, value) in fields {
                let own_arg = name == "sketch" || (name == "profile_face" && picked.is_some());
                if !own_arg && default_fields.get(&name) != Some(&value) {
                    args.insert(name, value);
                }
            }
            ctx.record(command, args, id);
        }
        (Some((_, body)), crate::task::TaskKind::Datum) => {
            let Ok(datum) = DatumFeature::from_json(&node.data) else {
                return;
            };
            let (kind, size) = match datum.shape {
                DatumShape::Plane { size } => ("plane", Some(size)),
                DatumShape::Line { length } => ("line", Some(length)),
                DatumShape::Point => ("point", None),
                DatumShape::CoordinateSystem { size } => ("coordinate_system", Some(size)),
            };
            let mut args = json!({
                "kind": kind,
                "body": body.0.to_string(),
                "name": node.name,
                "offset": datum.offset.translation,
                "rotation": datum.offset.rotation_deg,
                "flip": datum.offset.flip,
            });
            if datum.offset.tilt != [0.0; 2] {
                args["tilt"] = json!(datum.offset.tilt);
            }
            if let Some(size) = size {
                args["size"] = json!(size);
            }
            for (name, value) in crate::datum_refs::attachment_args(&datum.attachment) {
                args[name] = value;
            }
            ctx.record("design.datum", crate::commands::object(args), id);
        }
        (None, kind) => {
            let (before, after) = match kind {
                crate::task::TaskKind::Part => (inner(&task.snapshot), inner(&node.data)),
                crate::task::TaskKind::Datum => (
                    task.snapshot.as_object().cloned().unwrap_or_default(),
                    node.data.as_object().cloned().unwrap_or_default(),
                ),
            };
            let mut args = Map::new();
            args.insert("feature".into(), id.clone());
            for (name, value) in after {
                if before.get(&name) != Some(&value) {
                    args.insert(name, value);
                }
            }
            if args.len() > 1 {
                ctx.record("design.set", args, Value::Null);
            }
        }
    }
}

/// A borrow's task as a recording says it: made, as `design.borrow` with
/// what it borrows; edited, as `design.set` of its source and `design.freeze`
/// when it was frozen, taken again or let go.
#[cfg(feature = "egui")]
fn record_borrow(
    ctx: &mut WorkbenchRuntimeContext,
    task: &crate::task::TaskState,
    name: &str,
    source: crate::feature::BorrowSource,
    frozen: bool,
    options: crate::feature::BorrowOptions,
) {
    use crate::feature::BorrowSource;
    let id = json!(task.feature.0.to_string());
    if let Some((_, body)) = &task.made_by {
        let mut args = Map::new();
        args.insert("body".into(), json!(body.0.to_string()));
        args.insert("name".into(), json!(name));
        match &source {
            BorrowSource::Sketch(sketch) => {
                args.insert("sketch".into(), json!(sketch.0.to_string()));
            }
            BorrowSource::Solid { body, faces, edges } => {
                args.insert("from".into(), json!(body.0.to_string()));
                args.insert("faces".into(), json!(faces));
                args.insert("edges".into(), json!(edges));
            }
        }
        if frozen {
            args.insert("frozen".into(), json!(true));
        }
        if !options.is_plain() {
            args.insert("options".into(), json!(options));
        }
        ctx.record("design.borrow", args, id);
        return;
    }
    let Ok(DesignFeature::Borrow {
        source: source_before,
        frozen: frozen_before,
        options: options_before,
    }) = DesignFeature::from_json(&task.snapshot)
    else {
        return;
    };
    if source_before != source || options_before != options {
        let mut args = Map::new();
        args.insert("feature".into(), id.clone());
        if source_before != source {
            args.insert("source".into(), json!(source));
        }
        if options_before != options {
            args.insert("options".into(), json!(options));
        }
        ctx.record("design.set", args, Value::Null);
    }
    let now = ctx.document.get_feature_data(task.feature).and_then(|d| {
        match DesignFeature::from_json(d).ok()? {
            DesignFeature::Borrow { frozen, .. } => Some(frozen),
            _ => None,
        }
    });
    if now.is_some_and(|now| now != frozen_before) {
        let mut args = Map::new();
        args.insert("feature".into(), id);
        args.insert("frozen".into(), json!(frozen));
        ctx.record("design.freeze", args, Value::Null);
    }
}

/// The fields of a feature's JSON, inside its kind: `{"Pad": {...}}` gives
/// the `{...}`.
fn inner(value: &Value) -> Map<String, Value> {
    value
        .as_object()
        .and_then(|m| m.values().next())
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

/// A JSON object as named arguments.
pub(crate) fn object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

/// The feature `tool` makes for `body` from `sketch` (or the world-space
/// `face` picked) alone, nothing else selected: what the command makes
/// before any field is named.
#[cfg(feature = "egui")]
fn default_feature(
    bench: &DesignWorkbench,
    ctx: &mut WorkbenchRuntimeContext,
    tool: &str,
    body: BodyId,
    sketch: Option<FeatureId>,
    face: Option<core_document::FaceRef>,
) -> Option<Value> {
    let saved = (
        ctx.active_document_object,
        std::mem::replace(&mut ctx.selected_face, face),
        std::mem::take(&mut ctx.selected_edges),
        ctx.selected_body_id.take(),
    );
    ctx.active_document_object = sketch;
    let made = DesignWorkbench::feature_for_tool(
        core_document::base_tool_id(tool),
        core_document::tool_variant(tool),
        ctx,
        body,
    );
    ctx.active_document_object = saved.0;
    ctx.selected_face = saved.1;
    ctx.selected_edges = saved.2;
    ctx.selected_body_id = saved.3;
    made.ok().map(|(mut feature, _)| {
        feature.set_refine(bench.options.refine_result);
        feature.to_json()
    })
}

/// Replace the named fields of the JSON object `value`, refusing a name it
/// does not have.
fn merge_fields(kind: &str, value: &mut Value, fields: &Map<String, Value>) -> Result<(), String> {
    let Value::Object(own) = value else {
        return Err(format!("{kind} has no fields to set"));
    };
    for (name, field) in fields {
        if !own.contains_key(name) {
            let mut known: Vec<&str> = own.keys().map(String::as_str).collect();
            known.sort_unstable();
            return Err(format!(
                "{kind} has no field `{name}`; it has {}",
                known.join(", ")
            ));
        }
        own.insert(name.clone(), as_field(&own[name], field));
    }
    Ok(())
}

/// `given` for a field holding `current`: an empty table set on a list is
/// the empty list, since a script's `{}` cannot say which it means.
fn as_field(current: &Value, given: &Value) -> Value {
    match (current, given) {
        (Value::Array(_), Value::Object(map)) if map.is_empty() => Value::Array(Vec::new()),
        _ => given.clone(),
    }
}

/// A feature from its fields as a script gives them, which may say a
/// thing more than one way: `{}` comes as an empty list where a table is
/// wanted, and a datum is `{Datum = id}` or `{Datum = {datum = id}}`
/// wherever a feature takes one. Each way is tried; the first error is the
/// one told.
fn read_fields(value: Value) -> Result<DesignFeature, serde_json::Error> {
    let first = match serde_json::from_value(value.clone()) {
        Ok(read) => return Ok(read),
        Err(first) => first,
    };
    let mut tables = value;
    core_document::command::empty_lists_as_tables(&mut tables);
    if let Ok(read) = serde_json::from_value(tables.clone()) {
        return Ok(read);
    }
    for to_table in [true, false] {
        let mut again = tables.clone();
        datum_forms(&mut again, to_table);
        if let Ok(read) = serde_json::from_value(again) {
            return Ok(read);
        }
    }
    Err(first)
}

/// Every `{Datum = ...}` in `value` said the other way: an id alone as
/// `{datum = id}` (`to_table`), or `{datum = id}` alone as the id.
fn datum_forms(value: &mut Value, to_table: bool) {
    match value {
        Value::Object(map) => {
            if map.len() == 1
                && let Some(datum) = map.get_mut("Datum")
            {
                match datum {
                    Value::String(id) if to_table => {
                        *datum = json!({"datum": id.clone()});
                        return;
                    }
                    Value::Object(inner) if !to_table && inner.keys().all(|k| k == "datum") => {
                        if let Some(id) = inner.get("datum").cloned() {
                            *datum = id;
                        }
                        return;
                    }
                    _ => {}
                }
            }
            map.values_mut().for_each(|v| datum_forms(v, to_table));
        }
        Value::Array(items) => items.iter_mut().for_each(|v| datum_forms(v, to_table)),
        _ => {}
    }
}

/// Replace fields of `feature` with `fields`, refusing a name the feature
/// does not have or a value of the wrong kind.
fn apply_fields(feature: &mut DesignFeature, fields: &Map<String, Value>) -> Result<(), String> {
    if fields.is_empty() {
        return Ok(());
    }
    let mut value = serde_json::to_value(&*feature).map_err(|e| e.to_string())?;
    let Some((kind, Value::Object(own))) = value.as_object_mut().and_then(|m| m.iter_mut().next())
    else {
        return Err("this feature has no fields to set".to_string());
    };
    for (name, field) in fields {
        if !own.contains_key(name) {
            let mut known: Vec<&str> = own.keys().map(String::as_str).collect();
            known.sort_unstable();
            return Err(format!(
                "{kind} has no field `{name}`; it has {}",
                known.join(", ")
            ));
        }
        let value = as_field(&own[name], field);
        own.insert(name.clone(), value);
    }
    let kind = kind.clone();
    *feature = read_fields(value).map_err(|first| format!("{kind}: {first}"))?;
    // A Pocket's flag and its ThroughAll mode are one setting: a flag given
    // moves the mode (true to ThroughAll, false back to a plain depth), and
    // the flag then reads what the mode is.
    if let DesignFeature::Pocket {
        through_all, mode, ..
    } = feature
    {
        use crate::feature::ExtrudeMode;
        match fields.get("through_all").and_then(Value::as_bool) {
            Some(true) => *mode = ExtrudeMode::ThroughAll,
            Some(false) if *mode == ExtrudeMode::ThroughAll => *mode = ExtrudeMode::Dimension,
            _ => {}
        }
        *through_all = *mode == ExtrudeMode::ThroughAll;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_document::{Document, Workbench};

    fn call(
        bench: &mut DesignWorkbench,
        doc: &mut Document,
        id: &str,
        args: Value,
    ) -> CommandResult {
        let mut ctx = WorkbenchRuntimeContext::new(doc, [0.0; 3], [0.0; 3], (0, 0, 1, 1));
        bench.run_command(id, args.as_object().unwrap(), &mut ctx)
    }

    fn sketch_in(doc: &mut Document) -> (BodyId, FeatureId) {
        let body = doc.create_body(None);
        let sketch = doc
            .add_feature_in_body(
                wb_sketch::SketchFeature::new(
                    wb_sketch::sketch::Sketch::new("s"),
                    wb_sketch::sketch::SketchPlane::default(),
                ),
                "sketch".to_string(),
                Some(body),
            )
            .unwrap();
        (body, sketch)
    }

    /// A loft's sketch is its first section: given with more sections it
    /// goes before them, and sections alone take the first as the sketch.
    #[test]
    fn a_loft_s_sketch_leads_its_sections() {
        let mut doc = Document::new("t");
        let (_, a) = sketch_in(&mut doc);
        let body = doc.get_feature_meta(a).unwrap().body.unwrap();
        let b = doc
            .add_feature_in_body(
                wb_sketch::SketchFeature::new(
                    wb_sketch::sketch::Sketch::new("b"),
                    wb_sketch::sketch::SketchPlane::default(),
                ),
                "b".to_string(),
                Some(body),
            )
            .unwrap();
        let mut bench = DesignWorkbench::default();
        let id = |f: FeatureId| json!(f.0.to_string());
        for args in [
            json!({"sketch": id(a), "sections": [id(b)]}),
            json!({"sections": [id(a), id(b)]}),
            json!({"sketch": id(a), "sections": [id(a), id(b)]}),
        ] {
            let made = call(&mut bench, &mut doc, "design.loft", args.clone()).unwrap();
            let sections = fields(&doc, &made)["Loft"]["sections"].clone();
            assert_eq!(sections, json!([id(a), id(b)]), "{args}");
        }
    }

    /// A script names a datum either way, and writes `{}` for an empty
    /// table: the fields read all the same.
    #[test]
    fn fields_read_whichever_way_a_script_writes_them() {
        let datum = FeatureId::new();
        let id = datum.0.to_string();
        let mut revolve = DesignFeature::Revolution {
            sketch: FeatureId::new(),
            angle_deg: 360.0,
            axis: crate::feature::RevolveAxis::default(),
            reversed: false,
            midplane: false,
            second_angle_deg: None,
            refine: false,
            mode: Default::default(),
            up_to_face: None,
        };
        for axis in [json!({"Datum": id}), json!({"Datum": {"datum": id}})] {
            apply_fields(&mut revolve, json!({"axis": axis}).as_object().unwrap()).unwrap();
            assert!(matches!(
                &revolve,
                DesignFeature::Revolution { axis: crate::feature::RevolveAxis::Datum(d), .. } if *d == datum
            ));
        }
        // A mirror's plane is a table: a bare id reads as one too.
        let mut value = json!({"Mirrored": {"originals": [], "plane": {"Datum": id}}});
        datum_forms(&mut value, true);
        assert_eq!(value["Mirrored"]["plane"], json!({"Datum": {"datum": id}}));
        // `{Angled = {}}`: the drill's usual point.
        let point: crate::feature::DrillPoint = {
            let mut v = json!({"Angled": []});
            core_document::command::empty_lists_as_tables(&mut v);
            serde_json::from_value(v).unwrap()
        };
        assert_eq!(
            point,
            crate::feature::DrillPoint::Angled { angle_deg: 118.0 }
        );
    }

    /// A thread is a size alone, or whole; a wrong one says what is taken.
    #[test]
    fn a_thread_reads_as_a_size_or_whole_and_says_what_it_takes() {
        use crate::feature::ThreadSpec;
        use crate::hole_tables::ThreadStandard;
        let m6: ThreadSpec = serde_json::from_value(json!("M6")).unwrap();
        assert_eq!(m6.standard, ThreadStandard::IsoMetricCoarse);
        let unc: ThreadSpec = serde_json::from_value(json!("1/4-20")).unwrap();
        assert_eq!(unc.standard, ThreadStandard::Unc);
        let whole: ThreadSpec =
            serde_json::from_value(json!({"standard": "Unf", "size": "1/4-28"})).unwrap();
        assert_eq!(whole.size, "1/4-28");
        let old: ThreadSpec = serde_json::from_value(json!(5)).unwrap();
        assert_eq!(old.size, "M6");
        let wrong = serde_json::from_value::<ThreadSpec>(json!("M7.3")).unwrap_err();
        assert!(wrong.to_string().contains("IsoMetricCoarse"), "{wrong}");
    }

    fn fields(doc: &Document, id: &Value) -> Value {
        let id = FeatureId(uuid::Uuid::parse_str(id.as_str().unwrap()).unwrap());
        doc.get_feature_data(id).unwrap().clone()
    }

    #[test]
    fn a_pad_takes_its_sketch_and_the_fields_named() {
        let mut doc = Document::new("t");
        let (body, sketch) = sketch_in(&mut doc);
        let mut bench = DesignWorkbench::default();
        let pad = call(
            &mut bench,
            &mut doc,
            "design.pad",
            json!({"sketch": sketch.0.to_string(), "length": 25.0, "name": "Base"}),
        )
        .unwrap();
        let data = fields(&doc, &pad);
        assert_eq!(data["Pad"]["length"], json!(25.0));
        assert_eq!(data["Pad"]["sketch"], json!(sketch.0.to_string()));
        let id = FeatureId(uuid::Uuid::parse_str(pad.as_str().unwrap()).unwrap());
        let node = doc.get_feature_meta(id).unwrap();
        assert_eq!(node.name, "Base");
        assert_eq!(node.body, Some(body));

        call(
            &mut bench,
            &mut doc,
            "design.set",
            json!({"feature": pad, "length": 40.0, "reversed": true}),
        )
        .unwrap();
        let data = fields(&doc, &pad);
        assert_eq!(data["Pad"]["length"], json!(40.0));
        assert_eq!(data["Pad"]["reversed"], json!(true));
    }

    /// A pad moved to another body takes its sketch along, and the tree
    /// offers the move for each other body.
    #[test]
    fn a_pad_moves_to_another_body_with_its_sketch() {
        let mut doc = Document::new("t");
        let (from, sketch) = sketch_in(&mut doc);
        let mut bench = DesignWorkbench::default();
        let pad = call(
            &mut bench,
            &mut doc,
            "design.pad",
            json!({"sketch": sketch.0.to_string()}),
        )
        .unwrap();
        let pad_id = FeatureId(uuid::Uuid::parse_str(pad.as_str().unwrap()).unwrap());
        let to = doc.create_body(Some("Other".into()));
        let items = bench.menu_items(&core_document::MenuScope::TreeFeature(pad_id), &doc);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(labels, ["Duplicate", "Move to Other"]);

        let moved = call(
            &mut bench,
            &mut doc,
            "design.move_to_body",
            json!({"feature": pad, "body": to.0.to_string()}),
        )
        .unwrap();
        assert_eq!(moved, json!([sketch.0.to_string(), pad]));
        assert_eq!(doc.get_feature_meta(pad_id).unwrap().body, Some(to));
        assert_eq!(doc.get_feature_meta(sketch).unwrap().body, Some(to));
        assert!(crate::design_feature_ids(&doc, from).is_empty());
    }

    /// A duplicate reads a copy of its sketch, named as a new feature is,
    /// and the original is untouched.
    #[test]
    fn a_duplicated_pad_reads_its_own_copy_of_the_sketch() {
        let mut doc = Document::new("t");
        let (body, sketch) = sketch_in(&mut doc);
        let mut bench = DesignWorkbench::default();
        let pad = call(
            &mut bench,
            &mut doc,
            "design.pad",
            json!({"sketch": sketch.0.to_string(), "length": 7.0, "name": "Pad"}),
        )
        .unwrap();
        let made = call(
            &mut bench,
            &mut doc,
            "design.duplicate",
            json!({"feature": pad}),
        )
        .unwrap();
        let made: Vec<FeatureId> = made
            .as_array()
            .unwrap()
            .iter()
            .map(|v| FeatureId(uuid::Uuid::parse_str(v.as_str().unwrap()).unwrap()))
            .collect();
        assert_eq!(made.len(), 2, "the sketch and the pad");
        let (new_sketch, new_pad) = (made[0], made[1]);
        assert_ne!(new_sketch, sketch);
        let data = doc.get_feature_data(new_pad).unwrap();
        assert_eq!(data["Pad"]["sketch"], json!(new_sketch.0.to_string()));
        assert_eq!(data["Pad"]["length"], json!(7.0));
        assert_eq!(doc.feature_tree().dependencies(new_pad), [new_sketch]);
        assert_eq!(doc.get_feature_meta(new_pad).unwrap().name, "Pad_1");
        assert_eq!(doc.get_feature_meta(new_pad).unwrap().body, Some(body));
        assert_eq!(
            doc.get_feature_meta(new_sketch).unwrap().visible,
            doc.get_feature_meta(sketch).unwrap().visible
        );
        assert_eq!(
            fields(&doc, &pad)["Pad"]["sketch"],
            json!(sketch.0.to_string())
        );

        // Cut and paste puts it back, in the body selected.
        let other = doc.create_body(None);
        let pad_id = FeatureId(uuid::Uuid::parse_str(pad.as_str().unwrap()).unwrap());
        let mut ctx = WorkbenchRuntimeContext::new(&mut doc, [0.0; 3], [0.0; 3], (0, 0, 1, 1));
        ctx.active_document_object = Some(pad_id);
        assert!(bench.on_command("edit.cut", &core_document::MenuScope::EditMenu, &mut ctx));
        ctx.active_document_object = None;
        ctx.selected_body_id = Some(other.0);
        assert!(bench.on_command("edit.paste", &core_document::MenuScope::EditMenu, &mut ctx));
        let pasted = ctx.active_document_object.expect("the paste is selected");
        drop(ctx);
        assert!(doc.get_feature_meta(pad_id).is_none(), "cut");
        assert!(doc.get_feature_meta(sketch).is_none(), "with its sketch");
        assert_eq!(doc.get_feature_meta(pasted).unwrap().body, Some(other));
    }

    /// A boolean starts with the body made last, the one just built to
    /// combine, as its tool.
    #[test]
    fn a_boolean_starts_on_the_latest_other_body() {
        let mut doc = Document::new("t");
        let (body, sketch) = sketch_in(&mut doc);
        let mut bench = DesignWorkbench::default();
        call(
            &mut bench,
            &mut doc,
            "design.pad",
            json!({"sketch": sketch.0.to_string()}),
        )
        .unwrap();
        doc.create_body(None);
        let latest = doc.create_body(None);
        let boolean = call(
            &mut bench,
            &mut doc,
            "design.boolean",
            json!({"body": body.0.to_string()}),
        )
        .unwrap();
        let data = fields(&doc, &boolean);
        assert_eq!(
            data["BodyBoolean"]["tool_body"],
            json!(latest.0.to_string())
        );
    }

    /// A datum moves by the same arguments whether it is made or set.
    #[test]
    fn a_datum_is_set_by_the_offset_it_was_made_with() {
        let mut doc = Document::new("t");
        let body = doc.create_body(None);
        let mut bench = DesignWorkbench::default();
        let datum = call(
            &mut bench,
            &mut doc,
            "design.datum",
            json!({"body": body.0.to_string(), "kind": "plane", "offset": [0, 0, 5]}),
        )
        .unwrap();
        call(
            &mut bench,
            &mut doc,
            "design.set",
            json!({"feature": datum, "offset": {"x": 1, "y": 2, "z": 8}, "rotation": 30}),
        )
        .unwrap();
        let offset = &fields(&doc, &datum)["offset"];
        assert_eq!(offset["translation"], json!([1.0, 2.0, 8.0]));
        assert_eq!(offset["rotation_deg"], json!(30.0));

        call(
            &mut bench,
            &mut doc,
            "design.set",
            json!({"feature": datum, "offset":
                {"translation": [0, 0, 3], "rotation_deg": 0, "flip": true}}),
        )
        .unwrap();
        let offset = &fields(&doc, &datum)["offset"];
        assert_eq!(offset["translation"], json!([0.0, 0.0, 3.0]));
        assert_eq!(offset["flip"], json!(true));
    }

    /// A script's `{}` cannot say it is a list; set on a list field, it is
    /// the empty one.
    #[test]
    fn an_empty_table_empties_a_list_field() {
        let mut doc = Document::new("t");
        let (body, sketch) = sketch_in(&mut doc);
        let mut bench = DesignWorkbench::default();
        call(
            &mut bench,
            &mut doc,
            "design.pad",
            json!({"sketch": sketch.0.to_string()}),
        )
        .unwrap();
        let mirror = call(
            &mut bench,
            &mut doc,
            "design.mirror",
            json!({"body": body.0.to_string(), "originals": {}}),
        )
        .unwrap();
        assert_eq!(fields(&doc, &mirror)["Mirrored"]["originals"], json!([]));
    }

    /// `through_all` and the ThroughAll mode are one setting: either
    /// spelling sets it, and each reads what the other says.
    #[test]
    fn a_pocket_through_all_reads_back_as_its_mode() {
        let mut doc = Document::new("t");
        let (_, sketch) = sketch_in(&mut doc);
        let mut bench = DesignWorkbench::default();
        call(
            &mut bench,
            &mut doc,
            "design.pad",
            json!({"sketch": sketch.0.to_string()}),
        )
        .unwrap();
        let pocket = call(
            &mut bench,
            &mut doc,
            "design.pocket",
            json!({"sketch": sketch.0.to_string(), "through_all": true}),
        )
        .unwrap();
        let data = fields(&doc, &pocket);
        assert_eq!(data["Pocket"]["mode"], json!("ThroughAll"));
        assert_eq!(data["Pocket"]["through_all"], json!(true));

        call(
            &mut bench,
            &mut doc,
            "design.set",
            json!({"feature": pocket, "through_all": false}),
        )
        .unwrap();
        let data = fields(&doc, &pocket);
        assert_eq!(data["Pocket"]["mode"], json!("Dimension"));
        assert_eq!(data["Pocket"]["through_all"], json!(false));
        call(
            &mut bench,
            &mut doc,
            "design.set",
            json!({"feature": pocket, "mode": "ThroughAll"}),
        )
        .unwrap();
        assert_eq!(fields(&doc, &pocket)["Pocket"]["through_all"], json!(true));

        call(
            &mut bench,
            &mut doc,
            "design.set",
            json!({"feature": pocket, "mode": "Dimension", "symmetric": true}),
        )
        .unwrap();
        let data = fields(&doc, &pocket);
        assert_eq!(data["Pocket"]["mode"], json!("Dimension"));
        assert_eq!(data["Pocket"]["symmetric"], json!(true));
    }

    /// Draft and thickness read their face from the viewport's pick; a
    /// script passes it instead, in the body's own frame, wherever the body
    /// has been moved to.
    #[test]
    fn a_script_gives_a_thickness_or_a_draft_its_face() {
        let mut doc = Document::new("t");
        let (body, sketch) = sketch_in(&mut doc);
        doc.set_body_placement(
            body,
            core_document::BodyPlacement {
                translation: [30.0, -5.0, 2.0],
                rotation: [0.0, 0.0, 0.35f32.sin(), 0.35f32.cos()],
            },
        );
        let mut bench = DesignWorkbench::default();
        call(
            &mut bench,
            &mut doc,
            "design.pad",
            json!({"sketch": sketch.0.to_string(), "length": 10.0}),
        )
        .unwrap();
        let face = json!({"face_point": [5.0, 2.5, 10.0], "face_normal": [0.0, 0.0, 1.0]});
        let without = call(
            &mut bench,
            &mut doc,
            "design.thickness",
            json!({"body": body.0.to_string()}),
        );
        assert!(without.is_err(), "no face, no thickness");
        let near = |pick: &Value, want: [f64; 3]| {
            let point: Vec<f64> = pick["point"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect();
            point.iter().zip(want).all(|(g, w)| (g - w).abs() < 1e-3)
        };
        let mut args = face.clone();
        args["body"] = json!(body.0.to_string());
        let made = call(&mut bench, &mut doc, "design.thickness", args.clone()).unwrap();
        let thickness = fields(&doc, &made);
        let opened = thickness["Thickness"]["faces"].as_array().expect("faces");
        assert_eq!(opened.len(), 1);
        assert!(near(&opened[0], [5.0, 2.5, 10.0]), "{opened:?}");
        // A draft's face is its neutral plane; the faces to tilt are a field.
        args["faces"] = json!([{"point": [10.0, 2.5, 5.0], "normal": [1.0, 0.0, 0.0]}]);
        let made = call(&mut bench, &mut doc, "design.draft", args).unwrap();
        let draft = fields(&doc, &made);
        assert!(
            near(&draft["Draft"]["neutral"], [5.0, 2.5, 10.0]),
            "{draft}"
        );
        assert!(
            near(&draft["Draft"]["faces"][0], [10.0, 2.5, 5.0]),
            "{draft}"
        );
    }

    /// One frame of the task panel, as the host runs it; what it recorded
    /// and the active object it left.
    #[cfg(feature = "egui")]
    fn task_frame(
        bench: &mut DesignWorkbench,
        doc: &mut Document,
        active: FeatureId,
        request: core_document::TaskRequest,
    ) -> Vec<core_document::Recorded> {
        let egui_ctx = egui::Context::default();
        ui_kit::apply_theme(&egui_ctx);
        let mut recorded = Vec::new();
        let mut output = egui_ctx.run_ui(egui::RawInput::default(), |ui| {
            let mut ctx = WorkbenchRuntimeContext::new(doc, [0.0; 3], [0.0; 3], (0, 0, 1, 1));
            ctx.active_document_object = Some(active);
            bench.ui_task_panel(ui, &mut ctx, request);
            recorded = core_document::HookOutcome::take(&mut ctx).recorded;
        });
        output.textures_delta.clear();
        recorded
    }

    #[cfg(feature = "egui")]
    #[test]
    fn a_pad_made_with_the_tool_records_as_the_command_that_makes_it() {
        let mut doc = Document::new("t");
        let (_, sketch) = sketch_in(&mut doc);
        let before = doc.clone();
        let mut bench = DesignWorkbench::default();
        let pad = {
            let mut ctx = WorkbenchRuntimeContext::new(&mut doc, [0.0; 3], [0.0; 3], (0, 0, 1, 1));
            ctx.active_document_object = Some(sketch);
            bench.on_input(
                &core_document::WorkbenchInputEvent::KeyPress {
                    key: core_document::KeyCode::A,
                },
                Some("design.pad"),
                &mut ctx,
            );
            ctx.active_document_object.unwrap()
        };
        task_frame(&mut bench, &mut doc, pad, Default::default());
        // The panel's length field, as a person types into it.
        let mut data = doc.get_feature_data(pad).unwrap().clone();
        data["Pad"]["length"] = json!(25.0);
        doc.update_feature_data(pad, data).unwrap();
        let recorded = task_frame(
            &mut bench,
            &mut doc,
            pad,
            core_document::TaskRequest {
                accept: true,
                cancel: false,
            },
        );
        assert_eq!(recorded.len(), 1, "{recorded:?}");
        let call = &recorded[0];
        assert_eq!(call.id, "design.pad");
        assert_eq!(call.args["length"], json!(25.0));
        assert!(
            !call.args.contains_key("reversed"),
            "only what differs from the command's own: {:?}",
            call.args
        );

        // The call makes the same pad on the document as it was.
        let mut replay = before;
        let made = call_on(&mut replay, &call.id, Value::Object(call.args.clone())).unwrap();
        let made = FeatureId(uuid::Uuid::parse_str(made.as_str().unwrap()).unwrap());
        assert_eq!(
            replay.get_feature_data(made),
            doc.get_feature_data(pad),
            "the same fields"
        );
        assert_eq!(
            replay.get_feature_meta(made).unwrap().name,
            doc.get_feature_meta(pad).unwrap().name
        );

        // Editing it again records the change alone.
        {
            let mut ctx = WorkbenchRuntimeContext::new(&mut doc, [0.0; 3], [0.0; 3], (0, 0, 1, 1));
            ctx.active_document_object = Some(pad);
            core_document::Workbench::edit_feature(&mut bench, &mut ctx, pad);
        }
        task_frame(&mut bench, &mut doc, pad, Default::default());
        let mut data = doc.get_feature_data(pad).unwrap().clone();
        data["Pad"]["reversed"] = json!(true);
        doc.update_feature_data(pad, data).unwrap();
        let edit = task_frame(
            &mut bench,
            &mut doc,
            pad,
            core_document::TaskRequest {
                accept: true,
                cancel: false,
            },
        );
        assert_eq!(edit.len(), 1);
        assert_eq!(edit[0].id, "design.set");
        assert_eq!(
            edit[0].args.len(),
            2,
            "the feature and the one field: {:?}",
            edit[0].args
        );
    }

    fn call_on(doc: &mut Document, id: &str, args: Value) -> CommandResult {
        let mut bench = DesignWorkbench::default();
        call(&mut bench, doc, id, args)
    }

    /// A pad with no sketch takes the face given as its profile, and a pad
    /// the tool made of a picked face records as the call that gives it.
    #[cfg(feature = "egui")]
    #[test]
    fn a_face_profile_pad_is_made_and_recorded_with_its_face() {
        let mut doc = Document::new("t");
        let (body, sketch) = sketch_in(&mut doc);
        let mut bench = DesignWorkbench::default();
        call(
            &mut bench,
            &mut doc,
            "design.pad",
            json!({"sketch": sketch.0.to_string(), "length": 10.0}),
        )
        .unwrap();
        let before = doc.clone();
        let made = call(
            &mut bench,
            &mut doc,
            "design.pad",
            json!({"body": body.0.to_string(), "face_point": [5.0, 2.5, 10.0],
                "face_normal": [0.0, 0.0, 1.0], "length": 4.0}),
        )
        .unwrap();
        let data = fields(&doc, &made);
        assert_eq!(data["Pad"]["sketch"], Value::Null);
        assert_eq!(
            data["Pad"]["profile_face"]["point"],
            json!([5.0, 2.5, 10.0])
        );

        let mut doc = before.clone();
        let pad = {
            let mut ctx = WorkbenchRuntimeContext::new(&mut doc, [0.0; 3], [0.0; 3], (0, 0, 1, 1));
            ctx.selected_body_id = Some(body.0);
            ctx.selected_face = Some(core_document::FaceRef {
                name: 0,
                point: [5.0, 2.5, 10.0],
                normal: [0.0, 0.0, 1.0],
                surface: None,
            });
            bench.on_input(
                &core_document::WorkbenchInputEvent::KeyPress {
                    key: core_document::KeyCode::A,
                },
                Some("design.pad"),
                &mut ctx,
            );
            ctx.active_document_object.unwrap()
        };
        task_frame(&mut bench, &mut doc, pad, Default::default());
        let recorded = task_frame(
            &mut bench,
            &mut doc,
            pad,
            core_document::TaskRequest {
                accept: true,
                cancel: false,
            },
        );
        assert_eq!(recorded.len(), 1, "{recorded:?}");
        let call = &recorded[0];
        assert_eq!(call.args["face_point"], json!([5.0, 2.5, 10.0]));
        assert!(!call.args.contains_key("profile_face"), "{:?}", call.args);
        let mut replay = before;
        let made = call_on(&mut replay, &call.id, Value::Object(call.args.clone())).unwrap();
        let made = FeatureId(uuid::Uuid::parse_str(made.as_str().unwrap()).unwrap());
        assert_eq!(replay.get_feature_data(made), doc.get_feature_data(pad));
    }

    #[test]
    fn an_unknown_field_or_a_wrong_kind_is_refused_and_says_what_there_is() {
        let mut doc = Document::new("t");
        let (_, sketch) = sketch_in(&mut doc);
        let mut bench = DesignWorkbench::default();
        let err = call(
            &mut bench,
            &mut doc,
            "design.pad",
            json!({"sketch": sketch.0.to_string(), "colour": "red"}),
        )
        .unwrap_err()
        .to_string();
        assert!(
            err.contains("no field `colour`") && err.contains("length"),
            "{err}"
        );
        let err = call(
            &mut bench,
            &mut doc,
            "design.pad",
            json!({"sketch": sketch.0.to_string(), "length": "long"}),
        );
        assert!(err.is_err());
        assert_eq!(
            doc.feature_tree().all_nodes().count(),
            1,
            "a refused command adds nothing"
        );
    }
}
