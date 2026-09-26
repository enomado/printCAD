//! What a bench hands the host to rebuild a body's solid.

use kernel_api::{ChainProbe, ProbeAnswer, ShapeProbe, SolidOp};

use crate::feature::{BodyId, FeatureId};

/// A body's build chain plus the feature responsible for each op (one
/// feature can emit several ops, e.g. a counterbored hole), and what the
/// features standing on the solid part way through ask of it.
#[derive(Debug, Default)]
pub struct BuildPlan {
    pub ops: Vec<SolidOp>,
    pub op_features: Vec<FeatureId>,
    /// Questions about the solid, each asked where its feature stands in
    /// the history (a datum on a face finding the face again).
    pub probes: Vec<PlanProbe>,
}

/// One question a feature asks of its body's solid during a build.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlanProbe {
    pub feature: FeatureId,
    pub probe: ChainProbe,
}

/// What a build found of a feature's references: the probes it asked, in
/// its own order, and their answers. Derived on each replica, never an op.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProbedReferences {
    pub probes: Vec<ShapeProbe>,
    pub answers: Vec<Result<ProbeAnswer, String>>,
}

/// The answers of a build, by the feature whose probes asked them.
pub fn sort_answers(
    asked: &[PlanProbe],
    answers: &[Result<ProbeAnswer, String>],
) -> Vec<(FeatureId, ProbedReferences)> {
    let mut sorted: Vec<(FeatureId, ProbedReferences)> = Vec::new();
    for (asked, answer) in asked.iter().zip(answers) {
        let at = match sorted.iter().position(|(f, _)| *f == asked.feature) {
            Some(at) => at,
            None => {
                sorted.push((asked.feature, ProbedReferences::default()));
                sorted.len() - 1
            }
        };
        let references = &mut sorted[at].1;
        references.probes.push(asked.probe.probe);
        references.answers.push(answer.clone());
    }
    sorted
}

/// A translation failure attributed to the feature that caused it.
#[derive(Debug, Clone)]
pub struct BuildError {
    pub feature: Option<FeatureId>,
    pub message: String,
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// One body whose solid a bench wants rebuilt now. An empty plan means the
/// body has no history left: its derived solid goes, an imported one stays.
#[derive(Debug)]
pub struct RebuildJob {
    pub body: BodyId,
    pub plan: Result<BuildPlan, BuildError>,
}
