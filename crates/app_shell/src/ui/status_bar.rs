//! The status bar: activity or workbench state on the left, selection
//! summary, and mono readouts on the right (coordinates, dimensions,
//! navigation style, frame counters, the document server).

use axes::AxisSystem;
use core_document::{StatusItems, Unit};
use egui::{RichText, Vec2};
use glam::Vec3;
use ui_kit::sans;
use ui_kit::tokens::*;
use ui_kit::widgets::{mono_label, vseparator};

use super::overlays::rgb;

/// The document server serving a tab, as the status bar shows it.
#[derive(Debug, Clone, Default)]
pub struct ServerBadge {
    /// What the server calls itself, for the tooltip.
    pub name: String,
    /// The app reads and writes files itself, with no server process.
    pub standalone: bool,
    pub connected: bool,
    /// Other clients editing the same document.
    pub peers: u32,
}

/// Everything the status bar reads this frame.
pub struct StatusBarInputs<'a> {
    pub fps: Option<f32>,
    pub scene_redraws_per_s: u32,
    pub hovered_point: Option<[f32; 3]>,
    pub axis_system: AxisSystem,
    pub display_unit: Unit,
    pub pending_imports: u32,
    pub pending_document_open: u32,
    pub kernel_status: Option<&'a str>,
    pub kernel_cancellable: bool,
    pub kernel_progress: Option<(u64, u64)>,
    pub server: &'a ServerBadge,
    pub document_saving: bool,
    /// Bytes packed into the archive being saved, out of the whole.
    pub save_progress: Option<(u64, u64)>,
    pub nav_style: &'a str,
    /// The connected 6-DoF mouse, when there is one.
    pub nav_device: Option<&'a str>,
    pub items: Option<&'a StatusItems>,
    pub preselect: Option<&'a str>,
    /// "w × h × d" of the selection, already formatted.
    pub dimensions: Option<&'a str>,
    /// The script running, by name, while one is.
    pub script_running: Option<&'a str>,
    /// A recording is on.
    pub recording: bool,
}

/// What the status bar's buttons asked for.
#[derive(Debug, Default, Clone, Copy)]
pub struct StatusBarResult {
    /// Stop the running kernel job.
    pub cancel_kernel: bool,
    /// Stop the running script.
    pub stop_script: bool,
    /// Stop the recording and save it.
    pub stop_recording: bool,
}

/// A thin bar, `done` of `total` filled, the counts on hover.
fn progress_bar(ui: &mut egui::Ui, done: u64, total: u64, hover: String) {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(96.0, 4.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 2.0, BG4);
    let mut fill = rect;
    fill.set_width(rect.width() * (done as f32 / total as f32).clamp(0.0, 1.0));
    ui.painter().rect_filled(fill, 2.0, ACCENT);
    response.on_hover_text(hover);
}

fn megabytes(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

fn percent(done: u64, total: u64) -> String {
    format!("{:.0}%", 100.0 * done as f64 / total as f64)
}

/// Small text in the bar's type.
fn text(ui: &mut egui::Ui, s: impl Into<String>, color: egui::Color32) -> egui::Response {
    ui.add(egui::Label::new(RichText::new(s).font(sans(FONT_XS)).color(color)).truncate())
}

/// A coloured dot of the bar's size.
fn dot(ui: &mut egui::Ui, color: egui::Color32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(6.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 3.0, color);
    response
}

/// A button that reads as text until hovered.
fn quiet_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).font(sans(FONT_XS)).color(TEXT1))
            .fill(egui::Color32::TRANSPARENT)
            .stroke(egui::Stroke::new(1.0, BORDER))
            .corner_radius(3.0)
            .min_size(Vec2::new(0.0, 16.0)),
    )
}

