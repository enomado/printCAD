//! Core datum features: reference planes, lines, and points shared across
//! workbenches. A datum is a feature node (`workbench_id = "core.datum"`)
//! whose placement comes from an attachment plus a local offset, so
//! downstream sketches stay decoupled from generated-face topology churn.

use kernel_api::{ProbeAnswer, ShapeProbe};
use serde::{Deserialize, Serialize};

use crate::{DocumentResult, FeatureError, FeatureId, WorkbenchFeature, WorkbenchId};

/// The feature kind datums are stored under.
pub const DATUM_KIND: &str = "core.datum";

/// What geometry the datum represents.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum DatumShape {
    /// Reference plane, drawn as a `size`-sided square.
    Plane { size: f32 },
    /// Reference line along the attachment frame's normal-perpendicular
    /// x-axis, drawn `length` long.
    Line { length: f32 },
    /// Reference point at the attachment origin.
    Point,
    /// A local coordinate system: the frame's origin and its three axes
    /// (x, y and the normal as z), drawn `size` long. Its XY, XZ and YZ
    /// planes carry sketches the way the base planes do.
    CoordinateSystem { size: f32 },
}

impl DatumShape {
    pub fn label(&self) -> &'static str {
        match self {
            DatumShape::Plane { .. } => "Datum Plane",
            DatumShape::Line { .. } => "Datum Line",
            DatumShape::Point => "Datum Point",
            DatumShape::CoordinateSystem { .. } => "Local Coordinate System",
        }
    }
}

/// One of the document's three base planes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BasePlane {
    #[default]
    XY,
    XZ,
    YZ,
}

impl BasePlane {
    pub const ALL: [BasePlane; 3] = [BasePlane::XY, BasePlane::XZ, BasePlane::YZ];

    pub fn label(&self) -> &'static str {
        match self {
            BasePlane::XY => "XY (Top)",
            BasePlane::XZ => "XZ (Front)",
            BasePlane::YZ => "YZ (Side)",
        }
    }

    /// (origin, normal, x_axis) of the base plane.
    pub fn frame(&self) -> ([f32; 3], [f32; 3], [f32; 3]) {
        match self {
            BasePlane::XY => ([0.0; 3], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]),
            BasePlane::XZ => ([0.0; 3], [0.0, -1.0, 0.0], [1.0, 0.0, 0.0]),
            BasePlane::YZ => ([0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        }
    }
}

/// What the datum is anchored to. The references to a solid are kept
/// geometrically, in the body's own frame: a point and a direction, never a
/// face or edge number, which a rebuilt solid gives out afresh.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum DatumAttachment {
    /// One of the document base planes.
    BasePlane(BasePlane),
    /// A planar face, as a point and normal kept as they were picked.
    FlatFace { point: [f32; 3], normal: [f32; 3] },
    /// On a face at a point: its plane when the face is flat, the plane
    /// tangent to it there when it is curved.
    Face { face: FaceAnchor },
    /// The plane through three points; its x-axis runs from the first
    /// towards the second.
    ThreePoints { points: [PointAnchor; 3] },
    /// The plane square to an edge, where `spot` falls on it.
    NormalToEdge {
        edge: EdgeAnchor,
        #[serde(default)]
        spot: EdgeSpot,
    },
    /// Along a straight edge, centred on it; round a circular one, its
    /// axis through its centre.
    AlongEdge { edge: EdgeAnchor },
    /// The line through two points, centred between them.
    TwoPoints { points: [PointAnchor; 2] },
    /// The line where two planes meet, centred nearest their origins.
    PlaneIntersection { planes: [PlaneAnchor; 2] },
    /// The centre of a circular edge, square to it.
    CurveCentre { edge: EdgeAnchor },
    /// The body's centre of mass, on its principal axes of inertia: x the
    /// axis it turns about most easily, the normal the one it resists most.
    Inertia {
        centre: [f32; 3],
        axes: [[f32; 3]; 3],
    },
}

fn is_zero(name: &kernel_api::TopoName) -> bool {
    *name == 0
}

fn are_zero(names: &[kernel_api::TopoName; 2]) -> bool {
    names.iter().all(|n| *n == 0)
}

/// A face of a solid: a point on it and its outward normal there, and its
/// surface when known.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FaceAnchor {
    pub point: [f32; 3],
    pub normal: [f32; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface: Option<kernel_api::FaceSurface>,
    /// The face is on the datum's own body, whose rebuilds find it again,
    /// so the datum follows it.
    #[serde(default)]
    pub follows: bool,
    /// The face's name, which a rebuild finds it by (`kernel_api::naming`);
    /// zero when it has none, and the point finds it.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub name: kernel_api::TopoName,
}

/// An edge of a solid: a point on it and the way it runs there, with what
/// else is known of it (its ends, its middle, the circle it runs round).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EdgeAnchor {
    pub point: [f32; 3],
    pub direction: [f32; 3],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ends: Option<[[f32; 3]; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub middle: Option<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub circle: Option<AnchorCircle>,
    /// The edge is on the datum's own body, whose rebuilds find it again.
    #[serde(default)]
    pub follows: bool,
    /// The names of the two faces the edge runs between, which a rebuild
    /// finds it by; zeros when it has none, and the point finds it.
    #[serde(default, skip_serializing_if = "are_zero")]
    pub faces: [kernel_api::TopoName; 2],
}

/// The circle a circular edge runs round.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AnchorCircle {
    pub center: [f32; 3],
    pub normal: [f32; 3],
    pub radius: f32,
}

/// Which point of an edge a reference takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EdgeSpot {
    /// Where it was picked.
    #[default]
    Picked,
    Start,
    End,
    Middle,
    /// The centre of the circle it runs round.
    Centre,
}

