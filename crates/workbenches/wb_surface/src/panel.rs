//! The task panel of a surface step and the bench's Preferences page.
//!
//! A step's inputs are lists: each row names a sketch or an edge, with a
//! cross to take it out; "Add the selection" takes what is selected now
//! (a sketch in the tree, edges and a face of the body in the view). Every
//! edit writes the step at once and it builds again.

use core_document::{FeatureId, WorkbenchRuntimeContext};
use egui::{RichText, Ui};
use kernel_api::Continuity;
use ui_kit::tokens::*;
use ui_kit::widgets::{
    Note, PrefRow, QtyField, check_row, field_label, icon_button, note_card, pref_group,
    select_field, small_secondary_button,
};
use ui_kit::{sans, sans_medium};

use crate::Options;
use crate::feature::{Axis, CurveRef, Direction, EdgePick, FacePick, PlaneRef, SurfaceFeature};

const FIELD: f32 = 150.0;

/// What is selected now: curves and faces a step could take.
pub type Selection = (Vec<CurveRef>, Vec<FacePick>);

/// Draw the step's editor; whether anything changed.
pub fn editor(
    ui: &mut Ui,
    ctx: &WorkbenchRuntimeContext,
    id: FeatureId,
    feature: &mut SurfaceFeature,
    selection: Option<Selection>,
) -> bool {
    let (picked, faces) = selection.unwrap_or_default();
    let mut changed = false;
    if let Some(waits) = feature.kind().waits {
        note_card(
            ui,
            Note::Warning,
            Some("Waits on the geometry kernel"),
            &format!("{waits}. The step keeps its settings and builds once the kernel can."),
        );
    }
    if let Some(error) = ctx
        .document
        .get_feature_meta(id)
        .and_then(|n| n.error.clone())
    {
        note_card(ui, Note::Error, Some("Does not build"), &error);
    }
    ui.add_space(SPACE_1);
    match feature {
        SurfaceFeature::Extrude {
            curves,
            direction,
            length,
            symmetric,
            reversed,
        } => {
            changed |= curve_list(ui, ctx, "Curves", curves, &picked);
            changed |= length_row(ui, "Length", length);
            changed |= direction_row(ui, direction);
            changed |= check_row(ui, symmetric, "Both ways, half each").changed();
            changed |= check_row(ui, reversed, "Reversed").changed();
        }
        SurfaceFeature::Revolve {
            curves,
            axis,
            angle_deg,
        } => {
            changed |= curve_list(ui, ctx, "Curves", curves, &picked);
            changed |= axis_row(ui, axis);
            changed |= row(ui, "Angle", |ui| {
                QtyField::degrees(angle_deg).range(0.1..=360.0).show(ui)
            });
        }
        SurfaceFeature::PlanarFill { curves } => {
            hint(
                ui,
                "Closed loops in one plane; a loop inside another is a hole.",
            );
            changed |= curve_list(ui, ctx, "Loops", curves, &picked);
        }
        SurfaceFeature::Fill {
            boundary,
            continuity,
        } => {
            hint(
                ui,
                "Curves meeting end to end round the hole: four today. Pick edges of the \
                 body's sheets to fill between them.",
            );
            changed |= curve_list(ui, ctx, "Boundary", boundary, &picked);
            changed |= continuity_row(ui, continuity);
        }
        SurfaceFeature::Ruled { first, second } => {
            changed |= one_curve(ui, ctx, "First curve", first, &picked);
            changed |= one_curve(ui, ctx, "Second curve", second, &picked);
        }
        SurfaceFeature::Loft { sections, closed } => {
            hint(
                ui,
                "One curve per section, in the order the surface passes them.",
            );
            changed |= curve_list(ui, ctx, "Sections", sections, &picked);
            changed |= check_row(ui, closed, "Closed back to the first").changed();
        }
        SurfaceFeature::Sweep { profile, path } => {
            changed |= curve_list(ui, ctx, "Profile", profile, &picked);
            changed |= curve_list(ui, ctx, "Path", path, &picked);
        }
        SurfaceFeature::Offset { faces: f, distance } => {
            changed |= face_list(ui, "Faces", f, &faces);
            changed |= length_row(ui, "Distance", distance);
        }
        SurfaceFeature::Extend {
            edges,
            length,
            continuity,
        } => {
            changed |= edge_list(ui, "Edges", edges, &picked);
            changed |= length_row(ui, "Length", length);
            changed |= continuity_row(ui, continuity);
        }
        SurfaceFeature::Blend {
            first,
            second,
            continuity,
        } => {
            changed |= one_edge(ui, "First edge", first, &picked);
            changed |= one_edge(ui, "Second edge", second, &picked);
            changed |= continuity_row(ui, continuity);
        }
        SurfaceFeature::Split { faces: f, curves } => {
            changed |= face_list(ui, "Faces", f, &faces);
            changed |= curve_list(ui, ctx, "Cut along", curves, &picked);
        }
        SurfaceFeature::Sew => hint(
            ui,
            "Joins the body's surfaces where their edges meet. A shell that closes \
             becomes a solid.",
        ),
        SurfaceFeature::Thicken {
            thickness,
            both_sides,
        } => {
            changed |= length_row(ui, "Thickness", thickness);
            changed |= check_row(ui, both_sides, "Both sides, half each").changed();
        }
        SurfaceFeature::Trim {
            plane,
            offset,
            flip,
        } => {
            changed |= plane_row(ui, plane);
            changed |= length_row(ui, "Offset", offset);
            changed |= check_row(ui, flip, "Keep the other side").changed();
        }
        SurfaceFeature::Mirror { plane, offset } => {
            changed |= plane_row(ui, plane);
            changed |= length_row(ui, "Offset", offset);
        }
    }
    changed
}

