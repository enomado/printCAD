//! Filleting closed edges: a cylinder's two rims in one chain, and a rim
//! the kernel keeps as two half circles.

use ogeom::algo::make_cylinder;
use ogeom::core::Tolerances;
use ogeom::fillet::fillet_edges;
use ogeom::math::{Direction, Frame, Point};
use ogeom::topo::{EdgeRepr, Model, NodeData, ShapeType, explore_unique};

/// The circular edges of a solid.
fn circles(model: &Model, solid: &ogeom::topo::Shape) -> Vec<ogeom::topo::Shape> {
    explore_unique(model, solid, ShapeType::Edge)
        .unwrap()
        .into_iter()
        .filter(|e| {
            let Some(NodeData::Edge(data)) = model.node(e).map(|n| n.data()) else {
                return false;
            };
            let Some(EdgeRepr::Curve3d { curve, .. }) = data.curve3d() else {
                return false;
            };
            model
                .geometry()
                .curve(*curve)
                .is_some_and(|c| format!("{c:?}").contains("Circle"))
        })
        .collect()
}

#[test]
fn both_rims_of_a_cylinder_round_in_one_chain() {
    let tol = Tolerances::default();
    let mut model = Model::new();
    let frame = Frame::new(Point::new(0.0, 0.0, 0.0), Direction::Z, Direction::X, tol).unwrap();
    let cylinder = make_cylinder(&mut model, frame, 10.0, 20.0, tol)
        .unwrap()
        .shape;
    let rims = circles(&model, &cylinder);
    assert_eq!(rims.len(), 2, "a cylinder has two rims");
    let rounded = fillet_edges(&mut model, &cylinder, &rims, 1.0, tol);
    assert!(rounded.is_ok(), "{:?}", rounded.err());
}

/// A blind hole's rim, the two half circles the cut leaves of it, rounded
/// in one chain.
#[test]
fn a_hole_rim_of_two_halves_rounds_in_one_chain() {
    let tol = Tolerances::default();
    let mut model = Model::new();
    let at = |x, y, z| Frame::new(Point::new(x, y, z), Direction::Z, Direction::X, tol).unwrap();
    let block = ogeom::algo::make_box(&mut model, at(-20.0, -20.0, 0.0), (40.0, 40.0, 10.0), tol)
        .unwrap()
        .shape;
    let bore = make_cylinder(&mut model, at(0.0, 0.0, 5.0), 5.0, 10.0, tol)
        .unwrap()
        .shape;
    let holed = ogeom::boolean::cut(&mut model, &block, &bore, tol)
        .unwrap()
        .shape;
    // The rim: the circular edges at the top face.
    let rim: Vec<_> = circles(&model, &holed)
        .into_iter()
        .filter(|e| {
            let v = explore_unique(&model, e, ShapeType::Vertex).unwrap();
            v.iter().all(|v| {
                matches!(model.node(v).map(|n| n.data()),
                    Some(NodeData::Vertex(d)) if (v.transform(model.datums()).unwrap().apply(d.point).z - 10.0).abs() < 1e-6)
            })
        })
        .collect();
    assert!(!rim.is_empty());
    let rounded = fillet_edges(&mut model, &holed, &rim, 1.0, tol);
    assert!(
        rounded.is_ok(),
        "{} rim edges: {:?}",
        rim.len(),
        rounded.err()
    );
}
