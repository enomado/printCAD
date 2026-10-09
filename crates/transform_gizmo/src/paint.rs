//! What the gizmo draws, as screen shapes in colour roles. The host maps every
//! [`Ink`] to its palette and draws the shapes in list order (later on top).
use emath::Pos2;

use crate::axis::Axis;

/// A colour role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ink {
    /// Axis colour of a handle letter: X red, Y green, Z blue by convention.
    Axis(Axis),
    /// Planar handle of two slides with the same letter (say X of the world and X
    /// of a datum): there is no third letter to colour it by.
    SameLetterPlane,
    /// The screen-plane handle.
    View,
    /// The hovered or held handle, and the value plate's text.
    Highlight,
    /// Drag feedback: ghost, travel line, swept sector, plate frame.
    Feedback,
    /// Black with this alpha: shadows and outlines.
    Black(u8),
    /// White with this alpha: the sphere outline.
    White(u8),
}

/// A colour role, faded: the host multiplies its premultiplied colour by `fade`
/// (1 = as is).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paint {
    pub ink: Ink,
    pub fade: f32,
}

impl Paint {
    pub const fn of(ink: Ink) -> Self {
        Self { ink, fade: 1.0 }
    }

    pub fn faded(self, fade: f32) -> Self {
        Self {
            ink: self.ink,
            fade: self.fade * fade,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroke {
    /// Logical px.
    pub width: f32,
    pub paint: Paint,
}

impl Stroke {
    pub const fn new(width: f32, paint: Paint) -> Self {
        Self { width, paint }
    }
}

/// Dash and gap lengths along a line, px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dash {
    pub dash: f32,
    pub gap: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// Polyline through `points`, dashed when `dash` is set.
    Line {
        points: Vec<Pos2>,
        stroke: Stroke,
        dash: Option<Dash>,
    },
    /// Filled triangle fan from `points[0]`: triangles `(0, i, i + 1)`. Convex
    /// polygons and sectors seen from their centre are both fans.
    Fan {
        points: Vec<Pos2>,
        paint: Paint,
    },
    Dot {
        at: Pos2,
        radius: f32,
        paint: Paint,
    },
    Circle {
        at: Pos2,
        radius: f32,
        stroke: Stroke,
    },
    /// An axis letter centred on `at`, proportional font, 13 px.
    Letter {
        at: Pos2,
        text: &'static str,
        paint: Paint,
    },
    /// The value of a drag: monospace text, 13 px, its top left at `at`, in
    /// [`Ink::Highlight`] on a plate of `Ink::Black(200)` padded 4 px, rounded 3 px
    /// and framed 1 px in [`Ink::Feedback`].
    Plate {
        at: Pos2,
        text: String,
    },
}
