//! The application's own tasks in the right panel, beside the benches':
//! a body's placement, its appearance (colour, faces, see-through,
//! material) and a feature's place in its body's history. Edits apply to
//! the document as they are made, so the view shows them; OK keeps them as
//! one undo step, Cancel puts back what was there when the task opened.

use core_document::{
    BodyDisplay, BodyId, BodyPlacement, Document, FaceColor, FeatureId, Material, Recorded,
};
use egui::{RichText, Ui, Vec2};
use serde_json::{Value, json};
use ui_kit::tokens::*;
use ui_kit::widgets::{QtyField, overline, secondary_button, small_secondary_button};
use ui_kit::{sans, sans_semibold};

use super::UiCommand;

/// The colours offered first, a row of neutrals and two of hues.
pub const PALETTE: &[[f32; 3]] = &[
    [0.97, 0.97, 0.97],
    [0.78, 0.79, 0.81],
    [0.58, 0.61, 0.66],
    [0.38, 0.40, 0.43],
    [0.20, 0.21, 0.23],
    [0.06, 0.06, 0.07],
    [0.86, 0.20, 0.20],
    [0.95, 0.50, 0.15],
    [0.96, 0.80, 0.18],
    [0.55, 0.78, 0.25],
    [0.20, 0.62, 0.32],
    [0.16, 0.66, 0.66],
    [0.20, 0.58, 0.90],
    [0.22, 0.34, 0.80],
    [0.52, 0.34, 0.84],
    [0.84, 0.36, 0.72],
    [0.93, 0.62, 0.66],
    [0.55, 0.38, 0.22],
];

/// Materials offered, densities in g/cm³.
pub const MATERIALS: &[(&str, f32)] = &[
    ("PLA", 1.24),
    ("PETG", 1.27),
    ("ABS", 1.04),
    ("ASA", 1.07),
    ("TPU", 1.21),
    ("Nylon", 1.14),
    ("Polycarbonate", 1.20),
    ("Resin", 1.15),
    ("Aluminium", 2.70),
    ("Steel", 7.85),
    ("Stainless steel", 8.00),
    ("Brass", 8.50),
    ("Copper", 8.96),
    ("Wood", 0.60),
];

/// A task the application runs in the right panel.
#[derive(Debug, Clone)]
pub enum HostTask {
    Placement(PlacementTask),
    Appearance(AppearanceTask),
    History(HistoryTask),
}

#[derive(Debug, Clone)]
pub struct PlacementTask {
    body: BodyId,
    /// Every body that moves with it, where it was when the task opened.
    start: Vec<(BodyId, BodyPlacement)>,
    offset: [f32; 3],
    /// Turn about X, then Y, then Z, degrees.
    angles: [f32; 3],
}

#[derive(Debug, Clone)]
pub struct AppearanceTask {
    body: BodyId,
    start_display: Option<BodyDisplay>,
    start_material: Option<Material>,
    start_faces: Vec<FaceColor>,
    /// The colour the custom picker holds.
    custom: [f32; 3],
    /// The material is typed in rather than picked from the list.
    own_material: bool,
    /// The face the task was opened for, until one is selected in the view.
    face: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct HistoryTask {
    feature: FeatureId,
    /// What it came after when the task opened; `None` when first.
    start_after: Option<FeatureId>,
    /// Why the last move was refused.
    refused: Option<String>,
}

/// How a task ended.
pub enum HostTaskEnd {
    /// Kept: the undo step's label and the calls a recording keeps.
    Accepted(String, Vec<Recorded>),
    Cancelled,
}

/// What a task reads of the moment.
pub struct HostTaskInputs<'a> {
    pub document: &'a mut Document,
    /// The face selected in the view: its body, index and name.
    pub picked_face: Option<(BodyId, u32)>,
    pub custom_colors: &'a [[f32; 3]],
}

