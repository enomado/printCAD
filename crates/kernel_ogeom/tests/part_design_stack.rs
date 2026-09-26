//! Full-stack Part Design test: sketch geometry → Pad/Pocket features →
//! `wb_part::body_build_ops` → `OgeomKernel::execute_solid_chain` → mesh.
//! This exercises the exact pipeline the app's recompute driver runs.

use core_document::{BodyId, Document, FeatureId};
use kernel_api::TessellationSettings;
use kernel_ogeom::OgeomKernel;
use wb_part::PartFeature;
use wb_sketch::SketchFeature;
use wb_sketch::sketch::{Circle, GeometryElement, Line, Point, Sketch, Vec2D};

fn rect_sketch_on(plane: wb_sketch::sketch::SketchPlane, width: f32, height: f32) -> SketchFeature {
    let mut sketch = Sketch::new("s");
    sketch.plane = plane;
    let a = sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(0.0, 0.0))));
    let b = sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(width, 0.0))));
    let c = sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(
        width, height,
    ))));
    let d = sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(0.0, height))));
    for (s, e) in [(a, b), (b, c), (c, d), (d, a)] {
        sketch.add_geometry(GeometryElement::Line(Line::new(s, e)));
    }
    SketchFeature::new(sketch, plane)
}

fn rect_sketch(width: f32, height: f32) -> SketchFeature {
    let mut sketch = Sketch::new("s");
    let a = sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(0.0, 0.0))));
    let b = sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(width, 0.0))));
    let c = sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(
        width, height,
    ))));
    let d = sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(0.0, height))));
    for (s, e) in [(a, b), (b, c), (c, d), (d, a)] {
        sketch.add_geometry(GeometryElement::Line(Line::new(s, e)));
    }
    let plane = sketch.plane;
    SketchFeature::new(sketch, plane)
}

fn circle_sketch_on(
    plane: wb_sketch::sketch::SketchPlane,
    cx: f32,
    cy: f32,
    r: f32,
) -> SketchFeature {
    let mut sketch = Sketch::new("c");
    sketch.plane = plane;
    let center = sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(cx, cy))));
    sketch.add_geometry(GeometryElement::Circle(Circle::new(center, r)));
    SketchFeature::new(sketch, plane)
}

fn setup(width: f32, height: f32) -> (Document, BodyId, FeatureId) {
    let mut doc = Document::new("t");
    let body = doc.create_body(Some("Body".into()));
    let sketch_id = doc
        .add_feature_in_body(rect_sketch(width, height), "sketch".into(), Some(body))
        .unwrap();
    (doc, body, sketch_id)
}

fn pad_feature(sketch: FeatureId, length: f32, reversed: bool, symmetric: bool) -> PartFeature {
    PartFeature::Pad {
        refine: false,
        sketch: Some(sketch),
        length,
        reversed,
        symmetric,
        mode: wb_part::ExtrudeMode::Dimension,
        length2: 0.0,
        taper_deg: 0.0,
        up_to_face: None,
        up_to_offset: 0.0,
        profile_face: None,
        direction: Default::default(),
        up_to_shape: Vec::new(),
        mode2: None,
        up_to_face2: None,
        up_to_offset2: 0.0,
        up_to_shape2: Vec::new(),
    }
}

fn pocket_feature(sketch: FeatureId, depth: f32) -> PartFeature {
    PartFeature::Pocket {
        refine: false,
        sketch: Some(sketch),
        depth,
        reversed: false,
        symmetric: false,
        through_all: false,
        mode: wb_part::ExtrudeMode::Dimension,
        depth2: 0.0,
        taper_deg: 0.0,
        up_to_face: None,
        up_to_offset: 0.0,
        profile_face: None,
        direction: Default::default(),
        up_to_shape: Vec::new(),
        mode2: None,
        up_to_face2: None,
        up_to_offset2: 0.0,
        up_to_shape2: Vec::new(),
    }
}

fn mesh_bounds(mesh: &kernel_api::TriMesh) -> ([f32; 3], [f32; 3]) {
    mesh.bounds().expect("non-empty mesh")
}

#[test]
fn pad_feature_builds_a_box_through_the_full_stack() {
    let (mut doc, body, sketch_id) = setup(10.0, 5.0);
    doc.add_feature_in_body(
        pad_feature(sketch_id, 8.0, false, false),
        "Pad".into(),
        Some(body),
    )
    .unwrap();

    let ops = wb_part::body_build_ops(&doc, body).unwrap().ops;
    let mut kernel = OgeomKernel::new();
    let result = kernel
        .execute_solid_chain(&ops, &TessellationSettings::default())
        .unwrap();

    assert!(!result.brep_blob.is_empty());
    let (min, max) = mesh_bounds(&result.mesh);
    assert!((max[0] - min[0] - 10.0).abs() < 1e-3, "width");
    assert!((max[1] - min[1] - 5.0).abs() < 1e-3, "height");
    assert!((max[2] - min[2] - 8.0).abs() < 1e-3, "pad length");
    assert!(min[2].abs() < 1e-3, "starts on the sketch plane");
}

/// Regression for the reported "pocket did nothing" bug: a sketch drawn on
/// the TOP FACE of a pad has its normal pointing out of the material; the
/// pocket must cut against that normal (into the pad) by default.
#[test]
fn pocket_feature_cuts_into_the_pad() {
    let (mut doc, body, rect_id) = setup(20.0, 20.0);
    // The hole sketch sits on the pad's top face (z = 6, normal +Z), exactly
    // as produced by clicking the face and choosing "Selected face". The
    // plane's frame starts at the document origin, so the pad's middle is at
    // (10, 10) on it.
    let top_face = wb_sketch::sketch::SketchPlane::from_face([10.0, 10.0, 6.0], [0.0, 0.0, 1.0]);
    let hole_id = doc
        .add_feature_in_body(
            circle_sketch_on(top_face, 10.0, 10.0, 3.0),
            "hole".into(),
            Some(body),
        )
        .unwrap();
    doc.add_feature_in_body(
        pad_feature(rect_id, 6.0, false, false),
        "Pad".into(),
        Some(body),
    )
    .unwrap();
    doc.add_feature_in_body(pocket_feature(hole_id, 6.0), "Pocket".into(), Some(body))
        .unwrap();

    let ops = wb_part::body_build_ops(&doc, body).unwrap().ops;
    assert_eq!(ops.len(), 2);

    let mut kernel = OgeomKernel::new();
    let detail = TessellationSettings::default();

    // Pad only.
    let solid = kernel.execute_solid_chain(&ops[..1], &detail).unwrap();
    // Pad + pocket: same bounds, more triangles (the bore adds a wall).
    let with_hole = kernel.execute_solid_chain(&ops, &detail).unwrap();

    let (a_min, a_max) = mesh_bounds(&solid.mesh);
    let (b_min, b_max) = mesh_bounds(&with_hole.mesh);
    for i in 0..3 {
        assert!((a_min[i] - b_min[i]).abs() < 1e-3);
        assert!((a_max[i] - b_max[i]).abs() < 1e-3);
    }
    assert!(
        with_hole.mesh.indices.len() > solid.mesh.indices.len(),
        "through-hole adds bore triangles ({} vs {})",
        with_hole.mesh.indices.len(),
        solid.mesh.indices.len()
    );
}

#[test]
fn pad_on_front_plane_extrudes_along_minus_y() {
    let mut doc = Document::new("t");
    let body = doc.create_body(Some("Body".into()));
    // Front (XZ) plane: sketch x → world X, sketch y → world Z, normal -Y.
    let sketch_id = doc
        .add_feature_in_body(
            rect_sketch_on(wb_sketch::sketch::SketchPlane::xz(), 10.0, 4.0),
            "front".into(),
            Some(body),
        )
        .unwrap();
    doc.add_feature_in_body(
        pad_feature(sketch_id, 6.0, false, false),
        "Pad".into(),
        Some(body),
    )
    .unwrap();

    let ops = wb_part::body_build_ops(&doc, body).unwrap().ops;
    let mut kernel = OgeomKernel::new();
    let result = kernel
        .execute_solid_chain(&ops, &TessellationSettings::default())
        .unwrap();
    let (min, max) = mesh_bounds(&result.mesh);
    assert!((max[0] - min[0] - 10.0).abs() < 1e-3, "world X = sketch x");
    assert!((max[2] - min[2] - 4.0).abs() < 1e-3, "world Z = sketch y");
    assert!((max[1] - min[1] - 6.0).abs() < 1e-3, "extruded along Y");
    assert!(max[1].abs() < 1e-3, "normal is -Y: solid at negative Y");
}

#[test]
fn editing_the_pad_length_changes_the_solid() {
    let (mut doc, body, sketch_id) = setup(10.0, 5.0);
    let pad_id = doc
        .add_feature_in_body(
            pad_feature(sketch_id, 8.0, false, false),
            "Pad".into(),
            Some(body),
        )
        .unwrap();

    // Simulate the panel edit: update data, mark dirty, rebuild.
    use core_document::WorkbenchFeature;
    doc.update_feature_data(pad_id, pad_feature(sketch_id, 3.0, true, false).to_json())
        .unwrap();
    doc.mark_feature_dirty(pad_id);
    assert_eq!(wb_part::pending_body_rebuilds(&doc), vec![body]);

    let ops = wb_part::body_build_ops(&doc, body).unwrap().ops;
    let mut kernel = OgeomKernel::new();
    let result = kernel
        .execute_solid_chain(&ops, &TessellationSettings::default())
        .unwrap();
    let (min, max) = mesh_bounds(&result.mesh);
    assert!((max[2] - min[2] - 3.0).abs() < 1e-3, "new length");
    assert!(max[2].abs() < 1e-3, "reversed: solid below the plane");
}

#[test]
fn revolution_feature_builds_a_ring_through_the_full_stack() {
    let mut doc = Document::new("t");
    let body = doc.create_body(Some("Body".into()));
    // Rectangle x ∈ [5, 8], y ∈ [0, 2]: revolving about the sketch Y axis
    // sweeps a ring of outer radius 8.
    let mut sketch = Sketch::new("ring");
    let a = sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(5.0, 0.0))));
    let b = sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(8.0, 0.0))));
    let c = sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(8.0, 2.0))));
    let d = sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(5.0, 2.0))));
    for (s, e) in [(a, b), (b, c), (c, d), (d, a)] {
        sketch.add_geometry(GeometryElement::Line(Line::new(s, e)));
    }
    let plane = sketch.plane;
    let sketch_id = doc
        .add_feature_in_body(SketchFeature::new(sketch, plane), "ring".into(), Some(body))
        .unwrap();
    doc.add_feature_in_body(
        wb_part::PartFeature::Revolution {
            refine: false,
            sketch: sketch_id,
            angle_deg: 360.0,
            axis: wb_part::RevolveAxis::SketchY,
            reversed: false,
            midplane: false,
            second_angle_deg: None,
            mode: Default::default(),
            up_to_face: None,
        },
        "Revolution".into(),
        Some(body),
    )
    .unwrap();

    let ops = wb_part::body_build_ops(&doc, body).unwrap().ops;
    let mut kernel = OgeomKernel::new();
    let result = kernel
        .execute_solid_chain(&ops, &TessellationSettings::default())
        .unwrap();
    let (min, max) = mesh_bounds(&result.mesh);
    // XY sketch plane, revolve about its y axis (world Y through origin):
    // the swept ring spans ±8 in world X and Z, height 2 in world Y.
    assert!(
        (max[0] - 8.0).abs() < 0.1 && (min[0] + 8.0).abs() < 0.1,
        "x span"
    );
    assert!(
        (max[2] - 8.0).abs() < 0.1 && (min[2] + 8.0).abs() < 0.1,
        "z span"
    );
    assert!((max[1] - min[1] - 2.0).abs() < 0.1, "height");
}

#[test]
fn fillet_feature_rounds_the_pad_through_the_full_stack() {
    let (mut doc, body, sketch_id) = setup(20.0, 20.0);
    doc.add_feature_in_body(
        pad_feature(sketch_id, 10.0, false, false),
        "Pad".into(),
        Some(body),
    )
    .unwrap();

    let mut kernel = OgeomKernel::new();
    let detail = TessellationSettings::default();
    let plain = kernel
        .execute_solid_chain(&wb_part::body_build_ops(&doc, body).unwrap().ops, &detail)
        .unwrap();

    doc.add_feature_in_body(
        PartFeature::Fillet {
            radius: 2.0,
            edges: wb_part::EdgeSel::All,
            follow_tangent: false,
        },
        "Fillet".into(),
        Some(body),
    )
    .unwrap();
    let filleted = kernel
        .execute_solid_chain(&wb_part::body_build_ops(&doc, body).unwrap().ops, &detail)
        .unwrap();
    assert!(
        filleted.mesh.indices.len() > plain.mesh.indices.len(),
        "fillets add curved faces"
    );
}