impl EdgeSpot {
    pub const ALL: [EdgeSpot; 5] = [
        EdgeSpot::Picked,
        EdgeSpot::Start,
        EdgeSpot::End,
        EdgeSpot::Middle,
        EdgeSpot::Centre,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            EdgeSpot::Picked => "Where picked",
            EdgeSpot::Start => "Start",
            EdgeSpot::End => "End",
            EdgeSpot::Middle => "Middle",
            EdgeSpot::Centre => "Centre",
        }
    }

    /// The name a command takes it by.
    pub fn key(&self) -> &'static str {
        match self {
            EdgeSpot::Picked => "picked",
            EdgeSpot::Start => "start",
            EdgeSpot::End => "end",
            EdgeSpot::Middle => "middle",
            EdgeSpot::Centre => "centre",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.key() == key)
    }
}

/// A point a datum passes through.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PointAnchor {
    /// A point given by its coordinates.
    At { point: [f32; 3] },
    /// A point picked on a face.
    Face { face: FaceAnchor },
    /// A point of an edge.
    Edge {
        edge: EdgeAnchor,
        #[serde(default)]
        spot: EdgeSpot,
    },
}

impl PointAnchor {
    pub fn point(&self) -> [f32; 3] {
        match self {
            PointAnchor::At { point } => *point,
            PointAnchor::Face { face } => face.point,
            PointAnchor::Edge { edge, spot } => edge.spot_point(*spot),
        }
    }
}

/// A plane a datum is made from.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PlaneAnchor {
    Base(BasePlane),
    /// Another datum's plane (a coordinate system's XY plane), as it was
    /// last seen; the datum follows it.
    Datum {
        datum: FeatureId,
        origin: [f32; 3],
        normal: [f32; 3],
    },
    /// A face's plane, or the plane tangent to it at the point.
    Face {
        face: FaceAnchor,
    },
}

impl PlaneAnchor {
    /// (origin, normal) of the plane.
    pub fn plane(&self) -> ([f32; 3], [f32; 3]) {
        match self {
            PlaneAnchor::Base(plane) => {
                let (origin, normal, _) = plane.frame();
                (origin, normal)
            }
            PlaneAnchor::Datum { origin, normal, .. } => (*origin, normalize(*normal)),
            PlaneAnchor::Face { face } => (face.point, normalize(face.normal)),
        }
    }
}

impl EdgeAnchor {
    /// The point of the edge `spot` names; where it was picked when the
    /// spot is not known (an edge picked without its ends).
    pub fn spot_point(&self, spot: EdgeSpot) -> [f32; 3] {
        match spot {
            EdgeSpot::Picked => Some(self.point),
            EdgeSpot::Start => self.ends.map(|[a, _]| a),
            EdgeSpot::End => self.ends.map(|[_, b]| b),
            EdgeSpot::Middle => self.middle,
            EdgeSpot::Centre => self.circle.map(|c| c.center),
        }
        .unwrap_or(self.point)
    }

    /// The way the edge runs at `at`: square to the radius on a circle,
    /// turned the way it was picked; along the picked direction otherwise.
    fn tangent_at(&self, at: [f32; 3]) -> [f32; 3] {
        let picked = normalize(self.direction);
        if let Some(circle) = self.circle {
            let tangent = cross(circle.normal, sub(at, circle.center));
            if length(tangent) > 1e-6 {
                let tangent = normalize(tangent);
                return if dot(tangent, picked) < 0.0 {
                    scale(tangent, -1.0)
                } else {
                    tangent
                };
            }
        }
        picked
    }
}

