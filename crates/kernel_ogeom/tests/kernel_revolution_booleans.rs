//! Booleans between whole revolutions about one axis whose seams lie on
//! one half-plane, turned off the profiles' plane.

use ogeom::algo::{make_face, make_polygon, make_revolution, transformed};
use ogeom::core::Tolerances;
use ogeom::geom::{PlaneSurface, SurfaceGeometry};
use ogeom::math::{Axis, Direction, Frame, Plane, Point, Transform};
use ogeom::topo::{Model, Shape};

/// The rectangle `y0..y1` by `z0..z1` in the YZ plane, turned `seam`
/// radians about Z, then revolved a whole turn about Z: its seam lies at
/// `seam`.
fn whole_turn(model: &mut Model, y0: f64, y1: f64, z0: f64, z1: f64, seam: f64) -> Shape {
    let tol = Tolerances::default();
    let pts = [
        Point::new(0.0, y0, z0),
        Point::new(0.0, y1, z0),
        Point::new(0.0, y1, z1),
        Point::new(0.0, y0, z1),
    ];
    let wire = make_polygon(model, &pts, true, tol).unwrap().shape;
    let plane =
        Plane::new(Frame::new(Point::new(0.0, 0.0, 0.0), Direction::X, Direction::Y, tol).unwrap());
    let surface = PlaneSurface::over(plane, (-50.0, 50.0), (-50.0, 50.0)).unwrap();
    let face = make_face(model, SurfaceGeometry::Plane(surface), &[wire], tol)
        .unwrap()
        .shape;
    let axis = Axis::new(Point::new(0.0, 0.0, 0.0), Direction::Z);
    let face = transformed(model, &face, Transform::rotation(axis, seam))
        .unwrap()
        .shape;
    make_revolution(model, &face, axis, std::f64::consts::TAU, tol)
        .unwrap()
        .shape
}

/// A cylinder r 10, h 20 and a collar over its top rim (r 8..14, z
/// 18..24), both whole turns with their seams at one angle: at 0 they
/// fuse and cut; at 1 radian both fail.
#[test]
fn whole_revolutions_with_seams_together_fuse_and_cut() {
    let tol = Tolerances::default();
    for seam in [0.0, 1.0] {
        let mut model = Model::new();
        let cylinder = whole_turn(&mut model, 0.0, 10.0, 0.0, 20.0, seam);
        let collar = whole_turn(&mut model, 8.0, 14.0, 18.0, 24.0, seam);
        let fused = ogeom::boolean::fuse(&mut model, &cylinder, &collar, tol);
        let cut = ogeom::boolean::cut(&mut model, &cylinder, &collar, tol);
        assert!(fused.is_ok(), "fuse, seams at {seam}: {:?}", fused.err());
        assert!(cut.is_ok(), "cut, seams at {seam}: {:?}", cut.err());
    }
}
