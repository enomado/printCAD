//! The Generators: an involute gear, a chain sprocket and a stepped shaft,
//! each a sketch made from a few numbers (`wb_sketch::generator`), ready
//! for a Pad (the gear, the sprocket) or a Revolution (the shaft).
//!
//! The tool puts the sketch in the selected body, on the picked face
//! centred where it was picked, else on a base plane (the shaft stands on
//! XZ, so it turns about Z), and opens it, where its panel sets the
//! numbers. `design.gear`, `design.sprocket` and `design.shaft` make one from a
//! script.

use core_document::{
    Args, BodyId, CommandArgs, CommandError, CommandResult, CommandSpec, FeatureId, HostRequest,
    InputResult, ParamKind, ToolDescriptor, ToolVariant, WorkbenchContext, WorkbenchId,
    WorkbenchRuntimeContext,
};
use serde_json::{Map, Value, json};
use wb_sketch::generator::{Generator, new_sketch};
use wb_sketch::sketch::SketchPlane;

use crate::PartDesignWorkbench;

/// The generators, by variant: (variant, label, icon, command).
const GENERATORS: &[(&str, &str, &str, &str)] = &[
    ("gear", "Involute gear", "involute-gear", "design.gear"),
    ("sprocket", "Sprocket", "sprocket", "design.sprocket"),
    ("shaft", "Shaft", "revolution", "design.shaft"),
];

/// The toolbar's Generators dropdown.
pub(crate) fn tool() -> ToolDescriptor {
    ToolDescriptor::new_action("design.generator", "Generators", Some("generators"))
        .icon("involute-gear")
        .variants(
            GENERATORS
                .iter()
                .map(|(id, label, icon, _)| ToolVariant::new(id, label, icon))
                .collect(),
        )
}

/// Register `design.gear`, `design.sprocket` and `design.shaft`.
pub(crate) fn register(context: &mut WorkbenchContext) {
    let fields = [
        (
            "design.gear",
            "Make an involute spur gear's profile, outer or internal (ring): a sketch to pad",
            "module, teeth, pressure_angle_deg, profile_shift, addendum and dedendum (in \
             modules), backlash, root_fillet (in modules), bore, internal (true for a ring), \
             rim (a ring's outside diameter)",
        ),
        (
            "design.sprocket",
            "Make a roller chain sprocket's profile (ISO 606 teeth): a sketch to pad",
            "pitch, roller (the roller's diameter), teeth, bore",
        ),
        (
            "design.shaft",
            "Make a stepped shaft's half section: a sketch to revolve about its vertical axis",
            "sections = {{length, diameter, chamfer, fillet}, ...}, start_chamfer, and \
             loads = {bearings = {a, b}, forces = {{at, force, angle_deg}, ...}, torque (N·m), \
             torque_from, torque_to, modulus (GPa)} for its stresses and deflection",
        ),
    ];
    for (id, summary, extra) in fields {
        context.register_command(
            CommandSpec::new(id, summary)
                .optional(
                    "body",
                    ParamKind::Id,
                    "The body it goes in; the selected one, else a new one",
                )
                .optional(
                    "plane",
                    ParamKind::String,
                    "The base plane it lies on: XY, XZ or YZ (a gear and a sprocket take XY, \
                     a shaft XZ)",
                )
                .optional(
                    "face_point",
                    ParamKind::List,
                    "Or a face it lies on, centred at this point of it, {x, y, z}, in the \
                     body's own frame",
                )
                .optional(
                    "face_normal",
                    ParamKind::List,
                    "With face_point: the face's outward normal, {x, y, z}",
                )
                .optional("name", ParamKind::String, "Its name in the tree")
                .extra_args(extra)
                .returns("the sketch's id"),
        );
    }
}

/// Whether `id` is one of the generator commands.
pub(crate) fn is_command(id: &str) -> bool {
    GENERATORS.iter().any(|(_, _, _, command)| *command == id)
}

/// The plane a generator lies on when no face is given.
fn base_plane(generator: &Generator) -> (SketchPlane, &'static str) {
    match generator {
        Generator::Shaft(_) => (SketchPlane::xz(), "XZ"),
        _ => (SketchPlane::xy(), "XY"),
    }
}

/// The plane of a face, centred on `point` of it.
fn face_plane(point: [f32; 3], normal: [f32; 3]) -> SketchPlane {
    let frame = SketchPlane::from_face(point, normal);
    SketchPlane::from_frame(point, frame.normal, frame.x_axis)
}

/// Add the sketch `generator` makes to `body` on `plane`.
fn add(
    ctx: &mut WorkbenchRuntimeContext,
    body: BodyId,
    plane: SketchPlane,
    generator: Generator,
    name: Option<&str>,
) -> Result<FeatureId, String> {
    let name = match name {
        Some(name) => name.to_string(),
        None => PartDesignWorkbench::next_feature_name(ctx, generator.base_name()),
    };
    let feature = new_sketch(generator, plane, &name)?;
    ctx.document
        .add_feature_in_body(feature, name.clone(), Some(body))
        .map_err(|e| format!("Failed to create {name}: {e}"))
}