fn call(id: &str, args: Value) -> Recorded {
    Recorded {
        id: id.to_string(),
        args: match args {
            Value::Object(map) => map,
            _ => Default::default(),
        },
        result: Value::Null,
    }
}

fn body_name(document: &Document, body: BodyId) -> String {
    document
        .bodies()
        .iter()
        .find(|b| b.id == body)
        .map(|b| b.name.clone())
        .unwrap_or_default()
}

fn color32(c: [f32; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(
        (c[0].clamp(0.0, 1.0) * 255.0) as u8,
        (c[1].clamp(0.0, 1.0) * 255.0) as u8,
        (c[2].clamp(0.0, 1.0) * 255.0) as u8,
    )
}

fn same_color(a: [f32; 3], b: [f32; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-3)
}

/// One colour to click, ringed when it is the one in use.
fn swatch(ui: &mut Ui, color: [f32; 3], chosen: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(22.0), egui::Sense::click());
    let painter = ui.painter();
    painter.rect_filled(rect.shrink(2.0), RADIUS_SM, color32(color));
    let stroke = if chosen {
        egui::Stroke::new(2.0, ACCENT)
    } else if response.hovered() {
        egui::Stroke::new(1.0, TEXT2)
    } else {
        egui::Stroke::new(1.0, BORDER)
    };
    painter.rect_stroke(
        rect.shrink(1.0),
        RADIUS_SM,
        stroke,
        egui::StrokeKind::Inside,
    );
    response
}

/// The palette, then the user's own colours; the one clicked.
fn swatches(ui: &mut Ui, current: Option<[f32; 3]>, custom: &[[f32; 3]]) -> Option<[f32; 3]> {
    let mut picked = None;
    let is = |c: [f32; 3]| current.is_some_and(|now| same_color(now, c));
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::splat(2.0);
        for color in PALETTE {
            if swatch(ui, *color, is(*color)).clicked() {
                picked = Some(*color);
            }
        }
    });
    if !custom.is_empty() {
        ui.add_space(SPACE_1);
        ui.label(
            RichText::new("Your colours")
                .font(sans(FONT_XS))
                .color(TEXT3),
        );
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(2.0);
            for color in custom {
                if swatch(ui, *color, is(*color)).clicked() {
                    picked = Some(*color);
                }
            }
        });
    }
    picked
}

fn note(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).font(sans(FONT_SM)).color(TEXT3));
}

fn heading(ui: &mut Ui, text: &str) {
    ui.add_space(SPACE_3);
    overline(ui, text);
    ui.add_space(SPACE_1);
}

impl HostTask {
    pub fn placement(document: &Document, body: BodyId) -> Self {
        let placement = document.body_placement(body);
        let (x, y, z) = placement.quat().to_euler(glam::EulerRot::XYZ);
        Self::Placement(PlacementTask {
            body,
            start: document
                .rigid_unit_of(body)
                .into_iter()
                .map(|b| (b, document.body_placement(b)))
                .collect(),
            offset: placement.offset().to_array(),
            angles: [x, y, z].map(f32::to_degrees),
        })
    }

    pub fn appearance(document: &Document, body: BodyId, face: Option<u32>) -> Self {
        let entry = document.bodies().iter().find(|b| b.id == body);
        let display = entry.and_then(|b| b.display);
        let material = entry.and_then(|b| b.material.clone());
        let own_material = material
            .as_ref()
            .is_some_and(|m| !MATERIALS.iter().any(|(name, _)| *name == m.name));
        Self::Appearance(AppearanceTask {
            body,
            start_display: display,
            start_material: material,
            start_faces: entry.map(|b| b.face_colors.clone()).unwrap_or_default(),
            custom: display.unwrap_or_default().color,
            own_material,
            face,
        })
    }

    pub fn history(document: &Document, feature: FeatureId) -> Self {
        let order = document.body_history_of(feature);
        let at = order.iter().position(|f| *f == feature).unwrap_or(0);
        Self::History(HistoryTask {
            feature,
            start_after: at.checked_sub(1).map(|i| order[i]),
            refused: None,
        })
    }

