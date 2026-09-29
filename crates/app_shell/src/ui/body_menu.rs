//! The entries a body's menu has, in the tree and in the viewport alike:
//! its look, faces' colours, material, placement, freezing, whether clicks
//! pick it, copies and a rebuild. What only changes the window (the
//! property panel's page, the console's input, the placement dialog) comes
//! back as a [`MenuLocal`] for the UI to act on.

use core_document::{BodyDisplay, BodyId, Document, Material};
use egui::Ui;
use ui_kit::tokens::*;

use super::property_panel::PropertyTab;
use super::{BodyEdit, TreeItemId, UiCommand};

/// What a menu entry changes in the window rather than the document.
#[derive(Debug, Clone, PartialEq)]
pub enum MenuLocal {
    /// Select the item and show this page of the property panel.
    Properties(TreeItemId, PropertyTab),
    /// Select the item and put its name up for editing.
    Rename(TreeItemId),
    /// Put this text in the script console's input.
    Console(String),
    /// Open the placement dialog on a body.
    Placement(BodyId),
}

/// The placement dialog's numbers for a body, until applied.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacementDraft {
    pub body: BodyId,
    /// Where its origin goes, mm.
    pub offset: [f32; 3],
    /// Its turn about X, then Y, then Z, degrees.
    pub angles: [f32; 3],
}

impl PlacementDraft {
    /// The dialog opened on `body` as it sits.
    pub fn of(document: &Document, body: BodyId) -> Self {
        let placement = document.body_placement(body);
        let (x, y, z) = placement.quat().to_euler(glam::EulerRot::XYZ);
        Self {
            body,
            offset: placement.offset().to_array(),
            angles: [x, y, z].map(f32::to_degrees),
        }
    }

    fn placement(&self) -> core_document::BodyPlacement {
        let [x, y, z] = self.angles.map(f32::to_radians);
        core_document::BodyPlacement::new(
            glam::Quat::from_euler(glam::EulerRot::XYZ, x, y, z),
            glam::Vec3::from_array(self.offset),
        )
    }
}

/// The placement dialog: a body's position and turn by numbers, applied
/// as one step. Closes itself on OK or Cancel.
pub fn placement_window(
    ctx: &egui::Context,
    draft: &mut Option<PlacementDraft>,
    document: &Document,
    out: &mut Vec<UiCommand>,
) {
    let Some(editing) = draft.as_mut() else {
        return;
    };
    let Some(name) = document
        .bodies()
        .iter()
        .find(|b| b.id == editing.body)
        .map(|b| b.name.clone())
    else {
        *draft = None;
        return;
    };
    let mut open = true;
    let mut close = false;
    egui::Window::new(format!("Placement of {name}"))
        .id(egui::Id::new("body_placement_window"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            egui::Grid::new("body_placement_grid")
                .num_columns(2)
                .spacing([SPACE_3, SPACE_1])
                .show(ui, |ui| {
                    for (i, axis) in ["X", "Y", "Z"].iter().enumerate() {
                        ui.label(format!("Position {axis}"));
                        ui_kit::widgets::QtyField::mm(&mut editing.offset[i]).show(ui);
                        ui.end_row();
                    }
                    for (i, axis) in ["X", "Y", "Z"].iter().enumerate() {
                        ui.label(format!("Turn about {axis}"));
                        ui_kit::widgets::QtyField::degrees(&mut editing.angles[i])
                            .range(-180.0..=180.0)
                            .show(ui);
                        ui.end_row();
                    }
                });
            ui.add_space(SPACE_2);
            ui.horizontal(|ui| {
                if ui_kit::widgets::primary_button(ui, "OK").clicked() {
                    out.push(UiCommand::BodyEdit {
                        body: editing.body,
                        edit: BodyEdit::Place(editing.placement()),
                    });
                    close = true;
                }
                if ui_kit::widgets::secondary_button(ui, "Apply").clicked() {
                    out.push(UiCommand::BodyEdit {
                        body: editing.body,
                        edit: BodyEdit::Place(editing.placement()),
                    });
                }
                if ui_kit::widgets::secondary_button(ui, "Cancel").clicked() {
                    close = true;
                }
            });
        });
    if close || !open {
        *draft = None;
    }
}

