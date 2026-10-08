//! Camera pose, projection and screen rays without a windowing or rendering
//! backend.
//!
//! Anchor policy is not here: the host resolves the point a gesture holds
//! and hands it to [`crate::navigation`].

use glam::{Quat, Vec2, Vec3};

use crate::raycast::Ray;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraPose {
    pub position: Vec3,
    pub orientation: Quat,
}

impl CameraPose {
    /// `up` must not be parallel to the viewing direction; when it is, an
    /// arbitrary fallback axis picks the roll. View commands therefore choose
    /// their up with [`crate::view::view_orientation`] and never land here
    /// degenerate.
    pub fn looking_at(position: Vec3, target: Vec3, up: Vec3) -> Self {
        let forward = (target - position).normalize_or(Vec3::NEG_Z);
        let mut right = forward.cross(up);
        if right.length_squared() <= f32::EPSILON {
            let fallback_up = if forward.y.abs() < 0.9 {
                Vec3::Y
            } else {
                Vec3::X
            };
            right = forward.cross(fallback_up);
        }
        let right = right.normalize();
        let corrected_up = right.cross(forward).normalize();
        let orientation = Quat::from_mat3(&glam::Mat3::from_cols(right, corrected_up, -forward));
        Self {
            position,
            orientation,
        }
    }

    pub fn forward(self) -> Vec3 {
        self.orientation * Vec3::NEG_Z
    }

    pub fn view_ray(self) -> Ray {
        Ray::new(self.position, self.forward())
    }

    pub fn world_to_view(self, point: Vec3) -> Vec3 {
        self.orientation.conjugate() * (point - self.position)
    }

    pub fn translated(self, delta: Vec3) -> Self {
        Self {
            position: self.position + delta,
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Projection {
    Perspective {
        vertical_fov: f32,
    },
    Orthographic {
        left: f32,
        right: f32,
        bottom: f32,
        top: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraView {
    pub pose: CameraPose,
    pub projection: Projection,
    /// Logical pixels; screen coordinates use a top-left origin.
    pub viewport: Vec2,
}

impl CameraView {
    pub fn screen_ray(self, screen: Vec2) -> Option<Ray> {
        if self.viewport.x <= 0.0 || self.viewport.y <= 0.0 {
            return None;
        }
        let u = screen.x / self.viewport.x;
        let v = screen.y / self.viewport.y;
        match self.projection {
            Projection::Perspective { vertical_fov } => {
                let aspect = self.viewport.x / self.viewport.y;
                let half_height = (vertical_fov * 0.5).tan();
                let local = Vec3::new(
                    (2.0 * u - 1.0) * half_height * aspect,
                    (1.0 - 2.0 * v) * half_height,
                    -1.0,
                );
                Some(Ray::new(self.pose.position, self.pose.orientation * local))
            }
            Projection::Orthographic {
                left,
                right,
                bottom,
                top,
            } => {
                let local_origin =
                    Vec3::new(left + (right - left) * u, top - (top - bottom) * v, 0.0);
                Some(Ray::new(
                    self.pose.position + self.pose.orientation * local_origin,
                    self.pose.forward(),
                ))
            }
        }
    }

    pub fn world_to_screen(self, world: Vec3) -> Option<Vec2> {
        if self.viewport.x <= 0.0 || self.viewport.y <= 0.0 {
            return None;
        }
        let view = self.pose.world_to_view(world);
        let (u, v) = match self.projection {
            Projection::Perspective { vertical_fov } => {
                if view.z >= 0.0 {
                    return None;
                }
                let half_height = (vertical_fov * 0.5).tan();
                let aspect = self.viewport.x / self.viewport.y;
                let ndc_x = view.x / (-view.z * half_height * aspect);
                let ndc_y = view.y / (-view.z * half_height);
                (ndc_x.mul_add(0.5, 0.5), ndc_y.mul_add(-0.5, 0.5))
            }
            Projection::Orthographic {
                left,
                right,
                bottom,
                top,
            } => {
                if view.z >= 0.0 {
                    return None;
                }
                (
                    (view.x - left) / (right - left),
                    (top - view.y) / (top - bottom),
                )
            }
        };
        Some(Vec2::new(u * self.viewport.x, v * self.viewport.y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn perspective() -> CameraView {
        CameraView {
            pose: CameraPose::looking_at(Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, Vec3::Y),
            projection: Projection::Perspective {
                vertical_fov: std::f32::consts::FRAC_PI_2,
            },
            viewport: Vec2::new(800.0, 400.0),
        }
    }

    #[test]
    fn screen_center_is_the_camera_forward_ray() {
        let view = perspective();
        let ray = view.screen_ray(view.viewport * 0.5).unwrap();
        assert!(ray.direction.distance(view.pose.forward()) < 1e-6);
        assert_eq!(view.world_to_screen(Vec3::ZERO), Some(view.viewport * 0.5));
    }

    #[test]
    fn projection_and_unprojection_round_trip() {
        let view = perspective();
        for screen in [
            Vec2::new(100.0, 50.0),
            Vec2::new(400.0, 200.0),
            Vec2::new(700.0, 350.0),
        ] {
            let ray = view.screen_ray(screen).unwrap();
            let point = ray.point_at(20.0);
            assert!(view.world_to_screen(point).unwrap().distance(screen) < 1e-4);
        }
    }

    #[test]
    fn orthographic_rays_are_parallel_with_shifted_origins() {
        let mut view = perspective();
        view.projection = Projection::Orthographic {
            left: -4.0,
            right: 4.0,
            bottom: -2.0,
            top: 2.0,
        };
        let left = view.screen_ray(Vec2::new(0.0, 200.0)).unwrap();
        let right = view.screen_ray(Vec2::new(800.0, 200.0)).unwrap();
        assert_eq!(left.direction, right.direction);
        assert!(((right.origin - left.origin).length() - 8.0).abs() < 1e-6);
    }

    #[test]
    fn look_at_remains_orthonormal_at_a_pole() {
        let pose = CameraPose::looking_at(Vec3::Y * 10.0, Vec3::ZERO, Vec3::Y);
        let right = pose.orientation * Vec3::X;
        let up = pose.orientation * Vec3::Y;
        let forward = pose.forward();

        assert!(right.is_normalized());
        assert!(up.is_normalized());
        assert!(forward.is_normalized());
        assert!(right.dot(up).abs() < 1e-6);
        assert!(right.dot(forward).abs() < 1e-6);
        assert!(up.dot(forward).abs() < 1e-6);
    }
}
