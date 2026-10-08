use glam::{Vec2, Vec3};

use super::*;

fn controller() -> CameraController {
    CameraController::new(
        CameraPose::looking_at(Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, Vec3::Y),
        Projection::Perspective {
            vertical_fov: std::f32::consts::FRAC_PI_2,
        },
        Vec2::new(800.0, 400.0),
        Vec3::ZERO,
    )
}

/// Free orbit around a centred anchor keeps it centred and at the same
/// distance. World-up orbit is the ground branch of `NavigationDrag`.
#[test]
fn free_orbit_keeps_anchor_and_distance() {
    let mut camera = controller();
    camera.orbit_free(Vec3::ZERO, Vec2::new(80.0, -30.0));

    assert!((camera.pose().position.distance(Vec3::ZERO) - 10.0).abs() < 1e-5);
    assert!(
        camera
            .pose()
            .forward()
            .distance(-camera.pose().position.normalize())
            < 1e-5
    );
    assert_eq!(camera.pivot(), Vec3::ZERO);
}

#[test]
fn zoom_keeps_an_off_center_anchor_under_the_same_pixel() {
    let mut camera = controller();
    let anchor = Vec3::new(2.0, 1.0, 0.0);
    let before = camera.view().world_to_screen(anchor).unwrap();
    camera.zoom(1.0, anchor);
    let after = camera.view().world_to_screen(anchor).unwrap();
    assert!(before.distance(after) < 1e-4, "{before:?} != {after:?}");

    camera.set_projection(ProjectionKind::Orthographic);
    let before_ortho = camera.view().world_to_screen(anchor).unwrap();
    camera.zoom(1.0, anchor);
    let after_ortho = camera.view().world_to_screen(anchor).unwrap();
    assert!(before_ortho.distance(after_ortho) < 1e-4);
}

#[test]
fn projection_round_trip_preserves_the_frame() {
    let mut camera = controller();
    let points = [
        Vec3::ZERO,
        Vec3::new(2.0, 1.0, 0.0),
        Vec3::new(-2.0, -1.0, 0.0),
    ];
    let before = points.map(|point| camera.view().world_to_screen(point).unwrap());
    camera.set_projection(ProjectionKind::Orthographic);
    let ortho = points.map(|point| camera.view().world_to_screen(point).unwrap());
    for (a, b) in before.iter().zip(ortho) {
        assert!(a.distance(b) < 1e-4);
    }
    camera.zoom(0.7, Vec3::ZERO);
    let zoomed = points.map(|point| camera.view().world_to_screen(point).unwrap());
    camera.set_projection(ProjectionKind::Perspective {
        vertical_fov: std::f32::consts::FRAC_PI_2,
    });
    let restored = points.map(|point| camera.view().world_to_screen(point).unwrap());
    for (a, b) in zoomed.iter().zip(restored) {
        assert!(a.distance(b) < 1e-4);
    }
}

/// A projection change keeps an off-axis pivot's scale.
///
/// With the pivot off the axis its radial distance is longer than the axial
/// depth a pixel actually spans.
/// Mutation oracle: use `position.distance(pivot)` as the depth — the scale
/// at the pivot changes by `radial / axial`.
#[test]
fn projection_change_keeps_scale_and_pixel_of_an_off_axis_pivot() {
    let mut camera = controller();
    let pivot = Vec3::new(6.0, -3.0, -2.0);
    camera.set_pose(camera.pose(), pivot);
    let scale = camera.scale_at(pivot);
    let pixel = camera.view().world_to_screen(pivot).unwrap();
    for kind in [
        ProjectionKind::Orthographic,
        ProjectionKind::Perspective { vertical_fov: 0.3 },
        ProjectionKind::Perspective { vertical_fov: 2.4 },
    ] {
        camera.set_projection(kind);
        assert_eq!(ProjectionKind::of(camera.projection()), kind);
        assert!(
            (camera.scale_at(pivot) / scale - 1.0).abs() < 1e-5,
            "{kind:?}: scale {} != {scale}",
            camera.scale_at(pivot)
        );
        assert!(
            camera
                .view()
                .world_to_screen(pivot)
                .unwrap()
                .distance(pixel)
                < 1e-3
        );
    }
}

/// One wheel event scales the model units per pixel at the anchor by
/// `exp(-delta·k)` for any FOV: the exponential distance step preserves
/// the same zoom ratio across projections.
#[test]
fn zoom_step_is_independent_of_the_fov() {
    for fov in [0.2_f32, 0.8, 1.5] {
        let mut camera = controller();
        camera.set_projection(ProjectionKind::Perspective { vertical_fov: fov });
        let anchor = Vec3::new(1.0, 0.5, -1.0);
        let before = camera.scale_at(anchor);
        camera.zoom(3.0, anchor);
        let ratio = camera.scale_at(anchor) / before;
        let expected = (-3.0 * camera.config().zoom_sensitivity).exp();
        assert!(
            (ratio / expected - 1.0).abs() < 1e-4,
            "fov {fov}: ratio {ratio} != {expected}"
        );
    }
}

#[test]
fn resizing_ortho_preserves_vertical_span_and_updates_aspect() {
    let mut camera = controller();
    camera.set_projection(ProjectionKind::Orthographic);
    camera.set_viewport(Vec2::new(400.0, 400.0));
    let Projection::Orthographic {
        left,
        right,
        bottom,
        top,
    } = camera.projection()
    else {
        panic!("expected orthographic projection");
    };
    assert!(((right - left) - (top - bottom)).abs() < 1e-5);
}
