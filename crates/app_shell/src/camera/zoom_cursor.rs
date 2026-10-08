//! Zoom‑to‑cursor: keep the world point under the cursor stable after zoom (`docs/CAMERA.md`, Navigation).

use glam::{Vec2, Vec3, Vec4};
use settings::{CameraSettings, ProjectionMode};

use crate::camera::state::CadCameraState;

pub fn viewport_ray(
    state: &CadCameraState,
    axes: &axes::AxisSystem,
    viewport_xy: Vec2,
) -> Option<(Vec3, Vec3)> {
    let (w, h) = state.viewport_size;
    if w == 0 || h == 0 {
        return None;
    }
    let vw = w as f32;
    let vh = h as f32;

    let ndc_x = (viewport_xy.x / vw) * 2.0 - 1.0;
    let ndc_y = (viewport_xy.y / vh) * 2.0 - 1.0;

    let vp = state.view_projection(axes);
    let inv = vp.inverse();

    let near_clip = inv * Vec4::new(ndc_x, ndc_y, 0.0, 1.0);
    let far_clip = inv * Vec4::new(ndc_x, ndc_y, 1.0, 1.0);
    if near_clip.w.abs() < 1e-20 || far_clip.w.abs() < 1e-20 {
        return None;
    }
    let near_w = near_clip.truncate() / near_clip.w;
    let far_w = far_clip.truncate() / far_clip.w;
    let dir = (far_w - near_w).normalize();
    if dir.length_squared() < 1e-24 {
        return None;
    }
    // The origin lies on this pixel's ray. The eye would do for perspective rays, but orthographic
    // rays run parallel to forward, offset sideways, and from the eye the cursor correction would
    // collapse to zooming about the orbit centre.
    Some((near_w, dir))
}

#[cfg(test)]
pub fn intersect_focal_plane_world(
    state: &CadCameraState,
    axes: &axes::AxisSystem,
    viewport_xy: Vec2,
) -> Option<Vec3> {
    let focal = state.focal_point_vec3(axes);
    let plane_n = state.forward_world(axes).normalize();
    let (origin, dir) = viewport_ray(state, axes, viewport_xy)?;
    intersect_ray_plane(origin, dir, focal, plane_n)
}

pub fn intersect_ray_plane(
    origin: Vec3,
    dir: Vec3,
    plane_origin: Vec3,
    plane_normal: Vec3,
) -> Option<Vec3> {
    let n = plane_normal.normalize();
    let denom = dir.dot(n);
    if denom.abs() < 1e-8 {
        return None;
    }
    let t = (plane_origin - origin).dot(n) / denom;
    if t < 0.0 {
        return None;
    }
    Some(origin + dir * t)
}

/// Apply exponential zoom (`factor^wheel_lines`).
pub fn apply_zoom_wheels(
    state: &mut CadCameraState,
    axes: &axes::AxisSystem,
    viewport_cursor: Option<Vec2>,
    wheel_lines: f32,
    settings: &CameraSettings,
) {
    if wheel_lines == 0.0 {
        return;
    }
    let mut lines = wheel_lines;
    if settings.invert_zoom {
        lines = -lines;
    }
    let cursor = if settings.zoom_to_cursor {
        viewport_cursor
    } else {
        None
    };

    let log_factor = lines * settings.wheel_zoom_factor.ln();
    let log_factor = match state.projection {
        ProjectionMode::Perspective => log_factor.clamp(
            (f64::from(settings.min_focal_distance) / state.focal_distance).ln() as f32,
            (f64::from(settings.max_focal_distance) / state.focal_distance).ln() as f32,
        ),
        ProjectionMode::Orthographic => log_factor.clamp(
            (1e-9 / state.ortho_height).ln() as f32,
            (1e12 / state.ortho_height).ln() as f32,
        ),
    };
    let origin = state.eye;
    let mut camera = super::core::configured(state, axes, settings, origin);
    // At the focal plane a cursor anchor has the same axial depth as the
    // centre; the shared zoom therefore obeys the host's focal limits.
    let anchor = cursor
        .and_then(|point| camera.view().screen_ray(point))
        .and_then(|ray| {
            intersect_ray_plane(
                ray.origin,
                ray.direction,
                camera.pivot(),
                camera.pose().forward(),
            )
        })
        .unwrap_or(camera.pivot());
    camera.zoom_by(log_factor.exp(), anchor);
    super::core::apply(state, axes, origin, camera);
}
