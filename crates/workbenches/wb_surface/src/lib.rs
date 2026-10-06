//! The Surface workbench: sheets made from curves and joined into shells
//! and solids.
//!
//! A surface body holds surface features only: the first surface step on a
//! body that Design builds goes into a new body. Its steps build in the
//! kernel's surface chain (`kernel_api::SurfaceOp`): extruded, revolved,
//! planar, filled, ruled, lofted and swept surfaces added beside one
//! another, then sewn (a closed shell becomes a solid), trimmed by a plane,
//! split along curves, extended, offset, blended, rounded, thickened into
//! solids or mirrored. A kind of step the kernel cannot build carries a
//! `waits` note: its tool stays dim with it and it has no command.
//!
//! Curves come from sketches (every chain, open or closed) and from edges
//! of the body's own sheets, picked in the view. A task edits a step live;
//! Cancel puts it back, or takes away the step (and the body) the tool made.
//!
//! Check continuity measures how the faces of the selected body meet
//! across every edge they share (the gap, and the crease angle), labels
//! each edge in the view and lists them in a task of its own.

pub mod build;
mod commands;
pub mod feature;
#[cfg(feature = "egui")]
mod panel;

use core_document::{
    BodyId, CommandArgs, CommandError, CommandResult, Document, FeatureId, FeatureInfo,
    HostRequest, InputResult, Parameter, RebuildJob, TaskInfo, ToolDescriptor, Workbench,
    WorkbenchContext, WorkbenchDescriptor, WorkbenchFeature, WorkbenchInputEvent,
    WorkbenchRuntimeContext,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::feature::{CurveRef, EdgePick, FacePick, KIND, KINDS, SurfaceFeature};

/// The bench's preferences.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    /// A sketch a new surface step reads is hidden; the surface stands in
    /// for it.
    pub hide_used_sketches: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            hide_used_sketches: true,
        }
    }
}

/// The step whose task is open.
#[derive(Debug, Clone)]
struct Task {
    feature: FeatureId,
    /// Its data when the task opened; `None` for a step the tool made.
    opened: Option<Value>,
    /// A body the tool made for it, taken away again on Cancel.
    made_body: Option<BodyId>,
    /// Sketches it hid, shown again on Cancel.
    hidden: Vec<FeatureId>,
}

/// The tool that measures how a body's faces meet.
pub const CHECK_TOOL: &str = "surface.check";
/// The tool that starts a sketch in a surface body.
pub const SKETCH_TOOL: &str = "surface.new_sketch";
/// Changes fields of a surface step.
pub const SET_COMMAND: &str = "surface.set";

/// Below this crease the faces count as meeting tangent (degrees).
const TANGENT_DEG: f64 = 0.5;
/// Above this gap the faces count as apart (mm).
const APART_MM: f64 = 1e-3;

/// A continuity check on show: the body, and how its faces meet at each
/// shared edge, the points in world space.
#[derive(Debug, Clone)]
struct Check {
    body: BodyId,
    name: String,
    joins: Vec<([f32; 3], kernel_api::EdgeContinuity)>,
}

#[derive(Default)]
pub struct SurfaceWorkbench {
    options: Options,
    task: Option<Task>,
    check: Option<Check>,
}

impl SurfaceWorkbench {
    fn open_task(&mut self, task: Task) {
        self.task = Some(task);
    }

    /// The body a new step goes in: the selected feature's or the selected
    /// body, when it is a surface body or holds nothing but sketches and
    /// datums; otherwise a new one.
    fn target_body(ctx: &mut WorkbenchRuntimeContext) -> (BodyId, bool) {
        let chosen = match ctx
            .active_document_object
            .and_then(|id| ctx.document.get_feature_meta(id))
        {
            Some(node) if node.body.is_some() => node.body,
            _ => ctx.selected_body_id.map(BodyId),
        };
        if let Some(body) = chosen
            && Self::takes_surfaces(ctx.document, body)
        {
            return (body, false);
        }
        (ctx.document.create_body(Some("Surface".to_string())), true)
    }

    /// Whether surface steps may go in `body`: a surface body, or one
    /// holding only sketches and datums, with no shape from elsewhere.
    pub fn takes_surfaces(document: &Document, body: BodyId) -> bool {
        if build::is_surface_body(document, body) {
            return true;
        }
        let only_drawings = document
            .feature_tree()
            .all_nodes()
            .filter(|(_, n)| n.body == Some(body))
            .all(|(_, n)| matches!(n.workbench_id.as_str(), "wb.sketch" | "core.datum"));
        only_drawings
            && !document.body_solid_is_imported(body)
            && document.bodies().iter().any(|b| b.id == body)
    }

