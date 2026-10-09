//! Fit and focus commands. Fit uses the smaller of the vertical and horizontal
//! half-angles (orthographic: the smaller side). Focus derives its frame from
//! [`crate::view::view_orientation`] with the host's world-up axis.
//!
//! These functions return the target camera; a host animates to it with
//! [`crate::transition::CameraTransition::frame`].

use glam::{Vec2, Vec3};

use crate::camera::{CameraPose, Projection};
use crate::controller::{CameraController, orthographic};
use crate::view::view_orientation;

/// Free space around a fitted sphere, as a factor of its radius.
pub const FIT_MARGIN: f32 = 1.15;

/// What a fit has to show, in the camera's local frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoundingSphere {
    pub center: Vec3,
    /// Zero is a point: the fit centres it and keeps the scale.
    pub radius: f32,
}

impl BoundingSphere {
    /// The sphere through the corners of an axis-aligned box.
    pub fn from_aabb(min: Vec3, max: Vec3) -> Self {
        assert!(
            min.is_finite() && max.is_finite() && min.cmple(max).all(),
            "bounding box must be finite and ordered"
        );
        Self {
            center: min.midpoint(max),
            radius: (max - min).length() * 0.5,
        }
    }
}

/// Distance from the sphere's centre at which a perspective camera shows
/// the whole sphere with [`FIT_MARGIN`]: `r·margin / sin(α)`, `α` the smaller
/// of the vertical and horizontal half-angles. The sine accounts for tangency:
/// the cone of half-angle `α` touches the sphere at `r / sin α`.
pub fn fit_distance(vertical_fov: f32, viewport: Vec2, radius: f32) -> f32 {
    assert!(
        viewport.x > 0.0 && viewport.y > 0.0,
        "fit needs a positive viewport"
    );
    let half_vertical = vertical_fov * 0.5;
    let half_horizontal = (half_vertical.tan() * viewport.x / viewport.y).atan();
    radius * FIT_MARGIN / half_vertical.min(half_horizontal).sin()
}

/// The camera that shows `sphere` whole, centred, keeping orientation (and
/// roll) and projection kind. The pivot becomes the sphere's centre.
pub fn fit(camera: CameraController, sphere: BoundingSphere) -> CameraController {
    assert!(
        sphere.center.is_finite() && sphere.radius.is_finite() && sphere.radius >= 0.0,
        "fit sphere must be finite with a nonnegative radius"
    );
    let viewport = camera.viewport();
    let pose = camera.pose();
    let current_depth = -pose.world_to_view(camera.pivot()).z;
    let (projection, depth) = match camera.projection() {
        Projection::Perspective { vertical_fov } => {
            let depth = if sphere.radius == 0.0 {
                current_depth
            } else {
                fit_distance(vertical_fov, viewport, sphere.radius)
            };
            (camera.projection(), depth)
        }
        Projection::Orthographic { .. } => {
            let projection = if sphere.radius == 0.0 {
                camera.projection()
            } else {
                orthographic(
                    viewport,
                    2.0 * sphere.radius * FIT_MARGIN / viewport.min_element(),
                )
            };
            // Depth only has to keep the whole sphere in front of the camera.
            (
                projection,
                current_depth.max(2.0 * sphere.radius * FIT_MARGIN),
            )
        }
    };
    let mut fitted = camera;
    fitted.restore(
        CameraPose {
            position: sphere.center - pose.forward() * depth,
            orientation: pose.orientation,
        },
        projection,
        sphere.center,
    );
    fitted
}

/// Look at `target` from `distance` away. The side is `requested_from`
/// (pointing from the target towards the camera) or else the current one;
/// `fallback_from` covers a camera sitting on the target. The frame is the
/// world-up view of that side ([`view_orientation`]), so looking along
/// world_up is not degenerate.
pub fn focus_pose(
    current: CameraPose,
    target: Vec3,
    distance: f32,
    requested_from: Option<Vec3>,
    fallback_from: Vec3,
    world_up: Vec3,
) -> CameraPose {
    let direction = requested_from
        .unwrap_or(current.position - target)
        .normalize_or(fallback_from.normalize());
    CameraPose {
        position: target + direction * distance,
        orientation: view_orientation(direction, world_up, true, current.orientation),
    }
}

#[cfg(test)]
#[path = "fit_tests.rs"]
mod tests;
