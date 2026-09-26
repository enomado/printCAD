//! Datums standing on a solid follow it through the real benches and the
//! kernel: a rebuild finds their references again where they stand in the
//! history, and what is built on them follows.

use core_document::{
    AttachmentOffset, BodyId, DatumAttachment, DatumFeature, DatumShape, Document, DocumentService,
    EdgeAnchor, FaceAnchor, FeatureId, ImportedGeometry, WorkbenchFeature,
};
use kernel_api::{Placement, PrimitiveKind, TessellationSettings};
use kernel_ogeom::OgeomKernel;
use wb_part::PartFeature;
use wb_sketch::SketchFeature;
use wb_sketch::sketch::{GeometryElement, Line, Point, Sketch, Vec2D};

fn registry() -> DocumentService {
    let mut registry = DocumentService::default();
    registry
        .register_workbench(Box::new(wb_sketch::SketchWorkbench::default()))
        .unwrap();
    registry
        .register_workbench(Box::new(wb_part::PartDesignWorkbench::default()))
        .unwrap();
    registry
}

fn cylinder(radius: f64, height: f64) -> PartFeature {
    PartFeature::Primitive {
        kind: PrimitiveKind::Cylinder {
            radius,
            height,
            angle_deg: 360.0,
        },
        placement: Placement::default(),
        subtractive: false,
        refine: false,
    }
}

fn datum(shape: DatumShape, attachment: DatumAttachment) -> DatumFeature {
    DatumFeature {
        shape,
        attachment,
        offset: AttachmentOffset::default(),
    }
}

/// Build every body that asks, taking what the builds find, until nothing
/// is left to build, as the app's recompute loop does. Answers the bounds
/// of `body`'s solid.
fn settle(registry: &DocumentService, doc: &mut Document, body: BodyId) -> ([f32; 3], [f32; 3]) {
    let mut kernel = OgeomKernel::new();
    for _ in 0..8 {
        let jobs = registry.rebuild_jobs(doc);
        if jobs.is_empty() {
            break;
        }
        for job in jobs {
            let plan = job.plan.expect("the history translates");
            let asked: Vec<kernel_api::ChainProbe> = plan.probes.iter().map(|p| p.probe).collect();
            let result = kernel
                .execute_solid_chain_probing(
                    &plan.ops,
                    &TessellationSettings::default(),
                    None,
                    &asked,
                )
                .expect("the body builds");
            doc.store_probe_answers(&plan.probes, &result.probes);
            doc.set_imported_brep_data(job.body, result.brep_blob, Vec::new());
            doc.set_imported_geometry(
                job.body,
                ImportedGeometry {
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
    assert!(registry.rebuild_jobs(doc).is_empty(), "the builds settle");
    doc.imported_geometry(body)
        .and_then(|g| g.bounds_mm)
        .expect("the body has a solid")
}

fn frame_of(doc: &Document, id: FeatureId) -> core_document::DatumFrame {
    DatumFeature::from_json(doc.feature_values(id).unwrap())
        .unwrap()
        .frame()
}

fn near(a: [f32; 3], b: [f32; 3]) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() < 1e-3)
}

fn rect_sketch(width: f32, height: f32) -> SketchFeature {
    let mut sketch = Sketch::new("s");
    let corners = [(0.0, 0.0), (width, 0.0), (width, height), (0.0, height)]
        .map(|(x, y)| sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(x, y)))));
    for i in 0..4 {
        sketch.add_geometry(GeometryElement::Line(Line::new(
            corners[i],
            corners[(i + 1) % 4],
        )));
    }
    let plane = sketch.plane;
    SketchFeature::new(sketch, plane)
}