    /// What the selection gives a new step: the selected sketch, and the
    /// edges and face picked on `body`, in its frame.
    fn picked(ctx: &WorkbenchRuntimeContext, body: BodyId) -> (Vec<CurveRef>, Vec<FacePick>) {
        let mut curves = Vec::new();
        if let Some(id) = ctx.active_document_object
            && ctx
                .document
                .get_feature_meta(id)
                .is_some_and(|n| n.workbench_id.as_str() == "wb.sketch")
        {
            curves.push(CurveRef::Sketch(id));
        }
        for edge in ctx.selected_edges_in(body) {
            curves.push(CurveRef::Edge(EdgePick {
                point: edge.point,
                direction: edge.direction,
                faces: edge.faces,
                length: edge.length_mm,
            }));
        }
        let faces = ctx
            .selected_faces_in(body)
            .into_iter()
            .map(|face| FacePick {
                point: face.point,
                normal: face.normal,
                name: face.name,
            })
            .collect();
        (curves, faces)
    }

    /// The body the selection names, whichever bench builds it: the
    /// selected body, else the selected feature's.
    fn selected_body(ctx: &WorkbenchRuntimeContext) -> Option<BodyId> {
        ctx.selected_body_id.map(BodyId).or_else(|| {
            ctx.active_document_object
                .and_then(|id| ctx.document.get_feature_meta(id))
                .and_then(|n| n.body)
        })
    }

    /// Whether the selection gives a step a curve to start from: a sketch
    /// in the tree, or edges picked on a body that takes surfaces.
    fn has_curves(ctx: &WorkbenchRuntimeContext) -> bool {
        let sketch = ctx
            .active_document_object
            .and_then(|id| ctx.document.get_feature_meta(id))
            .is_some_and(|n| n.workbench_id.as_str() == "wb.sketch");
        sketch
            || ctx
                .selected_edges
                .iter()
                .any(|e| Self::takes_surfaces(ctx.document, BodyId(e.body)))
    }

    /// Open the sketcher on a surface body: the selected one when it takes
    /// surfaces, else a new one. Finishing the sketch comes back here with
    /// it selected.
    fn new_sketch(ctx: &mut WorkbenchRuntimeContext) {
        let (body, _) = Self::target_body(ctx);
        ctx.request(HostRequest::StartOn {
            workbench: core_document::WorkbenchId::from("wb.sketch"),
            attach: core_document::SketchAttachRequest {
                body: body.0,
                face: ctx
                    .selected_face
                    .filter(|_| ctx.selected_body_id == Some(body.0)),
                face_origin: core_document::FaceOrigin::Elsewhere,
            },
        });
    }

    /// Measure how the selected body's faces meet and show it.
    fn check(&mut self, ctx: &mut WorkbenchRuntimeContext) -> Result<(), String> {
        let body = Self::selected_body(ctx).ok_or("Select a body to check")?;
        self.check_body(ctx, body)
    }

    /// Measure how `body`'s faces meet and show it.
    fn check_body(
        &mut self,
        ctx: &mut WorkbenchRuntimeContext,
        body: BodyId,
    ) -> Result<(), String> {
        let brep = ctx
            .document
            .imported_brep_blob(body)
            .ok_or("The body has no shape to check yet")?;
        let kernel = ctx.kernel.ok_or("No geometry kernel to measure with")?;
        let joins = kernel.continuity(brep).map_err(|e| e.to_string())?;
        let placed = ctx.document.body_placement(body);
        let name = ctx
            .document
            .bodies()
            .iter()
            .find(|b| b.id == body)
            .map(|b| b.name.clone())
            .unwrap_or_default();
        self.check = Some(Check {
            body,
            name,
            joins: joins
                .into_iter()
                .map(|j| (placed.point(j.point.map(|c| c as f32)), j))
                .collect(),
        });
        Ok(())
    }

    /// Make the step `tool` names from the selection and open its task.
    fn start(
        &mut self,
        tool: &str,
        ctx: &mut WorkbenchRuntimeContext,
    ) -> Result<FeatureId, String> {
        let mut feature =
            SurfaceFeature::for_tool(tool).ok_or_else(|| format!("no surface tool `{tool}`"))?;
        let (body, made_body) = Self::target_body(ctx);
        let (curves, faces) = Self::picked(ctx, body);
        panel_free::take_selection(&mut feature, &curves, &faces);
        let (id, hidden) = self.add(ctx, body, feature)?;
        ctx.active_document_object = Some(id);
        ctx.request(HostRequest::SelectBody(body));
        self.open_task(Task {
            feature: id,
            opened: None,
            made_body: made_body.then_some(body),
            hidden,
        });
        Ok(id)
    }

    /// Add `feature` to `body`, the sketches it reads hidden when the
    /// preferences say so.
    fn add(
        &self,
        ctx: &mut WorkbenchRuntimeContext,
        body: BodyId,
        feature: SurfaceFeature,
    ) -> Result<(FeatureId, Vec<FeatureId>), String> {
        let name = next_name(ctx.document, feature.kind().label);
        let sketches = feature.sketches();
        let id = ctx
            .document
            .add_feature_in_body(feature, name.clone(), Some(body))
            .map_err(|e| format!("adding the surface failed: {e}"))?;
        ctx.document.mark_feature_dirty(id);
        let mut hidden = Vec::new();
        if self.options.hide_used_sketches {
            for sketch in sketches {
                if ctx
                    .document
                    .get_feature_meta(sketch)
                    .is_some_and(|n| n.visible)
                {
                    ctx.document.set_feature_visible(sketch, false);
                    hidden.push(sketch);
                }
            }
        }
        ctx.log_info(format!("Created {name}"));
        Ok((id, hidden))
    }

