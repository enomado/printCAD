//! A revolved hemisphere scaled unevenly into half an ellipsoid measures,
//! whatever its three radii and wherever its seam was turned to before the
//! revolution.

use ogeom::algo::{
    general_transformed_shape, make_edge_between, make_face, make_revolution, make_vertex,
    make_wire, transformed, volume_properties,
};
use ogeom::core::Tolerances;
use ogeom::geom::{CircleCurve, LineCurve, PlaneSurface};
use ogeom::math::{
    Axis, Circle, Direction, Frame, GeneralTransform, Matrix3, Plane, Point, Transform, Vector,
};
use ogeom::mesh::Deflection;
use ogeom::topo::{Model, Shape};

const PI: f64 = std::f64::consts::PI;

/// A quarter disc of radius 1 in the XZ plane: out along X, round the arc
/// to the pole on Z, back down the axis.
fn quarter_disc(model: &mut Model, t: Tolerances) -> Shape {
    let [o, x, z] = [
        Point::new(0.0, 0.0, 0.0),
        Point::new(1.0, 0.0, 0.0),
        Point::new(0.0, 0.0, 1.0),
    ]
    .map(|p| make_vertex(model, p).shape);
    let out = LineCurve::segment(Point::new(0.0, 0.0, 0.0), Point::new(1.0, 0.0, 0.0), t).unwrap();
    let out = make_edge_between(model, out.into(), (0.0, 1.0), &o, &x, t)
        .unwrap()
        .shape;
    let frame = Frame::new(Point::new(0.0, 0.0, 0.0), -Direction::Y, Direction::X, t).unwrap();
    let arc = CircleCurve::new(Circle::new(frame, 1.0, t).unwrap());
    let arc = make_edge_between(model, arc.into(), (0.0, PI / 2.0), &x, &z, t)
        .unwrap()
        .shape;
    let down = LineCurve::segment(Point::new(0.0, 0.0, 1.0), Point::new(0.0, 0.0, 0.0), t).unwrap();
    let down = make_edge_between(model, down.into(), (0.0, 1.0), &z, &o, t)
        .unwrap()
        .shape;
    let wire = make_wire(model, &[out, arc, down], t).unwrap().shape;
    let plane = Plane::new(frame);
    make_face(model, PlaneSurface::new(plane).into(), &[wire], t)
        .unwrap()
        .shape
}

/// Half an ellipsoid: the hemisphere turned `seam` radians about Z before
/// its whole turn, then scaled by three radii.
#[test]
#[ignore = "kernel: a hemisphere scaled on all three axes does not mesh (ogeom-rs#96)"]
fn a_scaled_hemisphere_measures_as_half_an_ellipsoid() {
    let t = Tolerances::millimetres();
    for radii in [[2.0, 1.0, 1.0], [8.0, 5.0, 3.0]] {
        for seam in [0.0, 1.0] {
            let want = 2.0 / 3.0 * PI * radii[0] * radii[1] * radii[2];
            let mut model = Model::new();
            let quarter = quarter_disc(&mut model, t);
            let axis = Axis::new(Point::new(0.0, 0.0, 0.0), Direction::Z);
            let quarter = transformed(&mut model, &quarter, Transform::rotation(axis, seam))
                .unwrap()
                .shape;
            let solid = make_revolution(&mut model, &quarter, axis, 2.0 * PI, t)
                .unwrap()
                .shape;
            let scale = GeneralTransform {
                linear: Matrix3::from_columns(
                    Vector::new(radii[0], 0.0, 0.0),
                    Vector::new(0.0, radii[1], 0.0),
                    Vector::new(0.0, 0.0, radii[2]),
                ),
                translation: Vector::new(0.0, 0.0, 0.0),
            };
            let scaled = general_transformed_shape(&mut model, &solid, &scale, t)
                .unwrap()
                .shape;
            let got =
                volume_properties(&model, &scaled, Deflection::default(), t).map(|p| p.mass.abs());
            assert!(
                got.as_ref().is_ok_and(|v| (v - want).abs() < 1e-3 * want),
                "radii {radii:?}, seam at {seam}: {got:?} for {want}"
            );
        }
    }
}
