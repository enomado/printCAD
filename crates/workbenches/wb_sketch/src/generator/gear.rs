//! An involute spur gear: module, teeth and pressure angle, with a
//! profile shift, clearance, backlash, a root fillet and a bore.
//!
//! Each flank is the involute of the base circle from where the root
//! fillet meets it to the tip circle, as a B-spline within
//! [`FLANK_TOLERANCE`] of the true curve; below the base circle, where no
//! involute exists, the flank runs radially down to the fillet. The tip
//! and root are arcs of their circles. The tooth is not undercut the way a
//! cutter would leave it on a pinion of few teeth.

use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

use super::{Edge, Loop, Outline, P2, add, fit, norm, polar, rotate, scale};

/// How far a flank's spline may stray from the involute, mm.
pub const FLANK_TOLERANCE: f64 = 0.001;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GearSpec {
    /// Pitch diameter over teeth, mm.
    pub module: f32,
    #[serde(deserialize_with = "super::count")]
    pub teeth: u32,
    pub pressure_angle_deg: f32,
    /// Profile shift coefficient: the tooth moved out by this many modules.
    pub profile_shift: f32,
    /// Root clearance coefficient: the root sits this many modules below
    /// the mating tip.
    pub clearance: f32,
    /// How much thinner each tooth is than half the circular pitch at the
    /// pitch circle, mm.
    pub backlash: f32,
    /// Root fillet radius coefficient, in modules; 0 leaves a sharp root.
    pub root_fillet: f32,
    /// Bore diameter, mm; 0 leaves the gear solid.
    pub bore: f32,
}

impl Default for GearSpec {
    fn default() -> Self {
        Self {
            module: 2.0,
            teeth: 20,
            pressure_angle_deg: 20.0,
            profile_shift: 0.0,
            clearance: 0.25,
            backlash: 0.0,
            root_fillet: 0.38,
            bore: 5.0,
        }
    }
}

/// tan φ − φ.
pub fn involute_function(phi: f64) -> f64 {
    phi.tan() - phi
}

/// The measured circles and angles of a gear, in millimetres and radians.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GearGeometry {
    pub pitch_radius: f64,
    pub base_radius: f64,
    pub tip_radius: f64,
    pub root_radius: f64,
    /// Half the tooth's angular thickness at the pitch circle.
    pub half_thickness: f64,
    pub pressure_angle: f64,
    pub teeth: u32,
}

impl GearGeometry {
    /// Half the tooth's angular thickness at radius `r` on the involute
    /// (`r` at least the base radius); below it, where the flank runs
    /// radially, the angle at the base circle.
    pub fn half_angle(&self, r: f64) -> f64 {
        let r = r.max(self.base_radius);
        let phi = (self.base_radius / r).clamp(-1.0, 1.0).acos();
        self.half_thickness + involute_function(self.pressure_angle) - involute_function(phi)
    }

    /// The left flank of the tooth centred on angle 0 at radius `r`.
    pub fn flank_point(&self, r: f64) -> P2 {
        polar(r, self.half_angle(r))
    }

    /// The flank's unit normal at radius `r`, pointing out of the tooth.
    fn flank_normal(&self, r: f64) -> P2 {
        let a = self.half_angle(r);
        let radial = polar(1.0, a);
        let across = polar(1.0, a + PI / 2.0);
        // The involute leans back by its pressure angle there: tan φ of
        // radial for one of across.
        let t = if r > self.base_radius {
            (r * r - self.base_radius * self.base_radius).sqrt() / self.base_radius
        } else {
            0.0
        };
        let n = add(scale(radial, t), across);
        scale(n, 1.0 / norm(n))
    }
}

