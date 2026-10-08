//! Conversion between the view's preset-relative, f64 model state and the
//! shared camera's canonical local frame. A hold freezes `origin` at its press.

use axes::AxisSystem;
use glam::{DVec3, Mat3, Quat, Vec2};
use settings::{CameraSettings, OrbitYawAxis, ProjectionMode};
use viewport_camera::camera::{CameraPose, Projection};
use viewport_camera::controller::{CameraController, CameraControllerConfig, GestureKind};
use viewport_camera::navigation::{NavigationDrag, NavigationSettings, PanGrip, ScreenPoint};

use super::math::control_horizontal_vec;
use super::state::CadCameraState;

fn basis(axes: &AxisSystem) -> Quat {
    Quat::from_mat3(&Mat3::from_cols(
        control_horizontal_vec(axes),
        axes.vertical().vector(),
        axes.depth().vector(),
    ))
}

pub(super) fn camera(state: &CadCameraState, axes: &AxisSystem, origin: DVec3) -> CameraController {
    let pose = CameraPose {
        position: (state.eye - origin).as_vec3(),
        orientation: (state.orientation * basis(axes)).normalize(),
    };
    let viewport = Vec2::new(
        state.viewport_size.0.max(1) as f32,
        state.viewport_size.1.max(1) as f32,
    );
    let projection = match state.projection {
        ProjectionMode::Perspective => Projection::Perspective {
            vertical_fov: state.height_angle_rad as f32,
        },
        ProjectionMode::Orthographic => {
            viewport_camera::controller::orthographic(viewport, state.world_per_pixel() as f32)
        }
    };
    CameraController::new(
        pose,
        projection,
        viewport,
        pose.position + pose.forward() * state.focal_distance as f32,
    )
}

pub(super) fn apply(
    state: &mut CadCameraState,
    axes: &AxisSystem,
    origin: DVec3,
    camera: CameraController,
) {
    state.eye = origin + camera.pose().position.as_dvec3();
    state.orientation = (camera.pose().orientation * basis(axes).conjugate()).normalize();
    state.focal_distance = f64::from(-camera.pose().world_to_view(camera.pivot()).z);
    match camera.projection() {
        Projection::Perspective { vertical_fov } => {
            state.projection = ProjectionMode::Perspective;
            state.height_angle_rad = f64::from(vertical_fov);
        }
        Projection::Orthographic { bottom, top, .. } => {
            state.projection = ProjectionMode::Orthographic;
            state.ortho_height = f64::from(top - bottom);
        }
    }
    state.clip_dirty = true;
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Hold {
    origin: DVec3,
    drag: NavigationDrag,
    pointer_scale: f32,
    motion_scale: f32,
    focal_distance: f64,
    orientation: Quat,
    fov: f64,
    ortho_height: f64,
    kind: GestureKind,
}

impl Hold {
    pub(super) fn begin(
        state: &CadCameraState,
        axes: &AxisSystem,
        settings: &CameraSettings,
        start: Vec2,
        anchor: DVec3,
        kind: GestureKind,
        pixels_per_point: f32,
    ) -> Self {
        let origin = state.eye;
        let mut camera = camera(state, axes, origin);
        camera.set_viewport(camera.viewport() / pixels_per_point);
        let navigation = NavigationSettings {
            keep_horizon: settings.orbit_yaw_axis == OrbitYawAxis::WorldUp,
            orbit_speed: 1.0,
            pan_grip: PanGrip::PivotDepth,
            ..NavigationSettings::default()
        };
        let range = match state.projection {
            ProjectionMode::Perspective => [
                (f64::from(settings.min_focal_distance) / state.focal_distance).ln() as f32,
                (f64::from(settings.max_focal_distance) / state.focal_distance).ln() as f32,
            ],
            ProjectionMode::Orthographic => [
                (1e-9 / state.ortho_height).ln() as f32,
                (1e12 / state.ortho_height).ln() as f32,
            ],
        };
        let drag = navigation
            .begin_drag_with_up(
                camera,
                ScreenPoint(start / pixels_per_point),
                (anchor - origin).as_vec3(),
                kind,
                axes.vertical().vector(),
            )
            .with_zoom_range([range[0].min(0.0), range[1].max(0.0)]);
        Self {
            origin,
            drag,
            pointer_scale: pixels_per_point,
            motion_scale: if kind == GestureKind::Pan {
                settings.pan_sensitivity
            } else {
                settings.orbit_sensitivity
            },
            focal_distance: state.focal_distance,
            orientation: state.orientation,
            fov: state.height_angle_rad,
            ortho_height: state.ortho_height,
            kind,
        }
    }

    pub(super) fn sample(self, state: &mut CadCameraState, axes: &AxisSystem, pointer: Vec2) {
        let start = self.drag.start().0;
        let point = start + (pointer / self.pointer_scale - start) * self.motion_scale;
        apply(
            state,
            axes,
            self.origin,
            self.drag.sample(ScreenPoint(point)),
        );
        // A navigation hold does not change the host's lens or axial focal
        // distance. Preserve those f64 quantities without a round trip through f32.
        let zoom = f64::from(self.drag.zoom()).exp();
        state.focal_distance = if state.projection == ProjectionMode::Perspective {
            self.focal_distance * zoom
        } else {
            self.focal_distance
        };
        state.height_angle_rad = self.fov;
        state.ortho_height = self.ortho_height
            * if state.projection == ProjectionMode::Orthographic {
                zoom
            } else {
                1.0
            };
        if self.kind == GestureKind::Pan || point == start {
            state.orientation = self.orientation;
        }
    }

    pub(super) fn zoomed(mut self, lines: f32, settings: &CameraSettings) -> Self {
        let sign = if settings.invert_zoom { -1.0 } else { 1.0 };
        self.drag = self
            .drag
            .zoomed(lines * sign * settings.wheel_zoom_factor.ln());
        self
    }
}

pub(super) fn configured(
    state: &CadCameraState,
    axes: &AxisSystem,
    settings: &CameraSettings,
    origin: DVec3,
) -> CameraController {
    camera(state, axes, origin).with_config(CameraControllerConfig {
        world_up: axes.vertical().vector(),
        min_focus_distance: settings.min_focal_distance,
        ..CameraControllerConfig::default()
    })
}
