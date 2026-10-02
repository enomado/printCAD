//! The Surface workbench's features, as the document keeps them: what each
//! step is built from (sketches by id, edges and faces of the body as picks
//! in its own frame) and its settings.

use core_document::{DocumentResult, FeatureError, FeatureId, WorkbenchFeature, WorkbenchId};
use kernel_api::{Continuity, TopoName};
use serde::{Deserialize, Serialize};

/// The kind every surface feature carries.
pub const KIND: &str = "wb.surface";

/// A curve a step is built from: every chain of a sketch, or an edge of
/// the body.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CurveRef {
    Sketch(FeatureId),
    Edge(EdgePick),
}

/// An edge of the body, as a point beside it and the way it runs there, in
/// the body's own frame, with the names of the faces it runs between.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EdgePick {
    pub point: [f32; 3],
    pub direction: [f32; 3],
    #[serde(default)]
    pub faces: [TopoName; 2],
    /// Its length when picked, to name it by.
    #[serde(default)]
    pub length: f32,
}

/// A face of the body, as a point on it and its normal there, in the
/// body's own frame, with its name.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FacePick {
    pub point: [f32; 3],
    pub normal: [f32; 3],
    #[serde(default)]
    pub name: TopoName,
}

/// Which way an extrusion runs.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum Direction {
    /// Square to the first sketch's plane.
    #[default]
    SketchNormal,
    X,
    Y,
    Z,
    Custom([f32; 3]),
}

/// The axis a revolution turns about.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum Axis {
    /// The first sketch's vertical axis, through its origin.
    #[default]
    SketchVertical,
    /// The first sketch's horizontal axis, through its origin.
    SketchHorizontal,
    X,
    Y,
    Z,
    Custom {
        origin: [f32; 3],
        direction: [f32; 3],
    },
}

