//! Transform handles in the host's 3D space, drawn on screen: layout, hit-test,
//! the drag and its delta, and the shapes to draw. No window and no painter: the
//! host paints [`paint::Shape`]s and feeds pointer input ([`gizmo::Gizmo`]).
//!
//! Host space is plain `glam` in `f64` (points, unit directions, a placement's
//! orientation), angles are radians and screen points are logical pixels
//! (`emath::Pos2`). The host converts its own types at this boundary.
pub mod axis;
pub mod gizmo;
pub mod hand;
pub mod paint;
pub mod screen;
pub mod view;
