//! The hand: a gizmo's world delta from the press as increments of the freedoms
//! its handles stand for.
//!
//! Handle axes are the directions taken at the press (`start_state + delta`): while
//! dragging, the host may move earlier joints, and the handle axis does not float
//! with them. A ring needs no projection: it is built about the joint axis, so its
//! angle is exactly the joint increment.
use glam::DVec3;

/// Tolerance on a handle direction being of unit length.
const UNIT_TOLERANCE: f64 = 1e-9;

/// Sine of the angle between a pair's axes below which there is no planar handle.
/// The gizmo draws the plane as a parallelogram; for nearly collinear axes it
/// degenerates into a segment and the Gram solution into a division by `≈ 0`.
const COLLINEAR_SIN: f64 = 1e-6;

fn assert_unit(direction: DVec3, what: &str) {
    let error = (direction.length_squared() - 1.0).abs();
    assert!(
        direction.is_finite() && error < UNIT_TOLERANCE,
        "{what}: direction must be a finite unit vector, got {direction:?}"
    );
}

/// The increment of one slide along the unit `direction`: `δv = ⟨Δt, u⟩`.
pub fn slide_delta(direction: DVec3, delta: DVec3) -> f64 {
    assert_unit(direction, "slide_delta");
    delta.dot(direction)
}

/// A planar handle of two slides whose unit axes `u₁, u₂` are not collinear (and
/// not necessarily orthogonal).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlidePlane {
    first: DVec3,
    second: DVec3,
    /// `c = u₁·u₂`, the off-diagonal entry of the Gram matrix.
    cosine: f64,
}

impl SlidePlane {
    /// `None` when the axes are collinear (`det G = 1 − c² ≈ 0`): such a pair has no
    /// planar handle, a property of the pair rather than an error.
    pub fn new(first: DVec3, second: DVec3) -> Option<Self> {
        assert_unit(first, "SlidePlane");
        assert_unit(second, "SlidePlane");
        let cosine = first.dot(second);
        (1.0 - cosine * cosine >= COLLINEAR_SIN * COLLINEAR_SIN).then_some(Self {
            first,
            second,
            cosine,
        })
    }

    /// The increments `(δv₁, δv₂)` solving `G·[δv₁ δv₂]ᵀ = [⟨Δt,u₁⟩ ⟨Δt,u₂⟩]ᵀ`. The part
    /// of `Δt` off the plane is dropped: `δv₁·u₁ + δv₂·u₂` is the orthogonal
    /// projection of `Δt`.
    pub fn split(self, delta: DVec3) -> (f64, f64) {
        let along_first = delta.dot(self.first);
        let along_second = delta.dot(self.second);
        // G⁻¹ = [[1, −c], [−c, 1]] / (1 − c²).
        let determinant = 1.0 - self.cosine * self.cosine;
        (
            (along_first - self.cosine * along_second) / determinant,
            (along_second - self.cosine * along_first) / determinant,
        )
    }
}

#[cfg(test)]
#[path = "hand_tests.rs"]
mod tests;