/// Materials the Material menu offers, densities in g/cm³.
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

/// The colours a face can take from its menu.
const SWATCHES: &[[f32; 3]] = &[
    [0.85, 0.20, 0.20],
    [0.95, 0.55, 0.15],
    [0.95, 0.85, 0.20],
    [0.35, 0.75, 0.30],
    [0.20, 0.65, 0.65],
    [0.25, 0.45, 0.90],
    [0.55, 0.35, 0.85],
    [0.90, 0.45, 0.70],
    [0.95, 0.95, 0.95],
    [0.55, 0.55, 0.55],
    [0.15, 0.15, 0.15],
    [0.55, 0.38, 0.22],
];

/// A lively colour picked at random: any hue, never grey or dark.
pub fn random_color() -> [f32; 3] {
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    // One step of a 64-bit mix, so close seeds land far apart.
    let mut x = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    let hue = (x % 360) as f32;
    hsv(hue, 0.55, 0.85)
}

fn hsv(hue: f32, s: f32, v: f32) -> [f32; 3] {
    let c = v * s;
    let h = hue / 60.0;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    [r + m, g + m, b + m]
}

/// A body as a script names it: its id, with its name beside.
pub fn body_reference(name: &str, body: BodyId) -> String {
    format!("\"{}\" --[[ {} ]]", body.0, name.replace("]]", "] ]"))
}

fn edit(out: &mut Vec<UiCommand>, body: BodyId, edit: BodyEdit) {
    out.push(UiCommand::BodyEdit { body, edit });
}

/// A row of colour swatches; the one clicked, if any.
fn swatches(ui: &mut Ui) -> Option<[f32; 3]> {
    let mut picked = None;
    ui.horizontal_wrapped(|ui| {
        ui.set_max_width(6.0 * 24.0);
        for color in SWATCHES {
            let fill = egui::Color32::from_rgb(
                (color[0] * 255.0) as u8,
                (color[1] * 255.0) as u8,
                (color[2] * 255.0) as u8,
            );
            let button = egui::Button::new("")
                .fill(fill)
                .min_size(egui::vec2(18.0, 18.0))
                .corner_radius(RADIUS_SM);
            if ui.add(button).clicked() {
                picked = Some(*color);
            }
        }
    });
    picked
}