/// Draws the bar; says which of its buttons were pressed.
pub fn draw_status_bar(ui: &mut egui::Ui, inputs: &StatusBarInputs<'_>) -> StatusBarResult {
    let mut result = StatusBarResult::default();
    egui::Panel::bottom("status_bar")
        .exact_size(STATUS_BAR)
        .frame(
            egui::Frame::new()
                .fill(BG0)
                .inner_margin(egui::Margin::symmetric(10, 0))
                .stroke(egui::Stroke::NONE),
        )
        .show(ui, |ui| {
            let rect = ui.max_rect();
            ui.painter()
                .hline(rect.x_range(), rect.top(), egui::Stroke::new(1.0, BORDER));
            // The readouts on the right keep their room; what is happening
            // on the left is cut short before it runs under them.
            egui::containers::Sides::new()
                .height(rect.height())
                .spacing(SPACE_4)
                .shrink_left()
                .truncate()
                .show(
                    ui,
                    |ui| {
                        ui.set_clip_rect(ui.max_rect());
                        ui.spacing_mut().item_spacing.x = SPACE_4;
                        draw_left(ui, inputs, &mut result);
                    },
                    |ui| {
                        ui.spacing_mut().item_spacing.x = SPACE_3;
                        draw_right(ui, inputs);
                    },
                );
        });
    result
}

/// What is happening: the activity, a recording, a script, the selection.
fn draw_left(ui: &mut egui::Ui, inputs: &StatusBarInputs<'_>, result: &mut StatusBarResult) {
    draw_activity(ui, inputs, &mut result.cancel_kernel);
    if inputs.recording {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = SPACE_2;
            dot(ui, DANGER);
            text(ui, "Recording", TEXT1);
            if quiet_button(ui, "Stop")
                .on_hover_text("Save what was recorded as a new script")
                .clicked()
            {
                result.stop_recording = true;
            }
        });
    }
    if let Some(script) = inputs.script_running {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = SPACE_2;
            ui.add(egui::Spinner::new().size(11.0).color(ACCENT));
            text(ui, format!("Running {script}"), TEXT1);
            if quiet_button(ui, "Stop").clicked() {
                result.stop_script = true;
            }
        });
    }
    let (what, name) = match (
        inputs.items.and_then(|i| i.selection.as_deref()),
        inputs.preselect,
    ) {
        (Some(sel), _) => ("Selected", sel),
        (None, Some(pre)) => ("Under the cursor", pre),
        (None, None) => return,
    };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = SPACE_1;
        text(ui, what, TEXT3);
        text(ui, name, TEXT1);
    });
}

/// The readouts, laid out from the right edge: navigation, performance,
/// the server, then what the bench or the cursor says.
fn draw_right(ui: &mut egui::Ui, inputs: &StatusBarInputs<'_>) {
    text(ui, inputs.nav_style, TEXT3).on_hover_text("Navigation style (Preferences › Input)");
    if let Some(device) = inputs.nav_device {
        text(ui, device, TEXT3).on_hover_text("6-DoF mouse connected");
    }
    vseparator(ui, 12.0);

    // Two numbers because they are two things: UI frames presented, and
    // how often the 3D scene was drawn again under them.
    let fps = match inputs.fps {
        Some(fps) if fps > 0.0 => format!("{fps:.0} fps"),
        Some(_) => "… fps".to_string(),
        // The loop is about to sleep; a frozen number would read as a live
        // measurement.
        None => "idle".to_string(),
    };
    let scene = if inputs.fps.is_none() || inputs.scene_redraws_per_s == 0 {
        "cached".to_string()
    } else {
        format!("{}/s", inputs.scene_redraws_per_s)
    };
    mono_label(ui, format!("{fps} · {scene}"), FONT_XS, TEXT3).on_hover_text(
        "Frames drawn per second, and how often the 3D scene was drawn again \
         (cached while nothing in it changes)",
    );
    vseparator(ui, 12.0);
    draw_server(ui, inputs.server);

    let coords = coords_text(inputs);
    let mode = inputs.items.and_then(|i| i.mode.as_deref());
    if coords.is_some() || inputs.dimensions.is_some() || mode.is_some() {
        vseparator(ui, 12.0);
    }
    if let Some(mode) = mode {
        text(ui, mode, TEXT2);
    }
    if let Some(dim) = inputs.dimensions {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = SPACE_1;
            mono_label(ui, dim, FONT_XS, TEXT2);
            text(ui, "Size", TEXT3);
        })
        .response
        .on_hover_text("The selection's bounding box");
    }
    if let Some(coords) = coords {
        mono_label(ui, coords, FONT_XS, TEXT2).on_hover_text("The point under the cursor");
    }
}