    /// The panel's title.
    pub fn title(&self, document: &Document) -> String {
        match self {
            Self::Placement(t) => format!("Placement · {}", body_name(document, t.body)),
            Self::Appearance(t) => format!("Appearance · {}", body_name(document, t.body)),
            Self::History(t) => format!(
                "Move · {}",
                document
                    .get_feature_meta(t.feature)
                    .map(|n| n.name.clone())
                    .unwrap_or_default()
            ),
        }
    }

    /// Whether what the task works on is still in the document.
    pub fn still_there(&self, document: &Document) -> bool {
        match self {
            Self::Placement(PlacementTask { body, .. })
            | Self::Appearance(AppearanceTask { body, .. }) => {
                document.bodies().iter().any(|b| b.id == *body)
            }
            Self::History(t) => document.get_feature_meta(t.feature).is_some(),
        }
    }

    /// Draw the task and apply its edits; how it ended, once it has.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        inputs: HostTaskInputs<'_>,
        accept: bool,
        cancel: bool,
        commands: &mut Vec<UiCommand>,
    ) -> Option<HostTaskEnd> {
        match self {
            Self::Placement(t) => t.show(ui, inputs.document, accept, cancel),
            Self::Appearance(t) => t.show(ui, inputs, accept, cancel, commands),
            Self::History(t) => t.show(ui, inputs.document, accept, cancel),
        }
    }
}

impl PlacementTask {
    fn placement(&self) -> BodyPlacement {
        let [x, y, z] = self.angles.map(f32::to_radians);
        BodyPlacement::new(
            glam::Quat::from_euler(glam::EulerRot::XYZ, x, y, z),
            glam::Vec3::from_array(self.offset),
        )
    }

    fn show(
        &mut self,
        ui: &mut Ui,
        document: &mut Document,
        accept: bool,
        cancel: bool,
    ) -> Option<HostTaskEnd> {
        if cancel {
            for (body, placement) in &self.start {
                document.set_body_placement(*body, *placement);
            }
            return Some(HostTaskEnd::Cancelled);
        }
        if accept {
            let placement = document.body_placement(self.body);
            return Some(HostTaskEnd::Accepted(
                format!("Place {}", body_name(document, self.body)),
                vec![call(
                    "doc.set_body",
                    json!({
                        "body": self.body.0.to_string(),
                        "translation": placement.translation,
                        "rotation": placement.rotation,
                    }),
                )],
            ));
        }
        note(
            ui,
            "Where the body's origin goes and how it is turned. The view follows as you type or drag.",
        );
        if self.start.len() > 1 {
            ui.add_space(SPACE_1);
            note(
                ui,
                &format!(
                    "{} other bodies of its component move with it.",
                    self.start.len() - 1
                ),
            );
        }
        let mut changed = false;
        heading(ui, "Position");
        egui::Grid::new("host_placement_position")
            .num_columns(2)
            .spacing([SPACE_3, SPACE_1])
            .show(ui, |ui| {
                for (i, axis) in ["X", "Y", "Z"].iter().enumerate() {
                    ui.label(RichText::new(*axis).font(sans(FONT_SM)).color(TEXT2));
                    changed |= QtyField::offset(&mut self.offset[i]).show(ui);
                    ui.end_row();
                }
            });
        heading(ui, "Turn");
        egui::Grid::new("host_placement_turn")
            .num_columns(2)
            .spacing([SPACE_3, SPACE_1])
            .show(ui, |ui| {
                for (i, axis) in ["About X", "About Y", "About Z"].iter().enumerate() {
                    ui.label(RichText::new(*axis).font(sans(FONT_SM)).color(TEXT2));
                    changed |= QtyField::degrees(&mut self.angles[i])
                        .range(-360.0..=360.0)
                        .show(ui);
                    ui.end_row();
                }
            });
        ui.add_space(SPACE_3);
        ui.horizontal(|ui| {
            if secondary_button(ui, "Where it was made").clicked() {
                self.offset = [0.0; 3];
                self.angles = [0.0; 3];
                changed = true;
            }
            if secondary_button(ui, "Where it was").clicked() {
                let start = self
                    .start
                    .iter()
                    .find(|(b, _)| *b == self.body)
                    .map(|(_, p)| *p)
                    .unwrap_or_default();
                let (x, y, z) = start.quat().to_euler(glam::EulerRot::XYZ);
                self.offset = start.offset().to_array();
                self.angles = [x, y, z].map(f32::to_degrees);
                changed = true;
            }
        });
        if changed {
            document.place_with_unit(self.body, self.placement());
        }
        None
    }
}