/// The body entries of a menu, after whatever came before. `face` is the
/// face the menu was opened on, when it was: its index and name. Returns
/// whether an entry was picked, so the caller can close the menu.
pub fn body_entries(
    ui: &mut Ui,
    document: &Document,
    body: BodyId,
    face: Option<(u32, kernel_api::naming::TopoName)>,
    out: &mut Vec<UiCommand>,
    local: &mut Option<MenuLocal>,
) -> bool {
    let Some(entry) = document.bodies().iter().find(|b| b.id == body) else {
        return false;
    };
    let mut picked = false;
    let display = entry.display;

    // Look.
    if ui
        .button("Appearance…")
        .on_hover_text("The body's colour and how much shows through")
        .clicked()
    {
        *local = Some(MenuLocal::Properties(
            TreeItemId::Body(body),
            PropertyTab::View,
        ));
        picked = true;
    }
    if ui.button("Random colour").clicked() {
        let opacity = display.map_or(1.0, |d| d.opacity);
        out.push(UiCommand::SetBodyDisplay {
            body,
            display: Some(BodyDisplay {
                color: random_color(),
                opacity,
            }),
        });
        picked = true;
    }
    let see_through = display.is_some_and(|d| d.opacity < 1.0);
    let transparency = if see_through { "Solid" } else { "Transparent" };
    if ui
        .button(transparency)
        .on_hover_text("Let what is behind the body show through it, or not")
        .clicked()
    {
        let mut next = display.unwrap_or_default();
        next.opacity = if see_through { 1.0 } else { 0.5 };
        out.push(UiCommand::SetBodyDisplay {
            body,
            display: Some(next),
        });
        picked = true;
    }
    if let Some((index, name)) = face {
        ui.menu_button("Face colour", |ui| {
            if let Some(color) = swatches(ui) {
                edit(
                    out,
                    body,
                    BodyEdit::FaceColor {
                        index,
                        name,
                        color: Some(color),
                    },
                );
                picked = true;
                ui.close();
            }
            let coloured = entry.face_colors.iter().any(|c| {
                document
                    .imported_geometry(body)
                    .is_some_and(|g| c.is_face(&g.mesh, index))
            });
            if coloured && ui.button("Body's colour").clicked() {
                edit(
                    out,
                    body,
                    BodyEdit::FaceColor {
                        index,
                        name,
                        color: None,
                    },
                );
                picked = true;
                ui.close();
            }
        });
    }
    if !entry.face_colors.is_empty() && ui.button("Clear face colours").clicked() {
        edit(out, body, BodyEdit::ClearFaceColors);
        picked = true;
    }
    ui.menu_button("Material", |ui| {
        for (name, density) in MATERIALS {
            let on = entry.material.as_ref().is_some_and(|m| m.name == *name);
            let label = format!("{name}  {density:.2} g/cm³");
            if ui.selectable_label(on, label).clicked() {
                edit(
                    out,
                    body,
                    BodyEdit::Material(Some(Material {
                        name: name.to_string(),
                        density: *density,
                    })),
                );
                picked = true;
                ui.close();
            }
        }
        if entry.material.is_some() {
            ui.separator();
            if ui.button("None").clicked() {
                edit(out, body, BodyEdit::Material(None));
                picked = true;
                ui.close();
            }
        }
    });
    ui.separator();

    // Where it is and how it behaves.
    if ui
        .button("Placement…")
        .on_hover_text("Move or turn the body by numbers")
        .clicked()
    {
        *local = Some(MenuLocal::Placement(body));
        picked = true;
    }
    let (freeze, hint) = if entry.frozen {
        ("Thaw", "Rebuild the body from its history again")
    } else {
        (
            "Freeze",
            "Keep the body as it stands: its features are not rebuilt until it thaws",
        )
    };
    if ui.button(freeze).on_hover_text(hint).clicked() {
        edit(out, body, BodyEdit::Frozen(!entry.frozen));
        picked = true;
    }
    let (select, hint) = if entry.unselectable {
        (
            "Make selectable",
            "Let clicks in the view pick the body again",
        )
    } else {
        (
            "Make unselectable",
            "Clicks in the view pass through the body to what is behind it",
        )
    };
    if ui.button(select).on_hover_text(hint).clicked() {
        edit(out, body, BodyEdit::Selectable(entry.unselectable));
        picked = true;
    }
    ui.separator();

    // Copies.
    if ui
        .button("Linked copy")
        .on_hover_text("A body of the same shape beside this one, following every change to it")
        .clicked()
    {
        edit(out, body, BodyEdit::LinkedCopy);
        picked = true;
    }
    if let Some(source) = entry.copy_of
        && ui
            .button("Select the original")
            .on_hover_text("The body this one is a copy of")
            .clicked()
    {
        out.push(UiCommand::SelectTreeItem(TreeItemId::Body(source)));
        out.push(UiCommand::RevealInTree(source));
        picked = true;
    }
    ui.separator();

    if ui
        .button("Recompute")
        .on_hover_text("Build the body again from its history")
        .clicked()
    {
        edit(out, body, BodyEdit::Recompute);
        picked = true;
    }
    if ui
        .button("Send to console")
        .on_hover_text("Put the body's id in the script console's input")
        .clicked()
    {
        *local = Some(MenuLocal::Console(body_reference(&entry.name, body)));
        picked = true;
    }
    if ui.button("Properties").clicked() {
        *local = Some(MenuLocal::Properties(
            TreeItemId::Body(body),
            PropertyTab::Data,
        ));
        picked = true;
    }
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_colours_are_bright_and_coloured() {
        for hue in [0.0, 50.0, 130.0, 200.0, 290.0, 359.0] {
            let c = hsv(hue, 0.55, 0.85);
            let (max, min) = (
                c.iter().fold(0f32, |a, b| a.max(*b)),
                c.iter().fold(1f32, |a, b| a.min(*b)),
            );
            assert!((max - 0.85).abs() < 1e-5, "{c:?}");
            assert!(max - min > 0.4, "{c:?}");
        }
    }
}
