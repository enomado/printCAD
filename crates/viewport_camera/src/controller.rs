//! Renderer-independent camera state: pose, projection, viewport and pivot.
//!
//! The controller holds no gesture, hold or inertia state. A
//! drag is an immutable press snapshot ([`crate::navigation::NavigationDrag`]) that
//! samples a fresh controller from its copy; view and projection changes are
//! [`crate::transition::CameraTransition`]s sampled from their source. So two
//! frontends fed the same commands produce the same camera view, and nothing
//! survives a release that could move the camera on its own.
//!
//! The numerical operations (`orbit_free`, `pan`, `zoom`, `set_projection`,
//! `orient_about_pivot`, `fitted`) are instantaneous and act on the camera's
//! local `f32` frame.

use glam::{Quat, Vec2, Vec3};

use crate::camera::{CameraPose, CameraView, Projection};

/// What a held pointer does. Wheel/pinch zoom is not a drag: it is an
/// instantaneous [`CameraController::zoom`] step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GestureKind {
    Orbit,
    Pan,
}

/// Projection requested by a command, without the extents an orthographic
/// projection derives from the scale it preserves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ProjectionKind {
    Perspective { vertical_fov: f32 },
    Orthographic,
}

impl ProjectionKind {
    pub fn of(projection: Projection) -> Self {
        match projection {
            Projection::Perspective { vertical_fov } => Self::Perspective { vertical_fov },
            Projection::Orthographic { .. } => Self::Orthographic,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraControllerConfig {
    /// Unit world-up axis for canonical views and world-up orbit.
    pub world_up: Vec3,
    /// Radians per logical pixel: `x` for horizontal drags (yaw), `y` for
    /// vertical ones (pitch) — upstream `Sensitivity.orbit: Vec2` (О2).
    pub orbit_sensitivity: Vec2,
    /// Exponential zoom coefficient per wheel unit/logical pixel.
    pub zoom_sensitivity: f32,
    pub min_focus_distance: f32,
}

impl Default for CameraControllerConfig {
    fn default() -> Self {
        Self {
            world_up: Vec3::Y,
            orbit_sensitivity: Vec2::splat(0.005),
            zoom_sensitivity: 0.12,
            min_focus_distance: 0.01,
        }
    }
}

/// Camera state. Coordinates passed to this type are logical pixels and local
/// `f32` model units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraController {
    pose: CameraPose,
    projection: Projection,
    viewport: Vec2,
    pivot: Vec3,
    config: CameraControllerConfig,
}

impl CameraController {
    pub fn new(pose: CameraPose, projection: Projection, viewport: Vec2, pivot: Vec3) -> Self {
        Self {
            pose,
            projection,
            viewport,
            pivot,
            config: CameraControllerConfig::default(),
        }
    }

    pub fn with_config(mut self, config: CameraControllerConfig) -> Self {
        assert!(
            config.world_up.is_finite() && config.world_up.is_normalized(),
            "camera world_up must be a finite unit vector"
        );
        self.config = config;
        self
    }

    pub fn config(&self) -> CameraControllerConfig {
        self.config
    }

    pub fn pose(&self) -> CameraPose {
        self.pose
    }

    pub fn projection(&self) -> Projection {
        self.projection
    }

    pub fn viewport(&self) -> Vec2 {
        self.viewport
    }

    pub fn pivot(&self) -> Vec3 {
        self.pivot
    }

    pub fn view(&self) -> CameraView {
        CameraView {
            pose: self.pose,
            projection: self.projection,
            viewport: self.viewport,
        }
    }

    pub fn set_viewport(&mut self, viewport: Vec2) {
        let old_aspect = aspect(self.viewport);
        self.viewport = viewport;
        let new_aspect = aspect(viewport);
        if let Projection::Orthographic {
            left,
            right,
            bottom,
            top,
        } = &mut self.projection
        {
            let center = Vec2::new(left.midpoint(*right), bottom.midpoint(*top));
            let half_height = (*top - *bottom).abs() * 0.5;
            let half_width = if old_aspect > 0.0 && new_aspect > 0.0 {
                half_height * new_aspect
            } else {
                (*right - *left).abs() * 0.5
            };
            *left = center.x - half_width;
            *right = center.x + half_width;
            *bottom = center.y - half_height;
            *top = center.y + half_height;
        }
    }

    /// Replace pose, projection and pivot as one atomic restore operation.
    pub fn restore(&mut self, pose: CameraPose, projection: Projection, pivot: Vec3) {
        self.pose = pose;
        self.projection = projection;
        self.pivot = pivot;
    }

    pub fn set_pose(&mut self, pose: CameraPose, pivot: Vec3) {
        self.pose = pose;
        self.pivot = pivot;
    }

    pub fn translate(&mut self, delta: Vec3) {
        self.pose = self.pose.translated(delta);
        self.pivot += delta;
    }

    /// Model units per logical pixel at `anchor`: in perspective measured at
    /// the anchor's **axial** depth (along the view direction), not at its
    /// radial distance, because that is what a screen pixel spans there.
    pub fn scale_at(&self, anchor: Vec3) -> f32 {
        assert!(
            self.viewport.y > 0.0,
            "camera scale needs a positive viewport"
        );
        match self.projection {
            Projection::Perspective { vertical_fov } => {
                2.0 * (-self.pose.world_to_view(anchor).z) * (vertical_fov * 0.5).tan()
                    / self.viewport.y
            }
            Projection::Orthographic { bottom, top, .. } => (top - bottom).abs() / self.viewport.y,
        }
    }

    /// Wheel/pinch zoom: one instantaneous exponential step around `anchor`,
    /// which keeps its pixel. There is no tail — a wheel event never seeds
    /// motion. An anchor behind the camera is refused.
    pub fn zoom(&mut self, delta: f32, anchor: Vec3) {
        if !anchor.is_finite() || self.pose.world_to_view(anchor).z >= 0.0 {
            return;
        }
        self.zoom_by((-delta * self.config.zoom_sensitivity).exp(), anchor);
    }

    /// The zoom step itself: `factor` is the new scale over the old one at
    /// `anchor` (< 1 zooms in), which keeps its pixel. Perspective stops at
    /// `min_focus_distance`. An anchor behind the camera is refused.
    pub fn zoom_by(&mut self, factor: f32, anchor: Vec3) {
        assert!(
            factor.is_finite() && factor > 0.0,
            "zoom factor must be finite and positive"
        );
        if !anchor.is_finite() || self.pose.world_to_view(anchor).z >= 0.0 {
            return;
        }
        match &mut self.projection {
            Projection::Perspective { .. } => {
                let offset = self.pose.position - anchor;
                let old_distance = offset.length();
                if old_distance <= f32::EPSILON {
                    return;
                }
                let distance = (old_distance * factor).max(self.config.min_focus_distance);
                self.pose.position = anchor + offset * (distance / old_distance);
            }
            Projection::Orthographic {
                left,
                right,
                bottom,
                top,
            } => {
                let local = self.pose.world_to_view(anchor);
                let right_axis = self.pose.orientation * Vec3::X;
                let up_axis = self.pose.orientation * Vec3::Y;
                self.pose.position += right_axis * (local.x * (1.0 - factor));
                self.pose.position += up_axis * (local.y * (1.0 - factor));
                *left *= factor;
                *right *= factor;
                *bottom *= factor;
                *top *= factor;
            }
        }
        self.pivot = anchor;
    }

    /// Free orbit (`keep_horizon = false`): rotate both position and frame
    /// around the camera's own screen axes, so the saved roll is kept and an
    /// exact top view is not singular. World-up orbit is
    /// [`crate::navigation::NavigationDrag`]'s ground branch; there is no third
    /// implementation.
    pub(crate) fn orbit_free(&mut self, anchor: Vec3, delta: Vec2) {
        // A press with no movement must not rebuild (and roll) the saved pose.
        if delta == Vec2::ZERO {
            return;
        }
        let offset = self.pose.position - anchor;
        if offset.length_squared() <= f32::EPSILON {
            return;
        }
        let up = self.pose.orientation * Vec3::Y;
        let yaw = Quat::from_axis_angle(up, -delta.x * self.config.orbit_sensitivity.x);
        let right = (yaw * (self.pose.orientation * Vec3::X)).normalize_or(Vec3::X);
        let pitch = Quat::from_axis_angle(right, -delta.y * self.config.orbit_sensitivity.y);
        self.pose.position = anchor + pitch * yaw * offset;
        self.pose.orientation = (pitch * yaw * self.pose.orientation).normalize();
        self.pivot = anchor;
    }

    /// Screen-parallel translation measured at `anchor`'s depth, so the
    /// anchor follows the pointer exactly. The pivot ends at
    /// `anchor + translation` — the same spot on screen.
    pub(crate) fn pan(&mut self, anchor: Vec3, delta: Vec2) {
        let scale = self.pan_scale(anchor);
        let right = self.pose.orientation * Vec3::X;
        let up = self.pose.orientation * Vec3::Y;
        let translation = right * (-delta.x * scale) + up * (delta.y * scale);
        self.pose = self.pose.translated(translation);
        self.pivot = anchor + translation;
    }

    /// Change projection or FOV (Ф1, dolly zoom) keeping the scale at the
    /// pivot's **axial** depth and the pivot's off-centre screen position.
    ///
    /// The pivot's view-space `x, y` stay; only the depth changes to the one
    /// at which the new FOV spans the old scale. Orthographic keeps the
    /// current axial depth and takes the scale as its extents. Radial distance
    /// would change the scale when the pivot is off the lens axis.
    pub fn set_projection(&mut self, kind: ProjectionKind) {
        self.set_projection_at(kind, self.pivot);
    }

    /// Dolly about an explicit pointed anchor, preserving its pixel and scale
    /// at axial depth. The anchor becomes the next orbit pivot.
    pub fn set_projection_at(&mut self, kind: ProjectionKind, anchor: Vec3) {
        assert!(
            anchor.is_finite() && self.pose.world_to_view(anchor).z < 0.0,
            "projection anchor must be finite and in front of the camera"
        );
        let scale = self.scale_at(anchor);
        let local = self.pose.world_to_view(anchor);
        let (projection, depth) = match kind {
            ProjectionKind::Orthographic => (orthographic(self.viewport, scale), -local.z),
            ProjectionKind::Perspective { vertical_fov } => {
                assert!(
                    vertical_fov.is_finite()
                        && vertical_fov > 0.0
                        && vertical_fov < std::f32::consts::PI,
                    "perspective FOV must be finite and in (0, π)"
                );
                (
                    Projection::Perspective { vertical_fov },
                    scale * self.viewport.y / (2.0 * (vertical_fov * 0.5).tan()),
                )
            }
        };
        let orientation = self.pose.orientation;
        self.pose = CameraPose {
            position: anchor - orientation * Vec3::new(local.x, local.y, -depth),
            orientation,
        };
        self.projection = projection;
        self.pivot = anchor;
    }

    /// Turn the frame to `orientation` around the pivot, keeping the pivot's
    /// view-space coordinates (its pixel and depth) and the projection. This
    /// is what a view command does at every sample.
    pub fn orient_about_pivot(&mut self, orientation: Quat) {
        let local = self.pose.world_to_view(self.pivot);
        self.pose = CameraPose {
            position: self.pivot - orientation * local,
            orientation,
        };
    }

    fn pan_scale(&self, anchor: Vec3) -> f32 {
        if self.viewport.y <= 0.0 {
            return 0.0;
        }
        match self.projection {
            Projection::Perspective { vertical_fov } => {
                let depth = -self.pose.world_to_view(anchor).z;
                2.0 * depth.max(self.config.min_focus_distance) * (vertical_fov * 0.5).tan()
                    / self.viewport.y
            }
            Projection::Orthographic { bottom, top, .. } => (top - bottom).abs() / self.viewport.y,
        }
    }
}

/// Centred orthographic extents for `scale` model units per logical pixel.
pub fn orthographic(viewport: Vec2, scale: f32) -> Projection {
    let half_width = viewport.x * scale * 0.5;
    let half_height = viewport.y * scale * 0.5;
    Projection::Orthographic {
        left: -half_width,
        right: half_width,
        bottom: -half_height,
        top: half_height,
    }
}

fn aspect(viewport: Vec2) -> f32 {
    if viewport.y > 0.0 {
        viewport.x / viewport.y
    } else {
        0.0
    }
}

#[cfg(test)]
#[path = "controller_tests.rs"]
mod tests;
