//! Surface steps through the whole chain: sheets made from sketch curves
//! and from the body's own edges, sewn into a solid, mirrored; and the steps
//! that wait on the kernel, which fail with a message saying so.

use kernel_api::{
    Continuity, CurveSource, ProfilePlane, ProfileSegment, ProfileWire, SolidBuildResult, SolidOp,
    SurfaceOp, TessellationSettings,
};
use kernel_ogeom::OgeomKernel;
use ogeom::topo::{Model, ShapeType, explore_unique};

fn build(ops: Vec<SurfaceOp>) -> Result<SolidBuildResult, kernel_api::ChainError> {
    let ops: Vec<SolidOp> = ops.into_iter().map(SolidOp::Surface).collect();
    OgeomKernel::new().execute_solid_chain(&ops, &TessellationSettings::default())
}

/// How many faces and solids the result's snapshot holds.
fn census(result: &SolidBuildResult) -> (usize, usize) {
    let mut model = Model::new();
    let text = std::str::from_utf8(&result.brep_blob).unwrap();
    let read = ogeom::io::native::read_into(&mut model, text).unwrap();
    let shape = &read.shapes[0];
    let faces = explore_unique(&model, shape, ShapeType::Face)
        .unwrap()
        .len();
    let solids = explore_unique(&model, shape, ShapeType::Solid)
        .unwrap()
        .len();
    (faces, solids)
}

fn shells(result: &SolidBuildResult) -> usize {
    let mut model = Model::new();
    let text = std::str::from_utf8(&result.brep_blob).unwrap();
    let read = ogeom::io::native::read_into(&mut model, text).unwrap();
    explore_unique(&model, &read.shapes[0], ShapeType::Shell)
        .unwrap()
        .len()
}

fn plane(origin: [f64; 3], x: [f64; 3], y: [f64; 3]) -> ProfilePlane {
    let n = [
        x[1] * y[2] - x[2] * y[1],
        x[2] * y[0] - x[0] * y[2],
        x[0] * y[1] - x[1] * y[0],
    ];
    ProfilePlane {
        origin,
        x_axis: x,
        y_axis: y,
        normal: n,
    }
}

fn xy(z: f64) -> ProfilePlane {
    plane([0.0, 0.0, z], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0])
}

fn sketch(plane: ProfilePlane, segments: Vec<ProfileSegment>) -> CurveSource {
    CurveSource::Sketch {
        plane,
        wire: ProfileWire::new(segments),
    }
}

fn line(a: [f64; 2], b: [f64; 2]) -> ProfileSegment {
    ProfileSegment::Line { start: a, end: b }
}

fn arc(a: [f64; 2], mid: [f64; 2], b: [f64; 2]) -> ProfileSegment {
    ProfileSegment::Arc {
        start: a,
        mid,
        end: b,
    }
}

/// An open chain on the XY plane: a line, then an arc bending on from it.
fn open_chain(z: f64) -> CurveSource {
    sketch(
        xy(z),
        vec![
            line([0.0, 0.0], [10.0, 0.0]),
            arc([10.0, 0.0], [15.0, 5.0], [10.0, 10.0]),
        ],
    )
}

fn square_loop(z: f64, size: f64) -> CurveSource {
    sketch(
        xy(z),
        vec![
            line([0.0, 0.0], [size, 0.0]),
            line([size, 0.0], [size, size]),
            line([size, size], [0.0, size]),
            line([0.0, size], [0.0, 0.0]),
        ],
    )
}

fn bounds(result: &SolidBuildResult) -> ([f32; 3], [f32; 3]) {
    result.bounds_mm.expect("bounds")
}

#[test]
fn an_open_chain_extrudes_into_a_sheet_of_one_face_per_curve() {
    let result = build(vec![SurfaceOp::Extrude {
        curves: vec![open_chain(0.0)],
        direction: [0.0, 0.0, 1.0],
        length: 8.0,
        symmetric: false,
    }])
    .unwrap();
    assert_eq!(census(&result), (2, 0));
    let (lo, hi) = bounds(&result);
    assert!(
        (lo[2] - 0.0).abs() < 1e-3 && (hi[2] - 8.0).abs() < 1e-3,
        "{lo:?} {hi:?}"
    );
    assert!(
        (hi[0] - 15.0).abs() < 1e-2,
        "the arc bulges to x = 15: {hi:?}"
    );
}

