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
    #[serde(
        default,
        deserialize_with = "kernel_api::naming::name_from_number_or_text"
    )]
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
    Sew {
        /// Edges this far apart (mm) are joined too; 0 joins only edges
        /// that meet.
        #[serde(default)]
        gap: f32,
    },
    /// A round of `radius` along picked edges where two faces of the
    /// body's sheets meet.
    Fillet {
        edges: Vec<EdgePick>,
        radius: f32,
    },
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
    /// What the step's command does, in a line.
    pub summary: &'static str,
    pub icon: &'static str,
    /// What the tool does once the kernel can: `None` when it builds now.
    pub waits: Option<&'static str>,
}

pub const KINDS: &[Kind] = &[
    Kind {
        tool: "surface.extrude",
        label: "Extruded surface",
        summary: "Extrude curves into a surface",
        icon: "surface-extrude",
        waits: None,
    },
    Kind {
        tool: "surface.revolve",
        label: "Revolved surface",
        summary: "Revolve curves about an axis into a surface",
        icon: "surface-revolve",
        waits: None,
    },
    Kind {
        tool: "surface.planar",
        label: "Planar surface",
        summary: "Fill closed flat loops with a planar surface",
        icon: "surface-planar",
        waits: None,
    },
    Kind {
        tool: "surface.fill",
        label: "Filling",
        summary: "Fill the hole curves close with a surface",
        icon: "surface-fill",
        waits: None,
    },
    Kind {
        tool: "surface.ruled",
        label: "Ruled surface",
        summary: "Span two curves with straight lines",
        icon: "surface-ruled",
        waits: None,
    },
    Kind {
        tool: "surface.loft",
        label: "Lofted surface",
        summary: "Loft a surface through sections in order",
        icon: "surface-loft",
        waits: None,
    },
    Kind {
        tool: "surface.sweep",
        label: "Swept surface",
        summary: "Sweep a profile along a path into a surface",
        icon: "surface-sweep",
        waits: None,
    },
    Kind {
        tool: "surface.offset",
        label: "Offset surface",
        summary: "Copy faces at a distance along their normals",
        icon: "surface-offset",
        waits: None,
    },
    Kind {
        tool: "surface.extend",
        label: "Extend surface",
        summary: "Extend faces past picked edges",
        icon: "surface-extend",
        waits: None,
    },
    Kind {
        tool: "surface.blend",
        label: "Blend surface",
        summary: "Bridge two edges with a surface",
        icon: "surface-blend",
        waits: None,
    },
    Kind {
        tool: "surface.split",
        label: "Split surface",
        summary: "Split faces along curves",
        icon: "surface-split",
        waits: None,
    },
    Kind {
        tool: "surface.sew",
        label: "Sew",
        summary: "Sew the body's surfaces together",
        icon: "surface-sew",
        waits: None,
    },
    Kind {
        tool: "surface.fillet",
        label: "Surface fillet",
        summary: "Round edges where two faces of a surface meet",
        icon: "surface-fillet",
        waits: None,
    },
    Kind {
        tool: "surface.thicken",
        label: "Thicken",
        summary: "Thicken the body's surfaces into solids",
        icon: "surface-thicken",
        waits: None,
    },
    Kind {
        tool: "surface.trim",
        label: "Trim by plane",
        summary: "Keep what of the body lies on one side of a plane",
        icon: "surface-trim",
        waits: None,
    },
    Kind {
        tool: "surface.mirror",
        label: "Mirror",
        summary: "Add the body's reflection in a plane",
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
            "surface.sew" => SurfaceFeature::Sew { gap: 0.0 },
            "surface.fillet" => SurfaceFeature::Fillet {
                edges: Vec::new(),
                radius: 2.0,
            },
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
            SurfaceFeature::Sew { .. } => "surface.sew",
            SurfaceFeature::Fillet { .. } => "surface.fillet",
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

    /// Take out the curves the step is built from, for new ones to go in.
    pub fn clear_curves(&mut self) {
        match self {
            Self::Extrude { curves, .. }
            | Self::Revolve { curves, .. }
            | Self::PlanarFill { curves }
            | Self::Split { curves, .. } => curves.clear(),
            Self::Fill { boundary, .. } => boundary.clear(),
            Self::Loft { sections, .. } => sections.clear(),
            Self::Ruled { first, second } => {
                *first = None;
                *second = None;
            }
            Self::Sweep { profile, path } => {
                profile.clear();
                path.clear();
            }
            Self::Offset { .. }
            | Self::Extend { .. }
            | Self::Blend { .. }
            | Self::Sew { .. }
            | Self::Fillet { .. }
            | Self::Thicken { .. }
            | Self::Trim { .. }
            | Self::Mirror { .. } => {}
        }
    }

    /// What the step still needs picked before it can build, in the words
    /// of the command field that gives it.
    pub fn missing(&self) -> Option<&'static str> {
        match self {
            Self::Extrude { curves, .. }
            | Self::Revolve { curves, .. }
            | Self::PlanarFill { curves } => curves
                .is_empty()
                .then_some("`sketches`: the curves to build from"),
            Self::Fill { boundary, .. } => boundary
                .is_empty()
                .then_some("`sketches` or `boundary`: the curves that close the hole"),
            Self::Ruled { first, second } => (first.is_none() || second.is_none())
                .then_some("`sketches`: two curves, the first and the second"),
            Self::Loft { sections, .. } => {
                (sections.len() < 2).then_some("`sketches`: two sections or more, in order")
            }
            Self::Sweep { profile, path } => (profile.is_empty() || path.is_empty())
                .then_some("`sketches`: the profile, then the path"),
            Self::Offset { faces, .. } => {
                faces.is_empty().then_some("`faces`: the faces to offset")
            }
            Self::Split { faces, curves } => (faces.is_empty() || curves.is_empty()).then_some(
                "`faces` and `sketches`: the faces to split and the curves to split them along",
            ),
            Self::Extend { edges, .. } => edges
                .is_empty()
                .then_some("`edges`: the edges to extend past"),
            Self::Fillet { edges, .. } => edges.is_empty().then_some("`edges`: the edges to round"),
            Self::Blend { first, second, .. } => (first.is_none() || second.is_none())
                .then_some("`first` and `second`: the two edges to bridge"),
            Self::Sew { .. } | Self::Thicken { .. } | Self::Trim { .. } | Self::Mirror { .. } => {
                None
            }
        }
    }

    /// Whether the step adds a sheet of its own, rather than working on
    /// what the body has.
    pub fn constructs(&self) -> bool {
        !matches!(
            self,
            SurfaceFeature::Sew { .. }
                | SurfaceFeature::Fillet { .. }
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