impl DatumAttachment {
    pub fn label(&self) -> &'static str {
        match self {
            DatumAttachment::BasePlane(plane) => plane.label(),
            DatumAttachment::FlatFace { .. } => "Picked face",
            DatumAttachment::Face { .. } => "On a face",
            DatumAttachment::ThreePoints { .. } => "Through three points",
            DatumAttachment::NormalToEdge { .. } => "Square to an edge",
            DatumAttachment::AlongEdge { .. } => "Along an edge",
            DatumAttachment::TwoPoints { .. } => "Through two points",
            DatumAttachment::PlaneIntersection { .. } => "Where two planes meet",
            DatumAttachment::CurveCentre { .. } => "Centre of a curve",
            DatumAttachment::Inertia { .. } => "Centre of mass",
        }
    }

    /// The name a command takes the mode by.
    pub fn mode(&self) -> &'static str {
        match self {
            DatumAttachment::BasePlane(_) => "base_plane",
            DatumAttachment::FlatFace { .. } => "flat_face",
            DatumAttachment::Face { .. } => "face",
            DatumAttachment::ThreePoints { .. } => "three_points",
            DatumAttachment::NormalToEdge { .. } => "normal_to_edge",
            DatumAttachment::AlongEdge { .. } => "along_edge",
            DatumAttachment::TwoPoints { .. } => "two_points",
            DatumAttachment::PlaneIntersection { .. } => "plane_intersection",
            DatumAttachment::CurveCentre { .. } => "curve_centre",
            DatumAttachment::Inertia { .. } => "inertia",
        }
    }

    /// The other datums it is made from.
    pub fn datums(&self) -> Vec<FeatureId> {
        match self {
            DatumAttachment::PlaneIntersection { planes } => planes
                .iter()
                .filter_map(|p| match p {
                    PlaneAnchor::Datum { datum, .. } => Some(*datum),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Why the references make no frame, when they make none: points on
    /// top of each other or in a line, parallel planes, an edge with no
    /// circle for a centre.
    pub fn problem(&self) -> Option<&'static str> {
        const LEAST: f32 = 1e-4;
        let not_round = "the edge is not a circle or an arc of one";
        match self {
            DatumAttachment::ThreePoints { points } => {
                let [a, b, c] = points.map(|p| p.point());
                if length(sub(b, a)) < LEAST || length(sub(c, a)) < LEAST {
                    Some("two of the points are the same point")
                } else if length(cross(normalize(sub(b, a)), normalize(sub(c, a)))) < LEAST {
                    Some("the three points lie on one line")
                } else {
                    None
                }
            }
            DatumAttachment::TwoPoints { points } => {
                let [a, b] = points.map(|p| p.point());
                (length(sub(b, a)) < LEAST).then_some("the two points are the same point")
            }
            DatumAttachment::PlaneIntersection { planes } => {
                let [(_, n1), (_, n2)] = planes.map(|p| p.plane());
                (length(cross(n1, n2)) < LEAST).then_some("the two planes are parallel")
            }
            DatumAttachment::CurveCentre { edge } => edge.circle.is_none().then_some(not_round),
            DatumAttachment::NormalToEdge {
                edge,
                spot: EdgeSpot::Centre,
            } => edge.circle.is_none().then_some(not_round),
            _ => None,
        }
    }

    /// (origin, normal, x_axis) the references make, before the offset.
    fn base_frame(&self) -> ([f32; 3], [f32; 3], [f32; 3]) {
        match *self {
            DatumAttachment::BasePlane(plane) => plane.frame(),
            DatumAttachment::FlatFace { point, normal } => {
                let n = normalize(normal);
                (point, n, stable_x_axis(n))
            }
            DatumAttachment::Face { face } => {
                let n = normalize(face.normal);
                // Along a turned face's axis, so a line on a cylinder runs
                // along it.
                let along = face
                    .surface
                    .and_then(|s| s.axis())
                    .map(|(_, axis)| reject(axis, n))
                    .filter(|x| length(*x) > 1e-4);
                (
                    face.point,
                    n,
                    along.map(normalize).unwrap_or_else(|| stable_x_axis(n)),
                )
            }
            DatumAttachment::ThreePoints { points } => {
                let [a, b, c] = points.map(|p| p.point());
                let x = sub(b, a);
                let n = cross(x, sub(c, a));
                if length(x) < 1e-6 || length(n) < 1e-9 {
                    return along_x(a, x);
                }
                let n = normalize(n);
                (a, n, normalize(reject(x, n)))
            }
            DatumAttachment::NormalToEdge { edge, spot } => {
                let at = edge.spot_point(spot);
                // A circle's centre is off the edge: the plane there is
                // square to the edge where it was picked.
                let on_edge = if spot == EdgeSpot::Centre {
                    edge.point
                } else {
                    at
                };
                let tangent = edge.tangent_at(on_edge);
                // On a circle the x-axis points out from its centre.
                let radial = edge
                    .circle
                    .map(|c| reject(sub(on_edge, c.center), tangent))
                    .filter(|r| length(*r) > 1e-4);
                (
                    at,
                    tangent,
                    radial
                        .map(normalize)
                        .unwrap_or_else(|| stable_x_axis(tangent)),
                )
            }
            DatumAttachment::AlongEdge { edge } => match edge.circle {
                Some(circle) => along_x(circle.center, circle.normal),
                None => {
                    let middle = match edge.ends {
                        Some([a, b]) => scale(add(a, b), 0.5),
                        None => edge.middle.unwrap_or(edge.point),
                    };
                    along_x(middle, edge.direction)
                }
            },
            DatumAttachment::TwoPoints { points } => {
                let [a, b] = points.map(|p| p.point());
                along_x(scale(add(a, b), 0.5), sub(b, a))
            }
            DatumAttachment::PlaneIntersection { planes } => {
                let [(o1, n1), (o2, n2)] = planes.map(|p| p.plane());
                let u = cross(n1, n2);
                let uu = dot(u, u);
                if uu < 1e-12 {
                    return (o1, n1, stable_x_axis(n1));
                }
                let (d1, d2) = (dot(n1, o1), dot(n2, o2));
                let on_both = scale(
                    add(scale(cross(n2, u), d1), scale(cross(u, n1), d2)),
                    1.0 / uu,
                );
                // The point of the line nearest the planes' own origins.
                let x = normalize(u);
                let middle = scale(add(o1, o2), 0.5);
                let origin = scale_add(on_both, x, dot(x, sub(middle, on_both)));
                (origin, normalize(reject(n1, x)), x)
            }
            DatumAttachment::CurveCentre { edge } => match edge.circle {
                Some(circle) => {
                    let n = normalize(circle.normal);
                    let radial = reject(sub(edge.point, circle.center), n);
                    let x = if length(radial) > 1e-4 {
                        normalize(radial)
                    } else {
                        stable_x_axis(n)
                    };
                    (circle.center, n, x)
                }
                None => along_x(edge.point, edge.direction),
            },
            DatumAttachment::Inertia { centre, axes } => {
                let x = normalize(axes[0]);
                let n = normalize(reject(axes[2], x));
                (centre, n, x)
            }
        }
    }
}

/// A frame whose x-axis runs along `x` through `origin`, the normal square
/// to it.
fn along_x(origin: [f32; 3], x: [f32; 3]) -> ([f32; 3], [f32; 3], [f32; 3]) {
    let x = normalize(x);
    (origin, stable_x_axis(x), x)
}

/// Extra placement applied IN the attachment coordinate system.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct AttachmentOffset {
    /// Translation along the attachment x/y/normal axes (millimetres).
    pub translation: [f32; 3],
    /// In-plane rotation about the attachment normal (degrees).
    pub rotation_deg: f32,
    /// Flip to the other side (180° about the local x-axis).
    pub flip: bool,
}

/// A datum feature payload.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DatumFeature {
    pub shape: DatumShape,
    pub attachment: DatumAttachment,
    #[serde(default)]
    pub offset: AttachmentOffset,
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len < 1e-9 {
        return [0.0, 0.0, 1.0];
    }
    [v[0] / len, v[1] / len, v[2] / len]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn scale_add(p: [f32; 3], v: [f32; 3], s: f32) -> [f32; 3] {
    [p[0] + v[0] * s, p[1] + v[1] * s, p[2] + v[2] * s]
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn scale(v: [f32; 3], s: f32) -> [f32; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn length(v: [f32; 3]) -> f32 {
    dot(v, v).sqrt()
}

/// `v` less its part along the unit vector `n`.
fn reject(v: [f32; 3], n: [f32; 3]) -> [f32; 3] {
    scale_add(v, n, -dot(v, n))
}

/// Stable in-plane basis for an arbitrary normal: pick the world axis least
/// aligned with the normal and orthogonalize it.
fn stable_x_axis(normal: [f32; 3]) -> [f32; 3] {
    let candidates = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let mut best = candidates[0];
    let mut best_dot = f32::MAX;
    for candidate in candidates {
        let dot =
            (candidate[0] * normal[0] + candidate[1] * normal[1] + candidate[2] * normal[2]).abs();
        if dot < best_dot {
            best_dot = dot;
            best = candidate;
        }
    }
    let dot = best[0] * normal[0] + best[1] * normal[1] + best[2] * normal[2];
    normalize([
        best[0] - normal[0] * dot,
        best[1] - normal[1] * dot,
        best[2] - normal[2] * dot,
    ])
}

/// A resolved datum placement in world coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DatumFrame {
    pub origin: [f32; 3],
    pub normal: [f32; 3],
    pub x_axis: [f32; 3],
}

impl DatumFrame {
    pub fn y_axis(&self) -> [f32; 3] {
        cross(self.normal, self.x_axis)
    }

    /// The frame's own XY, XZ and YZ planes, labelled, laid out as the
    /// document's base planes are on the world axes.
    pub fn planes(&self) -> [(&'static str, DatumFrame); 3] {
        let (x, y, z) = (self.x_axis, self.y_axis(), self.normal);
        let plane = |normal, x_axis| DatumFrame {
            origin: self.origin,
            normal,
            x_axis,
        };
        [
            ("XY", plane(z, x)),
            ("XZ", plane([-y[0], -y[1], -y[2]], x)),
            ("YZ", plane(x, y)),
        ]
    }
}

impl DatumFeature {
    /// Resolve the attachment + offset into a world placement.
    pub fn frame(&self) -> DatumFrame {
        let (origin, normal, x_axis) = self.attachment.base_frame();
        let mut normal = normalize(normal);
        let mut x_axis = normalize(x_axis);
        let mut y_axis = cross(normal, x_axis);

        // In-plane rotation about the normal.
        let rot = self.offset.rotation_deg.to_radians();
        if rot.abs() > 1e-9 {
            let (s, c) = rot.sin_cos();
            let rotated = [
                x_axis[0] * c + y_axis[0] * s,
                x_axis[1] * c + y_axis[1] * s,
                x_axis[2] * c + y_axis[2] * s,
            ];
            x_axis = normalize(rotated);
            y_axis = cross(normal, x_axis);
        }

        let mut origin = origin;
        origin = scale_add(origin, x_axis, self.offset.translation[0]);
        origin = scale_add(origin, y_axis, self.offset.translation[1]);
        origin = scale_add(origin, normal, self.offset.translation[2]);

        if self.offset.flip {
            // 180° about the local x-axis: the normal reverses, x stays.
            normal = [-normal[0], -normal[1], -normal[2]];
        }
        DatumFrame {
            origin,
            normal,
            x_axis,
        }
    }
}

impl WorkbenchFeature for DatumFeature {
    fn workbench_id() -> WorkbenchId {
        WorkbenchId::from(DATUM_KIND)
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    fn from_json(value: &serde_json::Value) -> DocumentResult<Self> {
        serde_json::from_value(value.clone()).map_err(|e| {
            crate::DocumentError::Feature(FeatureError::Deserialization(e.to_string()))
        })
    }

    fn dependencies(&self) -> Vec<FeatureId> {
        self.attachment.datums()
    }

    fn name(&self) -> &str {
        self.shape.label()
    }
}

/// A reference of a datum a build finds again on the body's solid.
enum Followed<'a> {
    Face(&'a mut FaceAnchor),
    Edge(&'a mut EdgeAnchor),
    Mass(&'a mut [f32; 3], &'a mut [[f32; 3]; 3]),
}

fn f64s(v: [f32; 3]) -> [f64; 3] {
    v.map(f64::from)
}

fn f32s(v: [f64; 3]) -> [f32; 3] {
    v.map(|c| c as f32)
}

impl DatumFeature {
    /// The references that follow the body's solid, in a fixed order.
    fn followed(&mut self) -> Vec<Followed<'_>> {
        let mut followed = Vec::new();
        match &mut self.attachment {
            DatumAttachment::BasePlane(_) | DatumAttachment::FlatFace { .. } => {}
            DatumAttachment::Face { face } => followed.push(Followed::Face(face)),
            DatumAttachment::ThreePoints { points } => {
                for anchor in points.iter_mut() {
                    push_point(anchor, &mut followed);
                }
            }
            DatumAttachment::TwoPoints { points } => {
                for anchor in points.iter_mut() {
                    push_point(anchor, &mut followed);
                }
            }
            DatumAttachment::NormalToEdge { edge, .. }
            | DatumAttachment::AlongEdge { edge }
            | DatumAttachment::CurveCentre { edge } => followed.push(Followed::Edge(edge)),
            DatumAttachment::PlaneIntersection { planes } => {
                for plane in planes.iter_mut() {
                    if let PlaneAnchor::Face { face } = plane {
                        followed.push(Followed::Face(face));
                    }
                }
            }
            DatumAttachment::Inertia { centre, axes } => {
                followed.push(Followed::Mass(centre, axes));
            }
        }
        followed.retain(|f| match f {
            Followed::Face(face) => face.follows,
            Followed::Edge(edge) => edge.follows,
            Followed::Mass(..) => true,
        });
        followed
    }

    /// What a build asks of the body's solid to find this datum's
    /// references again, in a fixed order.
    pub fn probes(&self) -> Vec<ShapeProbe> {
        let mut copy = *self;
        copy.followed()
            .into_iter()
            .map(|f| match f {
                Followed::Face(face) => ShapeProbe::Face {
                    point: f64s(face.point),
                    normal: f64s(face.normal),
                    name: face.name,
                },
                Followed::Edge(edge) => ShapeProbe::Edge {
                    point: f64s(edge.point),
                    direction: f64s(edge.direction),
                    faces: edge.faces,
                },
                Followed::Mass(..) => ShapeProbe::Mass,
            })
            .collect()
    }

    /// Take what the body's solid answered to [`Self::probes`], in the
    /// same order; a reference whose answer failed or does not fit keeps
    /// what it had.
    pub fn take_answers(&mut self, answers: &[Result<ProbeAnswer, String>]) {
        for (reference, answer) in self.followed().into_iter().zip(answers) {
            match (reference, answer) {
                (
                    Followed::Face(face),
                    Ok(ProbeAnswer::Face {
                        point,
                        normal,
                        surface,
                    }),
                ) => {
                    face.point = f32s(*point);
                    face.normal = f32s(*normal);
                    face.surface = Some(*surface);
                }
                (
                    Followed::Edge(edge),
                    Ok(ProbeAnswer::Edge {
                        point,
                        direction,
                        start,
                        end,
                        middle,
                        circle,
                    }),
                ) => {
                    edge.point = f32s(*point);
                    edge.direction = f32s(*direction);
                    edge.ends = Some([f32s(*start), f32s(*end)]);
                    edge.middle = Some(f32s(*middle));
                    edge.circle = circle.map(|c| AnchorCircle {
                        center: f32s(c.centre),
                        normal: f32s(c.normal),
                        radius: c.radius as f32,
                    });
                }
                (Followed::Mass(centre, axes), Ok(ProbeAnswer::Mass { centre: c, axes: a })) => {
                    *centre = f32s(*c);
                    *axes = a.map(f32s);
                }
                _ => {}
            }
        }
    }

    /// Take the planes of the other datums this one is made from, as
    /// `frame_of` gives them now.
    pub fn follow_datums(&mut self, frame_of: &dyn Fn(FeatureId) -> Option<DatumFrame>) {
        if let DatumAttachment::PlaneIntersection { planes } = &mut self.attachment {
            for plane in planes.iter_mut() {
                if let PlaneAnchor::Datum {
                    datum,
                    origin,
                    normal,
                } = plane
                    && let Some(frame) = frame_of(*datum)
                {
                    *origin = frame.origin;
                    *normal = frame.normal;
                }
            }
        }
    }
}

fn push_point<'a>(anchor: &'a mut PointAnchor, into: &mut Vec<Followed<'a>>) {
    match anchor {
        PointAnchor::At { .. } => {}
        PointAnchor::Face { face } => into.push(Followed::Face(face)),
        PointAnchor::Edge { edge, .. } => into.push(Followed::Edge(edge)),
    }
}

/// Bring a datum's working data up to date with what it stands on: the
/// answers the last build found of its references on the body's solid
/// (when they answer the references it has now), and the planes of the
/// datums it is made from. Answers whether anything changed.
pub fn derive(
    values: &mut serde_json::Value,
    probed: Option<&crate::rebuild::ProbedReferences>,
    values_of: &dyn Fn(FeatureId) -> Option<serde_json::Value>,
) -> bool {
    let Ok(mut datum) = DatumFeature::from_json(values) else {
        return false;
    };
    let before = datum;
    if let Some(probed) = probed
        && probed.probes == datum.probes()
    {
        datum.take_answers(&probed.answers);
    }
    datum.follow_datums(&|id| {
        let data = values_of(id)?;
        DatumFeature::from_json(&data).ok().map(|d| d.frame())
    });
    if datum == before {
        return false;
    }
    *values = datum.to_json();
    true
}

/// All datum features of a body, resolved and named.
pub fn datums_of_body(
    document: &crate::Document,
    body: crate::BodyId,
) -> Vec<(FeatureId, String, DatumFeature)> {
    let mut datums: Vec<(u64, FeatureId, String, DatumFeature)> = document
        .feature_tree()
        .all_nodes()
        .filter(|(_, n)| n.workbench_id.as_str() == "core.datum" && n.body == Some(body))
        .filter_map(|(id, n)| {
            DatumFeature::from_json(document.feature_values(*id)?)
                .ok()
                .map(|d| (n.seq, *id, n.name.clone(), d))
        })
        .collect();
    datums.sort_by_key(|(seq, ..)| *seq);
    datums
        .into_iter()
        .map(|(_, id, name, d)| (id, name, d))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-5)
    }

    #[test]
    fn base_plane_with_offset_translates_along_normal() {
        let datum = DatumFeature {
            shape: DatumShape::Plane { size: 20.0 },
            attachment: DatumAttachment::BasePlane(BasePlane::XY),
            offset: AttachmentOffset {
                translation: [0.0, 0.0, 7.5],
                rotation_deg: 0.0,
                flip: false,
            },
        };
        let frame = datum.frame();
        assert!(close(frame.origin, [0.0, 0.0, 7.5]));
        assert!(close(frame.normal, [0.0, 0.0, 1.0]));
    }

    #[test]
    fn flat_face_attachment_derives_orthonormal_frame() {
        let datum = DatumFeature {
            shape: DatumShape::Plane { size: 20.0 },
            attachment: DatumAttachment::FlatFace {
                point: [3.0, 4.0, 5.0],
                normal: [0.0, 3.0, 4.0],
            },
            offset: AttachmentOffset::default(),
        };
        let frame = datum.frame();
        let n = frame.normal;
        let x = frame.x_axis;
        let dot = n[0] * x[0] + n[1] * x[1] + n[2] * x[2];
        assert!(dot.abs() < 1e-5, "x-axis orthogonal to normal");
        assert!((n[0] * n[0] + n[1] * n[1] + n[2] * n[2] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn flip_reverses_the_normal() {
        let datum = DatumFeature {
            shape: DatumShape::Plane { size: 20.0 },
            attachment: DatumAttachment::BasePlane(BasePlane::XY),
            offset: AttachmentOffset {
                translation: [0.0; 3],
                rotation_deg: 0.0,
                flip: true,
            },
        };
        assert!(close(datum.frame().normal, [0.0, 0.0, -1.0]));
    }

    #[test]
    fn rotation_spins_the_x_axis_in_plane() {
        let datum = DatumFeature {
            shape: DatumShape::Plane { size: 20.0 },
            attachment: DatumAttachment::BasePlane(BasePlane::XY),
            offset: AttachmentOffset {
                translation: [0.0; 3],
                rotation_deg: 90.0,
                flip: false,
            },
        };
        assert!(close(datum.frame().x_axis, [0.0, 1.0, 0.0]));
    }

    #[test]
    fn a_coordinate_system_lays_out_its_planes_as_the_base_planes_are() {
        let datum = DatumFeature {
            shape: DatumShape::CoordinateSystem { size: 20.0 },
            attachment: DatumAttachment::BasePlane(BasePlane::XY),
            offset: AttachmentOffset {
                translation: [1.0, 2.0, 3.0],
                rotation_deg: 0.0,
                flip: false,
            },
        };
        let frame = datum.frame();
        for ((label, plane), base) in frame.planes().into_iter().zip(BasePlane::ALL) {
            let (_, base_normal, base_x) = base.frame();
            assert!(close(plane.origin, [1.0, 2.0, 3.0]), "{label}");
            assert!(close(plane.normal, base_normal), "{label}");
            assert!(close(plane.x_axis, base_x), "{label}");
        }
    }

    #[test]
    fn json_round_trip() {
        let datum = DatumFeature {
            shape: DatumShape::Line { length: 30.0 },
            attachment: DatumAttachment::FlatFace {
                point: [1.0, 2.0, 3.0],
                normal: [0.0, 0.0, 1.0],
            },
            offset: AttachmentOffset {
                translation: [1.0, 2.0, 3.0],
                rotation_deg: 15.0,
                flip: true,
            },
        };
        let json = datum.to_json();
        assert_eq!(DatumFeature::from_json(&json).unwrap(), datum);
    }

    fn datum(shape: DatumShape, attachment: DatumAttachment) -> DatumFeature {
        DatumFeature {
            shape,
            attachment,
            offset: AttachmentOffset::default(),
        }
    }

    fn plane(attachment: DatumAttachment) -> DatumFrame {
        datum(DatumShape::Plane { size: 20.0 }, attachment).frame()
    }

    fn line(attachment: DatumAttachment) -> DatumFrame {
        datum(DatumShape::Line { length: 20.0 }, attachment).frame()
    }

    fn parallel(a: [f32; 3], b: [f32; 3]) -> bool {
        (dot(normalize(a), normalize(b)).abs() - 1.0).abs() < 1e-5
    }

    fn orthonormal(frame: &DatumFrame) {
        assert!((length(frame.normal) - 1.0).abs() < 1e-5);
        assert!((length(frame.x_axis) - 1.0).abs() < 1e-5);
        assert!(dot(frame.normal, frame.x_axis).abs() < 1e-5);
    }

    fn at(point: [f32; 3]) -> PointAnchor {
        PointAnchor::At { point }
    }

    fn straight_edge() -> EdgeAnchor {
        EdgeAnchor {
            faces: [0, 0],
            point: [2.0, 0.0, 0.0],
            direction: [1.0, 0.0, 0.0],
            ends: Some([[0.0, 0.0, 0.0], [10.0, 0.0, 0.0]]),
            middle: Some([5.0, 0.0, 0.0]),
            circle: None,
            follows: true,
        }
    }

    fn rim() -> EdgeAnchor {
        EdgeAnchor {
            faces: [0, 0],
            point: [5.0, 0.0, 12.0],
            direction: [0.0, 1.0, 0.0],
            ends: None,
            middle: None,
            circle: Some(AnchorCircle {
                center: [0.0, 0.0, 12.0],
                normal: [0.0, 0.0, 1.0],
                radius: 5.0,
            }),
            follows: true,
        }
    }

    fn cylinder_face() -> FaceAnchor {
        FaceAnchor {
            name: 0,
            point: [0.0, 5.0, 3.0],
            normal: [0.0, 1.0, 0.0],
            surface: Some(kernel_api::FaceSurface::Cylinder {
                origin: [0.0; 3],
                axis: [0.0, 0.0, 1.0],
                radius: 5.0,
            }),
            follows: true,
        }
    }

    #[test]
    fn a_plane_on_a_cylinder_is_tangent_where_picked_its_x_along_the_axis() {
        let frame = plane(DatumAttachment::Face {
            face: cylinder_face(),
        });
        orthonormal(&frame);
        assert!(close(frame.origin, [0.0, 5.0, 3.0]));
        assert!(close(frame.normal, [0.0, 1.0, 0.0]));
        assert!(parallel(frame.x_axis, [0.0, 0.0, 1.0]));
    }

    #[test]
    fn three_points_make_the_plane_through_them() {
        let frame = plane(DatumAttachment::ThreePoints {
            points: [
                at([1.0, 1.0, 2.0]),
                at([4.0, 1.0, 2.0]),
                at([1.0, 5.0, 2.0]),
            ],
        });
        orthonormal(&frame);
        assert!(close(frame.origin, [1.0, 1.0, 2.0]));
        assert!(close(frame.normal, [0.0, 0.0, 1.0]));
        assert!(close(frame.x_axis, [1.0, 0.0, 0.0]));
    }

    #[test]
    fn points_in_a_line_or_on_top_of_each_other_are_refused() {
        let collinear = DatumAttachment::ThreePoints {
            points: [at([0.0; 3]), at([1.0, 0.0, 0.0]), at([2.0, 0.0, 0.0])],
        };
        assert!(collinear.problem().is_some());
        let same = DatumAttachment::TwoPoints {
            points: [at([1.0; 3]), at([1.0; 3])],
        };
        assert!(same.problem().is_some());
        let fine = DatumAttachment::TwoPoints {
            points: [at([0.0; 3]), at([1.0; 3])],
        };
        assert!(fine.problem().is_none());
    }

    #[test]
    fn a_plane_square_to_an_edge_stands_where_the_spot_is() {
        let frame = plane(DatumAttachment::NormalToEdge {
            edge: straight_edge(),
            spot: EdgeSpot::End,
        });
        orthonormal(&frame);
        assert!(close(frame.origin, [10.0, 0.0, 0.0]));
        assert!(parallel(frame.normal, [1.0, 0.0, 0.0]));

        // On a circle: square to it where picked, x out from the centre.
        let frame = plane(DatumAttachment::NormalToEdge {
            edge: rim(),
            spot: EdgeSpot::Picked,
        });
        orthonormal(&frame);
        assert!(close(frame.origin, [5.0, 0.0, 12.0]));
        assert!(close(frame.normal, [0.0, 1.0, 0.0]));
        assert!(close(frame.x_axis, [1.0, 0.0, 0.0]));
    }

    #[test]
    fn a_line_along_an_edge_runs_along_it_or_round_a_circle_on_its_axis() {
        let frame = line(DatumAttachment::AlongEdge {
            edge: straight_edge(),
        });
        orthonormal(&frame);
        assert!(close(frame.origin, [5.0, 0.0, 0.0]));
        assert!(parallel(frame.x_axis, [1.0, 0.0, 0.0]));

        let frame = line(DatumAttachment::AlongEdge { edge: rim() });
        assert!(close(frame.origin, [0.0, 0.0, 12.0]));
        assert!(parallel(frame.x_axis, [0.0, 0.0, 1.0]));
    }

    #[test]
    fn a_line_through_two_points_is_centred_between_them() {
        let frame = line(DatumAttachment::TwoPoints {
            points: [
                at([0.0, 0.0, 0.0]),
                PointAnchor::Edge {
                    edge: straight_edge(),
                    spot: EdgeSpot::End,
                },
            ],
        });
        orthonormal(&frame);
        assert!(close(frame.origin, [5.0, 0.0, 0.0]));
        assert!(parallel(frame.x_axis, [1.0, 0.0, 0.0]));
    }

    #[test]
    fn two_planes_meet_in_a_line_on_both() {
        let attachment = DatumAttachment::PlaneIntersection {
            planes: [
                PlaneAnchor::Datum {
                    datum: FeatureId(uuid::Uuid::nil()),
                    origin: [0.0, 0.0, 4.0],
                    normal: [0.0, 0.0, 1.0],
                },
                PlaneAnchor::Base(BasePlane::YZ),
            ],
        };
        let frame = line(attachment);
        orthonormal(&frame);
        assert!(parallel(frame.x_axis, [0.0, 1.0, 0.0]));
        assert!(frame.origin[0].abs() < 1e-5 && (frame.origin[2] - 4.0).abs() < 1e-5);
        assert_eq!(attachment.datums(), vec![FeatureId(uuid::Uuid::nil())]);

        let parallel_planes = DatumAttachment::PlaneIntersection {
            planes: [
                PlaneAnchor::Base(BasePlane::XY),
                PlaneAnchor::Face {
                    face: FaceAnchor {
                        name: 0,
                        point: [0.0, 0.0, 3.0],
                        normal: [0.0, 0.0, -1.0],
                        surface: None,
                        follows: false,
                    },
                },
            ],
        };
        assert!(parallel_planes.problem().is_some());
    }

    #[test]
    fn a_point_at_a_curve_s_centre_sits_on_the_circle_s_centre() {
        let frame = datum(
            DatumShape::Point,
            DatumAttachment::CurveCentre { edge: rim() },
        )
        .frame();
        assert!(close(frame.origin, [0.0, 0.0, 12.0]));
        assert!(parallel(frame.normal, [0.0, 0.0, 1.0]));
        let mut straight = straight_edge();
        straight.circle = None;
        assert!(
            DatumAttachment::CurveCentre { edge: straight }
                .problem()
                .is_some()
        );
    }

    #[test]
    fn inertia_axes_make_the_coordinate_system() {
        let frame = datum(
            DatumShape::CoordinateSystem { size: 10.0 },
            DatumAttachment::Inertia {
                centre: [1.0, 2.0, 3.0],
                axes: [[0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            },
        )
        .frame();
        orthonormal(&frame);
        assert!(close(frame.origin, [1.0, 2.0, 3.0]));
        assert!(close(frame.x_axis, [0.0, 0.0, 1.0]));
        assert!(close(frame.y_axis(), [1.0, 0.0, 0.0]));
        assert!(close(frame.normal, [0.0, 1.0, 0.0]));
    }

    #[test]
    fn only_references_on_the_body_are_probed_and_answers_land_in_order() {
        let mut other = cylinder_face();
        other.follows = false;
        let mut datum = datum(
            DatumShape::Plane { size: 20.0 },
            DatumAttachment::ThreePoints {
                points: [
                    PointAnchor::Face {
                        face: cylinder_face(),
                    },
                    PointAnchor::Face { face: other },
                    PointAnchor::Edge {
                        edge: straight_edge(),
                        spot: EdgeSpot::Start,
                    },
                ],
            },
        );
        let probes = datum.probes();
        assert_eq!(probes.len(), 2);
        assert!(matches!(probes[0], ShapeProbe::Face { .. }));
        assert!(matches!(probes[1], ShapeProbe::Edge { .. }));

        datum.take_answers(&[
            Ok(ProbeAnswer::Face {
                point: [0.0, 8.0, 3.0],
                normal: [0.0, 1.0, 0.0],
                surface: kernel_api::FaceSurface::Other,
            }),
            Ok(ProbeAnswer::Edge {
                point: [2.0, 0.0, 0.0],
                direction: [1.0, 0.0, 0.0],
                start: [-1.0, 0.0, 0.0],
                end: [11.0, 0.0, 0.0],
                middle: [5.0, 0.0, 0.0],
                circle: None,
            }),
        ]);
        let DatumAttachment::ThreePoints { points } = datum.attachment else {
            unreachable!()
        };
        assert!(close(points[0].point(), [0.0, 8.0, 3.0]));
        assert!(close(points[1].point(), [0.0, 5.0, 3.0]), "not probed");
        assert!(close(points[2].point(), [-1.0, 0.0, 0.0]));
    }

    #[test]
    fn derive_takes_answers_only_for_the_references_they_were_asked_of() {
        let datum = datum(
            DatumShape::Plane { size: 20.0 },
            DatumAttachment::Face {
                face: cylinder_face(),
            },
        );
        let answer = Ok(ProbeAnswer::Face {
            point: [0.0, 8.0, 3.0],
            normal: [0.0, 1.0, 0.0],
            surface: kernel_api::FaceSurface::Other,
        });
        let none = |_: FeatureId| None;
        let mut values = datum.to_json();
        let stale = crate::rebuild::ProbedReferences {
            probes: vec![ShapeProbe::Mass],
            answers: vec![answer.clone()],
        };
        assert!(!derive(&mut values, Some(&stale), &none));
        let fresh = crate::rebuild::ProbedReferences {
            probes: datum.probes(),
            answers: vec![answer],
        };
        assert!(derive(&mut values, Some(&fresh), &none));
        let followed = DatumFeature::from_json(&values).unwrap().frame();
        assert!(close(followed.origin, [0.0, 8.0, 3.0]));
    }

    #[test]
    fn derive_follows_the_datums_a_line_is_made_from() {
        let other = FeatureId(uuid::Uuid::new_v4());
        let line = datum(
            DatumShape::Line { length: 20.0 },
            DatumAttachment::PlaneIntersection {
                planes: [
                    PlaneAnchor::Datum {
                        datum: other,
                        origin: [0.0; 3],
                        normal: [0.0, 0.0, 1.0],
                    },
                    PlaneAnchor::Base(BasePlane::YZ),
                ],
            },
        );
        let moved = DatumFeature {
            shape: DatumShape::Plane { size: 20.0 },
            attachment: DatumAttachment::BasePlane(BasePlane::XY),
            offset: AttachmentOffset {
                translation: [0.0, 0.0, 6.0],
                rotation_deg: 0.0,
                flip: false,
            },
        };
        let values_of = |id: FeatureId| (id == other).then(|| moved.to_json());
        let mut values = line.to_json();
        assert!(derive(&mut values, None, &values_of));
        let frame = DatumFeature::from_json(&values).unwrap().frame();
        assert!((frame.origin[2] - 6.0).abs() < 1e-5, "{:?}", frame.origin);
    }

    #[test]
    fn every_mode_round_trips_through_json() {
        let modes = [
            DatumAttachment::Face {
                face: cylinder_face(),
            },
            DatumAttachment::ThreePoints {
                points: [
                    at([0.0; 3]),
                    PointAnchor::Face {
                        face: cylinder_face(),
                    },
                    PointAnchor::Edge {
                        edge: rim(),
                        spot: EdgeSpot::Centre,
                    },
                ],
            },
            DatumAttachment::NormalToEdge {
                edge: straight_edge(),
                spot: EdgeSpot::Middle,
            },
            DatumAttachment::AlongEdge { edge: rim() },
            DatumAttachment::TwoPoints {
                points: [at([0.0; 3]), at([1.0; 3])],
            },
            DatumAttachment::PlaneIntersection {
                planes: [
                    PlaneAnchor::Base(BasePlane::XZ),
                    PlaneAnchor::Face {
                        face: cylinder_face(),
                    },
                ],
            },
            DatumAttachment::CurveCentre { edge: rim() },
            DatumAttachment::Inertia {
                centre: [1.0; 3],
                axes: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            },
        ];
        for attachment in modes {
            let datum = datum(DatumShape::Point, attachment);
            assert_eq!(DatumFeature::from_json(&datum.to_json()).unwrap(), datum);
        }
    }

    #[test]
    fn a_datum_saved_before_the_modes_still_loads() {
        let old = serde_json::json!({
            "shape": { "Plane": { "size": 30.0 } },
            "attachment": { "FlatFace": { "point": [1.0, 2.0, 3.0], "normal": [0.0, 0.0, 1.0] } },
        });
        let datum = DatumFeature::from_json(&old).unwrap();
        assert!(close(datum.frame().origin, [1.0, 2.0, 3.0]));
        assert!(datum.probes().is_empty());
    }
}
