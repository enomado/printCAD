//! The Assembly task panels: the prompt while a joint's faces are picked,
//! a joint's or a coupling's settings, a body moved by numbers, and the
//! assembly's own tasks (interference, exploded views, motion over time,
//! linked copies, replacing a body, rigid groups, mass, the parts list).

use core_document::{
    BodyId, BodyPlacement, FeatureId, TaskOutcome, TaskRequest, WorkbenchFeature,
    WorkbenchRuntimeContext,
};
use egui::RichText;
use ui_kit::tokens::*;
use ui_kit::widgets::{Card, Note, QtyField, check_row, destructive_button, note_card, overline};
use ui_kit::{sans, sans_semibold};

use crate::{
    AssemblyWorkbench, Coupling, Gearing, JointFeature, JointKind, JointTool, Task, body_name,
    restore_placements,
};

/// Frames in a recorded sweep: there and back in four seconds.
const SWEEP_FRAMES: usize = 60;

fn header(ui: &mut egui::Ui, icon: &str, title: &str) {
    egui::Frame::new()
        .fill(BG2)
        .stroke(egui::Stroke::new(1.0, BORDER))
        .corner_radius(5)
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = SPACE_2;
                ui_kit::icon::draw(ui, icon, 18.0, ACCENT);
                ui.label(
                    RichText::new(title)
                        .font(sans_semibold(FONT_MD))
                        .color(TEXT1),
                );
            });
        });
}

fn row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [90.0, INPUT],
            egui::Label::new(RichText::new(label).font(sans(FONT_SM)).color(TEXT2)),
        );
        ui.label(RichText::new(value).font(sans(FONT_SM)).color(TEXT1));
    });
}

impl AssemblyWorkbench {
    pub(crate) fn draw_task_panel(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &mut WorkbenchRuntimeContext,
        request: TaskRequest,
    ) -> TaskOutcome {
        if let Some(picking) = &self.picking {
            if request.accept || request.cancel {
                self.picking = None;
                return TaskOutcome::Cancelled;
            }
            header(ui, picking.kind.icon(), picking.kind.label());
            ui.add_space(SPACE_2);
            ui.label(
                RichText::new(picking.prompt())
                    .font(sans(FONT_SM))
                    .color(TEXT1),
            );
            ui.add_space(SPACE_1);
            ui.label(
                RichText::new(
                    "The first body moves; the second stays where it is. A body \
                     with no joints of its own never moves. A datum plane or line \
                     selected in the tree is taken as a face.",
                )
                .font(sans(FONT_XS))
                .color(TEXT3),
            );
            if let Some((_, first, ..)) = picking.first {
                let offered: Vec<_> = crate::ORIGIN
                    .iter()
                    .filter(|(_, anchor)| picking.kind.takes_anchor(anchor, Some(first)))
                    .collect();
                if !offered.is_empty() {
                    ui.add_space(SPACE_2);
                    overline(ui, "Or the origin's");
                    ui.horizontal_wrapped(|ui| {
                        for (name, anchor) in offered {
                            if ui_kit::widgets::small_secondary_button(ui, name).clicked() {
                                self.picked(ctx, crate::WORLD, *anchor, None, 0);
                            }
                        }
                    });
                }
            }
            return TaskOutcome::Open;
        }
        match self.task.clone() {
            Some(Task::Joint {
                id,
                before,
                placements,
            }) => self.joint_panel(ui, ctx, request, id, before, &placements),
            Some(Task::Coupling {
                id,
                before,
                placements,
            }) => self.coupling_panel(ui, ctx, request, id, before, &placements),
            Some(Task::Move { body, placements }) => {
                self.move_panel(ui, ctx, request, body, &placements)
            }
            Some(Task::Interference {
                found,
                seq,
                around,
                clearance,
            }) => {
                self.interference_panel(ui, ctx, request, found.as_ref(), (seq, around, clearance))
            }
            Some(Task::Explode {
                placements,
                spread,
                steps,
            }) => {
                if steps.view.is_some() || !steps.picked.is_empty() {
                    return self.steps_panel(ui, ctx, request, placements, *steps);
                }
                self.explode_panel(ui, ctx, request, placements, spread)
            }
            Some(Task::Parts) => self.parts_panel(ui, ctx, request),
            Some(Task::Motion(studying)) => self.motion_panel(ui, ctx, request, *studying),
            Some(Task::Copies {
                body,
                count,
                step,
                around,
                mirror,
            }) => self.copies_panel(ui, ctx, request, body, (count, step, around), mirror),
            Some(Task::Replace { old, new }) => self.replace_panel(ui, ctx, request, old, new),
            Some(Task::Group { editing, members }) => {
                self.group_panel(ui, ctx, request, editing, &members)
            }
            Some(Task::Mass { found, density }) => {
                self.mass_panel(ui, ctx, request, found.as_ref(), density)
            }
            None => TaskOutcome::Open,
        }
    }

    fn verdict_card(&self, ui: &mut egui::Ui) {
        match &self.verdict {
            Some(Ok(message)) => {
                note_card(ui, Note::Success, None, message);
            }
            Some(Err(message)) => {
                note_card(ui, Note::Error, Some("Joints left apart"), message);
            }
            None => {}
        }
    }

