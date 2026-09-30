//! Painters for the viewport annotations workbenches emit: constant-width
//! lines, point markers and icon glyphs, text labels, and the orbit pivot.
//! Coordinates arrive in physical pixels relative to the viewport origin.

use egui::{Color32, Context};

pub fn rgb(color: [f32; 3], alpha: f32) -> Color32 {
    Color32::from_rgb(
        (color[0] * 255.0) as u8,
        (color[1] * 255.0) as u8,
        (color[2] * 255.0) as u8,
    )
    .gamma_multiply(alpha)
}

fn viewport_painter(ctx: &Context, viewport_rect: egui::Rect, id: &'static str) -> egui::Painter {
    // Background order draws beneath UI panels and on top of the 3D scene,
    // which is composited separately; clip to the viewport area.
    let layer_id = egui::LayerId::new(egui::Order::Background, egui::Id::new(id));
    ctx.layer_painter(layer_id).with_clip_rect(viewport_rect)
}

/// How far a line's dark rim reaches past it on each side, in pixels.
const RIM_PX: f32 = 1.0;
/// How dark the rim is at a line's full strength.
const RIM_ALPHA: f32 = 0.55;
/// The strength from which a line gets a rim: fainter lines (the grid,
/// guides) are background, and stay unrimmed.
const RIMMED_FROM: f32 = 0.6;

/// Where `overlay` starts and ends on screen.
fn ends(
    viewport_rect: egui::Rect,
    overlay: &core_document::ScreenSpaceOverlay,
    ppp: f32,
) -> (egui::Pos2, egui::Pos2) {
    (
        egui::pos2(
            viewport_rect.min.x + overlay.start[0] / ppp,
            viewport_rect.min.y + overlay.start[1] / ppp,
        ),
        egui::pos2(
            viewport_rect.min.x + overlay.end[0] / ppp,
            viewport_rect.min.y + overlay.end[1] / ppp,
        ),
    )
}

/// Draw the pictures a bench lays over the viewport, each a textured quad
/// between its corners; a picture no longer shown lets its texture go.
pub fn draw_screen_space_images(
    ctx: &Context,
    viewport_rect: egui::Rect,
    images: &[core_document::ScreenSpaceImage],
    textures: &mut std::collections::HashMap<u64, egui::TextureHandle>,
) {
    textures.retain(|key, _| images.iter().any(|i| i.key == *key));
    if images.is_empty() {
        return;
    }
    let ppp = ctx.pixels_per_point();
    let painter = viewport_painter(ctx, viewport_rect, "screen_space_images");
    for image in images {
        let texture = textures.entry(image.key).or_insert_with(|| {
            let pixels = egui::ColorImage::from_rgba_unmultiplied(
                [image.size[0] as usize, image.size[1] as usize],
                &image.rgba,
            );
            ctx.load_texture(
                format!("screen_image::{}", image.key),
                pixels,
                egui::TextureOptions::LINEAR,
            )
        });
        let mut mesh = egui::Mesh::with_texture(texture.id());
        let tint = Color32::from_white_alpha((image.opacity.clamp(0.0, 1.0) * 255.0) as u8);
        let uv = [
            egui::pos2(0.0, 0.0),
            egui::pos2(1.0, 0.0),
            egui::pos2(1.0, 1.0),
            egui::pos2(0.0, 1.0),
        ];
        for (corner, uv) in image.corners.iter().zip(uv) {
            let pos = egui::pos2(
                viewport_rect.min.x + corner[0] / ppp,
                viewport_rect.min.y + corner[1] / ppp,
            );
            mesh.vertices.push(egui::epaint::Vertex {
                pos,
                uv,
                color: tint,
            });
        }
        mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
        painter.add(egui::Shape::mesh(mesh));
    }
}

/// Draw constant-thickness lines in the viewport area.
pub fn draw_screen_space_overlays(
    ctx: &Context,
    viewport_rect: egui::Rect,
    overlays: &[core_document::ScreenSpaceOverlay],
) {
    if overlays.is_empty() {
        return;
    }
    let ppp = ctx.pixels_per_point();
    let painter = viewport_painter(ctx, viewport_rect, "screen_space_overlays");
    // A dark rim under every strong line first, so a light line reads on a
    // light face and a coloured one on a face of its colour; the rims all
    // go down before any line, so none covers a neighbour.
    for overlay in overlays.iter().filter(|o| o.alpha >= RIMMED_FROM) {
        let (start, end) = ends(viewport_rect, overlay, ppp);
        let rim = egui::Stroke::new(
            (overlay.thickness + 2.0 * RIM_PX) / ppp,
            Color32::from_black_alpha((RIM_ALPHA * overlay.alpha * 255.0) as u8),
        );
        match overlay.dash {
            Some((dash, gap)) => {
                painter.add(egui::Shape::dashed_line(
                    &[start, end],
                    rim,
                    dash / ppp,
                    gap / ppp,
                ));
            }
            None => {
                painter.line_segment([start, end], rim);
            }
        }
    }
    for overlay in overlays {
        let start = egui::pos2(
            viewport_rect.min.x + overlay.start[0] / ppp,
            viewport_rect.min.y + overlay.start[1] / ppp,
        );
        let end = egui::pos2(
            viewport_rect.min.x + overlay.end[0] / ppp,
            viewport_rect.min.y + overlay.end[1] / ppp,
        );
        let stroke = egui::Stroke::new(overlay.thickness / ppp, rgb(overlay.color, overlay.alpha));
        match overlay.dash {
            Some((dash, gap)) => {
                painter.add(egui::Shape::dashed_line(
                    &[start, end],
                    stroke,
                    dash / ppp,
                    gap / ppp,
                ));
            }
            None => {
                painter.line_segment([start, end], stroke);
            }
        }
    }
}

