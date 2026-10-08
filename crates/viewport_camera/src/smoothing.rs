//! Time-based input smoothing only; camera mathematics remains in [`crate::controller`].
//! Each input delta is delivered linearly over a bounded time window. Samples
//! use explicit elapsed time, so quiet frames consume pending input without
//! generating momentum, and observing a sample never consumes it twice.
use std::collections::VecDeque;
use std::time::Duration;

use glam::DVec2;
use serde::{Deserialize, Serialize};

/// Milliseconds at the settings/file boundary, not a frame count.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SmoothingMillis(pub u16);

impl SmoothingMillis {
    pub fn duration(self) -> Duration {
        Duration::from_millis(u64::from(self.0))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SmoothingSettings {
    pub enabled: bool,
    pub pan: SmoothingMillis,
    pub orbit: SmoothingMillis,
    pub zoom: SmoothingMillis,
}

impl Default for SmoothingSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            pan: SmoothingMillis(10),
            orbit: SmoothingMillis(30),
            zoom: SmoothingMillis(60),
        }
    }
}

impl SmoothingSettings {
    pub fn validate(self) -> Result<(), String> {
        if [self.pan, self.orbit, self.zoom]
            .iter()
            .any(|ms| ms.0 > 250)
        {
            return Err("camera smoothing windows must be in 0..=250 milliseconds".into());
        }
        Ok(())
    }
}

struct InputDelta {
    time: Duration,
    value: DVec2,
}

/// Numerical input filter. Adapters use logical-pixel positions for drag and
/// log scale for zoom; these values never enter world-space geometry directly.
pub struct SmoothedInput {
    time: Duration,
    window: Duration,
    latest: DVec2,
    pending: VecDeque<InputDelta>,
}

impl SmoothedInput {
    pub fn new(initial: DVec2, window: Duration) -> Self {
        assert!(initial.is_finite(), "smoothing input must be finite");
        Self {
            time: Duration::ZERO,
            window,
            latest: initial,
            pending: VecDeque::new(),
        }
    }

    pub fn advance(&mut self, delta: Duration) {
        self.time = self
            .time
            .checked_add(delta)
            .expect("smoothing clock overflow");
        while self
            .pending
            .front()
            .is_some_and(|entry| self.time - entry.time >= self.window)
        {
            self.pending.pop_front();
        }
    }

    pub fn input(&mut self, value: DVec2) {
        assert!(value.is_finite(), "smoothing input must be finite");
        let delta = value - self.latest;
        assert!(delta.is_finite(), "smoothing delta must be finite");
        self.latest = value;
        if !self.window.is_zero() && delta != DVec2::ZERO {
            self.pending.push_back(InputDelta {
                time: self.time,
                value: delta,
            });
        }
    }

    pub fn value(&self) -> DVec2 {
        // Subtract undelivered fractions from the latest absolute target.
        // After the window expires, the endpoint is exactly the latest input;
        // rounding cannot accumulate through a succession of quiet frames.
        let mut value = self.latest;
        for entry in &self.pending {
            let fraction = (self.time - entry.time).as_secs_f64() / self.window.as_secs_f64();
            value -= entry.value * (1.0 - fraction);
        }
        assert!(value.is_finite(), "smoothed input must be finite");
        value
    }

    pub fn active(&self) -> bool {
        !self.pending.is_empty()
    }
    pub fn target(&self) -> DVec2 {
        self.latest
    }
}

#[cfg(test)]
#[path = "smoothing_tests.rs"]
mod tests;