    /// Write `feature` over the step's data, its dependencies following its
    /// sketches, and build it again.
    fn write(ctx: &mut WorkbenchRuntimeContext, id: FeatureId, feature: &SurfaceFeature) {
        if let Err(why) = ctx.document.update_feature_data(id, feature.to_json()) {
            ctx.log_warn(why.to_string());
            return;
        }
        ctx.document
            .set_feature_dependencies(id, feature.dependencies());
        ctx.document.mark_feature_dirty(id);
    }

    /// Close the open task: keep its edits, recording the command that
    /// makes the step, or put things back.
    fn close(&mut self, ctx: &mut WorkbenchRuntimeContext, accept: bool) -> Option<String> {
        let task = self.task.take()?;
        let feature = ctx
            .document
            .feature_values(task.feature)
            .and_then(|v| SurfaceFeature::from_json(v).ok());
        if accept {
            if task.opened.is_none()
                && let Some(feature) = &feature
            {
                let body = ctx
                    .document
                    .get_feature_meta(task.feature)
                    .and_then(|n| n.body);
                ctx.record(
                    feature.tool(),
                    command_args(body, feature),
                    json!(task.feature.0.to_string()),
                );
            }
            return Some(
                feature
                    .map(|f| format!("Edit {}", f.kind().label.to_lowercase()))
                    .unwrap_or_else(|| "Edit surface".into()),
            );
        }
        match task.opened {
            Some(data) => {
                if let Err(why) = ctx.document.update_feature_data(task.feature, data) {
                    ctx.log_warn(why.to_string());
                }
                if let Some(feature) = feature {
                    ctx.document
                        .set_feature_dependencies(task.feature, feature.dependencies());
                }
                ctx.document.mark_feature_dirty(task.feature);
            }
            None => {
                let body = ctx
                    .document
                    .get_feature_meta(task.feature)
                    .and_then(|n| n.body);
                if let Err(why) = ctx.document.remove_feature(task.feature) {
                    ctx.log_warn(why.to_string());
                }
                for sketch in task.hidden {
                    ctx.document.set_feature_visible(sketch, true);
                }
                match (task.made_body, body) {
                    (Some(made), _) => {
                        ctx.document.remove_body(made);
                    }
                    (None, Some(body)) => build::invalidate_body(ctx.document, body),
                    (None, None) => {}
                }
                ctx.active_document_object = None;
            }
        }
        None
    }

    fn create_by_command(
        &self,
        id: &str,
        args: &CommandArgs,
        ctx: &mut WorkbenchRuntimeContext,
    ) -> CommandResult {
        let a = core_document::command::Args(args);
        let mut feature =
            SurfaceFeature::for_tool(id).ok_or_else(|| CommandError::Unknown(id.into()))?;
        let named = match a.opt_id("body")? {
            Some(raw) => {
                // A body, or a feature standing for the body it is in.
                let body = if ctx.document.bodies().iter().any(|b| b.id == BodyId(raw)) {
                    BodyId(raw)
                } else {
                    ctx.document
                        .get_feature_meta(FeatureId(raw))
                        .and_then(|n| n.body)
                        .ok_or_else(|| {
                            CommandError::bad("body", "is neither a body nor a feature in one")
                        })?
                };
                if !Self::takes_surfaces(ctx.document, body) {
                    return Err(CommandError::bad(
                        "body",
                        "is built by another workbench; surfaces go in a body of their own",
                    ));
                }
                Some(body)
            }
            None => None,
        };
        let curves = sketch_curves(&feature, args, ctx.document)?;
        let body = match named {
            Some(body) => body,
            None if feature.needs_body() => {
                return Err(CommandError::bad(
                    "body",
                    "is required: this step works on the surfaces a body holds",
                ));
            }
            // Beside its sketch, when the sketch's body takes surfaces.
            None => curves
                .iter()
                .find_map(|c| match c {
                    CurveRef::Sketch(id) => ctx.document.get_feature_meta(*id).and_then(|n| n.body),
                    CurveRef::Edge(_) => None,
                })
                .filter(|b| Self::takes_surfaces(ctx.document, *b))
                .unwrap_or_else(|| ctx.document.create_body(Some("Surface".to_string()))),
        };
        panel_free::take_selection(&mut feature, &curves, &[]);
        let mut data = feature.to_json();
        merge_fields(&mut data, args)?;
        let feature = SurfaceFeature::from_json(&data).map_err(unreadable)?;
        if let Some(missing) = feature.missing() {
            return Err(CommandError::failed(format!("it needs {missing}")));
        }
        let (made, _) = self.add(ctx, body, feature).map_err(CommandError::failed)?;
        if let Some(name) = core_document::command::Args(args).opt_string("name")? {
            ctx.document.rename_feature(made, name);
        }
        Ok(json!(made.0.to_string()))
    }
}

