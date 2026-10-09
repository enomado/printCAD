use serde::{Deserialize, Serialize};

/// View scale in host model units per logical screen pixel.
/// Finite and strictly positive.
///
/// Stored in `crate::navigation::NavigationLimits` as a bare number through
/// `serde(transparent)`. Deserialization bypasses `new`; file values are
/// checked by `NavigationLimits::validate`.
///
/// Arithmetic is handled explicitly at the numerical boundary.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PerPx(pub f64);

impl PerPx {
    pub const fn new(value: f64) -> Self {
        assert!(
            value.is_finite() && value > 0.0,
            "PerPx: scale must be finite and positive"
        );
        Self(value)
    }
}
