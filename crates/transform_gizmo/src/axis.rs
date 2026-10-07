//! The letter of a handle.
use glam::DVec3;

/// X, Y or Z: a handle's letter and colour (X red, Y green, Z blue by convention),
/// and the unit axis of a placement's orientation it stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    pub const ALL: [Self; 3] = [Self::X, Self::Y, Self::Z];

    pub fn unit(self) -> DVec3 {
        match self {
            Self::X => DVec3::X,
            Self::Y => DVec3::Y,
            Self::Z => DVec3::Z,
        }
    }

    pub fn letter(self) -> &'static str {
        match self {
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
        }
    }

    pub(crate) fn index(self) -> usize {
        match self {
            Self::X => 0,
            Self::Y => 1,
            Self::Z => 2,
        }
    }

    /// Y after X, Z after Y, X after Z.
    pub(crate) fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % 3]
    }
}
