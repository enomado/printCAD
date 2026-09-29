//! The Hole task panel: the thread it is sized from (standard, size,
//! class, hand), the drill and its point, the cut around its mouth.

use core_document::{BodyId, FeatureId, WorkbenchRuntimeContext};
use egui::Ui;
use ui_kit::tokens::*;
use ui_kit::widgets::{check_row, mono_label};

use super::{Formulas, deg_drag, label_cell, mm_drag, sketch_combo};
use crate::feature::{DesignFeature, DrillPoint, HoleCut, HoleFit, ThreadSpec};
use crate::hole_tables::{ScrewSeat, ThreadStandard, user_cut_profiles};

/// The drill points the panel offers by name, included angle in degrees.
const POINT_ANGLES: [f32; 2] = [118.0, 135.0];

/// A combo row: `label` beside a dropdown showing `current`, whose
/// entries `add` draws.
fn combo_row(
    ui: &mut Ui,
    label: &str,
    id_salt: impl egui::AsIdSalt,
    current: impl Into<egui::WidgetText>,
    add: impl FnOnce(&mut Ui),
) {
    ui.horizontal(|ui| {
        label_cell(ui, label);
        egui::ComboBox::from_id_salt(id_salt)
            .selected_text(current)
            .show_ui(ui, add);
    });
}