fn hint(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).font(sans(FONT_XS)).color(TEXT3)).wrap());
    ui.add_space(SPACE_1);
}

fn row(ui: &mut Ui, label: &str, control: impl FnOnce(&mut Ui) -> bool) -> bool {
    ui.horizontal(|ui| {
        field_label(ui, label);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), control)
            .inner
    })
    .inner
}

fn length_row(ui: &mut Ui, label: &str, value: &mut f32) -> bool {
    row(ui, label, |ui| QtyField::new(value).unit("mm").show(ui))
}

fn continuity_row(ui: &mut Ui, continuity: &mut Continuity) -> bool {
    row(ui, "Continuity", |ui| {
        select_field(
            ui,
            "surface_continuity",
            continuity,
            &[
                (Continuity::G0, "Touching (G0)"),
                (Continuity::G1, "Tangent (G1)"),
                (Continuity::G2, "Curvature (G2)"),
            ],
            FIELD,
        )
    })
}

fn direction_row(ui: &mut Ui, direction: &mut Direction) -> bool {
    let mut choice = match *direction {
        Direction::Custom(_) => Direction::SketchNormal,
        other => other,
    };
    let custom = matches!(direction, Direction::Custom(_));
    let changed = row(ui, "Direction", |ui| {
        select_field(
            ui,
            "surface_direction",
            &mut choice,
            &[
                (Direction::SketchNormal, "Square to the sketch"),
                (Direction::X, "X"),
                (Direction::Y, "Y"),
                (Direction::Z, "Z"),
            ],
            FIELD,
        )
    });
    if changed || !custom {
        *direction = choice;
    }
    changed
}

fn axis_row(ui: &mut Ui, axis: &mut Axis) -> bool {
    if matches!(axis, Axis::Custom { .. }) {
        hint(ui, "A custom axis, set by a script.");
    }
    let mut choice = *axis;
    let changed = row(ui, "Axis", |ui| {
        select_field(
            ui,
            "surface_axis",
            &mut choice,
            &[
                (Axis::SketchVertical, "Sketch's vertical"),
                (Axis::SketchHorizontal, "Sketch's horizontal"),
                (Axis::X, "X"),
                (Axis::Y, "Y"),
                (Axis::Z, "Z"),
            ],
            FIELD,
        )
    });
    if changed {
        *axis = choice;
    }
    changed
}

fn plane_row(ui: &mut Ui, plane: &mut PlaneRef) -> bool {
    if matches!(plane, PlaneRef::Custom { .. }) {
        hint(ui, "A custom plane, set by a script.");
    }
    let mut choice = *plane;
    let changed = row(ui, "Plane", |ui| {
        select_field(
            ui,
            "surface_plane",
            &mut choice,
            &[
                (PlaneRef::YZ, "YZ"),
                (PlaneRef::XZ, "XZ"),
                (PlaneRef::XY, "XY"),
            ],
            FIELD,
        )
    });
    if changed {
        *plane = choice;
    }
    changed
}