    /// What an interference check found: each clash, a click selecting
    /// the first of its bodies.
    fn interference_panel(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &mut WorkbenchRuntimeContext,
        request: TaskRequest,
        found: Option<&crate::Interference>,
        (seq, around, clearance): (u64, Option<core_document::BodyId>, Option<f32>),
    ) -> TaskOutcome {
        if request.accept || request.cancel {
            self.checking = None;
            self.task = None;
            return TaskOutcome::Cancelled;
        }
        header(ui, "check-geometry", "Interference");
        ui.add_space(SPACE_2);
        self.collect_interference(ctx);
        let Some(found) = found else {
            let (done, total) = self.interference_progress().unwrap_or((0, 0));
            ui.label(
                RichText::new(format!("Checking {done} of {total} pairs that may touch"))
                    .font(sans(FONT_SM))
                    .color(TEXT1),
            );
            ui.add(egui::ProgressBar::new(if total == 0 {
                0.0
            } else {
                done as f32 / total as f32
            }));
            ui.add_space(SPACE_2);
            if ui_kit::widgets::secondary_button(ui, "Stop")
                .on_hover_text("Stop checking; the clashes found so far stay")
                .clicked()
            {
                self.stop_interference();
            }
            // The answer arrives on another thread, with no event to wake
            // the window.
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
            return TaskOutcome::Open;
        };
        let bodies = format!(
            "{} bod{}",
            found.checked,
            if found.checked == 1 { "y" } else { "ies" }
        );
        if let Some(gap) = clearance {
            match found.near.len() {
                0 => note_card(
                    ui,
                    Note::Success,
                    None,
                    &format!("No pair nearer than {gap} mm among {bodies}"),
                ),
                n => note_card(
                    ui,
                    Note::Warning,
                    Some(&format!(
                        "{n} pair{} nearer than {gap} mm",
                        if n == 1 { "" } else { "s" }
                    )),
                    &format!("Among {bodies}; click one to select its first body"),
                ),
            };
            ui.add_space(SPACE_2);
            for near in &found.near {
                let text = format!(
                    "{} and {}: {:.2} mm",
                    body_name(ctx, near.a),
                    body_name(ctx, near.b),
                    near.distance_mm
                );
                let row = ui.add(
                    egui::Button::new(RichText::new(text).font(sans(FONT_SM)).color(TEXT1))
                        .frame(false),
                );
                if row.clicked() {
                    ctx.request(core_document::HostRequest::SelectBody(near.a));
                }
            }
        }
        if clearance.is_none() {
            match found.clashes.len() {
                0 => note_card(
                    ui,
                    Note::Success,
                    None,
                    &format!("No interference among {bodies}"),
                ),
                n => note_card(
                    ui,
                    Note::Error,
                    Some(&format!("{n} clash{}", if n == 1 { "" } else { "es" })),
                    &format!("Among {bodies}; click one to select its first body"),
                ),
            };
        }
        ui.add_space(SPACE_2);
        for clash in &found.clashes {
            let text = format!(
                "{} and {}: {:.2} mm³",
                body_name(ctx, clash.a),
                body_name(ctx, clash.b),
                clash.volume_mm3
            );
            let row = ui.add(
                egui::Button::new(RichText::new(text).font(sans(FONT_SM)).color(TEXT1))
                    .frame(false),
            );
            if row.clicked() {
                ctx.request(core_document::HostRequest::SelectBody(clash.a));
            }
        }
        if found.stopped {
            ui.add_space(SPACE_1);
            note_card(
                ui,
                Note::Warning,
                None,
                "Stopped early: some pairs were not checked",
            );
        }
        if found.skipped > 0 {
            ui.add_space(SPACE_1);
            ui.label(
                RichText::new(format!(
                    "{} visible bod{} without a solid (a mesh, or not built yet) left out",
                    found.skipped,
                    if found.skipped == 1 { "y" } else { "ies" }
                ))
                .font(sans(FONT_XS))
                .color(TEXT3),
            );
        }
        if ctx.document.mutation_seq() != seq {
            ui.add_space(SPACE_1);
            note_card(
                ui,
                Note::Warning,
                None,
                "The assembly has changed since this check",
            );
        }
        if let Some(body) = around {
            ui.add_space(SPACE_1);
            note_card(
                ui,
                Note::Info,
                None,
                &format!("{} against every other body", crate::body_name(ctx, body)),
            );
        }
        ui.add_space(SPACE_2);
        let mut gap = self.clearance_mm.or(clearance).unwrap_or(0.5);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Clearance").font(sans(FONT_SM)).color(TEXT2));
            if QtyField::mm(&mut gap).range(0.0..=1000.0).show(ui) {
                self.clearance_mm = Some(gap);
            }
        });
        ui.add_space(SPACE_1);
        ui.horizontal(|ui| {
            if ui_kit::widgets::secondary_button(ui, "Check clashes")
                .on_hover_text("Pairs that share material")
                .clicked()
            {
                self.check_interference(ctx, around);
            }
            if ui_kit::widgets::secondary_button(ui, "Check clearance")
                .on_hover_text("Pairs nearer to each other than the clearance")
                .clicked()
            {
                self.check_clearance(ctx, around, gap);
            }
        });
        if around.is_some() && ui_kit::widgets::secondary_button(ui, "Check every pair").clicked() {
            match clearance {
                Some(gap) => self.check_clearance(ctx, None, gap),
                None => self.check_interference(ctx, None),
            }
        }
        TaskOutcome::Open
    }

    /// The exploded view's spread; closing puts every body back.
    fn explode_panel(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &mut WorkbenchRuntimeContext,
        request: TaskRequest,
        placements: Vec<(BodyId, BodyPlacement)>,
        mut spread: f32,
    ) -> TaskOutcome {
        if request.accept || request.cancel {
            self.put_back_explosion(ctx);
            return TaskOutcome::Cancelled;
        }
        header(ui, "scale-geometry", "Exploded view");
        ui.add_space(SPACE_2);
        let changed = ui
            .horizontal(|ui| {
                ui.add_sized(
                    [90.0, INPUT],
                    egui::Label::new(RichText::new("Spread").font(sans(FONT_SM)).color(TEXT2)),
                );
                ui.add(egui::Slider::new(&mut spread, 0.0..=3.0).fixed_decimals(2))
                    .on_hover_text(
                        "How far each body moves out, as a share of its distance from the middle",
                    )
                    .changed()
            })
            .inner;
        if changed && let Some(Task::Explode { spread: kept, .. }) = &mut self.task {
            *kept = spread;
            crate::explode(ctx, &placements, spread);
        }
        ui.add_space(SPACE_2);
        ui.label(
            RichText::new(
                "Each body moves straight out from the middle of the assembly. \
                 Nothing is kept: the bodies go back when this closes. Click bodies to \
                 make a step of a view kept in the document.",
            )
            .font(sans(FONT_XS))
            .color(TEXT3),
        );
        TaskOutcome::Open
    }

    /// An exploded view made of steps: each a set of bodies and a shift,
    /// played in order; the view is kept in the document, the bodies go
    /// back when it closes.
    fn steps_panel(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &mut WorkbenchRuntimeContext,
        request: TaskRequest,
        placements: Vec<(BodyId, BodyPlacement)>,
        steps: crate::Stepping,
    ) -> TaskOutcome {
        if request.accept || request.cancel {
            self.put_back_explosion(ctx);
            return TaskOutcome::Cancelled;
        }
        header(ui, "scale-geometry", "Exploded view");
        ui.add_space(SPACE_2);
        let view = steps
            .view
            .and_then(|id| crate::exploded::view_of(ctx.document, id))
            .unwrap_or_default();
        let count = view.steps.len();
        let mut next = steps.clone();
        let mut edited: Option<crate::ExplodedView> = None;
        let mut remove = None;
        for (i, step) in view.steps.iter().enumerate() {
            ui.horizontal(|ui| {
                let names: Vec<String> = step.bodies.iter().map(|b| body_name(ctx, *b)).collect();
                ui.label(
                    RichText::new(format!(
                        "{}. {} by ({:.1}, {:.1}, {:.1})",
                        i + 1,
                        names.join(", "),
                        step.shift[0],
                        step.shift[1],
                        step.shift[2]
                    ))
                    .font(sans(FONT_SM))
                    .color(TEXT1),
                );
                if ui_kit::widgets::small_secondary_button(ui, "Remove").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            let mut changed = view.clone();
            changed.steps.remove(i);
            next.at = next.at.min(changed.steps.len() as f32);
            edited = Some(changed);
        }
        if count > 0 {
            ui.add_space(SPACE_1);
            ui.horizontal(|ui| {
                ui.add_sized(
                    [90.0, INPUT],
                    egui::Label::new(RichText::new("Progress").font(sans(FONT_SM)).color(TEXT2)),
                );
                ui.add(egui::Slider::new(&mut next.at, 0.0..=count as f32).fixed_decimals(2));
            });
            let label = if next.playing { "Stop" } else { "Play" };
            if ui_kit::widgets::secondary_button(ui, label)
                .on_hover_text("Play the steps in order, a step a second, round again")
                .clicked()
            {
                next.playing = !next.playing;
            }
        }
        ui.add_space(SPACE_2);
        overline(ui, "Next step");
        let picked: Vec<String> = next.picked.iter().map(|b| body_name(ctx, *b)).collect();
        ui.label(
            RichText::new(if picked.is_empty() {
                "Click the bodies it moves".to_string()
            } else {
                picked.join(", ")
            })
            .font(sans(FONT_SM))
            .color(TEXT1),
        );
        for (k, label) in ["Shift x", "Shift y", "Shift z"].into_iter().enumerate() {
            ui.horizontal(|ui| {
                ui.add_sized(
                    [90.0, INPUT],
                    egui::Label::new(RichText::new(label).font(sans(FONT_SM)).color(TEXT2)),
                );
                QtyField::offset(&mut next.shift[k]).show(ui);
            });
        }
        if !next.picked.is_empty() && ui_kit::widgets::secondary_button(ui, "Add step").clicked() {
            let mut changed = edited.clone().unwrap_or(view.clone());
            changed.steps.push(crate::ExplodeStep {
                bodies: std::mem::take(&mut next.picked),
                shift: next.shift,
            });
            next.at = changed.steps.len() as f32;
            edited = Some(changed);
        }
        if let Some(changed) = edited {
            // Back where they sat, so the view plays from there.
            crate::restore_placements(ctx, &placements);
            match next.view {
                Some(id) => {
                    if ctx
                        .document
                        .update_feature_data(id, changed.to_json())
                        .is_ok()
                    {
                        ctx.document.clear_feature_dirty(id);
                    }
                }
                None => {
                    let name = crate::commands::next_name(ctx.document, "Exploded view");
                    if let Ok(id) = ctx
                        .document
                        .add_feature_in_body(changed.clone(), name, None)
                    {
                        ctx.document.clear_feature_dirty(id);
                        next.view = Some(id);
                    }
                }
            }
            if let Some(id) = next.view {
                ctx.record(
                    "asm.exploded_view",
                    crate::commands::object(serde_json::json!({
                        "view": id.0.to_string(),
                        "steps": serde_json::to_value(&changed.steps).unwrap_or_default(),
                    })),
                    serde_json::json!(id.0.to_string()),
                );
            }
        }
        let moved = next.at != steps.at || next.view != steps.view;
        if let Some(Task::Explode { steps: kept, .. }) = &mut self.task {
            **kept = next;
        }
        if moved || count == 0 {
            self.show_steps(ctx);
        }
        ui.add_space(SPACE_2);
        ui.label(
            RichText::new(
                "The view is kept in the document: double-click it in the tree to show it \
                 again. The bodies go back when this closes.",
            )
            .font(sans(FONT_XS))
            .color(TEXT3),
        );
        TaskOutcome::Open
    }

    /// A motion over time: when it starts and ends, its step, and each
    /// joint driven by a formula of `t`; worked out into frames to scrub,
    /// play and record. The bodies go back when it closes.
    fn motion_panel(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &mut WorkbenchRuntimeContext,
        request: TaskRequest,
        mut studying: crate::Studying,
    ) -> TaskOutcome {
        if request.accept || request.cancel {
            self.put_back_motion(ctx);
            return TaskOutcome::Cancelled;
        }
        header(ui, "polar-pattern", "Motion over time");
        ui.add_space(SPACE_2);
        let mut settings_changed = false;
        for (label, value, unit) in [
            ("Start", &mut studying.draft.start, " s"),
            ("End", &mut studying.draft.end, " s"),
            ("Step", &mut studying.draft.step, " s"),
        ] {
            ui.horizontal(|ui| {
                ui.add_sized(
                    [90.0, INPUT],
                    egui::Label::new(RichText::new(label).font(sans(FONT_SM)).color(TEXT2)),
                );
                settings_changed |= QtyField::new(value).unit(unit).speed(0.01).show(ui);
            });
        }
        ui.add_space(SPACE_1);
        overline(ui, "Drives");
        let movable: Vec<crate::Joint> = crate::joints(ctx.document)
            .into_iter()
            .filter(|j| {
                matches!(
                    j.feature.kind,
                    JointKind::Hinge { .. } | JointKind::Slider { .. }
                )
            })
            .collect();
        let name_of = |id: FeatureId| {
            movable
                .iter()
                .find(|j| j.id == id)
                .map_or("a removed joint".to_string(), |j| j.name.clone())
        };
        let mut remove = None;
        for (i, drive) in studying.draft.drives.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(name_of(drive.joint))
                        .font(sans(FONT_SM))
                        .color(TEXT1),
                );
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut drive.formula)
                        .desired_width(140.0)
                        .font(ui_kit::mono(FONT_SM)),
                );
                settings_changed |= edit.changed();
                if ui_kit::widgets::small_secondary_button(ui, "Remove").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            studying.draft.drives.remove(i);
            settings_changed = true;
        }
        let unused: Vec<&crate::Joint> = movable
            .iter()
            .filter(|j| !studying.draft.drives.iter().any(|d| d.joint == j.id))
            .collect();
        if !unused.is_empty() {
            egui::ComboBox::from_id_salt("motion_add_drive")
                .selected_text(RichText::new("Drive a joint…").font(sans(FONT_SM)))
                .show_ui(ui, |ui| {
                    for joint in unused {
                        if ui
                            .selectable_label(false, RichText::new(&joint.name).font(sans(FONT_SM)))
                            .clicked()
                        {
                            let formula = match joint.feature.kind {
                                JointKind::Hinge { .. } => "90 * t",
                                _ => "10 * t",
                            };
                            studying.draft.drives.push(crate::TimedDrive {
                                joint: joint.id,
                                formula: formula.to_string(),
                            });
                            settings_changed = true;
                        }
                    }
                });
        }
        ui.label(
            RichText::new(
                "Each formula gives the joint's drive at time t, in seconds: a hinge's angle \
                 in degrees, a slider's position in millimetres (30 * sin(t * 180°)).",
            )
            .font(sans(FONT_XS))
            .color(TEXT3),
        );
        if settings_changed {
            studying.frames = None;
            studying.playing = false;
        }
        ui.add_space(SPACE_2);
        if studying.frames.is_none()
            && !studying.draft.drives.is_empty()
            && ui_kit::widgets::secondary_button(ui, "Work out the motion").clicked()
        {
            crate::restore_placements(ctx, &studying.placements);
            match studying.draft.frames(ctx.document) {
                Ok(frames) => {
                    studying.frames = Some(frames);
                    studying.frame = 0;
                    let data = studying.draft.to_json();
                    match studying.study {
                        Some(id) => {
                            if ctx.document.update_feature_data(id, data).is_ok() {
                                ctx.document.clear_feature_dirty(id);
                            }
                        }
                        None => {
                            let name = crate::commands::next_name(ctx.document, "Motion");
                            if let Ok(id) =
                                ctx.document
                                    .add_feature_in_body(studying.draft.clone(), name, None)
                            {
                                ctx.document.clear_feature_dirty(id);
                                studying.study = Some(id);
                            }
                        }
                    }
                    if let Some(id) = studying.study {
                        let mut args = serde_json::to_value(&studying.draft).unwrap_or_default();
                        args["study"] = serde_json::json!(id.0.to_string());
                        ctx.record(
                            "asm.motion",
                            crate::commands::object(args),
                            serde_json::json!(id.0.to_string()),
                        );
                    }
                }
                Err(why) => ctx.log_warn(format!("The motion could not be worked out: {why}")),
            }
        }
        let mut show = false;
        if let Some(frames) = &studying.frames {
            let last = frames.len().saturating_sub(1);
            let mut frame = studying.frame as f32;
            let time = frames.get(studying.frame).map_or(0.0, |(t, _)| *t);
            ui.horizontal(|ui| {
                ui.add_sized(
                    [90.0, INPUT],
                    egui::Label::new(
                        RichText::new(format!("t = {time:.2} s"))
                            .font(ui_kit::mono(FONT_SM))
                            .color(TEXT2),
                    ),
                );
                if ui
                    .add(
                        egui::Slider::new(&mut frame, 0.0..=last as f32)
                            .step_by(1.0)
                            .show_value(false),
                    )
                    .changed()
                {
                    studying.frame = frame as usize;
                    show = true;
                }
            });
            ui.horizontal(|ui| {
                let label = if studying.playing { "Stop" } else { "Play" };
                if ui_kit::widgets::secondary_button(ui, label).clicked() {
                    studying.playing = !studying.playing;
                }
                if ui_kit::widgets::secondary_button(ui, "Record")
                    .on_hover_text("Save the frames as an animation seen from the current view")
                    .clicked()
                {
                    ctx.request(core_document::HostRequest::RecordAnimation {
                        name: "motion".into(),
                        frames: frames.iter().map(|(_, p)| p.clone()).collect(),
                        frame_ms: (studying.draft.step.max(0.01) * 1000.0) as u32,
                    });
                }
            });
        }
        if let Some(frames) = &studying.frames {
            ui.add_space(SPACE_2);
            overline(ui, "Traces");
            if ui_kit::widgets::small_secondary_button(
                ui,
                if studying.tracing {
                    "Click a point on a body…"
                } else {
                    "Follow a point"
                },
            )
            .on_hover_text("Click a face of a body: its path and speed through the motion")
            .clicked()
            {
                studying.tracing = !studying.tracing;
            }
            let mut remove = None;
            let mut curves: Vec<Vec<(f32, f32)>> = Vec::new();
            for (i, (body, point)) in studying.traces.iter().enumerate() {
                let path = crate::motion::trace(frames, *body, *point);
                let now = path.get(studying.frame).map_or(0.0, |(_, _, v)| *v);
                let top = path.iter().map(|(_, _, v)| *v).fold(0.0f32, f32::max);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "{}: {now:.1} mm/s now, {top:.1} at most",
                            body_name(ctx, *body)
                        ))
                        .font(sans(FONT_SM))
                        .color(TEXT1),
                    );
                    if ui_kit::widgets::small_secondary_button(ui, "Remove").clicked() {
                        remove = Some(i);
                    }
                });
                curves.push(path.iter().map(|(t, _, v)| (*t, *v)).collect());
            }
            if let Some(i) = remove {
                studying.traces.remove(i);
            }
            // Each driven joint's value over time too, scaled to the plot.
            for drive in &studying.draft.drives {
                let values: Vec<(f32, f32)> = frames
                    .iter()
                    .filter_map(|(t, _)| {
                        Some((
                            *t,
                            crate::motion::value_at(&drive.formula, f64::from(*t)).ok()? as f32,
                        ))
                    })
                    .collect();
                curves.push(values);
            }
            if !curves.is_empty() {
                let time = frames.get(studying.frame).map(|(t, _)| *t);
                plot(ui, &curves, studying.traces.len(), time);
            }
        }
        self.task = Some(Task::Motion(Box::new(studying)));
        if show {
            self.show_frame(ctx);
        }
        if matches!(&self.task, Some(Task::Motion(s)) if s.playing) {
            ui.ctx().request_repaint();
        }
        TaskOutcome::Open
    }

    /// Linked copies of a body: how many and how far apart; OK inserts them
    /// in a row, to be dragged where they go.
    fn copies_panel(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &mut WorkbenchRuntimeContext,
        request: TaskRequest,
        body: BodyId,
        (count, step, around): (u32, [f32; 3], Option<crate::Around>),
        mirror: Option<([f32; 3], [f32; 3])>,
    ) -> TaskOutcome {
        if request.cancel {
            self.task = None;
            return TaskOutcome::Cancelled;
        }
        if request.accept
            && let Some((point, normal)) = mirror
        {
            let plane = core_document::MirrorPlane { point, normal };
            let made = ctx.document.create_mirrored_copy(body, plane, None);
            match made {
                Some(copy) => {
                    ctx.record(
                        "asm.mirror",
                        crate::commands::object(serde_json::json!({
                            "body": body.0.to_string(),
                            "point": point,
                            "normal": normal,
                        })),
                        serde_json::json!(copy.0.to_string()),
                    );
                    self.task = None;
                    return TaskOutcome::Accepted {
                        label: "Insert mirrored copy".to_string(),
                    };
                }
                None => ctx.log_warn("A mirrored copy cannot be mirrored again"),
            }
            return TaskOutcome::Open;
        }
        if request.accept {
            let made = match around {
                Some((point, axis, angle)) => crate::commands::insert_copies_around(
                    ctx,
                    body,
                    count as usize,
                    (
                        glam::Vec3::from_array(point),
                        glam::Vec3::from_array(axis).normalize_or_zero(),
                        angle,
                    ),
                ),
                None => crate::commands::insert_copies(
                    ctx,
                    body,
                    count as usize,
                    Some(glam::Vec3::from_array(step)),
                ),
            }
            .unwrap_or_default();
            let mut args = serde_json::json!({"body": body.0.to_string(), "count": count});
            match around {
                Some((point, direction, angle)) => {
                    args["around"] =
                        serde_json::json!({"point": point, "direction": direction, "angle": angle});
                }
                None => args["step"] = serde_json::json!(step),
            }
            ctx.record(
                "asm.copy",
                crate::commands::object(args),
                serde_json::json!(made.iter().map(|b| b.0.to_string()).collect::<Vec<_>>()),
            );
            ctx.log_info(format!(
                "Inserted {} linked cop{} of {}: drag them where they go",
                made.len(),
                if made.len() == 1 { "y" } else { "ies" },
                body_name(ctx, body)
            ));
            self.task = None;
            return TaskOutcome::Accepted {
                label: "Insert linked copies".to_string(),
            };
        }
        header(ui, "clone", "Insert linked copies");
        ui.add_space(SPACE_2);
        row(ui, "Of", &body_name(ctx, body));
        let mut n = count as f32;
        let mut at = step;
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.add_sized(
                [90.0, INPUT],
                egui::Label::new(RichText::new("How many").font(sans(FONT_SM)).color(TEXT2)),
            );
            changed |= QtyField::new(&mut n)
                .range(1.0..=500.0)
                .decimals(0)
                .speed(0.1)
                .show(ui);
        });
        let mut mirrored = mirror.is_some();
        let mut plane = mirror.unwrap_or(([0.0; 3], [1.0, 0.0, 0.0]));
        let mut mirror_changed = check_row(ui, &mut mirrored, "A mirror image")
            .on_hover_text("One copy mirrored across a plane, rather than copies as they are")
            .changed();
        if mirrored {
            ui.horizontal(|ui| {
                ui.add_sized(
                    [90.0, INPUT],
                    egui::Label::new(RichText::new("Across").font(sans(FONT_SM)).color(TEXT2)),
                );
                for (name, normal) in [
                    ("YZ", [1.0, 0.0, 0.0]),
                    ("XZ", [0.0, 1.0, 0.0]),
                    ("XY", [0.0, 0.0, 1.0]),
                ] {
                    if ui
                        .selectable_label(
                            plane.1 == normal,
                            RichText::new(name).font(sans(FONT_SM)),
                        )
                        .clicked()
                    {
                        plane.1 = normal;
                        mirror_changed = true;
                    }
                }
            });
            for (k, label) in ["Through x", "Through y", "Through z"]
                .into_iter()
                .enumerate()
            {
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [90.0, INPUT],
                        egui::Label::new(RichText::new(label).font(sans(FONT_SM)).color(TEXT2)),
                    );
                    mirror_changed |= QtyField::offset(&mut plane.0[k]).show(ui);
                });
            }
        }
        if mirror_changed && let Some(Task::Copies { mirror, .. }) = &mut self.task {
            *mirror = mirrored.then_some(plane);
        }
        if mirrored {
            ui.add_space(SPACE_1);
            note(
                ui,
                "The mirror image takes the body's shape, mirrored, and follows every change \
                 to it.",
            );
            return TaskOutcome::Open;
        }
        let mut turned = around.is_some();
        let mut pivot = around.unwrap_or(([0.0; 3], [0.0, 0.0, 1.0], 360.0));
        changed |= check_row(ui, &mut turned, "Around an axis")
            .on_hover_text("Turn the copies about an axis instead of setting them in a row")
            .changed();
        let label_row = |ui: &mut egui::Ui, label: &str| {
            ui.add_sized(
                [90.0, INPUT],
                egui::Label::new(RichText::new(label).font(sans(FONT_SM)).color(TEXT2)),
            );
        };
        if turned {
            ui.horizontal(|ui| {
                label_row(ui, "Axis");
                for (name, axis) in [
                    ("X", [1.0, 0.0, 0.0]),
                    ("Y", [0.0, 1.0, 0.0]),
                    ("Z", [0.0, 0.0, 1.0]),
                ] {
                    if ui
                        .selectable_label(pivot.1 == axis, RichText::new(name).font(sans(FONT_SM)))
                        .clicked()
                    {
                        pivot.1 = axis;
                        changed = true;
                    }
                }
            });
            for (k, label) in ["Through x", "Through y", "Through z"]
                .into_iter()
                .enumerate()
            {
                ui.horizontal(|ui| {
                    label_row(ui, label);
                    changed |= QtyField::offset(&mut pivot.0[k]).show(ui);
                });
            }
            ui.horizontal(|ui| {
                label_row(ui, "Over");
                changed |= QtyField::degrees(&mut pivot.2).show(ui);
            });
        } else {
            for (k, label) in ["Step x", "Step y", "Step z"].into_iter().enumerate() {
                ui.horizontal(|ui| {
                    label_row(ui, label);
                    changed |= QtyField::offset(&mut at[k]).show(ui);
                });
            }
        }
        if changed
            && let Some(Task::Copies {
                count,
                step,
                around,
                ..
            }) = &mut self.task
        {
            *count = n.round().max(1.0) as u32;
            *step = at;
            *around = turned.then_some(pivot);
        }
        ui.add_space(SPACE_1);
        note(
            ui,
            "Each copy takes the body's shape and follows every change to it. They go in a \
             row, each a step from the one before; drag one to put it where it goes.",
        );
        TaskOutcome::Open
    }

    /// A body and the one picked to take its place and joints.
    fn replace_panel(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &mut WorkbenchRuntimeContext,
        request: TaskRequest,
        old: BodyId,
        new: Option<BodyId>,
    ) -> TaskOutcome {
        if request.cancel {
            self.task = None;
            return TaskOutcome::Cancelled;
        }
        if request.accept {
            let Some(new) = new else {
                ctx.log_warn("Click the body that takes its place");
                return TaskOutcome::Open;
            };
            match crate::replace::replace(ctx.document, old, new) {
                Ok(report) => {
                    self.solve_and_apply(ctx);
                    ctx.record(
                        "asm.replace",
                        crate::commands::object(serde_json::json!({
                            "body": old.0.to_string(),
                            "with": new.0.to_string(),
                        })),
                        serde_json::json!({"kept": report.kept, "unmatched": report.unmatched}),
                    );
                    if report.unmatched.is_empty() {
                        ctx.log_info(format!(
                            "{} took the place of {}; every joint found its faces",
                            body_name(ctx, new),
                            body_name(ctx, old)
                        ));
                    } else {
                        ctx.log_warn(format!(
                            "{} took the place of {}; no face matched for {}",
                            body_name(ctx, new),
                            body_name(ctx, old),
                            report.unmatched.join(", ")
                        ));
                    }
                    self.task = None;
                    return TaskOutcome::Accepted {
                        label: "Replace body".to_string(),
                    };
                }
                Err(why) => {
                    ctx.log_warn(format!("Could not replace the body: {why}"));
                    return TaskOutcome::Open;
                }
            }
        }
        header(ui, "carbon-copy", "Replace body");
        ui.add_space(SPACE_2);
        row(ui, "Replace", &body_name(ctx, old));
        row(
            ui,
            "With",
            &new.map_or("click a body".to_string(), |b| body_name(ctx, b)),
        );
        ui.add_space(SPACE_1);
        note(
            ui,
            "The new body goes where the old one sits and takes its joints, each end on \
             the new body's nearest face of the same kind; the old body is hidden.",
        );
        TaskOutcome::Open
    }

    /// The bodies of a rigid group, picked one click each; OK locks them
    /// where they sit.
    fn group_panel(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &mut WorkbenchRuntimeContext,
        request: TaskRequest,
        editing: Option<FeatureId>,
        members: &[BodyId],
    ) -> TaskOutcome {
        if request.cancel {
            self.task = None;
            return TaskOutcome::Cancelled;
        }
        if request.accept {
            let Some(id) = self.make_group(ctx, editing, members) else {
                return TaskOutcome::Open;
            };
            let bodies: Vec<String> = members.iter().map(|b| b.0.to_string()).collect();
            let args = match editing {
                Some(_) => serde_json::json!({"group": id.0.to_string(), "bodies": bodies}),
                None => serde_json::json!({"bodies": bodies}),
            };
            ctx.record(
                "asm.group",
                crate::commands::object(args),
                serde_json::json!(id.0.to_string()),
            );
            self.task = None;
            ctx.active_document_object = Some(id);
            return TaskOutcome::Accepted {
                label: if editing.is_some() {
                    "Edit rigid group"
                } else {
                    "Rigid group"
                }
                .to_string(),
            };
        }
        header(ui, "tree-group", "Rigid group");
        ui.add_space(SPACE_2);
        ui.label(
            RichText::new(
                "Click the bodies to lock together; a second click takes one out. They \
                 move as one, held as they sit now, the first the one the rest hold to.",
            )
            .font(sans(FONT_SM))
            .color(TEXT2),
        );
        ui.add_space(SPACE_2);
        let mut remove = None;
        for (i, body) in members.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(body_name(ctx, *body))
                        .font(sans(FONT_SM))
                        .color(TEXT1),
                );
                if ui_kit::widgets::small_secondary_button(ui, "Remove").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let (Some(i), Some(Task::Group { members, .. })) = (remove, &mut self.task) {
            members.remove(i);
        }
        if members.len() < 2 {
            ui.add_space(SPACE_1);
            note_card(ui, Note::Info, None, "A group takes two bodies or more");
        }
        if let Some(id) = editing {
            ui.add_space(SPACE_2);
            if destructive_button(ui, "Dissolve group")
                .on_hover_text("Remove the group; the bodies stay where they are")
                .clicked()
                && ctx.document.remove_feature(id).is_ok()
            {
                ctx.record(
                    "doc.delete",
                    crate::commands::object(serde_json::json!({"id": id.0.to_string()})),
                    serde_json::Value::Null,
                );
                self.task = None;
                ctx.active_document_object = None;
                return TaskOutcome::Accepted {
                    label: "Dissolve rigid group".to_string(),
                };
            }
        }
        TaskOutcome::Open
    }

    /// The mass of the visible bodies at one density, their centre of mass,
    /// and each body's share.
    fn mass_panel(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &mut WorkbenchRuntimeContext,
        request: TaskRequest,
        found: Option<&crate::MassReport>,
        density: f32,
    ) -> TaskOutcome {
        if request.accept || request.cancel {
            self.measuring = None;
            self.task = None;
            return TaskOutcome::Cancelled;
        }
        header(ui, "measure", "Mass");
        ui.add_space(SPACE_2);
        self.collect_mass(ctx);
        let Some(report) = found else {
            let (done, total) = self.mass_progress().unwrap_or((0, 0));
            ui.label(
                RichText::new(format!("Measuring {done} of {total} bodies"))
                    .font(sans(FONT_SM))
                    .color(TEXT1),
            );
            ui.add(egui::ProgressBar::new(if total == 0 {
                0.0
            } else {
                done as f32 / total as f32
            }));
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(100));
            return TaskOutcome::Open;
        };
        let mut edited = density;
        ui.horizontal(|ui| {
            ui.label(RichText::new("Density").font(sans(FONT_SM)).color(TEXT2));
            QtyField::new(&mut edited)
                .unit("g/cm³")
                .speed(0.01)
                .range(0.0..=100.0)
                .show(ui);
        });
        if edited != density
            && let Some(crate::Task::Mass { density, .. }) = &mut self.task
        {
            *density = edited;
        }
        let density = f64::from(edited);
        let unit = ctx.document.display_unit();
        let line = |ui: &mut egui::Ui, label: &str, text: String| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(label).font(sans(FONT_SM)).color(TEXT2));
                ui.label(RichText::new(text).font(ui_kit::mono(FONT_SM)).color(TEXT1));
            });
        };
        ui.add_space(SPACE_1);
        line(ui, "Mass", mass_text(report.mass_g(density)));
        line(
            ui,
            "Volume",
            core_document::format_volume_mm3(report.volume_mm3(), unit, 2),
        );
        if let Some(c) = report.centre(density) {
            let f = |v: f64| core_document::format_length_mm(v as f32, unit, 2);
            line(
                ui,
                "Centre of mass",
                format!("{}, {}, {}", f(c[0]), f(c[1]), f(c[2])),
            );
        }
        ui.add_space(SPACE_2);
        egui::Grid::new("assembly_mass")
            .num_columns(2)
            .striped(true)
            .spacing([SPACE_3, SPACE_1])
            .show(ui, |ui| {
                for heading in ["Body", "Mass"] {
                    ui.label(RichText::new(heading).font(sans(FONT_XS)).color(TEXT3));
                }
                ui.end_row();
                for b in &report.bodies {
                    let name = ui.add(
                        egui::Button::new(
                            RichText::new(body_name(ctx, b.body))
                                .font(sans(FONT_SM))
                                .color(TEXT1),
                        )
                        .frame(false),
                    );
                    if name.clicked() {
                        ctx.request(core_document::HostRequest::SelectBody(b.body));
                    }
                    ui.label(
                        RichText::new(mass_text(b.mass_g(density)))
                            .font(ui_kit::mono(FONT_SM))
                            .color(TEXT1),
                    );
                    ui.end_row();
                }
            });
        if report.skipped > 0 {
            ui.add_space(SPACE_1);
            ui.label(
                RichText::new(format!(
                    "{} visible bod{} without a closed solid left out",
                    report.skipped,
                    if report.skipped == 1 { "y" } else { "ies" }
                ))
                .font(sans(FONT_XS))
                .color(TEXT3),
            );
        }
        ui.add_space(SPACE_2);
        if ui_kit::widgets::secondary_button(ui, "Measure again").clicked() {
            self.measure_mass(ctx, edited);
        }
        TaskOutcome::Open
    }

    /// Every part, how many of it and its size, with a copy for a
    /// spreadsheet; its item number, whether it is bought and the added
    /// columns, kept in the document.
    fn parts_panel(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &mut WorkbenchRuntimeContext,
        request: TaskRequest,
    ) -> TaskOutcome {
        if request.accept || request.cancel {
            self.task = None;
            return TaskOutcome::Cancelled;
        }
        header(ui, "file-document", "Parts list");
        ui.add_space(SPACE_2);
        let parts = crate::parts_list(ctx.document);
        let mut table = crate::parts::table_of(ctx.document)
            .map(|(_, t)| t)
            .unwrap_or_default();
        let before = table.clone();
        let total: usize = parts.iter().map(|p| p.bodies.len()).sum();
        ui.label(
            RichText::new(format!(
                "{} part{}, {total} bod{}",
                parts.len(),
                if parts.len() == 1 { "" } else { "s" },
                if total == 1 { "y" } else { "ies" }
            ))
            .font(sans(FONT_SM))
            .color(TEXT2),
        );
        ui.add_space(SPACE_1);
        let columns = table.columns.clone();
        if !ctx.document.components().is_empty() {
            ui.checkbox(&mut table.by_component, "By component")
                .on_hover_text("Each component's parts under it, nested as the components are");
        }
        let by_component = table.by_component && !ctx.document.components().is_empty();
        egui::ScrollArea::horizontal().show(ui, |ui| {
            egui::Grid::new("assembly_parts")
                .num_columns(5 + columns.len())
                .striped(true)
                .spacing([SPACE_3, SPACE_1])
                .show(ui, |ui| {
                    for heading in ["No.", "Part", "Qty", "Size (mm)", "Bought"] {
                        ui.label(RichText::new(heading).font(sans(FONT_XS)).color(TEXT3));
                    }
                    for column in &columns {
                        ui.label(RichText::new(column).font(sans(FONT_XS)).color(TEXT3));
                    }
                    ui.end_row();
                    let rows = if by_component {
                        crate::parts::parts_by_component(ctx.document, &parts)
                    } else {
                        parts
                            .iter()
                            .enumerate()
                            .map(|(i, p)| crate::parts::LevelRow::Part {
                                depth: 0,
                                part: i,
                                bodies: p.bodies.clone(),
                            })
                            .collect()
                    };
                    for row in &rows {
                        let (depth, part, here) = match row {
                            crate::parts::LevelRow::Component { depth, name, .. } => {
                                ui.label("");
                                ui.label(
                                    RichText::new(format!("{}{name}", "    ".repeat(*depth)))
                                        .font(ui_kit::sans_semibold(FONT_SM))
                                        .color(TEXT1),
                                );
                                ui.label(
                                    RichText::new("1").font(ui_kit::mono(FONT_SM)).color(TEXT1),
                                );
                                ui.end_row();
                                continue;
                            }
                            crate::parts::LevelRow::Part {
                                depth,
                                part,
                                bodies,
                            } => (*depth, &parts[*part], bodies),
                        };
                        let number = part.number.map_or("-".to_string(), |n| n.to_string());
                        ui.label(
                            RichText::new(number)
                                .font(ui_kit::mono(FONT_SM))
                                .color(TEXT2),
                        );
                        let name = ui.add(
                            egui::Button::new(
                                RichText::new(format!("{}{}", "    ".repeat(depth), part.name))
                                    .font(sans(FONT_SM))
                                    .color(TEXT1),
                            )
                            .frame(false),
                        );
                        if name.clicked() {
                            ctx.request(core_document::HostRequest::SelectBody(here[0]));
                        }
                        ui.label(
                            RichText::new(here.len().to_string())
                                .font(ui_kit::mono(FONT_SM))
                                .color(TEXT1),
                        );
                        let size = part.size_mm.map_or_else(
                            || "-".to_string(),
                            |s| format!("{:.1} × {:.1} × {:.1}", s[0], s[1], s[2]),
                        );
                        ui.label(RichText::new(size).font(ui_kit::mono(FONT_SM)).color(TEXT1));
                        let mut bought = part.bought;
                        if ui
                            .checkbox(&mut bought, "")
                            .on_hover_text(
                                "Bought rather than made: left out of exports and the slicer",
                            )
                            .changed()
                        {
                            table.entry_mut(&part.bodies).bought = bought;
                        }
                        for column in &columns {
                            let mut text = part.values.get(column).cloned().unwrap_or_default();
                            let edit = ui.add(
                                egui::TextEdit::singleline(&mut text)
                                    .desired_width(90.0)
                                    .font(ui_kit::sans(FONT_SM)),
                            );
                            if edit.lost_focus()
                                && part.values.get(column).cloned().unwrap_or_default() != text
                            {
                                table
                                    .entry_mut(&part.bodies)
                                    .values
                                    .insert(column.clone(), text);
                            }
                        }
                        ui.end_row();
                    }
                });
        });
        ui.add_space(SPACE_2);
        ui.horizontal(|ui| {
            if ui_kit::widgets::secondary_button(ui, "Number the parts")
                .on_hover_text("Give every part without an item number the next one, in list order")
                .clicked()
            {
                table.number(&parts);
            }
            let draft_id = ui.id().with("new_parts_column");
            let mut draft: String = ui.data(|d| d.get_temp(draft_id)).unwrap_or_default();
            ui.add(
                egui::TextEdit::singleline(&mut draft)
                    .hint_text("New column")
                    .desired_width(100.0)
                    .font(ui_kit::sans(FONT_SM)),
            );
            let name = draft.trim().to_string();
            if ui_kit::widgets::small_secondary_button(ui, "Add column").clicked()
                && !name.is_empty()
                && !table.columns.contains(&name)
            {
                table.columns.push(name);
                draft.clear();
            }
            ui.data_mut(|d| d.insert_temp(draft_id, draft));
        });
        if !columns.is_empty() {
            ui.horizontal_wrapped(|ui| {
                for column in &columns {
                    if ui_kit::widgets::small_secondary_button(ui, &format!("Remove {column}"))
                        .clicked()
                    {
                        table.columns.retain(|c| c != column);
                        for entry in table.entries.values_mut() {
                            entry.values.remove(column);
                        }
                    }
                }
            });
        }
        if table != before {
            match crate::parts::store_table(ctx.document, &table) {
                Ok(_) => {
                    ctx.record(
                        "asm.parts_table",
                        crate::commands::object(serde_json::json!({
                            "table": serde_json::to_value(&table).unwrap_or_default(),
                        })),
                        serde_json::Value::Null,
                    );
                    ctx.request(core_document::HostRequest::JournalLabel(
                        "Edit parts list".into(),
                    ));
                }
                Err(why) => ctx.log_warn(format!("Could not keep the parts list: {why}")),
            }
        }
        ui.add_space(SPACE_2);
        let parts = crate::parts_list(ctx.document);
        let csv = if by_component {
            let rows = crate::parts::parts_by_component(ctx.document, &parts);
            crate::parts::levels_csv(&parts, &rows, &table.columns)
        } else {
            crate::parts_csv(&parts, &table.columns)
        };
        ui.horizontal(|ui| {
            if ui_kit::widgets::secondary_button(ui, "Copy as CSV")
                .on_hover_text("For a spreadsheet: every column of the list")
                .clicked()
            {
                ui.ctx().copy_text(csv.clone());
                ctx.log_info("Parts list copied");
            }
            if ui_kit::widgets::secondary_button(ui, "Save as CSV")
                .on_hover_text("Write the list to a file a spreadsheet opens")
                .clicked()
            {
                ctx.request(core_document::HostRequest::SaveFile {
                    name: "parts.csv".into(),
                    kind: "Comma-separated values".into(),
                    extension: "csv".into(),
                    contents: csv.clone().into_bytes(),
                });
            }
        });
        TaskOutcome::Open
    }

    /// End a sweep, the drive back at the value it started from.
    fn stop_playing(&mut self, ctx: &mut WorkbenchRuntimeContext) {
        let Some(play) = self.playing.take() else {
            return;
        };
        let Some(node) = ctx.document.get_feature_meta(play.joint) else {
            return;
        };
        let Ok(mut joint) = JointFeature::from_json(&node.data) else {
            return;
        };
        if let JointKind::Hinge { drive, .. } | JointKind::Slider { drive, .. } = &mut joint.kind {
            drive.to = drive.to.map(|_| play.start);
        }
        if let Err(why) = ctx
            .document
            .update_feature_data(play.joint, joint.to_json())
        {
            ctx.log_warn(why.to_string());
        }
        ctx.document.clear_feature_dirty(play.joint);
        self.solve_and_apply(ctx);
    }

    fn joint_panel(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &mut WorkbenchRuntimeContext,
        request: TaskRequest,
        id: FeatureId,
        before: Option<serde_json::Value>,
        placements: &[(BodyId, BodyPlacement)],
    ) -> TaskOutcome {
        let created = before.is_none();
        if request.cancel {
            self.playing = None;
            match &before {
                Some(data) => {
                    if let Err(why) = ctx.document.update_feature_data(id, data.clone()) {
                        ctx.log_warn(why.to_string());
                    }
                    ctx.document.clear_feature_dirty(id);
                }
                None => {
                    if let Err(why) = ctx.document.remove_feature(id) {
                        ctx.log_warn(why.to_string());
                    }
                    ctx.active_document_object = None;
                }
            }
            restore_placements(ctx, placements);
            self.task = None;
            return TaskOutcome::Cancelled;
        }
        if request.accept {
            self.stop_playing(ctx);
            crate::commands::record_joint(ctx, id, before.as_ref(), placements);
            self.task = None;
            ctx.active_document_object = None;
            return TaskOutcome::Accepted {
                label: if created { "Add joint" } else { "Edit joint" }.to_string(),
            };
        }
        let Some(node) = ctx.document.get_feature_meta(id).cloned() else {
            self.task = None;
            return TaskOutcome::Cancelled;
        };
        let Ok(mut joint) = JointFeature::from_json(&node.data) else {
            note_card(
                ui,
                Note::Error,
                Some("Unreadable joint"),
                "The stored joint does not parse.",
            );
            return TaskOutcome::Open;
        };
        header(ui, joint.kind.icon(), &node.name);
        ui.add_space(SPACE_2);
        let moving = node.body.map(|b| body_name(ctx, b)).unwrap_or_default();
        row(ui, "Moves", &moving);
        row(ui, "Against", &body_name(ctx, joint.other_body));
        if let Some(current) = JointTool::of_kind(&joint.kind) {
            let mut chosen = None;
            ui.horizontal(|ui| {
                ui.add_sized(
                    [90.0, INPUT],
                    egui::Label::new(RichText::new("Kind").font(sans(FONT_SM)).color(TEXT2)),
                );
                egui::ComboBox::from_id_salt(("joint_kind", id))
                    .selected_text(RichText::new(current.label()).font(sans(FONT_SM)))
                    .width(ui.available_width() - 2.0 * ui.spacing().button_padding.x)
                    .show_ui(ui, |ui| {
                        for tool in JointTool::ALL {
                            let fits = tool.fits(&joint.moving, &joint.fixed);
                            let on = tool == current;
                            let hint = if fits {
                                tool.summary().to_string()
                            } else {
                                format!("{}; pick its faces", tool.summary())
                            };
                            if ui
                                .add(egui::Button::selectable(
                                    on,
                                    RichText::new(tool.label()).font(sans(FONT_SM)),
                                ))
                                .on_hover_text(hint)
                                .clicked()
                                && !on
                            {
                                chosen = Some((tool, fits));
                            }
                        }
                    });
            });
            match chosen {
                Some((tool, true)) => {
                    self.change_kind(ctx, id, created, placements, tool);
                    return TaskOutcome::Open;
                }
                // Faces of the wrong sort for it: pick them afresh.
                Some((tool, false)) => {
                    if created {
                        crate::commands::record_joint(ctx, id, None, placements);
                    }
                    self.start_repick(id, tool);
                    return TaskOutcome::Open;
                }
                None => {}
            }
            if ui_kit::widgets::secondary_button(ui, "Pick faces again")
                .on_hover_text("Pick the two faces afresh; the joint keeps its name and kind")
                .clicked()
            {
                if created {
                    crate::commands::record_joint(ctx, id, None, placements);
                }
                self.start_repick(id, current);
                return TaskOutcome::Open;
            }
        }
        ui.add_space(SPACE_2);
        let mut changed = false;
        let mut formula_edits: Vec<(String, Option<String>)> = Vec::new();
        let document: &core_document::Document = ctx.document;
        let placed = |b: BodyId| -> crate::Rigid { document.body_placement(b).into() };
        let now = node
            .body
            .and_then(|b| joint.travel(&placed(b), &placed(joint.other_body)));
        let align_now = node
            .body
            .and_then(|b| joint.align_travel(&placed(b), &placed(joint.other_body)));
        let dt = f64::from(ui.input(|i| i.stable_dt).min(0.1));
        let playing = &mut self.playing;
        let mut record = None;
        Card::new().padding(SPACE_3).show(ui, |ui| {
            ui.set_width(ui.available_width());
            match &mut joint.kind {
                JointKind::Mate { flip, offset } => {
                    ui.horizontal(|ui| {
                        ui.add_sized(
                            [90.0, INPUT],
                            egui::Label::new(RichText::new("Gap").font(sans(FONT_SM)).color(TEXT2)),
                        );
                        changed |= formula_field(
                            ui,
                            document,
                            id,
                            "/kind/Mate/offset",
                            core_document::expr::Dim::LENGTH,
                            offset,
                            &mut formula_edits,
                        );
                    })
                    .response
                    .on_hover_text("How far apart the two faces sit");
                    changed |= check_row(ui, flip, "Same way")
                        .on_hover_text("The faces point the same way instead of at each other")
                        .changed();
                }
                JointKind::Angle { degrees } => {
                    ui.horizontal(|ui| {
                        ui.add_sized(
                            [90.0, INPUT],
                            egui::Label::new(
                                RichText::new("Angle").font(sans(FONT_SM)).color(TEXT2),
                            ),
                        );
                        changed |= formula_field(
                            ui,
                            document,
                            id,
                            "/kind/Angle/degrees",
                            core_document::expr::Dim::ANGLE,
                            degrees,
                            &mut formula_edits,
                        );
                    })
                    .response
                    .on_hover_text(
                        "Between the faces' outward normals: 180 faces them at each other",
                    );
                    ui.label(
                        RichText::new(
                            "Only the turn is held: pair it with a mate or an alignment to \
                             say where the body sits.",
                        )
                        .font(sans(FONT_SM))
                        .color(TEXT2),
                    );
                }
                JointKind::Ground => {
                    ui.label(
                        RichText::new(
                            "The body stays where it is; the bodies joined to it are \
                             placed against it.",
                        )
                        .font(sans(FONT_SM))
                        .color(TEXT2),
                    );
                }
                JointKind::Align { turn, slide, .. } => {
                    let (turn_now, slide_now) = align_now.unzip();
                    overline(ui, "Turn");
                    changed |= drive_rows(
                        ui,
                        (document, id, &mut formula_edits),
                        ("/kind/Align/turn", true, false),
                        turn,
                        (turn_now, dt),
                        playing,
                        &mut record,
                    );
                    ui.add_space(SPACE_1);
                    overline(ui, "Slide");
                    changed |= drive_rows(
                        ui,
                        (document, id, &mut formula_edits),
                        ("/kind/Align/slide", false, false),
                        slide,
                        (slide_now, dt),
                        playing,
                        &mut record,
                    );
                    note(
                        ui,
                        "The body can turn about the axis and slide along it, each \
                         free, held or kept within limits.",
                    );
                }
                JointKind::Hinge { offset, drive, .. } => {
                    changed |= number_row(
                        ui,
                        (document, id, &mut formula_edits),
                        (
                            "Height",
                            "How far along the axis the body sits from the other",
                        ),
                        "/kind/Hinge/offset",
                        core_document::expr::Dim::LENGTH,
                        offset,
                    );
                    changed |= drive_rows(
                        ui,
                        (document, id, &mut formula_edits),
                        ("/kind/Hinge/drive", true, true),
                        drive,
                        (now, dt),
                        playing,
                        &mut record,
                    );
                    note(
                        ui,
                        "The body can only turn about the axis. Its angle counts from \
                         where it sat when the joint was made.",
                    );
                }
                JointKind::Distance { offset } => {
                    changed |= number_row(
                        ui,
                        (document, id, &mut formula_edits),
                        ("Distance", "Along the other face's normal"),
                        "/kind/Distance/offset",
                        core_document::expr::Dim::LENGTH,
                        offset,
                    );
                    note(
                        ui,
                        "Only the distance is held: the faces may turn and slide past \
                         each other.",
                    );
                }
                JointKind::Tangent { radius } => {
                    changed |= number_row(
                        ui,
                        (document, id, &mut formula_edits),
                        ("Radius", "The round face's radius"),
                        "/kind/Tangent/radius",
                        core_document::expr::Dim::LENGTH,
                        radius,
                    );
                    note(
                        ui,
                        "The round face rests on the flat one; it can still roll and \
                         slide along it.",
                    );
                }
                JointKind::Slider { drive, .. } => {
                    changed |= drive_rows(
                        ui,
                        (document, id, &mut formula_edits),
                        ("/kind/Slider/drive", false, true),
                        drive,
                        (now, dt),
                        playing,
                        &mut record,
                    );
                    note(
                        ui,
                        "The body can only slide along the axis, turned as it was when \
                         the joint was made.",
                    );
                }
                JointKind::Fixed { shift, .. } => {
                    for (k, label) in ["Shift x", "Shift y", "Shift z"].into_iter().enumerate() {
                        let mut value = shift[k] as f32;
                        if number_row(
                            ui,
                            (document, id, &mut formula_edits),
                            (
                                label,
                                "Where the body sits from the other, along the other's own axes",
                            ),
                            &format!("/kind/Fixed/shift/{k}"),
                            core_document::expr::Dim::LENGTH,
                            &mut value,
                        ) {
                            shift[k] = f64::from(value);
                            changed = true;
                        }
                    }
                    note(
                        ui,
                        "The body is held to the other as it sat when the joint was made; \
                         it moves only with it.",
                    );
                }
                JointKind::Parallel => note(
                    ui,
                    "Only the turn is held, the faces parallel: pair it with other joints \
                     to say where the body sits.",
                ),
                JointKind::Perpendicular => note(
                    ui,
                    "Only the turn is held, the faces square: pair it with other joints \
                     to say where the body sits.",
                ),
                JointKind::Ball => note(
                    ui,
                    "The two points are held as one; the body can still turn every way \
                     about them.",
                ),
                JointKind::Universal => note(
                    ui,
                    "The pins cross at one point, square to each other: the body turns \
                     about either pin.",
                ),
                JointKind::Slot => note(
                    ui,
                    "The pin stays on the slot's line: it slides along it and turns every way.",
                ),
                JointKind::Width => note(
                    ui,
                    "The tab's two faces stay centred between the slot's two walls: it \
                     slides along the slot and turns in it.",
                ),
                JointKind::Path => note(
                    ui,
                    "The point stays on the edge, whatever its shape: it runs along it and \
                     turns every way.",
                ),
                JointKind::Cam { radius } => {
                    changed |= number_row(
                        ui,
                        (document, id, &mut formula_edits),
                        ("Radius", "The follower's roller radius; 0 for a point"),
                        "/kind/Cam/radius",
                        core_document::expr::Dim::LENGTH,
                        radius,
                    );
                    note(ui, "The follower stays on the cam's face, a radius off it.");
                }
            }
            if joint.kind != JointKind::Ground {
                ui.add_space(SPACE_1);
                for (end, label) in [(0, "Moving end"), (1, "Fixed end")] {
                    changed |= number_row(
                        ui,
                        (document, id, &mut formula_edits),
                        (
                            label,
                            "How far this end moves along its own normal or axis before the \
                             joint holds it",
                        ),
                        &format!("/ends/{end}"),
                        core_document::expr::Dim::LENGTH,
                        &mut joint.ends[end],
                    );
                }
            }
        });
        for (key, formula) in formula_edits {
            if let Err(why) = ctx.document.set_feature_formula(id, &key, formula) {
                ctx.log_warn(why.to_string());
            }
        }
        if changed {
            if let Err(why) = ctx.document.update_feature_data(id, joint.to_json()) {
                ctx.log_warn(why.to_string());
            }
            ctx.document.clear_feature_dirty(id);
            self.solve_and_apply(ctx);
        }
        if let Some((low, high)) = record {
            let frames = crate::sweep_frames(ctx.document, id, low, high, SWEEP_FRAMES);
            if frames.is_empty() {
                ctx.log_warn("Nothing moves through this joint's range");
            } else {
                ctx.request(core_document::HostRequest::RecordAnimation {
                    name: node.name.clone(),
                    frames,
                    frame_ms: 4000 / SWEEP_FRAMES as u32,
                });
            }
        }
        if let JointKind::Hinge { drive, .. } | JointKind::Slider { drive, .. } = joint.kind {
            let hinge = matches!(joint.kind, JointKind::Hinge { .. });
            let (low, high) = match drive.limits {
                Some([low, high]) => (low, high),
                None if hinge => (-180.0, 180.0),
                None => {
                    let at = now.unwrap_or(0.0) as f32;
                    (at - 25.0, at + 25.0)
                }
            };
            self.motion_section(ui, ctx, id, (low, high), if hinge { "°" } else { " mm" });
        }
        if joint.kind != JointKind::Ground {
            ui.add_space(SPACE_2);
            let mut by = self.turn_by.unwrap_or(90.0);
            let mut turn = None;
            ui.horizontal(|ui| {
                if QtyField::degrees(&mut by).width(70.0).show(ui) {
                    self.turn_by = Some(by);
                }
                if ui_kit::widgets::secondary_button(ui, "Turn")
                    .on_hover_text("Turn the body about the joint's axis or normal by this much")
                    .clicked()
                {
                    turn = Some((f64::from(by), false));
                }
                if ui_kit::widgets::secondary_button(ui, "Turn over")
                    .on_hover_text("Half a turn across the joint: the body the other way round")
                    .clicked()
                {
                    turn = Some((0.0, true));
                }
            });
            if let Some((degrees, over)) = turn {
                self.turn_in_task(ctx, id, created, placements, degrees, over);
            }
        }
        ui.add_space(SPACE_2);
        self.verdict_card(ui);
        if self.redundant_now(ctx).iter().any(|(j, _)| *j == id) {
            note_card(
                ui,
                Note::Warning,
                Some("Redundant"),
                "The body's other joints already hold everything this one does; it could go.",
            );
        }
        if let Some(body) = node.body {
            freedom_line(ui, ctx, body);
        }
        ui.add_space(SPACE_2);
        if destructive_button(ui, "Delete joint")
            .on_hover_text("Remove the joint; the bodies stay where they are")
            .clicked()
            && ctx.document.remove_feature(id).is_ok()
        {
            self.playing = None;
            // A joint the task made has nothing to undo in a recording.
            if !created {
                ctx.record(
                    "doc.delete",
                    crate::commands::object(serde_json::json!({"id": id.0.to_string()})),
                    serde_json::Value::Null,
                );
            }
            self.task = None;
            ctx.active_document_object = None;
            return TaskOutcome::Accepted {
                label: "Delete joint".to_string(),
            };
        }
        TaskOutcome::Open
    }

    /// The check of a joint's motion for collisions: its button, its
    /// progress, what it found.
    fn motion_section(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &mut WorkbenchRuntimeContext,
        id: FeatureId,
        (low, high): (f32, f32),
        unit: &str,
    ) {
        ui.add_space(SPACE_2);
        self.collect_sweep(ctx);
        match self.sweep_progress() {
            Some((joint, done, total)) if joint == id => {
                ui.label(
                    RichText::new(format!("Checking the motion: {done} of {total} pairs"))
                        .font(sans(FONT_SM))
                        .color(TEXT1),
                );
                ui.add(egui::ProgressBar::new(if total == 0 {
                    0.0
                } else {
                    done as f32 / total as f32
                }));
                if ui_kit::widgets::secondary_button(ui, "Stop").clicked() {
                    self.sweeping = None;
                }
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(100));
                return;
            }
            _ => {
                if ui_kit::widgets::secondary_button(ui, "Check collisions through the motion")
                    .on_hover_text(format!(
                        "Step the drive from {low:.1}{unit} to {high:.1}{unit} and look for \
                         bodies that share material on the way"
                    ))
                    .clicked()
                {
                    self.check_sweep(ctx, id, (low, high));
                }
            }
        }
        let Some((joint, found)) = &self.motion_clashes else {
            return;
        };
        if *joint != id {
            return;
        }
        match found {
            Ok(found) if found.is_empty() => {
                note_card(ui, Note::Success, None, "No collisions through the motion");
            }
            Ok(found) => {
                note_card(
                    ui,
                    Note::Error,
                    Some(&format!(
                        "{} collision{}",
                        found.len(),
                        if found.len() == 1 { "" } else { "s" }
                    )),
                    "Click one to select its first body",
                );
                for clash in found.clone() {
                    let text = format!(
                        "At {:.1}{unit}: {} and {}, {:.2} mm³",
                        clash.at,
                        body_name(ctx, clash.a),
                        body_name(ctx, clash.b),
                        clash.volume_mm3
                    );
                    let row = ui.add(
                        egui::Button::new(RichText::new(text).font(sans(FONT_SM)).color(TEXT1))
                            .frame(false),
                    );
                    if row.clicked() {
                        ctx.request(core_document::HostRequest::SelectBody(clash.a));
                    }
                }
            }
            Err(why) => {
                note_card(ui, Note::Error, Some("Not checked"), why);
            }
        }
    }

    /// Make the joint another kind, from where the bodies stand, as
    /// `asm.set` with a `kind` does.
    fn change_kind(
        &mut self,
        ctx: &mut WorkbenchRuntimeContext,
        id: FeatureId,
        created: bool,
        placements: &[(BodyId, BodyPlacement)],
        tool: JointTool,
    ) {
        if created {
            crate::commands::record_joint(ctx, id, None, placements);
        }
        let args = crate::commands::object(
            serde_json::json!({"joint": id.0.to_string(), "kind": tool.word()}),
        );
        match crate::commands::run("asm.set", &args, ctx) {
            Ok(_) => ctx.record("asm.set", args, serde_json::Value::Null),
            Err(why) => ctx.log_warn(why.to_string()),
        }
        self.task = Some(crate::Task::Joint {
            id,
            before: ctx.document.get_feature_data(id).cloned(),
            placements: crate::all_placements(ctx),
        });
    }

    /// Turn the joint's body from its task. A joint the task just made is
    /// recorded first, so the recording has it to turn; the task goes on
    /// as an edit of it from the turned place.
    fn turn_in_task(
        &mut self,
        ctx: &mut WorkbenchRuntimeContext,
        id: FeatureId,
        created: bool,
        placements: &[(BodyId, BodyPlacement)],
        degrees: f64,
        over: bool,
    ) {
        let Some(joint) = crate::joints(ctx.document).into_iter().find(|j| j.id == id) else {
            return;
        };
        if created {
            crate::commands::record_joint(ctx, id, None, placements);
        }
        let (command, args) = if over {
            (
                "asm.flip",
                serde_json::json!({"joint": joint.id.0.to_string()}),
            )
        } else {
            (
                "asm.turn",
                serde_json::json!({"joint": joint.id.0.to_string(), "degrees": degrees}),
            )
        };
        // As the command turns it: a hinge's motion moves on, a coupled
        // one takes its partner along.
        let args = crate::commands::object(args);
        if let Err(why) = crate::commands::run(command, &args, ctx) {
            ctx.log_warn(format!("Could not turn the body: {why}"));
            return;
        }
        self.solve_and_apply(ctx);
        ctx.record(command, args, serde_json::Value::Null);
        self.task = Some(crate::Task::Joint {
            id,
            before: ctx.document.get_feature_data(id).cloned(),
            placements: crate::all_placements(ctx),
        });
    }

    /// A coupling's settings: its two joints, what ties them, the ratio.
    /// Each change is made as `asm.set` makes it and solves at once.
    fn coupling_panel(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &mut WorkbenchRuntimeContext,
        request: TaskRequest,
        id: FeatureId,
        before: Option<serde_json::Value>,
        placements: &[(BodyId, BodyPlacement)],
    ) -> TaskOutcome {
        let created = before.is_none();
        if request.cancel {
            match &before {
                Some(data) => {
                    if let Err(why) = ctx.document.update_feature_data(id, data.clone()) {
                        ctx.log_warn(why.to_string());
                    }
                    ctx.document.clear_feature_dirty(id);
                }
                None => {
                    if let Err(why) = ctx.document.remove_feature(id) {
                        ctx.log_warn(why.to_string());
                    }
                    ctx.active_document_object = None;
                }
            }
            restore_placements(ctx, placements);
            self.task = None;
            return TaskOutcome::Cancelled;
        }
        if request.accept {
            crate::commands::record_coupling(ctx, id, before.as_ref());
            self.task = None;
            ctx.active_document_object = None;
            return TaskOutcome::Accepted {
                label: if created {
                    "Add coupling"
                } else {
                    "Edit coupling"
                }
                .to_string(),
            };
        }
        let Some(node) = ctx.document.get_feature_meta(id).cloned() else {
            self.task = None;
            return TaskOutcome::Cancelled;
        };
        let Ok(coupling) = Coupling::from_json(&node.data) else {
            note_card(
                ui,
                Note::Error,
                Some("Unreadable coupling"),
                "The stored coupling does not parse.",
            );
            return TaskOutcome::Open;
        };
        header(ui, "involute-gear", &node.name);
        ui.add_space(SPACE_2);
        // Only hinges and sliders move in a way a coupling can tie.
        let movable: Vec<crate::Joint> = crate::joints(ctx.document)
            .into_iter()
            .filter(|j| {
                matches!(
                    j.feature.kind,
                    JointKind::Hinge { .. } | JointKind::Slider { .. }
                )
            })
            .collect();
        let named = |id: FeatureId| {
            movable
                .iter()
                .find(|j| j.id == id)
                .map_or("a removed joint".to_string(), |j| {
                    format!("{} ({})", j.name, j.feature.kind.label())
                })
        };
        let kind_of = |id: FeatureId| movable.iter().find(|j| j.id == id).map(|j| j.feature.kind);
        let mut args = serde_json::Map::new();
        let document: &core_document::Document = ctx.document;
        let mut formula_edits: Vec<(String, Option<String>)> = Vec::new();
        let mut ratio = coupling.ratio;
        let mut reverse = coupling.reverse;
        Card::new().padding(SPACE_3).show(ui, |ui| {
            ui.set_width(ui.available_width());
            for (label, key, current) in [
                ("Driving", "driver", coupling.driver),
                ("Driven", "driven", coupling.driven),
            ] {
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [90.0, INPUT],
                        egui::Label::new(RichText::new(label).font(sans(FONT_SM)).color(TEXT2)),
                    );
                    egui::ComboBox::from_id_salt(("coupling_joint", id, key))
                        .selected_text(RichText::new(named(current)).font(sans(FONT_SM)))
                        .width(ui.available_width() - 2.0 * ui.spacing().button_padding.x)
                        .show_ui(ui, |ui| {
                            for joint in &movable {
                                let on = joint.id == current;
                                if ui
                                    .selectable_label(
                                        on,
                                        RichText::new(named(joint.id)).font(sans(FONT_SM)),
                                    )
                                    .clicked()
                                    && !on
                                {
                                    args.insert(key.into(), joint.id.0.to_string().into());
                                }
                            }
                        });
                });
            }
            let (driver, driven) = (kind_of(coupling.driver), kind_of(coupling.driven));
            ui.horizontal(|ui| {
                ui.add_sized(
                    [90.0, INPUT],
                    egui::Label::new(RichText::new("Kind").font(sans(FONT_SM)).color(TEXT2)),
                );
                egui::ComboBox::from_id_salt(("coupling_kind", id))
                    .selected_text(RichText::new(coupling.gearing.label()).font(sans(FONT_SM)))
                    .width(ui.available_width() - 2.0 * ui.spacing().button_padding.x)
                    .show_ui(ui, |ui| {
                        for gearing in Gearing::ALL {
                            let fits = driver
                                .zip(driven)
                                .is_some_and(|(a, b)| gearing.fits(&a, &b));
                            let on = gearing == coupling.gearing;
                            let clicked = ui
                                .add_enabled(
                                    fits,
                                    egui::Button::selectable(
                                        on,
                                        RichText::new(gearing.label()).font(sans(FONT_SM)),
                                    ),
                                )
                                .on_hover_text(gearing.summary())
                                .clicked();
                            if clicked && !on {
                                args.insert("gearing".into(), gearing.word().into());
                            }
                        }
                    });
            });
            let (ratio_label, length) = coupling.gearing.ratio_label();
            let dim = if length {
                core_document::expr::Dim::LENGTH
            } else {
                core_document::expr::Dim::NUMBER
            };
            if number_row(
                ui,
                (document, id, &mut formula_edits),
                (ratio_label, coupling.gearing.summary()),
                "/ratio",
                dim,
                &mut ratio,
            ) && ratio > 0.0
            {
                args.insert("ratio".into(), serde_json::json!(ratio));
            }
            if check_row(ui, &mut reverse, "Reverse")
                .on_hover_text("The driven joint moves the other way")
                .changed()
            {
                args.insert("reverse".into(), reverse.into());
            }
            note(
                ui,
                "Move the driving joint (drag its body, or drive it) and the driven \
                 one follows.",
            );
        });
        for (key, formula) in formula_edits {
            if let Err(why) = ctx.document.set_feature_formula(id, &key, formula) {
                ctx.log_warn(why.to_string());
            }
        }
        if !args.is_empty() {
            args.insert("joint".into(), id.0.to_string().into());
            match crate::commands::run("asm.set", &args, ctx) {
                Ok(_) => self.verdict = None,
                Err(err) => self.verdict = Some(Err(err.to_string())),
            }
        }
        ui.add_space(SPACE_2);
        self.verdict_card(ui);
        if let Some(body) = node.body {
            freedom_line(ui, ctx, body);
        }
        ui.add_space(SPACE_2);
        if destructive_button(ui, "Delete coupling")
            .on_hover_text("Remove the coupling; the joints move apart again")
            .clicked()
            && ctx.document.remove_feature(id).is_ok()
        {
            if !created {
                ctx.record(
                    "doc.delete",
                    crate::commands::object(serde_json::json!({"id": id.0.to_string()})),
                    serde_json::Value::Null,
                );
            }
            self.task = None;
            ctx.active_document_object = None;
            return TaskOutcome::Accepted {
                label: "Delete coupling".to_string(),
            };
        }
        TaskOutcome::Open
    }

    fn move_panel(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &mut WorkbenchRuntimeContext,
        request: TaskRequest,
        body: BodyId,
        placements: &[(BodyId, BodyPlacement)],
    ) -> TaskOutcome {
        if request.cancel {
            restore_placements(ctx, placements);
            self.task = None;
            return TaskOutcome::Cancelled;
        }
        if request.accept {
            let placement = ctx.document.body_placement(body);
            ctx.record(
                "asm.place",
                crate::commands::object(serde_json::json!({
                    "body": body.0.to_string(),
                    "translation": placement.translation,
                    "rotation": placement.rotation,
                })),
                serde_json::Value::Null,
            );
            self.task = None;
            return TaskOutcome::Accepted {
                label: "Move body".to_string(),
            };
        }
        header(ui, "move-geometry", &body_name(ctx, body));
        ui.add_space(SPACE_2);
        let held = Self::held_by_joints(ctx, body);
        if held {
            note_card(
                ui,
                Note::Info,
                Some("Placed by its joints"),
                "Its joints decide where it sits. Move the body it is joined to, \
                 or delete a joint to free it.",
            );
        }
        let placement = ctx.document.body_placement(body);
        let mut offset = placement.translation;
        let (ax, ay, az) = placement.quat().to_euler(glam::EulerRot::XYZ);
        let mut angles = [ax, ay, az].map(f32::to_degrees);
        let mut changed = false;
        Card::new().padding(SPACE_3).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add_enabled_ui(!held, |ui| {
                for (axis, value) in ["X", "Y", "Z"].iter().zip(offset.iter_mut()) {
                    ui.horizontal(|ui| {
                        ui.add_sized(
                            [90.0, INPUT],
                            egui::Label::new(
                                RichText::new(format!("Position {axis}"))
                                    .font(sans(FONT_SM))
                                    .color(TEXT2),
                            ),
                        );
                        changed |= QtyField::offset(value).show(ui);
                    });
                }
                for (axis, value) in ["X", "Y", "Z"].iter().zip(angles.iter_mut()) {
                    ui.horizontal(|ui| {
                        ui.add_sized(
                            [90.0, INPUT],
                            egui::Label::new(
                                RichText::new(format!("Turn about {axis}"))
                                    .font(sans(FONT_SM))
                                    .color(TEXT2),
                            ),
                        );
                        changed |= QtyField::degrees(value).range(-180.0..=180.0).show(ui);
                    });
                }
            });
        });
        if changed {
            let [ax, ay, az] = angles.map(f32::to_radians);
            let moved = BodyPlacement::new(
                glam::Quat::from_euler(glam::EulerRot::XYZ, ax, ay, az),
                glam::Vec3::from_array(offset),
            );
            crate::components::move_with_unit(ctx.document, body, moved);
            // Whatever is joined to it follows.
            self.solve_and_apply(ctx);
        }
        ui.add_space(SPACE_2);
        if !held
            && ui_kit::widgets::secondary_button(ui, "Back to where it was made")
                .on_hover_text("No move, no turn")
                .clicked()
        {
            crate::components::move_with_unit(ctx.document, body, BodyPlacement::IDENTITY);
            self.solve_and_apply(ctx);
        }
        self.verdict_card(ui);
        TaskOutcome::Open
    }
}