impl AppearanceTask {
    fn show(
        &mut self,
        ui: &mut Ui,
        inputs: HostTaskInputs<'_>,
        accept: bool,
        cancel: bool,
        commands: &mut Vec<UiCommand>,
    ) -> Option<HostTaskEnd> {
        let HostTaskInputs {
            document,
            picked_face,
            custom_colors,
        } = inputs;
        let body = self.body;
        if cancel {
            document.set_body_display(body, self.start_display);
            document.set_body_material(body, self.start_material.clone());
            let now: Vec<FaceColor> = document
                .bodies()
                .iter()
                .find(|b| b.id == body)
                .map(|b| b.face_colors.clone())
                .unwrap_or_default();
            for c in now {
                document.set_face_color(body, c.index, c.name, None);
            }
            for c in &self.start_faces {
                document.set_face_color(body, c.index, c.name, Some(c.color));
            }
            return Some(HostTaskEnd::Cancelled);
        }
        let Some(entry) = document.bodies().iter().find(|b| b.id == body).cloned() else {
            return Some(HostTaskEnd::Cancelled);
        };
        if accept {
            return Some(HostTaskEnd::Accepted(
                format!("Appearance of {}", entry.name),
                self.recorded(&entry),
            ));
        }

        // The body's colour.
        heading(ui, "Colour");
        let display = entry.display;
        if let Some(color) = swatches(ui, display.map(|d| d.color), custom_colors) {
            self.custom = color;
            document.set_body_display(
                body,
                Some(BodyDisplay {
                    color,
                    opacity: display.map_or(1.0, |d| d.opacity),
                }),
            );
        }
        ui.add_space(SPACE_2);
        ui.horizontal(|ui| {
            let before = self.custom;
            ui.label(RichText::new("Custom").font(sans(FONT_SM)).color(TEXT2));
            egui::color_picker::color_edit_button_rgb(ui, &mut self.custom);
            if self.custom != before {
                document.set_body_display(
                    body,
                    Some(BodyDisplay {
                        color: self.custom,
                        opacity: display.map_or(1.0, |d| d.opacity),
                    }),
                );
            }
            let kept = custom_colors.iter().any(|c| same_color(*c, self.custom));
            if !kept
                && small_secondary_button(ui, "Keep")
                    .on_hover_text("Add it to your colours, offered for every body")
                    .clicked()
            {
                let mut all = custom_colors.to_vec();
                all.push(self.custom);
                commands.push(UiCommand::SetCustomColors(all));
            }
            if kept
                && small_secondary_button(ui, "Forget")
                    .on_hover_text("Take it out of your colours")
                    .clicked()
            {
                let all = custom_colors
                    .iter()
                    .copied()
                    .filter(|c| !same_color(*c, self.custom))
                    .collect();
                commands.push(UiCommand::SetCustomColors(all));
            }
        });
        if display.is_some() && secondary_button(ui, "The colour it came with").clicked() {
            document.set_body_display(body, None);
        }

        // How much shows through.
        heading(ui, "See-through");
        let mut through = (1.0 - display.map_or(1.0, |d| d.opacity)) * 100.0;
        let slider = ui.add(
            egui::Slider::new(&mut through, 0.0..=90.0)
                .suffix(" %")
                .fixed_decimals(0),
        );
        if slider.changed() {
            let mut next = display.unwrap_or_default();
            next.opacity = 1.0 - through / 100.0;
            document.set_body_display(body, Some(next));
        }

        // Single faces.
        heading(ui, "Faces");
        if let Some((_, index)) = picked_face.filter(|(b, _)| *b == body) {
            self.face = Some(index);
        }
        match self.face {
            Some(index) => {
                let own = document.face_color(body, index);
                ui.label(
                    RichText::new(format!("Selected face {}", index + 1))
                        .font(sans_semibold(FONT_SM))
                        .color(TEXT1),
                );
                if let Some(color) = swatches(ui, own, custom_colors) {
                    document.color_face(body, index, Some(color));
                }
                if own.is_some() && secondary_button(ui, "The body's colour").clicked() {
                    document.color_face(body, index, None);
                }
            }
            None => note(
                ui,
                "Click a face of the body in the view to give it a colour of its own.",
            ),
        }
        let coloured = entry.face_colors.len();
        if coloured > 0 {
            ui.add_space(SPACE_1);
            ui.horizontal(|ui| {
                note(
                    ui,
                    &format!(
                        "{coloured} face{} coloured",
                        if coloured == 1 { "" } else { "s" }
                    ),
                );
                if small_secondary_button(ui, "Clear").clicked() {
                    for c in &entry.face_colors {
                        document.set_face_color(body, c.index, c.name, None);
                    }
                }
            });
        }

        // What it is made of.
        heading(ui, "Material");
        let mut choice: usize = match &entry.material {
            None => 0,
            Some(_) if self.own_material => MATERIALS.len() + 1,
            Some(m) => MATERIALS
                .iter()
                .position(|(name, _)| *name == m.name)
                .map_or(MATERIALS.len() + 1, |i| i + 1),
        };
        let mut options: Vec<(usize, String)> = vec![(0, "None".to_string())];
        options.extend(
            MATERIALS
                .iter()
                .enumerate()
                .map(|(i, (name, density))| (i + 1, format!("{name}  ·  {density:.2} g/cm³"))),
        );
        options.push((MATERIALS.len() + 1, "Other…".to_string()));
        let labels: Vec<(usize, &str)> = options.iter().map(|(i, l)| (*i, l.as_str())).collect();
        if ui_kit::widgets::select_field(ui, "host_material", &mut choice, &labels, 240.0) {
            self.own_material = choice == MATERIALS.len() + 1;
            let material = match choice {
                0 => None,
                i if i <= MATERIALS.len() => Some(Material {
                    name: MATERIALS[i - 1].0.to_string(),
                    density: MATERIALS[i - 1].1,
                }),
                _ => Some(entry.material.clone().unwrap_or(Material {
                    name: "Material".into(),
                    density: 1.0,
                })),
            };
            document.set_body_material(body, material);
        }
        if self.own_material
            && let Some(mut material) = entry.material.clone()
        {
            let before = material.clone();
            egui::Grid::new("host_material_own")
                .num_columns(2)
                .spacing([SPACE_3, SPACE_1])
                .show(ui, |ui| {
                    ui.label(RichText::new("Name").font(sans(FONT_SM)).color(TEXT2));
                    ui.add(egui::TextEdit::singleline(&mut material.name).desired_width(140.0));
                    ui.end_row();
                    ui.label(RichText::new("Density").font(sans(FONT_SM)).color(TEXT2));
                    QtyField::new(&mut material.density)
                        .unit("g/cm³")
                        .range(0.01..=30.0)
                        .speed(0.01)
                        .show(ui);
                    ui.end_row();
                });
            if material != before && !material.name.trim().is_empty() {
                document.set_body_material(body, Some(material));
            }
        }
        if entry.material.is_some() {
            note(ui, "The Physical group of its properties gives its mass.");
        }
        None
    }