/// The Hole editor; `true` when it changed the feature.
pub(super) fn hole_editor(
    ui: &mut Ui,
    ctx: &WorkbenchRuntimeContext,
    fx: &mut Formulas,
    body: BodyId,
    feature_id: FeatureId,
    feature: &mut DesignFeature,
) -> bool {
    // What the hole drills as it stands, before this frame's edits.
    let drilled = crate::build::hole_diameter(feature);
    let DesignFeature::Hole {
        refine: _,
        sketch,
        diameter,
        depth,
        through_all,
        cut,
        thread,
        threaded,
        modeled_thread,
        thread_depth,
        fit,
        clearance,
        thread_length,
        drill_point,
        point_in_depth,
        taper_deg,
        reversed,
    } = feature
    else {
        return false;
    };
    let mut changed = false;
    if let Some(new) = sketch_combo(
        ui,
        ctx,
        body,
        ("hole_sketch", feature_id),
        Some(*sketch),
        "Positions:",
    ) {
        *sketch = new;
        changed = true;
    }

    let standard = thread.as_ref().map(|t| t.standard);
    combo_row(
        ui,
        "Standard",
        ("hole_standard", feature_id),
        standard.map_or("Custom", ThreadStandard::label),
        |ui| {
            if ui.selectable_label(standard.is_none(), "Custom").clicked() && standard.is_some() {
                *thread = None;
                changed = true;
            }
            for candidate in ThreadStandard::ALL {
                if ui
                    .selectable_label(standard == Some(candidate), candidate.label())
                    .clicked()
                    && standard != Some(candidate)
                {
                    // The size nearest what the hole drills now.
                    let nearest = candidate
                        .sizes()
                        .into_iter()
                        .min_by(|a, b| {
                            let off = |d: f64| (d - f64::from(drilled)).abs();
                            off(a.major).total_cmp(&off(b.major))
                        })
                        .map_or("", |s| s.name);
                    let left_handed = thread.as_ref().is_some_and(|t| t.left_handed);
                    let mut spec = ThreadSpec::new(candidate, nearest);
                    spec.left_handed = left_handed;
                    *thread = Some(spec);
                    changed = true;
                }
            }
        },
    );

    if let Some(spec) = thread {
        let sizes = spec.standard.sizes();
        combo_row(
            ui,
            "Size",
            ("hole_size", feature_id),
            spec.size.clone(),
            |ui| {
                for size in &sizes {
                    if ui
                        .selectable_label(spec.size == size.name, size.name)
                        .clicked()
                        && spec.size != size.name
                    {
                        spec.size = size.name.to_string();
                        changed = true;
                    }
                }
            },
        );
        let classes = spec.standard.classes();
        if !classes.is_empty() {
            let current = spec.class().to_string();
            combo_row(
                ui,
                "Class",
                ("hole_class", feature_id),
                current.clone(),
                |ui| {
                    for class in classes {
                        if ui.selectable_label(current == *class, *class).clicked()
                            && current != *class
                        {
                            spec.class = class.to_string();
                            changed = true;
                        }
                    }
                },
            );
        }
        changed |= check_row(ui, &mut spec.left_handed, "Left-hand thread")
            .on_hover_text("The thread turns the other way")
            .changed();
        let size = spec.resolve();
        changed |= check_row(ui, threaded, "Threaded (tap drill)")
            .on_hover_text("Use the tap-drill diameter for later thread cutting")
            .changed();
        if *threaded {
            if check_row(ui, modeled_thread, "Modeled thread")
                .on_hover_text("Cut the thread itself into the wall, to print it")
                .changed()
            {
                if *modeled_thread && *thread_depth <= 0.0 {
                    *thread_depth = if *through_all { 10.0 } else { *depth };
                }
                changed = true;
            }
            if *modeled_thread {
                combo_row(
                    ui,
                    "Thread length",
                    ("hole_thread_length", feature_id),
                    thread_length.label(),
                    |ui| {
                        for candidate in crate::feature::ThreadLength::ALL {
                            if ui
                                .selectable_label(*thread_length == candidate, candidate.label())
                                .clicked()
                                && *thread_length != candidate
                            {
                                *thread_length = candidate;
                                changed = true;
                            }
                        }
                    },
                );
                if *thread_length == crate::feature::ThreadLength::Given {
                    changed |= mm_drag(ui, fx, thread_depth, "Thread depth:");
                }
            }
        } else if size.as_ref().is_ok() {
            let shown = match clearance {
                Some(_) => "Custom",
                None if size.as_ref().is_ok_and(|s| s.clearance.is_some()) => fit.label(),
                None => "Major diameter",
            };
            combo_row(ui, "Fit", ("hole_fit", feature_id), shown, |ui| {
                if size.as_ref().is_ok_and(|s| s.clearance.is_some()) {
                    for candidate in HoleFit::ALL {
                        let current = clearance.is_none() && *fit == candidate;
                        if ui.selectable_label(current, candidate.label()).clicked() && !current {
                            *fit = candidate;
                            *clearance = None;
                            changed = true;
                        }
                    }
                }
                if ui.selectable_label(clearance.is_some(), "Custom").clicked()
                    && clearance.is_none()
                {
                    *clearance = Some(drilled);
                    changed = true;
                }
            });
            if let Some(own) = clearance {
                changed |= mm_drag(ui, fx, own, "Clearance:");
            }
        }
        let summary = match &size {
            Ok(size) => {
                let mut text = format!(
                    "{} · pitch {:.3} mm · drill Ø {drilled:.2} mm",
                    spec.designation(),
                    size.pitch
                );
                if *threaded && spec.standard.is_tapered() {
                    text.push_str(" at the face, 1:16 taper");
                }
                text
            }
            Err(e) => e.clone(),
        };
        mono_label(ui, summary, FONT_SM, TEXT2);
    } else {
        changed |= mm_drag(ui, fx, diameter, "Diameter:");
    }

    changed |= check_row(ui, through_all, "Through all").changed();
    if !*through_all {
        changed |= mm_drag(ui, fx, depth, "Depth:");
        let point_label = match drill_point {
            DrillPoint::Flat => "Flat".to_string(),
            DrillPoint::Angled { angle_deg } => format!("{angle_deg}°"),
        };
        combo_row(
            ui,
            "Drill point",
            ("hole_point", feature_id),
            point_label,
            |ui| {
                if ui
                    .selectable_label(*drill_point == DrillPoint::Flat, "Flat")
                    .clicked()
                    && *drill_point != DrillPoint::Flat
                {
                    *drill_point = DrillPoint::Flat;
                    changed = true;
                }
                for angle_deg in POINT_ANGLES {
                    let point = DrillPoint::Angled { angle_deg };
                    if ui
                        .selectable_label(*drill_point == point, format!("{angle_deg}°"))
                        .clicked()
                        && *drill_point != point
                    {
                        *drill_point = point;
                        changed = true;
                    }
                }
            },
        );
        if let DrillPoint::Angled { angle_deg } = drill_point {
            changed |= deg_drag(ui, fx, angle_deg, "Point angle:", 10.0..=170.0);
            changed |= check_row(ui, point_in_depth, "Point within the depth")
                .on_hover_text("The depth runs to the tip of the point, not to the wall's end")
                .changed();
        }
    }
    let standard_taper = *threaded && thread.as_ref().is_some_and(|t| t.standard.is_tapered());
    if !standard_taper {
        changed |= deg_drag(ui, fx, taper_deg, "Taper:", -44.0..=44.0);
    }

    let metric = thread.as_ref().is_some_and(|t| t.standard.is_metric());
    let profiles = user_cut_profiles();
    let current = profiles
        .iter()
        .find(|p| p.cut == *cut)
        .map_or_else(|| cut.label().to_string(), |p| p.name.clone());
    combo_row(ui, "Hole cut", ("hole_cut", feature_id), current, |ui| {
        // Sized from the diameter the hole is drilled at, a standard
        // size's as much as a custom one.
        let mut options = vec![
            HoleCut::None,
            HoleCut::Counterbore {
                diameter: drilled * 2.0,
                depth: 2.0,
            },
            HoleCut::Spotface {
                diameter: drilled * 2.0,
                depth: 0.5,
            },
            HoleCut::Countersink {
                diameter: drilled * 2.0,
                angle_deg: 90.0,
            },
            HoleCut::Counterdrill {
                diameter: drilled * 1.5,
                depth: 2.0,
                angle_deg: 90.0,
            },
        ];
        if metric {
            options.extend(ScrewSeat::ALL.map(|seat| HoleCut::Seat { seat }));
        }
        for candidate in options {
            let is_current = match (&*cut, &candidate) {
                (HoleCut::Seat { seat: a }, HoleCut::Seat { seat: b }) => a == b,
                (a, b) => std::mem::discriminant(a) == std::mem::discriminant(b),
            };
            if ui.selectable_label(is_current, candidate.label()).clicked() && !is_current {
                *cut = candidate;
                changed = true;
            }
        }
        if !profiles.is_empty() {
            ui.separator();
            for profile in profiles {
                let is_current = profile.cut == *cut;
                if ui.selectable_label(is_current, &profile.name).clicked() && !is_current {
                    *cut = profile.cut;
                    changed = true;
                }
            }
        }
    });
    match cut {
        HoleCut::None => {}
        HoleCut::Counterbore { diameter, depth } => {
            changed |= mm_drag(ui, fx, diameter, "Bore Ø:");
            changed |= mm_drag(ui, fx, depth, "Bore depth:");
        }
        HoleCut::Spotface { diameter, depth } => {
            changed |= mm_drag(ui, fx, diameter, "Spot Ø:");
            changed |= mm_drag(ui, fx, depth, "Spot depth:");
        }
        HoleCut::Countersink {
            diameter,
            angle_deg,
        } => {
            changed |= mm_drag(ui, fx, diameter, "Sink Ø:");
            changed |= deg_drag(ui, fx, angle_deg, "Sink angle:", 10.0..=170.0);
        }
        HoleCut::Counterdrill {
            diameter,
            depth,
            angle_deg,
        } => {
            changed |= mm_drag(ui, fx, diameter, "Counterdrill Ø:");
            changed |= mm_drag(ui, fx, depth, "Counterdrill depth:");
            changed |= deg_drag(ui, fx, angle_deg, "Counterdrill angle:", 10.0..=170.0);
        }
        HoleCut::Seat { seat } => {
            let made = thread
                .as_ref()
                .filter(|t| t.standard.is_metric())
                .and_then(|t| t.resolve().ok())
                .and_then(|size| seat.cut(size.major));
            let text = match made {
                Some(HoleCut::Counterbore { diameter, depth }) => {
                    format!("Counterbore Ø {diameter:.1} mm, {depth:.1} mm deep")
                }
                Some(HoleCut::Countersink {
                    diameter,
                    angle_deg,
                }) => format!("Countersink Ø {diameter:.1} mm at {angle_deg}°"),
                _ => "No seat for this size".to_string(),
            };
            mono_label(ui, text, FONT_SM, TEXT2);
        }
    }
    changed |= check_row(ui, reversed, "Reversed").changed();
    changed
}