/// A hinge's or a slider's drive: held at a value, kept within limits,
/// and, while held, swept through its range to show the motion.
fn drive_rows(
    ui: &mut egui::Ui,
    (document, joint, edits): (
        &core_document::Document,
        core_document::FeatureId,
        &mut Vec<(String, Option<String>)>,
    ),
    (base, angular, sweep): (&str, bool, bool),
    drive: &mut crate::Drive,
    (now, dt): (Option<f64>, f64),
    playing: &mut Option<crate::Play>,
    record: &mut Option<(f32, f32)>,
) -> bool {
    use core_document::expr::Dim;
    let (dim, unit) = if angular {
        (Dim::ANGLE, "°")
    } else {
        (Dim::LENGTH, " mm")
    };
    let mut changed = false;
    if let Some(now) = now {
        row(
            ui,
            if angular { "Angle now" } else { "Position now" },
            &format!("{now:.2}{unit}"),
        );
    }
    let to_key = format!("{base}/to");
    let mut driven = drive.to.is_some();
    if check_row(ui, &mut driven, "Drive")
        .on_hover_text(if angular {
            "Hold the hinge at an angle"
        } else {
            "Hold the slider at a position"
        })
        .changed()
    {
        drive.to = driven.then(|| now.unwrap_or(0.0) as f32);
        if !driven {
            edits.push((to_key.clone(), None));
            *playing = None;
        }
        changed = true;
    }
    if let Some(to) = &mut drive.to {
        changed |= number_row(
            ui,
            (document, joint, edits),
            (
                if angular { "Angle" } else { "Position" },
                "Where the drive holds it",
            ),
            &to_key,
            dim,
            to,
        );
    }
    let mut limited = drive.limits.is_some();
    if check_row(ui, &mut limited, "Limits")
        .on_hover_text("Keep the motion within a range while it is not driven")
        .changed()
    {
        let at = now.unwrap_or(0.0) as f32;
        drive.limits = limited.then(|| {
            if angular {
                [(at - 45.0).max(-180.0), (at + 45.0).min(180.0)]
            } else {
                [at - 10.0, at + 10.0]
            }
        });
        if !limited {
            for end in 0..2 {
                edits.push((format!("{base}/limits/{end}"), None));
            }
        }
        changed = true;
    }
    if let Some([low, high]) = &mut drive.limits {
        for (end, value, label) in [(0, &mut *low, "Lowest"), (1, &mut *high, "Highest")] {
            changed |= number_row(
                ui,
                (document, joint, edits),
                (label, "An end of the range the motion stays in"),
                &format!("{base}/limits/{end}"),
                dim,
                value,
            );
        }
        if *low > *high {
            std::mem::swap(low, high);
        }
    }
    let limits = drive.limits;
    let Some(to) = &mut drive.to else {
        return changed;
    };
    if !sweep {
        return changed;
    }
    // The sweep: through the limits, or a whole turn, or 25 mm either side
    // of where it started.
    let mine = playing.filter(|p| p.joint == joint);
    let centre = f64::from(mine.map_or(*to, |p| p.start));
    let (low, high) = match limits {
        Some([low, high]) => (f64::from(low), f64::from(high)),
        None if angular => (-179.0, 179.0),
        None => (centre - 25.0, centre + 25.0),
    };
    let label = if mine.is_some() { "Stop" } else { "Play" };
    if ui_kit::widgets::secondary_button(ui, "Record")
        .on_hover_text("Save the sweep through its range as an animation")
        .clicked()
    {
        *record = Some((low as f32, high as f32));
    }
    if document.feature_formula(joint, &to_key).is_some() {
        // A formula holds the value; a sweep would fight it every frame.
        *playing = playing.filter(|p| p.joint != joint);
    } else if ui_kit::widgets::secondary_button(ui, label)
        .on_hover_text("Sweep the drive through its range; stopping puts it back")
        .clicked()
    {
        match mine {
            Some(p) => {
                *to = p.start;
                *playing = None;
            }
            None => {
                let span = (high - low).max(1e-6);
                let from = ((f64::from(*to) - low) / span).clamp(0.0, 1.0);
                *playing = Some(crate::Play {
                    joint,
                    start: *to,
                    phase: (1.0 - 2.0 * from).acos(),
                });
            }
        }
        changed = true;
    } else if let Some(play) = playing.as_mut().filter(|p| p.joint == joint) {
        // Back and forth every four seconds.
        play.phase += dt * std::f64::consts::TAU / 4.0;
        *to = (low + (high - low) * (0.5 - 0.5 * play.phase.cos())) as f32;
        changed = true;
        ui.ctx().request_repaint();
    }
    changed
}

