//! Move handles on the body the Move task has open: an arrow along each
//! world axis to slide it and a ring about each to turn it, drawn about the
//! middle of its box a fixed size on screen.

use core_document::{BodyId, BodyPlacement, ScreenSpaceOverlay, WorkbenchRuntimeContext};
use glam::{Quat, Vec3};

/// How long an arrow and how wide a ring are on screen, pixels.
const ARROW_PX: f32 = 70.0;
const RING_PX: f32 = 45.0;
/// How near the cursor must come to a handle to take it, pixels.
const REACH_PX: f32 = 7.0;
/// Segments a ring is drawn with.
const RING_SEGMENTS: usize = 40;

const AXES: [Vec3; 3] = [Vec3::X, Vec3::Y, Vec3::Z];

/// Which handle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Handle {
    Arrow(usize),
    Ring(usize),
}

/// A handle held: the body where it was when taken, the middle it turns
/// about, and where on the handle the cursor was.
#[derive(Debug, Clone, Copy)]
pub struct Held {
    pub handle: Handle,
    pub start: BodyPlacement,
    pub centre: Vec3,
    /// An arrow: how far along its axis the cursor was; a ring: the
    /// direction from the middle to the cursor, in the ring's plane.
    pub from: Vec3,
}

/// The middle of `body`'s box where it sits.
pub fn centre(ctx: &WorkbenchRuntimeContext, body: BodyId) -> Option<Vec3> {
    let g = ctx.document.imported_geometry(body)?;
    let (lo, hi) = g.bounds_mm.or_else(|| g.mesh.bounds())?;
    Some((Vec3::from_array(lo) + Vec3::from_array(hi)) * 0.5)
}

/// World length that shows `px` pixels long at `at`.
fn world_per(ctx: &WorkbenchRuntimeContext, at: Vec3, px: f32) -> Option<f32> {
    let a = ctx.world_to_viewport(at.to_array())?;
    // Along whichever world axis shows longest, so a view down one axis
    // still measures.
    let longest = AXES
        .iter()
        .filter_map(|axis| {
            let b = ctx.world_to_viewport((at + *axis).to_array())?;
            Some((b.0 - a.0).hypot(b.1 - a.1))
        })
        .fold(0.0f32, f32::max);
    (longest > 1e-6).then(|| px / longest)
}

/// Each handle's drawing, as screen points: an arrow as its two ends, a
/// ring as its polyline.
pub fn shapes(ctx: &WorkbenchRuntimeContext, centre: Vec3) -> Vec<(Handle, Vec<[f32; 2]>)> {
    let (Some(arrow), Some(ring)) = (
        world_per(ctx, centre, ARROW_PX),
        world_per(ctx, centre, RING_PX),
    ) else {
        return Vec::new();
    };
    let screen = |p: Vec3| ctx.world_to_viewport(p.to_array()).map(|(x, y)| [x, y]);
    let mut out = Vec::new();
    for (i, axis) in AXES.iter().enumerate() {
        if let (Some(a), Some(b)) = (screen(centre), screen(centre + *axis * arrow)) {
            out.push((Handle::Arrow(i), vec![a, b]));
        }
        let (u, v) = axis.any_orthonormal_pair();
        let points: Option<Vec<[f32; 2]>> = (0..=RING_SEGMENTS)
            .map(|k| {
                let a = std::f32::consts::TAU * k as f32 / RING_SEGMENTS as f32;
                screen(centre + (u * a.cos() + v * a.sin()) * ring)
            })
            .collect();
        if let Some(points) = points {
            out.push((Handle::Ring(i), points));
        }
    }
    out
}

/// The handle within reach of `at`, arrows first.
pub fn under(ctx: &WorkbenchRuntimeContext, centre: Vec3, at: (f32, f32)) -> Option<Handle> {
    let p = glam::Vec2::new(at.0, at.1);
    let near = |points: &[[f32; 2]]| {
        points
            .windows(2)
            .map(|w| {
                let (a, b) = (glam::Vec2::from(w[0]), glam::Vec2::from(w[1]));
                let ab = b - a;
                let t = if ab.length_squared() > 0.0 {
                    ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                (a + ab * t).distance(p)
            })
            .fold(f32::MAX, f32::min)
    };
    let all = shapes(ctx, centre);
    all.iter()
        .filter(|(h, _)| matches!(h, Handle::Arrow(_)))
        .chain(all.iter().filter(|(h, _)| matches!(h, Handle::Ring(_))))
        .find(|(_, points)| near(points) <= REACH_PX)
        .map(|(h, _)| *h)
}

/// Where the cursor at `at` is on `handle`: along an arrow's axis, or the
/// direction in a ring's plane.
pub fn on(
    ctx: &WorkbenchRuntimeContext,
    handle: Handle,
    centre: Vec3,
    at: (f32, f32),
) -> Option<Vec3> {
    let (origin, dir) = ctx.viewport_to_ray(at)?;
    let (o, d) = (
        Vec3::from_array(origin),
        Vec3::from_array(dir).normalize_or_zero(),
    );
    match handle {
        Handle::Arrow(i) => {
            // The point of the axis nearest the ray.
            let e = AXES[i];
            let w = centre - o;
            let b = e.dot(d);
            let denom = 1.0 - b * b;
            if denom < 1e-6 {
                return None;
            }
            let s = (b * d.dot(w) - e.dot(w)) / denom;
            Some(e * s)
        }
        Handle::Ring(i) => {
            let n = AXES[i];
            let facing = d.dot(n);
            if facing.abs() < 1e-6 {
                return None;
            }
            let t = (centre - o).dot(n) / facing;
            let hit = o + d * t - centre;
            (hit.length() > 1e-6).then(|| hit.normalize())
        }
    }
}

/// The body's placement with the handle dragged from `held.from` to `now`.
pub fn dragged(held: &Held, now: Vec3) -> BodyPlacement {
    match held.handle {
        Handle::Arrow(_) => {
            BodyPlacement::new(held.start.quat(), held.start.offset() + (now - held.from))
        }
        Handle::Ring(i) => {
            let axis = AXES[i];
            let angle = held.from.cross(now).dot(axis).atan2(held.from.dot(now));
            let turn = Quat::from_axis_angle(axis, angle);
            let step = BodyPlacement::new(turn, held.centre - turn * held.centre);
            step.after(&held.start)
        }
    }
}

/// The handles as lines: X red, Y green, Z blue; the one held brighter.
pub fn overlays(
    ctx: &WorkbenchRuntimeContext,
    centre: Vec3,
    held: Option<Handle>,
) -> Vec<ScreenSpaceOverlay> {
    let colors = [
        ctx.sketch_palette.axis_x,
        ctx.sketch_palette.axis_y,
        ctx.sketch_palette.selected,
    ];
    shapes(ctx, centre)
        .into_iter()
        .flat_map(|(handle, points)| {
            let (i, width) = match handle {
                Handle::Arrow(i) => (i, 3.0),
                Handle::Ring(i) => (i, 1.5),
            };
            let width = if held == Some(handle) {
                width + 1.5
            } else {
                width
            };
            points
                .windows(2)
                .map(|w| ScreenSpaceOverlay::new(w[0], w[1], colors[i], width))
                .collect::<Vec<_>>()
        })
        .collect()
}