#[test]
fn hole_feature_drills_the_pad_through_the_full_stack() {
    let (mut doc, body, rect_id) = setup(30.0, 20.0);
    doc.add_feature_in_body(
        pad_feature(rect_id, 6.0, false, false),
        "Pad".into(),
        Some(body),
    )
    .unwrap();

    // Two hole positions on the pad's top face, either side of its middle
    // (the plane's frame starts at the document origin).
    let top_face = wb_sketch::sketch::SketchPlane::from_face([15.0, 10.0, 6.0], [0.0, 0.0, 1.0]);
    let mut holes = Sketch::new("holes");
    holes.plane = top_face;
    for x in [7.0f32, 23.0] {
        let center = holes.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(x, 10.0))));
        holes.add_geometry(GeometryElement::Circle(Circle::new(center, 1.0)));
    }
    let holes_id = doc
        .add_feature_in_body(
            SketchFeature::new(holes, top_face),
            "holes".into(),
            Some(body),
        )
        .unwrap();
    doc.add_feature_in_body(
        PartFeature::Hole {
            refine: false,
            sketch: holes_id,
            diameter: 4.0,
            depth: 3.0,
            through_all: true,
            cut: wb_part::HoleCut::None,
            thread: None,
            threaded: false,
            modeled_thread: false,
            thread_depth: 0.0,
            fit: wb_part::HoleFit::Normal,
            drill_point: wb_part::DrillPoint::Flat,
            point_in_depth: false,
            taper_deg: 0.0,
            reversed: false,
        },
        "Hole".into(),
        Some(body),
    )
    .unwrap();

    let mut kernel = OgeomKernel::new();
    let plan = wb_part::body_build_ops(&doc, body).unwrap();
    let result = kernel
        .execute_solid_chain(&plan.ops, &TessellationSettings::default())
        .unwrap();
    let (min, max) = mesh_bounds(&result.mesh);
    assert!((max[0] - min[0] - 30.0).abs() < 1e-3, "plate width kept");
    // The two through-bores add interior walls: more than the 12 box tris.
    assert!(result.mesh.indices.len() / 3 > 12);
}

#[test]
fn linear_pattern_feature_repeats_a_boss_through_the_full_stack() {
    let (mut doc, body, plate_id) = setup(60.0, 20.0);
    doc.add_feature_in_body(
        pad_feature(plate_id, 4.0, false, false),
        "Pad".into(),
        Some(body),
    )
    .unwrap();

    let mut boss = Sketch::new("boss");
    let center = boss.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(10.0, 10.0))));
    boss.add_geometry(GeometryElement::Circle(Circle::new(center, 3.0)));
    let boss_plane = boss.plane;
    let boss_id = doc
        .add_feature_in_body(
            SketchFeature::new(boss, boss_plane),
            "boss".into(),
            Some(body),
        )
        .unwrap();
    let boss_pad = doc
        .add_feature_in_body(
            pad_feature(boss_id, 12.0, false, false),
            "Boss".into(),
            Some(body),
        )
        .unwrap();
    doc.add_feature_in_body(
        PartFeature::LinearPattern {
            refine: false,
            originals: vec![boss_pad],
            axis: wb_part::PatternAxis::X,
            length: 40.0,
            occurrences: 3,
            spacing_mode: false,
            spacings: Vec::new(),
            reversed: false,
        },
        "Pattern".into(),
        Some(body),
    )
    .unwrap();

    let mut kernel = OgeomKernel::new();
    let plan = wb_part::body_build_ops(&doc, body).unwrap();
    assert_eq!(plan.ops.len(), 3);
    let result = kernel
        .execute_solid_chain(&plan.ops, &TessellationSettings::default())
        .unwrap();
    let (min, max) = mesh_bounds(&result.mesh);
    // Bosses at x = 10, 30, 50, all inside the 60-wide plate.
    assert!((max[0] - min[0] - 60.0).abs() < 1e-3, "plate width kept");
    assert!((max[2] - 12.0).abs() < 1e-3, "boss height everywhere");
}

#[test]
fn symmetric_pad_straddles_the_sketch_plane() {
    let (mut doc, body, sketch_id) = setup(10.0, 5.0);
    doc.add_feature_in_body(
        pad_feature(sketch_id, 8.0, false, true),
        "Pad".into(),
        Some(body),
    )
    .unwrap();
    let ops = wb_part::body_build_ops(&doc, body).unwrap().ops;
    let mut kernel = OgeomKernel::new();
    let result = kernel
        .execute_solid_chain(&ops, &TessellationSettings::default())
        .unwrap();
    let (min, max) = mesh_bounds(&result.mesh);
    assert!(
        (max[2] - 4.0).abs() < 1e-3 && (min[2] + 4.0).abs() < 1e-3,
        "±4 about the plane"
    );
}

/// Whether every face of `mesh` winds outward: its triangles' normals point
/// away from the mesh centroid, as a solid's skin must whichever way the
/// profile it came from was drawn.
fn every_face_winds_outward(mesh: &kernel_api::TriMesh) -> bool {
    let (min, max) = mesh_bounds(mesh);
    let centre = [
        (min[0] + max[0]) / 2.0,
        (min[1] + max[1]) / 2.0,
        (min[2] + max[2]) / 2.0,
    ];
    mesh.indices.chunks(3).all(|tri| {
        let p = |i: u32| mesh.positions[i as usize];
        let (a, b, c) = (p(tri[0]), p(tri[1]), p(tri[2]));
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        let out = [
            (a[0] + b[0] + c[0]) / 3.0 - centre[0],
            (a[1] + b[1] + c[1]) / 3.0 - centre[1],
            (a[2] + b[2] + c[2]) / 3.0 - centre[2],
        ];
        n[0] * out[0] + n[1] * out[1] + n[2] * out[2] > 0.0
    })
}

/// A square drawn clockwise pads to the same solid as one drawn
/// counter-clockwise: every face of its skin faces out.
#[test]
fn a_clockwise_profile_pads_to_an_outward_facing_solid() {
    for clockwise in [false, true] {
        let mut sketch = Sketch::new("s");
        let mut corners = [(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)];
        if clockwise {
            corners.reverse();
        }
        let ids: Vec<_> = corners
            .iter()
            .map(|(x, y)| {
                sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(*x, *y))))
            })
            .collect();
        for i in 0..4 {
            sketch.add_geometry(GeometryElement::Line(Line::new(ids[i], ids[(i + 1) % 4])));
        }
        let plane = sketch.plane;
        let mut doc = Document::new("t");
        let body = doc.create_body(Some("Body".into()));
        let sketch_id = doc
            .add_feature_in_body(
                SketchFeature::new(sketch, plane),
                "sketch".into(),
                Some(body),
            )
            .unwrap();
        doc.add_feature_in_body(
            pad_feature(sketch_id, 10.0, false, false),
            "Pad".into(),
            Some(body),
        )
        .unwrap();

        let ops = wb_part::body_build_ops(&doc, body).unwrap().ops;
        let mesh = OgeomKernel::new()
            .execute_solid_chain(&ops, &TessellationSettings::default())
            .unwrap()
            .mesh;
        assert_eq!(mesh.indices.len() / 3, 12, "clockwise={clockwise}: a box");
        assert!(
            every_face_winds_outward(&mesh),
            "clockwise={clockwise}: a face winds into the solid"
        );
    }
}

/// A second pad stacked flush on the first leaves each side split along
/// the seam; with Refine on it the block comes out with its six faces.
#[test]
fn a_refined_pad_stacked_on_a_pad_leaves_six_faces() {
    let faces_with = |refine: bool| {
        let (mut doc, body, sketch_id) = setup(20.0, 20.0);
        doc.add_feature_in_body(
            pad_feature(sketch_id, 10.0, false, false),
            "Pad".into(),
            Some(body),
        )
        .unwrap();
        let raised = wb_sketch::sketch::SketchPlane {
            origin: [0.0, 0.0, 10.0],
            ..wb_sketch::sketch::SketchPlane::xy()
        };
        let upper = doc
            .add_feature_in_body(
                rect_sketch_on(raised, 20.0, 20.0),
                "upper".into(),
                Some(body),
            )
            .unwrap();
        let mut second = pad_feature(upper, 10.0, false, false);
        second.set_refine(refine);
        doc.add_feature_in_body(second, "Pad001".into(), Some(body))
            .unwrap();
        let ops = wb_part::body_build_ops(&doc, body).unwrap().ops;
        let mesh = OgeomKernel::new()
            .execute_solid_chain(&ops, &TessellationSettings::default())
            .unwrap()
            .mesh;
        mesh.faces
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
    };
    assert!(faces_with(false) > 6, "unrefined, the sides are split");
    assert_eq!(faces_with(true), 6, "refined, one face per side");
}

/// Half an ellipse closed by its major axis pads to the half-elliptic
/// prism: the arc's endpoints and the line's meet exactly in the profile.
#[test]
fn an_arc_of_ellipse_closed_by_a_line_pads_to_its_area() {
    use wb_sketch::sketch::Ellipse;
    let (a, b, height) = (10.0f32, 5.0f32, 4.0f32);
    let mut sketch = Sketch::new("half ellipse");
    let center = sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(0.0, 0.0))));
    let right = sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(a, 0.0))));
    let left = sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(-a, 0.0))));
    sketch.add_geometry(GeometryElement::Ellipse(Ellipse::new_arc(
        center,
        Vec2D::new(a, 0.0),
        b / a,
        right,
        left,
    )));
    sketch.add_geometry(GeometryElement::Line(Line::new(left, right)));
    let plane = sketch.plane;

    let mut doc = Document::new("t");
    let body = doc.create_body(Some("Body".into()));
    let sketch_id = doc
        .add_feature_in_body(
            SketchFeature::new(sketch, plane),
            "sketch".into(),
            Some(body),
        )
        .unwrap();
    doc.add_feature_in_body(
        pad_feature(sketch_id, height, false, false),
        "Pad".into(),
        Some(body),
    )
    .unwrap();
    let ops = wb_part::body_build_ops(&doc, body).unwrap().ops;
    let mut kernel = OgeomKernel::new();
    let result = kernel
        .execute_solid_chain(&ops, &TessellationSettings::default())
        .expect("the half ellipse pads");
    let (lo, hi) = result.bounds_mm.expect("bounds");
    for (got, want) in [
        (lo[0], -a),
        (hi[0], a),
        (lo[1], 0.0),
        (hi[1], b),
        (hi[2] - lo[2], height),
    ] {
        assert!((got - want).abs() < 1e-3, "bounds {lo:?}..{hi:?}");
    }
    // The elliptic wall is a line swept along an ellipse, which the kernel
    // integrates in closed form: the volume is exact, and says so.
    let props = kernel.physical_properties(&result.brep_blob).unwrap();
    let volume = props.volume_mm3.expect("a closed solid");
    let expected = f64::from(std::f32::consts::PI * a * b / 2.0 * height);
    assert!(!props.approximate);
    assert!(
        (volume - expected).abs() < 1e-5 * expected,
        "volume {volume} vs {expected}"
    );
}

/// A block with a bore through it, the bore's top rim rounded: the fillet
/// a printed part's hole mouth takes most often.
#[test]
fn bore_rim_fillets() {
    let (mut doc, body, rect_id) = setup(40.0, 30.0);
    doc.add_feature_in_body(
        pad_feature(rect_id, 12.0, false, false),
        "Pad".into(),
        Some(body),
    )
    .unwrap();
    let top = wb_sketch::sketch::SketchPlane {
        origin: [0.0, 0.0, 12.0],
        ..Default::default()
    };
    let bore = doc
        .add_feature_in_body(
            circle_sketch_on(top, 20.0, 15.0, 6.0),
            "bore".into(),
            Some(body),
        )
        .unwrap();
    doc.add_feature_in_body(
        PartFeature::Pocket {
            refine: false,
            sketch: Some(bore),
            depth: 12.0,
            reversed: false,
            symmetric: false,
            through_all: true,
            mode: wb_part::ExtrudeMode::Dimension,
            depth2: 0.0,
            taper_deg: 0.0,
            up_to_face: None,
            up_to_offset: 0.0,
            profile_face: None,
            direction: Default::default(),
            up_to_shape: Vec::new(),
            mode2: None,
            up_to_face2: None,
            up_to_offset2: 0.0,
            up_to_shape2: Vec::new(),
        },
        "Pocket".into(),
        Some(body),
    )
    .unwrap();
    doc.add_feature_in_body(
        PartFeature::Fillet {
            radius: 2.0,
            edges: wb_part::EdgeSel::Edges(vec![wb_part::EdgePick {
                point: [26.0, 15.0, 12.0],
                direction: [0.0, 1.0, 0.0],
            }]),
            follow_tangent: false,
        },
        "Fillet".into(),
        Some(body),
    )
    .unwrap();

    let mut kernel = OgeomKernel::new();
    let result = kernel
        .execute_solid_chain(
            &wb_part::body_build_ops(&doc, body).unwrap().ops,
            &TessellationSettings::default(),
        )
        .expect("the rim rounds");
    // The fillet takes the ring of area rho^2 (1 - pi/4) off the rim, its
    // centroid rho (10 - 3 pi) / (3 (4 - pi)) out from it (Pappus).
    let (r, rho) = (6.0_f64, 2.0_f64);
    let pi = std::f64::consts::PI;
    let area = rho * rho * (1.0 - pi / 4.0);
    let out = rho * (10.0 - 3.0 * pi) / (3.0 * (4.0 - pi));
    let expected = 40.0 * 30.0 * 12.0 - pi * r * r * 12.0 - 2.0 * pi * (r + out) * area;
    let volume = kernel
        .physical_properties(&result.brep_blob)
        .unwrap()
        .volume_mm3
        .expect("a closed solid");
    assert!(
        (volume - expected).abs() < 1e-3 * expected,
        "volume {volume} vs {expected}"
    );
}

