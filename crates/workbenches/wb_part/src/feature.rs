//! Part Design feature payloads stored in the document feature tree.

use core_document::{
    BodyId, DocumentResult, FeatureError, FeatureId, WorkbenchFeature, WorkbenchId,
};
use serde::{Deserialize, Serialize};

use crate::hole_tables::{ThreadSize, ThreadStandard};

/// What a revolution or helix spins about. Every axis ends up in the
/// sketch plane: the sketch's own axes and lines lie there already, and a
/// picked edge or a datum line is taken as its shadow on the plane.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum RevolveAxis {
    /// The sketch's vertical (y) axis through the origin (the default).
    #[default]
    SketchY,
    /// The sketch's horizontal (x) axis through the origin.
    SketchX,
    /// An arbitrary in-plane axis (point + direction in sketch coordinates).
    Custom { origin: [f32; 2], dir: [f32; 2] },
    /// A straight edge of the solid, picked in the viewport, in the body's
    /// own frame.
    Edge(EdgePick),
    /// A datum line of the body.
    Datum(FeatureId),
    /// A line of the sketch itself, by its element id: a construction line
    /// drawn for the purpose, or an edge of the profile.
    SketchLine(uuid::Uuid),
    /// A straight edge another body lends this one.
    Borrowed(BorrowedRef),
}

impl RevolveAxis {
    pub fn label(&self) -> &'static str {
        match self {
            RevolveAxis::SketchY => "Sketch Y axis",
            RevolveAxis::SketchX => "Sketch X axis",
            RevolveAxis::Custom { .. } => "Custom axis",
            RevolveAxis::Edge(_) => "Picked edge",
            RevolveAxis::Datum(_) => "Datum line",
            RevolveAxis::SketchLine(_) => "Sketch line",
            RevolveAxis::Borrowed(_) => "Borrowed edge",
        }
    }
}

/// Where a revolution or groove stops turning.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum RevolveMode {
    /// Through its angle (or two, or centred on the sketch).
    #[default]
    Angle,
    /// On the first face of the existing material it meets.
    ToFirst,
    /// On the last face of the existing material it meets.
    ToLast,
    /// On a picked flat face whose plane holds the axis.
    UpToFace,
}

impl RevolveMode {
    pub const ALL: [RevolveMode; 4] = [
        RevolveMode::Angle,
        RevolveMode::ToFirst,
        RevolveMode::ToLast,
        RevolveMode::UpToFace,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            RevolveMode::Angle => "Angle",
            RevolveMode::ToFirst => "To first",
            RevolveMode::ToLast => "To last",
            RevolveMode::UpToFace => "Up to face",
        }
    }
}

/// Which way a pad or pocket runs.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum ExtrudeDirection {
    /// Along the sketch's normal (or the profile face's).
    #[default]
    Normal,
    /// Along a vector, in the body's own frame.
    Custom([f32; 3]),
    /// Along a straight edge of the solid, picked in the viewport.
    Edge(EdgePick),
    /// Along a straight edge another body lends this one.
    Borrowed(BorrowedRef),
}

impl ExtrudeDirection {
    pub fn label(&self) -> &'static str {
        match self {
            ExtrudeDirection::Normal => "Sketch normal",
            ExtrudeDirection::Custom(_) => "Custom vector",
            ExtrudeDirection::Edge(_) => "Picked edge",
            ExtrudeDirection::Borrowed(_) => "Borrowed edge",
        }
    }

    /// The direction set, in the body's own frame; `None` along the normal.
    /// A borrowed edge's is where its body has it, which the build works
    /// out.
    pub fn vector(&self) -> Option<[f64; 3]> {
        match self {
            ExtrudeDirection::Normal | ExtrudeDirection::Borrowed(_) => None,
            ExtrudeDirection::Custom(v) => Some(v.map(f64::from)),
            ExtrudeDirection::Edge(edge) => Some(edge.direction.map(f64::from)),
        }
    }
}

/// A planar face picked in the viewport, identified geometrically.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FacePick {
    pub point: [f32; 3],
    pub normal: [f32; 3],
}

/// Where a pad/pocket stops along the sweep direction.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum ExtrudeMode {
    /// Fixed length/depth.
    #[default]
    Dimension,
    /// Independent lengths to each side of the sketch plane.
    TwoLengths,
    /// Through every face of the existing material.
    ThroughAll,
    /// Stop at the first face hit along the direction.
    ToFirst,
    /// Stop at the last face hit along the direction.
    ToLast,
    /// Stop on a picked face (plus offset).
    UpToFace,
    /// Stop on a set of picked faces, each line of the sweep at the first
    /// of them it meets (plus offset along the sweep).
    UpToShape,
    /// Stop on a face another body lends (plus offset).
    UpToBorrowed(BorrowedRef),
}

impl ExtrudeMode {
    pub const ALL: [ExtrudeMode; 7] = [
        ExtrudeMode::Dimension,
        ExtrudeMode::TwoLengths,
        ExtrudeMode::ThroughAll,
        ExtrudeMode::ToFirst,
        ExtrudeMode::ToLast,
        ExtrudeMode::UpToFace,
        ExtrudeMode::UpToShape,
    ];

