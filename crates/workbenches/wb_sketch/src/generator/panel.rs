//! The task panel of a generated sketch: its numbers, each a field a
//! formula may set, what they come to, and the way out to a plain sketch.
//! Every change makes the curves again at once, so the sketch on screen is
//! the preview.

use core_document::{FeatureId, Parameter, WorkbenchFeature, WorkbenchRuntimeContext};
use egui::RichText;
use serde_json::{Map, Value, json};
use ui_kit::tokens::*;
use ui_kit::widgets::{
    Card, FormulaEdit, FormulaField, Note, check_row, field_label, mono_label, note_card, overline,
    secondary_button, select_field, small_secondary_button,
};
use ui_kit::{icon, sans_semibold};

use super::{Generator, SPROCKET_CHAINS, regenerate, summary};
use crate::feature::SketchFeature;

/// The width of the label column.
const LABEL: f32 = 110.0;

/// Draw the panel for generated sketch `id` and apply what it changes.
pub(crate) fn show(ui: &mut egui::Ui, ctx: &mut WorkbenchRuntimeContext, id: FeatureId) {
    let Some(node) = ctx.document.get_feature_meta(id).cloned() else {
        return;
    };
    // As its formulas leave it: what the fields show.
    let values = ctx
        .document
        .feature_values(id)
        .cloned()
        .unwrap_or_else(|| node.data.clone());
    let Some(generator) = SketchFeature::from_json(&values)
        .ok()
        .and_then(|f| f.generator)
    else {
        return;
    };
    let mut data = node.data.clone();
    let mut edited: Vec<String> = Vec::new();
    let mut formulas: Vec<(String, Option<String>)> = Vec::new();

    Card::new().show(ui, |ui| {
        ui.horizontal(|ui| {
            icon::draw(ui, generator.icon(), 16.0, ACCENT);
            ui.label(
                RichText::new(generator.label())
                    .font(sans_semibold(FONT_MD))
                    .color(TEXT1),
            );
        });
        ui.add_space(SPACE_2);
        if let Generator::Sprocket(spec) = &generator {
            let current = spec
                .chain()
                .and_then(|name| SPROCKET_CHAINS.iter().position(|(n, _, _)| *n == name));
            let mut chosen = current.unwrap_or(SPROCKET_CHAINS.len());
            let mut options: Vec<(usize, &str)> = SPROCKET_CHAINS
                .iter()
                .enumerate()
                .map(|(i, (name, _, _))| (i, *name))
                .collect();
            options.push((SPROCKET_CHAINS.len(), "Custom"));
            let picked = ui
                .horizontal(|ui| {
                    label_cell(ui, "Chain");
                    select_field(ui, ("sprocket_chain", id), &mut chosen, &options, 140.0)
                })
                .inner;
            if picked && let Some((_, pitch, roller)) = SPROCKET_CHAINS.get(chosen) {
                set(&mut data, "/generator/Sprocket/pitch", f64::from(*pitch));
                set(&mut data, "/generator/Sprocket/roller", f64::from(*roller));
                edited.extend(["pitch".to_string(), "roller".to_string()]);
            }
        }
        if let Generator::Gear(spec) = &generator {
            let mut internal = spec.internal;
            if check_row(ui, &mut internal, "Internal (ring) gear").changed() {
                set_value(&mut data, "/generator/Gear/internal", json!(internal));
                edited.push("internal".into());
            }
        }
        let params = generator.parameters();
        for (i, p) in params.iter().enumerate() {
            match (&generator, p.key.as_str()) {
                (Generator::Gear(spec), "bore" | "root_fillet") if spec.internal => continue,
                (Generator::Gear(spec), "rim") if !spec.internal => continue,
                _ => {}
            }
            if let Some(section) = shaft_section_start(p) {
                ui.add_space(SPACE_2);
                overline(ui, &format!("Section {section}"));
            } else if i == 0 && matches!(generator, Generator::Shaft(_)) {
                overline(ui, "Ends");
            }
            let value = values.pointer(&p.pointer).and_then(Value::as_f64);
            let Some(value) = value else { continue };
            match number_row(ui, ctx, id, p, value / p.scale) {
                Some(FormulaEdit::Value(v)) => {
                    let stored = v * p.scale;
                    set(
                        &mut data,
                        &p.pointer,
                        if p.integer { stored.round() } else { stored },
                    );
                    if ctx.document.feature_formula(id, &p.key).is_some() {
                        formulas.push((p.key.clone(), None));
                    }
                    edited.push(field_of(&p.pointer));
                }
                Some(FormulaEdit::Formula(text)) => formulas.push((p.key.clone(), Some(text))),
                None => {}
            }
        }
        if let Generator::Shaft(spec) = &generator {
            ui.add_space(SPACE_2);
            ui.horizontal(|ui| {
                if small_secondary_button(ui, "Add section").clicked()
                    && let Some(sections) = data
                        .pointer_mut("/generator/Shaft/sections")
                        .and_then(Value::as_array_mut)
                {
                    let last = sections.last().cloned().unwrap_or(json!({}));
                    sections.push(last);
                    edited.push("sections".into());
                }
                let n = spec.sections.len();
                if n > 1 && small_secondary_button(ui, "Remove last section").clicked() {
                    if let Some(sections) = data
                        .pointer_mut("/generator/Shaft/sections")
                        .and_then(Value::as_array_mut)
                    {
                        sections.pop();
                    }
                    for field in ["length", "diameter", "chamfer", "fillet"] {
                        let key = format!("/generator/Shaft/sections/{}/{field}", n - 1);
                        if ctx.document.feature_formula(id, &key).is_some() {
                            formulas.push((key, None));
                        }
                    }
                    edited.push("sections".into());
                }
            });
        }
    });

    measured(ui, &generator);

    ui.add_space(SPACE_2);
    let detach = secondary_button(ui, "Detach into a plain sketch")
        .on_hover_text("Keep the curves and forget the numbers, to edit them by hand")
        .clicked();

    for (key, formula) in formulas {
        if let Err(e) = ctx.document.set_feature_formula(id, key, formula) {
            ctx.log_error(format!("Could not set the formula: {e}"));
        }
    }
    if detach {
        apply(ctx, id, data, true);
        ctx.record(
            "sketch.generator",
            object(json!({"sketch": id.0.to_string(), "detach": true})),
            Value::Null,
        );
        return;
    }
    if !edited.is_empty() && apply(ctx, id, data, false) {
        remember(ui, ctx, id, &edited);
    }
    flush(ui, ctx, id);
}