#[test]
fn a_variable_drives_the_pad_and_changing_it_rebuilds_the_solid() {
    use core_document::{DocumentService, Variable, VariableSet, WorkbenchFeature};
    let mut registry = DocumentService::default();
    registry
        .register_workbench(Box::new(wb_part::PartDesignWorkbench::default()))
        .unwrap();
    let (mut doc, body, sketch_id) = setup(10.0, 5.0);
    let sizes = doc
        .add_feature(
            VariableSet {
                variables: vec![
                    Variable {
                        name: "base".into(),
                        formula: "4 mm".into(),
                        comment: String::new(),
                    },
                    Variable {
                        name: "height".into(),
                        formula: "Sizes.base * 2 - 1 mm".into(),
                        comment: String::new(),
                    },
                ],
            },
            "Sizes".into(),
        )
        .unwrap();
    let pad_id = doc
        .add_feature_in_body(
            pad_feature(sketch_id, 8.0, false, false),
            "Pad".into(),
            Some(body),
        )
        .unwrap();
    doc.set_feature_formula(pad_id, "/Pad/length", Some("Sizes.height".into()))
        .unwrap();

    let height = |doc: &mut Document| {
        let jobs = registry.rebuild_jobs(doc);
        let job = jobs
            .into_iter()
            .find(|j| j.body == body)
            .expect("a rebuild");
        let ops = job.plan.unwrap().ops;
        let result = OgeomKernel::new()
            .execute_solid_chain(&ops, &TessellationSettings::default())
            .unwrap();
        let (min, max) = mesh_bounds(&result.mesh);
        max[2] - min[2]
    };
    assert!((height(&mut doc) - 7.0).abs() < 1e-3, "2 * 4 - 1");

    // Change the variable: the pad rebuilds at the new height, and nothing
    // else asked for it.
    let mut set = VariableSet::from_json(doc.get_feature_data(sizes).unwrap()).unwrap();
    set.variables[0].formula = "6 mm".into();
    doc.update_feature_data(sizes, set.to_json()).unwrap();
    assert!((height(&mut doc) - 11.0).abs() < 1e-3, "2 * 6 - 1");
    assert!(registry.rebuild_jobs(&mut doc).is_empty(), "settled");
}

#[test]
fn a_variable_drives_a_named_sketch_dimension_and_the_pad_on_it() {
    use core_document::{DocumentService, Variable, VariableSet, WorkbenchFeature};
    use wb_sketch::sketch::{Constraint, ConstraintKind};
    let mut registry = DocumentService::default();
    registry
        .register_workbench(Box::new(wb_sketch::SketchWorkbench::default()))
        .unwrap();
    registry
        .register_workbench(Box::new(wb_part::PartDesignWorkbench::default()))
        .unwrap();

    // A 10 x 5 rectangle, fixed at the origin, square, its bottom named
    // `width`.
    let mut sketch = Sketch::new("s");
    let at = |x, y| GeometryElement::Point(Point::new(Vec2D::new(x, y)));
    let a = sketch.add_geometry(at(0.0, 0.0));
    let b = sketch.add_geometry(at(10.0, 0.0));
    let c = sketch.add_geometry(at(10.0, 5.0));
    let d = sketch.add_geometry(at(0.0, 5.0));
    let lines: Vec<_> = [(a, b), (b, c), (c, d), (d, a)]
        .into_iter()
        .map(|(s, e)| sketch.add_geometry(GeometryElement::Line(Line::new(s, e))))
        .collect();
    let mut add = |kind| sketch.constraints.push(Constraint::new(kind));
    add(ConstraintKind::FixedPoint {
        point: a,
        position: Vec2D::new(0.0, 0.0),
    });
    add(ConstraintKind::Horizontal { element: lines[0] });
    add(ConstraintKind::Vertical { element: lines[1] });
    add(ConstraintKind::Horizontal { element: lines[2] });
    add(ConstraintKind::Vertical { element: lines[3] });
    add(ConstraintKind::Length {
        line: lines[1],
        length: 5.0,
    });
    let mut width = Constraint::new(ConstraintKind::Length {
        line: lines[0],
        length: 10.0,
    });
    width.name = Some("width".into());
    let width_key = width.id.to_string();
    sketch.constraints.push(width);
    let plane = sketch.plane;

    let mut doc = Document::new("t");
    let body = doc.create_body(Some("Body".into()));
    let sizes = doc
        .add_feature(
            VariableSet {
                variables: vec![Variable {
                    name: "w".into(),
                    formula: "24 mm".into(),
                    comment: String::new(),
                }],
            },
            "Sizes".into(),
        )
        .unwrap();
    let sketch_id = doc
        .add_feature_in_body(
            SketchFeature::new(sketch, plane),
            "Profile".into(),
            Some(body),
        )
        .unwrap();
    doc.set_feature_formula(sketch_id, &width_key, Some("Sizes.w".into()))
        .unwrap();
    let pad_id = doc
        .add_feature_in_body(
            pad_feature(sketch_id, 3.0, false, false),
            "Pad".into(),
            Some(body),
        )
        .unwrap();
    // The pad reads the sketch's named dimension too.
    doc.set_feature_formula(pad_id, "/Pad/length", Some("Profile.width / 4".into()))
        .unwrap();

    let size = |doc: &mut Document| {
        let job = registry
            .rebuild_jobs(doc)
            .into_iter()
            .find(|j| j.body == body)
            .expect("a rebuild");
        let result = OgeomKernel::new()
            .execute_solid_chain(&job.plan.unwrap().ops, &TessellationSettings::default())
            .unwrap();
        let (min, max) = mesh_bounds(&result.mesh);
        [max[0] - min[0], max[1] - min[1], max[2] - min[2]]
    };
    let [x, y, z] = size(&mut doc);
    assert!(
        (x - 24.0).abs() < 1e-3,
        "the sketch solved to the variable: {x}"
    );
    assert!((y - 5.0).abs() < 1e-3, "{y}");
    assert!((z - 6.0).abs() < 1e-3, "the pad read Profile.width: {z}");

    let mut set = VariableSet::from_json(doc.get_feature_data(sizes).unwrap()).unwrap();
    set.variables[0].formula = "16 mm".into();
    doc.update_feature_data(sizes, set.to_json()).unwrap();
    let [x, _, z] = size(&mut doc);
    assert!((x - 16.0).abs() < 1e-3, "{x}");
    assert!((z - 4.0).abs() < 1e-3, "{z}");
}

/// An M6 hole with its thread modeled: the thread's groove is cut into the
/// tap-drilled wall, out toward the M6 major diameter, a closed solid.
#[test]
fn a_modeled_thread_cuts_its_groove_into_the_hole_wall() {
    let (mut doc, body, rect_id) = setup(20.0, 20.0);
    doc.add_feature_in_body(
        pad_feature(rect_id, 10.0, false, false),
        "Pad".into(),
        Some(body),
    )
    .unwrap();
    let top_face = wb_sketch::sketch::SketchPlane::from_face([10.0, 10.0, 10.0], [0.0, 0.0, 1.0]);
    let mut holes = Sketch::new("holes");
    holes.plane = top_face;
    holes.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(10.0, 10.0))));
    let holes_id = doc
        .add_feature_in_body(
            SketchFeature::new(holes, top_face),
            "holes".into(),
            Some(body),
        )
        .unwrap();
    let hole = |modeled_thread: bool| PartFeature::Hole {
        refine: false,
        sketch: holes_id,
        diameter: 5.0,
        depth: 8.0,
        through_all: false,
        cut: wb_part::HoleCut::None,
        thread: Some(wb_part::ThreadSpec::new(
            wb_part::ThreadStandard::IsoMetricCoarse,
            "M6",
        )),
        threaded: true,
        modeled_thread,
        thread_depth: 6.0,
        fit: wb_part::HoleFit::Normal,
        drill_point: wb_part::DrillPoint::Flat,
        point_in_depth: false,
        taper_deg: 0.0,
        reversed: false,
    };
    let hole_id = doc
        .add_feature_in_body(hole(false), "Hole".into(), Some(body))
        .unwrap();
    let mut kernel = OgeomKernel::new();
    let mut volume = |doc: &Document| {
        let plan = wb_part::body_build_ops(doc, body).unwrap();
        let result = kernel
            .execute_solid_chain(&plan.ops, &TessellationSettings::default())
            .unwrap_or_else(|e| panic!("builds: {e}"));
        kernel
            .physical_properties(&result.brep_blob)
            .expect("measures")
            .volume_mm3
            .expect("a volume")
    };
    let tapped = volume(&doc);
    let block = 20.0 * 20.0 * 10.0;
    let drill = std::f64::consts::PI * 2.5 * 2.5 * 8.0;
    assert!((tapped - (block - drill)).abs() < 0.01, "{tapped}");
    doc.update_feature_data(hole_id, serde_json::to_value(hole(true)).unwrap())
        .unwrap();
    let threaded = volume(&doc);
    let major = std::f64::consts::PI * 3.0 * 3.0 * 8.0;
    assert!(
        threaded < tapped - 5.0 && threaded > block - major,
        "the groove takes some of the wall, not all of it: {threaded} (tapped {tapped})"
    );
}

/// A box off the origin mirrored across each base plane and across one of
/// its own faces: each copy lands on the far side of that plane.
#[test]
fn mirrored_copies_the_pad_across_every_plane_it_is_given() {
    use wb_part::{FacePick, MirrorPlane};
    let cases: [(MirrorPlane, [f32; 3], [f32; 3]); 4] = [
        (MirrorPlane::XY, [0.0, 0.0, -8.0], [10.0, 5.0, 8.0]),
        (MirrorPlane::XZ, [0.0, -5.0, 0.0], [10.0, 5.0, 8.0]),
        (MirrorPlane::YZ, [-10.0, 0.0, 0.0], [10.0, 5.0, 8.0]),
        (
            MirrorPlane::Face(FacePick {
                point: [10.0, 2.5, 4.0],
                normal: [1.0, 0.0, 0.0],
            }),
            [0.0, 0.0, 0.0],
            [20.0, 5.0, 8.0],
        ),
    ];
    let mut failures = Vec::new();
    for (plane, want_min, want_max) in cases {
        let (mut doc, body, sketch_id) = setup(10.0, 5.0);
        let pad = doc
            .add_feature_in_body(
                pad_feature(sketch_id, 8.0, false, false),
                "Pad".into(),
                Some(body),
            )
            .unwrap();
        doc.add_feature_in_body(
            PartFeature::Mirrored {
                refine: false,
                originals: vec![pad],
                plane,
            },
            "Mirror".into(),
            Some(body),
        )
        .unwrap();
        let plan = wb_part::body_build_ops(&doc, body).unwrap();
        let result =
            OgeomKernel::new().execute_solid_chain(&plan.ops, &TessellationSettings::default());
        let result = match result {
            Ok(result) => result,
            Err(e) => {
                failures.push(format!("{plane:?}: {e}"));
                continue;
            }
        };
        let (min, max) = mesh_bounds(&result.mesh);
        let fits = (0..3)
            .all(|i| (min[i] - want_min[i]).abs() < 1e-3 && (max[i] - want_max[i]).abs() < 1e-3);
        if !fits {
            failures.push(format!(
                "{plane:?}: bounds {min:?}..{max:?}, want {want_min:?}..{want_max:?}"
            ));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// A quarter turn is not symmetric, so its mirror image shows whether the
/// copy turned the right way: across each base plane it must be the
/// original's reflection, filling the bounds the reflection fills.
#[test]
fn a_mirrored_revolution_turns_the_way_the_mirror_puts_it() {
    use wb_part::MirrorPlane;
    let build = |mirror: Option<MirrorPlane>| {
        let mut doc = Document::new("t");
        let body = doc.create_body(Some("Body".into()));
        let mut sketch = Sketch::new("ring");
        let a = sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(5.0, 0.0))));
        let b = sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(8.0, 0.0))));
        let c = sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(8.0, 2.0))));
        let d = sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(5.0, 2.0))));
        for (s, e) in [(a, b), (b, c), (c, d), (d, a)] {
            sketch.add_geometry(GeometryElement::Line(Line::new(s, e)));
        }
        let plane = sketch.plane;
        let sketch_id = doc
            .add_feature_in_body(SketchFeature::new(sketch, plane), "ring".into(), Some(body))
            .unwrap();
        let turn = doc
            .add_feature_in_body(
                PartFeature::Revolution {
                    refine: false,
                    sketch: sketch_id,
                    angle_deg: 90.0,
                    axis: wb_part::RevolveAxis::SketchY,
                    reversed: false,
                    midplane: false,
                    second_angle_deg: None,
                    mode: Default::default(),
                    up_to_face: None,
                },
                "Revolution".into(),
                Some(body),
            )
            .unwrap();
        if let Some(plane) = mirror {
            doc.add_feature_in_body(
                PartFeature::Mirrored {
                    refine: false,
                    originals: vec![turn],
                    plane,
                },
                "Mirror".into(),
                Some(body),
            )
            .unwrap();
        }
        let ops = wb_part::body_build_ops(&doc, body).unwrap().ops;
        let result = OgeomKernel::new()
            .execute_solid_chain(&ops, &TessellationSettings::default())
            .unwrap();
        mesh_bounds(&result.mesh)
    };
    let (min, max) = build(None);
    for (plane, axis) in [
        (MirrorPlane::YZ, 0),
        (MirrorPlane::XZ, 1),
        (MirrorPlane::XY, 2),
    ] {
        let (got_min, got_max) = build(Some(plane));
        for i in 0..3 {
            let (want_lo, want_hi) = if i == axis {
                (min[i].min(-max[i]), max[i].max(-min[i]))
            } else {
                (min[i], max[i])
            };
            assert!(
                (got_min[i] - want_lo).abs() < 0.1 && (got_max[i] - want_hi).abs() < 0.1,
                "{plane:?} axis {i}: {:?}..{:?}, want {want_lo}..{want_hi} (original {min:?}..{max:?})",
                got_min[i],
                got_max[i]
            );
        }
    }
}

