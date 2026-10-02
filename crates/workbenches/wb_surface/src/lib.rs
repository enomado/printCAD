//! The Surface workbench: sheets made from curves and joined into shells
//! and solids.
//!
//! A surface body holds surface features only: the first surface step on a
//! body that Design builds goes into a new body. Its steps build in the
//! kernel's surface chain (`kernel_api::SurfaceOp`): extruded, revolved,
//! planar, filled, ruled, lofted and swept surfaces added beside one
//! another, then sewn (a closed shell becomes a solid) or mirrored. The
//! steps the kernel has no operation for yet (offset, extend, blend,
//! split, thicken, trim) are tools marked planned; their features, made by
//! a script, keep their data and fail with the kernel's reason.
//!
//! Curves come from sketches (every chain, open or closed) and from edges
//! of the body's own sheets, picked in the view. A task edits a step live;
//! Cancel puts it back, or takes away the step (and the body) the tool made.

pub mod build;
pub mod feature;
#[cfg(feature = "egui")]
mod panel;

use core_document::{
    BodyId, CommandArgs, CommandError, CommandResult, CommandSpec, Document, FeatureId,
    FeatureInfo, HostRequest, InputResult, ParamKind, Parameter, RebuildJob, TaskInfo,
    ToolDescriptor, Workbench, WorkbenchContext, WorkbenchDescriptor, WorkbenchFeature,
    WorkbenchInputEvent, WorkbenchRuntimeContext,
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

#[derive(Default)]
pub struct SurfaceWorkbench {
    options: Options,
    task: Option<Task>,
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
            .selected_face_in(body)
            .map(|face| FacePick {
                point: face.point,
                normal: face.normal,
                name: face.name,
            })
            .into_iter()
            .collect();
        (curves, faces)
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
        let mut curves = Vec::new();
        if let Some(list) = args.get("sketches").and_then(Value::as_array) {
            for item in list {
                let raw = item
                    .as_str()
                    .and_then(|s| uuid::Uuid::parse_str(s).ok())
                    .ok_or_else(|| CommandError::bad("sketches", "must list sketch ids"))?;
                let sketch = FeatureId(raw);
                let is_sketch = ctx
                    .document
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
        }
        let body = match named {
            Some(body) => body,
            None if !feature.constructs() => {
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
        let feature = SurfaceFeature::from_json(&data)
            .map_err(|e| CommandError::failed(format!("the fields do not make a surface: {e}")))?;
        let (made, _) = self.add(ctx, body, feature).map_err(CommandError::failed)?;
        Ok(json!(made.0.to_string()))
    }
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
            SurfaceFeature::Sew
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
        // A step with no fields (Sew) takes none.
        return Ok(());
    };
    let Some(Value::Object(fields)) = outer.values_mut().next() else {
        return Ok(());
    };
    for (name, value) in args {
        if matches!(name.as_str(), "body" | "sketches") {
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
        for kind in KINDS {
            let category = match kind.tool {
                "surface.sew" | "surface.thicken" | "surface.trim" | "surface.mirror"
                | "surface.offset" | "surface.extend" | "surface.split" => "modify",
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
                context.register_command(command_spec(kind));
            }
        }
    }

    fn on_input(
        &mut self,
        event: &WorkbenchInputEvent,
        active_tool: Option<&str>,
        ctx: &mut WorkbenchRuntimeContext,
    ) -> InputResult {
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

    fn editing_feature(&self) -> Option<FeatureId> {
        self.task.as_ref().map(|t| t.feature)
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
        self.create_by_command(id, args, ctx)
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

fn command_spec(kind: &feature::Kind) -> CommandSpec {
    let spec = CommandSpec::new(kind.tool, format!("Make a {}", kind.label.to_lowercase()))
        .optional(
            "body",
            ParamKind::Id,
            "The surface body it goes in, or a feature in it; else its sketch's body when \
             that holds only drawings and surfaces, else a new one",
        )
        .returns("The new feature's id");
    match kind.tool {
        "surface.sew" => spec,
        "surface.mirror" => spec.extra_args("`plane` (\"YZ\", \"XZ\", \"XY\" or {\"Custom\": {\"origin\", \"normal\"}}) and `offset`"),
        _ => spec
            .optional(
                "sketches",
                ParamKind::List,
                "The sketches it is built from, in order: every chain of each, open or closed",
            )
            .extra_args("Any field of the surface, by name (`length`, `direction`, `angle_deg`, `continuity`…)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_names_an_icon_the_set_has() {
        let mut context = WorkbenchContext::default();
        SurfaceWorkbench::default().configure(&mut context);
        assert_eq!(context.tools().len(), KINDS.len());
        #[cfg(feature = "egui")]
        for tool in context.tools() {
            let icon = tool.icon.expect("an icon");
            assert!(ui_kit::icon::exists(icon), "{icon}");
        }
        #[cfg(feature = "egui")]
        assert!(ui_kit::icon::exists("workbench-surface"));
    }

    #[test]
    fn the_planned_tools_are_the_ones_the_kernel_lacks() {
        let mut context = WorkbenchContext::default();
        SurfaceWorkbench::default().configure(&mut context);
        let planned: Vec<&str> = context
            .tools()
            .iter()
            .filter(|t| t.planned.is_some())
            .map(|t| t.id.as_str())
            .collect();
        assert_eq!(
            planned,
            [
                "surface.offset",
                "surface.extend",
                "surface.blend",
                "surface.split",
                "surface.thicken",
                "surface.trim"
            ]
        );
        assert!(
            context
                .commands()
                .iter()
                .all(|c| !planned.contains(&c.id.as_str())),
            "no command makes a planned step"
        );
    }
}