    /// The ways the second side of a two-sided extrusion may end.
    pub const SECOND_SIDE: [ExtrudeMode; 6] = [
        ExtrudeMode::Dimension,
        ExtrudeMode::ThroughAll,
        ExtrudeMode::ToFirst,
        ExtrudeMode::ToLast,
        ExtrudeMode::UpToFace,
        ExtrudeMode::UpToShape,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            ExtrudeMode::Dimension => "Dimension",
            ExtrudeMode::TwoLengths => "Two lengths",
            ExtrudeMode::ThroughAll => "Through all",
            ExtrudeMode::ToFirst => "To first",
            ExtrudeMode::ToLast => "To last",
            ExtrudeMode::UpToFace => "Up to face",
            ExtrudeMode::UpToShape => "Up to shape",
            ExtrudeMode::UpToBorrowed(_) => "Up to borrowed face",
        }
    }

    /// Whether the mode ends on existing material, so needs some.
    pub fn needs_material(&self) -> bool {
        matches!(
            self,
            ExtrudeMode::ThroughAll
                | ExtrudeMode::ToFirst
                | ExtrudeMode::ToLast
                | ExtrudeMode::UpToShape
        )
    }

    /// The end condition of each side: the first, and the second when the
    /// extrusion runs both ways. Two lengths is a dimension each way, the
    /// second side ending as `mode2` says when it is set.
    pub fn sides(self, mode2: Option<ExtrudeMode>) -> (ExtrudeMode, Option<ExtrudeMode>) {
        match self {
            ExtrudeMode::TwoLengths => (
                ExtrudeMode::Dimension,
                Some(mode2.unwrap_or(ExtrudeMode::Dimension)),
            ),
            mode => (mode, mode2),
        }
    }
}

/// How a helix's extent is specified; the missing quantity is derived.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum HelixMode {
    #[default]
    PitchHeight,
    PitchTurns,
    HeightTurns,
}

impl HelixMode {
    pub const ALL: [HelixMode; 3] = [
        HelixMode::PitchHeight,
        HelixMode::PitchTurns,
        HelixMode::HeightTurns,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            HelixMode::PitchHeight => "Pitch + height",
            HelixMode::PitchTurns => "Pitch + turns",
            HelixMode::HeightTurns => "Height + turns",
        }
    }
}

/// Chamfer sizing style.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum ChamferMode {
    #[default]
    EqualDistance,
    TwoDistances,
    DistanceAngle,
}

impl ChamferMode {
    pub const ALL: [ChamferMode; 3] = [
        ChamferMode::EqualDistance,
        ChamferMode::TwoDistances,
        ChamferMode::DistanceAngle,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            ChamferMode::EqualDistance => "Equal distance",
            ChamferMode::TwoDistances => "Two distances",
            ChamferMode::DistanceAngle => "Distance + angle",
        }
    }
}

/// Which edges a dress-up applies to.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub enum EdgeSel {
    /// Every edge of the solid (immune to topology churn).
    #[default]
    All,
    /// The edges bordering the picked faces.
    Faces(Vec<FacePick>),
    /// Edges picked one by one in the viewport.
    Edges(Vec<EdgePick>),
}

/// A picked edge, by a point on it and its direction there, re-resolved
/// against the current solid each rebuild: a rebuilt solid numbers its
/// edges afresh, but the edge nearest the point is the one picked.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EdgePick {
    pub point: [f32; 3],
    pub direction: [f32; 3],
}

/// A mirror/pattern reference plane.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum MirrorPlane {
    #[default]
    XY,
    XZ,
    YZ,
    Face(FacePick),
}

impl MirrorPlane {
    pub const BASE: [MirrorPlane; 3] = [MirrorPlane::XY, MirrorPlane::XZ, MirrorPlane::YZ];

    pub fn label(&self) -> &'static str {
        match self {
            MirrorPlane::XY => "XY plane",
            MirrorPlane::XZ => "XZ plane",
            MirrorPlane::YZ => "YZ plane",
            MirrorPlane::Face(_) => "Picked face",
        }
    }

    /// The plane as (point, normal), in the body's own frame.
    pub fn plane(&self) -> ([f64; 3], [f64; 3]) {
        match self {
            MirrorPlane::XY => ([0.0; 3], [0.0, 0.0, 1.0]),
            MirrorPlane::XZ => ([0.0; 3], [0.0, 1.0, 0.0]),
            MirrorPlane::YZ => ([0.0; 3], [1.0, 0.0, 0.0]),
            MirrorPlane::Face(pick) => (
                [
                    pick.point[0] as f64,
                    pick.point[1] as f64,
                    pick.point[2] as f64,
                ],
                [
                    pick.normal[0] as f64,
                    pick.normal[1] as f64,
                    pick.normal[2] as f64,
                ],
            ),
        }
    }
}

/// A pattern direction/axis in world space.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum PatternAxis {
    X,
    #[default]
    Y,
    Z,
    Custom {
        origin: [f32; 3],
        dir: [f32; 3],
    },
}

impl PatternAxis {
    pub const BASE: [PatternAxis; 3] = [PatternAxis::X, PatternAxis::Y, PatternAxis::Z];

    pub fn label(&self) -> &'static str {
        match self {
            PatternAxis::X => "X axis",
            PatternAxis::Y => "Y axis",
            PatternAxis::Z => "Z axis",
            PatternAxis::Custom { .. } => "Custom axis",
        }
    }

    pub fn origin(&self) -> [f64; 3] {
        match self {
            PatternAxis::Custom { origin, .. } => {
                [origin[0] as f64, origin[1] as f64, origin[2] as f64]
            }
            _ => [0.0; 3],
        }
    }

    pub fn dir(&self) -> [f64; 3] {
        match self {
            PatternAxis::X => [1.0, 0.0, 0.0],
            PatternAxis::Y => [0.0, 1.0, 0.0],
            PatternAxis::Z => [0.0, 0.0, 1.0],
            PatternAxis::Custom { dir, .. } => [dir[0] as f64, dir[1] as f64, dir[2] as f64],
        }
    }
}

/// One step of a multi-transform (each applies to every result of the
/// previous step).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TransformStep {
    Linear {
        axis: PatternAxis,
        length: f32,
        occurrences: u32,
    },
    Polar {
        axis: PatternAxis,
        angle_deg: f32,
        occurrences: u32,
    },
    Mirror {
        plane: MirrorPlane,
    },
    Scale {
        factor: f32,
        center: [f32; 3],
        occurrences: u32,
    },
}