/// A sketch drawn on a datum plane follows it: the pad on the sketch moves
/// when the datum is moved, turned or flipped, and settles once rebuilt.
#[test]
fn a_pad_on_a_datum_sketch_follows_the_datum() {
    use core_document::{
        AttachmentOffset, BasePlane, DatumAttachment, DatumFeature, DatumShape, DocumentService,
        WorkbenchFeature,
    };
    let mut registry = DocumentService::default();
    registry
        .register_workbench(Box::new(wb_sketch::SketchWorkbench::default()))
        .unwrap();
    registry
        .register_workbench(Box::new(wb_part::PartDesignWorkbench::default()))
        .unwrap();
    let mut doc = Document::new("t");
    let body = doc.create_body(Some("Body".into()));
    let datum_at = |z: f32, rotation_deg: f32, flip: bool| DatumFeature {
        shape: DatumShape::Plane { size: 20.0 },
        attachment: DatumAttachment::BasePlane(BasePlane::XY),
        offset: AttachmentOffset {
            translation: [0.0, 0.0, z],
            rotation_deg,
            flip,
        },
    };
    let datum = doc
        .add_feature_in_body(datum_at(10.0, 0.0, false), "Datum".into(), Some(body))
        .unwrap();
    // A 4 x 2 rectangle drawn on the datum, as `sketch.new{on = datum}` makes it.
    let mut sketch = rect_sketch(4.0, 2.0);
    sketch.support = Some(wb_sketch::DatumSupport {
        datum,
        plane: None,
        offset: 0.0,
    });
    let sketch_id = doc
        .add_feature_in_body(sketch, "sketch".into(), Some(body))
        .unwrap();
    doc.add_feature_in_body(
        pad_feature(sketch_id, 3.0, false, false),
        "Pad".into(),
        Some(body),
    )
    .unwrap();

    let bounds = |doc: &mut Document| {
        let job = registry
            .rebuild_jobs(doc)
            .into_iter()
            .find(|j| j.body == body)
            .expect("a rebuild");
        let result = OgeomKernel::new()
            .execute_solid_chain(&job.plan.unwrap().ops, &TessellationSettings::default())
            .unwrap();
        mesh_bounds(&result.mesh)
    };
    let near = |a: [f32; 3], b: [f32; 3]| (0..3).all(|i| (a[i] - b[i]).abs() < 1e-3);

    let (min, max) = bounds(&mut doc);
    assert!(
        near(min, [0.0, 0.0, 10.0]) && near(max, [4.0, 2.0, 13.0]),
        "{min:?}..{max:?}"
    );

    // Moved up: the pad goes with it.
    doc.update_feature_data(datum, datum_at(20.0, 0.0, false).to_json())
        .unwrap();
    let (min, max) = bounds(&mut doc);
    assert!(
        near(min, [0.0, 0.0, 20.0]) && near(max, [4.0, 2.0, 23.0]),
        "moved: {min:?}..{max:?}"
    );

    // Turned a quarter about its normal: the rectangle stands along Y.
    doc.update_feature_data(datum, datum_at(20.0, 90.0, false).to_json())
        .unwrap();
    let (min, max) = bounds(&mut doc);
    assert!(
        near(min, [-2.0, 0.0, 20.0]) && near(max, [0.0, 4.0, 23.0]),
        "turned: {min:?}..{max:?}"
    );

    // Flipped: the pad grows down from the datum.
    doc.update_feature_data(datum, datum_at(20.0, 0.0, true).to_json())
        .unwrap();
    let (min, max) = bounds(&mut doc);
    assert!(
        (max[2] - 20.0).abs() < 1e-3 && (min[2] - 17.0).abs() < 1e-3,
        "flipped: {min:?}..{max:?}"
    );
    assert!(registry.rebuild_jobs(&mut doc).is_empty(), "settled");
}

/// A symmetric pocket from the top face cuts half its depth into the
/// material and half into the air above it.
#[test]
fn a_symmetric_pocket_cuts_half_its_depth_each_way() {
    let removed = |symmetric: bool| {
        let (mut doc, body, rect_id) = setup(20.0, 20.0);
        doc.add_feature_in_body(
            pad_feature(rect_id, 10.0, false, false),
            "Pad".into(),
            Some(body),
        )
        .unwrap();
        let top = wb_sketch::sketch::SketchPlane {
            origin: [5.0, 5.0, 10.0],
            ..Default::default()
        };
        let hole = doc
            .add_feature_in_body(rect_sketch_on(top, 5.0, 5.0), "top".into(), Some(body))
            .unwrap();
        doc.add_feature_in_body(
            PartFeature::Pocket {
                refine: false,
                sketch: Some(hole),
                depth: 4.0,
                reversed: false,
                symmetric,
                through_all: false,
                mode: wb_part::ExtrudeMode::Dimension,
                depth2: 0.0,
                taper_deg: 0.0,
                up_to_face: None,
                up_to_offset: 0.0,
                profile_face: None,
                direction: Default::default(),
                up_to_shape: Vec::new(),
                mode2: None,
                up_to_face2: None,
                up_to_offset2: 0.0,
                up_to_shape2: Vec::new(),
            },
            "Pocket".into(),
            Some(body),
        )
        .unwrap();
        let mut kernel = OgeomKernel::new();
        let result = kernel
            .execute_solid_chain(
                &wb_part::body_build_ops(&doc, body).unwrap().ops,
                &TessellationSettings::default(),
            )
            .unwrap_or_else(|e| panic!("symmetric {symmetric}: {e:?}"));
        let volume = kernel
            .physical_properties(&result.brep_blob)
            .unwrap()
            .volume_mm3
            .expect("a closed solid measures");
        20.0 * 20.0 * 10.0 - volume
    };
    assert!((removed(false) - 100.0).abs() < 1e-6, "{}", removed(false));
    assert!((removed(true) - 50.0).abs() < 1e-6, "{}", removed(true));
}

/// Rebuild every body the way the host does, until nothing is left to
/// rebuild: plan, build, store each solid.
fn settle(doc: &mut Document, kernel: &mut OgeomKernel) {
    for _ in 0..16 {
        let jobs = wb_part::rebuild_jobs(doc);
        if jobs.is_empty() {
            return;
        }
        for job in jobs {
            let Ok(plan) = job.plan else { continue };
            if plan.ops.is_empty() {
                continue;
            }
            if let Ok(result) =
                kernel.execute_solid_chain(&plan.ops, &TessellationSettings::default())
            {
                doc.set_imported_brep_data(job.body, result.brep_blob, Vec::new());
                doc.set_imported_geometry(
                    job.body,
                    core_document::ImportedGeometry {
                        mesh: std::sync::Arc::new(result.mesh),
                        source_asset: None,
                        revision: 0,
                        bounds_mm: result.bounds_mm,
                        brep_blob_path: None,
                        face_colors_path: None,
                        health: None,
                    },
                );
            }
        }
    }
    panic!("the rebuilds never settled");
}

fn volume_of_body(doc: &Document, kernel: &mut OgeomKernel, body: BodyId) -> f64 {
    let blob = doc.imported_brep_blob_arc(body).expect("a solid");
    kernel
        .physical_properties(&blob)
        .unwrap()
        .volume_mm3
        .expect("a closed solid")
}

/// A Boolean follows its tool body: made before the tool is built, it
/// waits for it, and a change to the tool rebuilds it.
#[test]
fn a_boolean_follows_its_tool_body() {
    let (mut doc, target, base) = setup(20.0, 20.0);
    doc.add_feature_in_body(
        pad_feature(base, 10.0, false, false),
        "Pad".into(),
        Some(target),
    )
    .unwrap();
    let tool = doc.create_body(Some("Tool".into()));
    doc.add_feature_in_body(
        PartFeature::BodyBoolean {
            refine: false,
            tool_body: tool,
            kind: kernel_api::BoolKind::Cut,
        },
        "Boolean".into(),
        Some(target),
    )
    .unwrap();
    let circle = doc
        .add_feature_in_body(
            circle_sketch_on(wb_sketch::sketch::SketchPlane::default(), 10.0, 10.0, 5.0),
            "circle".into(),
            Some(tool),
        )
        .unwrap();
    let cylinder = doc
        .add_feature_in_body(
            pad_feature(circle, 10.0, false, false),
            "Cylinder".into(),
            Some(tool),
        )
        .unwrap();

    // As a document opens: everything to build.
    wb_part::mark_all_part_features_dirty(&mut doc);
    let mut kernel = OgeomKernel::new();
    settle(&mut doc, &mut kernel);
    let pi = std::f64::consts::PI;
    let expect = |depth: f64| 4000.0 - pi * 25.0 * depth;
    let got = volume_of_body(&doc, &mut kernel, target);
    assert!((got - expect(10.0)).abs() < 1e-3, "{got}");
    assert!(
        doc.feature_tree()
            .all_nodes()
            .all(|(_, n)| n.error.is_none()),
        "nothing failed on the way"
    );

    doc.update_feature_data(
        cylinder,
        core_document::WorkbenchFeature::to_json(&pad_feature(circle, 5.0, false, false)),
    )
    .unwrap();
    doc.mark_feature_dirty(cylinder);
    settle(&mut doc, &mut kernel);
    let got = volume_of_body(&doc, &mut kernel, target);
    assert!((got - expect(5.0)).abs() < 1e-3, "{got}");
}

/// Two bodies that take each other as tools can never be built: the
/// Boolean says so rather than rebuilding them in turn forever.
#[test]
fn bodies_that_take_each_other_as_tools_fail_once() {
    let (mut doc, a, base) = setup(20.0, 20.0);
    doc.add_feature_in_body(pad_feature(base, 10.0, false, false), "Pad".into(), Some(a))
        .unwrap();
    let b = doc.create_body(Some("B".into()));
    let circle = doc
        .add_feature_in_body(
            circle_sketch_on(wb_sketch::sketch::SketchPlane::default(), 10.0, 10.0, 5.0),
            "circle".into(),
            Some(b),
        )
        .unwrap();
    doc.add_feature_in_body(
        pad_feature(circle, 10.0, false, false),
        "Cylinder".into(),
        Some(b),
    )
    .unwrap();
    for (body, tool) in [(a, b), (b, a)] {
        doc.add_feature_in_body(
            PartFeature::BodyBoolean {
                refine: false,
                tool_body: tool,
                kind: kernel_api::BoolKind::Fuse,
            },
            "Boolean".into(),
            Some(body),
        )
        .unwrap();
    }
    wb_part::mark_all_part_features_dirty(&mut doc);
    let mut kernel = OgeomKernel::new();
    settle(&mut doc, &mut kernel);
    let error = wb_part::body_build_ops(&doc, a).unwrap_err();
    assert!(
        error.message.contains("as a tool in turn"),
        "{}",
        error.message
    );
}

/// A quartic drawn through points and closed by a line pads: the solid
/// reaches every point the curve was drawn through, and no further along x.
#[test]
fn a_spline_through_points_closed_by_a_line_pads() {
    use wb_sketch::sketch::BSpline;
    let clicks = [[0.0, 0.0], [4.0, 6.0], [9.0, 7.0], [14.0, 3.0], [18.0, 0.0]];
    let fit = wb_sketch::spline::interpolate(&clicks, 4, false).expect("a fit");
    let mut sketch = Sketch::new("spline");
    let fit_points: Vec<_> = clicks
        .iter()
        .map(|p| {
            sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(
                p[0] as f32,
                p[1] as f32,
            ))))
        })
        .collect();
    let control: Vec<_> = fit
        .control
        .iter()
        .enumerate()
        .map(|(i, p)| match i {
            0 => fit_points[0],
            4 => fit_points[4],
            _ => sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(
                p[0] as f32,
                p[1] as f32,
            )))),
        })
        .collect();
    sketch.add_geometry(GeometryElement::BSpline(BSpline {
        degree: fit.degree,
        knots: fit.knots.clone(),
        fit_points: fit_points.clone(),
        fit_params: fit.params.clone(),
        ..BSpline::new(control, false)
    }));
    sketch.add_geometry(GeometryElement::Line(Line::new(
        fit_points[4],
        fit_points[0],
    )));
    let plane = sketch.plane;

    let mut doc = Document::new("t");
    let body = doc.create_body(Some("Body".into()));
    let sketch_id = doc
        .add_feature_in_body(
            SketchFeature::new(sketch, plane),
            "sketch".into(),
            Some(body),
        )
        .unwrap();
    doc.add_feature_in_body(
        pad_feature(sketch_id, 3.0, false, false),
        "Pad".into(),
        Some(body),
    )
    .unwrap();
    let ops = wb_part::body_build_ops(&doc, body).unwrap().ops;
    let mut kernel = OgeomKernel::new();
    let result = kernel
        .execute_solid_chain(&ops, &TessellationSettings::default())
        .expect("the spline pads");
    let (lo, hi) = mesh_bounds(&result.mesh);
    assert!(
        lo[0].abs() < 1e-3 && (hi[0] - 18.0).abs() < 1e-2,
        "{lo:?}..{hi:?}"
    );
    assert!(hi[1] >= 7.0 - 1e-2 && hi[1] < 8.0, "reaches (9, 7): {hi:?}");
    assert!((hi[2] - lo[2] - 3.0).abs() < 1e-3);
}