/// Draw point markers and icon glyphs, above the lines and beneath the
/// labels.
pub fn draw_screen_space_marks(
    ctx: &Context,
    viewport_rect: egui::Rect,
    marks: &[core_document::ScreenSpaceMark],
) {
    if marks.is_empty() {
        return;
    }
    let ppp = ctx.pixels_per_point();
    let painter = viewport_painter(ctx, viewport_rect, "screen_space_marks");
    let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    for mark in marks {
        let pos = egui::pos2(
            viewport_rect.min.x + mark.pos[0] / ppp,
            viewport_rect.min.y + mark.pos[1] / ppp,
        );
        let color = rgb(mark.color, mark.alpha);
        match mark.kind {
            core_document::MarkKind::Dot { radius } => {
                painter.circle_filled(pos, radius / ppp, color);
            }
            core_document::MarkKind::Crosshair { size } => {
                let h = size / ppp / 2.0;
                let stroke = egui::Stroke::new(1.0, color);
                painter.line_segment(
                    [egui::pos2(pos.x - h, pos.y), egui::pos2(pos.x + h, pos.y)],
                    stroke,
                );
                painter.line_segment(
                    [egui::pos2(pos.x, pos.y - h), egui::pos2(pos.x, pos.y + h)],
                    stroke,
                );
            }
            core_document::MarkKind::Icon { name, size } => {
                if let Some(tex) = ui_kit::icon::texture(ctx, name) {
                    let rect = egui::Rect::from_center_size(pos, egui::Vec2::splat(size / ppp));
                    painter.image(tex.id(), rect, uv, color);
                }
            }
        }
    }
}

/// Draw text labels: dimension values, constraint suffixes, readouts.
pub fn draw_screen_space_labels(
    ctx: &Context,
    viewport_rect: egui::Rect,
    labels: &[core_document::ScreenSpaceLabel],
) {
    if labels.is_empty() {
        return;
    }
    let ppp = ctx.pixels_per_point();
    let painter = viewport_painter(ctx, viewport_rect, "screen_space_labels");
    for label in labels {
        let pos = egui::pos2(
            viewport_rect.min.x + label.pos[0] / ppp,
            viewport_rect.min.y + label.pos[1] / ppp,
        );
        let color = rgb(label.color, 1.0);
        let font = if label.mono {
            ui_kit::mono(label.size / ppp)
        } else {
            ui_kit::sans(label.size / ppp)
        };
        let galley = painter.layout_no_wrap(label.text.clone(), font, color);
        let rect = egui::Rect::from_center_size(pos, galley.size());
        if label.background {
            painter.rect_filled(
                rect.expand2(egui::Vec2::from(ui_kit::tokens::PILL_PAD) / ppp),
                ui_kit::tokens::RADIUS_SM,
                ui_kit::tokens::BG0,
            );
        }
        painter.galley(rect.min, galley, color);
    }
}

/// The orbit pivot marker: a small ringed dot where the next orbit turns.
pub fn draw_pivot_indicator(ctx: &Context, x: f32, y: f32) {
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("pivot_indicator"),
    ));
    let ppp = ctx.pixels_per_point();
    let pos = egui::pos2(x / ppp, y / ppp);
    let accent = ui_kit::tokens::ACCENT;
    painter.circle(
        pos,
        8.0,
        ui_kit::tokens::ACCENT_DIM,
        egui::Stroke::new(1.5, accent),
    );
    let cross = 4.0;
    let stroke = egui::Stroke::new(1.5, ui_kit::tokens::TEXT1);
    painter.line_segment(
        [
            egui::pos2(pos.x - cross, pos.y),
            egui::pos2(pos.x + cross, pos.y),
        ],
        stroke,
    );
    painter.line_segment(
        [
            egui::pos2(pos.x, pos.y - cross),
            egui::pos2(pos.x, pos.y + cross),
        ],
        stroke,
    );
}