/// A labelled number a formula can set, in a joint's settings.
fn number_row(
    ui: &mut egui::Ui,
    (document, joint, edits): (
        &core_document::Document,
        core_document::FeatureId,
        &mut Vec<(String, Option<String>)>,
    ),
    (label, hover): (&str, &str),
    key: &str,
    dim: core_document::expr::Dim,
    value: &mut f32,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.add_sized(
            [90.0, INPUT],
            egui::Label::new(RichText::new(label).font(sans(FONT_SM)).color(TEXT2)),
        );
        changed = formula_field(ui, document, joint, key, dim, value, edits);
    })
    .response
    .on_hover_text(hover);
    changed
}

fn note(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).font(sans(FONT_SM)).color(TEXT2));
}

/// A joint's number as a formula field: a value typed or dragged goes into
/// `value`; a formula goes into `edits` and what it comes to into `value`,
/// so the body moves while the panel is open.
fn formula_field(
    ui: &mut egui::Ui,
    document: &core_document::Document,
    joint: core_document::FeatureId,
    key: &str,
    dim: core_document::expr::Dim,
    value: &mut f32,
    edits: &mut Vec<(String, Option<String>)>,
) -> bool {
    let formula = document.feature_formula(joint, key);
    let slot = document
        .evaluated_slots(joint)
        .iter()
        .find(|s| s.key == key);
    let shown = match (formula, slot.map(|s| &s.result)) {
        (Some(_), Some(Ok(q))) => q.value,
        _ => f64::from(*value),
    };
    let host = core_document::DocumentFormulas { document, dim };
    let angle = dim == core_document::expr::Dim::ANGLE;
    let edit = ui_kit::widgets::FormulaField::new(
        egui::Id::new(("joint_field", joint, key)),
        shown,
        &host,
    )
    .formula(formula)
    .error(
        slot.and_then(|s| s.result.as_ref().err())
            .map(String::as_str),
    )
    .unit(if angle {
        "°"
    } else if dim == core_document::expr::Dim::LENGTH {
        "mm"
    } else {
        ""
    })
    .speed(if angle { 1.0 } else { 0.1 })
    .show(ui);
    match edit {
        Some(ui_kit::widgets::FormulaEdit::Value(v)) => {
            if formula.is_some() {
                edits.push((key.to_string(), None));
            }
            *value = v as f32;
            true
        }
        Some(ui_kit::widgets::FormulaEdit::Formula(text)) => {
            let now = document.evaluate_formula(&text, Some(dim));
            edits.push((key.to_string(), Some(text)));
            match now {
                Ok(q) => {
                    *value = q.value as f32;
                    true
                }
                Err(_) => false,
            }
        }
        None => false,
    }
}

