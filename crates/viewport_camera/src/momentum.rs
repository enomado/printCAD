//! Inertia after releasing a pointer gesture.
//!
//! Inertia is the **continuation of the released gesture**: the camera keeps
//! being sampled from the same immutable press snapshot ([`NavigationDrag`]) at
//! a virtual pointer `release + s(t)·direction`. Anchor, world-up orbit with
//! its pole clamp, `PanGrip` and the host's floating origin therefore work with
//! no new camera mathematics, and stopping simply keeps the shown camera.
//!
//! The brake uses damping proportional to
//! speed plus constant friction, `dv/dt = −(a·v + b)` — but in closed form
//! over explicit elapsed time instead of per-frame Euler steps, so the path
//! does not depend on the frame rate or on how frames split the time:
//!
//! - `v(t) = (v₀ + b/a)·e^(−a·t) − b/a`;
//! - stop at `t* = ln(1 + a·v₀/b) / a` (finite because `b > 0`);
//! - path `s(t) = (v₀ + b/a)·(1 − e^(−a·t))/a − b·t/a` for `t ≤ t*`, `s(t*)` after.
//!
//! The release speed comes from the pointer track over a time window before
//! the release ([`PointerTrack`]), not from the last frame's `delta / dt`: a
//! pointer that stood still for the whole window releases with zero speed and
//! nothing coasts. The wheel never starts inertia.
use std::collections::VecDeque;
use std::time::Duration;

use glam::Vec2;
use serde::{Deserialize, Serialize};

use crate::controller::{CameraController, GestureKind};
use crate::navigation::{NavigationDrag, ScreenPoint};
use crate::smoothing::SmoothingMillis;

/// Wheel and trackpad-scroll tail arriving this soon after a release neither
/// stop the inertia nor zoom: they are ignored (upstream `input_debounce`, И6).
/// Later wheel input stops the inertia and zooms.
pub const WHEEL_DEBOUNCE: Duration = Duration::from_millis(80);

/// Speed-proportional damping `a`, per second.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DampingRate(pub f32);

/// Constant friction `b`, logical pixels per second squared. Being positive,
/// it ends every inertia in finite time.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Friction(pub f32);

/// Pointer velocity, logical pixels per second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenVelocity(pub Vec2);

/// Brake and release-speed window for one gesture kind.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MomentumLaw {
    pub damping: DampingRate,
    pub friction: Friction,
    /// Release speed = pointer displacement over this window before the
    /// release, divided by the window.
    pub window: SmoothingMillis,
}

impl MomentumLaw {
    /// Upstream defaults `damping 160`, `friction 0.2` in our units:
    /// `a = 160/256·10 = 6.25 1/s`, `b = 0.2·40 = 8 px/s²`.
    fn upstream(window: SmoothingMillis) -> Self {
        Self {
            damping: DampingRate(6.25),
            friction: Friction(8.0),
            window,
        }
    }

    pub fn validate(self) -> Result<(), String> {
        if !self.damping.0.is_finite() || !(0.5..=50.0).contains(&self.damping.0) {
            return Err("camera inertia damping must be finite and in 0.5..=50 per second".into());
        }
        if !self.friction.0.is_finite() || !(0.5..=5000.0).contains(&self.friction.0) {
            return Err("camera inertia friction must be finite and in 0.5..=5000 px/s²".into());
        }
        if !(10..=250).contains(&self.window.0) {
            return Err("camera inertia velocity window must be in 10..=250 milliseconds".into());
        }
        Ok(())
    }

    /// Time until the speed `v0` (px/s) brakes to zero.
    pub fn stop_time(self, speed: f32) -> Duration {
        let (a, b, v0) = self.coefficients(speed);
        Duration::from_secs_f64((1.0 + a * v0 / b).ln() / a)
    }

    /// Path `s(t)` in logical pixels, held at `s(t*)` after the stop.
    pub fn distance(self, speed: f32, elapsed: Duration) -> f32 {
        let (a, b, v0) = self.coefficients(speed);
        let t = elapsed.min(self.stop_time(speed)).as_secs_f64();
        let s = (v0 + b / a) * (-(-a * t).exp_m1()) / a - b * t / a;
        // Rounding near `t*` may dip a hair below the maximum; never backwards.
        s.max(0.0) as f32
    }

    /// f64 for the closed form: `1 − e^(−a·t)` and `b·t/a` nearly cancel near
    /// the stop, and the difference is what moves the camera.
    fn coefficients(self, speed: f32) -> (f64, f64, f64) {
        assert!(
            speed.is_finite() && speed >= 0.0,
            "inertia speed must be finite and non-negative"
        );
        (
            f64::from(self.damping.0),
            f64::from(self.friction.0),
            f64::from(speed),
        )
    }
}

/// Inertia for pan and orbit separately; the wheel has none (И4).
/// Off by default: the default is the exact CAD camera, release means stop.
/// Container default: a file without the field, or without one of its
/// fields, loads the default (`NavigationSettings.momentum`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MomentumSettings {
    pub enabled: bool,
    pub pan: MomentumLaw,
    pub orbit: MomentumLaw,
}

impl Default for MomentumSettings {
    fn default() -> Self {
        // Upstream `init_pan` 40 ms, `init_orbit` 60 ms.
        Self {
            enabled: false,
            pan: MomentumLaw::upstream(SmoothingMillis(40)),
            orbit: MomentumLaw::upstream(SmoothingMillis(60)),
        }
    }
}