impl GearSpec {
    pub fn geometry(&self) -> Result<GearGeometry, String> {
        let m = f64::from(self.module);
        if !(m > 0.0 && m.is_finite()) {
            return Err("the module must be more than zero".into());
        }
        if self.teeth < 3 {
            return Err("a gear needs at least 3 teeth".into());
        }
        let alpha = f64::from(self.pressure_angle_deg).to_radians();
        if !(alpha > 0.0 && alpha < 45f64.to_radians()) {
            return Err("the pressure angle must be between 0° and 45°".into());
        }
        let z = f64::from(self.teeth);
        let x = f64::from(self.profile_shift);
        let r = m * z / 2.0;
        let s = m * (PI / 2.0 + 2.0 * x * alpha.tan()) - f64::from(self.backlash);
        let g = GearGeometry {
            pitch_radius: r,
            base_radius: r * alpha.cos(),
            tip_radius: r + m * (1.0 + x),
            root_radius: r - m * (1.0 + f64::from(self.clearance) - x),
            half_thickness: s / (2.0 * r),
            pressure_angle: alpha,
            teeth: self.teeth,
        };
        if g.root_radius <= 0.0 {
            return Err(
                "the root circle comes to nothing: use more teeth or less clearance".into(),
            );
        }
        if g.root_radius >= g.tip_radius {
            return Err("the root circle is outside the tip circle".into());
        }
        if g.half_angle(g.tip_radius) <= 0.0 {
            return Err(
                "the teeth come to a point below the tip circle: use less profile shift or \
                 backlash"
                    .into(),
            );
        }
        if g.half_angle(g.root_radius) >= PI / z {
            return Err("the teeth meet at the root: use a larger backlash or fewer teeth".into());
        }
        let bore = f64::from(self.bore);
        if bore < 0.0 || bore / 2.0 >= g.root_radius {
            return Err("the bore must fit inside the root circle".into());
        }
        Ok(g)
    }

    pub fn outline(&self) -> Result<Outline, String> {
        let g = self.geometry()?;
        let z = self.teeth as usize;
        let pitch = 2.0 * PI / z as f64;
        let rho = f64::from(self.root_fillet.max(0.0)) * f64::from(self.module);
        let foot = root_foot(&g, rho);

        // The left flank of tooth 0, bottom to top: the fillet's end on
        // the flank, a radial line to the base circle when it is below
        // it, then the involute to the tip.
        let start = foot.flank_radius;
        let involute_from = start.max(g.base_radius);
        let radial = (start < g.base_radius - 1e-9).then(|| g.flank_point(g.base_radius));
        let spline = involute_spline(&g, involute_from, g.tip_radius);

        let mirror = |p: P2| [p[0], -p[1]];
        let tip = *spline.last().expect("poles");
        let inner = &spline[1..spline.len() - 1];
        let root_arc = || Edge::Arc { center: [0.0, 0.0] };
        let mut lp = Loop::default();
        for k in 0..z {
            let turn = pitch * k as f64;
            let right = |p: P2| rotate(mirror(p), turn);
            let left = |p: P2| rotate(p, turn);
            // Each joint with the edge that leaves it: up the right flank
            // from the root, over the tip, down the left flank, and along
            // the root to the next tooth.
            let mut chain: Vec<(P2, Edge)> = Vec::new();
            if let Some(f) = &foot.fillet {
                chain.push((
                    right(f.on_root),
                    Edge::Arc {
                        center: right(f.center),
                    },
                ));
            }
            let right_inner = Edge::Spline {
                inner: inner.iter().map(|p| right(*p)).collect(),
            };
            match radial {
                Some(base) => {
                    chain.push((right(g.flank_point(start)), Edge::Line));
                    chain.push((right(base), right_inner));
                }
                None => chain.push((right(spline[0]), right_inner)),
            }
            chain.push((right(tip), root_arc()));
            chain.push((
                left(tip),
                Edge::Spline {
                    inner: inner.iter().rev().map(|p| left(*p)).collect(),
                },
            ));
            let down = match &foot.fillet {
                Some(f) => Edge::Arc {
                    center: left(f.center),
                },
                None => root_arc(),
            };
            match radial {
                Some(base) => {
                    chain.push((left(base), Edge::Line));
                    chain.push((left(g.flank_point(start)), down));
                }
                None => chain.push((left(spline[0]), down)),
            }
            if let Some(f) = &foot.fillet {
                chain.push((left(f.on_root), root_arc()));
            }
            for (joint, edge) in chain {
                lp.joints.push(joint);
                lp.edges.push(edge);
            }
        }

        let mut outline = Outline {
            loops: vec![lp],
            guides: vec![([0.0, 0.0], g.pitch_radius)],
            ..Outline::default()
        };
        if self.bore > 0.0 {
            outline
                .circles
                .push(([0.0, 0.0], f64::from(self.bore) / 2.0));
        }
        Ok(outline)
    }
}