/// What a hole cuts around its mouth, beyond the drill.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum HoleCut {
    #[default]
    None,
    Counterbore {
        diameter: f32,
        depth: f32,
    },
    Countersink {
        diameter: f32,
        angle_deg: f32,
    },
    /// A shallow counterbore: a flat seat faced into a rough or curved
    /// surface.
    Spotface {
        diameter: f32,
        depth: f32,
    },
    /// A wider bore ending in a cone that narrows to the hole, as a
    /// stepped drill leaves it: `angle_deg` is the cone's included angle.
    Counterdrill {
        diameter: f32,
        depth: f32,
        angle_deg: f32,
    },
    /// The seat of a standard screw's head, sized from the hole's metric
    /// thread size.
    Seat {
        seat: crate::hole_tables::ScrewSeat,
    },
}

impl HoleCut {
    pub fn label(&self) -> &'static str {
        match self {
            HoleCut::None => "None",
            HoleCut::Counterbore { .. } => "Counterbore",
            HoleCut::Countersink { .. } => "Countersink",
            HoleCut::Spotface { .. } => "Spotface",
            HoleCut::Counterdrill { .. } => "Counterdrill",
            HoleCut::Seat { seat } => seat.label(),
        }
    }
}

/// ISO 273 metric clearance style for a threaded hole size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HoleFit {
    Close,
    #[default]
    Normal,
    Loose,
}

impl HoleFit {
    pub const ALL: [HoleFit; 3] = [HoleFit::Close, HoleFit::Normal, HoleFit::Loose];

    pub fn label(&self) -> &'static str {
        match self {
            HoleFit::Close => "Close",
            HoleFit::Normal => "Normal",
            HoleFit::Loose => "Loose",
        }
    }
}

/// The thread a standards-driven hole is sized from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "ThreadSpecRepr")]
pub struct ThreadSpec {
    pub standard: ThreadStandard,
    /// The size's designation in its standard: "M6", "1/4-20", "1/2".
    pub size: String,
    /// The class of fit ("6H", "2B"); empty for the standard's default.
    #[serde(default)]
    pub class: String,
    /// A left-hand thread: a modeled one turns the other way.
    #[serde(default)]
    pub left_handed: bool,
}

/// The ISO metric coarse sizes in the order a hole's `metric_index`
/// numbers them.
const INDEXED_METRIC_SIZES: [&str; 10] = [
    "M2", "M2.5", "M3", "M4", "M5", "M6", "M8", "M10", "M12", "M16",
];

/// A thread as a document holds it: whole, or as a `metric_index`, the
/// place of an ISO metric coarse size in [`INDEXED_METRIC_SIZES`].
#[derive(Deserialize)]
#[serde(untagged)]
enum ThreadSpecRepr {
    Index(usize),
    Spec {
        standard: ThreadStandard,
        size: String,
        #[serde(default)]
        class: String,
        #[serde(default)]
        left_handed: bool,
    },
}

impl From<ThreadSpecRepr> for ThreadSpec {
    fn from(repr: ThreadSpecRepr) -> Self {
        match repr {
            ThreadSpecRepr::Index(index) => ThreadSpec::new(
                ThreadStandard::IsoMetricCoarse,
                &INDEXED_METRIC_SIZES
                    .get(index)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("#{index}")),
            ),
            ThreadSpecRepr::Spec {
                standard,
                size,
                class,
                left_handed,
            } => ThreadSpec {
                standard,
                size,
                class,
                left_handed,
            },
        }
    }
}

impl ThreadSpec {
    pub fn new(standard: ThreadStandard, size: &str) -> Self {
        Self {
            standard,
            size: size.to_string(),
            class: String::new(),
            left_handed: false,
        }
    }

    /// The class the thread is cut to: its own, or its standard's default.
    pub fn class(&self) -> &str {
        if self.class.is_empty() {
            self.standard.default_class()
        } else {
            &self.class
        }
    }

    /// The thread's size, or why the standard has none by its name or
    /// class.
    pub fn resolve(&self) -> Result<ThreadSize, String> {
        let size = self.standard.size(&self.size).ok_or_else(|| {
            format!(
                "there is no {} size \"{}\"",
                self.standard.label(),
                self.size
            )
        })?;
        let classes = self.standard.classes();
        if !self.class.is_empty() && !classes.contains(&self.class.as_str()) {
            return Err(if classes.is_empty() {
                format!("{} threads have no class to choose", self.standard.label())
            } else {
                format!(
                    "{} has no class {}; it has {}",
                    self.standard.label(),
                    self.class,
                    classes.join(", ")
                )
            });
        }
        Ok(size)
    }

    /// How far the class moves the thread's diameters out, mm.
    pub fn allowance(&self, size: &ThreadSize) -> f64 {
        self.standard.class_allowance(self.class(), size.pitch)
    }

    /// The designation as a drawing gives it: "M6-6H", "1/4-20 UNC-2B",
    /// "G 1/2", "M8x1-6G LH".
    pub fn designation(&self) -> String {
        use ThreadStandard as S;
        let mut text = match self.standard {
            S::IsoMetricCoarse | S::IsoMetricFine => self.size.clone(),
            S::Unc | S::Unf | S::Unef | S::Bsw | S::Bsf => {
                format!("{} {}", self.size, self.standard.label())
            }
            S::BspParallel => format!("G {}", self.size),
            S::BspTaper => format!("Rc {}", self.size),
            S::Npt => format!("{} NPT", self.size),
        };
        let class = self.class();
        if !class.is_empty() {
            text.push('-');
            text.push_str(class);
        }
        if self.left_handed {
            text.push_str(" LH");
        }
        text
    }
}