/// A plane a mirror reflects in or a trim cuts by, in the body's frame.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum PlaneRef {
    #[default]
    YZ,
    XZ,
    XY,
    Custom {
        origin: [f32; 3],
        normal: [f32; 3],
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SurfaceFeature {
    Extrude {
        curves: Vec<CurveRef>,
        #[serde(default)]
        direction: Direction,
        length: f32,
        #[serde(default)]
        symmetric: bool,
        #[serde(default)]
        reversed: bool,
    },
    Revolve {
        curves: Vec<CurveRef>,
        #[serde(default)]
        axis: Axis,
        angle_deg: f32,
    },
    PlanarFill {
        curves: Vec<CurveRef>,
    },
    Fill {
        boundary: Vec<CurveRef>,
        #[serde(default)]
        continuity: Continuity,
    },
    Ruled {
        first: Option<CurveRef>,
        second: Option<CurveRef>,
    },
    Loft {
        sections: Vec<CurveRef>,
        #[serde(default)]
        closed: bool,
    },
    Sweep {
        profile: Vec<CurveRef>,
        path: Vec<CurveRef>,
    },
    Offset {
        faces: Vec<FacePick>,
        distance: f32,
    },
    Extend {
        edges: Vec<EdgePick>,
        length: f32,
        #[serde(default)]
        continuity: Continuity,
    },
    Blend {
        first: Option<EdgePick>,
        second: Option<EdgePick>,
        #[serde(default)]
        continuity: Continuity,
    },
    Split {
        faces: Vec<FacePick>,
        curves: Vec<CurveRef>,
    },
    Sew,
    Thicken {
        thickness: f32,
        #[serde(default)]
        both_sides: bool,
    },
    Trim {
        #[serde(default)]
        plane: PlaneRef,
        #[serde(default)]
        offset: f32,
        /// Keep the side the plane's normal points away from.
        #[serde(default)]
        flip: bool,
    },
    Mirror {
        #[serde(default)]
        plane: PlaneRef,
        #[serde(default)]
        offset: f32,
    },
}

/// What each kind of step is, for the toolbar, the tree and the commands:
/// its tool id, its name, its icon and whether the kernel builds it yet.
pub struct Kind {
    pub tool: &'static str,
    pub label: &'static str,
    pub icon: &'static str,
    /// What the tool does once the kernel can: `None` when it builds now.
    pub waits: Option<&'static str>,
}

pub const KINDS: &[Kind] = &[
    Kind {
        tool: "surface.extrude",
        label: "Extruded surface",
        icon: "surface-extrude",
        waits: None,
    },
    Kind {
        tool: "surface.revolve",
        label: "Revolved surface",
        icon: "surface-revolve",
        waits: None,
    },
    Kind {
        tool: "surface.planar",
        label: "Planar surface",
        icon: "surface-planar",
        waits: None,
    },
    Kind {
        tool: "surface.fill",
        label: "Filling",
        icon: "surface-fill",
        waits: None,
    },
    Kind {
        tool: "surface.ruled",
        label: "Ruled surface",
        icon: "surface-ruled",
        waits: None,
    },
    Kind {
        tool: "surface.loft",
        label: "Lofted surface",
        icon: "surface-loft",
        waits: None,
    },
    Kind {
        tool: "surface.sweep",
        label: "Swept surface",
        icon: "surface-sweep",
        waits: None,
    },
    Kind {
        tool: "surface.offset",
        label: "Offset surface",
        icon: "surface-offset",
        waits: Some(
            "Copies faces at a distance along their normals, once the geometry kernel can offset free-form faces",
        ),
    },
    Kind {
        tool: "surface.extend",
        label: "Extend surface",
        icon: "surface-extend",
        waits: Some("Grows a face past a picked edge, once the geometry kernel can extend faces"),
    },
    Kind {
        tool: "surface.blend",
        label: "Blend surface",
        icon: "surface-blend",
        waits: Some(
            "Bridges two edges with a tangent or curvature-continuous surface, once the geometry kernel can blend",
        ),
    },
    Kind {
        tool: "surface.split",
        label: "Split surface",
        icon: "surface-split",
        waits: Some(
            "Cuts faces along curves projected onto them, once the geometry kernel can split a face",
        ),
    },
    Kind {
        tool: "surface.sew",
        label: "Sew",
        icon: "surface-sew",
        waits: None,
    },
    Kind {
        tool: "surface.thicken",
        label: "Thicken",
        icon: "surface-thicken",
        waits: Some(
            "Gives the body's sheets a thickness, making a solid, once the geometry kernel can thicken a sheet",
        ),
    },
    Kind {
        tool: "surface.trim",
        label: "Trim by plane",
        icon: "surface-trim",
        waits: Some("Cuts the body's sheets by a plane, once the geometry kernel can cut a sheet"),
    },
    Kind {
        tool: "surface.mirror",
        label: "Mirror",
        icon: "surface-mirror",
        waits: None,
    },
];

impl SurfaceFeature {
    /// The step a tool makes, with nothing picked yet.
    pub fn for_tool(tool: &str) -> Option<Self> {
        Some(match tool {
            "surface.extrude" => SurfaceFeature::Extrude {
                curves: Vec::new(),
                direction: Direction::SketchNormal,
                length: 10.0,
                symmetric: false,
                reversed: false,
            },
            "surface.revolve" => SurfaceFeature::Revolve {
                curves: Vec::new(),
                axis: Axis::SketchVertical,
                angle_deg: 360.0,
            },
            "surface.planar" => SurfaceFeature::PlanarFill { curves: Vec::new() },
            "surface.fill" => SurfaceFeature::Fill {
                boundary: Vec::new(),
                continuity: Continuity::G0,
            },
            "surface.ruled" => SurfaceFeature::Ruled {
                first: None,
                second: None,
            },
            "surface.loft" => SurfaceFeature::Loft {
                sections: Vec::new(),
                closed: false,
            },
            "surface.sweep" => SurfaceFeature::Sweep {
                profile: Vec::new(),
                path: Vec::new(),
            },
            "surface.offset" => SurfaceFeature::Offset {
                faces: Vec::new(),
                distance: 1.0,
            },
            "surface.extend" => SurfaceFeature::Extend {
                edges: Vec::new(),
                length: 5.0,
                continuity: Continuity::G1,
            },
            "surface.blend" => SurfaceFeature::Blend {
                first: None,
                second: None,
                continuity: Continuity::G1,
            },
            "surface.split" => SurfaceFeature::Split {
                faces: Vec::new(),
                curves: Vec::new(),
            },
            "surface.sew" => SurfaceFeature::Sew,
            "surface.thicken" => SurfaceFeature::Thicken {
                thickness: 2.0,
                both_sides: false,
            },
            "surface.trim" => SurfaceFeature::Trim {
                plane: PlaneRef::YZ,
                offset: 0.0,
                flip: false,
            },
            "surface.mirror" => SurfaceFeature::Mirror {
                plane: PlaneRef::YZ,
                offset: 0.0,
            },
            _ => return None,
        })
    }

    /// The tool that makes this kind of step.
    pub fn tool(&self) -> &'static str {
        match self {
            SurfaceFeature::Extrude { .. } => "surface.extrude",
            SurfaceFeature::Revolve { .. } => "surface.revolve",
            SurfaceFeature::PlanarFill { .. } => "surface.planar",
            SurfaceFeature::Fill { .. } => "surface.fill",
            SurfaceFeature::Ruled { .. } => "surface.ruled",
            SurfaceFeature::Loft { .. } => "surface.loft",
            SurfaceFeature::Sweep { .. } => "surface.sweep",
            SurfaceFeature::Offset { .. } => "surface.offset",
            SurfaceFeature::Extend { .. } => "surface.extend",
            SurfaceFeature::Blend { .. } => "surface.blend",
            SurfaceFeature::Split { .. } => "surface.split",
            SurfaceFeature::Sew => "surface.sew",
            SurfaceFeature::Thicken { .. } => "surface.thicken",
            SurfaceFeature::Trim { .. } => "surface.trim",
            SurfaceFeature::Mirror { .. } => "surface.mirror",
        }
    }

    pub fn kind(&self) -> &'static Kind {
        let tool = self.tool();
        KINDS
            .iter()
            .find(|k| k.tool == tool)
            .expect("every step has a kind")
    }

    /// Every curve the step reads, in order.
    pub fn curves(&self) -> Vec<CurveRef> {
        match self {
            SurfaceFeature::Extrude { curves, .. }
            | SurfaceFeature::Revolve { curves, .. }
            | SurfaceFeature::PlanarFill { curves }
            | SurfaceFeature::Split { curves, .. } => curves.clone(),
            SurfaceFeature::Fill { boundary, .. } => boundary.clone(),
            SurfaceFeature::Loft { sections, .. } => sections.clone(),
            SurfaceFeature::Ruled { first, second } => {
                first.iter().chain(second).copied().collect()
            }
            SurfaceFeature::Sweep { profile, path } => {
                profile.iter().chain(path).copied().collect()
            }
            _ => Vec::new(),
        }
    }

    /// The sketches the step reads.
    pub fn sketches(&self) -> Vec<FeatureId> {
        let mut out = Vec::new();
        for curve in self.curves() {
            if let CurveRef::Sketch(id) = curve
                && !out.contains(&id)
            {
                out.push(id);
            }
        }
        out
    }

    /// Whether the step is built from curves, so a tool makes it only with
    /// one picked.
    pub fn curves_needed(&self) -> bool {
        matches!(
            self,
            SurfaceFeature::Extrude { .. }
                | SurfaceFeature::Revolve { .. }
                | SurfaceFeature::PlanarFill { .. }
                | SurfaceFeature::Fill { .. }
                | SurfaceFeature::Ruled { .. }
                | SurfaceFeature::Loft { .. }
                | SurfaceFeature::Sweep { .. }
        )
    }

    /// Whether the step adds a sheet of its own, rather than working on
    /// what the body has.
    pub fn constructs(&self) -> bool {
        !matches!(
            self,
            SurfaceFeature::Sew
                | SurfaceFeature::Thicken { .. }
                | SurfaceFeature::Trim { .. }
                | SurfaceFeature::Mirror { .. }
                | SurfaceFeature::Split { .. }
                | SurfaceFeature::Extend { .. }
        )
    }
}

