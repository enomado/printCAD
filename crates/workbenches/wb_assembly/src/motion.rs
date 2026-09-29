//! Motion over time: several joints driven at once, each by a formula of
//! the time `t` in seconds, from a start to an end in steps. The frames
//! are solved on a copy of the document; the document itself is not moved.

use core_document::{
    Document, DocumentResult, FeatureError, FeatureId, WorkbenchFeature, WorkbenchId,
};
use serde::{Deserialize, Serialize};

use crate::joint::{JointFeature, JointKind};

/// The feature kind motion studies are stored as.
pub const MOTION_KIND: &str = "wb.assembly.motion";

/// A joint driven by a formula of `t`: a hinge's angle in degrees, a
/// slider's position in millimetres.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimedDrive {
    pub joint: FeatureId,
    pub formula: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MotionStudy {
    pub start: f32,
    pub end: f32,
    pub step: f32,
    pub drives: Vec<TimedDrive>,
}

impl Default for MotionStudy {
    fn default() -> Self {
        Self {
            start: 0.0,
            end: 2.0,
            step: 0.05,
            drives: Vec::new(),
        }
    }
}

impl WorkbenchFeature for MotionStudy {
    fn workbench_id() -> WorkbenchId {
        WorkbenchId::from(MOTION_KIND)
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
        "Motion"
    }
}

/// `formula` with every bare `t` read as `t` seconds.
fn with_time(formula: &str, t: f64) -> String {
    let chars: Vec<char> = formula.chars().collect();
    let word = |c: char| c.is_alphanumeric() || c == '_' || c == '`' || c == '.';
    let mut out = String::new();
    for (i, c) in chars.iter().enumerate() {
        let before = i.checked_sub(1).map(|j| chars[j]);
        let after = chars.get(i + 1).copied();
        if *c == 't' && !before.is_some_and(word) && !after.is_some_and(word) {
            out.push_str(&format!("({t})"));
        } else {
            out.push(*c);
        }
    }
    out
}

/// The value `formula` gives at `t` seconds: degrees for an angle,
/// millimetres for a length, else the number.
pub fn value_at(formula: &str, t: f64) -> Result<f64, String> {
    let ctx = core_document::expr::Context {
        length_unit: core_document::units::Unit::Mm,
        resolve: &core_document::expr::NoReferences,
    };
    core_document::expr::evaluate(&with_time(formula, t), &ctx)
        .map(|q| q.value)
        .map_err(|e| format!("{formula}: {e}"))
}

impl MotionStudy {
    /// The times of its frames, start to end.
    pub fn times(&self) -> Vec<f32> {
        let step = self.step.max(1e-3);
        let count = (((self.end - self.start) / step).floor().max(0.0) as usize).min(10_000);
        (0..=count).map(|i| self.start + step * i as f32).collect()
    }

    /// Every body's placement at each frame, each drive held where its
    /// formula says, solved on a copy of `document`.
    pub fn frames(&self, document: &Document) -> Result<crate::Frames, String> {
        let mut copy = document.clone();
        let mut frames = Vec::new();
        for t in self.times() {
            for drive in &self.drives {
                let value = value_at(&drive.formula, f64::from(t))? as f32;
                let mut feature = copy
                    .get_feature_data(drive.joint)
                    .and_then(|d| JointFeature::from_json(d).ok())
                    .ok_or("a driven joint is gone")?;
                match &mut feature.kind {
                    JointKind::Hinge { drive: d, .. } | JointKind::Slider { drive: d, .. } => {
                        d.to = Some(value);
                    }
                    _ => return Err("only hinges and sliders are driven".into()),
                }
                copy.update_feature_data(drive.joint, feature.to_json())
                    .map_err(|e| e.to_string())?;
            }
            if let Ok(moves) = crate::solve(&copy) {
                crate::place_bodies(&mut copy, &moves);
            }
            frames.push((
                t,
                copy.bodies().iter().map(|b| (b.id, b.placement)).collect(),
            ));
        }
        Ok(frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_formula_reads_the_time() {
        assert_eq!(value_at("90 * t", 0.5).unwrap(), 45.0);
        assert!((value_at("30 * sin(t * 90°)", 1.0).unwrap() - 30.0).abs() < 1e-9);
        assert_eq!(with_time("tan(t) + t2 + a.t", 2.0), "tan((2)) + t2 + a.t");
        let study = MotionStudy {
            start: 0.0,
            end: 1.0,
            step: 0.25,
            drives: Vec::new(),
        };
        assert_eq!(study.times(), [0.0, 0.25, 0.5, 0.75, 1.0]);
    }
}