/// The bottom of a blind hole.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum DrillPoint {
    /// Square to the axis, as an end mill leaves it.
    #[default]
    Flat,
    /// A cone of the drill's included point angle (118° and 135° are the
    /// usual ones).
    Angled { angle_deg: f32 },
}

/// What a body borrows from another: one of its sketches, or faces and
/// edges of its solid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BorrowSource {
    /// A sketch of another body: its profile, where that body sits.
    Sketch(FeatureId),
    /// Faces and edges of another body's solid, each where it was picked,
    /// in that body's own frame, and re-found on the solid as it is built.
    Solid {
        body: BodyId,
        #[serde(default)]
        faces: Vec<FacePick>,
        #[serde(default)]
        edges: Vec<EdgePick>,
    },
}

/// Borrowed geometry as it was when it was frozen, in the borrowing body's
/// frame: what the body keeps once it stops following the source.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FrozenBorrow {
    /// The sketch, with its plane in the borrowing body's frame, as a
    /// sketch feature's data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sketch: Option<serde_json::Value>,
    #[serde(default)]
    pub faces: Vec<FrozenFace>,
    #[serde(default)]
    pub edges: Vec<FrozenEdge>,
}

/// A borrowed face as it was frozen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrozenFace {
    /// Where it was picked, in the borrowing body's frame.
    pub pick: FacePick,
    /// The face alone, as a native-format snapshot in its own body's frame.
    pub shape: String,
    /// What moves `shape` into the borrowing body's frame, a rigid
    /// row-major 4×4 matrix.
    pub transform: [[f64; 4]; 4],
    /// Its surface, in the borrowing body's frame, when the mesh knew it.
    #[serde(default)]
    pub surface: Option<kernel_api::FaceSurface>,
    /// Its outline, pairs of points in the borrowing body's frame.
    #[serde(default)]
    pub outline: Vec<[f32; 3]>,
}

/// A borrowed edge as it was frozen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrozenEdge {
    /// A point of it and its direction there, in the borrowing body's
    /// frame.
    pub pick: EdgePick,
    /// Its outline, pairs of points in the borrowing body's frame.
    #[serde(default)]
    pub outline: Vec<[f32; 3]>,
}

/// One face or edge of a borrow: the borrow feature and its place in the
/// borrow's list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BorrowedRef {
    pub borrow: FeatureId,
    pub index: usize,
}

