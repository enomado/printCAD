//! Sketch feature implementation for the document feature tree.

use core_document::{DocumentResult, FeatureError, FeatureId, WorkbenchFeature, WorkbenchId};
use serde::{Deserialize, Serialize};

use crate::sketch::{Sketch, SketchPlane};

/// A sketch feature that can be stored in the document's feature tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SketchFeature {
    /// The sketch data.
    pub sketch: Sketch,
    /// The reference plane for the sketch.
    pub plane: SketchPlane,
    /// The datum the sketch was drawn on, which its plane follows: moved,
    /// turned or flipped, the datum takes the sketch with it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub support: Option<DatumSupport>,
    /// The face of the body's solid the sketch was placed on, which its
    /// plane follows: the face moved or turned by a change before it in
    /// the history, the sketch moves with it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face: Option<FaceSupport>,
    /// The numbers the sketch's curves are made from, for a generated
    /// sketch (a gear, a sprocket, a shaft); its curves follow them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generator: Option<crate::generator::Generator>,
    /// While the sketch is edited, everything on the viewer's side of its
    /// plane is cut away.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub section_view: bool,
}

/// A sketch's place on a datum: a datum plane, or one of a coordinate
/// system's three planes, pushed `offset` along its normal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatumSupport {
    pub datum: FeatureId,
    /// A coordinate system's plane: `XY`, `XZ` or `YZ`. A datum plane has
    /// only its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plane: Option<String>,
    /// Millimetres along the plane's normal.
    #[serde(default)]
    pub offset: f32,
}

/// A sketch's place on a face of its body's solid: the face as it was
/// picked (a point on it, its outward normal there and its name, in the
/// body's frame) and the plane the sketch had on it then.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FaceSupport {
    pub point: [f32; 3],
    pub normal: [f32; 3],
    #[serde(default)]
    pub name: kernel_api::TopoName,
    /// The sketch's plane when it was placed.
    pub placed: SketchPlane,
}

impl FaceSupport {
    /// The sketch placed with `placed` on `face`.
    pub fn on(face: &core_document::FaceRef, placed: SketchPlane) -> Self {
        Self {
            point: face.point,
            normal: face.normal,
            name: face.name,
            placed,
        }
    }

    /// What a build asks of the solid to find the face again.
    pub fn probe(&self) -> kernel_api::ShapeProbe {
        kernel_api::ShapeProbe::Face {
            point: self.point.map(f64::from),
            normal: self.normal.map(f64::from),
            name: self.name,
        }
    }

    /// The sketch's plane on the face as the solid now has it (`point` on
    /// it, `normal` out of it): turned as the face turned, and carried
    /// along the face's normal onto it, never sliding across it.
    pub fn plane_at(&self, point: [f64; 3], normal: [f64; 3]) -> SketchPlane {
        use glam::Vec3;
        let n0 = Vec3::from_array(self.normal).normalize_or_zero();
        let n1 =
            Vec3::new(normal[0] as f32, normal[1] as f32, normal[2] as f32).normalize_or_zero();
        let p0 = Vec3::from_array(self.point);
        let p1 = Vec3::new(point[0] as f32, point[1] as f32, point[2] as f32);
        let turn = glam::Quat::from_rotation_arc(n0, n1);
        let turned = |v: [f32; 3]| (turn * Vec3::from_array(v)).to_array();
        let origin = p0 + turn * (Vec3::from_array(self.placed.origin) - p0);
        let origin = origin + n1 * (p1 - origin).dot(n1);
        SketchPlane {
            origin: origin.to_array(),
            normal: turned(self.placed.normal),
            x_axis: turned(self.placed.x_axis),
            y_axis: turned(self.placed.y_axis),
        }
    }
}

impl DatumSupport {
    /// The plane this support puts a sketch on, from the datum's `data`;
    /// `None` when it is not a datum plane or coordinate system.
    pub fn plane_from(&self, data: &serde_json::Value) -> Option<SketchPlane> {
        use core_document::{DatumFeature, DatumShape};
        let datum = DatumFeature::from_json(data).ok()?;
        let frame = match datum.shape {
            DatumShape::Plane { .. } => datum.frame(),
            DatumShape::CoordinateSystem { .. } => {
                let which = self.plane.as_deref().unwrap_or("XY");
                datum
                    .frame()
                    .planes()
                    .into_iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case(which))?
                    .1
            }
            _ => return None,
        };
        let mut plane = SketchPlane {
            origin: frame.origin,
            normal: frame.normal,
            x_axis: frame.x_axis,
            y_axis: frame.y_axis(),
        };
        for (o, n) in plane.origin.iter_mut().zip(plane.normal) {
            *o += n * self.offset;
        }
        Some(plane)
    }
}

impl SketchFeature {
    pub fn new(sketch: Sketch, plane: SketchPlane) -> Self {
        Self {
            sketch,
            plane,
            support: None,
            face: None,
            generator: None,
            section_view: false,
        }
    }

    pub fn from_sketch(sketch: Sketch) -> Self {
        Self {
            sketch,
            plane: SketchPlane::default(),
            support: None,
            face: None,
            generator: None,
            section_view: false,
        }
    }
}

impl WorkbenchFeature for SketchFeature {
    fn workbench_id() -> WorkbenchId {
        WorkbenchId::from("wb.sketch")
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("SketchFeature should always serialize")
    }

    fn from_json(value: &serde_json::Value) -> DocumentResult<Self> {
        serde_json::from_value(value.clone()).map_err(|e| {
            core_document::DocumentError::Feature(FeatureError::Deserialization(e.to_string()))
        })
    }

    fn dependencies(&self) -> Vec<FeatureId> {
        self.support.iter().map(|s| s.datum).collect()
    }

    fn name(&self) -> &str {
        &self.sketch.name
    }
}

#[cfg(test)]
mod face_support {
    use super::*;

    #[test]
    fn a_sketch_on_a_face_moves_along_it_and_turns_with_it() {
        let top = core_document::FaceRef {
            point: [3.0, 4.0, 10.0],
            normal: [0.0, 0.0, 1.0],
            surface: None,
            name: 7,
        };
        let placed = SketchPlane::from_face(top.point, top.normal);
        let support = FaceSupport::on(&top, placed);
        // The face rises 5 mm, found at another spot of it: the sketch
        // rises with it and does not slide.
        let risen = support.plane_at([8.0, 1.0, 15.0], [0.0, 0.0, 1.0]);
        assert_eq!(risen.origin, [0.0, 0.0, 15.0]);
        assert_eq!(risen.x_axis, placed.x_axis);
        // The face turns to face +X through the same point: so does the
        // sketch, its normal with the face's.
        let turned = support.plane_at([3.0, 4.0, 10.0], [1.0, 0.0, 0.0]);
        assert!((glam::Vec3::from_array(turned.normal) - glam::Vec3::X).length() < 1e-6);
        let origin = glam::Vec3::from_array(turned.origin);
        assert!(
            (origin - glam::Vec3::new(3.0, 4.0, 10.0))
                .dot(glam::Vec3::X)
                .abs()
                < 1e-5
        );
    }
}