impl SurfaceWorkbench {
    /// `surface.check`: the body's joins, shown in the view as the tool
    /// shows them.
    fn check_by_command(
        &mut self,
        args: &CommandArgs,
        ctx: &mut WorkbenchRuntimeContext,
    ) -> CommandResult {
        let raw = core_document::command::Args(args).id("body")?;
        let body = body_of(ctx.document, raw)?;
        self.check_body(ctx, body).map_err(CommandError::failed)?;
        let joins = self
            .check
            .as_ref()
            .map(|c| c.joins.as_slice())
            .unwrap_or_default();
        Ok(Value::Array(
            joins
                .iter()
                .map(|(at, j)| json!({"point": at, "gap": j.gap, "angle_deg": j.angle_deg}))
                .collect(),
        ))
    }
}

/// A body named by its id, or by a feature in it.
fn body_of(document: &Document, raw: uuid::Uuid) -> Result<BodyId, CommandError> {
    if document.bodies().iter().any(|b| b.id == BodyId(raw)) {
        return Ok(BodyId(raw));
    }
    document
        .get_feature_meta(FeatureId(raw))
        .and_then(|n| n.body)
        .ok_or_else(|| CommandError::bad("body", "is neither a body nor a feature in one"))
}

/// `surface.set`: named fields merged into a step, `sketches` replacing its
/// curves, recorded as one edit.
fn set_by_command(args: &CommandArgs, ctx: &mut WorkbenchRuntimeContext) -> CommandResult {
    let id = FeatureId(core_document::command::Args(args).id("feature")?);
    let node = ctx
        .document
        .get_feature_meta(id)
        .ok_or_else(|| CommandError::bad("feature", "is not in this document"))?;
    if node.workbench_id.as_str() != KIND {
        return Err(CommandError::bad("feature", "is not a surface step"));
    }
    let mut feature = ctx
        .document
        .get_feature_data(id)
        .and_then(|data| SurfaceFeature::from_json(data).ok())
        .ok_or_else(|| CommandError::failed("the step's data does not read"))?;
    if args.contains_key("sketches") {
        let curves = sketch_curves(&feature, args, ctx.document)?;
        feature.clear_curves();
        panel_free::take_selection(&mut feature, &curves, &[]);
    }
    let mut fields = args.clone();
    fields.remove("feature");
    fields.remove("sketches");
    let mut data = feature.to_json();
    merge_fields(&mut data, &fields)?;
    let feature = SurfaceFeature::from_json(&data).map_err(unreadable)?;
    if let Some(missing) = feature.missing() {
        return Err(CommandError::failed(format!("it needs {missing}")));
    }
    ctx.document
        .update_feature_data(id, feature.to_json())
        .map_err(|e| CommandError::failed(e.to_string()))?;
    ctx.document.mark_feature_dirty(id);
    Ok(Value::Null)
}

/// The sketches a command names in `sketches`, each checked to be one, and
/// no more than the step takes.
fn sketch_curves(
    feature: &SurfaceFeature,
    args: &CommandArgs,
    document: &Document,
) -> Result<Vec<CurveRef>, CommandError> {
    let mut curves = Vec::new();
    let Some(list) = args.get("sketches") else {
        return Ok(curves);
    };
    let list = list
        .as_array()
        .ok_or_else(|| CommandError::bad("sketches", "must list sketch ids"))?;
    for item in list {
        let raw = item
            .as_str()
            .and_then(|s| uuid::Uuid::parse_str(s).ok())
            .ok_or_else(|| CommandError::bad("sketches", "must list sketch ids"))?;
        let sketch = FeatureId(raw);
        let is_sketch = document
            .get_feature_meta(sketch)
            .is_some_and(|n| n.workbench_id.as_str() == "wb.sketch");
        if !is_sketch {
            return Err(CommandError::bad(
                "sketches",
                format!("{raw} is not a sketch"),
            ));
        }
        curves.push(CurveRef::Sketch(sketch));
    }
    if matches!(feature, SurfaceFeature::Ruled { .. }) && curves.len() > 2 {
        return Err(CommandError::bad(
            "sketches",
            format!("gives {} curves; a ruled surface spans two", curves.len()),
        ));
    }
    Ok(curves)
}

/// A step's fields that do not read as one, with the form a `Custom`
/// direction, axis or plane takes when that is what went wrong.
fn unreadable(error: core_document::DocumentError) -> CommandError {
    let e = error.to_string();
    let custom = [
        "unit variant",
        "expected struct variant",
        "expected newtype variant",
    ];
    let hint = if custom.iter().any(|c| e.contains(c)) {
        "; a `Custom` direction, axis or plane takes its numbers: \
         direction = {Custom = {0, 0, 1}}, \
         axis = {Custom = {origin = {0, 0, 0}, direction = {0, 0, 1}}}, \
         plane = {Custom = {origin = {0, 0, 0}, normal = {1, 0, 0}}}"
    } else {
        ""
    };
    CommandError::failed(format!("the fields do not make a surface: {e}{hint}"))
}

