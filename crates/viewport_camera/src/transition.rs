//! Animated camera transitions: view, fit and projection/FOV commands.
//!
//! Every sample is computed from the immutable source camera and the elapsed
//! time, never by accumulating per-frame deltas: the end state does not depend
//! on how the frames split the duration, and stopping never rolls back. A
//! duration of zero is an instant command. Easing is smoothstep.
//!
//! The host owns the clock and interruption, and anything outside the camera
//! (including a floating origin: samples stay in the source's local frame).

use std::time::Duration;

use glam::{Quat, Vec2, Vec3};

use crate::camera::{CameraPose, Projection};
use crate::controller::{CameraController, ProjectionKind, orthographic};

/// Perspective FOV that stands in for orthographic during a projection
/// animation. True ortho is only the endpoint, never a zero-FOV matrix: the
/// dolly is limited here to keep the local `f32` camera finite.
pub const ORTHO_STAND_IN_FOV: f32 = 0.01;

#[derive(Clone, Copy, Debug)]
pub struct CameraTransition {
    source: CameraController,
    target: TransitionTarget,
    elapsed: Duration,
    duration: Duration,
}

#[derive(Clone, Copy, Debug)]
enum TransitionTarget {
    /// Dolly zoom through FOVs, keeping the scale at the pivot (Ф1).
    Projection {
        from_fov: f32,
        target: ProjectionKind,
    },
    /// Pose, pivot and scale towards a camera of the same projection kind.
    Frame(CameraController),
}

impl CameraTransition {
    pub fn projection(
        source: CameraController,
        target: ProjectionKind,
        duration: Duration,
    ) -> Self {
        let from_fov = match source.projection() {
            Projection::Perspective { vertical_fov } => vertical_fov,
            Projection::Orthographic { .. } => ORTHO_STAND_IN_FOV,
        };
        Self {
            source,
            target: TransitionTarget::Projection { from_fov, target },
            elapsed: Duration::ZERO,
            duration,
        }
    }

    /// Towards `target`, which must have the source's viewport and projection
    /// kind (a fit or a view command changes neither).
    pub fn frame(source: CameraController, target: CameraController, duration: Duration) -> Self {
        assert_eq!(
            source.viewport(),
            target.viewport(),
            "a frame transition keeps the viewport"
        );
        assert!(
            match (source.projection(), target.projection()) {
                (Projection::Perspective { .. }, Projection::Perspective { .. }) =>
                    source.projection() == target.projection(),
                (Projection::Orthographic { .. }, Projection::Orthographic { .. }) => true,
                _ => false,
            },
            "a frame transition keeps the projection kind and the FOV"
        );
        Self {
            source,
            target: TransitionTarget::Frame(target),
            elapsed: Duration::ZERO,
            duration,
        }
    }

    /// Turn to `orientation` around the source's pivot: the pivot keeps its
    /// pixel and depth through the whole animation.
    pub fn view(source: CameraController, orientation: Quat, duration: Duration) -> Self {
        let mut target = source;
        target.orient_about_pivot(orientation);
        Self::frame(source, target, duration)
    }

    pub fn source(&self) -> CameraController {
        self.source
    }

    pub fn is_projection(&self) -> bool {
        matches!(self.target, TransitionTarget::Projection { .. })
    }

    pub fn is_done(&self) -> bool {
        self.elapsed >= self.duration
    }

    pub fn advance(&mut self, delta: Duration) {
        self.elapsed = self.elapsed.saturating_add(delta);
    }

    pub fn sample(&self) -> CameraController {
        let done = self.is_done();
        let t = if done {
            1.0
        } else {
            (self.elapsed.as_secs_f64() / self.duration.as_secs_f64()) as f32
        };
        let ease = t * t * (3.0 - 2.0 * t);
        let mut camera = self.source;
        match self.target {
            TransitionTarget::Projection { target, .. } if done => camera.set_projection(target),
            TransitionTarget::Projection { target, from_fov } => {
                if !self.elapsed.is_zero() {
                    let target_fov = match target {
                        ProjectionKind::Perspective { vertical_fov } => vertical_fov,
                        ProjectionKind::Orthographic => ORTHO_STAND_IN_FOV,
                    };
                    camera.set_projection(ProjectionKind::Perspective {
                        vertical_fov: from_fov + (target_fov - from_fov) * ease,
                    });
                }
            }
            TransitionTarget::Frame(target) if done => camera = target,
            TransitionTarget::Frame(target) => {
                camera = interpolate_frame(self.source, target, ease)
            }
        }
        camera
    }
}

/// Orientation slerps; the pivot, its off-centre pixel and the scale at it
/// (log-linearly, so a zoom feels even) move to the target's. Unchanged
/// quantities are taken as they are, so a pure view turn keeps the pivot's
/// view coordinates exactly.
fn interpolate_frame(
    source: CameraController,
    target: CameraController,
    ease: f32,
) -> CameraController {
    let orientation = source
        .pose()
        .orientation
        .slerp(target.pose().orientation, ease)
        .normalize();
    let pivot = if source.pivot() == target.pivot() {
        source.pivot()
    } else {
        source.pivot().lerp(target.pivot(), ease)
    };
    let source_local = source.pose().world_to_view(source.pivot());
    let target_local = target.pose().world_to_view(target.pivot());
    let (from, to) = (
        Vec2::new(source_local.x, source_local.y),
        Vec2::new(target_local.x, target_local.y),
    );
    let offset = if from == to {
        from
    } else {
        from.lerp(to, ease)
    };
    // `z` is the pivot's view-space z (negative in front of the camera).
    let (projection, z) = match source.projection() {
        // Same FOV at both ends, so the scale is proportional to the axial
        // depth: a log-linear depth is a log-linear scale.
        Projection::Perspective { .. } => (
            source.projection(),
            -log_lerp(-source_local.z, -target_local.z, ease),
        ),
        // Orthographic depth does not change the picture; only keep it in front.
        Projection::Orthographic { .. } => {
            let (from, to) = (
                source.scale_at(source.pivot()),
                target.scale_at(target.pivot()),
            );
            let projection = if from == to {
                source.projection()
            } else {
                orthographic(source.viewport(), log_lerp(from, to, ease))
            };
            (
                projection,
                source_local.z + (target_local.z - source_local.z) * ease,
            )
        }
    };
    let mut camera = source;
    camera.restore(
        CameraPose {
            position: pivot - orientation * Vec3::new(offset.x, offset.y, z),
            orientation,
        },
        projection,
        pivot,
    );
    camera
}

/// `exp(lerp(ln a, ln b, t))`, exactly `a` when `a == b`.
fn log_lerp(a: f32, b: f32, t: f32) -> f32 {
    if a == b {
        a
    } else {
        (a.ln() + (b.ln() - a.ln()) * t).exp()
    }
}

#[cfg(test)]
#[path = "transition_tests.rs"]
mod tests;