/// A plane tangent to a cylinder, a sketch on it and a pad on the sketch:
/// when the cylinder grows, the plane stays tangent and the pad goes with
/// it.
#[test]
fn a_tangent_plane_and_the_pad_on_it_follow_the_cylinder_s_radius() {
    let registry = registry();
    let mut doc = Document::new("t");
    let body = doc.create_body(Some("Body".into()));
    let base = doc
        .add_feature_in_body(cylinder(5.0, 10.0), "Cylinder".into(), Some(body))
        .unwrap();
    let tangent = doc
        .add_feature_in_body(
            datum(
                DatumShape::Plane { size: 20.0 },
                DatumAttachment::Face {
                    face: FaceAnchor {
                        point: [0.0, 5.0, 5.0],
                        normal: [0.0, 1.0, 0.0],
                        surface: None,
                        follows: true,
                    },
                },
            ),
            "Tangent".into(),
            Some(body),
        )
        .unwrap();
    // A 4 x 2 rectangle on the plane: along its x (the cylinder's axis)
    // and its y (across it), padded 3 out along its normal.
    let mut sketch = rect_sketch(4.0, 2.0);
    sketch.support = Some(wb_sketch::DatumSupport {
        datum: tangent,
        plane: None,
        offset: 0.0,
    });
    let sketch = doc
        .add_feature_in_body(sketch, "Sketch".into(), Some(body))
        .unwrap();
    doc.add_feature_in_body(
        PartFeature::Pad {
            refine: false,
            sketch: Some(sketch),
            length: 3.0,
            reversed: false,
            symmetric: false,
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
        },
        "Pad".into(),
        Some(body),
    )
    .unwrap();

    let (_, max) = settle(&registry, &mut doc, body);
    let frame = frame_of(&doc, tangent);
    assert!(near(frame.origin, [0.0, 5.0, 5.0]), "{frame:?}");
    assert!(near(frame.normal, [0.0, 1.0, 0.0]), "{frame:?}");
    assert!((max[1] - 8.0).abs() < 1e-3, "the pad stands on it: {max:?}");

    doc.update_feature_data(base, cylinder(8.0, 10.0).to_json())
        .unwrap();
    doc.mark_feature_dirty(base);
    let (_, max) = settle(&registry, &mut doc, body);
    let frame = frame_of(&doc, tangent);
    assert!(near(frame.origin, [0.0, 8.0, 5.0]), "followed: {frame:?}");
    assert!(near(frame.normal, [0.0, 1.0, 0.0]), "{frame:?}");
    assert!(
        (max[1] - 11.0).abs() < 1e-3,
        "the pad followed the plane: {max:?}"
    );
    assert!(
        doc.get_feature_meta(tangent).unwrap().error.is_none(),
        "the face was found"
    );
}

/// A point at a rim's centre and a coordinate system on the centre of mass
/// follow the cylinder they stand on as it changes.
#[test]
fn centres_follow_the_solid_they_are_found_on() {
    let registry = registry();
    let mut doc = Document::new("t");
    let body = doc.create_body(Some("Body".into()));
    let base = doc
        .add_feature_in_body(cylinder(5.0, 10.0), "Cylinder".into(), Some(body))
        .unwrap();
    let rim = doc
        .add_feature_in_body(
            datum(
                DatumShape::Point,
                DatumAttachment::CurveCentre {
                    edge: EdgeAnchor {
                        point: [5.0, 0.0, 10.0],
                        direction: [0.0, 1.0, 0.0],
                        ends: None,
                        middle: None,
                        circle: None,
                        follows: true,
                    },
                },
            ),
            "Rim centre".into(),
            Some(body),
        )
        .unwrap();
    let mass = doc
        .add_feature_in_body(
            datum(
                DatumShape::CoordinateSystem { size: 10.0 },
                DatumAttachment::Inertia {
                    centre: [0.0; 3],
                    axes: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                },
            ),
            "Centre of mass".into(),
            Some(body),
        )
        .unwrap();

    settle(&registry, &mut doc, body);
    assert!(near(frame_of(&doc, rim).origin, [0.0, 0.0, 10.0]));
    let centre = frame_of(&doc, mass);
    assert!(near(centre.origin, [0.0, 0.0, 5.0]), "{centre:?}");
    assert!(
        centre.x_axis[2].abs() > 1.0 - 1e-4,
        "this cylinder turns most easily about its own axis: {centre:?}"
    );

    // Wider: the rim's centre is found on the new rim, at the same place.
    doc.update_feature_data(base, cylinder(8.0, 10.0).to_json())
        .unwrap();
    doc.mark_feature_dirty(base);
    settle(&registry, &mut doc, body);
    assert!(
        near(frame_of(&doc, rim).origin, [0.0, 0.0, 10.0]),
        "{:?}",
        frame_of(&doc, rim)
    );
    assert!(doc.get_feature_meta(rim).unwrap().error.is_none());

    // Flatter: the centre of mass drops, and the axis of the least moment
    // turns from the cylinder's own to a diameter.
    doc.update_feature_data(base, cylinder(5.0, 2.0).to_json())
        .unwrap();
    doc.mark_feature_dirty(base);
    settle(&registry, &mut doc, body);
    let centre = frame_of(&doc, mass);
    assert!(near(centre.origin, [0.0, 0.0, 1.0]), "{centre:?}");
    assert!(
        centre.x_axis[2].abs() < 1e-4,
        "a disc turns most easily about a diameter: {centre:?}"
    );
}