/// What `body` may still do, its joints holding: "fully placed", or its
/// free motions.
fn freedom_line(ui: &mut egui::Ui, ctx: &WorkbenchRuntimeContext, body: BodyId) {
    let Some((_, motions)) = crate::freedom(ctx.document)
        .into_iter()
        .find(|(b, _)| *b == body)
    else {
        return;
    };
    let text = if motions.is_empty() {
        "Its joints place this body fully.".to_string()
    } else {
        let words: Vec<String> = motions.iter().map(crate::Motion::describe).collect();
        format!("It may still {}.", words.join(", "))
    };
    ui.add_space(SPACE_1);
    ui.add(egui::Label::new(RichText::new(text).font(sans(FONT_SM)).color(TEXT2)).wrap());
}

/// A mass in grams, or kilograms from a thousand.
fn mass_text(grams: f64) -> String {
    if grams >= 1000.0 {
        format!("{:.3} kg", grams / 1000.0)
    } else {
        format!("{grams:.2} g")
    }
}

/// Curves over time, each scaled to its own range: the first `speeds` in
/// the accent colour (traced points' speeds), the rest (driven joints'
/// values) in the second; a line where the frame shown stands.
fn plot(ui: &mut egui::Ui, curves: &[Vec<(f32, f32)>], speeds: usize, at: Option<f32>) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::Vec2::new(width, 90.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, BG1);
    let (t0, t1) = curves
        .iter()
        .flatten()
        .fold((f32::MAX, f32::MIN), |(lo, hi), (t, _)| {
            (lo.min(*t), hi.max(*t))
        });
    let span = (t1 - t0).max(1e-6);
    let x = |t: f32| rect.left() + rect.width() * (t - t0) / span;
    for (i, curve) in curves.iter().enumerate() {
        let (lo, hi) = curve.iter().fold((f32::MAX, f32::MIN), |(lo, hi), (_, v)| {
            (lo.min(*v), hi.max(*v))
        });
        let range = (hi - lo).max(1e-6);
        let points: Vec<egui::Pos2> = curve
            .iter()
            .map(|(t, v)| {
                egui::pos2(
                    x(*t),
                    rect.bottom() - 4.0 - (rect.height() - 8.0) * (v - lo) / range,
                )
            })
            .collect();
        let color = if i < speeds { ACCENT } else { WARNING };
        painter.add(egui::Shape::line(points, egui::Stroke::new(1.5, color)));
    }
    if let Some(t) = at {
        painter.line_segment(
            [
                egui::pos2(x(t), rect.top()),
                egui::pos2(x(t), rect.bottom()),
            ],
            egui::Stroke::new(1.0, TEXT3),
        );
    }
    ui.horizontal(|ui| {
        if speeds > 0 {
            ui.label(
                RichText::new("speed")
                    .font(ui_kit::mono(FONT_SM))
                    .color(ACCENT),
            );
        }
        if curves.len() > speeds {
            ui.label(
                RichText::new("drives")
                    .font(ui_kit::mono(FONT_SM))
                    .color(WARNING),
            );
        }
    });
}