#[test]
fn a_symmetric_extrusion_runs_half_each_way() {
    let result = build(vec![SurfaceOp::Extrude {
        curves: vec![open_chain(0.0)],
        direction: [0.0, 0.0, 1.0],
        length: 8.0,
        symmetric: true,
    }])
    .unwrap();
    let (lo, hi) = bounds(&result);
    assert!(
        (lo[2] + 4.0).abs() < 1e-3 && (hi[2] - 4.0).abs() < 1e-3,
        "{lo:?} {hi:?}"
    );
}

#[test]
fn an_open_line_revolves_into_a_cone() {
    let profile = sketch(
        plane([0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        vec![line([5.0, 0.0], [10.0, 10.0])],
    );
    let result = build(vec![SurfaceOp::Revolve {
        curves: vec![profile],
        origin: [0.0; 3],
        axis: [0.0, 0.0, 1.0],
        angle_deg: 360.0,
    }])
    .unwrap();
    assert_eq!(census(&result).1, 0, "a sheet, not a solid");
    let (lo, hi) = bounds(&result);
    assert!(
        (hi[0] - 10.0).abs() < 1e-2 && (lo[0] + 10.0).abs() < 1e-2,
        "{lo:?} {hi:?}"
    );
}

#[test]
fn a_closed_loop_with_a_hole_fills_flat() {
    let outer = square_loop(0.0, 20.0);
    let hole = sketch(
        xy(0.0),
        vec![ProfileSegment::Circle {
            center: [10.0, 10.0],
            radius: 4.0,
        }],
    );
    let result = build(vec![SurfaceOp::PlanarFill {
        curves: vec![outer, hole],
    }])
    .unwrap();
    assert_eq!(census(&result), (1, 0));
}

/// Four walls and two lids, each its own sheet, sew into a closed box: a
/// solid.
#[test]
#[ignore = "kernel: sew does not join a prism's end edges to a coincident face (ogeom-rs#104)"]
fn six_sheets_closing_a_box_sew_into_a_solid() {
    let base = square_loop(0.0, 10.0);
    let result = build(vec![
        SurfaceOp::Extrude {
            curves: vec![base.clone()],
            direction: [0.0, 0.0, 1.0],
            length: 5.0,
            symmetric: false,
        },
        SurfaceOp::PlanarFill { curves: vec![base] },
        SurfaceOp::PlanarFill {
            curves: vec![square_loop(5.0, 10.0)],
        },
        SurfaceOp::Sew,
    ])
    .unwrap();
    assert_eq!(census(&result), (6, 1));
}

#[test]
fn sheets_that_do_not_close_sew_into_a_shell() {
    let result = build(vec![
        SurfaceOp::Extrude {
            curves: vec![open_chain(0.0)],
            direction: [0.0, 0.0, 1.0],
            length: 5.0,
            symmetric: false,
        },
        SurfaceOp::Sew,
    ])
    .unwrap();
    assert_eq!(census(&result), (2, 0));
}

/// A saddle: four lines meeting end to end, on two heights, filled.
fn saddle() -> Vec<CurveSource> {
    let side = |a: [f64; 3], b: [f64; 3]| {
        // A line in its own vertical plane, through both points.
        let x = [b[0] - a[0], b[1] - a[1], 0.0];
        let len = (x[0] * x[0] + x[1] * x[1]).sqrt();
        let x = [x[0] / len, x[1] / len, 0.0];
        sketch(
            plane([a[0], a[1], 0.0], x, [0.0, 0.0, 1.0]),
            vec![line([0.0, a[2]], [len, b[2]])],
        )
    };
    vec![
        side([0.0, 0.0, 0.0], [10.0, 0.0, 4.0]),
        side([10.0, 0.0, 4.0], [10.0, 10.0, 0.0]),
        side([10.0, 10.0, 0.0], [0.0, 10.0, 4.0]),
        side([0.0, 10.0, 4.0], [0.0, 0.0, 0.0]),
    ]
}

#[test]
fn four_curves_meeting_end_to_end_fill() {
    let result = build(vec![SurfaceOp::Fill {
        boundary: saddle(),
        continuity: Continuity::G0,
    }])
    .unwrap();
    assert_eq!(census(&result), (1, 0));
    let (lo, hi) = bounds(&result);
    assert!(lo[2] > -0.1 && hi[2] < 4.1, "{lo:?} {hi:?}");
}

#[test]
fn a_ruled_surface_spans_two_curves() {
    let result = build(vec![SurfaceOp::Ruled {
        first: open_chain(0.0),
        second: sketch(xy(6.0), vec![line([0.0, 0.0], [12.0, 6.0])]),
    }])
    .unwrap();
    assert_eq!(census(&result), (1, 0));
    let (lo, hi) = bounds(&result);
    assert!(
        (lo[2]).abs() < 1e-2 && (hi[2] - 6.0).abs() < 1e-2,
        "{lo:?} {hi:?}"
    );
}

#[test]
fn a_loft_runs_through_open_sections_in_order() {
    let section =
        |z: f64, bulge: f64| sketch(xy(z), vec![arc([0.0, 0.0], [5.0, bulge], [10.0, 0.0])]);
    let result = build(vec![SurfaceOp::Loft {
        sections: vec![section(0.0, 2.0), section(5.0, 5.0), section(10.0, 1.0)],
        closed: false,
    }])
    .unwrap();
    let (lo, hi) = bounds(&result);
    assert!((hi[2] - 10.0).abs() < 1e-2 && hi[1] > 4.5, "{lo:?} {hi:?}");
}

#[test]
fn a_straight_path_sweeps_exactly() {
    let path = sketch(
        plane([0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        vec![line([0.0, 0.0], [0.0, 7.0])],
    );
    let result = build(vec![SurfaceOp::Sweep {
        profile: vec![open_chain(0.0)],
        path: vec![path],
        frame: Default::default(),
    }])
    .unwrap();
    assert_eq!(census(&result), (2, 0));
    assert!((bounds(&result).1[2] - 7.0).abs() < 1e-3);
}

/// An edge of the body's own sheet, picked, is extruded on from it.
#[test]
fn a_picked_edge_of_the_body_extrudes() {
    let first = SurfaceOp::Extrude {
        curves: vec![sketch(xy(0.0), vec![line([0.0, 0.0], [10.0, 0.0])])],
        direction: [0.0, 0.0, 1.0],
        length: 5.0,
        symmetric: false,
    };
    // The sheet's top edge, at z = 5.
    let top = CurveSource::Edge(kernel_api::EdgeProbe {
        point: [5.0, 0.0, 5.0],
        direction: [1.0, 0.0, 0.0],
        faces: [0, 0],
    });
    let result = build(vec![
        first,
        SurfaceOp::Extrude {
            curves: vec![top],
            direction: [0.0, 1.0, 0.0],
            length: 3.0,
            symmetric: false,
        },
    ])
    .unwrap();
    assert_eq!(census(&result), (2, 0));
    let (_, hi) = bounds(&result);
    assert!(
        (hi[1] - 3.0).abs() < 1e-3 && (hi[2] - 5.0).abs() < 1e-3,
        "{hi:?}"
    );
}

#[test]
fn a_mirror_keeps_the_sheet_and_adds_its_image() {
    let result = build(vec![
        SurfaceOp::Extrude {
            curves: vec![open_chain(0.0)],
            direction: [0.0, 0.0, 1.0],
            length: 5.0,
            symmetric: false,
        },
        SurfaceOp::Mirror {
            origin: [0.0; 3],
            normal: [1.0, 0.0, 0.0],
        },
    ])
    .unwrap();
    assert_eq!(census(&result), (4, 0));
    let (lo, hi) = bounds(&result);
    assert!(
        (lo[0] + 15.0).abs() < 1e-2 && (hi[0] - 15.0).abs() < 1e-2,
        "{lo:?} {hi:?}"
    );
}

#[test]
fn a_step_on_nothing_says_why() {
    let sew = build(vec![SurfaceOp::Sew]).unwrap_err();
    assert_eq!(sew.op_index, 0);
}

/// An open square tray: a floor and four walls, each its own sheet.
fn tray() -> Vec<SurfaceOp> {
    vec![
        SurfaceOp::PlanarFill {
            curves: vec![square_loop(0.0, 10.0)],
        },
        SurfaceOp::Extrude {
            curves: vec![square_loop(0.0, 10.0)],
            direction: [0.0, 0.0, 1.0],
            length: 5.0,
            symmetric: false,
        },
    ]
}

/// A corner patch: three quarter circles meeting end to end.
fn three_sided() -> Vec<CurveSource> {
    let quarter = |plane: ProfilePlane| {
        sketch(
            plane,
            vec![ProfileSegment::Arc {
                start: [10.0, 0.0],
                mid: [7.071_067_811_865_475, 7.071_067_811_865_475],
                end: [0.0, 10.0],
            }],
        )
    };
    vec![
        quarter(plane([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0])),
        quarter(plane([0.0; 3], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0])),
        quarter(plane([0.0; 3], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0])),
    ]
}

#[test]
#[ignore = "kernel: a fill takes exactly four edges (ogeom-rs#108)"]
fn three_curves_meeting_end_to_end_fill() {
    let result = build(vec![SurfaceOp::Fill {
        boundary: three_sided(),
        continuity: Continuity::G0,
    }])
    .unwrap();
    assert_eq!(census(&result), (1, 0));
}

#[test]
#[ignore = "kernel: a fill meets its neighbours G0 only, and comes with no boundary edges to sew (ogeom-rs#108)"]
fn a_tangent_fill_closes_a_tray_into_a_solid() {
    let mut ops = tray();
    let top = |a: [f64; 3], b: [f64; 3]| {
        CurveSource::Edge(kernel_api::EdgeProbe {
            point: [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0, 5.0],
            direction: [b[0] - a[0], b[1] - a[1], 0.0],
            faces: [0, 0],
        })
    };
    ops.push(SurfaceOp::Fill {
        boundary: vec![
            top([0.0, 0.0, 5.0], [10.0, 0.0, 5.0]),
            top([10.0, 0.0, 5.0], [10.0, 10.0, 5.0]),
            top([10.0, 10.0, 5.0], [0.0, 10.0, 5.0]),
            top([0.0, 10.0, 5.0], [0.0, 0.0, 5.0]),
        ],
        continuity: Continuity::G1,
    });
    ops.push(SurfaceOp::Sew);
    let result = build(ops).unwrap();
    assert_eq!(census(&result).1, 1, "the tray and its lid close");
}

#[test]
#[ignore = "kernel: sew joins only edges that coincide within the model's tolerance (ogeom-rs#105)"]
fn sheets_a_hair_apart_sew() {
    let near = sketch(xy(0.0), vec![line([10.05, 0.0], [20.0, 0.0])]);
    let result = build(vec![
        SurfaceOp::Extrude {
            curves: vec![sketch(xy(0.0), vec![line([0.0, 0.0], [10.0, 0.0])])],
            direction: [0.0, 1.0, 0.0],
            length: 5.0,
            symmetric: false,
        },
        SurfaceOp::Extrude {
            curves: vec![near],
            direction: [0.0, 1.0, 0.0],
            length: 5.0,
            symmetric: false,
        },
        SurfaceOp::Sew,
    ])
    .unwrap();
    assert_eq!(shells(&result), 1, "the strips join across the gap");
}

#[test]
#[ignore = "kernel: a sheet cannot be thickened into a solid (ogeom-rs#111)"]
fn a_sheet_thickens_into_a_solid() {
    let result = build(vec![
        SurfaceOp::PlanarFill {
            curves: vec![square_loop(0.0, 10.0)],
        },
        SurfaceOp::Thicken {
            thickness: 2.0,
            both_sides: false,
        },
    ])
    .unwrap();
    assert_eq!(census(&result), (6, 1));
}

#[test]
#[ignore = "kernel: a sheet's faces cannot be offset (ogeom-rs#111)"]
fn a_sheet_offsets() {
    let result = build(vec![
        SurfaceOp::Fill {
            boundary: saddle(),
            continuity: Continuity::G0,
        },
        SurfaceOp::Offset {
            faces: vec![kernel_api::FaceProbe {
                point: [5.0, 5.0, 2.0],
                normal: [0.0, 0.0, 1.0],
                name: 0,
            }],
            distance: 1.0,
        },
    ])
    .unwrap();
    assert_eq!(census(&result), (2, 0));
}

#[test]
#[ignore = "kernel: booleans refuse sheets, so nothing trims one by a plane (ogeom-rs#106)"]
fn a_sheet_trims_by_a_plane() {
    let result = build(vec![
        SurfaceOp::PlanarFill {
            curves: vec![square_loop(0.0, 10.0)],
        },
        SurfaceOp::TrimByPlane {
            origin: [5.0, 0.0, 0.0],
            normal: [-1.0, 0.0, 0.0],
        },
    ])
    .unwrap();
    assert!((bounds(&result).1[0] - 5.0).abs() < 1e-3);
}

#[test]
#[ignore = "kernel: nothing splits a face along a curve (ogeom-rs#107)"]
fn a_face_splits_along_a_curve() {
    let result = build(vec![
        SurfaceOp::PlanarFill {
            curves: vec![square_loop(0.0, 10.0)],
        },
        SurfaceOp::Split {
            faces: vec![kernel_api::FaceProbe {
                point: [5.0, 5.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                name: 0,
            }],
            curves: vec![sketch(xy(0.0), vec![line([0.0, 5.0], [10.0, 5.0])])],
        },
    ])
    .unwrap();
    assert_eq!(census(&result), (2, 0));
}

#[test]
#[ignore = "kernel: a face cannot be extended past its edge (ogeom-rs#112)"]
fn a_face_extends_past_its_edge() {
    let result = build(vec![
        SurfaceOp::PlanarFill {
            curves: vec![square_loop(0.0, 10.0)],
        },
        SurfaceOp::Extend {
            edges: vec![kernel_api::EdgeProbe {
                point: [10.0, 5.0, 0.0],
                direction: [0.0, 1.0, 0.0],
                faces: [0, 0],
            }],
            length: 5.0,
            continuity: Continuity::G0,
        },
    ])
    .unwrap();
    assert!((bounds(&result).1[0] - 15.0).abs() < 1e-3);
}

#[test]
#[ignore = "kernel: no blend surface between two edges (ogeom-rs#110)"]
fn two_strips_blend() {
    let strip = |y: f64, z: f64| SurfaceOp::Extrude {
        curves: vec![sketch(xy(z), vec![line([0.0, y], [10.0, y])])],
        direction: [0.0, 1.0, 0.0],
        length: 5.0,
        symmetric: false,
    };
    let edge = |y: f64, z: f64| kernel_api::EdgeProbe {
        point: [5.0, y, z],
        direction: [1.0, 0.0, 0.0],
        faces: [0, 0],
    };
    let result = build(vec![
        strip(0.0, 0.0),
        strip(15.0, 6.0),
        SurfaceOp::Blend {
            first: edge(5.0, 0.0),
            second: edge(15.0, 6.0),
            continuity: Continuity::G1,
        },
    ])
    .unwrap();
    assert_eq!(census(&result), (3, 0));
}

#[test]
#[ignore = "kernel: sweeps build closed solids only, so an open profile does not sweep along a curve (ogeom-rs#109)"]
fn an_open_profile_sweeps_along_a_curve() {
    let path = sketch(
        plane([0.0; 3], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        vec![arc([0.0, 0.0], [5.0, 2.0], [10.0, 0.0])],
    );
    let profile = sketch(
        plane([0.0; 3], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]),
        vec![line([0.0, 0.0], [3.0, 0.0]), line([3.0, 0.0], [3.0, 3.0])],
    );
    let result = build(vec![SurfaceOp::Sweep {
        profile: vec![profile],
        path: vec![path],
        frame: Default::default(),
    }])
    .unwrap();
    assert_eq!(census(&result), (2, 0));
}

#[test]
#[ignore = "kernel: a lofted surface cannot close on itself (ogeom-rs#109)"]
fn a_closed_loft_comes_back_to_its_first_section() {
    let ring = |z: f64, r: f64| {
        sketch(
            xy(z),
            vec![ProfileSegment::Circle {
                center: [0.0, 0.0],
                radius: r,
            }],
        )
    };
    let result = build(vec![SurfaceOp::Loft {
        sections: vec![ring(0.0, 5.0), ring(5.0, 8.0), ring(10.0, 5.0)],
        closed: true,
    }])
    .unwrap();
    assert_eq!(census(&result).1, 0);
}

#[test]
#[ignore = "kernel: STEP export writes solids only (ogeom-rs#113)"]
fn a_sheet_exports_as_step() {
    let result = build(vec![SurfaceOp::Extrude {
        curves: vec![open_chain(0.0)],
        direction: [0.0, 0.0, 1.0],
        length: 5.0,
        symmetric: false,
    }])
    .unwrap();
    let body = kernel_ogeom::export::ExportBody {
        name: "sheet".into(),
        brep: Some(&result.brep_blob),
        transform: None,
        mesh: &result.mesh,
        finish: None,
    };
    let written = kernel_ogeom::export::export(
        &[body],
        kernel_ogeom::export::ExportFormat::Step,
        &TessellationSettings::default(),
    )
    .unwrap();
    assert_eq!(written.written, 1, "skipped: {:?}", written.skipped);
}

/// Where faces meet, measured: an L's two walls meet at a right angle, a
/// line running on into a tangent arc meets it smoothly.
#[test]
fn continuity_tells_a_crease_from_a_tangent_join() {
    use kernel_api::KernelQueries;
    let walls = |segments| {
        build(vec![SurfaceOp::Extrude {
            curves: vec![sketch(xy(0.0), segments)],
            direction: [0.0, 0.0, 1.0],
            length: 5.0,
            symmetric: false,
        }])
        .unwrap()
    };
    let l = walls(vec![
        line([0.0, 0.0], [10.0, 0.0]),
        line([10.0, 0.0], [10.0, 10.0]),
    ]);
    let joins = kernel_ogeom::QUERIES.continuity(&l.brep_blob).unwrap();
    assert_eq!(joins.len(), 1, "{joins:?}");
    assert!((joins[0].angle_deg - 90.0).abs() < 0.5, "{joins:?}");
    assert!(joins[0].gap < 1e-6);
    assert!((joins[0].point[0] - 10.0).abs() < 1e-6 && (joins[0].point[2] - 2.5).abs() < 1e-6);

    // The arc leaves (10, 0) heading along +x, as the line does.
    let smooth = walls(vec![
        line([0.0, 0.0], [10.0, 0.0]),
        arc(
            [10.0, 0.0],
            [13.535_533_905_932_737, 1.464_466_094_067_262_4],
            [15.0, 5.0],
        ),
    ]);
    let joins = kernel_ogeom::QUERIES.continuity(&smooth.brep_blob).unwrap();
    assert_eq!(joins.len(), 1, "{joins:?}");
    assert!(joins[0].angle_deg < 0.5, "{joins:?}");
}

/// Walls with a floor sewn on: every edge two faces share meets with no
/// gap.
#[test]
#[ignore = "kernel: a prism's end edges are matched as the profile's edges they were moved from (ogeom-rs#104)"]
fn a_sewn_floor_meets_its_walls_with_no_gap() {
    use kernel_api::KernelQueries;
    let result = build(vec![
        SurfaceOp::Extrude {
            curves: vec![square_loop(0.0, 10.0)],
            direction: [0.0, 0.0, 1.0],
            length: 5.0,
            symmetric: false,
        },
        SurfaceOp::PlanarFill {
            curves: vec![square_loop(0.0, 10.0)],
        },
        SurfaceOp::Sew,
    ])
    .unwrap();
    let joins = kernel_ogeom::QUERIES.continuity(&result.brep_blob).unwrap();
    assert_eq!(
        joins.len(),
        8,
        "four corners and four floor edges: {joins:?}"
    );
    assert!(joins.iter().all(|j| j.gap < 1e-6), "{joins:?}");
}
