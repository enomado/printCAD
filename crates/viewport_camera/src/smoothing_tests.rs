use std::time::Duration;

use glam::DVec2;

use super::{SmoothedInput, SmoothingMillis, SmoothingSettings};

#[test]
fn input_sum_endpoint_and_delay_are_independent_of_quiet_frame_partition() {
    let mut cases = 0;
    for millis in [0, 10, 30, 60, 250] {
        for frames in [1, 2, 10, 100] {
            let window = Duration::from_millis(millis);
            let mut input = SmoothedInput::new(DVec2::ZERO, window);
            input.input(DVec2::new(40.0, -20.0));
            if millis > 0 {
                assert_eq!(input.value(), DVec2::ZERO);
                assert!(input.active());
            }
            for _ in 0..frames {
                input.advance(window / frames);
                assert_eq!(input.value(), input.value(), "reads must be idempotent");
            }
            input.advance(window); // Also cover pauses longer than the window.
            assert_eq!(input.value(), DVec2::new(40.0, -20.0));
            assert!(!input.active());
            input.input(DVec2::new(-10.0, 35.0));
            input.advance(window);
            assert_eq!(input.value(), DVec2::new(-10.0, 35.0));
            cases += 1;
        }
    }
    assert_eq!(cases, 20);
}

#[test]
fn timestamps_preserve_overlapping_impulses_across_render_rates() {
    let play = |quantum| {
        let mut input = SmoothedInput::new(DVec2::ZERO, Duration::from_millis(60));
        input.input(DVec2::new(60.0, 0.0));
        for _ in 0..30 / quantum {
            input.advance(Duration::from_millis(quantum));
        }
        input.input(DVec2::new(30.0, 15.0));
        for _ in 0..30 / quantum {
            input.advance(Duration::from_millis(quantum));
        }
        assert_eq!(input.value(), DVec2::new(45.0, 7.5));
        input.advance(Duration::from_millis(30));
        assert_eq!(input.value(), DVec2::new(30.0, 15.0));
        assert!(!input.active());
        input.value()
    };
    for quantum in [1, 5, 10, 30] {
        assert_eq!(play(quantum), DVec2::new(30.0, 15.0));
    }
    let mut settings = SmoothingSettings::default();
    for millis in [250, 251, u16::MAX] {
        settings.pan = SmoothingMillis(millis);
        assert_eq!(settings.validate().is_ok(), millis <= 250);
    }
}