/// What a selection gives a step, shared by the tools and the commands.
mod panel_free {
    use crate::feature::{CurveRef, FacePick, SurfaceFeature};

    /// Put picked curves and faces where the step takes them.
    pub fn take_selection(feature: &mut SurfaceFeature, curves: &[CurveRef], faces: &[FacePick]) {
        let edges = || {
            curves
                .iter()
                .filter_map(|c| match c {
                    CurveRef::Edge(e) => Some(*e),
                    CurveRef::Sketch(_) => None,
                })
                .collect::<Vec<_>>()
        };
        match feature {
            SurfaceFeature::Extrude { curves: c, .. }
            | SurfaceFeature::Revolve { curves: c, .. }
            | SurfaceFeature::PlanarFill { curves: c } => c.extend_from_slice(curves),
            SurfaceFeature::Fill { boundary, .. } => boundary.extend_from_slice(curves),
            SurfaceFeature::Loft { sections, .. } => sections.extend_from_slice(curves),
            SurfaceFeature::Ruled { first, second } => {
                let mut it = curves.iter().copied();
                *first = it.next();
                *second = it.next();
            }
            SurfaceFeature::Sweep { profile, path } => {
                let mut it = curves.iter().copied();
                profile.extend(it.next());
                path.extend(it);
            }
            SurfaceFeature::Offset { faces: f, .. } => f.extend_from_slice(faces),
            SurfaceFeature::Split {
                faces: f,
                curves: c,
            } => {
                f.extend_from_slice(faces);
                c.extend(curves.iter().filter(|c| matches!(c, CurveRef::Sketch(_))));
            }
            SurfaceFeature::Extend { edges: e, .. } => e.extend(edges()),
            SurfaceFeature::Blend { first, second, .. } => {
                let mut it = edges().into_iter();
                *first = it.next();
                *second = it.next();
            }
            SurfaceFeature::Fillet { edges: e, .. } => e.extend(edges()),
            SurfaceFeature::Sew { .. }
            | SurfaceFeature::Thicken { .. }
            | SurfaceFeature::Trim { .. }
            | SurfaceFeature::Mirror { .. } => {}
        }
    }
}

/// `base` when no feature has that name, else `base 2`, `base 3`, … one past
/// the highest in use.
fn next_name(document: &Document, base: &str) -> String {
    let taken: Vec<&str> = document
        .feature_tree()
        .all_nodes()
        .map(|(_, n)| n.name.as_str())
        .collect();
    if !taken.contains(&base) {
        return base.to_string();
    }
    let highest = taken
        .iter()
        .filter_map(|n| n.strip_prefix(base)?.strip_prefix(' ')?.parse::<u32>().ok())
        .max()
        .unwrap_or(1);
    format!("{base} {}", highest + 1)
}

/// The command that makes `feature` in `body`: its fields as named
/// arguments.
fn command_args(body: Option<BodyId>, feature: &SurfaceFeature) -> CommandArgs {
    let mut args = CommandArgs::new();
    if let Some(body) = body {
        args.insert("body".into(), json!(body.0.to_string()));
    }
    if let Value::Object(outer) = feature.to_json()
        && let Some(Value::Object(fields)) = outer.into_values().next()
    {
        args.extend(fields);
    }
    args
}

/// Named arguments laid over a step's fields.
fn merge_fields(data: &mut Value, args: &CommandArgs) -> Result<(), CommandError> {
    let Value::Object(outer) = data else {
        // A step with no fields takes none.
        return Ok(());
    };
    let Some(Value::Object(fields)) = outer.values_mut().next() else {
        return Ok(());
    };
    for (name, value) in args {
        if matches!(name.as_str(), "body" | "sketches" | "name") {
            continue;
        }
        if !fields.contains_key(name) {
            return Err(CommandError::bad(name, "is not a field of this surface"));
        }
        fields.insert(name.clone(), value.clone());
    }
    Ok(())
}

impl Workbench for SurfaceWorkbench {
    fn descriptor(&self) -> WorkbenchDescriptor {
        WorkbenchDescriptor::new(
            KIND,
            "Surface",
            "Sheets made from curves: extruded, revolved, filled, ruled, lofted and swept, sewn into shells and solids",
        )
        .icon("workbench-surface")
        .feature_kinds([KIND])
    }

    fn feature_info(&self, node: &core_document::FeatureNode) -> FeatureInfo {
        let kind = SurfaceFeature::from_json(&node.data).ok().map(|f| f.kind());
        FeatureInfo {
            icon: kind.map_or("workbench-surface", |k| k.icon),
            kind_label: kind.map_or("Surface", |k| k.label).to_string(),
            family_label: "Surface feature".to_string(),
            builds_solid: true,
        }
    }