/// Where the root fillet of the tooth centred on angle 0 sits, on its left
/// flank.
#[derive(Debug, Clone, Copy)]
struct Fillet {
    center: P2,
    /// Where it meets the root circle.
    on_root: P2,
}

struct Foot {
    /// Where the flank begins, as a radius: the fillet's end on it, or the
    /// root circle when there is no fillet.
    flank_radius: f64,
    fillet: Option<Fillet>,
}

/// The root fillet of radius `rho`, tangent to the root circle and the
/// flank; smaller when the gap between teeth is too narrow for it, none
/// when it cannot be made at all.
fn root_foot(g: &GearGeometry, rho: f64) -> Foot {
    let half_gap = PI / f64::from(g.teeth);
    let bare = Foot {
        flank_radius: g.root_radius,
        fillet: None,
    };
    let mut rho = rho;
    for _ in 0..24 {
        if rho < 1e-4 {
            return bare;
        }
        if let Some((r, center)) = fillet_on_flank(g, rho) {
            let angle = center[1].atan2(center[0]);
            // The fillets of both flanks leave some root arc between them.
            if angle < half_gap * 0.98 {
                return Foot {
                    flank_radius: r,
                    fillet: Some(Fillet {
                        center,
                        on_root: polar(g.root_radius, angle),
                    }),
                };
            }
        }
        rho *= 0.7;
    }
    bare
}

/// The radius on the flank where a circle of radius `rho` touches both it
/// and the root circle from the gap's side, and that circle's centre.
fn fillet_on_flank(g: &GearGeometry, rho: f64) -> Option<(f64, P2)> {
    let reach = |r: f64| {
        let c = add(g.flank_point(r), scale(g.flank_normal(r), rho));
        (norm(c) - (g.root_radius + rho), c)
    };
    let (mut lo, mut hi) = (g.root_radius, g.tip_radius);
    if reach(lo).0 >= 0.0 || reach(hi).0 <= 0.0 {
        return None;
    }
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        if reach(mid).0 < 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let r = 0.5 * (lo + hi);
    Some((r, reach(r).1))
}