fn volume(doc: &Document, body: BodyId) -> f64 {
    let blob = doc.imported_brep_blob(body).expect("the body has a solid");
    OgeomKernel::new()
        .physical_properties(blob)
        .expect("the solid measures")
        .volume_mm3
        .expect("the solid is closed")
}

/// A revolution turns about a datum line along a cylinder's rim, which is
/// the cylinder's axis; when the cylinder moves, the line and the ring
/// turned about it go with it.
#[test]
fn a_revolution_turns_about_a_datum_line_that_follows_a_rim() {
    use std::f64::consts::PI;
    let registry = registry();
    let mut doc = Document::new("t");
    let body = doc.create_body(Some("Body".into()));
    let base = doc
        .add_feature_in_body(cylinder(2.0, 10.0), "Cylinder".into(), Some(body))
        .unwrap();
    // The rim as a pick brings it: its circle known, as `part.datum` and
    // the task fill it in from the body's solid.
    let axis = doc
        .add_feature_in_body(
            datum(
                DatumShape::Line { length: 20.0 },
                DatumAttachment::AlongEdge {
                    edge: EdgeAnchor {
                        point: [2.0, 0.0, 10.0],
                        direction: [0.0, 1.0, 0.0],
                        ends: None,
                        middle: None,
                        circle: Some(core_document::AnchorCircle {
                            center: [0.0, 0.0, 10.0],
                            normal: [0.0, 0.0, 1.0],
                            radius: 2.0,
                        }),
                        follows: true,
                    },
                },
            ),
            "Axis".into(),
            Some(body),
        )
        .unwrap();
    // A 3 x 2 rectangle on XZ, 5 to 8 out along x, turned about the axis.
    let mut ring = rect_sketch(3.0, 2.0);
    let plane = wb_sketch::sketch::SketchPlane::xz();
    ring.sketch.plane = plane;
    ring.plane = plane;
    for element in ring.sketch.geometry.iter_mut() {
        if let GeometryElement::Point(p) = element {
            p.position.x += 5.0;
        }
    }
    let ring = doc
        .add_feature_in_body(ring, "Ring".into(), Some(body))
        .unwrap();
    doc.add_feature_in_body(
        PartFeature::Revolution {
            refine: false,
            sketch: ring,
            angle_deg: 360.0,
            axis: wb_part::RevolveAxis::Datum(axis),
            reversed: false,
            midplane: false,
            second_angle_deg: None,
            mode: wb_part::RevolveMode::Angle,
            up_to_face: None,
        },
        "Revolution".into(),
        Some(body),
    )
    .unwrap();

    settle(&registry, &mut doc, body);
    let frame = frame_of(&doc, axis);
    assert!(near(frame.origin, [0.0, 0.0, 10.0]), "{frame:?}");
    let cylinder_volume = PI * 4.0 * 10.0;
    let want = cylinder_volume + PI * (64.0 - 25.0) * 2.0;
    let got = volume(&doc, body);
    assert!((got - want).abs() < want * 1e-4, "{got} against {want}");

    // Moved 1 along x: the ring turns about x = 1, from 4 to 7 out.
    let mut moved = cylinder(2.0, 10.0);
    if let PartFeature::Primitive { placement, .. } = &mut moved {
        placement.origin = [1.0, 0.0, 0.0];
    }
    doc.update_feature_data(base, moved.to_json()).unwrap();
    doc.mark_feature_dirty(base);
    settle(&registry, &mut doc, body);
    let frame = frame_of(&doc, axis);
    assert!(
        near(frame.origin, [1.0, 0.0, 10.0]),
        "followed: {frame:?} {:?} {:?}",
        doc.get_feature_meta(axis).unwrap().error,
        doc.probed_references(axis)
    );
    let want = cylinder_volume + PI * (49.0 - 16.0) * 2.0;
    let got = volume(&doc, body);
    assert!((got - want).abs() < want * 1e-4, "{got} against {want}");
}