    fn configure(&self, context: &mut WorkbenchContext) {
        context.register_tool(
            ToolDescriptor::new_action(SKETCH_TOOL, "Create sketch", Some("structure"))
                .icon("sketch-new"),
        );
        for kind in KINDS {
            let category = match kind.tool {
                "surface.sew" | "surface.fillet" | "surface.thicken" | "surface.trim"
                | "surface.mirror" | "surface.offset" | "surface.extend" | "surface.split" => {
                    "modify"
                }
                _ => "create",
            };
            let mut tool =
                ToolDescriptor::new_action(kind.tool, kind.label, Some(category)).icon(kind.icon);
            if let Some(waits) = kind.waits {
                // PLANNED: the step builds once the geometry kernel has its
                // operation; until then the tool stays dim with this note.
                tool = tool.planned(waits);
            }
            context.register_tool(tool);
            if kind.waits.is_none() {
                context.register_command(commands::step(kind));
            }
        }
        context.register_tool(
            ToolDescriptor::new_action(CHECK_TOOL, "Check continuity", Some("analysis"))
                .icon("surface-continuity"),
        );
        context.register_command(commands::check());
        context.register_command(commands::set());
    }

    fn on_input(
        &mut self,
        event: &WorkbenchInputEvent,
        active_tool: Option<&str>,
        ctx: &mut WorkbenchRuntimeContext,
    ) -> InputResult {
        if let WorkbenchInputEvent::ToolActivated = event
            && active_tool == Some(SKETCH_TOOL)
        {
            Self::new_sketch(ctx);
            return InputResult::consumed();
        }
        if let WorkbenchInputEvent::ToolActivated = event
            && active_tool == Some(CHECK_TOOL)
        {
            if let Err(why) = self.check(ctx) {
                ctx.log_warn(why);
            }
            return InputResult::consumed();
        }
        if let WorkbenchInputEvent::ToolActivated = event
            && let Some(tool) = active_tool
            && SurfaceFeature::for_tool(tool).is_some()
        {
            if let Some(task) = &self.task {
                let open = task.feature;
                ctx.log_warn("Finish the surface being edited first");
                ctx.active_document_object = Some(open);
                return InputResult::consumed();
            }
            if let Err(why) = self.start(tool, ctx) {
                ctx.log_warn(why);
            }
            return InputResult::consumed();
        }
        InputResult::ignored()
    }

    fn task(&self, ctx: &WorkbenchRuntimeContext) -> Option<TaskInfo> {
        if self.task.is_none() && self.check.is_some() {
            return Some(TaskInfo {
                title: "Continuity".into(),
                icon: "surface-continuity",
                confirmable: false,
                stepwise: false,
            });
        }
        let task = self.task.as_ref()?;
        let feature = ctx
            .document
            .feature_values(task.feature)
            .and_then(|v| SurfaceFeature::from_json(v).ok())?;
        let kind = feature.kind();
        Some(TaskInfo {
            title: kind.label.to_string(),
            icon: kind.icon,
            confirmable: true,
            stepwise: false,
        })
    }

    #[cfg(feature = "egui")]
    fn ui_task_panel(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &mut WorkbenchRuntimeContext,
        request: core_document::TaskRequest,
    ) -> core_document::TaskOutcome {
        use core_document::TaskOutcome;
        let Some(task) = self.task.clone() else {
            if let Some(check) = &self.check {
                if request.accept || request.cancel {
                    self.check = None;
                    return TaskOutcome::Cancelled;
                }
                panel::continuity(ui, &check.name, &check.joins, TANGENT_DEG, APART_MM);
            }
            return TaskOutcome::Open;
        };
        // The task closes on its own when its step is gone (an undo).
        let Some(mut feature) = ctx
            .document
            .get_feature_data(task.feature)
            .and_then(|v| SurfaceFeature::from_json(v).ok())
        else {
            self.task = None;
            return TaskOutcome::Cancelled;
        };
        if request.accept {
            let label = self.close(ctx, true).unwrap_or_default();
            return TaskOutcome::Accepted { label };
        }
        if request.cancel {
            self.close(ctx, false);
            return TaskOutcome::Cancelled;
        }
        let body = ctx
            .document
            .get_feature_meta(task.feature)
            .and_then(|n| n.body);
        let selection = body.map(|b| Self::picked(ctx, b));
        if panel::editor(ui, ctx, task.feature, &mut feature, selection) {
            Self::write(ctx, task.feature, &feature);
        }
        TaskOutcome::Open
    }

    fn get_screen_space_labels(
        &self,
        ctx: &WorkbenchRuntimeContext,
        _active_feature: Option<FeatureId>,
    ) -> Vec<core_document::ScreenSpaceLabel> {
        let Some(check) = &self.check else {
            return Vec::new();
        };
        if !ctx.document.bodies().iter().any(|b| b.id == check.body) {
            return Vec::new();
        }
        let palette = &ctx.sketch_palette;
        check
            .joins
            .iter()
            .filter_map(|(at, join)| {
                let (x, y) = ctx.world_to_viewport(*at)?;
                let (text, color) = if join.gap > APART_MM {
                    (format!("gap {:.3}", join.gap), palette.conflict)
                } else if join.angle_deg < TANGENT_DEG {
                    ("G1".to_string(), palette.fully_constrained)
                } else {
                    (format!("{:.1}°", join.angle_deg), palette.constraint)
                };
                Some(core_document::ScreenSpaceLabel {
                    pos: [x, y],
                    text,
                    color,
                    size: 11.0,
                    background: true,
                    mono: true,
                })
            })
            .collect()
    }

