//! The datum task's attachment: the mode selector and a row for each
//! reference the mode takes, filled from what is picked in the viewport.

use core_document::{
    BasePlane, BodyId, DatumAttachment, DatumFeature, EdgeAnchor, EdgeSpot, FaceAnchor, FeatureId,
    PlaneAnchor, PointAnchor, WorkbenchRuntimeContext,
};
use egui::Ui;
use ui_kit::tokens::*;
use ui_kit::widgets::{Note, accent_outline_button, mono_label, note_card};

use crate::datum_refs::{edge_anchor, face_anchor, settle};
use crate::editors::label_cell;

/// The modes the selector offers after the base planes, with what each
/// needs picked first.
const MODES: &[(&str, &str)] = &[
    ("face", "Click a face first"),
    ("three_points", ""),
    ("normal_to_edge", "Click an edge first"),
    ("along_edge", "Click an edge first"),
    ("two_points", ""),
    ("plane_intersection", ""),
    ("curve_centre", "Click a circular edge first"),
    ("inertia", "Needs the body's solid"),
];

/// What is picked in the viewport, as references in the datum's body.
struct Picked {
    face: Option<FaceAnchor>,
    edges: Vec<EdgeAnchor>,
}

impl Picked {
    fn of(ctx: &WorkbenchRuntimeContext, body: BodyId) -> Self {
        let on_body = ctx.selected_body_id == Some(body.0);
        let face = ctx
            .selected_face_in(body)
            .map(|face| face_anchor(&face, on_body));
        let edges = ctx
            .selected_edges_in(body)
            .iter()
            .map(|edge| edge_anchor(edge, edge.body == body.0))
            .collect();
        Self { face, edges }
    }

    /// The picked points: each edge where it was picked, then the face.
    fn points(&self) -> Vec<PointAnchor> {
        self.edges
            .iter()
            .map(|edge| PointAnchor::Edge {
                edge: *edge,
                spot: EdgeSpot::Picked,
            })
            .chain(self.face.map(|face| PointAnchor::Face { face }))
            .collect()
    }
}

fn add(a: [f32; 3], b: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] + b[0] * s, a[1] + b[1] * s, a[2] + b[2] * s]
}

/// The edge the datum stands on now, when its mode takes one.
fn current_edge(attachment: &DatumAttachment) -> Option<EdgeAnchor> {
    match attachment {
        DatumAttachment::NormalToEdge { edge, .. }
        | DatumAttachment::AlongEdge { edge }
        | DatumAttachment::CurveCentre { edge } => Some(*edge),
        _ => None,
    }
}

/// The attachment `mode` makes from what is picked, the datum's own frame
/// filling in the points not picked; `None` when the mode needs a pick
/// there is not.
fn candidate(
    mode: &str,
    ctx: &WorkbenchRuntimeContext,
    body: BodyId,
    datum: &DatumFeature,
    picked: &Picked,
) -> Option<DatumAttachment> {
    let frame = datum.frame();
    let edge = picked
        .edges
        .first()
        .copied()
        .or_else(|| current_edge(&datum.attachment));
    let points = |n: usize| -> Vec<PointAnchor> {
        let spare = [
            frame.origin,
            add(frame.origin, frame.x_axis, 10.0),
            add(frame.origin, frame.y_axis(), 10.0),
        ];
        let mut points = picked.points();
        points.truncate(n);
        while points.len() < n {
            points.push(PointAnchor::At {
                point: spare[points.len()],
            });
        }
        points
    };
    Some(match mode {
        "face" => DatumAttachment::Face { face: picked.face? },
        "three_points" => DatumAttachment::ThreePoints {
            points: points(3).try_into().ok()?,
        },
        "two_points" => DatumAttachment::TwoPoints {
            points: points(2).try_into().ok()?,
        },
        "normal_to_edge" => DatumAttachment::NormalToEdge {
            edge: edge?,
            spot: EdgeSpot::Picked,
        },
        "along_edge" => DatumAttachment::AlongEdge { edge: edge? },
        "curve_centre" => DatumAttachment::CurveCentre {
            edge: edge.filter(|e| e.circle.is_some())?,
        },
        "plane_intersection" => {
            let first = match picked.face {
                Some(face) => PlaneAnchor::Face { face },
                None => PlaneAnchor::Base(BasePlane::XY),
            };
            let (_, normal) = first.plane();
            // The base plane least like the first.
            let second = BasePlane::ALL
                .into_iter()
                .min_by(|a, b| {
                    let along = |p: &BasePlane| {
                        let (_, n, _) = p.frame();
                        (n[0] * normal[0] + n[1] * normal[1] + n[2] * normal[2]).abs()
                    };
                    along(a).total_cmp(&along(b))
                })
                .map(PlaneAnchor::Base)?;
            DatumAttachment::PlaneIntersection {
                planes: [first, second],
            }
        }
        "inertia" => {
            ctx.kernel?;
            ctx.document.imported_brep_blob(body)?;
            DatumAttachment::Inertia {
                centre: [0.0; 3],
                axes: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            }
        }
        _ => return None,
    })
}