    /// The calls that set the body as it stands, for a recording.
    fn recorded(&self, entry: &core_document::Body) -> Vec<Recorded> {
        let id = entry.id.0.to_string();
        let mut calls = Vec::new();
        if entry.display != self.start_display || entry.material != self.start_material {
            let mut args = json!({"body": id});
            match entry.display {
                Some(d) => {
                    args["color"] = json!(d.color);
                    args["opacity"] = json!(d.opacity);
                }
                None => args["color"] = Value::Null,
            }
            args["material"] = entry.material.as_ref().map_or(
                Value::Null,
                |m| json!({"name": m.name, "density": m.density}),
            );
            calls.push(call("doc.set_body", args));
        }
        if entry.face_colors != self.start_faces {
            calls.push(call("doc.set_body", json!({"body": id, "face_colors": []})));
            for c in &entry.face_colors {
                calls.push(call(
                    "doc.set_face_color",
                    json!({"body": id, "face": c.index, "color": c.color}),
                ));
            }
        }
        calls
    }
}

impl HistoryTask {
    fn show(
        &mut self,
        ui: &mut Ui,
        document: &mut Document,
        accept: bool,
        cancel: bool,
    ) -> Option<HostTaskEnd> {
        let order = document.body_history_of(self.feature);
        let at = order.iter().position(|f| *f == self.feature);
        let after = at.and_then(|i| i.checked_sub(1)).map(|i| order[i]);
        if cancel {
            let _ = document.move_feature_after(self.feature, self.start_after);
            return Some(HostTaskEnd::Cancelled);
        }
        if accept {
            let mut args = json!({"id": self.feature.0.to_string()});
            if let Some(after) = after {
                args["after"] = json!(after.0.to_string());
            }
            return Some(HostTaskEnd::Accepted(
                "Reorder history".into(),
                if after == self.start_after {
                    Vec::new()
                } else {
                    vec![call("doc.move_after", args)]
                },
            ));
        }
        note(
            ui,
            "Click the feature it should come after. A feature never goes before one it is built from.",
        );
        ui.add_space(SPACE_2);
        let mut target: Option<Option<FeatureId>> = None;
        if ui
            .add_enabled(at != Some(0), egui::Button::new("At the start"))
            .clicked()
        {
            target = Some(None);
        }
        for (i, id) in order.iter().enumerate() {
            let Some(node) = document.get_feature_meta(*id) else {
                continue;
            };
            let this = *id == self.feature;
            let text = RichText::new(format!("{}. {}", i + 1, node.name)).font(sans(FONT_SM));
            let text = if this {
                text.color(ACCENT).strong()
            } else {
                text.color(TEXT1)
            };
            let row = ui.add_enabled(
                !this,
                egui::Button::new(text)
                    .frame(false)
                    .min_size(Vec2::new(ui.available_width(), 22.0)),
            );
            if row.clicked() {
                target = Some(Some(*id));
            }
        }
        if let Some(after) = target {
            self.refused = document
                .move_feature_after(self.feature, after)
                .err()
                .map(|refused| match refused {
                    core_document::MoveRefused::Dependency { neighbour } => format!(
                        "It stops by {}: one is built from the other.",
                        document
                            .get_feature_meta(neighbour)
                            .map(|n| n.name.clone())
                            .unwrap_or_default()
                    ),
                    _ => "It cannot go there.".to_string(),
                });
        }
        if let Some(why) = &self.refused {
            ui.add_space(SPACE_2);
            ui.label(RichText::new(why).font(sans(FONT_SM)).color(WARNING));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run `f` in a frame of a panel, as the task panel would.
    fn in_frame(f: impl FnOnce(&mut Ui)) {
        let ctx = egui::Context::default();
        let mut f = Some(f);
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            if let Some(f) = f.take() {
                f(ui);
            }
        });
        output.textures_delta.clear();
    }