    fn editing_feature(&self) -> Option<FeatureId> {
        self.task.as_ref().map(|t| t.feature)
    }

    fn is_tool_enabled(&self, tool_id: &str, ctx: &WorkbenchRuntimeContext) -> bool {
        let shaped = |body: BodyId| ctx.document.imported_brep_blob(body).is_some();
        match tool_id {
            SKETCH_TOOL => true,
            CHECK_TOOL => Self::selected_body(ctx).is_some_and(shaped),
            "surface.sew" | "surface.mirror" => Self::selected_body(ctx)
                .is_some_and(|b| build::is_surface_body(ctx.document, b) && shaped(b)),
            tool => match SurfaceFeature::for_tool(tool) {
                Some(feature) if !feature.curves_needed() => true,
                Some(_) => Self::has_curves(ctx),
                None => true,
            },
        }
    }

    fn edit_feature(&mut self, ctx: &mut WorkbenchRuntimeContext, id: FeatureId) {
        if self.task.is_some() {
            return;
        }
        let Some(data) = ctx.document.get_feature_data(id).cloned() else {
            return;
        };
        self.open_task(Task {
            feature: id,
            opened: Some(data),
            made_body: None,
            hidden: Vec::new(),
        });
    }

    fn parameters(&self, node: &core_document::FeatureNode) -> Vec<Parameter> {
        use core_document::expr::Dim;
        let Ok(feature) = SurfaceFeature::from_json(&node.data) else {
            return Vec::new();
        };
        let variant = match feature.to_json() {
            Value::Object(outer) => outer.keys().next().cloned().unwrap_or_default(),
            _ => return Vec::new(),
        };
        let field = |name: &str, label: &str, dim: Dim, key: &str| {
            Parameter::new(name, label, dim, format!("/{variant}/{key}"))
        };
        match feature {
            SurfaceFeature::Extrude { .. } => {
                vec![field("length", "Length", Dim::LENGTH, "length")]
            }
            SurfaceFeature::Revolve { .. } => {
                vec![field("angle", "Angle", Dim::ANGLE, "angle_deg")]
            }
            SurfaceFeature::Offset { .. } => {
                vec![field("distance", "Distance", Dim::LENGTH, "distance")]
            }
            SurfaceFeature::Extend { .. } => vec![field("length", "Length", Dim::LENGTH, "length")],
            SurfaceFeature::Thicken { .. } => {
                vec![field("thickness", "Thickness", Dim::LENGTH, "thickness")]
            }
            SurfaceFeature::Fillet { .. } => vec![field("radius", "Radius", Dim::LENGTH, "radius")],
            SurfaceFeature::Sew { .. } => vec![field("gap", "Gap", Dim::LENGTH, "gap")],
            SurfaceFeature::Trim { .. } | SurfaceFeature::Mirror { .. } => {
                vec![field("offset", "Offset", Dim::LENGTH, "offset")]
            }
            _ => Vec::new(),
        }
    }

    fn rebuild_jobs(&self, document: &mut Document) -> Vec<RebuildJob> {
        build::rebuild_jobs(document)
    }

    fn invalidate_body(&self, document: &mut Document, body: BodyId) {
        build::invalidate_body(document, body);
    }

    fn invalidate_all(&self, document: &mut Document) {
        build::invalidate_all(document);
    }

    fn run_command(
        &mut self,
        id: &str,
        args: &CommandArgs,
        ctx: &mut WorkbenchRuntimeContext,
    ) -> CommandResult {
        match id {
            CHECK_TOOL => self.check_by_command(args, ctx),
            SET_COMMAND => set_by_command(args, ctx),
            _ => self.create_by_command(id, args, ctx),
        }
    }

    fn has_settings(&self) -> bool {
        true
    }

    #[cfg(feature = "egui")]
    fn ui_settings(&mut self, ui: &mut egui::Ui, filter: &str) {
        panel::settings(ui, &mut self.options, filter);
    }

    fn settings_json(&self) -> Option<Value> {
        serde_json::to_value(&self.options).ok()
    }

    fn apply_settings_json(&mut self, value: &Value) {
        if let Ok(options) = serde_json::from_value(value.clone()) {
            self.options = options;
        }
    }

    fn suspend_session(&mut self) -> Option<Box<dyn std::any::Any + Send>> {
        self.task
            .take()
            .map(|t| Box::new(t) as Box<dyn std::any::Any + Send>)
    }

