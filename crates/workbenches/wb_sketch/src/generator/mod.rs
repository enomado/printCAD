//! Generated profiles: a sketch whose curves are made from a few numbers
//! (an involute gear, a chain sprocket, a stepped shaft) rather than drawn.
//!
//! The numbers live on the sketch ([`crate::SketchFeature::generator`]);
//! the curves are made again from them whenever they change, by hand, by a
//! formula (every number is a parameter formulas can set) or by a command.
//! Everything that takes a sketch takes a generated one: a Pad pads the
//! gear, a Revolution turns the shaft. The curves carry ids derived from
//! the sketch's own, so making them again from the same numbers gives the
//! same sketch.

mod fit;
mod gear;
#[cfg(feature = "egui")]
pub(crate) mod panel;
mod shaft;
mod sprocket;

use core_document::expr::Dim;
use core_document::{FeatureNode, Parameter, WorkbenchFeature};
use serde::{Deserialize, Deserializer, Serialize};
use uuid::Uuid;

use crate::feature::SketchFeature;
use crate::sketch::{Arc, BSpline, Circle, GeometryElement, Line, Point, Sketch, Vec2D};

pub use gear::GearSpec;
pub use shaft::{ShaftSection, ShaftSpec};
pub use sprocket::{SPROCKET_CHAINS, SprocketSpec};

/// What a generated sketch is made from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Generator {
    Gear(GearSpec),
    Sprocket(SprocketSpec),
    Shaft(ShaftSpec),
}

impl Generator {
    /// The generator a command or tool names: `gear`, `sprocket` or
    /// `shaft`, with its defaults.
    pub fn named(kind: &str) -> Option<Self> {
        Some(match kind {
            "gear" => Generator::Gear(GearSpec::default()),
            "sprocket" => Generator::Sprocket(SprocketSpec::default()),
            "shaft" => Generator::Shaft(ShaftSpec::default()),
            _ => return None,
        })
    }