    fn show(
        task: &mut HostTask,
        doc: &mut Document,
        accept: bool,
        cancel: bool,
    ) -> Option<HostTaskEnd> {
        let mut end = None;
        in_frame(|ui| {
            end = task.show(
                ui,
                HostTaskInputs {
                    document: doc,
                    picked_face: None,
                    custom_colors: &[],
                },
                accept,
                cancel,
                &mut Vec::new(),
            );
        });
        end
    }

    #[test]
    fn placement_moves_live_and_cancel_puts_it_back() {
        let mut doc = Document::new("t");
        let body = doc.create_body(None);
        let mut task = HostTask::placement(&doc, body);
        if let HostTask::Placement(t) = &mut task {
            t.offset = [-5.0, 2.0, 0.0];
            t.angles = [0.0, 0.0, 90.0];
            doc.place_with_unit(body, t.placement());
        }
        assert_eq!(doc.body_placement(body).translation, [-5.0, 2.0, 0.0]);
        assert!(matches!(
            show(&mut task, &mut doc, false, true),
            Some(HostTaskEnd::Cancelled)
        ));
        assert_eq!(doc.body_placement(body), BodyPlacement::IDENTITY);

        let mut task = HostTask::placement(&doc, body);
        if let HostTask::Placement(t) = &mut task {
            t.offset = [-5.0, 0.0, 0.0];
            doc.place_with_unit(body, t.placement());
        }
        let Some(HostTaskEnd::Accepted(_, calls)) = show(&mut task, &mut doc, true, false) else {
            panic!("accepted");
        };
        assert_eq!(calls[0].id, "doc.set_body");
        assert_eq!(calls[0].args["translation"], json!([-5.0, 0.0, 0.0]));
    }

