//! A body's tip is the point in its history on show: a new feature goes in
//! right after it, the tip follows the new feature, and moving the tip on
//! its own is navigation, not an edit to undo.

use core_document::datum::{DatumAttachment, DatumFeature, DatumShape};
use core_document::history::OpJournal;
use core_document::{BasePlane, BodyId, Document, FeatureId};

fn datum() -> DatumFeature {
    DatumFeature {
        shape: DatumShape::Plane { size: 10.0 },
        attachment: DatumAttachment::BasePlane(BasePlane::XY),
        offset: Default::default(),
    }
}

/// The body's features, in history order.
fn history(doc: &Document, body: BodyId) -> Vec<FeatureId> {
    let mut all: Vec<(u64, FeatureId)> = doc
        .feature_tree()
        .all_nodes()
        .filter(|(_, n)| n.body == Some(body))
        .map(|(id, n)| (n.seq, *id))
        .collect();
    all.sort();
    all.into_iter().map(|(_, id)| id).collect()
}

fn tip(doc: &Document, body: BodyId) -> Option<FeatureId> {
    doc.bodies()
        .iter()
        .find(|b| b.id == body)
        .and_then(|b| b.tip)
}

fn add(doc: &mut Document, body: BodyId, name: &str) -> FeatureId {
    doc.add_feature_in_body(datum(), name.into(), Some(body))
        .unwrap()
}

#[test]
fn features_added_at_an_earlier_point_go_in_there_and_the_tip_follows() {
    let mut doc = Document::new("t");
    let body = doc.create_body(None);
    let a = add(&mut doc, body, "a");
    let b = add(&mut doc, body, "b");
    let c = add(&mut doc, body, "c");

    // At the end of history nothing moves.
    assert_eq!(history(&doc, body), [a, b, c]);
    assert_eq!(tip(&doc, body), None);

    // Back at `a`: two features go in after it, in the order made.
    doc.set_body_tip(body, Some(a));
    let x = add(&mut doc, body, "x");
    assert_eq!(history(&doc, body), [a, x, b, c]);
    assert_eq!(tip(&doc, body), Some(x));
    let y = add(&mut doc, body, "y");
    assert_eq!(history(&doc, body), [a, x, y, b, c]);
    assert_eq!(tip(&doc, body), Some(y));

    // Another body's history is not touched.
    let other = doc.create_body(None);
    let z = add(&mut doc, other, "z");
    assert_eq!(history(&doc, other), [z]);
    assert_eq!(history(&doc, body), [a, x, y, b, c]);
}

#[test]
fn moving_the_tip_alone_is_no_undo_step_and_an_insert_undoes_whole() {
    let mut doc = Document::new("t");
    let mut journal = OpJournal::new(50);
    let body = doc.create_body(None);
    let a = add(&mut doc, body, "a");
    let b = add(&mut doc, body, "b");
    journal.note(&mut doc);

    // Navigation: the tip moves back to `a`, and undo still takes back
    // the adds, the one step there is.
    doc.set_body_tip(body, Some(a));
    journal.note(&mut doc);
    journal.undo(&mut doc).expect("the adds");
    assert!(doc.get_feature_meta(a).is_none() && doc.get_feature_meta(b).is_none());
    assert!(!journal.can_undo(), "the tip move made no step of its own");
    journal.redo(&mut doc);
    doc.set_body_tip(body, Some(a));
    journal.note(&mut doc);

    // An insert at the tip and the tip moving with it are one step.
    let x = add(&mut doc, body, "x");
    journal.note(&mut doc);
    assert_eq!(history(&doc, body), [a, x, b]);
    journal.undo(&mut doc).expect("the insert");
    assert!(doc.get_feature_meta(x).is_none());
    assert_eq!(tip(&doc, body), Some(a), "the tip goes back with it");
    assert_eq!(history(&doc, body), [a, b]);
}
