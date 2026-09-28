//! Faces keep their names through a rebuild: references that kept a name
//! find their face again after dimensions change, where the point they
//! were picked at no longer lies on it.

use kernel_api::{
    BooleanOp, ChainProbe, ExtrudeTermination, ProbeAnswer, Profile, ProfilePlane, ProfileSegment,
    ProfileWire, ShapeProbe, SolidOp, SweepKind, TessellationSettings, TopoName, naming,
};
use kernel_ogeom::OgeomKernel;

fn plane(z: f64) -> ProfilePlane {
    ProfilePlane {
        origin: [0.0, 0.0, z],
        x_axis: [1.0, 0.0, 0.0],
        y_axis: [0.0, 1.0, 0.0],
        normal: [0.0, 0.0, 1.0],
    }
}

/// A square of side `w` at (`x`, `y`), its sides named 1 to 4.
fn square(x: f64, y: f64, w: f64) -> ProfileWire {
    let p = [[x, y], [x + w, y], [x + w, y + w], [x, y + w]];
    ProfileWire {
        segments: (0..4)
            .map(|i| ProfileSegment::Line {
                start: p[i],
                end: p[(i + 1) % 4],
            })
            .collect(),
        names: (1..=4).map(|i| naming::name_of(&[i])).collect(),
    }
}

fn extrude(termination: ExtrudeTermination) -> SweepKind {
    SweepKind::Extrude {
        termination,
        second_side: None,
        symmetric: false,
        reversed: false,
        taper_deg: 0.0,
        direction: None,
    }
}

/// A 20 mm square block `height` tall.
fn block(height: f64) -> SolidOp {
    SolidOp::Sweep {
        profile: Profile {
            plane: plane(0.0),
            wires: vec![square(0.0, 0.0, 20.0)],
        },
        kind: extrude(ExtrudeTermination::Blind { distance: height }),
        op: BooleanOp::NewSolid,
    }
}

const TAGS: [TopoName; 2] = [101, 202];

fn build(ops: &[SolidOp], probes: &[ChainProbe]) -> kernel_api::SolidBuildResult {
    OgeomKernel::new()
        .execute_solid_chain_named(ops, &TAGS, &TessellationSettings::default(), None, probes)
        .unwrap()
}

/// The name of the face of `mesh` whose triangles all satisfy `on`.
fn face_name(mesh: &kernel_api::TriMesh, on: impl Fn([f32; 3]) -> bool) -> TopoName {
    for (face, name) in mesh.face_names.iter().enumerate() {
        let points: Vec<[f32; 3]> = mesh
            .indices
            .chunks(3)
            .zip(&mesh.faces)
            .filter(|(_, f)| **f as usize == face)
            .flat_map(|(t, _)| t.iter().map(|i| mesh.positions[*i as usize]))
            .collect();
        if !points.is_empty() && points.iter().all(|p| on(*p)) {
            return *name;
        }
    }
    panic!("no such face")
}

#[test]
fn every_face_is_named_and_the_names_hold_across_builds() {
    let a = build(&[block(10.0)], &[]);
    let b = build(&[block(10.0)], &[]);
    assert!(a.mesh.face_names.iter().all(|n| *n != 0));
    assert_eq!(a.mesh.face_names, b.mesh.face_names, "the same every time");
    let mut unique = a.mesh.face_names.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), 6, "six faces, six names");
    // A taller block names its faces alike: the walls by their sketch
    // segments, the top and bottom by where they stand.
    let tall = build(&[block(25.0)], &[]);
    let mut a_sorted = a.mesh.face_names.clone();
    a_sorted.sort_unstable();
    let mut tall_sorted = tall.mesh.face_names.clone();
    tall_sorted.sort_unstable();
    assert_eq!(a_sorted, tall_sorted);
    assert!(
        a.mesh
            .edge_faces
            .iter()
            .all(|pair| pair.iter().all(|n| *n != 0))
    );
}

/// A pad stopping on the block's top keeps stopping on it when the block
/// grows: the face is found by its name, where the point it was picked at
/// is now 10 mm below it.
#[test]
fn an_up_to_face_follows_its_face_by_name() {
    let first = build(&[block(10.0)], &[]);
    let top = face_name(&first.mesh, |p| (p[2] - 10.0).abs() < 1e-3);
    // A 5 mm square from below the block, up to its top.
    let post = |name: TopoName| SolidOp::Sweep {
        profile: Profile {
            plane: plane(-10.0),
            wires: vec![square(30.0, 0.0, 5.0)],
        },
        kind: extrude(ExtrudeTermination::UpToFace {
            point: [10.0, 10.0, 10.0],
            normal: [0.0, 0.0, 1.0],
            offset: 0.0,
            name,
        }),
        op: BooleanOp::Fuse,
    };
    let height = |result: &kernel_api::SolidBuildResult| {
        result
            .mesh
            .positions
            .iter()
            .filter(|p| p[0] > 29.0)
            .map(|p| p[2])
            .fold(f32::MIN, f32::max)
    };
    let short = build(&[block(10.0), post(top)], &[]);
    assert!((height(&short) - 10.0).abs() < 1e-3, "{}", height(&short));
    let tall = build(&[block(20.0), post(top)], &[]);
    assert!(
        (height(&tall) - 20.0).abs() < 1e-3,
        "by name: {}",
        height(&tall)
    );
    // The point alone stops on the plane it was picked on.
    let unnamed = build(&[block(20.0), post(0)], &[]);
    assert!(
        (height(&unnamed) - 10.0).abs() < 1e-3,
        "{}",
        height(&unnamed)
    );
}

/// A reference standing on the block's top (a datum on it) is answered
/// where the top is now.
#[test]
fn a_face_probe_finds_its_face_by_name() {
    let first = build(&[block(10.0)], &[]);
    let top = face_name(&first.mesh, |p| (p[2] - 10.0).abs() < 1e-3);
    let probe = |name| ChainProbe {
        after_op: 1,
        probe: ShapeProbe::Face {
            point: [5.0, 5.0, 10.0],
            normal: [0.0, 0.0, 1.0],
            name,
        },
    };
    let answered = build(&[block(30.0)], &[probe(top)]);
    let Ok(ProbeAnswer::Face { point, normal, .. }) = &answered.probes[0] else {
        panic!("{:?}", answered.probes[0]);
    };
    assert!((point[2] - 30.0).abs() < 1e-6 && (normal[2] - 1.0).abs() < 1e-9);
}

/// A shell opened at the block's top stays open at the top when the block
/// grows: the open face is found by its name.
#[test]
fn a_thickness_opens_its_face_by_name() {
    let first = build(&[block(10.0)], &[]);
    let top = face_name(&first.mesh, |p| (p[2] - 10.0).abs() < 1e-3);
    let shell = |name: TopoName| SolidOp::Thickness {
        value: 1.0,
        open_faces: vec![[10.0, 10.0, 10.0]],
        open_face_names: vec![name],
        inward: true,
        join: kernel_api::ThicknessJoin::Intersection,
    };
    let volume = |ops: &[SolidOp]| {
        OgeomKernel::new()
            .physical_properties(&build(ops, &[]).brep_blob)
            .unwrap()
            .volume_mm3
            .unwrap()
    };
    // Open at the top: the walls and the floor, 1 mm thick.
    let open_top = |h: f64| 400.0 * h - 18.0 * 18.0 * (h - 1.0);
    assert!((volume(&[block(10.0), shell(top)]) - open_top(10.0)).abs() < 0.5);
    assert!((volume(&[block(30.0), shell(top)]) - open_top(30.0)).abs() < 0.5);
}