/// Store `data` as the sketch, its curves made again (or its numbers
/// dropped, `detach`). Answers whether it went in.
fn apply(ctx: &mut WorkbenchRuntimeContext, id: FeatureId, data: Value, detach: bool) -> bool {
    let Ok(mut feature) = SketchFeature::from_json(&data) else {
        ctx.log_error("The generated sketch's numbers do not read back");
        return false;
    };
    if detach {
        feature.generator = None;
    } else if let Err(why) = regenerate(&mut feature) {
        ctx.log_warn(why);
    }
    match ctx.document.update_feature_data(id, feature.to_json()) {
        Ok(()) => {
            ctx.document.mark_feature_dirty(id);
            true
        }
        Err(e) => {
            ctx.log_error(format!("Failed to update the sketch: {e}"));
            false
        }
    }
}

/// One number: its label and a field a formula may set.
fn number_row(
    ui: &mut egui::Ui,
    ctx: &WorkbenchRuntimeContext,
    id: FeatureId,
    p: &Parameter,
    value: f64,
) -> Option<FormulaEdit> {
    let formula = ctx.document.feature_formula(id, &p.key);
    let error = ctx
        .document
        .evaluated_slots(id)
        .iter()
        .find(|s| s.key == p.key)
        .and_then(|s| s.result.as_ref().err())
        .map(String::as_str);
    let host = core_document::DocumentFormulas {
        document: ctx.document,
        dim: p.dim,
    };
    let label = p
        .label
        .rsplit_once(' ')
        .filter(|(_, n)| n.parse::<usize>().is_ok())
        .map_or(p.label.as_str(), |(l, _)| l);
    ui.horizontal(|ui| {
        label_cell(ui, label);
        FormulaField::new(egui::Id::new(("generator_field", id, &p.key)), value, &host)
            .formula(formula)
            .error(error)
            .unit(match p.dim {
                core_document::expr::Dim::LENGTH => "mm",
                core_document::expr::Dim::ANGLE => "°",
                _ => "",
            })
            .decimals(if p.integer { 0 } else { 3 })
            .speed(0.05)
            .show(ui)
    })
    .inner
}