fn curve_name(ctx: &WorkbenchRuntimeContext, curve: &CurveRef) -> String {
    match curve {
        CurveRef::Sketch(id) => ctx
            .document
            .get_feature_meta(*id)
            .map(|n| n.name.clone())
            .unwrap_or_else(|| "A sketch that is gone".into()),
        CurveRef::Edge(edge) => edge_name(edge),
    }
}

fn edge_name(edge: &EdgePick) -> String {
    if edge.length > 0.0 {
        format!("Edge, {:.2} mm", edge.length)
    } else {
        "Edge".into()
    }
}

/// A heading with an "Add the selection" button, then one row per item with
/// a cross to take it out. Whether the list changed.
fn list<T: Clone + PartialEq>(
    ui: &mut Ui,
    title: &str,
    items: &mut Vec<T>,
    selection: &[T],
    name: impl Fn(&T) -> String,
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).font(sans_medium(FONT_SM)).color(TEXT1));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let fresh: Vec<T> = selection
                .iter()
                .filter(|s| !items.contains(s))
                .cloned()
                .collect();
            if ui
                .add_enabled_ui(!fresh.is_empty(), |ui| {
                    small_secondary_button(ui, "Add the selection")
                })
                .inner
                .on_disabled_hover_text(
                    "Select a sketch in the tree, or edges or a face in the view",
                )
                .clicked()
            {
                items.extend(fresh);
                changed = true;
            }
        });
    });
    if items.is_empty() {
        hint(ui, "Nothing picked yet.");
    }
    let mut remove = None;
    for (index, item) in items.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.add_space(SPACE_2);
            ui.label(RichText::new(name(item)).font(sans(FONT_SM)).color(TEXT2));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icon_button(ui, "close", "Take it out").clicked() {
                    remove = Some(index);
                }
            });
        });
    }
    if let Some(index) = remove {
        items.remove(index);
        changed = true;
    }
    ui.add_space(SPACE_1);
    changed
}

fn curve_list(
    ui: &mut Ui,
    ctx: &WorkbenchRuntimeContext,
    title: &str,
    curves: &mut Vec<CurveRef>,
    picked: &[CurveRef],
) -> bool {
    list(ui, title, curves, picked, |c| curve_name(ctx, c))
}

fn edge_list(ui: &mut Ui, title: &str, edges: &mut Vec<EdgePick>, picked: &[CurveRef]) -> bool {
    let picked: Vec<EdgePick> = picked
        .iter()
        .filter_map(|c| match c {
            CurveRef::Edge(e) => Some(*e),
            CurveRef::Sketch(_) => None,
        })
        .collect();
    list(ui, title, edges, &picked, edge_name)
}

fn face_list(ui: &mut Ui, title: &str, faces: &mut Vec<FacePick>, picked: &[FacePick]) -> bool {
    list(ui, title, faces, picked, |_| "Face".to_string())
}

/// A single curve slot, filled from the selection's first curve.
fn one_curve(
    ui: &mut Ui,
    ctx: &WorkbenchRuntimeContext,
    title: &str,
    slot: &mut Option<CurveRef>,
    picked: &[CurveRef],
) -> bool {
    let mut items: Vec<CurveRef> = slot.iter().copied().collect();
    let first: Vec<CurveRef> = picked.iter().take(1).copied().collect();
    let before = items.clone();
    if list(ui, title, &mut items, &first, |c| curve_name(ctx, c)) {
        *slot = items.last().copied();
    }
    items != before
}

fn one_edge(ui: &mut Ui, title: &str, slot: &mut Option<EdgePick>, picked: &[CurveRef]) -> bool {
    let mut items: Vec<EdgePick> = slot.iter().copied().collect();
    let first: Vec<EdgePick> = picked
        .iter()
        .find_map(|c| match c {
            CurveRef::Edge(e) => Some(*e),
            CurveRef::Sketch(_) => None,
        })
        .into_iter()
        .collect();
    let before = items.clone();
    if list(ui, title, &mut items, &first, edge_name) {
        *slot = items.last().copied();
    }
    items != before
}

/// The Surface preferences page.
pub fn settings(ui: &mut Ui, options: &mut Options, filter: &str) {
    pref_group(
        ui,
        "New surfaces",
        vec![
            PrefRow::toggle(
                "Hide the sketch a surface uses",
                &mut options.hide_used_sketches,
            )
            .hint("Keep the sketches a new surface is built from out of the view"),
        ],
        filter,
    );
}