fn mode_label(mode: &str) -> &'static str {
    let face = FaceAnchor {
        point: [0.0; 3],
        normal: [0.0, 0.0, 1.0],
        surface: None,
        follows: false,
    };
    let edge = EdgeAnchor {
        point: [0.0; 3],
        direction: [1.0, 0.0, 0.0],
        ends: None,
        middle: None,
        circle: None,
        follows: false,
    };
    let point = PointAnchor::At { point: [0.0; 3] };
    let plane = PlaneAnchor::Base(BasePlane::XY);
    match mode {
        "face" => DatumAttachment::Face { face },
        "three_points" => DatumAttachment::ThreePoints { points: [point; 3] },
        "normal_to_edge" => DatumAttachment::NormalToEdge {
            edge,
            spot: EdgeSpot::Picked,
        },
        "along_edge" => DatumAttachment::AlongEdge { edge },
        "two_points" => DatumAttachment::TwoPoints { points: [point; 2] },
        "plane_intersection" => DatumAttachment::PlaneIntersection { planes: [plane; 2] },
        "curve_centre" => DatumAttachment::CurveCentre { edge },
        _ => DatumAttachment::Inertia {
            centre: [0.0; 3],
            axes: [[0.0; 3]; 3],
        },
    }
    .label()
}

fn coords(p: [f32; 3]) -> String {
    format!("({:.1}, {:.1}, {:.1})", p[0], p[1], p[2])
}

/// A reference row: its label, what it holds, and a button that takes the
/// pick for it. Answers whether the button was pressed with a pick there.
fn reference_row(ui: &mut Ui, label: &str, shown: String, can_pick: bool, button: &str) -> bool {
    ui.horizontal_wrapped(|ui| {
        label_cell(ui, label);
        mono_label(ui, shown, FONT_XS, TEXT1);
        ui.add_enabled_ui(can_pick, |ui| accent_outline_button(ui, button))
            .inner
            .on_hover_text("Click it in the viewport first, then press this")
            .clicked()
            && can_pick
    })
    .inner
}

fn spot_row(ui: &mut Ui, salt: (FeatureId, usize), spot: &mut EdgeSpot, edge: &EdgeAnchor) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        label_cell(ui, "At");
        egui::ComboBox::from_id_salt(("datum_spot", salt))
            .selected_text(spot.label())
            .show_ui(ui, |ui| {
                for candidate in EdgeSpot::ALL {
                    let known = match candidate {
                        EdgeSpot::Centre => edge.circle.is_some(),
                        _ => true,
                    };
                    if ui
                        .add_enabled(
                            known,
                            egui::Button::selectable(*spot == candidate, candidate.label()),
                        )
                        .clicked()
                        && *spot != candidate
                    {
                        *spot = candidate;
                        changed = true;
                    }
                }
            });
    });
    changed
}