/// A sketch of one conic arc from `start` to `end`, closed by a line, padded
/// `height`: the solid's volume.
fn padded_conic_volume(
    kind: wb_sketch::sketch::ConicKind,
    center: Vec2D,
    axis: Vec2D,
    minor: f32,
    start: Vec2D,
    end: Vec2D,
    height: f32,
) -> f64 {
    use wb_sketch::sketch::Conic;
    let mut sketch = Sketch::new("conic");
    let c = sketch.add_geometry(GeometryElement::Point(Point::new(center)));
    let s = sketch.add_geometry(GeometryElement::Point(Point::new(start)));
    let e = sketch.add_geometry(GeometryElement::Point(Point::new(end)));
    sketch.add_geometry(GeometryElement::Conic(Conic::new(
        kind, c, axis, minor, s, e,
    )));
    sketch.add_geometry(GeometryElement::Line(Line::new(e, s)));
    let plane = sketch.plane;
    let mut doc = Document::new("t");
    let body = doc.create_body(Some("Body".into()));
    let sketch_id = doc
        .add_feature_in_body(
            SketchFeature::new(sketch, plane),
            "sketch".into(),
            Some(body),
        )
        .unwrap();
    doc.add_feature_in_body(
        pad_feature(sketch_id, height, false, false),
        "Pad".into(),
        Some(body),
    )
    .unwrap();
    let ops = wb_part::body_build_ops(&doc, body).unwrap().ops;
    let mut kernel = OgeomKernel::new();
    let result = kernel
        .execute_solid_chain(&ops, &TessellationSettings::default())
        .expect("the conic pads");
    kernel
        .physical_properties(&result.brep_blob)
        .unwrap()
        .volume_mm3
        .expect("a closed solid")
}

/// A parabolic segment is two thirds of the rectangle round it, and a
/// hyperbolic one what its integral says: the arcs pad exactly.
#[test]
fn arcs_of_parabola_and_hyperbola_closed_by_a_line_pad_to_their_areas() {
    use wb_sketch::sketch::ConicKind;
    // y = x² / 8 from x = -4 to 4, cut off at y = 2.
    let volume = padded_conic_volume(
        ConicKind::Parabola,
        Vec2D::new(0.0, 0.0),
        Vec2D::new(0.0, 2.0),
        0.0,
        Vec2D::new(4.0, 2.0),
        Vec2D::new(-4.0, 2.0),
        3.0,
    );
    let expected = 2.0 / 3.0 * 8.0 * 2.0 * 3.0;
    assert!(
        (volume - expected).abs() < 1e-3 * expected,
        "volume {volume} vs {expected}"
    );

    // x = 3·sqrt(1 + y² / 1.5²), cut off at x = 5 (y = ±2).
    let volume = padded_conic_volume(
        ConicKind::Hyperbola,
        Vec2D::new(0.0, 0.0),
        Vec2D::new(3.0, 0.0),
        1.5,
        Vec2D::new(5.0, -2.0),
        Vec2D::new(5.0, 2.0),
        2.0,
    );
    let steps = 2000;
    let area: f64 = (0..steps)
        .map(|i| {
            let y = -2.0 + 4.0 * (i as f64 + 0.5) / steps as f64;
            (5.0 - 3.0 * (1.0 + y * y / 2.25).sqrt()) * 4.0 / steps as f64
        })
        .sum();
    let expected = area * 2.0;
    assert!(
        (volume - expected).abs() < 1e-3 * expected,
        "volume {volume} vs {expected}"
    );
}

/// A `size`×`size`×`height` block with one hole position at the middle of
/// its top face, and the hole `hole` makes from that sketch: the block's
/// volume and properties once drilled.
fn drilled_block(
    size: f32,
    height: f32,
    hole: impl FnOnce(FeatureId) -> PartFeature,
) -> Result<kernel_api::PhysicalProperties, String> {
    let (result, mut kernel) = drilled_block_solid(size, height, hole)?;
    kernel
        .physical_properties(&result.brep_blob)
        .map_err(|e| e.to_string())
}

/// As [`drilled_block`], the built solid itself.
fn drilled_block_solid(
    size: f32,
    height: f32,
    hole: impl FnOnce(FeatureId) -> PartFeature,
) -> Result<(kernel_api::SolidBuildResult, OgeomKernel), String> {
    let (mut doc, body, rect_id) = setup(size, size);
    doc.add_feature_in_body(
        pad_feature(rect_id, height, false, false),
        "Pad".into(),
        Some(body),
    )
    .unwrap();
    let top_face = wb_sketch::sketch::SketchPlane::from_face(
        [size * 0.5, size * 0.5, height],
        [0.0, 0.0, 1.0],
    );
    let mut holes = Sketch::new("holes");
    holes.plane = top_face;
    holes.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(
        size * 0.5,
        size * 0.5,
    ))));
    let holes_id = doc
        .add_feature_in_body(
            SketchFeature::new(holes, top_face),
            "holes".into(),
            Some(body),
        )
        .unwrap();
    doc.add_feature_in_body(hole(holes_id), "Hole".into(), Some(body))
        .unwrap();
    let plan = wb_part::body_build_ops(&doc, body).map_err(|e| e.message)?;
    let mut kernel = OgeomKernel::new();
    let result = kernel
        .execute_solid_chain(&plan.ops, &TessellationSettings::default())
        .map_err(|e| e.to_string())?;
    Ok((result, kernel))
}

/// A plain blind hole, `diameter` across and `depth` deep, for a test to
/// change.
fn plain_hole(sketch: FeatureId, diameter: f32, depth: f32) -> PartFeature {
    PartFeature::Hole {
        refine: false,
        sketch,
        diameter,
        depth,
        through_all: false,
        cut: wb_part::HoleCut::None,
        thread: None,
        threaded: false,
        modeled_thread: false,
        thread_depth: 0.0,
        fit: wb_part::HoleFit::Normal,
        drill_point: wb_part::DrillPoint::Flat,
        point_in_depth: false,
        taper_deg: 0.0,
        reversed: false,
    }
}

fn with_hole(mut feature: PartFeature, edit: impl FnOnce(&mut PartFeature)) -> PartFeature {
    edit(&mut feature);
    feature
}

fn volume(properties: &kernel_api::PhysicalProperties) -> f64 {
    properties.volume_mm3.expect("a closed solid")
}

use std::f64::consts::PI;

/// A 118° drill point below the depth adds its cone under the wall; within
/// the depth the wall stops short so the tip lands on the depth.
#[test]
fn an_angled_drill_point_cones_the_bottom_of_a_blind_hole() {
    let block = 20.0 * 20.0 * 10.0;
    let (r, depth) = (3.0f64, 7.0f64);
    let cone_height = r / 59f64.to_radians().tan();
    let cone = PI * r * r * cone_height / 3.0;
    for in_depth in [false, true] {
        let drilled = drilled_block(20.0, 10.0, |sketch| {
            with_hole(plain_hole(sketch, 6.0, 7.0), |f| {
                if let PartFeature::Hole {
                    drill_point,
                    point_in_depth,
                    ..
                } = f
                {
                    *drill_point = wb_part::DrillPoint::Angled { angle_deg: 118.0 };
                    *point_in_depth = in_depth;
                }
            })
        })
        .unwrap_or_else(|e| panic!("builds: {e}"));
        let wall = if in_depth { depth - cone_height } else { depth };
        let want = block - PI * r * r * wall - cone;
        assert!(
            (volume(&drilled) - want).abs() < 0.05,
            "point in depth {in_depth}: {} against {want}",
            volume(&drilled)
        );
    }
}

/// A counterdrill: a wide bore to its depth, then a 90° cone narrowing to
/// the hole.
#[test]
fn a_counterdrill_bores_then_cones_down_to_the_hole() {
    let drilled = drilled_block(20.0, 10.0, |sketch| {
        with_hole(plain_hole(sketch, 4.0, 8.0), |f| {
            if let PartFeature::Hole { cut, .. } = f {
                *cut = wb_part::HoleCut::Counterdrill {
                    diameter: 8.0,
                    depth: 3.0,
                    angle_deg: 90.0,
                };
            }
        })
    })
    .unwrap_or_else(|e| panic!("builds: {e}"));
    // Radius 4 to 3 deep, a cone from 4 to 2 over the next 2, radius 2
    // to 8 deep.
    let removed = PI * 16.0 * 3.0 + PI * 2.0 / 3.0 * (16.0 + 8.0 + 4.0) + PI * 4.0 * 3.0;
    let want = 20.0 * 20.0 * 10.0 - removed;
    assert!(
        (volume(&drilled) - want).abs() < 0.05,
        "{} against {want}",
        volume(&drilled)
    );
}

/// A spotface faces a shallow seat around the hole.
#[test]
fn a_spotface_faces_a_shallow_seat() {
    let drilled = drilled_block(20.0, 10.0, |sketch| {
        with_hole(plain_hole(sketch, 4.0, 8.0), |f| {
            if let PartFeature::Hole { cut, .. } = f {
                *cut = wb_part::HoleCut::Spotface {
                    diameter: 10.0,
                    depth: 0.5,
                };
            }
        })
    })
    .unwrap_or_else(|e| panic!("builds: {e}"));
    let want = 20.0 * 20.0 * 10.0 - PI * 25.0 * 0.5 - PI * 4.0 * 7.5;
    assert!(
        (volume(&drilled) - want).abs() < 0.05,
        "{} against {want}",
        volume(&drilled)
    );
}

/// A tapered hole narrows toward its bottom: a frustum.
#[test]
fn a_tapered_hole_narrows_toward_its_bottom() {
    let drilled = drilled_block(20.0, 10.0, |sketch| {
        with_hole(plain_hole(sketch, 6.0, 8.0), |f| {
            if let PartFeature::Hole { taper_deg, .. } = f {
                *taper_deg = 5.0;
            }
        })
    })
    .unwrap_or_else(|e| panic!("builds: {e}"));
    let (r1, h) = (3.0f64, 8.0f64);
    let r2 = r1 - h * 5f64.to_radians().tan();
    let want = 20.0 * 20.0 * 10.0 - PI * h / 3.0 * (r1 * r1 + r1 * r2 + r2 * r2);
    assert!(
        (volume(&drilled) - want).abs() < 0.05,
        "{} against {want}",
        volume(&drilled)
    );
}

/// A tapped 1/4 NPT hole is drilled at the thread's minor diameter where
/// it opens and narrows 1:16 on the diameter toward its bottom.
#[test]
fn an_npt_hole_tapers_one_in_sixteen() {
    let npt = wb_part::ThreadStandard::Npt.size("1/4").unwrap();
    let drilled = drilled_block(30.0, 15.0, |sketch| {
        with_hole(plain_hole(sketch, 1.0, 10.0), |f| {
            if let PartFeature::Hole {
                thread, threaded, ..
            } = f
            {
                *thread = Some(wb_part::ThreadSpec::new(
                    wb_part::ThreadStandard::Npt,
                    "1/4",
                ));
                *threaded = true;
            }
        })
    })
    .unwrap_or_else(|e| panic!("builds: {e}"));
    let r1 = npt.minor * 0.5;
    let r2 = r1 - 10.0 / 32.0;
    let want = 30.0 * 30.0 * 15.0 - PI * 10.0 / 3.0 * (r1 * r1 + r1 * r2 + r2 * r2);
    assert!(
        (volume(&drilled) - want).abs() < 0.05,
        "{} against {want}",
        volume(&drilled)
    );
}

/// A modeled NPT thread follows the taper: its groove cuts into the
/// tapered wall and no further out than the major diameter at the face.
#[test]
fn a_modeled_npt_thread_cuts_along_the_taper() {
    let npt = wb_part::ThreadStandard::Npt.size("1/4").unwrap();
    let hole = |modeled: bool| {
        move |sketch| {
            with_hole(plain_hole(sketch, 1.0, 10.0), |f| {
                if let PartFeature::Hole {
                    thread,
                    threaded,
                    modeled_thread,
                    thread_depth,
                    ..
                } = f
                {
                    *thread = Some(wb_part::ThreadSpec::new(
                        wb_part::ThreadStandard::Npt,
                        "1/4",
                    ));
                    *threaded = true;
                    *modeled_thread = modeled;
                    *thread_depth = 6.0;
                }
            })
        }
    };
    let tapped = volume(&drilled_block(30.0, 15.0, hole(false)).unwrap());
    let threaded =
        volume(&drilled_block(30.0, 15.0, hole(true)).unwrap_or_else(|e| panic!("builds: {e}")));
    let outer = PI * (npt.major * 0.5).powi(2) * 7.0;
    assert!(
        threaded < tapped - 5.0 && threaded > tapped - outer,
        "the groove takes some of the wall, not all of it: {threaded} (tapped {tapped})"
    );
}

