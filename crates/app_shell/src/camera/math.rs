use axes::AxisSystem;
use glam::{Mat3, Vec3};

pub(crate) fn axis_basis(axes: &AxisSystem) -> Mat3 {
    Mat3::from_cols(
        axes.horizontal().vector(),
        axes.vertical().vector(),
        axes.depth().vector(),
    )
}

pub(crate) fn axis_parity(axes: &AxisSystem) -> f32 {
    let triple = axes
        .horizontal()
        .vector()
        .cross(axes.vertical().vector())
        .dot(axes.depth().vector());
    if triple < 0.0 { -1.0 } else { 1.0 }
}

pub(crate) fn control_horizontal_vec(axes: &AxisSystem) -> Vec3 {
    let mut h = axes.horizontal().vector();
    if axis_parity(axes) < 0.0 {
        h = -h;
    }
    h
}
