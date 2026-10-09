//! The camera boundary of the gizmo.
use emath::Pos2;
use glam::DVec3;

/// Projection and rays of the host's viewport, in host space (`f64`). `project`
/// and `ray` must use the same lens and the same floating origin; the gizmo never
/// sees either, only host space and logical screen pixels.
pub trait GizmoView {
    fn project(&self, point: DVec3) -> Option<Pos2>;
    /// Origin and unit direction of the ray through a screen point.
    fn ray(&self, point: Pos2) -> Option<(DVec3, DVec3)>;
    /// Unit line of sight.
    fn forward(&self) -> DVec3;
    /// Host-space units spanned by one logical pixel at `point` (axial depth in
    /// perspective), finite and positive: the handles keep a constant pixel size.
    fn world_per_pixel(&self, point: DVec3) -> f64;
}
