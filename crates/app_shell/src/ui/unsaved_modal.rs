//! The question a document with unsaved edits asks before it is closed:
//! drawn in the window, so it needs nothing of the system, and answered
//! when the user chooses (the close waits for it).

use egui::{Context, RichText};
use ui_kit::sans_semibold;
use ui_kit::tokens::*;
use ui_kit::widgets::{destructive_button, primary_button, secondary_button};

use super::UiCommand;
use crate::app::unsaved::Unsaved;

/// Ask about the document `name`; the answer, once given, as a command.
pub fn draw(ctx: &Context, name: &str, commands: &mut Vec<UiCommand>) {
    let frame = egui::Frame::new()
        .fill(BG1)
        .stroke(egui::Stroke::new(1.0, BORDER_STRONG))
        .corner_radius(RADIUS_LG as u8)
        .shadow(SHADOW_DIALOG)
        .inner_margin(egui::Margin::symmetric(18, 16));
    let mut answer = None;
    egui::Modal::new(egui::Id::new("unsaved_modal"))
        .frame(frame)
        .backdrop_color(egui::Color32::from_black_alpha(140))
        .show(ctx, |ui| {
            ui.set_width(400.0);
            ui.spacing_mut().item_spacing.y = SPACE_2;
            ui.label(
                RichText::new(format!("Save changes to {name}?"))
                    .font(sans_semibold(FONT_LG))
                    .color(TEXT1),
            );
            ui.label(
                RichText::new("Your edits are lost if you don't save them.")
                    .font(ui_kit::sans(FONT_SM))
                    .color(TEXT2),
            );
            ui.add_space(SPACE_2);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if primary_button(ui, "Save").clicked() {
                    answer = Some(Unsaved::Save);
                }
                if secondary_button(ui, "Cancel").clicked() {
                    answer = Some(Unsaved::Cancel);
                }
                if destructive_button(ui, "Don't save").clicked() {
                    answer = Some(Unsaved::Discard);
                }
            });
        });
    if answer.is_none() && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        answer = Some(Unsaved::Cancel);
    }
    if let Some(answer) = answer {
        commands.push(UiCommand::AnswerUnsaved(answer));
    }
}