/// A solid-modeling feature in a body's linear history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PartFeature {
    /// Start the body from a copy of another body's solid, as that body
    /// is built now.
    Clone { source: BodyId },
    /// Extrude the sketch profile (or a flat face of the solid), adding
    /// material.
    Pad {
        /// The sketch whose profile it extrudes; `None` when the profile is
        /// `profile_face`.
        #[serde(default)]
        sketch: Option<FeatureId>,
        length: f32,
        /// Extrude along -normal instead of +normal.
        reversed: bool,
        /// Extrude half the length to each side of the sketch plane.
        #[serde(default)]
        symmetric: bool,
        #[serde(default)]
        mode: ExtrudeMode,
        #[serde(default)]
        length2: f32,
        #[serde(default)]
        taper_deg: f32,
        #[serde(default)]
        up_to_face: Option<FacePick>,
        #[serde(default)]
        up_to_offset: f32,
        /// Merge the coplanar faces the fuse or cut leaves behind.
        #[serde(default)]
        refine: bool,
        /// A flat face of the solid extruded in place of a sketch: its
        /// boundaries are the profile and its outward normal the sketch's.
        #[serde(default)]
        profile_face: Option<FacePick>,
        #[serde(default)]
        direction: ExtrudeDirection,
        /// The faces an up-to-shape extrusion stops on.
        #[serde(default)]
        up_to_shape: Vec<FacePick>,
        /// How the second side ends, when the pad runs both ways.
        #[serde(default)]
        mode2: Option<ExtrudeMode>,
        #[serde(default)]
        up_to_face2: Option<FacePick>,
        #[serde(default)]
        up_to_offset2: f32,
        #[serde(default)]
        up_to_shape2: Vec<FacePick>,
    },
    /// Extrude the sketch profile (or a flat face of the solid) and
    /// subtract it (cuts against the sketch normal by default: a face
    /// sketch's normal points out of the material).
    Pocket {
        /// The sketch whose profile it cuts; `None` when the profile is
        /// `profile_face`.
        #[serde(default)]
        sketch: Option<FeatureId>,
        depth: f32,
        reversed: bool,
        /// Cut half the depth to each side of the sketch plane.
        #[serde(default)]
        symmetric: bool,
        /// The same setting as `mode` ThroughAll, kept in step with it by
        /// commands and the panel; a file with only this set cuts through
        /// all.
        #[serde(default)]
        through_all: bool,
        #[serde(default)]
        mode: ExtrudeMode,
        #[serde(default)]
        depth2: f32,
        #[serde(default)]
        taper_deg: f32,
        #[serde(default)]
        up_to_face: Option<FacePick>,
        #[serde(default)]
        up_to_offset: f32,
        /// Merge the coplanar faces the fuse or cut leaves behind.
        #[serde(default)]
        refine: bool,
        /// A flat face of the solid cut in place of a sketch's profile.
        #[serde(default)]
        profile_face: Option<FacePick>,
        /// A direction set here is the way the cut runs; along the normal
        /// it runs against it.
        #[serde(default)]
        direction: ExtrudeDirection,
        #[serde(default)]
        up_to_shape: Vec<FacePick>,
        #[serde(default)]
        mode2: Option<ExtrudeMode>,
        #[serde(default)]
        up_to_face2: Option<FacePick>,
        #[serde(default)]
        up_to_offset2: f32,
        #[serde(default)]
        up_to_shape2: Vec<FacePick>,
    },
    /// Revolve the sketch profile about an in-plane axis, adding material.
    Revolution {
        sketch: FeatureId,
        angle_deg: f32,
        #[serde(default)]
        axis: RevolveAxis,
        #[serde(default)]
        reversed: bool,
        #[serde(default)]
        midplane: bool,
        #[serde(default)]
        second_angle_deg: Option<f32>,
        /// Merge the coplanar faces the fuse or cut leaves behind.
        #[serde(default)]
        refine: bool,
        #[serde(default)]
        mode: RevolveMode,
        #[serde(default)]
        up_to_face: Option<FacePick>,
    },
    /// Revolve the sketch profile and subtract it.
    Groove {
        sketch: FeatureId,
        angle_deg: f32,
        #[serde(default)]
        axis: RevolveAxis,
        #[serde(default)]
        reversed: bool,
        #[serde(default)]
        midplane: bool,
        #[serde(default)]
        second_angle_deg: Option<f32>,
        /// Merge the coplanar faces the fuse or cut leaves behind.
        #[serde(default)]
        refine: bool,
        #[serde(default)]
        mode: RevolveMode,
        #[serde(default)]
        up_to_face: Option<FacePick>,
    },
    /// Skin through two or more section sketches.
    Loft {
        sections: Vec<FeatureId>,
        ruled: bool,
        closed: bool,
        subtractive: bool,
        /// Merge the coplanar faces the fuse or cut leaves behind.
        #[serde(default)]
        refine: bool,
    },
    /// Sweep a profile sketch along a spine sketch's path.
    Pipe {
        profile: FeatureId,
        spine: FeatureId,
        frenet: bool,
        subtractive: bool,
        /// Merge the coplanar faces the fuse or cut leaves behind.
        #[serde(default)]
        refine: bool,
    },
    /// Sweep the sketch profile along a helix about an in-plane axis.
    Helix {
        sketch: FeatureId,
        axis: RevolveAxis,
        mode: HelixMode,
        pitch: f32,
        height: f32,
        turns: f32,
        left_handed: bool,
        cone_angle_deg: f32,
        reversed: bool,
        subtractive: bool,
        /// Merge the coplanar faces the fuse or cut leaves behind.
        #[serde(default)]
        refine: bool,
    },
    /// Parametric primitive fused into (or cut from) the body.
    Primitive {
        kind: kernel_api::PrimitiveKind,
        placement: kernel_api::Placement,
        subtractive: bool,
        /// Merge the coplanar faces the fuse or cut leaves behind.
        #[serde(default)]
        refine: bool,
    },
    /// Standards-aware drilled cuts at every circle center of a sketch.
    Hole {
        sketch: FeatureId,
        diameter: f32,
        depth: f32,
        through_all: bool,
        #[serde(default)]
        cut: HoleCut,
        /// The thread the hole is sized from when it is standards-driven;
        /// the diameter then derives from the thread and the fit.
        #[serde(default, alias = "metric_index")]
        thread: Option<ThreadSpec>,
        #[serde(default)]
        threaded: bool,
        /// A threaded hole's thread cut into its wall, not only its tap
        /// drill: for printing threads rather than tapping them.
        #[serde(default)]
        modeled_thread: bool,
        /// How far down a modeled thread runs, mm.
        #[serde(default)]
        thread_depth: f32,
        #[serde(default)]
        fit: HoleFit,
        /// The bottom of a blind hole.
        #[serde(default)]
        drill_point: DrillPoint,
        /// The drill point lies within the depth rather than below it.
        #[serde(default)]
        point_in_depth: bool,
        /// The wall leans in toward the bottom by this angle, degrees;
        /// a tapered thread standard's taper stands in for it.
        #[serde(default)]
        taper_deg: f32,
        #[serde(default)]
        reversed: bool,
        /// Merge the coplanar faces the fuse or cut leaves behind.
        #[serde(default)]
        refine: bool,
    },
    Fillet {
        radius: f32,
        #[serde(default)]
        edges: EdgeSel,
    },
    Chamfer {
        size: f32,
        #[serde(default)]
        mode: ChamferMode,
        #[serde(default)]
        size2: f32,
        #[serde(default)]
        angle_deg: f32,
        #[serde(default)]
        flip: bool,
        #[serde(default)]
        edges: EdgeSel,
    },
    Draft {
        angle_deg: f32,
        neutral: FacePick,
        faces: Vec<FacePick>,
        #[serde(default)]
        reversed: bool,
    },
    Thickness {
        value: f32,
        faces: Vec<FacePick>,
        #[serde(default = "default_true")]
        inward: bool,
    },
    Mirrored {
        originals: Vec<FeatureId>,
        plane: MirrorPlane,
        /// Merge the coplanar faces the fuse or cut leaves behind.
        #[serde(default)]
        refine: bool,
    },
    LinearPattern {
        originals: Vec<FeatureId>,
        axis: PatternAxis,
        length: f32,
        occurrences: u32,
        /// `length` is the spacing between occurrences instead of the total.
        #[serde(default)]
        spacing_mode: bool,
        #[serde(default)]
        reversed: bool,
        /// Merge the coplanar faces the fuse or cut leaves behind.
        #[serde(default)]
        refine: bool,
    },
    PolarPattern {
        originals: Vec<FeatureId>,
        axis: PatternAxis,
        angle_deg: f32,
        occurrences: u32,
        #[serde(default)]
        reversed: bool,
        /// Merge the coplanar faces the fuse or cut leaves behind.
        #[serde(default)]
        refine: bool,
    },
    MultiTransform {
        originals: Vec<FeatureId>,
        steps: Vec<TransformStep>,
        /// Merge the coplanar faces the fuse or cut leaves behind.
        #[serde(default)]
        refine: bool,
    },
    /// Boolean against another body's built solid.
    BodyBoolean {
        tool_body: BodyId,
        kind: kernel_api::BoolKind,
        /// Merge the coplanar faces the fuse or cut leaves behind.
        #[serde(default)]
        refine: bool,
    },
    /// Geometry of another body lent to this one, where the two bodies
    /// sit: a sketch whose profile features here take, faces they stop on
    /// or sketch on, edges they turn about or run along. It builds nothing
    /// itself.
    Borrow {
        source: BorrowSource,
        /// The geometry as it was frozen; `None` follows the source.
        #[serde(default)]
        frozen: Option<FrozenBorrow>,
    },
}