fn point_text(point: &PointAnchor) -> String {
    let at = coords(point.point());
    match point {
        PointAnchor::At { .. } => at,
        PointAnchor::Face { .. } => format!("{at} on a face"),
        PointAnchor::Edge { .. } => format!("{at} on an edge"),
    }
}

fn plane_row(
    ui: &mut Ui,
    ctx: &WorkbenchRuntimeContext,
    (feature_id, index): (FeatureId, usize),
    body: BodyId,
    plane: &mut PlaneAnchor,
    picked: &Picked,
) -> bool {
    let mut changed = false;
    let shown = match plane {
        PlaneAnchor::Base(base) => base.label().to_string(),
        PlaneAnchor::Datum { datum, .. } => ctx
            .document
            .get_feature_meta(*datum)
            .map(|n| n.name.clone())
            .unwrap_or_else(|| "(gone)".into()),
        PlaneAnchor::Face { face } => format!("face at {}", coords(face.point)),
    };
    // Only datums made before this one: none of them can be made from it.
    let own_seq = ctx.document.get_feature_meta(feature_id).map(|n| n.seq);
    let datums: Vec<(FeatureId, String, DatumFeature)> =
        core_document::datums_of_body(ctx.document, body)
            .into_iter()
            .filter(|(id, _, datum)| {
                crate::datum_refs::has_plane(datum)
                    && ctx
                        .document
                        .get_feature_meta(*id)
                        .is_some_and(|n| Some(n.seq) < own_seq)
            })
            .collect();
    ui.horizontal_wrapped(|ui| {
        label_cell(ui, &format!("Plane {}", index + 1));
        egui::ComboBox::from_id_salt(("datum_plane", feature_id, index))
            .selected_text(shown)
            .show_ui(ui, |ui| {
                for base in BasePlane::ALL {
                    let candidate = PlaneAnchor::Base(base);
                    if ui
                        .selectable_label(*plane == candidate, base.label())
                        .clicked()
                    {
                        *plane = candidate;
                        changed = true;
                    }
                }
                for (id, name, datum) in &datums {
                    let chosen = matches!(plane, PlaneAnchor::Datum { datum: d, .. } if d == id);
                    if ui.selectable_label(chosen, name).clicked() && !chosen {
                        let frame = datum.frame();
                        *plane = PlaneAnchor::Datum {
                            datum: *id,
                            origin: frame.origin,
                            normal: frame.normal,
                        };
                        changed = true;
                    }
                }
            });
        if ui
            .add_enabled_ui(picked.face.is_some(), |ui| {
                accent_outline_button(ui, "Use selected face")
            })
            .inner
            .on_hover_text("Click a face in the viewport first, then press this")
            .clicked()
            && let Some(face) = picked.face
        {
            *plane = PlaneAnchor::Face { face };
            changed = true;
        }
    });
    changed
}