/// A left-hand modeled thread is the mirror of the right-hand one. A
/// quarter pitch of thread is the tail of a groove that enters the
/// material over its last turn, most of it past the turn's start, where
/// the section plane sits (+X). A right-hand thread turns by the
/// right-hand rule about the way it runs in (-Z), from +X toward -Y, so
/// its groove's wall lies on the -Y side of the axis; a left-hand one's on
/// the +Y side.
#[test]
fn a_left_hand_thread_turns_the_other_way() {
    let hole = |left_handed: bool| {
        move |sketch| {
            with_hole(plain_hole(sketch, 5.0, 8.0), |f| {
                if let PartFeature::Hole {
                    thread,
                    threaded,
                    modeled_thread,
                    thread_depth,
                    ..
                } = f
                {
                    let mut spec =
                        wb_part::ThreadSpec::new(wb_part::ThreadStandard::IsoMetricCoarse, "M6");
                    spec.left_handed = left_handed;
                    *thread = Some(spec);
                    *threaded = true;
                    *modeled_thread = true;
                    *thread_depth = 0.25;
                }
            })
        }
    };
    // The mean offset from the axis of the mesh's points in the groove:
    // out past the drilled wall (radius 2.5) and below the face.
    let groove = |left_handed: bool| {
        let (result, _) = drilled_block_solid(20.0, 10.0, hole(left_handed))
            .unwrap_or_else(|e| panic!("builds: {e}"));
        let points: Vec<[f32; 3]> = result
            .mesh
            .positions
            .iter()
            .copied()
            .filter(|p| {
                let r = ((p[0] - 10.0).powi(2) + (p[1] - 10.0).powi(2)).sqrt();
                r > 2.6 && r < 3.5 && p[2] < 9.99
            })
            .collect();
        assert!(!points.is_empty(), "the groove is cut");
        let n = points.len() as f32;
        let mean = |k: usize| points.iter().map(|p| p[k] - 10.0).sum::<f32>() / n;
        [mean(0), mean(1)]
    };
    let (right, left) = (groove(false), groove(true));
    assert!(right[1] < -0.3, "right-hand runs toward -Y: {right:?}");
    assert!(left[1] > 0.3, "left-hand runs toward +Y: {left:?}");
    assert!(
        (right[0] - left[0]).abs() < 0.05 && (right[1] + left[1]).abs() < 0.05,
        "mirror images across the XZ plane: {right:?} {left:?}"
    );
}

/// A document written when a hole named its ISO metric size by
/// `metric_index` still loads, as that size, and builds the same drill.
#[test]
fn a_metric_index_hole_loads_as_its_iso_metric_size() {
    let (_, _, sketch) = setup(1.0, 1.0);
    let old = serde_json::json!({
        "Hole": {
            "sketch": sketch.0.to_string(),
            "diameter": 99.0,
            "depth": 4.0,
            "through_all": false,
            "cut": "None",
            "metric_index": 5,
            "threaded": true,
            "fit": "Normal",
            "reversed": false
        }
    });
    let feature: PartFeature = serde_json::from_value(old).unwrap();
    let PartFeature::Hole { thread, .. } = &feature else {
        panic!("a hole");
    };
    assert_eq!(
        thread.as_ref().map(|t| (t.standard, t.size.as_str())),
        Some((wb_part::ThreadStandard::IsoMetricCoarse, "M6"))
    );
    assert!((wb_part::hole_diameter(&feature) - 5.0).abs() < 1e-6);
    let drilled = drilled_block(20.0, 10.0, |sketch| {
        let mut json = serde_json::to_value(&feature).unwrap();
        json["Hole"]["sketch"] = serde_json::json!(sketch.0.to_string());
        serde_json::from_value(json).unwrap()
    })
    .unwrap_or_else(|e| panic!("builds: {e}"));
    let want = 20.0 * 20.0 * 10.0 - PI * 2.5 * 2.5 * 4.0;
    assert!(
        (volume(&drilled) - want).abs() < 0.01,
        "{}",
        volume(&drilled)
    );
}

/// An ISO 4762 seat on an M6 hole is the DIN 974-1 counterbore, 11 mm
/// across and 6.4 deep.
#[test]
fn a_socket_head_seat_counterbores_to_the_table() {
    let drilled = drilled_block(20.0, 10.0, |sketch| {
        with_hole(plain_hole(sketch, 1.0, 9.0), |f| {
            if let PartFeature::Hole {
                thread, cut, fit, ..
            } = f
            {
                *thread = Some(wb_part::ThreadSpec::new(
                    wb_part::ThreadStandard::IsoMetricCoarse,
                    "M6",
                ));
                *fit = wb_part::HoleFit::Normal;
                *cut = wb_part::HoleCut::Seat {
                    seat: wb_part::ScrewSeat::SocketHead,
                };
            }
        })
    })
    .unwrap_or_else(|e| panic!("builds: {e}"));
    let want = 20.0 * 20.0 * 10.0 - PI * 5.5 * 5.5 * 6.4 - PI * 3.3 * 3.3 * (9.0 - 6.4);
    assert!(
        (volume(&drilled) - want).abs() < 0.05,
        "{} against {want}",
        volume(&drilled)
    );
}

/// A rectangle from (x0, y0) to (x1, y1) on `plane`.
fn rect_at(
    plane: wb_sketch::sketch::SketchPlane,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
) -> SketchFeature {
    let mut sketch = Sketch::new("r");
    sketch.plane = plane;
    let corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
        .map(|(x, y)| sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(x, y)))));
    for i in 0..4 {
        sketch.add_geometry(GeometryElement::Line(Line::new(
            corners[i],
            corners[(i + 1) % 4],
        )));
    }
    SketchFeature::new(sketch, plane)
}

/// The plane z = `z`, sketched as the XY plane is.
fn plane_at_z(z: f32) -> wb_sketch::sketch::SketchPlane {
    wb_sketch::sketch::SketchPlane::from_frame([0.0, 0.0, z], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0])
}

/// The volume the body's features build, or why they do not.
fn built_volume(doc: &Document, body: BodyId) -> Result<f64, String> {
    let ops = wb_part::body_build_ops(doc, body)
        .map_err(|e| e.message)?
        .ops;
    let mut kernel = OgeomKernel::new();
    let built = kernel
        .execute_solid_chain(&ops, &TessellationSettings::default())
        .map_err(|e| e.to_string())?;
    kernel
        .physical_properties(&built.brep_blob)
        .map_err(|e| e.to_string())?
        .volume_mm3
        .ok_or_else(|| "no closed volume".into())
}

fn assert_built_volume(doc: &Document, body: BodyId, want: f64, what: &str) {
    let got = built_volume(doc, body).unwrap_or_else(|e| panic!("{what}: {e}"));
    assert!(
        (got - want).abs() <= want * 1e-4,
        "{what}: volume {got}, want {want}"
    );
}

/// A 20 × 20 × 10 block, the body's first feature.
fn block() -> (Document, BodyId) {
    let (mut doc, body, sketch) = setup(20.0, 20.0);
    doc.add_feature_in_body(
        pad_feature(sketch, 10.0, false, false),
        "Block".into(),
        Some(body),
    )
    .unwrap();
    (doc, body)
}

/// Set fields of a pad or pocket the way `part.set` does.
fn with(mut feature: PartFeature, fields: serde_json::Value) -> PartFeature {
    use core_document::WorkbenchFeature;
    let mut value = feature.to_json();
    let inner = value
        .as_object_mut()
        .and_then(|m| m.values_mut().next())
        .and_then(|v| v.as_object_mut())
        .unwrap();
    for (k, v) in fields.as_object().unwrap() {
        assert!(inner.contains_key(k), "no field {k}");
        inner.insert(k.clone(), v.clone());
    }
    feature = PartFeature::from_json(&value).unwrap();
    feature
}

/// A pad runs along a vector or a picked edge as it is told, its length
/// measured along that way, and refuses one that lies in the sketch.
#[test]
fn a_pad_runs_along_a_custom_vector_or_a_picked_edge() {
    let (mut doc, body, sketch) = setup(10.0, 10.0);
    let slanted = 1000.0 * std::f64::consts::FRAC_1_SQRT_2;
    let pad = doc
        .add_feature_in_body(
            with(
                pad_feature(sketch, 10.0, false, false),
                serde_json::json!({"direction": {"Custom": [0.0, 1.0, 1.0]}}),
            ),
            "Pad".into(),
            Some(body),
        )
        .unwrap();
    assert_built_volume(&doc, body, slanted, "along a vector");
    let data = with(
        pad_feature(sketch, 10.0, false, false),
        serde_json::json!({"direction": {"Edge": {"point": [0.0, 0.0, 0.0], "direction": [0.0, 1.0, 1.0]}}}),
    );
    doc.update_feature_data(pad, core_document::WorkbenchFeature::to_json(&data))
        .unwrap();
    assert_built_volume(&doc, body, slanted, "along an edge");
    let data = with(
        pad_feature(sketch, 10.0, false, false),
        serde_json::json!({"direction": {"Custom": [1.0, 0.0, 0.0]}}),
    );
    doc.update_feature_data(pad, core_document::WorkbenchFeature::to_json(&data))
        .unwrap();
    let refused = built_volume(&doc, body).unwrap_err();
    assert!(refused.contains("plane"), "{refused}");
}

/// Each side of a two-sided pad ends its own way: up 5 from its sketch,
/// and down to the block's top (picked, or the first face it meets).
#[test]
fn each_side_of_a_two_sided_pad_ends_its_own_way() {
    for second in [
        serde_json::json!({"mode2": "UpToFace", "up_to_face2": {"point": [5.0, 5.0, 10.0], "normal": [0.0, 0.0, 1.0]}}),
        serde_json::json!({"mode2": "ToFirst"}),
        serde_json::json!({"mode": "TwoLengths", "mode2": "ToFirst"}),
    ] {
        let (mut doc, body) = block();
        let top = doc
            .add_feature_in_body(
                rect_at(plane_at_z(20.0), 0.0, 0.0, 10.0, 10.0),
                "top".into(),
                Some(body),
            )
            .unwrap();
        doc.add_feature_in_body(
            with(pad_feature(top, 5.0, false, false), second.clone()),
            "Pad".into(),
            Some(body),
        )
        .unwrap();
        // The block, and 10 × 10 from z = 10 up to z = 25.
        assert_built_volume(&doc, body, 4000.0 + 1500.0, &second.to_string());
    }
}

/// A flat face of the solid is a profile: padded out of the material, or
/// pocketed into it, with no sketch.
#[test]
fn a_flat_face_of_the_solid_pads_and_pockets_with_no_sketch() {
    let top = serde_json::json!({"point": [10.0, 10.0, 10.0], "normal": [0.0, 0.0, 1.0]});
    let (mut doc, body) = block();
    let (_, _, any) = setup(1.0, 1.0);
    doc.add_feature_in_body(
        with(
            pad_feature(any, 5.0, false, false),
            serde_json::json!({"sketch": null, "profile_face": top}),
        ),
        "Pad".into(),
        Some(body),
    )
    .unwrap();
    assert_built_volume(&doc, body, 6000.0, "the top face padded 5");

    let (mut doc, body) = block();
    doc.add_feature_in_body(
        with(
            pocket_feature(any, 3.0),
            serde_json::json!({"sketch": null, "profile_face": top}),
        ),
        "Pocket".into(),
        Some(body),
    )
    .unwrap();
    assert_built_volume(&doc, body, 2800.0, "the top face pocketed 3");
}

/// Up to shape: a pad run down over the step's edge stops on the step's
/// top where it lies over the step and on the block's beside it.
#[test]
fn an_up_to_shape_pad_stops_where_each_line_first_meets_a_picked_face() {
    let (mut doc, body) = block();
    let step = doc
        .add_feature_in_body(
            rect_at(plane_at_z(10.0), 0.0, 0.0, 10.0, 20.0),
            "step".into(),
            Some(body),
        )
        .unwrap();
    doc.add_feature_in_body(
        pad_feature(step, 5.0, false, false),
        "Step".into(),
        Some(body),
    )
    .unwrap();
    let top = doc
        .add_feature_in_body(
            rect_at(plane_at_z(20.0), 4.0, 5.0, 16.0, 15.0),
            "top".into(),
            Some(body),
        )
        .unwrap();
    doc.add_feature_in_body(
        with(
            pad_feature(top, 5.0, true, false),
            serde_json::json!({
                "mode": "UpToShape",
                "up_to_shape": [
                    {"point": [5.0, 10.0, 15.0], "normal": [0.0, 0.0, 1.0]},
                    {"point": [15.0, 10.0, 10.0], "normal": [0.0, 0.0, 1.0]}
                ]
            }),
        ),
        "Pad".into(),
        Some(body),
    )
    .unwrap();
    assert_built_volume(&doc, body, 4000.0 + 1000.0 + 300.0 + 600.0, "up to shape");
}

/// A 5 × 10 rectangle standing on the XZ plane at x in [5, 10], and a
/// wall x in [-20, 0], y in [0, 20], 10 high, beside the Z axis.
fn wall_and_profile() -> (Document, BodyId, FeatureId) {
    let mut doc = Document::new("t");
    let body = doc.create_body(Some("Body".into()));
    let wall = doc
        .add_feature_in_body(
            rect_at(wb_sketch::sketch::SketchPlane::xy(), -20.0, 0.0, 0.0, 20.0),
            "wall".into(),
            Some(body),
        )
        .unwrap();
    doc.add_feature_in_body(
        pad_feature(wall, 10.0, false, false),
        "Wall".into(),
        Some(body),
    )
    .unwrap();
    let profile = doc
        .add_feature_in_body(
            rect_at(wb_sketch::sketch::SketchPlane::xz(), 5.0, 0.0, 10.0, 10.0),
            "profile".into(),
            Some(body),
        )
        .unwrap();
    (doc, body, profile)
}

fn revolution(sketch: FeatureId, fields: serde_json::Value) -> PartFeature {
    with(
        PartFeature::Revolution {
            refine: false,
            sketch,
            angle_deg: 360.0,
            axis: wb_part::RevolveAxis::SketchY,
            reversed: false,
            midplane: false,
            second_angle_deg: None,
            mode: wb_part::RevolveMode::Angle,
            up_to_face: None,
        },
        fields,
    )
}