fn default_true() -> bool {
    true
}

impl PartFeature {
    /// The sketch this feature consumes, when it is sketch-based.
    pub fn sketch(&self) -> Option<FeatureId> {
        match self {
            PartFeature::Pad { sketch, .. } | PartFeature::Pocket { sketch, .. } => *sketch,
            PartFeature::Revolution { sketch, .. }
            | PartFeature::Groove { sketch, .. }
            | PartFeature::Helix { sketch, .. }
            | PartFeature::Hole { sketch, .. } => Some(*sketch),
            PartFeature::Pipe { profile, .. } => Some(*profile),
            _ => None,
        }
    }

    /// Every sketch referenced by this feature.
    pub fn sketches(&self) -> Vec<FeatureId> {
        match self {
            PartFeature::Loft { sections, .. } => sections.clone(),
            PartFeature::Pipe { profile, spine, .. } => vec![*profile, *spine],
            _ => self.sketch().into_iter().collect(),
        }
    }

    /// The borrows whose faces or edges this feature takes: a face it
    /// stops on, an edge it turns about or runs along.
    pub fn borrows(&self) -> Vec<FeatureId> {
        let mut borrows = Vec::new();
        match self {
            PartFeature::Pad {
                mode,
                mode2,
                direction,
                ..
            }
            | PartFeature::Pocket {
                mode,
                mode2,
                direction,
                ..
            } => {
                for mode in std::iter::once(mode).chain(mode2) {
                    if let ExtrudeMode::UpToBorrowed(r) = mode {
                        borrows.push(r.borrow);
                    }
                }
                if let ExtrudeDirection::Borrowed(r) = direction {
                    borrows.push(r.borrow);
                }
            }
            PartFeature::Revolution { axis, .. }
            | PartFeature::Groove { axis, .. }
            | PartFeature::Helix { axis, .. } => {
                if let RevolveAxis::Borrowed(r) = axis {
                    borrows.push(r.borrow);
                }
            }
            _ => {}
        }
        borrows.dedup();
        borrows
    }

    /// Earlier part features this feature re-applies (patterns/mirror).
    pub fn originals(&self) -> Option<&[FeatureId]> {
        match self {
            PartFeature::Mirrored { originals, .. }
            | PartFeature::LinearPattern { originals, .. }
            | PartFeature::PolarPattern { originals, .. }
            | PartFeature::MultiTransform { originals, .. } => Some(originals),
            _ => None,
        }
    }