impl MomentumSettings {
    pub fn validate(self) -> Result<(), String> {
        self.pan.validate()?;
        self.orbit.validate()
    }

    pub fn law(self, kind: GestureKind) -> MomentumLaw {
        match kind {
            GestureKind::Pan => self.pan,
            GestureKind::Orbit => self.orbit,
        }
    }
}

/// Timestamped pointer positions of one hold, for the release speed (И3).
/// Time is the host's explicit clock since the press, like
/// [`crate::smoothing::SmoothedInput`]; positions are the raw pointer, never
/// a smoothed one.
pub struct PointerTrack {
    window: Duration,
    time: Duration,
    /// Ordered by time. The first entry is the newest sample at or before
    /// `time − window` — the pointer's position at the window's start — or
    /// the press while the hold is younger than the window (before the
    /// press the pointer was at the press).
    samples: VecDeque<(Duration, Vec2)>,
}

impl PointerTrack {
    pub fn new(press: ScreenPoint, window: Duration) -> Self {
        assert!(press.0.is_finite(), "pointer track press must be finite");
        assert!(!window.is_zero(), "pointer track window must be positive");
        Self {
            window,
            time: Duration::ZERO,
            samples: VecDeque::from([(Duration::ZERO, press.0)]),
        }
    }

    pub fn advance(&mut self, delta: Duration) {
        self.time = self
            .time
            .checked_add(delta)
            .expect("pointer track clock overflow");
        if let Some(start) = self.time.checked_sub(self.window) {
            while self.samples.len() >= 2 && self.samples[1].0 <= start {
                self.samples.pop_front();
            }
        }
    }

    /// The pointer at the current time; a second sample at the same time
    /// replaces the first.
    pub fn input(&mut self, point: ScreenPoint) {
        assert!(point.0.is_finite(), "pointer track sample must be finite");
        match self.samples.back_mut() {
            Some((time, position)) if *time == self.time => *position = point.0,
            _ => self.samples.push_back((self.time, point.0)),
        }
    }

    /// Average velocity over the window ending now. A pointer that has not
    /// moved during the whole window has zero velocity.
    ///
    /// The position at the window's start is interpolated linearly between
    /// the samples around it: frames may be coarser than the window (33 ms at
    /// 30 FPS against a 40 ms window), and taking the older sample as is would
    /// stretch the displacement over more time than the window. A still
    /// pointer has equal samples on both sides, so stillness stays exactly 0.
    pub fn velocity(&self) -> ScreenVelocity {
        let now = self.samples.back().unwrap().1;
        let (first_time, first) = self.samples[0];
        let start = match (self.time.checked_sub(self.window), self.samples.get(1)) {
            (Some(start), Some(&(next_time, next))) if first_time <= start => {
                let span = (next_time - first_time).as_secs_f64();
                let fraction = (start - first_time).as_secs_f64() / span;
                first.lerp(next, fraction as f32)
            }
            // Younger than the window, or one sample: the press (or the only
            // sample) is where the pointer was at the window's start.
            _ => first,
        };
        ScreenVelocity((now - start) / self.window.as_secs_f32())
    }
}

/// The released gesture coasting to a stop.
#[derive(Clone, Copy, Debug)]
pub struct Momentum {
    drag: NavigationDrag,
    release: ScreenPoint,
    /// Unit screen direction of the release velocity.
    direction: Vec2,
    /// Release speed, px/s, positive.
    speed: f32,
    law: MomentumLaw,
    elapsed: Duration,
}

impl Momentum {
    /// `release` must be the point the drag was sampled at on release, so the
    /// first inertia sample is exactly the released camera. Zero velocity:
    /// no inertia.
    pub fn release(
        drag: NavigationDrag,
        release: ScreenPoint,
        velocity: ScreenVelocity,
        law: MomentumLaw,
    ) -> Option<Self> {
        assert!(
            release.0.is_finite(),
            "inertia release point must be finite"
        );
        assert!(velocity.0.is_finite(), "inertia velocity must be finite");
        law.validate()
            .expect("inertia law validated at the UI/file boundary");
        let speed = velocity.0.length();
        (speed > 0.0).then(|| Self {
            drag,
            release,
            direction: velocity.0 / speed,
            speed,
            law,
            elapsed: Duration::ZERO,
        })
    }

    pub fn advance(&mut self, delta: Duration) {
        self.elapsed = self.elapsed.saturating_add(delta);
    }

    pub fn is_done(&self) -> bool {
        self.elapsed >= self.law.stop_time(self.speed)
    }

    /// Wheel input now is a scroll tail of the gesture and is ignored (И6).
    pub fn wheel_debounced(&self) -> bool {
        self.elapsed < WHEEL_DEBOUNCE
    }

    /// The virtual pointer: the release point moved along the path so far.
    pub fn pointer(&self) -> ScreenPoint {
        ScreenPoint(self.release.0 + self.direction * self.law.distance(self.speed, self.elapsed))
    }

    pub fn sample(&self) -> CameraController {
        self.drag.sample(self.pointer())
    }

    pub fn drag(&self) -> NavigationDrag {
        self.drag
    }
}

#[cfg(test)]
#[path = "momentum_tests.rs"]
mod tests;