/// The control points of the left flank's involute of tooth 0 from radius
/// `from` to `to`.
fn involute_spline(g: &GearGeometry, from: f64, to: f64) -> Vec<P2> {
    let rb = g.base_radius;
    // Evenly in the roll angle, which crowds the points near the base
    // circle where the involute bends most.
    let roll = |r: f64| ((r / rb).powi(2) - 1.0).max(0.0).sqrt();
    let (t0, t1) = (roll(from), roll(to));
    let points: Vec<P2> = (0..=240)
        .map(|i| {
            let t = t0 + (t1 - t0) * i as f64 / 240.0;
            g.flank_point(rb * (1.0 + t * t).sqrt())
        })
        .collect();
    // How far a point is from the involute: along the circle through it,
    // which is never less than the distance square to the curve.
    let error = |p: P2| {
        let r = norm(p);
        let angle = p[1].atan2(p[0]);
        if r < rb {
            return (rb - r) + r * (angle - g.half_angle(rb)).abs();
        }
        r * (angle - g.half_angle(r)).abs()
    };
    fit::fit(&points, FLANK_TOLERANCE, 40, error).0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::{Outline, sub};

    fn spec() -> GearSpec {
        GearSpec {
            bore: 0.0,
            ..GearSpec::default()
        }
    }

    #[test]
    fn the_pitch_circle_is_module_times_teeth_over_two() {
        let g = GearSpec {
            module: 1.5,
            teeth: 30,
            ..spec()
        }
        .geometry()
        .unwrap();
        assert!((g.pitch_radius - 22.5).abs() < 1e-12);
        assert!((g.base_radius - 22.5 * 20f64.to_radians().cos()).abs() < 1e-12);
        assert!((g.tip_radius - 24.0).abs() < 1e-12);
        assert!((g.root_radius - (22.5 - 1.25 * 1.5)).abs() < 1e-12);
    }

    #[test]
    fn a_tooth_is_half_the_circular_pitch_thick_at_the_pitch_circle() {
        let g = spec().geometry().unwrap();
        let thickness = 2.0 * g.half_angle(g.pitch_radius) * g.pitch_radius;
        assert!((thickness - PI * 2.0 / 2.0).abs() < 1e-9, "{thickness}");
        // Shifted out, the tooth is thicker by 2 x m tan α.
        let shifted = GearSpec {
            profile_shift: 0.3,
            ..spec()
        }
        .geometry()
        .unwrap();
        let thickness = 2.0 * shifted.half_angle(shifted.pitch_radius) * shifted.pitch_radius;
        let x = f64::from(0.3f32);
        let want = 2.0 * (PI / 2.0 + 2.0 * x * 20f64.to_radians().tan());
        assert!((thickness - want).abs() < 1e-9);
    }

    #[test]
    fn the_flank_lies_on_the_involute_of_the_base_circle() {
        let g = spec().geometry().unwrap();
        let rb = g.base_radius;
        // The involute unwound from the base point at angle a0: every point
        // is its roll angle t from its base point, square to the radius.
        let a0 = g.half_angle(rb);
        for t in [0.1, 0.3, 0.5] {
            let p = g.flank_point(rb * (1.0f64 + t * t).sqrt());
            // The string comes off the base circle at a0 - t, square to it.
            let base = polar(rb, a0 - t);
            let tangent = polar(1.0, a0 - t + PI / 2.0);
            let want = add(base, scale(tangent, rb * t));
            assert!(norm(sub(p, want)) < 1e-9, "t {t}: {p:?} vs {want:?}");
        }
    }

    #[test]
    fn the_flank_spline_stays_within_tolerance() {
        for (module, teeth) in [(0.5, 12), (2.0, 20), (5.0, 60)] {
            let g = GearSpec {
                module,
                teeth,
                ..spec()
            }
            .geometry()
            .unwrap();
            let poles = involute_spline(&g, g.base_radius.max(g.root_radius), g.tip_radius);
            for i in 0..=1000 {
                let p = fit::evaluate(&poles, i as f64 / 1000.0);
                let r = norm(p);
                let off = r * (p[1].atan2(p[0]) - g.half_angle(r)).abs();
                assert!(off <= FLANK_TOLERANCE, "m {module} z {teeth}: {off}");
            }
        }
    }

    fn outline_radii(outline: &Outline) -> (f64, f64) {
        let pts = outline.loops[0].polyline(32);
        let rs: Vec<f64> = pts.iter().map(|p| norm(*p)).collect();
        (
            rs.iter().copied().fold(f64::MAX, f64::min),
            rs.iter().copied().fold(0.0, f64::max),
        )
    }

    #[test]
    fn the_outline_spans_root_to_tip_with_one_tip_arc_a_tooth() {
        let spec = spec();
        let g = spec.geometry().unwrap();
        let outline = spec.outline().unwrap();
        let (lo, hi) = outline_radii(&outline);
        assert!((hi - g.tip_radius).abs() < 1e-6, "{hi}");
        assert!((lo - g.root_radius).abs() < 1e-6, "{lo}");
        let lp = &outline.loops[0];
        assert_eq!(lp.joints.len(), lp.edges.len());
        let tips = lp
            .edges
            .iter()
            .enumerate()
            .filter(|(i, e)| {
                matches!(e, Edge::Arc { center } if norm(*center) < 1e-12)
                    && (norm(lp.joints[*i]) - g.tip_radius).abs() < 1e-9
            })
            .count();
        assert_eq!(tips, 20);
    }

    #[test]
    fn the_area_is_about_the_pitch_disc() {
        // The teeth add above the pitch circle about what the gaps take
        // below it.
        let outline = spec().outline().unwrap();
        let r = 20.0;
        let area = outline.area();
        assert!((area / (PI * r * r) - 1.0).abs() < 0.03, "{area}");
    }

    #[test]
    fn the_root_fillet_touches_the_root_circle_and_the_flank() {
        let g = spec().geometry().unwrap();
        let rho = 0.38 * 2.0;
        let foot = root_foot(&g, rho);
        let f = foot.fillet.expect("a fillet fits");
        assert!((norm(f.center) - (g.root_radius + rho)).abs() < 1e-9);
        let on_flank = g.flank_point(foot.flank_radius);
        assert!((norm(sub(on_flank, f.center)) - rho).abs() < 1e-9);
    }

    #[test]
    fn pointed_teeth_are_refused() {
        let why = GearSpec {
            profile_shift: -0.2,
            backlash: 3.0,
            ..spec()
        }
        .outline()
        .unwrap_err();
        assert!(why.contains("point"), "{why}");
    }
}