    fn resume_session(&mut self, state: Option<Box<dyn std::any::Any + Send>>) {
        self.task = state.and_then(|s| s.downcast::<Task>().ok()).map(|t| *t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_names_an_icon_the_set_has() {
        let mut context = WorkbenchContext::default();
        SurfaceWorkbench::default().configure(&mut context);
        assert_eq!(context.tools().len(), KINDS.len() + 2);
        #[cfg(feature = "egui")]
        for tool in context.tools() {
            let icon = tool.icon.expect("an icon");
            assert!(ui_kit::icon::exists(icon), "{icon}");
        }
        #[cfg(feature = "egui")]
        assert!(ui_kit::icon::exists("workbench-surface"));
    }

    /// A kernel whose shapes all meet at one crease and one tangent join.
    struct TwoJoins;

    impl kernel_api::KernelQueries for TwoJoins {
        fn project_edge(
            &self,
            _brep: &[u8],
            _near: [f64; 3],
            _plane: &kernel_api::ProfilePlane,
        ) -> kernel_api::KernelResult<kernel_api::ProjectedEdge> {
            Err(kernel_api::KernelError::Unsupported("project_edge".into()))
        }

        fn continuity(
            &self,
            _brep: &[u8],
        ) -> kernel_api::KernelResult<Vec<kernel_api::EdgeContinuity>> {
            Ok(vec![
                kernel_api::EdgeContinuity {
                    point: [1.0, 0.0, 0.0],
                    gap: 0.0,
                    angle_deg: 90.0,
                },
                kernel_api::EdgeContinuity {
                    point: [2.0, 0.0, 0.0],
                    gap: 0.0,
                    angle_deg: 0.1,
                },
            ])
        }
    }

    #[test]
    fn checking_a_body_shows_its_joins_in_a_task_until_closed() {
        static KERNEL: TwoJoins = TwoJoins;
        let mut document = Document::new("t");
        let body = document.create_body(Some("Shell".into()));
        document.set_imported_brep_data(body, b"ogeom".to_vec(), Vec::new());
        let mut bench = SurfaceWorkbench::default();
        let mut ctx = WorkbenchRuntimeContext::new(&mut document, [0.0; 3], [0.0; 3], (0, 0, 1, 1));
        ctx.kernel = Some(&KERNEL);
        ctx.selected_body_id = Some(body.0);
        bench.on_input(
            &WorkbenchInputEvent::ToolActivated,
            Some(CHECK_TOOL),
            &mut ctx,
        );
        assert_eq!(bench.task(&ctx).map(|t| t.title), Some("Continuity".into()));
        let joins = &bench.check.as_ref().unwrap().joins;
        assert_eq!(joins.len(), 2);
        assert_eq!(joins[0].0, [1.0, 0.0, 0.0], "placed where the body sits");
        bench.check = None;
        assert!(bench.task(&ctx).is_none());
    }

    #[test]
    fn a_curve_tool_waits_for_a_sketch_or_edges() {
        let mut document = Document::new("t");
        let body = document.create_body(None);
        let sketch = document.add_feature_of_kind(
            core_document::WorkbenchId::new("wb.sketch"),
            "Sketch".into(),
            Some(body),
            Vec::new(),
            json!({}),
            core_document::FeatureOrigin::default(),
        );
        let bench = SurfaceWorkbench::default();
        let mut ctx = WorkbenchRuntimeContext::new(&mut document, [0.0; 3], [0.0; 3], (0, 0, 1, 1));
        assert!(!bench.is_tool_enabled("surface.extrude", &ctx));
        assert!(
            !bench.is_tool_enabled("surface.sew", &ctx),
            "no surface body yet"
        );
        assert!(bench.is_tool_enabled(SKETCH_TOOL, &ctx));
        ctx.active_document_object = Some(sketch);
        assert!(bench.is_tool_enabled("surface.extrude", &ctx));
        assert!(bench.is_tool_enabled("surface.loft", &ctx));
    }

    #[test]
    fn create_sketch_hands_a_surface_body_to_the_sketcher() {
        let mut document = Document::new("t");
        let mut bench = SurfaceWorkbench::default();
        let mut ctx = WorkbenchRuntimeContext::new(&mut document, [0.0; 3], [0.0; 3], (0, 0, 1, 1));
        bench.on_input(
            &WorkbenchInputEvent::ToolActivated,
            Some(SKETCH_TOOL),
            &mut ctx,
        );
        let requests = ctx.take_requests();
        let Some(HostRequest::StartOn { workbench, attach }) = requests.first() else {
            panic!("{requests:?}");
        };
        assert_eq!(workbench.as_str(), "wb.sketch");
        let body = document
            .bodies()
            .iter()
            .find(|b| b.id.0 == attach.body)
            .unwrap();
        assert_eq!(body.name, "Surface", "a new surface body");
    }

    #[test]
    fn checking_with_nothing_selected_says_so() {
        let mut document = Document::new("t");
        let mut bench = SurfaceWorkbench::default();
        let mut ctx = WorkbenchRuntimeContext::new(&mut document, [0.0; 3], [0.0; 3], (0, 0, 1, 1));
        assert_eq!(bench.check(&mut ctx), Err("Select a body to check".into()));
    }

    #[test]
    fn every_step_has_its_tool_and_its_command() {
        let mut context = WorkbenchContext::default();
        SurfaceWorkbench::default().configure(&mut context);
        assert!(context.tools().iter().all(|t| t.planned.is_none()));
        for kind in KINDS {
            assert!(
                context.commands().iter().any(|c| c.id == kind.tool),
                "{} has a command",
                kind.tool
            );
        }
    }
}