/// The toolbar's path: make the generator `tool` names in the selected
/// body and open it for its numbers.
pub(crate) fn insert(ctx: &mut WorkbenchRuntimeContext, tool: &str) -> InputResult {
    let variant = core_document::tool_variant(tool).unwrap_or("gear");
    let Some((_, _, _, command)) = GENERATORS.iter().find(|(id, ..)| *id == variant) else {
        return InputResult::consumed();
    };
    let Some(generator) = Generator::named(variant) else {
        return InputResult::consumed();
    };
    let Some(body) = PartDesignWorkbench::target_body(ctx) else {
        ctx.log_warn("Select a body (or one of its features) first");
        return InputResult::consumed();
    };
    let face = ctx.selected_face_in(body);
    let (plane, mut args) = match face {
        Some(face) => (
            face_plane(face.point, face.normal),
            json!({
                "face_point": face.point,
                "face_normal": face.normal,
            }),
        ),
        None => {
            let (plane, name) = base_plane(&generator);
            (plane, json!({ "plane": name }))
        }
    };
    let label = generator.label();
    match add(ctx, body, plane, generator, None) {
        Ok(id) => {
            if let Value::Object(map) = &mut args {
                map.insert("body".into(), json!(body.0.to_string()));
            }
            ctx.record(*command, object(args), json!(id.0.to_string()));
            ctx.log_info(format!("Created {label}: set its numbers in the panel"));
            // The sketcher opens it: its panel holds the numbers.
            ctx.active_document_object = Some(id);
            ctx.request(HostRequest::SwitchWorkbench(WorkbenchId::from("wb.sketch")));
        }
        Err(why) => ctx.log_warn(why),
    }
    InputResult::consumed()
}

/// Run `design.gear`, `design.sprocket` or `design.shaft`.
pub(crate) fn command(
    id: &str,
    args: &CommandArgs,
    ctx: &mut WorkbenchRuntimeContext,
) -> CommandResult {
    let a = Args(args);
    let variant = GENERATORS
        .iter()
        .find(|(.., command)| *command == id)
        .map(|(variant, ..)| *variant)
        .ok_or_else(|| CommandError::Unknown(id.to_string()))?;
    let mut generator = Generator::named(variant).expect("a generator per command");
    let own = ["body", "plane", "face_point", "face_normal", "name"];
    let fields: Map<String, Value> = args
        .iter()
        .filter(|(k, _)| !own.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    generator.merge(&fields).map_err(CommandError::failed)?;
    let body = match a.opt_id("body")? {
        Some(id) => {
            let body = BodyId(id);
            if !ctx.document.bodies().iter().any(|b| b.id == body) {
                return Err(CommandError::bad("body", "is not a body of this document"));
            }
            body
        }
        None => match ctx.selected_body_id {
            Some(id) => BodyId(id),
            None => ctx.document.create_body(None),
        },
    };
    let plane = if a.has("face_point") {
        face_plane(
            vector3(args.get("face_point"), "face_point")?,
            vector3(args.get("face_normal"), "face_normal")?,
        )
    } else {
        match a.opt_string("plane")? {
            None => base_plane(&generator).0,
            Some(p) if p.eq_ignore_ascii_case("XY") => SketchPlane::xy(),
            Some(p) if p.eq_ignore_ascii_case("XZ") => SketchPlane::xz(),
            Some(p) if p.eq_ignore_ascii_case("YZ") => SketchPlane::yz(),
            Some(_) => return Err(CommandError::bad("plane", "must be XY, XZ or YZ")),
        }
    };
    let name = a.opt_string("name")?.map(str::to_string);
    let id = add(ctx, body, plane, generator, name.as_deref()).map_err(CommandError::failed)?;
    Ok(json!(id.0.to_string()))
}

fn object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

/// `{x, y, z}` or `[x, y, z]`.
fn vector3(value: Option<&Value>, name: &str) -> Result<[f32; 3], CommandError> {
    let bad = || CommandError::bad(name, "must be {x, y, z}");
    let parts = match value {
        Some(Value::Array(v)) if v.len() == 3 => [v[0].as_f64(), v[1].as_f64(), v[2].as_f64()],
        Some(Value::Object(m)) => ["x", "y", "z"].map(|k| m.get(k).and_then(Value::as_f64)),
        _ => return Err(bad()),
    };
    match parts {
        [Some(x), Some(y), Some(z)] => Ok([x as f32, y as f32, z as f32]),
        _ => Err(bad()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_document::{Document, WorkbenchFeature};
    use wb_sketch::SketchFeature;

    fn run(doc: &mut Document, id: &str, args: Value) -> CommandResult {
        let mut ctx = WorkbenchRuntimeContext::new(doc, [0.0; 3], [0.0; 3], (0, 0, 800, 600));
        command(id, &object(args), &mut ctx)
    }

    #[test]
    fn each_command_makes_its_sketch_with_the_numbers_given() {
        let mut doc = Document::new("g");
        let body = doc.create_body(None);
        let id = run(
            &mut doc,
            "design.gear",
            json!({"body": body.0.to_string(), "teeth": 31, "module": 1.5}),
        )
        .unwrap();
        let id = FeatureId(uuid::Uuid::parse_str(id.as_str().unwrap()).unwrap());
        let feature = SketchFeature::from_json(doc.get_feature_data(id).unwrap()).unwrap();
        let Some(Generator::Gear(spec)) = &feature.generator else {
            panic!("a gear")
        };
        assert_eq!((spec.teeth, spec.module), (31, 1.5));
        assert!(!feature.sketch.geometry.is_empty());
        assert_eq!(doc.get_feature_meta(id).unwrap().name, "Gear");

        let shaft = run(
            &mut doc,
            "design.shaft",
            json!({"body": body.0.to_string()}),
        )
        .unwrap();
        let shaft = FeatureId(uuid::Uuid::parse_str(shaft.as_str().unwrap()).unwrap());
        let feature = SketchFeature::from_json(doc.get_feature_data(shaft).unwrap()).unwrap();
        assert_eq!(feature.plane.normal, SketchPlane::xz().normal);
    }

    #[test]
    fn a_field_the_generator_lacks_is_refused() {
        let mut doc = Document::new("g");
        let err = run(&mut doc, "design.sprocket", json!({"module": 2})).unwrap_err();
        assert!(err.to_string().contains("module"), "{err}");
    }
}
