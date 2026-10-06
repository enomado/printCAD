//! The Assembly's declared panels: each makes the edits, and records the
//! calls, its fields and buttons stand for.

use super::*;
use crate::tests::{face_up, frame, scene, task_frame};
use core_document::{Document, Recorded, Workbench, WorkbenchInputEvent};

fn context(doc: &mut Document) -> WorkbenchRuntimeContext<'_> {
    WorkbenchRuntimeContext::new(doc, [0.0; 3], [0.0; 3], (0, 0, 800, 600))
}

/// Hand `event` to the open task's panel, as the renderer hands it; what
/// that closed with, and what it recorded.
fn send(
    wb: &mut AssemblyWorkbench,
    doc: &mut Document,
    event: PanelEvent,
) -> (Option<TaskOutcome>, Vec<Recorded>) {
    let mut ctx = context(doc);
    let outcome = wb.task_event(&mut ctx, &event);
    (outcome, core_document::HookOutcome::take(&mut ctx).recorded)
}

fn number(id: &str, value: f64) -> PanelEvent {
    PanelEvent::Number {
        id: id.into(),
        value,
    }
}

fn button(id: &str) -> PanelEvent {
    PanelEvent::Button { id: id.into() }
}

fn widgets(wb: &AssemblyWorkbench, doc: &mut Document) -> Vec<Widget> {
    wb.task_widgets(&context(doc))
}

/// Every widget, those in rows, hints and groups too.
fn flat(widgets: &[Widget]) -> Vec<&Widget> {
    let mut all = Vec::new();
    for widget in widgets {
        all.push(widget);
        match widget {
            Widget::Row { children } | Widget::Group { children, .. } => all.extend(flat(children)),
            Widget::Hinted { widget, .. } => all.extend(flat(std::slice::from_ref(widget))),
            _ => {}
        }
    }
    all
}

/// The widget with id `id`.
fn field<'a>(widgets: &'a [Widget], id: &str) -> Option<&'a Widget> {
    flat(widgets).into_iter().find(|w| match w {
        Widget::Number { id: i, .. }
        | Widget::Choice { id: i, .. }
        | Widget::Toggle { id: i, .. }
        | Widget::TextField { id: i, .. }
        | Widget::Button { id: i, .. }
        | Widget::Slider { id: i, .. }
        | Widget::List { id: i, .. }
        | Widget::Sheet { id: i, .. } => i == id,
        _ => false,
    })
}

fn value_of(widgets: &[Widget], id: &str) -> f64 {
    match field(widgets, id) {
        Some(Widget::Number { value, .. } | Widget::Slider { value, .. }) => *value,
        other => panic!("{id}: {other:?}"),
    }
}

/// Everything the panel says, in words.
fn words(widgets: &[Widget]) -> String {
    flat(widgets)
        .iter()
        .filter_map(|w| match w {
            Widget::Text { text, .. } | Widget::Heading { text } => Some(text.clone()),
            Widget::Note { title, text, .. } => {
                Some(format!("{} {text}", title.clone().unwrap_or_default()))
            }
            Widget::Value { label, value, .. } => Some(format!("{label}: {value}")),
            Widget::Header { title, .. } => Some(title.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The panel drawn once through the renderer, nothing touched: it draws,
/// records nothing and leaves the task open.
fn draws(wb: &mut AssemblyWorkbench, doc: &mut Document) {
    let recorded = task_frame(wb, doc, TaskRequest::default());
    assert!(recorded.is_empty(), "{recorded:?}");
    assert!(wb.task.is_some() || wb.picking.is_some());
}

fn accept(wb: &mut AssemblyWorkbench, doc: &mut Document) -> Vec<Recorded> {
    task_frame(
        wb,
        doc,
        TaskRequest {
            accept: true,
            cancel: false,
        },
    )
}

fn close_to(a: [f32; 3], b: [f32; 3]) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() < 1e-4)
}

/// Two bodies mated by picks, the joint's task open.
fn mated() -> (AssemblyWorkbench, Document, BodyId, BodyId, FeatureId) {
    let (mut doc, base, part) = scene();
    let mut wb = AssemblyWorkbench::default();
    wb.on_input(
        &WorkbenchInputEvent::ToolActivated,
        Some("asm.mate"),
        &mut context(&mut doc),
    );
    frame(&mut wb, &mut doc, Some((part, face_up(40.0))));
    frame(&mut wb, &mut doc, Some((base, face_up(0.0))));
    let Some(Task::Joint { id, .. }) = wb.task.clone() else {
        panic!("the mate's task is open");
    };
    (wb, doc, base, part, id)
}

#[test]
fn the_move_panel_moves_the_body_as_asm_place_does_and_records_it() {
    let (mut doc, _, part) = scene();
    let mut wb = AssemblyWorkbench {
        task: Some(Task::Move {
            body: part,
            placements: crate::all_placements(&context(&mut doc)),
        }),
        ..AssemblyWorkbench::default()
    };
    draws(&mut wb, &mut doc);
    let panel = widgets(&wb, &mut doc);
    assert_eq!(value_of(&panel, "x"), 30.0);
    assert_eq!(value_of(&panel, "z"), 40.0);
    let before = doc.clone();

    send(&mut wb, &mut doc, number("x", 12.0));
    send(&mut wb, &mut doc, number("turn_z", 90.0));
    let mut by_hand = before;
    crate::components::move_with_unit(
        &mut by_hand,
        part,
        BodyPlacement::new(
            glam::Quat::from_euler(glam::EulerRot::XYZ, 0.0, 0.0, 90f32.to_radians()),
            glam::Vec3::new(12.0, 0.0, 40.0),
        ),
    );
    let (moved, want) = (doc.body_placement(part), by_hand.body_placement(part));
    assert!(close_to(moved.translation, want.translation), "{moved:?}");
    assert!(moved.quat().angle_between(want.quat()) < 1e-4, "{moved:?}");
    assert_eq!(value_of(&widgets(&wb, &mut doc), "turn_z").round(), 90.0);

    let recorded = accept(&mut wb, &mut doc);
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].id, "asm.place");
    assert_eq!(
        recorded[0].args["translation"],
        serde_json::json!(moved.translation)
    );

    wb.task = Some(Task::Move {
        body: part,
        placements: Vec::new(),
    });
    send(&mut wb, &mut doc, button("home"));
    assert_eq!(doc.body_placement(part), BodyPlacement::IDENTITY);
}

#[test]
fn a_body_its_joints_place_shows_its_place_without_fields() {
    let (mut wb, mut doc, _, part, _) = mated();
    wb.task = Some(Task::Move {
        body: part,
        placements: Vec::new(),
    });
    let panel = widgets(&wb, &mut doc);
    assert!(field(&panel, "x").is_none());
    assert!(field(&panel, "home").is_none());
    assert!(words(&panel).contains("Placed by its joints"));
    assert!(words(&panel).contains("Position X: "));
    draws(&mut wb, &mut doc);
}
