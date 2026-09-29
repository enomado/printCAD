//! Collisions through a joint's motion: the drive held at steps across its
//! range, and at each the pairs the motion moves checked for material they
//! share beyond what they shared where the joint stands now (a pin in its
//! hole shares a sliver the whole way round).
//!
//! [`plan`] reads the document; [`SweepCheck::run`] needs only the kernel,
//! so it can run away from the window.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use core_document::{BodyId, Document, FeatureId};
use kernel_api::KernelQueries;

use crate::interference::{self, Check};

/// Material, in mm³, a step may add to a pair before it is a collision.
const CLASH_MM3: f64 = 1e-3;

/// A pair that shares material at a step of the motion.
#[derive(Debug, Clone, PartialEq)]
pub struct MotionClash {
    /// Where the drive stood: degrees for a hinge, mm for a slider.
    pub at: f32,
    pub a: BodyId,
    pub b: BodyId,
    /// What they share beyond what they share where the joint stands.
    pub volume_mm3: f64,
}

/// The checks a sweep asks: where the joint stands, and each step.
pub struct SweepCheck {
    start: Check,
    steps: Vec<(f32, Check)>,
}

/// The drive of `joint` at `count` steps from `low` to `high`, each a
/// check of the pairs with a body the motion moves. `None` for a joint
/// that is not a hinge or a slider, or one that moves nothing.
pub fn plan(
    document: &Document,
    joint: FeatureId,
    low: f32,
    high: f32,
    count: usize,
) -> Option<SweepCheck> {
    let count = count.max(2);
    let values: Vec<f32> = (0..count)
        .map(|i| low + (high - low) * i as f32 / (count - 1) as f32)
        .collect();
    let frames = crate::sweep_values(document, joint, &values);
    if frames.is_empty() {
        return None;
    }
    let start: HashMap<BodyId, _> = document
        .bodies()
        .iter()
        .map(|b| (b.id, b.placement))
        .collect();
    let moved: Vec<BodyId> = frames
        .iter()
        .flatten()
        .filter(|(b, p)| {
            start
                .get(b)
                .is_some_and(|s| !s.after(&p.inverse()).is_identity())
        })
        .map(|(b, _)| *b)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    if moved.is_empty() {
        return None;
    }
    let mut copy = document.clone();
    let steps = values
        .iter()
        .zip(&frames)
        .map(|(value, placements)| {
            for (body, placement) in placements {
                copy.set_body_placement(*body, *placement);
            }
            (*value, interference::plan(&copy, None).moving(&moved))
        })
        .collect();
    Some(SweepCheck {
        start: interference::plan(document, None).moving(&moved),
        steps,
    })
}

impl SweepCheck {
    /// Pairs asked about in all.
    pub fn pairs(&self) -> usize {
        self.start.pairs() + self.steps.iter().map(|(_, c)| c.pairs()).sum::<usize>()
    }

    /// Ask the kernel about every step, counting pairs off in `done`, until
    /// `stop`.
    pub fn run(
        &self,
        kernel: &dyn KernelQueries,
        done: &AtomicUsize,
        stop: &AtomicBool,
    ) -> Result<Vec<MotionClash>, String> {
        let pair = |a: BodyId, b: BodyId| if a < b { (a, b) } else { (b, a) };
        let base: HashMap<(BodyId, BodyId), f64> = self
            .start
            .run(kernel, done, stop)?
            .clashes
            .into_iter()
            .map(|c| (pair(c.a, c.b), c.volume_mm3))
            .collect();
        let mut out = Vec::new();
        for (at, check) in &self.steps {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            for clash in check.run(kernel, done, stop)?.clashes {
                let extra = clash.volume_mm3 - base.get(&pair(clash.a, clash.b)).unwrap_or(&0.0);
                if extra > CLASH_MM3 {
                    out.push(MotionClash {
                        at: *at,
                        a: clash.a,
                        b: clash.b,
                        volume_mm3: extra,
                    });
                }
            }
        }
        Ok(out)
    }
}
