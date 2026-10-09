//! Orientation of a named view (Top, Front, Isometric, a view cube face) —
//! Here the choice is explicit and a pure function of its inputs:
//! - **world-up views** (`keep_horizon`): up is world_up; at a pole (looking
//!   along world_up) it is the fixed [`pole_up`] of that world_up, the same
//!   for Top and Bottom and independent of the current frame, so a view
//!   command always lands on the same picture and view cube labels stay
//!   readable;
//! - **free views** (`keep_horizon = false`): up is the axis of ±X/±Y/±Z,
//!   not parallel to the view direction, whose frame is the smallest rotation
//!   away from the current one. Ties go to the first axis in
//!   that order, so the result is deterministic.

use glam::{Quat, Vec3};

use crate::camera::CameraPose;

/// Below this `|direction × world_up|` (sine of the angle) a view direction
/// counts as along world_up, with a 0.01 rad tolerance.
const POLE_SINE: f32 = 0.01;

/// Rotation angles closer than this are a tie; the axis order decides.
const TIE_ANGLE: f32 = 1e-4;

const AXES: [Vec3; 6] = [
    Vec3::X,
    Vec3::NEG_X,
    Vec3::Y,
    Vec3::NEG_Y,
    Vec3::Z,
    Vec3::NEG_Z,
];

/// Screen-up of the views along `world_up` (Top and Bottom): `world_up × X`,
/// or `world_up × Y` when world_up is along X. For Z-up this is +Y (front at
/// the bottom of the Top view, the CAD convention); for Y-up it is −Z (the
/// front, seen from +Z, again at the bottom).
pub fn pole_up(world_up: Vec3) -> Vec3 {
    assert!(
        world_up.is_finite() && world_up.is_normalized(),
        "world_up must be a finite unit vector"
    );
    let up = world_up.cross(Vec3::X);
    if up.length_squared() > 1e-6 {
        up.normalize()
    } else {
        world_up.cross(Vec3::Y).normalize()
    }
}

/// Frame of the view seen from `direction` (pointing from the pivot towards
/// the camera, as in a view command), see the module comment for the up.
pub fn view_orientation(
    direction: Vec3,
    world_up: Vec3,
    keep_horizon: bool,
    current: Quat,
) -> Quat {
    assert!(
        direction.is_finite() && direction.length_squared() > 1e-12,
        "view direction must be finite and nonzero"
    );
    let direction = direction.normalize();
    let up = if keep_horizon {
        if direction.cross(world_up).length() < POLE_SINE {
            pole_up(world_up)
        } else {
            world_up
        }
    } else {
        nearest_up(direction, current)
    };
    frame(direction, up)
}

fn nearest_up(direction: Vec3, current: Quat) -> Vec3 {
    let mut best: Option<(Vec3, f32)> = None;
    for axis in AXES {
        if direction.cross(axis).length() < POLE_SINE {
            continue;
        }
        let angle = frame(direction, axis).angle_between(current);
        if best.is_none_or(|(_, best_angle)| angle < best_angle - TIE_ANGLE) {
            best = Some((axis, angle));
        }
    }
    // A unit direction is parallel to at most two of the six axes.
    best.expect("some axis is not parallel to the view direction")
        .0
}

fn frame(direction: Vec3, up: Vec3) -> Quat {
    CameraPose::looking_at(direction, Vec3::ZERO, up)
        .orientation
        .normalize()
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