impl WorkbenchFeature for SurfaceFeature {
    fn workbench_id() -> WorkbenchId {
        WorkbenchId::from(KIND)
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    fn from_json(value: &serde_json::Value) -> DocumentResult<Self> {
        serde_json::from_value(value.clone()).map_err(|e| {
            core_document::DocumentError::Feature(FeatureError::Deserialization(e.to_string()))
        })
    }

    fn dependencies(&self) -> Vec<FeatureId> {
        self.sketches()
    }

    fn name(&self) -> &str {
        self.kind().label
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_makes_its_own_kind() {
        for kind in KINDS {
            let feature = SurfaceFeature::for_tool(kind.tool).expect(kind.tool);
            assert_eq!(feature.tool(), kind.tool);
            let back = SurfaceFeature::from_json(&feature.to_json()).unwrap();
            assert_eq!(back, feature);
        }
    }

    #[test]
    fn a_step_depends_on_the_sketches_it_reads_once_each() {
        let a = FeatureId::new();
        let b = FeatureId::new();
        let loft = SurfaceFeature::Loft {
            sections: vec![
                CurveRef::Sketch(a),
                CurveRef::Sketch(b),
                CurveRef::Sketch(a),
            ],
            closed: false,
        };
        assert_eq!(loft.dependencies(), vec![a, b]);
    }
}
