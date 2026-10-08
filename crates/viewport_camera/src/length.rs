use serde::{Deserialize, Serialize};

/// Length in host model units: ray distance, clipping interval or camera
/// depth. Finite and non-negative.
///
/// Camera depths are stored in `NavigationLimits` as bare numbers through
/// `serde(transparent)`. Deserialization bypasses `new`; file values are
/// checked by `NavigationLimits::validate`.
///
/// Arithmetic is handled explicitly at the numerical boundary.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Length(pub f64);

impl Length {
    pub const ZERO: Self = Self::new(0.0);

    pub const fn new(value: f64) -> Self {
        assert!(
            value.is_finite() && value >= 0.0,
            "Length: must be finite and non-negative"
        );
        Self(value)
    }
}

/// Signed displacement along an axis. Finite; the sign indicates direction.
/// Unlike [`Length`], offsets may be negative.
///
/// Arithmetic is handled explicitly at the numerical boundary.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Offset(pub f64);

impl Offset {
    pub const ZERO: Self = Self::new(0.0);

    pub const fn new(value: f64) -> Self {
        assert!(value.is_finite(), "Offset: must be finite");
        Self(value)
    }
}