/// The attachment part of the datum task: the mode, and the references
/// it takes. Answers whether the datum changed.
pub fn attachment_editor(
    ui: &mut Ui,
    ctx: &WorkbenchRuntimeContext,
    feature_id: FeatureId,
    datum: &mut DatumFeature,
) -> bool {
    let Some(body) = ctx
        .document
        .get_feature_meta(feature_id)
        .and_then(|n| n.body)
    else {
        return false;
    };
    let picked = Picked::of(ctx, body);
    let mut changed = false;
    let mut picks_changed = false;

    ui.horizontal(|ui| {
        label_cell(ui, "Attached to");
        egui::ComboBox::from_id_salt(("datum_attach", feature_id))
            .selected_text(datum.attachment.label())
            .show_ui(ui, |ui| {
                for plane in BasePlane::ALL {
                    let candidate = DatumAttachment::BasePlane(plane);
                    if ui
                        .selectable_label(datum.attachment == candidate, plane.label())
                        .clicked()
                        && datum.attachment != candidate
                    {
                        datum.attachment = candidate;
                        changed = true;
                    }
                }
                if matches!(datum.attachment, DatumAttachment::FlatFace { .. }) {
                    let _ = ui.selectable_label(true, datum.attachment.label());
                }
                ui.separator();
                for (mode, needs) in MODES {
                    let current = datum.attachment.mode() == *mode;
                    let made = candidate(mode, ctx, body, datum, &picked);
                    let response = ui.add_enabled(
                        current || made.is_some(),
                        egui::Button::selectable(current, mode_label(mode)),
                    );
                    let response = if needs.is_empty() {
                        response
                    } else {
                        response.on_disabled_hover_text(*needs)
                    };
                    if response.clicked()
                        && !current
                        && let Some(made) = made
                    {
                        datum.attachment = made;
                        picks_changed = true;
                    }
                }
            });
    });

    let salt = feature_id;
    match &mut datum.attachment {
        DatumAttachment::Face { face } => {
            if reference_row(
                ui,
                "Face",
                coords(face.point),
                picked.face.is_some(),
                "Use selected face",
            ) && let Some(new) = picked.face
            {
                *face = new;
                picks_changed = true;
            }
        }
        DatumAttachment::ThreePoints { points } => {
            picks_changed |= point_rows(ui, salt, points, &picked);
        }
        DatumAttachment::TwoPoints { points } => {
            picks_changed |= point_rows(ui, salt, points, &picked);
        }
        DatumAttachment::NormalToEdge { edge, spot } => {
            picks_changed |= edge_row(ui, edge, &picked);
            changed |= spot_row(ui, (salt, 0), spot, edge);
        }
        DatumAttachment::AlongEdge { edge } | DatumAttachment::CurveCentre { edge } => {
            picks_changed |= edge_row(ui, edge, &picked);
        }
        DatumAttachment::PlaneIntersection { planes } => {
            for (i, plane) in planes.iter_mut().enumerate() {
                picks_changed |= plane_row(ui, ctx, (salt, i), body, plane, &picked);
            }
        }
        DatumAttachment::Inertia { centre, .. } => {
            ui.horizontal(|ui| {
                label_cell(ui, "Centre");
                mono_label(ui, coords(*centre), FONT_XS, TEXT1);
            });
        }
        DatumAttachment::BasePlane(_) | DatumAttachment::FlatFace { .. } => {}
    }

    let mut problem = None;
    if picks_changed {
        if let Err(e) = settle(ctx, body, datum) {
            problem = Some(e);
        }
        changed = true;
    }
    if let Some(problem) = problem.or_else(|| datum.attachment.problem().map(str::to_string)) {
        ui.add_space(SPACE_1);
        note_card(ui, Note::Warning, None, &problem);
    }
    changed
}

fn edge_row(ui: &mut Ui, edge: &mut EdgeAnchor, picked: &Picked) -> bool {
    let shown = match edge.circle {
        Some(circle) => format!("circle R{:.2} at {}", circle.radius, coords(circle.center)),
        None => coords(edge.point),
    };
    if reference_row(
        ui,
        "Edge",
        shown,
        !picked.edges.is_empty(),
        "Use selected edge",
    ) && let Some(new) = picked.edges.first()
    {
        *edge = *new;
        return true;
    }
    false
}

fn point_rows(ui: &mut Ui, salt: FeatureId, points: &mut [PointAnchor], picked: &Picked) -> bool {
    let mut changed = false;
    let from_pick = picked.points().first().copied();
    for (i, point) in points.iter_mut().enumerate() {
        if reference_row(
            ui,
            &format!("Point {}", i + 1),
            point_text(point),
            from_pick.is_some(),
            "Use selected",
        ) && let Some(new) = from_pick
        {
            *point = new;
            changed = true;
        }
        if let PointAnchor::Edge { edge, spot } = point {
            let edge = *edge;
            changed |= spot_row(ui, (salt, i + 1), spot, &edge);
        }
    }
    changed
}
