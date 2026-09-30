use std::collections::HashMap;

use super::*;

/// A 10 mm cube from the origin, each face its own points, as the kernel
/// meshes one: face 0 bottom, 1 top, 2..=5 the sides.
fn cube() -> TriMesh {
    let mut mesh = TriMesh::default();
    let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
        (
            [0.0, 0.0, -1.0],
            [[0., 0., 0.], [0., 10., 0.], [10., 10., 0.], [10., 0., 0.]],
        ),
        (
            [0.0, 0.0, 1.0],
            [
                [0., 0., 10.],
                [10., 0., 10.],
                [10., 10., 10.],
                [0., 10., 10.],
            ],
        ),
        (
            [0.0, -1.0, 0.0],
            [[0., 0., 0.], [10., 0., 0.], [10., 0., 10.], [0., 0., 10.]],
        ),
        (
            [1.0, 0.0, 0.0],
            [
                [10., 0., 0.],
                [10., 10., 0.],
                [10., 10., 10.],
                [10., 0., 10.],
            ],
        ),
        (
            [0.0, 1.0, 0.0],
            [
                [10., 10., 0.],
                [0., 10., 0.],
                [0., 10., 10.],
                [10., 10., 10.],
            ],
        ),
        (
            [-1.0, 0.0, 0.0],
            [[0., 10., 0.], [0., 0., 0.], [0., 0., 10.], [0., 10., 10.]],
        ),
    ];
    for (id, (normal, corners)) in faces.iter().enumerate() {
        let base = mesh.positions.len() as u32;
        mesh.positions.extend_from_slice(corners);
        mesh.normals.extend_from_slice(&[*normal; 4]);
        mesh.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        mesh.faces.extend_from_slice(&[id as u32, id as u32]);
    }
    mesh
}

/// Every edge of the mesh, its points welded by position, shared by
/// exactly two triangles: the mesh is closed, no crack in it.
fn closed(mesh: &TriMesh) -> bool {
    let key = |i: u32| {
        let p = Vec3::from_array(mesh.positions[i as usize]);
        (p * 1.0e4).round().as_ivec3()
    };
    let mut count: HashMap<(glam::IVec3, glam::IVec3), usize> = HashMap::new();
    for t in mesh.indices.as_chunks::<3>().0.iter() {
        for k in 0..3 {
            let (a, b) = (key(t[k]), key(t[(k + 1) % 3]));
            let e = if (a.x, a.y, a.z) < (b.x, b.y, b.z) {
                (a, b)
            } else {
                (b, a)
            };
            *count.entry(e).or_default() += 1;
        }
    }
    count.values().all(|n| *n == 2)
}

fn job<'a>(texture: &'a Texture, faces: &'a [u32]) -> Vec<Job<'a>> {
    vec![Job {
        texture,
        faces,
        image: None,
    }]
}

/// Only the top is textured: it rises by no more than the depth, the rest
/// does not move, and the mesh is closed.
#[test]
fn a_textured_face_rises_and_the_rest_stays() {
    let texture = Texture {
        tile_mm: 2.0,
        depth_mm: 0.5,
        ..Texture::default()
    };
    let out = apply(&cube(), &job(&texture, &[1]), Detail::PREVIEW);
    assert!(closed(&out), "no crack");
    assert!(out.indices.len() / 3 > 100, "the top was split");
    let mut raised = 0;
    for (t, face) in out.indices.as_chunks::<3>().0.iter().zip(&out.faces) {
        for &i in t {
            let p = out.positions[i as usize];
            if *face == 1 {
                assert!((10.0 - 1e-4..=10.5 + 1e-4).contains(&p[2]), "{p:?}");
                raised += usize::from(p[2] > 10.01);
            } else {
                // A side or the bottom keeps to its plane.
                let on_plane = p.iter().any(|v| v.abs() < 1e-4 || (v - 10.0).abs() < 1e-4);
                assert!(on_plane, "face {face} moved: {p:?}");
            }
        }
    }
    assert!(raised > 50, "the pattern shows: {raised}");
    assert_eq!(out.faces.len(), out.indices.len() / 3);
}

/// Inward, the pattern goes into the solid.
#[test]
fn an_inward_texture_digs_in() {
    let texture = Texture {
        tile_mm: 2.0,
        depth_mm: 0.5,
        inward: true,
        ..Texture::default()
    };
    let out = apply(&cube(), &job(&texture, &[1]), Detail::PREVIEW);
    let top_z: Vec<f32> = out
        .indices
        .as_chunks::<3>()
        .0
        .iter()
        .zip(&out.faces)
        .filter(|(_, f)| **f == 1)
        .flat_map(|(t, _)| t.iter().map(|i| out.positions[*i as usize][2]))
        .collect();
    assert!(top_z.iter().all(|z| (9.5 - 1e-4..=10.0 + 1e-4).contains(z)));
    assert!(top_z.iter().any(|z| *z < 9.99));
}

/// Kept flat, the top and bottom stay smooth while the sides take the
/// pattern.
#[test]
fn flat_faces_can_stay_smooth() {
    let texture = Texture {
        tile_mm: 2.0,
        depth_mm: 0.5,
        keep_flat_deg: 10.0,
        ..Texture::default()
    };
    let out = apply(&cube(), &job(&texture, &[]), Detail::PREVIEW);
    assert!(closed(&out));
    for (t, face) in out.indices.as_chunks::<3>().0.iter().zip(&out.faces) {
        for &i in t {
            let z = out.positions[i as usize][2];
            match face {
                0 => assert!(z.abs() < 1e-4),
                1 => assert!((z - 10.0).abs() < 1e-4),
                _ => {}
            }
        }
    }
    let side_moved = out.positions.iter().any(|p| p[0] > 10.01 || p[0] < -0.01);
    assert!(side_moved, "the sides take the pattern");
}

/// However fine the pattern, the triangles stay under the cap.
#[test]
fn the_triangle_cap_holds() {
    let texture = Texture {
        tile_mm: 0.05,
        ..Texture::default()
    };
    let detail = Detail {
        max_triangles: 20_000,
        ..Detail::EXPORT
    };
    let out = apply(&cube(), &job(&texture, &[]), detail);
    assert!(out.indices.len() / 3 <= 20_000, "{}", out.indices.len() / 3);
    assert!(closed(&out));
}

/// No texture, nothing changes.
#[test]
fn no_job_leaves_the_mesh_as_it_is() {
    let mesh = cube();
    let out = apply(&mesh, &[], Detail::PREVIEW);
    assert_eq!(out.positions, mesh.positions);
}