/// A revolution turns until it meets the wall: up to its first face a
/// quarter turn on, adding material there; a groove to its last face (or
/// that face picked), half a turn on, cuts the quarter through the wall.
#[test]
fn a_revolution_or_a_groove_turns_up_to_the_face_it_meets() {
    use core_document::WorkbenchFeature;
    let ring = 75.0 * std::f64::consts::PI * 10.0;
    let groove = |sketch, fields| {
        let json = revolution(sketch, fields).to_json();
        let inner = json.get("Revolution").cloned().unwrap();
        PartFeature::from_json(&serde_json::json!({ "Groove": inner })).unwrap()
    };
    for (fields, cuts, change) in [
        (serde_json::json!({"mode": "ToFirst"}), false, ring / 4.0),
        (serde_json::json!({"mode": "ToLast"}), true, -ring / 4.0),
        (
            serde_json::json!({"mode": "UpToFace",
                "up_to_face": {"point": [-10.0, 0.0, 5.0], "normal": [0.0, -1.0, 0.0]}}),
            true,
            -ring / 4.0,
        ),
    ] {
        let (mut doc, body, profile) = wall_and_profile();
        let feature = if cuts {
            groove(profile, fields.clone())
        } else {
            revolution(profile, fields.clone())
        };
        doc.add_feature_in_body(feature, "Rev".into(), Some(body))
            .unwrap();
        assert_built_volume(&doc, body, 4000.0 + change, &fields.to_string());
    }
}

/// A revolution turns about a line of its sketch, a datum line or a picked
/// edge as it does about the sketch's own axis; an edge across the sketch
/// plane is refused.
#[test]
fn a_revolution_turns_about_a_sketch_line_a_datum_line_or_a_picked_edge() {
    use core_document::{BasePlane, DatumAttachment, DatumFeature, DatumShape};
    let mut doc = Document::new("t");
    let body = doc.create_body(Some("Body".into()));
    let mut ring = rect_at(wb_sketch::sketch::SketchPlane::xy(), 5.0, 0.0, 8.0, 2.0);
    let a = ring
        .sketch
        .add_geometry(GeometryElement::Point(Point::new(Vec2D::new(0.0, -1.0))));
    let b = ring
        .sketch
        .add_geometry(GeometryElement::Point(Point::new(Vec2D::new(0.0, 4.0))));
    let line = ring
        .sketch
        .add_geometry(GeometryElement::Line(Line::new(a, b)));
    ring.sketch.set_construction(line, true);
    let sketch = doc
        .add_feature_in_body(ring, "ring".into(), Some(body))
        .unwrap();
    // A datum line along world Y: the YZ plane's x axis.
    let datum = doc
        .add_feature_in_body(
            DatumFeature {
                shape: DatumShape::Line { length: 20.0 },
                attachment: DatumAttachment::BasePlane(BasePlane::YZ),
                offset: Default::default(),
            },
            "Line".into(),
            Some(body),
        )
        .unwrap();
    let want = std::f64::consts::PI * (64.0 - 25.0) * 2.0;
    let feature = doc
        .add_feature_in_body(
            revolution(sketch, serde_json::json!({})),
            "Rev".into(),
            Some(body),
        )
        .unwrap();
    let edge = |direction: [f32; 3]| {
        wb_part::RevolveAxis::Edge(wb_part::EdgePick {
            point: [0.0, 3.0, 0.0],
            direction,
        })
    };
    for axis in [
        wb_part::RevolveAxis::SketchLine(line),
        wb_part::RevolveAxis::Datum(datum),
        edge([0.0, 1.0, 0.0]),
    ] {
        let data = revolution(
            sketch,
            serde_json::json!({"axis": serde_json::to_value(axis).unwrap()}),
        );
        doc.update_feature_data(feature, core_document::WorkbenchFeature::to_json(&data))
            .unwrap();
        assert_built_volume(&doc, body, want, &format!("{axis:?}"));
    }
    let data = revolution(
        sketch,
        serde_json::json!({"axis": serde_json::to_value(edge([0.0, 0.0, 1.0])).unwrap()}),
    );
    doc.update_feature_data(feature, core_document::WorkbenchFeature::to_json(&data))
        .unwrap();
    let refused = built_volume(&doc, body).unwrap_err();
    assert!(refused.contains("sketch plane"), "{refused}");
}

/// A 60 × 60 × 4 plate and a boss of radius 3, 6 high on its top at
/// (`x`, `y`): the document, the body, the plate's sketch and the boss.
fn plate_with_boss(x: f32, y: f32) -> (Document, BodyId, FeatureId, FeatureId) {
    let (mut doc, body, plate) = setup(60.0, 60.0);
    doc.add_feature_in_body(
        pad_feature(plate, 4.0, false, false),
        "Plate".into(),
        Some(body),
    )
    .unwrap();
    let boss = doc
        .add_feature_in_body(
            circle_sketch_on(plane_at_z(4.0), x, y, 3.0),
            "boss".into(),
            Some(body),
        )
        .unwrap();
    let boss = doc
        .add_feature_in_body(
            pad_feature(boss, 6.0, false, false),
            "Boss".into(),
            Some(body),
        )
        .unwrap();
    (doc, body, plate, boss)
}

/// The plate and `n` whole bosses.
fn plate_and_bosses(n: f64) -> f64 {
    60.0 * 60.0 * 4.0 + n * std::f64::consts::PI * 9.0 * 6.0
}

/// The volume and the bounds of what the body's features build.
fn built_solid(doc: &Document, body: BodyId) -> (f64, [f32; 3], [f32; 3]) {
    let ops = wb_part::body_build_ops(doc, body).unwrap().ops;
    let mut kernel = OgeomKernel::new();
    let built = kernel
        .execute_solid_chain(&ops, &TessellationSettings::default())
        .unwrap();
    let volume = kernel
        .physical_properties(&built.brep_blob)
        .unwrap()
        .volume_mm3
        .unwrap();
    let (min, max) = mesh_bounds(&built.mesh);
    (volume, min, max)
}

fn linear_pattern(original: FeatureId, fields: serde_json::Value) -> PartFeature {
    with(
        PartFeature::LinearPattern {
            refine: false,
            originals: vec![original],
            axis: wb_part::PatternAxis::X,
            length: 50.0,
            occurrences: 3,
            spacing_mode: false,
            spacings: Vec::new(),
            reversed: false,
        },
        fields,
    )
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.1
}

/// A linear pattern runs along a picked edge, a datum line or a sketch's
/// axis as it does along the body's Y axis: bosses at y = 10, 35 and 60,
/// the last overhanging the plate's far side.
#[test]
fn a_linear_pattern_runs_along_an_edge_a_datum_line_or_a_sketch_axis() {
    use core_document::{BasePlane, DatumAttachment, DatumFeature, DatumShape};
    let (mut doc, body, plate, boss) = plate_with_boss(10.0, 10.0);
    // The YZ plane's datum line runs along world Y.
    let datum = doc
        .add_feature_in_body(
            DatumFeature {
                shape: DatumShape::Line { length: 20.0 },
                attachment: DatumAttachment::BasePlane(BasePlane::YZ),
                offset: Default::default(),
            },
            "Line".into(),
            Some(body),
        )
        .unwrap();
    let pattern = doc
        .add_feature_in_body(
            linear_pattern(boss, serde_json::json!({})),
            "Pattern".into(),
            Some(body),
        )
        .unwrap();
    let edge = wb_part::PatternAxis::Edge(wb_part::EdgePick {
        point: [0.0, 30.0, 4.0],
        direction: [0.0, 1.0, 0.0],
    });
    let sketch_v = wb_part::PatternAxis::Sketch {
        sketch: plate,
        axis: wb_part::SketchAxis::Vertical,
    };
    for axis in [
        wb_part::PatternAxis::Y,
        edge,
        wb_part::PatternAxis::Datum(datum),
        sketch_v,
    ] {
        let data = linear_pattern(
            boss,
            serde_json::json!({"axis": serde_json::to_value(axis).unwrap()}),
        );
        doc.update_feature_data(pattern, core_document::WorkbenchFeature::to_json(&data))
            .unwrap();
        let (volume, _, max) = built_solid(&doc, body);
        assert!(
            (volume - plate_and_bosses(3.0)).abs() < 1.0,
            "{axis:?}: volume {volume}"
        );
        assert!(
            close(max[1], 63.0) && close(max[0], 60.0),
            "{axis:?}: {max:?}"
        );
    }
}

/// Uneven spacing: the bosses 15 then 35 apart, the last overhanging the
/// plate's side; along the sketch's normal the copies stack up in z.
#[test]
fn an_uneven_linear_pattern_spaces_each_occurrence_its_own_way() {
    let (mut doc, body, _, boss) = plate_with_boss(10.0, 10.0);
    doc.add_feature_in_body(
        linear_pattern(boss, serde_json::json!({"spacings": [15.0, 35.0]})),
        "Pattern".into(),
        Some(body),
    )
    .unwrap();
    let (volume, _, max) = built_solid(&doc, body);
    assert!((volume - plate_and_bosses(3.0)).abs() < 1.0, "{volume}");
    assert!(close(max[0], 63.0), "{max:?}");

    // Along the plate sketch's normal, the boss copied 6 and 20 up: one
    // column 6 + 6 high and one standing apart.
    let (mut doc, body, plate, boss) = plate_with_boss(10.0, 10.0);
    let normal = wb_part::PatternAxis::Sketch {
        sketch: plate,
        axis: wb_part::SketchAxis::Normal,
    };
    doc.add_feature_in_body(
        linear_pattern(
            boss,
            serde_json::json!({
                "axis": serde_json::to_value(normal).unwrap(),
                "spacings": [6.0, 14.0]
            }),
        ),
        "Pattern".into(),
        Some(body),
    )
    .unwrap();
    let (volume, _, max) = built_solid(&doc, body);
    assert!((volume - plate_and_bosses(3.0)).abs() < 1.0, "{volume}");
    assert!(close(max[2], 30.0), "{max:?}");
}

/// A polar pattern by step about a round edge's axis: 90° a step, three
/// occurrences stand at 0°, 90° and 180° about the plate's centre, each
/// overhanging a side; the same angle as the overall one puts them at
/// 0°, 45° and 90°.
#[test]
fn a_polar_pattern_by_step_turns_each_occurrence_by_its_angle() {
    let axis = wb_part::PatternAxis::Edge(wb_part::EdgePick {
        point: [30.0, 30.0, 10.0],
        direction: [0.0, 0.0, 1.0],
    });
    let polar = |boss, step_mode: bool, angles: &[f32]| PartFeature::PolarPattern {
        refine: false,
        originals: vec![boss],
        axis,
        angle_deg: 90.0,
        occurrences: 3,
        reversed: false,
        step_mode,
        angles: angles.to_vec(),
    };
    let (mut doc, body, _, boss) = plate_with_boss(58.0, 30.0);
    let pattern = doc
        .add_feature_in_body(polar(boss, true, &[]), "Pattern".into(), Some(body))
        .unwrap();
    let (volume, min, max) = built_solid(&doc, body);
    assert!((volume - plate_and_bosses(3.0)).abs() < 1.0, "{volume}");
    assert!(close(min[0], -1.0) && close(min[1], 0.0), "{min:?}");
    assert!(close(max[0], 61.0) && close(max[1], 61.0), "{max:?}");

    let data = polar(boss, false, &[]);
    doc.update_feature_data(pattern, core_document::WorkbenchFeature::to_json(&data))
        .unwrap();
    let (_, min, _) = built_solid(&doc, body);
    assert!(close(min[0], 0.0), "overall 90°: {min:?}");

    // Uneven: 180° then 90° more stands the last at 270°, off the near side.
    let data = polar(boss, true, &[180.0, 90.0]);
    doc.update_feature_data(pattern, core_document::WorkbenchFeature::to_json(&data))
        .unwrap();
    let (_, min, max) = built_solid(&doc, body);
    assert!(close(min[0], -1.0) && close(min[1], -1.0), "{min:?}");
    assert!(close(max[1], 60.0), "{max:?}");
}

/// The block hollowed to walls of 1, open at the top, its walls joined as
/// `join` says; `inward` keeps the block's outside, else the walls grow
/// around it.
fn thickened_block(inward: bool, join: kernel_api::ThicknessJoin) -> Result<f64, String> {
    let (mut doc, body) = block();
    doc.add_feature_in_body(
        PartFeature::Thickness {
            value: 1.0,
            faces: vec![wb_part::FacePick {
                point: [10.0, 10.0, 10.0],
                normal: [0.0, 0.0, 1.0],
            }],
            inward,
            join,
        },
        "Thickness".into(),
        Some(body),
    )
    .unwrap();
    built_volume(&doc, body)
}

/// The intersection join: walls that run on to sharp corners, inward
/// (the block less an 18 × 18 × 9 cavity) and outward (a 22 × 22 × 11
/// box less the block). Inward on a convex block, the arc join meets the
/// intersection's walls as they are.
#[test]
fn a_thickness_joins_its_walls_by_intersection() {
    use kernel_api::ThicknessJoin::Intersection;
    let inward = thickened_block(true, Intersection).unwrap();
    assert!((inward - 1084.0).abs() < 1084.0 * 1e-4, "inward {inward}");
    let outward = thickened_block(false, Intersection).unwrap();
    assert!(
        (outward - 1324.0).abs() < 1324.0 * 1e-4,
        "outward {outward}"
    );
}

