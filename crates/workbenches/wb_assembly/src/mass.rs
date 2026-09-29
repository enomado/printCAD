//! The assembly's mass and centre of mass: each visible solid body
//! measured by the kernel in its own frame, its centre placed where the
//! body sits, and each weighed at its material's density, or at one given
//! density where it has no material.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use core_document::{BodyId, BodyPlacement, Document};
use kernel_api::KernelQueries;

/// One body as measured.
#[derive(Debug, Clone, PartialEq)]
pub struct BodyMass {
    pub body: BodyId,
    pub volume_mm3: f64,
    /// World space.
    pub centre: [f64; 3],
    /// Its material's density, g/cm³, when it has one.
    pub density: Option<f64>,
}

impl BodyMass {
    /// Its mass in grams, at `density` g/cm³ when it has no material.
    pub fn mass_g(&self, density: f64) -> f64 {
        self.volume_mm3 * self.density.unwrap_or(density) / 1000.0
    }
}

/// What the measuring found.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MassReport {
    pub bodies: Vec<BodyMass>,
    /// Visible bodies with no solid, or none that encloses a volume.
    pub skipped: usize,
    pub stopped: bool,
}

impl MassReport {
    pub fn volume_mm3(&self) -> f64 {
        self.bodies.iter().map(|b| b.volume_mm3).sum()
    }

    /// The mass in grams, bodies without a material at `density` g/cm³.
    pub fn mass_g(&self, density: f64) -> f64 {
        self.bodies.iter().map(|b| b.mass_g(density)).sum()
    }

    /// The centre of mass, bodies without a material at `density`, world
    /// space.
    pub fn centre(&self, density: f64) -> Option<[f64; 3]> {
        let total = self.mass_g(density);
        (total > 0.0).then(|| {
            let mut c = [0.0; 3];
            for b in &self.bodies {
                for (k, v) in c.iter_mut().enumerate() {
                    *v += b.centre[k] * b.mass_g(density) / total;
                }
            }
            c
        })
    }
}

/// A body to measure: its snapshot, where it sits and its material's
/// density.
type Solid = (BodyId, Arc<Vec<u8>>, BodyPlacement, Option<f64>);

/// The bodies to measure, read from the document.
pub struct Weighing {
    solids: Vec<Solid>,
    skipped: usize,
}

/// The visible solid bodies, or only `among` when given.
pub fn plan(document: &Document, among: Option<&[BodyId]>) -> Weighing {
    let mut solids = Vec::new();
    let mut skipped = 0;
    for body in document.bodies() {
        if among.is_some_and(|only| !only.contains(&body.id))
            || !document.imported_body_effective_visible(body.id)
        {
            continue;
        }
        match document.imported_brep_blob_arc(body.id) {
            Some(blob) => solids.push((
                body.id,
                blob,
                document.body_placement(body.id),
                body.material.as_ref().map(|m| f64::from(m.density)),
            )),
            None => skipped += 1,
        }
    }
    Weighing { solids, skipped }
}

impl Weighing {
    pub fn len(&self) -> usize {
        self.solids.len()
    }

    /// Measure each body, counting them off in `done`, until `stop`.
    pub fn run(
        &self,
        kernel: &dyn KernelQueries,
        done: &AtomicUsize,
        stop: &AtomicBool,
    ) -> Result<MassReport, String> {
        let mut report = MassReport {
            skipped: self.skipped,
            ..MassReport::default()
        };
        for (body, blob, placement, density) in &self.solids {
            if stop.load(Ordering::Relaxed) {
                report.stopped = true;
                break;
            }
            let measured = kernel.measure(blob).map_err(|e| e.to_string())?;
            done.fetch_add(1, Ordering::Relaxed);
            let Some(volume) = measured.volume_mm3.filter(|v| *v > 0.0) else {
                report.skipped += 1;
                continue;
            };
            let c = measured.centre_mm.map(|v| v as f32);
            report.bodies.push(BodyMass {
                body: *body,
                volume_mm3: volume,
                centre: placement.point(c).map(f64::from),
                density: *density,
            });
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_centre_weighs_each_body_by_its_volume() {
        let body = |volume, x| BodyMass {
            body: BodyId::new(),
            volume_mm3: volume,
            centre: [x, 0.0, 0.0],
            density: None,
        };
        let report = MassReport {
            bodies: vec![body(1000.0, 0.0), body(3000.0, 10.0)],
            ..MassReport::default()
        };
        assert_eq!(report.centre(1.0), Some([7.5, 0.0, 0.0]));
        assert!((report.mass_g(1.25) - 5.0).abs() < 1e-9);
        assert_eq!(MassReport::default().centre(1.0), None);
    }
}