/// What the numbers come to, or why they make nothing.
fn measured(ui: &mut egui::Ui, generator: &Generator) {
    if let Err(why) = generator.outline() {
        note_card(ui, Note::Error, Some("No profile"), &why);
        return;
    }
    let Value::Object(values) = summary(generator) else {
        return;
    };
    Card::new().show(ui, |ui| {
        for (key, value) in values {
            let Some(v) = value.as_f64() else { continue };
            ui.horizontal(|ui| {
                label_cell(ui, &key.replace('_', " "));
                mono_label(ui, format!("{v:.3} mm"), FONT_SM, TEXT1);
            });
        }
    });
}

fn label_cell(ui: &mut egui::Ui, text: &str) {
    let mut chars = text.chars();
    let text: String = chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default();
    ui.allocate_ui(egui::Vec2::new(LABEL, INPUT), |ui| {
        ui.set_min_width(LABEL);
        field_label(ui, &text);
    });
}

/// The section a shaft's parameter opens, counted from one, when it is the
/// first of that section's.
fn shaft_section_start(p: &Parameter) -> Option<usize> {
    let rest = p.pointer.strip_prefix("/generator/Shaft/sections/")?;
    let (index, field) = rest.split_once('/')?;
    (field == "length").then(|| index.parse::<usize>().ok().map(|i| i + 1))?
}

/// The generator's field a pointer lands in: `teeth`, or `sections` for
/// anything inside a shaft's sections.
fn field_of(pointer: &str) -> String {
    pointer.split('/').nth(3).unwrap_or_default().to_string()
}

fn set(data: &mut Value, pointer: &str, value: f64) {
    if let Some(slot) = data.pointer_mut(pointer) {
        *slot = json!(value);
    }
}

/// Set the field at `pointer`, adding it to its object when a file
/// written before it had none.
fn set_value(data: &mut Value, pointer: &str, value: Value) {
    let Some((parent, field)) = pointer.rsplit_once('/') else {
        return;
    };
    if let Some(object) = data.pointer_mut(parent).and_then(Value::as_object_mut) {
        object.insert(field.to_string(), value);
    }
}

fn object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

/// The fields changed and not yet recorded, kept while a drag goes on so
/// the recording takes where it ended rather than every step on the way.
fn pending_id(id: FeatureId) -> egui::Id {
    egui::Id::new(("generator_record", id))
}

fn remember(ui: &egui::Ui, ctx: &WorkbenchRuntimeContext, id: FeatureId, fields: &[String]) {
    let Some(generator) = ctx
        .document
        .get_feature_data(id)
        .and_then(|d| d.get("generator"))
        .and_then(Value::as_object)
        .and_then(|o| o.values().next())
        .cloned()
    else {
        return;
    };
    let mut pending: Map<String, Value> = ui
        .data(|d| d.get_temp::<Map<String, Value>>(pending_id(id)))
        .unwrap_or_default();
    for field in fields {
        if let Some(v) = generator.get(field) {
            pending.insert(field.clone(), v.clone());
        }
    }
    ui.data_mut(|d| d.insert_temp(pending_id(id), pending));
}

/// Record what changed once the pointer is let go.
fn flush(ui: &egui::Ui, ctx: &mut WorkbenchRuntimeContext, id: FeatureId) {
    if ui.input(|i| i.pointer.any_down()) {
        return;
    }
    let Some(mut pending) = ui.data_mut(|d| d.remove_temp::<Map<String, Value>>(pending_id(id)))
    else {
        return;
    };
    if pending.is_empty() {
        return;
    }
    pending.insert("sketch".into(), json!(id.0.to_string()));
    let answer = ctx
        .document
        .feature_values(id)
        .and_then(|d| SketchFeature::from_json(d).ok())
        .and_then(|f| f.generator)
        .map(|g| summary(&g))
        .unwrap_or(Value::Null);
    ctx.record("sketch.generator", pending, answer);
}