/// The arc join: outward, the walls round about every edge of the block
/// below its open top, radius 1: the block grown by a ball of radius 1,
/// cut flush at the top, less the block itself.
#[test]
#[ignore = "kernel: make_thick_solid has no arc join (ogeom-rs#75)"]
fn a_thickness_joins_its_walls_by_intersection_or_arc() {
    use kernel_api::ThicknessJoin::Arc;
    let pi = std::f64::consts::PI;
    let rounded = 1200.0 + 30.0 * pi + 2.0 / 3.0 * pi;
    let outward = thickened_block(false, Arc).unwrap();
    assert!(
        (outward - rounded).abs() < rounded * 1e-4,
        "outward {outward}"
    );
    let inward = thickened_block(true, Arc).unwrap();
    assert!((inward - 1084.0).abs() < 1084.0 * 1e-4, "inward {inward}");
}

/// The block with its vertical edge at x = y = 20 rounded to radius 3,
/// and a dress-up of the top edges on top of it.
fn block_with_rounded_corner(dress_up: PartFeature) -> Result<f64, String> {
    let (mut doc, body) = block();
    doc.add_feature_in_body(
        PartFeature::Fillet {
            radius: 3.0,
            edges: wb_part::EdgeSel::Edges(vec![wb_part::EdgePick {
                point: [20.0, 20.0, 5.0],
                direction: [0.0, 0.0, 1.0],
            }]),
            follow_tangent: false,
        },
        "Corner".into(),
        Some(body),
    )
    .unwrap();
    doc.add_feature_in_body(dress_up, "Top".into(), Some(body))
        .unwrap();
    built_volume(&doc, body)
}

/// The top edges the rounded corner joins: the side at x = 20, the round,
/// and the side at y = 20.
fn top_chain() -> Vec<wb_part::EdgePick> {
    let d = 3.0 * std::f32::consts::FRAC_1_SQRT_2;
    vec![
        wb_part::EdgePick {
            point: [20.0, 8.0, 10.0],
            direction: [0.0, 1.0, 0.0],
        },
        wb_part::EdgePick {
            point: [17.0 + d, 17.0 + d, 10.0],
            direction: [-1.0, 1.0, 0.0],
        },
        wb_part::EdgePick {
            point: [8.0, 20.0, 10.0],
            direction: [1.0, 0.0, 0.0],
        },
    ]
}

/// A fillet or a chamfer on one top edge beside the rounded corner takes
/// the whole tangent chain: the round and the side past it, as picking all
/// three does; the chain stops at the block's sharp corners.
#[test]
fn a_dress_up_on_one_edge_takes_its_tangent_chain() {
    let fillet = |edges: Vec<wb_part::EdgePick>, follow_tangent| PartFeature::Fillet {
        radius: 1.0,
        edges: wb_part::EdgeSel::Edges(edges),
        follow_tangent,
    };
    let chain = top_chain();
    let one = vec![chain[0]];
    let all = block_with_rounded_corner(fillet(chain.clone(), false)).unwrap();
    let followed = block_with_rounded_corner(fillet(one.clone(), true)).unwrap();
    assert!(
        (all - followed).abs() < all * 1e-6,
        "the chain followed {followed}, picked edge by edge {all}"
    );
    let rounded_corner = 4000.0 - 9.0 * (1.0 - std::f64::consts::FRAC_PI_4) * 10.0;
    // Three edges' worth of material off: a round of radius 1 takes
    // (1 - pi/4) mm² of section along some 31 mm of edge.
    let taken = rounded_corner - followed;
    assert!(taken > 0.2146 * 25.0 && taken < 0.2146 * 40.0, "{taken}");

    let chamfer = |edges: Vec<wb_part::EdgePick>, follow_tangent| PartFeature::Chamfer {
        size: 1.0,
        mode: wb_part::ChamferMode::EqualDistance,
        size2: 1.0,
        angle_deg: 45.0,
        flip: false,
        edges: wb_part::EdgeSel::Edges(edges),
        follow_tangent,
    };
    let all = block_with_rounded_corner(chamfer(chain, false)).unwrap();
    let followed = block_with_rounded_corner(chamfer(one, true)).unwrap();
    assert!(
        (all - followed).abs() < all * 1e-6,
        "the chain followed {followed}, picked edge by edge {all}"
    );
}

/// The volume and bounds of a body built from its features.
fn built_body(doc: &Document, body: BodyId) -> Result<(f64, [f32; 3], [f32; 3]), String> {
    let ops = wb_part::body_build_ops(doc, body)
        .map_err(|e| e.message)?
        .ops;
    let mut kernel = OgeomKernel::new();
    let built = kernel
        .execute_solid_chain(&ops, &TessellationSettings::default())
        .map_err(|e| e.to_string())?;
    let volume = kernel
        .physical_properties(&built.brep_blob)
        .map_err(|e| e.to_string())?
        .volume_mm3
        .ok_or("no closed volume")?;
    let (min, max) = built.bounds_mm.ok_or("no bounds")?;
    Ok((volume, min, max))
}

fn assert_near(got: f64, want: f64, rel: f64, what: &str) {
    assert!(
        (got - want).abs() <= want * rel,
        "{what}: {got}, want {want}"
    );
}

/// A sketch of one open chain of lines through `points`, on `plane`.
fn polyline_sketch(plane: wb_sketch::sketch::SketchPlane, points: &[[f32; 2]]) -> SketchFeature {
    let mut sketch = Sketch::new("path");
    sketch.plane = plane;
    let ids: Vec<_> = points
        .iter()
        .map(|p| sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(p[0], p[1])))))
        .collect();
    for pair in ids.windows(2) {
        sketch.add_geometry(GeometryElement::Line(Line::new(pair[0], pair[1])));
    }
    SketchFeature::new(sketch, plane)
}

/// A 1 × 1 square 5 to 6 mm from the sketch's Y axis.
fn coil_section() -> SketchFeature {
    let mut sketch = Sketch::new("coil");
    let corners = [[5.0, 0.0], [6.0, 0.0], [6.0, 1.0], [5.0, 1.0]];
    let ids: Vec<_> = corners
        .iter()
        .map(|p| sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(p[0], p[1])))))
        .collect();
    for i in 0..4 {
        sketch.add_geometry(GeometryElement::Line(Line::new(ids[i], ids[(i + 1) % 4])));
    }
    let plane = sketch.plane;
    SketchFeature::new(sketch, plane)
}

fn growing_helix(sketch: FeatureId, height: f32, turns: f32, growth: f32) -> PartFeature {
    PartFeature::Helix {
        refine: false,
        sketch,
        axis: wb_part::RevolveAxis::SketchY,
        mode: wb_part::HelixMode::HeightTurnsGrowth,
        pitch: 0.0,
        height,
        turns,
        left_handed: false,
        cone_angle_deg: 0.0,
        reversed: false,
        subtractive: false,
        growth,
        keep_inside: false,
    }
}

/// The volume the unit-area coil section sweeps turning `turns` times about
/// an axis in its plane, its centroid starting 5.5 mm out and moving
/// `growth` further out each turn: the area times the arc its centroid
/// turns through, however it climbs.
fn coil_volume(turns: f64, growth: f64) -> f64 {
    let pi = std::f64::consts::PI;
    2.0 * pi * turns * 5.5 + pi * growth * turns * turns
}

/// A helix given by height, turns and growth per turn widens as it climbs.
#[test]
fn a_helix_grows_by_its_growth_per_turn() {
    let mut doc = Document::new("t");
    let body = doc.create_body(Some("Body".into()));
    let sketch = doc
        .add_feature_in_body(coil_section(), "coil".into(), Some(body))
        .unwrap();
    doc.add_feature_in_body(
        growing_helix(sketch, 12.0, 4.0, 1.0),
        "Helix".into(),
        Some(body),
    )
    .unwrap();
    let (volume, min, max) = built_body(&doc, body).unwrap();
    assert_near(volume, coil_volume(4.0, 1.0), 5e-3, "conical coil");
    // Four turns out by a millimetre each: the outer edge ends 6 + 4 out.
    assert!(max[0] > 9.9 && max[0] < 10.1, "x to {}", max[0]);
    assert!((max[1] - min[1] - 13.0).abs() < 0.05, "climbs 12 + 1");
}

/// A helix of no height is a flat spiral, each turn 2 mm further out than
/// the last.
#[test]
#[ignore = "kernel: make_helical_sweep refuses a pitch of 0, so a flat spiral cannot be swept (ogeom-rs#79)"]
fn a_helix_of_no_height_is_a_flat_spiral() {
    let mut doc = Document::new("t");
    let body = doc.create_body(Some("Body".into()));
    let sketch = doc
        .add_feature_in_body(coil_section(), "coil".into(), Some(body))
        .unwrap();
    doc.add_feature_in_body(
        growing_helix(sketch, 0.0, 3.0, 2.0),
        "Spiral".into(),
        Some(body),
    )
    .unwrap();
    let (volume, min, max) = built_body(&doc, body).unwrap();
    assert_near(volume, coil_volume(3.0, 2.0), 5e-3, "flat spiral");
    assert!(
        (max[1] - min[1] - 1.0).abs() < 0.01,
        "stays one section high"
    );
}

/// A 40 mm thick block from `x0` to 20 in X and -5 to 25 in Y, clear of the
/// coil's ends, and four turns of the coil 12 mm high cut from it, or kept
/// inside it.
fn block_and_coil(x0: f32, keep_inside: bool) -> (Document, BodyId) {
    let mut doc = Document::new("t");
    let body = doc.create_body(Some("Body".into()));
    let mut block = polyline_sketch(
        wb_sketch::sketch::SketchPlane::xy(),
        &[[x0, -5.0], [20.0, -5.0], [20.0, 25.0], [x0, 25.0]],
    );
    let corners: Vec<_> = block
        .sketch
        .geometry
        .iter()
        .filter_map(|e| match e {
            GeometryElement::Point(p) => Some(p.id),
            _ => None,
        })
        .collect();
    block
        .sketch
        .add_geometry(GeometryElement::Line(Line::new(corners[3], corners[0])));
    let block = doc
        .add_feature_in_body(block, "block".into(), Some(body))
        .unwrap();
    doc.add_feature_in_body(
        pad_feature(block, 40.0, false, true),
        "Block".into(),
        Some(body),
    )
    .unwrap();
    let coil = doc
        .add_feature_in_body(coil_section(), "coil".into(), Some(body))
        .unwrap();
    let mut helix = growing_helix(coil, 12.0, 4.0, 0.0);
    if let PartFeature::Helix {
        subtractive,
        keep_inside: keep,
        ..
    } = &mut helix
    {
        *subtractive = true;
        *keep = keep_inside;
    }
    doc.add_feature_in_body(helix, "Helix".into(), Some(body))
        .unwrap();
    (doc, body)
}

/// A body's volume read off a fine mesh of it, by the divergence theorem.
fn fine_mesh_volume(doc: &Document, body: BodyId) -> (f64, [f32; 3]) {
    let ops = wb_part::body_build_ops(doc, body).unwrap().ops;
    let fine = TessellationSettings {
        linear_deflection_mode: kernel_api::LinearDeflectionMode::AbsoluteMm,
        chord_tolerance: 0.002,
        angular_tolerance_deg: 2.0,
        ..TessellationSettings::default()
    };
    let built = OgeomKernel::new().execute_solid_chain(&ops, &fine).unwrap();
    let mesh = &built.mesh;
    let mut volume = 0.0;
    for t in mesh.indices.chunks(3) {
        let [a, b, c] = [t[0], t[1], t[2]].map(|i| mesh.positions[i as usize].map(f64::from));
        volume += (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
            + a[2] * (b[0] * c[1] - b[1] * c[0]))
            / 6.0;
    }
    (volume, built.bounds_mm.unwrap().0)
}

/// What of the coil lies at X of at least `x0`: each point of the section,
/// `r` out, spends acos(x0 / r) of every half turn there, over four turns.
fn coil_beyond(x0: f64) -> f64 {
    let steps = 2000;
    (0..steps)
        .map(|i| {
            let r = 5.0 + (i as f64 + 0.5) / steps as f64;
            8.0 * r * (x0 / r).clamp(-1.0, 1.0).acos() / steps as f64
        })
        .sum()
}

/// A subtractive helix kept inside leaves only what it shares with the
/// body; not kept inside, it cuts that away.
#[test]
fn a_subtractive_helix_kept_inside_leaves_what_it_shares() {
    let shared = coil_beyond(-3.0);
    let (doc, body) = block_and_coil(-3.0, true);
    let (volume, min) = fine_mesh_volume(&doc, body);
    assert_near(volume, shared, 2e-3, "kept inside");
    assert!(min[0] > -3.01, "nothing past the block: {}", min[0]);

    let (doc, body) = block_and_coil(-3.0, false);
    let (volume, _) = fine_mesh_volume(&doc, body);
    assert_near(volume, 23.0 * 30.0 * 40.0 - shared, 1e-5, "cut away");
}

/// The measured volume of what a helix keeps inside a body.
#[test]
#[ignore = "kernel: volume_properties reads a helical sweep's walls trimmed by a boolean about 3% out (ogeom-rs#80)"]
fn the_measured_volume_of_a_helix_kept_inside_is_its_own() {
    let (doc, body) = block_and_coil(-3.0, true);
    assert_near(
        built_body(&doc, body).unwrap().0,
        coil_beyond(-3.0),
        2e-3,
        "measured",
    );
}

/// A block whose face holds the helix's axis keeps half of every turn.
#[test]
#[ignore = "kernel: a boolean of a helical sweep with a solid whose face holds the helix axis does not close its shell (ogeom-rs#81)"]
fn a_helix_kept_inside_a_block_on_its_axis_keeps_half_of_it() {
    let (doc, body) = block_and_coil(0.0, true);
    let (volume, _) = fine_mesh_volume(&doc, body);
    assert_near(volume, coil_volume(4.0, 0.0) / 2.0, 2e-3, "half the coil");
}