    pub fn kind_label(&self) -> &'static str {
        match self {
            PartFeature::Pad { .. } => "Pad",
            PartFeature::Pocket { .. } => "Pocket",
            PartFeature::Revolution { .. } => "Revolution",
            PartFeature::Groove { .. } => "Groove",
            PartFeature::Loft { subtractive, .. } => {
                if *subtractive {
                    "Subtractive Loft"
                } else {
                    "Additive Loft"
                }
            }
            PartFeature::Pipe { subtractive, .. } => {
                if *subtractive {
                    "Subtractive Pipe"
                } else {
                    "Additive Pipe"
                }
            }
            PartFeature::Helix { subtractive, .. } => {
                if *subtractive {
                    "Subtractive Helix"
                } else {
                    "Additive Helix"
                }
            }
            PartFeature::Primitive { subtractive, .. } => {
                if *subtractive {
                    "Subtractive Primitive"
                } else {
                    "Primitive"
                }
            }
            PartFeature::Hole { .. } => "Hole",
            PartFeature::Fillet { .. } => "Fillet",
            PartFeature::Chamfer { .. } => "Chamfer",
            PartFeature::Draft { .. } => "Draft",
            PartFeature::Thickness { .. } => "Thickness",
            PartFeature::Mirrored { .. } => "Mirrored",
            PartFeature::LinearPattern { .. } => "Linear Pattern",
            PartFeature::PolarPattern { .. } => "Polar Pattern",
            PartFeature::MultiTransform { .. } => "Multi Transform",
            PartFeature::BodyBoolean { .. } => "Boolean",
            PartFeature::Borrow { .. } => "Borrowed geometry",
            PartFeature::Clone { .. } => "Clone",
        }
    }

    /// The design set's icon for this feature.
    pub fn icon(&self) -> &'static str {
        use kernel_api::PrimitiveKind as P;
        match self {
            PartFeature::Pad { .. } => "pad",
            PartFeature::Pocket { .. } => "pocket",
            PartFeature::Revolution { .. } => "revolution",
            PartFeature::Groove { .. } => "groove",
            PartFeature::Loft { subtractive, .. } => {
                if *subtractive {
                    "subtractive-loft"
                } else {
                    "additive-loft"
                }
            }
            PartFeature::Pipe { subtractive, .. } => {
                if *subtractive {
                    "subtractive-pipe"
                } else {
                    "additive-pipe"
                }
            }
            PartFeature::Helix { subtractive, .. } => {
                if *subtractive {
                    "subtractive-helix"
                } else {
                    "additive-helix"
                }
            }
            PartFeature::Primitive {
                kind, subtractive, ..
            } => {
                let shape = match kind {
                    P::Box { .. } => "box",
                    P::Cylinder { .. } => "cylinder",
                    P::Sphere { .. } => "sphere",
                    P::Cone { .. } => "cone",
                    P::Torus { .. } => "torus",
                    P::Ellipsoid { .. } => "ellipsoid",
                    P::Prism { .. } => "prism",
                    P::Wedge { .. } => "wedge",
                };
                primitive_icon(shape, *subtractive)
            }
            PartFeature::Hole { .. } => "hole",
            PartFeature::Fillet { .. } => "fillet",
            PartFeature::Chamfer { .. } => "chamfer",
            PartFeature::Draft { .. } => "draft",
            PartFeature::Thickness { .. } => "thickness",
            PartFeature::Mirrored { .. } => "mirrored",
            PartFeature::LinearPattern { .. } => "linear-pattern",
            PartFeature::PolarPattern { .. } => "polar-pattern",
            PartFeature::MultiTransform { .. } => "multi-transform",
            PartFeature::BodyBoolean { .. } => "boolean",
            PartFeature::Borrow { .. } => "clone-geometry",
            PartFeature::Clone { .. } => "clone",
        }
    }

    /// Whether the feature merges the coplanar faces its fuse or cut
    /// leaves behind.
    pub fn refine(&self) -> bool {
        match self {
            PartFeature::Pad { refine, .. }
            | PartFeature::Pocket { refine, .. }
            | PartFeature::Revolution { refine, .. }
            | PartFeature::Groove { refine, .. }
            | PartFeature::Loft { refine, .. }
            | PartFeature::Pipe { refine, .. }
            | PartFeature::Helix { refine, .. }
            | PartFeature::Primitive { refine, .. }
            | PartFeature::Hole { refine, .. }
            | PartFeature::Mirrored { refine, .. }
            | PartFeature::LinearPattern { refine, .. }
            | PartFeature::PolarPattern { refine, .. }
            | PartFeature::MultiTransform { refine, .. }
            | PartFeature::BodyBoolean { refine, .. } => *refine,
            _ => false,
        }
    }

    /// Set whether the feature refines its result. A feature that neither
    /// fuses nor cuts has nothing to refine and is left as it is.
    pub fn set_refine(&mut self, on: bool) {
        match self {
            PartFeature::Pad { refine, .. }
            | PartFeature::Pocket { refine, .. }
            | PartFeature::Revolution { refine, .. }
            | PartFeature::Groove { refine, .. }
            | PartFeature::Loft { refine, .. }
            | PartFeature::Pipe { refine, .. }
            | PartFeature::Helix { refine, .. }
            | PartFeature::Primitive { refine, .. }
            | PartFeature::Hole { refine, .. }
            | PartFeature::Mirrored { refine, .. }
            | PartFeature::LinearPattern { refine, .. }
            | PartFeature::PolarPattern { refine, .. }
            | PartFeature::MultiTransform { refine, .. }
            | PartFeature::BodyBoolean { refine, .. } => *refine = on,
            _ => {}
        }
    }

    /// Whether the feature has a refine switch at all.
    pub fn can_refine(&self) -> bool {
        let mut probe = self.clone();
        probe.set_refine(true);
        probe.refine()
    }

    /// True when this feature removes material (must not be a body's first).
    pub fn is_subtractive(&self) -> bool {
        match self {
            PartFeature::Pocket { .. } | PartFeature::Groove { .. } | PartFeature::Hole { .. } => {
                true
            }
            PartFeature::Loft { subtractive, .. }
            | PartFeature::Pipe { subtractive, .. }
            | PartFeature::Helix { subtractive, .. }
            | PartFeature::Primitive { subtractive, .. } => *subtractive,
            _ => false,
        }
    }

    /// True for features that modify the running solid instead of sweeping a
    /// new tool (dress-ups, patterns, booleans). These need existing material.
    pub fn is_modifier(&self) -> bool {
        matches!(
            self,
            PartFeature::Fillet { .. }
                | PartFeature::Chamfer { .. }
                | PartFeature::Draft { .. }
                | PartFeature::Thickness { .. }
                | PartFeature::Mirrored { .. }
                | PartFeature::LinearPattern { .. }
                | PartFeature::PolarPattern { .. }
                | PartFeature::MultiTransform { .. }
                | PartFeature::BodyBoolean { .. }
        )
    }
}

impl WorkbenchFeature for PartFeature {
    fn workbench_id() -> WorkbenchId {
        WorkbenchId::from("wb.part")
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
        let mut deps = self.sketches();
        if let Some(originals) = self.originals() {
            deps.extend_from_slice(originals);
        }
        if let PartFeature::Revolution {
            axis: RevolveAxis::Datum(datum),
            ..
        }
        | PartFeature::Groove {
            axis: RevolveAxis::Datum(datum),
            ..
        }
        | PartFeature::Helix {
            axis: RevolveAxis::Datum(datum),
            ..
        } = self
        {
            deps.push(*datum);
        }
        for borrow in self.borrows() {
            if !deps.contains(&borrow) {
                deps.push(borrow);
            }
        }
        // A borrowed sketch that follows its source changes with it.
        if let PartFeature::Borrow {
            source: BorrowSource::Sketch(sketch),
            frozen: None,
        } = self
        {
            deps.push(*sketch);
        }
        deps
    }

    fn name(&self) -> &str {
        self.kind_label()
    }
}

/// The icon of a primitive shape, additive or subtractive.
pub fn primitive_icon(shape: &str, subtractive: bool) -> &'static str {
    match (shape, subtractive) {
        ("box", false) => "additive-box",
        ("box", true) => "subtractive-box",
        ("cylinder", false) => "additive-cylinder",
        ("cylinder", true) => "subtractive-cylinder",
        ("sphere", false) => "additive-sphere",
        ("sphere", true) => "subtractive-sphere",
        ("cone", false) => "additive-cone",
        ("cone", true) => "subtractive-cone",
        ("torus", false) => "additive-torus",
        ("torus", true) => "subtractive-torus",
        ("ellipsoid", false) => "additive-ellipsoid",
        ("ellipsoid", true) => "subtractive-ellipsoid",
        ("prism", false) => "additive-prism",
        ("prism", true) => "subtractive-prism",
        ("wedge", false) => "additive-wedge",
        ("wedge", true) => "subtractive-wedge",
        (_, false) => "additive-box",
        (_, true) => "subtractive-box",
    }
}

