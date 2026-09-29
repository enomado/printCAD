//! Exploded views kept in the document: ordered steps, each moving some
//! bodies by a shift, played from the assembly as it stands. Showing one
//! moves nothing for good: the bodies go back when the view closes.

use core_document::{
    BodyId, BodyPlacement, Document, DocumentResult, FeatureError, FeatureId, WorkbenchFeature,
    WorkbenchId,
};
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// The feature kind exploded views are stored as.
pub const EXPLODED_KIND: &str = "wb.assembly.exploded";

/// One step: these bodies moved by `shift` (world, mm).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExplodeStep {
    pub bodies: Vec<BodyId>,
    pub shift: [f32; 3],
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ExplodedView {
    pub steps: Vec<ExplodeStep>,
}

impl WorkbenchFeature for ExplodedView {
    fn workbench_id() -> WorkbenchId {
        WorkbenchId::from(EXPLODED_KIND)
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    fn from_json(value: &serde_json::Value) -> DocumentResult<Self> {
        serde_json::from_value(value.clone()).map_err(|e| {
            core_document::DocumentError::Feature(FeatureError::Deserialization(e.to_string()))
        })
    }

    fn dependencies(&self) -> Vec<FeatureId> {
        Vec::new()
    }

    fn name(&self) -> &str {
        "Exploded view"
    }
}

impl ExplodedView {
    /// Every body `start` names, where the view has it at `at` steps in:
    /// each step whole before it, the one it is in by the share it has got
    /// through.
    pub fn placed_at(
        &self,
        start: &[(BodyId, BodyPlacement)],
        at: f32,
    ) -> Vec<(BodyId, BodyPlacement)> {
        start
            .iter()
            .map(|(body, placement)| {
                let by: Vec3 = self
                    .steps
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.bodies.contains(body))
                    .map(|(i, s)| Vec3::from_array(s.shift) * (at - i as f32).clamp(0.0, 1.0))
                    .sum();
                (
                    *body,
                    BodyPlacement::new(placement.quat(), placement.offset() + by),
                )
            })
            .collect()
    }
}

/// The view `id` as stored.
pub fn view_of(document: &Document, id: FeatureId) -> Option<ExplodedView> {
    ExplodedView::from_json(document.get_feature_data(id)?).ok()
}

/// The middle of `body`'s box where `placement` puts it.
pub fn centre(document: &Document, body: BodyId, placement: &BodyPlacement) -> Option<[f32; 3]> {
    let (mesh, bounds) = document.local_geometry(body)?;
    let (lo, hi) = bounds.or_else(|| mesh.bounds())?;
    let middle = (Vec3::from_array(lo) + Vec3::from_array(hi)) * 0.5;
    Some(placement.point(middle.to_array()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_play_in_order_each_by_its_share() {
        let (a, b) = (BodyId::new(), BodyId::new());
        let view = ExplodedView {
            steps: vec![
                ExplodeStep {
                    bodies: vec![a],
                    shift: [0.0, 0.0, 10.0],
                },
                ExplodeStep {
                    bodies: vec![a, b],
                    shift: [5.0, 0.0, 0.0],
                },
            ],
        };
        let start = [(a, BodyPlacement::default()), (b, BodyPlacement::default())];
        let at = |t: f32| view.placed_at(&start, t);
        assert_eq!(at(0.0)[0].1.offset(), Vec3::ZERO);
        assert_eq!(at(0.5)[0].1.offset(), Vec3::new(0.0, 0.0, 5.0));
        assert_eq!(at(1.5)[0].1.offset(), Vec3::new(2.5, 0.0, 10.0));
        assert_eq!(at(1.5)[1].1.offset(), Vec3::new(2.5, 0.0, 0.0));
        assert_eq!(at(2.0)[1].1.offset(), Vec3::new(5.0, 0.0, 0.0));
    }
}