    #[test]
    fn appearance_cancel_puts_back_colour_material_and_faces() {
        let mut doc = Document::new("t");
        let body = doc.create_body(None);
        let mut task = HostTask::appearance(&doc, body, None);
        doc.set_body_display(
            body,
            Some(BodyDisplay {
                color: PALETTE[6],
                opacity: 0.5,
            }),
        );
        doc.set_body_material(
            body,
            Some(Material {
                name: "PLA".into(),
                density: 1.24,
            }),
        );
        doc.set_face_color(body, 3, 9, Some([1.0, 0.0, 0.0]));
        let Some(HostTaskEnd::Accepted(_, calls)) =
            show(&mut task.clone(), &mut doc.clone(), true, false)
        else {
            panic!("accepted");
        };
        let ids: Vec<&str> = calls.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["doc.set_body", "doc.set_body", "doc.set_face_color"]);
        assert!(matches!(
            show(&mut task, &mut doc, false, true),
            Some(HostTaskEnd::Cancelled)
        ));
        let entry = &doc.bodies()[0];
        assert!(entry.display.is_none() && entry.material.is_none());
        assert!(entry.face_colors.is_empty());
    }

    #[test]
    fn a_history_move_cancels_back_to_where_it_was() {
        let mut doc = Document::new("t");
        let body = doc.create_body(None);
        let ids: Vec<FeatureId> = ["a", "b", "c"]
            .iter()
            .map(|n| {
                doc.add_feature_in_body(
                    core_document::DatumFeature {
                        shape: core_document::DatumShape::Point,
                        attachment: core_document::DatumAttachment::BasePlane(
                            core_document::BasePlane::XY,
                        ),
                        offset: Default::default(),
                    },
                    n.to_string(),
                    Some(body),
                )
                .unwrap()
            })
            .collect();
        let mut task = HostTask::history(&doc, ids[1]);
        doc.move_feature_after(ids[1], Some(ids[2])).unwrap();
        assert_eq!(doc.body_history_of(ids[0]), [ids[0], ids[2], ids[1]]);
        assert!(matches!(
            show(&mut task, &mut doc, false, true),
            Some(HostTaskEnd::Cancelled)
        ));
        assert_eq!(doc.body_history_of(ids[0]), ids);
    }
}
