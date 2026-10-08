use glam::{Quat, Vec3};

use super::*;

fn up_of(orientation: Quat) -> Vec3 {
    orientation * Vec3::Y
}

fn some_frames() -> [Quat; 4] {
    [
        Quat::IDENTITY,
        Quat::from_rotation_z(1.3) * Quat::from_rotation_x(0.4),
        Quat::from_rotation_y(-2.2),
        CameraPose::looking_at(Vec3::ONE, Vec3::ZERO, Vec3::Z).orientation,
    ]
}

/// The camera looks back along the requested direction.
#[test]
fn a_view_faces_the_pivot_from_the_requested_side() {
    let orientation = view_orientation(Vec3::X, Vec3::Y, true, Quat::IDENTITY);
    assert!((orientation * Vec3::NEG_Z).distance(Vec3::NEG_X) < 1e-6);
    assert!(up_of(orientation).distance(Vec3::Y) < 1e-6);
}

/// World-up views off the pole keep world_up on screen-up's side and the
/// horizon level, whatever the current frame.
#[test]
fn world_up_views_are_level_and_independent_of_the_current_frame() {
    for world_up in [Vec3::Z, Vec3::Y] {
        for direction in [Vec3::X, Vec3::NEG_Y, Vec3::ONE, Vec3::new(-0.3, 0.8, -0.5)] {
            if direction.normalize().cross(world_up).length() < 0.1 {
                continue; // A pole: `pole_views_take_the_roll_from_the_pole_axis`.
            }
            let expected = view_orientation(direction, world_up, true, Quat::IDENTITY);
            for current in some_frames() {
                let orientation = view_orientation(direction, world_up, true, current);
                assert_eq!(
                    orientation, expected,
                    "a world-up view ignores the current frame"
                );
                assert!(
                    (orientation * Vec3::X).dot(world_up).abs() < 1e-6,
                    "horizon not level"
                );
                assert!(up_of(orientation).dot(world_up) > 0.0);
            }
        }
    }
}

/// A pole view takes its roll from an explicit axis.
///
/// Z-up: Top and Bottom have +Y up (the view cube labels rely on it). Y-up:
/// −Z up. Independent of the
/// current frame. Mutation oracle: build the pole view with
/// `looking_at(direction, 0, world_up)` — Y-up yields +X and fails.
#[test]
fn pole_views_take_the_roll_from_the_pole_axis() {
    for (world_up, expected_up) in [
        (Vec3::Z, Vec3::Y),
        (Vec3::Y, Vec3::NEG_Z),
        (Vec3::X, Vec3::Z),
    ] {
        assert!(pole_up(world_up).distance(expected_up) < 1e-6);
        for direction in [world_up, -world_up] {
            for current in some_frames() {
                let orientation = view_orientation(direction, world_up, true, current);
                assert!(
                    up_of(orientation).distance(expected_up) < 1e-6,
                    "up {:?} for world_up {world_up:?}, direction {direction:?}",
                    up_of(orientation)
                );
                assert!((orientation * Vec3::NEG_Z).distance(-direction) < 1e-6);
            }
        }
    }
    // A direction within the pole tolerance counts as the pole.
    let tilted = Vec3::new(0.001, 0.0, 1.0);
    assert!(
        up_of(view_orientation(tilted, Vec3::Z, true, Quat::IDENTITY)).distance(Vec3::Y) < 1e-3
    );
}

/// Free views keep the roll nearest to the current frame: a Top view
/// rotated by 90° stays rotated, where a world-up view levels it.
/// Mutation oracle: ignore `current` in free mode — the rolled frame snaps
/// back to +Y.
#[test]
fn free_views_pick_the_axis_with_the_smallest_rotation() {
    let rolled_top =
        view_orientation(Vec3::Z, Vec3::Z, true, Quat::IDENTITY) * Quat::from_rotation_z(1.4);
    let free = view_orientation(Vec3::Z, Vec3::Z, false, rolled_top);
    assert!(
        up_of(free).distance(Vec3::NEG_X) < 1e-6,
        "up {:?}",
        up_of(free)
    );
    let level = view_orientation(Vec3::Z, Vec3::Z, true, rolled_top);
    assert!(up_of(level).distance(Vec3::Y) < 1e-6);

    // A frame that already is an axis view is kept exactly.
    for direction in [Vec3::X, Vec3::NEG_Y, Vec3::Z] {
        for up in [Vec3::X, Vec3::Y, Vec3::Z, Vec3::NEG_Z] {
            if direction.cross(up).length() < 0.5 {
                continue;
            }
            let current = CameraPose::looking_at(direction, Vec3::ZERO, up).orientation;
            let free = view_orientation(direction, Vec3::Z, false, current);
            assert!(free.dot(current).abs() > 1.0 - 1e-6, "{direction:?} {up:?}");
        }
    }
}

/// From the isometric view −X and −Y are an exact tie for a free Top view;
/// the axis order decides, so the result does not depend on rounding.
#[test]
fn free_view_ties_resolve_in_axis_order() {
    let iso = CameraPose::looking_at(Vec3::ONE, Vec3::ZERO, Vec3::Z).orientation;
    let top = view_orientation(Vec3::Z, Vec3::Z, false, iso);
    assert!(
        up_of(top).distance(Vec3::NEG_X) < 1e-6,
        "up {:?}",
        up_of(top)
    );
    let nudged = Quat::from_rotation_z(1e-6) * iso;
    assert_eq!(view_orientation(Vec3::Z, Vec3::Z, false, nudged), top);
}