/// The document server: a dot while all is well, words when it is not.
fn draw_server(ui: &mut egui::Ui, server: &ServerBadge) {
    let (color, words, hover) = if server.standalone {
        (
            WARNING,
            Some("No server".to_string()),
            "The document server is not running: printCAD reads and writes files \
             itself, with no edit log beside them and no one else editing"
                .to_string(),
        )
    } else if !server.connected {
        (
            DANGER,
            Some("Server lost".to_string()),
            format!(
                "The connection to the {} was lost; save to keep your work",
                server.name
            ),
        )
    } else if server.peers > 0 {
        let peers = if server.peers == 1 {
            "1 peer".to_string()
        } else {
            format!("{} peers", server.peers)
        };
        (
            ACCENT,
            Some(peers.clone()),
            format!("Served by the {}, with {peers} editing too", server.name),
        )
    } else {
        (SUCCESS, None, format!("Served by the {}", server.name))
    };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = SPACE_1;
        if let Some(words) = words {
            text(ui, words, TEXT2);
        }
        dot(
            ui,
            color.gamma_multiply(if server.connected && !server.standalone {
                0.7
            } else {
                1.0
            }),
        );
    })
    .response
    .on_hover_text(hover);
}

/// Where the cursor is, while it is over something (or what the bench
/// says in its place): each axis by its direction, in the document's unit.
fn coords_text(inputs: &StatusBarInputs<'_>) -> Option<String> {
    if let Some(coords) = inputs.items.and_then(|i| i.coords.as_deref()) {
        return Some(coords.to_owned());
    }
    let pos = inputs.hovered_point?;
    let canonical = inputs
        .axis_system
        .world_to_canonical(Vec3::from_array(pos))
        .to_array();
    let axes = [
        inputs.axis_system.horizontal(),
        inputs.axis_system.vertical(),
        inputs.axis_system.depth(),
    ];
    let unit = inputs.display_unit;
    let values = axes
        .iter()
        .zip(canonical)
        .map(|(axis, mm)| format!("{} {:.2}", axis.signed_label(), unit.from_mm(mm)))
        .collect::<Vec<_>>()
        .join("  ");
    Some(format!("{values} {}", unit.short_label()))
}

/// The far-left slot: a state dot with a title, or, while the kernel, an
/// open or a save is busy, a spinner or a bar with the announced stage and
/// a Cancel button.
fn draw_activity(ui: &mut egui::Ui, inputs: &StatusBarInputs<'_>, cancel: &mut bool) {
    let busy = inputs.pending_imports > 0 || inputs.pending_document_open > 0;
    if !busy {
        let (color, title) = match inputs.items.and_then(|i| i.state.clone()) {
            Some((rgb_color, title)) => (rgb(rgb_color, 1.0), title),
            None => (SUCCESS, "Ready".to_string()),
        };
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            dot(ui, color);
            text(ui, title, TEXT2);
        });
        return;
    }

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = SPACE_2;
        // A stage that announced its counts earns a real bar; anything
        // else keeps the honest spinner.
        match (inputs.kernel_progress, inputs.save_progress) {
            (Some((done, total)), _) if total > 0 => {
                progress_bar(ui, done, total, format!("{done} of {total}"));
                mono_label(ui, percent(done, total), FONT_XS, TEXT2);
            }
            // A save counts bytes, which read better as megabytes.
            (None, Some((done, total))) if total > 0 => {
                progress_bar(
                    ui,
                    done,
                    total,
                    format!("{:.1} of {:.1} MB", megabytes(done), megabytes(total)),
                );
                mono_label(ui, percent(done, total), FONT_XS, TEXT2);
            }
            _ => {
                ui.add(egui::Spinner::new().size(11.0).color(ACCENT));
            }
        }
        let mut parts = Vec::new();
        match inputs.kernel_status {
            Some(status) => parts.push(status.to_owned()),
            // Nothing announced yet: say only what is certain, which is
            // how many jobs are outstanding.
            None if inputs.pending_imports == 1 => parts.push("Working…".to_string()),
            None if inputs.pending_imports > 1 => {
                parts.push(format!("Working… ({} jobs)", inputs.pending_imports));
            }
            None => {}
        }
        if inputs.document_saving {
            parts.push("Saving…".to_string());
        } else if inputs.pending_document_open > 0 {
            parts.push("Opening…".to_string());
        }
        ui.label(
            RichText::new(parts.join(" · "))
                .font(sans(FONT_XS))
                .color(TEXT1),
        );
        if inputs.kernel_cancellable && quiet_button(ui, "Cancel").clicked() {
            *cancel = true;
        }
    });
}
