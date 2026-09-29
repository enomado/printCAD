//! Rigid components as the solver reads them: every body of one held where
//! it sits against the others, the joints between them resting, and a
//! move of any of them moving them all.

use core_document::{BodyId, BodyPlacement, Document, FeatureId};

use crate::joint::{Anchor, JointFeature, JointKind, Rigid, relative};
use crate::solve::Joint;

/// Whether `a` and `b` move as one.
pub fn together(document: &Document, a: BodyId, b: BodyId) -> bool {
    a != b && document.rigid_unit_of(a).contains(&b)
}

/// Each rigid component's bodies held to one of them where they sit now,
/// under the component's name. The one held to is a body with a joint of
/// its own in `joints`, when one has, so a component joined to the rest
/// moves with that joint.
pub fn holds(document: &Document, joints: &[Joint]) -> Vec<Joint> {
    let origin = Anchor::Plane {
        point: [0.0; 3],
        normal: [0.0, 0.0, 1.0],
    };
    let at = |b: BodyId| Rigid::from(document.body_placement(b));
    document
        .rigid_units()
        .into_iter()
        .flat_map(|unit| {
            let Some(root) = document
                .rigid_root(unit[0])
                .and_then(|r| document.component(r))
            else {
                return Vec::new();
            };
            let base = unit
                .iter()
                .copied()
                .find(|b| {
                    joints
                        .iter()
                        .any(|j| j.body == *b && j.feature.kind != JointKind::Ground)
                })
                .unwrap_or(unit[0]);
            let id = FeatureId(root.id.0);
            unit.iter()
                .filter(|b| **b != base)
                .map(|b| {
                    let (turn, shift) = relative(&at(*b), &at(base));
                    Joint {
                        id,
                        name: root.name.clone(),
                        body: *b,
                        feature: JointFeature {
                            second: None,
                            shape: Vec::new(),
                            ends: [0.0; 2],
                            names: [0; 2],
                            kind: JointKind::Fixed {
                                turn: turn.to_array(),
                                shift: shift.to_array(),
                            },
                            moving: origin,
                            other_body: base,
                            fixed: origin,
                        },
                    }
                })
                .collect()
        })
        .collect()
}

/// Whether a joint of `body`'s own, or of a body that moves with it, ties
/// it to something outside: what lets the solver move it.
pub fn joined_outside(document: &Document, body: BodyId, joints: &[Joint]) -> bool {
    let unit = document.rigid_unit_of(body);
    joints.iter().any(|j| {
        unit.contains(&j.body)
            && (j.feature.kind == JointKind::Ground || !unit.contains(&j.feature.other_body))
    })
}

/// Move `body` to `placement`, and every body that moves as one with it
/// by the same.
pub fn move_with_unit(document: &mut Document, body: BodyId, placement: BodyPlacement) {
    let step = placement.after(&document.body_placement(body).inverse());
    for member in document.rigid_unit_of(body) {
        let moved = if member == body {
            placement
        } else {
            step.after(&document.body_placement(member))
        };
        document.set_body_placement(member, moved);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::{Quat, Vec3};

    #[test]
    fn a_rigid_component_moves_as_one_and_a_flexible_one_does_not() {
        let mut doc = Document::new("t");
        let (a, b) = (
            doc.create_body(Some("A".into())),
            doc.create_body(Some("B".into())),
        );
        doc.set_body_placement(b, BodyPlacement::new(Quat::IDENTITY, Vec3::X * 10.0));
        let c = doc.create_component("Pair".into(), None).unwrap();
        doc.set_body_component(a, Some(c)).unwrap();
        doc.set_body_component(b, Some(c)).unwrap();
        let turn = BodyPlacement::new(Quat::from_rotation_z(90f32.to_radians()), Vec3::Z * 5.0);
        move_with_unit(&mut doc, a, turn);
        let b_at = doc.body_placement(b).offset();
        assert!(b_at.distance(Vec3::new(0.0, 10.0, 5.0)) < 1e-4, "{b_at}");
        assert_eq!(holds(&doc, &[]).len(), 1);

        let mut flexible = doc.component(c).unwrap().clone();
        flexible.flexible = true;
        doc.update_component(flexible).unwrap();
        move_with_unit(&mut doc, a, BodyPlacement::IDENTITY);
        assert!(doc.body_placement(b).offset().distance(b_at) < 1e-4);
        assert!(holds(&doc, &[]).is_empty());
    }
}