    /// `gear`, `sprocket` or `shaft`.
    pub fn kind(&self) -> &'static str {
        match self {
            Generator::Gear(_) => "gear",
            Generator::Sprocket(_) => "sprocket",
            Generator::Shaft(_) => "shaft",
        }
    }

    /// What the tree and the panel call it.
    pub fn label(&self) -> &'static str {
        match self {
            Generator::Gear(_) => "Involute gear",
            Generator::Sprocket(_) => "Sprocket",
            Generator::Shaft(_) => "Shaft",
        }
    }

    /// Its icon in the set.
    pub fn icon(&self) -> &'static str {
        match self {
            Generator::Gear(_) => "involute-gear",
            Generator::Sprocket(_) => "sprocket",
            Generator::Shaft(_) => "revolution",
        }
    }

    /// The name a new one takes in the tree.
    pub fn base_name(&self) -> &'static str {
        match self {
            Generator::Gear(_) => "Gear",
            Generator::Sprocket(_) => "Sprocket",
            Generator::Shaft(_) => "Shaft",
        }
    }

    /// The curves its numbers make, or why they make none.
    pub fn outline(&self) -> Result<Outline, String> {
        match self {
            Generator::Gear(spec) => spec.outline(),
            Generator::Sprocket(spec) => spec.outline(),
            Generator::Shaft(spec) => spec.outline(),
        }
    }

    /// Its numbers as parameters formulas can set, with where each is in
    /// the sketch feature's JSON.
    pub fn parameters(&self) -> Vec<Parameter> {
        let at = |variant: &str, field: &str| format!("/generator/{variant}/{field}");
        let length = |name: &str, label: &str, variant: &str, field: &str| {
            Parameter::new(name, label, Dim::LENGTH, at(variant, field))
        };
        let number = |name: &str, label: &str, variant: &str, field: &str| {
            Parameter::new(name, label, Dim::NUMBER, at(variant, field))
        };
        match self {
            Generator::Gear(_) => vec![
                length("module", "Module", "Gear", "module"),
                Parameter::count("teeth", "Teeth", at("Gear", "teeth")),
                Parameter::new(
                    "pressure_angle",
                    "Pressure angle",
                    Dim::ANGLE,
                    at("Gear", "pressure_angle_deg"),
                ),
                number("profile_shift", "Profile shift", "Gear", "profile_shift"),
                number("clearance", "Clearance", "Gear", "clearance"),
                length("backlash", "Backlash", "Gear", "backlash"),
                number("root_fillet", "Root fillet", "Gear", "root_fillet"),
                length("bore", "Bore", "Gear", "bore"),
            ],
            Generator::Sprocket(_) => vec![
                length("pitch", "Chain pitch", "Sprocket", "pitch"),
                length("roller", "Roller diameter", "Sprocket", "roller"),
                Parameter::count("teeth", "Teeth", at("Sprocket", "teeth")),
                length("bore", "Bore", "Sprocket", "bore"),
            ],
            Generator::Shaft(spec) => {
                let mut out = vec![length(
                    "start_chamfer",
                    "Start chamfer",
                    "Shaft",
                    "start_chamfer",
                )];
                for i in 0..spec.sections.len() {
                    let n = i + 1;
                    for (field, label) in [
                        ("length", "Length"),
                        ("diameter", "Diameter"),
                        ("chamfer", "Chamfer"),
                        ("fillet", "Fillet"),
                    ] {
                        out.push(Parameter::new(
                            &format!("{field}_{n}"),
                            &format!("{label} {n}"),
                            Dim::LENGTH,
                            format!("/generator/Shaft/sections/{i}/{field}"),
                        ));
                    }
                }
                out
            }
        }
    }

    /// Change the fields `fields` names (`module = 2`, `sections = {...}`),
    /// leaving the rest; a field it does not have is refused by name.
    pub fn merge(
        &mut self,
        fields: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), String> {
        let label = self.label();
        let mut value = serde_json::to_value(&*self).map_err(|e| e.to_string())?;
        let inner = value
            .as_object_mut()
            .and_then(|o| o.values_mut().next())
            .and_then(|v| v.as_object_mut())
            .ok_or("the generator has no fields")?;
        for (key, given) in fields {
            if !inner.contains_key(key) {
                let known: Vec<&String> = inner.keys().collect();
                return Err(format!(
                    "{label} has no field {key}; it has {}",
                    known
                        .iter()
                        .map(|k| k.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            inner.insert(key.clone(), given.clone());
        }
        *self = serde_json::from_value(value).map_err(|e| format!("{label}: {e}"))?;
        Ok(())
    }
}

/// Make the sketch's curves again from its generator. Everything else it
/// held goes: a generated sketch is its numbers. When they make no
/// profile the sketch is left empty, so what is built from it says so,
/// and the reason comes back.
pub fn regenerate(feature: &mut SketchFeature) -> Result<(), String> {
    let Some(generator) = &feature.generator else {
        return Ok(());
    };
    let outline = generator.outline();
    let sketch = &mut feature.sketch;
    sketch.geometry.clear();
    sketch.constraints.clear();
    sketch.construction.clear();
    sketch.external.clear();
    let outline = outline?;
    outline.emit(sketch);
    // Nothing is left for the solver to move: the numbers fix every curve.
    sketch.is_fully_constrained = true;
    Ok(())
}

/// The generated sketch's numbers as formulas see them.
pub fn parameters(feature: &SketchFeature) -> Vec<Parameter> {
    feature
        .generator
        .as_ref()
        .map(Generator::parameters)
        .unwrap_or_default()
}

impl crate::SketchWorkbench {
    /// Whether the sketch open for editing is a generated one. Read off
    /// the JSON rather than the whole sketch: the toolbar asks every frame.
    pub(crate) fn editing_generated(&self, ctx: &core_document::WorkbenchRuntimeContext) -> bool {
        self.active_sketch_id
            .and_then(|id| ctx.document.get_feature_data(id))
            .and_then(|data| data.get("generator"))
            .is_some_and(|g| !g.is_null())
    }
}

/// Whether the node is a generated sketch, and which.
pub fn generator_of(node: &FeatureNode) -> Option<Generator> {
    SketchFeature::from_json(&node.data).ok()?.generator
}

/// A new sketch named `name` on `plane`, made by `generator`.
pub fn new_sketch(
    generator: Generator,
    plane: crate::sketch::SketchPlane,
    name: &str,
) -> Result<SketchFeature, String> {
    let mut sketch = Sketch::new(name);
    sketch.plane = plane;
    let mut feature = SketchFeature::new(sketch, plane);
    feature.generator = Some(generator);
    regenerate(&mut feature)?;
    Ok(feature)
}

/// Register the command that changes a generated sketch's numbers.
pub fn register(context: &mut core_document::WorkbenchContext) {
    use core_document::{CommandSpec, ParamKind};
    context.register_command(
        CommandSpec::new(
            "sketch.generator",
            "Change the numbers a generated sketch (a gear, a sprocket, a shaft) is made from, \
             or detach it into a plain sketch",
        )
        .param("sketch", ParamKind::Id, "The generated sketch")
        .optional(
            "detach",
            ParamKind::Bool,
            "Keep the curves as they are and forget the numbers: a plain sketch to edit by hand",
        )
        .extra_args(
            "The numbers to change, such as teeth = 24 or module = 1.5; a shaft takes \
             sections = {{length = 20, diameter = 10, chamfer = 0.5, fillet = 0}, ...}",
        )
        .returns("what the numbers come to: its diameters, or its length"),
    );
}

/// Run `sketch.generator`.
pub fn command(
    args: &core_document::CommandArgs,
    ctx: &mut core_document::WorkbenchRuntimeContext,
) -> core_document::CommandResult {
    use core_document::{Args, CommandError, FeatureId};
    let a = Args(args);
    let id = FeatureId(a.id("sketch")?);
    let data = ctx
        .document
        .get_feature_data(id)
        .ok_or_else(|| CommandError::bad("sketch", "is not a feature of this document"))?;
    let mut feature = SketchFeature::from_json(data)
        .map_err(|_| CommandError::bad("sketch", "is not a sketch"))?;
    let Some(generator) = feature.generator.as_mut() else {
        return Err(CommandError::bad("sketch", "is not a generated sketch"));
    };
    let fields: serde_json::Map<String, serde_json::Value> = args
        .iter()
        .filter(|(k, _)| !matches!(k.as_str(), "sketch" | "detach"))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    generator.merge(&fields).map_err(CommandError::failed)?;
    let answer = summary(generator);
    if a.opt_bool("detach")?.unwrap_or(false) {
        feature.generator = None;
    } else {
        regenerate(&mut feature).map_err(CommandError::failed)?;
    }
    ctx.document
        .update_feature_data(id, feature.to_json())
        .map_err(|e| CommandError::failed(e.to_string()))?;
    ctx.document.mark_feature_dirty(id);
    Ok(answer)
}

/// What a generator's numbers come to, for the panel and the command's
/// answer: a gear's and a sprocket's circles, a shaft's length.
pub fn summary(generator: &Generator) -> serde_json::Value {
    use serde_json::json;
    match generator {
        Generator::Gear(spec) => match spec.geometry() {
            Ok(g) => json!({
                "pitch_diameter": 2.0 * g.pitch_radius,
                "base_diameter": 2.0 * g.base_radius,
                "tip_diameter": 2.0 * g.tip_radius,
                "root_diameter": 2.0 * g.root_radius,
            }),
            Err(why) => json!({ "error": why }),
        },
        Generator::Sprocket(spec) => match spec.geometry() {
            Ok(g) => json!({
                "pitch_diameter": g.pitch_diameter,
                "tip_diameter": g.tip_diameter,
                "root_diameter": g.root_diameter,
            }),
            Err(why) => json!({ "error": why }),
        },
        Generator::Shaft(spec) => json!({
            "length": spec.sections.iter().map(|s| f64::from(s.length)).sum::<f64>(),
        }),
    }
}

/// A count as JSON holds it: a formula may leave `12.0`, a hand edit
/// `12`, a stray negative is none at all.
pub(crate) fn count<'de, D: Deserializer<'de>>(de: D) -> Result<u32, D::Error> {
    let value = f64::deserialize(de)?;
    Ok(if value.is_finite() && value > 0.0 {
        value.round().min(f64::from(u32::MAX)) as u32
    } else {
        0
    })
}

/// A 2D point, in millimetres, double precision while it is worked out.
pub type P2 = [f64; 2];

pub(crate) fn polar(r: f64, angle: f64) -> P2 {
    [r * angle.cos(), r * angle.sin()]
}

pub(crate) fn rotate(p: P2, angle: f64) -> P2 {
    let (s, c) = angle.sin_cos();
    [p[0] * c - p[1] * s, p[0] * s + p[1] * c]
}

pub(crate) fn sub(a: P2, b: P2) -> P2 {
    [a[0] - b[0], a[1] - b[1]]
}

pub(crate) fn add(a: P2, b: P2) -> P2 {
    [a[0] + b[0], a[1] + b[1]]
}

pub(crate) fn scale(a: P2, k: f64) -> P2 {
    [a[0] * k, a[1] * k]
}

pub(crate) fn norm(a: P2) -> f64 {
    a[0].hypot(a[1])
}

pub(crate) fn cross(a: P2, b: P2) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

/// One piece of a loop, from its joint to the next one.
#[derive(Debug, Clone, PartialEq)]
pub enum Edge {
    Line,
    /// A circular arc about `center`, the short way round.
    Arc {
        center: P2,
    },
    /// A clamped cubic B-spline; these are its control points between the
    /// two joints, which are its first and last.
    Spline {
        inner: Vec<P2>,
    },
}

/// A closed loop: `joints[i]` to `joints[i + 1]` (wrapping) along
/// `edges[i]`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Loop {
    pub joints: Vec<P2>,
    pub edges: Vec<Edge>,
}

impl Loop {
    /// Continue the loop from its last joint along `edge` to `to`; the
    /// loop closes from its last joint back to its first with
    /// [`Loop::close`].
    pub fn push(&mut self, edge: Edge, to: P2) {
        self.edges.push(edge);
        self.joints.push(to);
    }

    pub fn start(at: P2) -> Self {
        Self {
            joints: vec![at],
            edges: Vec::new(),
        }
    }

    /// Close the loop along `edge`: its last point is where it began, so
    /// that point goes and the edge returns to the first.
    pub fn close(&mut self, edge: Edge) {
        self.joints.pop();
        self.edges.push(edge);
    }

    /// The loop as a closed polyline, every curve sampled `per_curve`
    /// times: for measuring what it encloses.
    pub fn polyline(&self, per_curve: usize) -> Vec<P2> {
        let n = self.joints.len();
        let mut out = Vec::new();
        for (i, edge) in self.edges.iter().enumerate() {
            let a = self.joints[i];
            let b = self.joints[(i + 1) % n];
            match edge {
                Edge::Line => out.push(a),
                Edge::Arc { center } => {
                    let (a0, sweep) = arc_span(*center, a, b);
                    let r = norm(sub(a, *center));
                    for k in 0..per_curve {
                        let t = a0 + sweep * k as f64 / per_curve as f64;
                        out.push(add(*center, polar(r, t)));
                    }
                }
                Edge::Spline { inner } => {
                    let mut poles = vec![a];
                    poles.extend(inner.iter().copied());
                    poles.push(b);
                    for k in 0..per_curve {
                        out.push(fit::evaluate(&poles, k as f64 / per_curve as f64));
                    }
                }
            }
        }
        out
    }
}

/// The start angle and signed sweep of the short arc about `center` from
/// `a` to `b`.
pub(crate) fn arc_span(center: P2, a: P2, b: P2) -> (f64, f64) {
    let (va, vb) = (sub(a, center), sub(b, center));
    let a0 = va[1].atan2(va[0]);
    let sweep = cross(va, vb).atan2(va[0] * vb[0] + va[1] * vb[1]);
    (a0, sweep)
}

/// The area a closed polyline encloses (positive counter-clockwise).
pub fn shoelace(points: &[P2]) -> f64 {
    let n = points.len();
    (0..n)
        .map(|i| cross(points[i], points[(i + 1) % n]))
        .sum::<f64>()
        / 2.0
}

/// What a generator makes: closed loops, whole circles (a bore), and
/// construction circles drawn as guides (a pitch circle).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Outline {
    pub loops: Vec<Loop>,
    pub circles: Vec<(P2, f64)>,
    pub guides: Vec<(P2, f64)>,
}

/// Ids for generated curves, the same each time from the same sketch:
/// the n-th element's id follows from the sketch's.
struct Ids {
    base: u128,
    next: u128,
}

impl Ids {
    fn new(sketch: &Sketch) -> Self {
        Self {
            base: sketch.id.as_u128(),
            next: 0,
        }
    }

    fn take(&mut self) -> Uuid {
        self.next += 1;
        // An odd multiplier walks every value before it repeats.
        Uuid::from_u128(
            self.base.wrapping_add(
                self.next
                    .wrapping_mul(0x9E37_79B9_7F4A_7C15_F39C_C060_5CED_C835),
            ),
        )
    }
}

fn v(p: P2) -> Vec2D {
    Vec2D::new(p[0] as f32, p[1] as f32)
}

impl Outline {
    /// Add the outline to `sketch` as points and curves, the joints of a
    /// loop shared by the curves that meet there, so the loop is closed
    /// the way the profile builder reads one.
    pub fn emit(&self, sketch: &mut Sketch) {
        let mut ids = Ids::new(sketch);
        let point = |sketch: &mut Sketch, ids: &mut Ids, p: P2| {
            let id = ids.take();
            sketch.add_geometry(GeometryElement::Point(Point { id, position: v(p) }));
            id
        };
        for lp in &self.loops {
            let joints: Vec<Uuid> = lp
                .joints
                .iter()
                .map(|p| point(sketch, &mut ids, *p))
                .collect();
            let n = joints.len();
            for (i, edge) in lp.edges.iter().enumerate() {
                let (a, b) = (joints[i], joints[(i + 1) % n]);
                let (pa, pb) = (lp.joints[i], lp.joints[(i + 1) % n]);
                let element = match edge {
                    Edge::Line => {
                        let mut line = Line::new(a, b);
                        line.id = ids.take();
                        GeometryElement::Line(line)
                    }
                    Edge::Arc { center } => {
                        let c = point(sketch, &mut ids, *center);
                        // A sketch arc runs counter-clockwise from start
                        // to end.
                        let (_, sweep) = arc_span(*center, pa, pb);
                        let (s, e) = if sweep >= 0.0 { (a, b) } else { (b, a) };
                        let radius = norm(sub(pa, *center)) as f32;
                        let mut arc = Arc::new(c, s, e, radius);
                        arc.id = ids.take();
                        GeometryElement::Arc(arc)
                    }
                    Edge::Spline { inner } => {
                        let mut poles = vec![a];
                        for p in inner {
                            poles.push(point(sketch, &mut ids, *p));
                        }
                        poles.push(b);
                        let mut spline = BSpline::new(poles, false);
                        spline.id = ids.take();
                        GeometryElement::BSpline(spline)
                    }
                };
                sketch.add_geometry(element);
            }
        }
        for (circles, guide) in [(&self.circles, false), (&self.guides, true)] {
            for (center, radius) in circles {
                let c = point(sketch, &mut ids, *center);
                let mut circle = Circle::new(c, *radius as f32);
                circle.id = ids.take();
                let id = sketch.add_geometry(GeometryElement::Circle(circle));
                if guide {
                    sketch.set_construction(id, true);
                }
            }
        }
    }

    /// The area the outline encloses: its loops, less its circles (the
    /// bores inside them).
    pub fn area(&self) -> f64 {
        let loops: f64 = self
            .loops
            .iter()
            .map(|l| shoelace(&l.polyline(64)).abs())
            .sum();
        let holes: f64 = self
            .circles
            .iter()
            .map(|(_, r)| std::f64::consts::PI * r * r)
            .sum();
        loops - holes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_regenerated_sketch_is_the_same_sketch() {
        let mut feature = SketchFeature::from_sketch(Sketch::new("g"));
        feature.generator = Generator::named("gear");
        regenerate(&mut feature).unwrap();
        let first = serde_json::to_value(&feature.sketch).unwrap();
        regenerate(&mut feature).unwrap();
        assert_eq!(serde_json::to_value(&feature.sketch).unwrap(), first);
    }

    #[test]
    fn every_generator_makes_one_closed_profile() {
        for kind in ["gear", "sprocket", "shaft"] {
            let mut feature = SketchFeature::from_sketch(Sketch::new(kind));
            feature.generator = Generator::named(kind);
            regenerate(&mut feature).unwrap();
            let wires = crate::profile::extract_wires(&feature.sketch).unwrap();
            let expect = if kind == "shaft" { 1 } else { 2 };
            assert_eq!(wires.len(), expect, "{kind}: its outline and bore");
        }
    }

    #[test]
    fn numbers_it_cannot_make_leave_the_sketch_empty_and_say_why() {
        let mut feature = SketchFeature::from_sketch(Sketch::new("g"));
        feature.generator = Some(Generator::Gear(GearSpec {
            teeth: 2,
            ..GearSpec::default()
        }));
        let why = regenerate(&mut feature).unwrap_err();
        assert!(why.contains("teeth"), "{why}");
        assert!(feature.sketch.geometry.is_empty());
    }

    #[test]
    fn a_field_merges_by_name_and_an_unknown_one_is_refused() {
        let mut generator = Generator::named("gear").unwrap();
        let mut fields = serde_json::Map::new();
        fields.insert("teeth".into(), serde_json::json!(31));
        generator.merge(&fields).unwrap();
        let Generator::Gear(spec) = &generator else {
            unreachable!()
        };
        assert_eq!(spec.teeth, 31);
        fields.insert("colour".into(), serde_json::json!(1));
        assert!(generator.merge(&fields).unwrap_err().contains("colour"));
    }

    #[test]
    fn every_parameter_points_at_a_number() {
        for kind in ["gear", "sprocket", "shaft"] {
            let mut feature = SketchFeature::from_sketch(Sketch::new(kind));
            feature.generator = Generator::named(kind);
            let json = feature.to_json();
            for p in parameters(&feature) {
                assert!(
                    json.pointer(&p.pointer).is_some_and(|v| v.is_number()),
                    "{kind}: {}",
                    p.pointer
                );
            }
        }
    }
}