/// A primitive of the named shape with default dimensions.
pub fn primitive_preset(shape: &str) -> Option<kernel_api::PrimitiveKind> {
    use kernel_api::PrimitiveKind as P;
    Some(match shape {
        "box" => P::Box {
            length: 10.0,
            width: 10.0,
            height: 10.0,
        },
        "cylinder" => P::Cylinder {
            radius: 5.0,
            height: 10.0,
            angle_deg: 360.0,
        },
        "sphere" => P::Sphere {
            radius: 5.0,
            angle1_deg: -90.0,
            angle2_deg: 90.0,
            angle3_deg: 360.0,
        },
        "cone" => P::Cone {
            radius1: 5.0,
            radius2: 2.0,
            height: 10.0,
            angle_deg: 360.0,
        },
        "torus" => P::Torus {
            radius1: 10.0,
            radius2: 2.0,
            angle1_deg: -180.0,
            angle2_deg: 180.0,
            angle3_deg: 360.0,
        },
        "ellipsoid" => P::Ellipsoid {
            radius1: 8.0,
            radius2: 5.0,
            radius3: 3.0,
        },
        "prism" => P::Prism {
            sides: 6,
            circumradius: 5.0,
            height: 10.0,
        },
        "wedge" => P::Wedge {
            xmin: 0.0,
            xmax: 10.0,
            ymin: 0.0,
            ymax: 10.0,
            zmin: 0.0,
            zmax: 10.0,
            x2min: 2.0,
            x2max: 8.0,
            z2min: 2.0,
            z2max: 8.0,
        },
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_pad_json_without_new_fields_still_deserializes() {
        // A Pad serialized before the termination-mode fields existed.
        let old = serde_json::json!({
            "Pad": { "sketch": FeatureId::new(), "length": 5.0, "reversed": false }
        });
        let feature = PartFeature::from_json(&old).unwrap();
        assert!(matches!(
            feature,
            PartFeature::Pad {
                symmetric: false,
                mode: ExtrudeMode::Dimension,
                taper_deg: t,
                up_to_face: None,
                ..
            } if t == 0.0
        ));
    }

    #[test]
    fn old_pocket_json_still_deserializes() {
        let old = serde_json::json!({
            "Pocket": {
                "sketch": FeatureId::new(),
                "depth": 5.0,
                "reversed": false,
                "through_all": true
            }
        });
        let feature = PartFeature::from_json(&old).unwrap();
        assert!(matches!(
            feature,
            PartFeature::Pocket {
                symmetric: false,
                through_all: true,
                mode: ExtrudeMode::Dimension,
                ..
            }
        ));
    }

    #[test]
    fn an_old_pad_reads_as_one_sided_along_the_normal_from_its_sketch() {
        let sketch = FeatureId::new();
        let old = serde_json::json!({
            "Pad": { "sketch": sketch, "length": 5.0, "reversed": false, "mode": "TwoLengths" }
        });
        let feature = PartFeature::from_json(&old).unwrap();
        assert_eq!(feature.sketch(), Some(sketch));
        let PartFeature::Pad {
            profile_face,
            direction,
            up_to_shape,
            mode,
            mode2,
            ..
        } = feature
        else {
            panic!("a pad");
        };
        assert_eq!(profile_face, None);
        assert_eq!(direction, ExtrudeDirection::Normal);
        assert!(up_to_shape.is_empty());
        // Two lengths is a dimension each way.
        assert_eq!(
            mode.sides(mode2),
            (ExtrudeMode::Dimension, Some(ExtrudeMode::Dimension))
        );
        assert_eq!(
            ExtrudeMode::UpToFace.sides(Some(ExtrudeMode::ThroughAll)),
            (ExtrudeMode::UpToFace, Some(ExtrudeMode::ThroughAll))
        );
        assert_eq!(
            ExtrudeMode::ToFirst.sides(None),
            (ExtrudeMode::ToFirst, None)
        );
    }

    #[test]
    fn a_datum_axis_is_a_dependency() {
        let (sketch, datum) = (FeatureId::new(), FeatureId::new());
        let old = serde_json::json!({
            "Revolution": { "sketch": sketch, "angle_deg": 180.0, "axis": {"Datum": datum} }
        });
        let feature = PartFeature::from_json(&old).unwrap();
        assert_eq!(feature.dependencies(), vec![sketch, datum]);
    }

    #[test]
    fn old_revolution_json_still_deserializes() {
        let old = serde_json::json!({
            "Revolution": { "sketch": FeatureId::new(), "angle_deg": 180.0 }
        });
        let feature = PartFeature::from_json(&old).unwrap();
        assert!(matches!(
            feature,
            PartFeature::Revolution {
                axis: RevolveAxis::SketchY,
                midplane: false,
                second_angle_deg: None,
                ..
            }
        ));
    }

    #[test]
    fn dependencies_cover_sketches_and_originals() {
        let a = FeatureId::new();
        let b = FeatureId::new();
        let pipe = PartFeature::Pipe {
            refine: false,
            profile: a,
            spine: b,
            frenet: false,
            subtractive: false,
        };
        assert_eq!(pipe.dependencies(), vec![a, b]);

        let pattern = PartFeature::LinearPattern {
            refine: false,
            originals: vec![a],
            axis: PatternAxis::X,
            length: 10.0,
            occurrences: 3,
            spacing_mode: false,
            reversed: false,
        };
        assert_eq!(pattern.dependencies(), vec![a]);
    }
}
