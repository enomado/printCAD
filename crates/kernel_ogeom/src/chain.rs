//! `execute_solid_chain`: threads one in-memory `(Model, Shape)` through a
//! body's `SolidOp` list. Native-format blobs appear only at the boundaries:
//! the final result out, and `SolidOp::Boolean`'s external tool in. Tool
//! snapshots for patterns are in-model `Shape`s — no per-op serialization.

use kernel_api::{
    BoolKind, BooleanOp, ChainError, ChainProbe, FeaturePreview, ProbeAnswer, SolidBuildResult,
    SolidOp, TessellationSettings,
};
use kernel_api::{Profile, TopoName, naming as names};
use ogeom::math::{Point, Vector};
use ogeom::topo::{Model, Shape};

use crate::naming::{self, NameMap};
use crate::ops::{self, pattern};
use crate::{progress, tess};

struct ToolSnapshot {
    op: SolidOp,
    subtractive: bool,
    /// The tool itself, for an op a pattern cannot re-run elsewhere: one
    /// that sweeps a face of the solid it was built on.
    solid: Option<Shape>,
}

impl ToolSnapshot {
    /// The tool a pattern can repeat: one that adds or cuts. A tool that
    /// keeps only what it shares with the body has none, since each copy
    /// would keep less of what the one before left.
    fn of(op: &SolidOp, boolean: BooleanOp, solid: Option<Shape>) -> Option<Self> {
        (boolean != BooleanOp::Common).then(|| ToolSnapshot {
            op: op.clone(),
            subtractive: boolean == BooleanOp::Cut,
            solid,
        })
    }
}

pub fn execute(
    ops_list: &[SolidOp],
    detail: &TessellationSettings,
) -> Result<SolidBuildResult, ChainError> {
    execute_previewing(ops_list, detail, None)
}

/// [`execute`], and with `preview` (the ops of the feature being edited)
/// what that feature does: its tool solid and the body without it, in
/// [`SolidBuildResult::preview`]. A feature with no tool of its own (a
/// dress-up, a pattern) has no preview.
pub fn execute_previewing(
    ops_list: &[SolidOp],
    detail: &TessellationSettings,
    preview: Option<std::ops::Range<usize>>,
) -> Result<SolidBuildResult, ChainError> {
    execute_probing(ops_list, detail, preview, &[])
}

/// [`execute_previewing`], and the answers to `probes`, each asked of the
/// solid as it stands after its `after_op` ops, in
/// [`SolidBuildResult::probes`]. A probe asked before any op, or past the
/// chain's end, is answered with an error.
pub fn execute_probing(
    ops_list: &[SolidOp],
    detail: &TessellationSettings,
    preview: Option<std::ops::Range<usize>>,
    probes: &[ChainProbe],
) -> Result<SolidBuildResult, ChainError> {
    execute_named(ops_list, &[], detail, preview, probes)
}

/// The name each op's faces are named under: `tags[index]` (the feature
/// the op builds, [`kernel_api::naming::name_of_id`] of its id), the second
/// and later ops of one feature told apart by their count; an op without
/// a tag is named by where it stands in the chain.
fn op_tags(ops_list: &[SolidOp], tags: &[TopoName]) -> Vec<TopoName> {
    let mut seen: Vec<(TopoName, u32)> = Vec::new();
    (0..ops_list.len())
        .map(|index| {
            let tag = tags
                .get(index)
                .copied()
                .filter(|t| *t != 0)
                .unwrap_or_else(|| names::name_of(format!("op {index}").as_bytes()));
            let count = match seen.iter_mut().find(|(t, _)| *t == tag) {
                Some((_, n)) => {
                    *n += 1;
                    *n
                }
                None => {
                    seen.push((tag, 0));
                    0
                }
            };
            if count == 0 {
                tag
            } else {
                names::child(tag, &count.to_le_bytes())
            }
        })
        .collect()
}

/// The names of a sweep's tool: its walls after the profile's segments
/// (every sweep's wall passes through the segment it sweeps) and its ends
/// after where they stand along the profile's normal; the rest afresh.
fn sweep_names(model: &Model, tool: &Shape, tag: TopoName, profile: &Profile) -> NameMap {
    let plane = &profile.plane;
    let origin = Point::new(plane.origin[0], plane.origin[1], plane.origin[2]);
    let normal = Vector::new(plane.normal[0], plane.normal[1], plane.normal[2]);
    naming::tool_names(
        model,
        tool,
        tag,
        &naming::profile_segments(profile),
        origin,
        normal,
    )
}

/// A tool's faces named afresh, by their surfaces and where they face.
fn tool_names_fresh(model: &Model, tool: &Shape, tag: TopoName) -> NameMap {
    NameMap::assign(model, tool, tag, |_| Vec::new())
}

/// [`execute_probing`], naming the faces of the solid as it builds under
/// `tags`, one per op (see [`op_tags`]): the result's mesh carries each
/// face's name and each edge's faces' names, and references with names
/// find their faces by them.
pub fn execute_named(
    ops_list: &[SolidOp],
    tags: &[TopoName],
    detail: &TessellationSettings,
    preview: Option<std::ops::Range<usize>>,
    probes: &[ChainProbe],
) -> Result<SolidBuildResult, ChainError> {
    let chain_err = |op_index: usize, message: String| ChainError { op_index, message };

    if ops_list.is_empty() {
        return Err(chain_err(0, "solid-op chain is empty".into()));
    }
    match ops_list[0].boolean_op() {
        Some(BooleanOp::NewSolid) => {}
        Some(_) => {
            return Err(chain_err(
                0,
                "first solid op in a chain must be NewSolid".into(),
            ));
        }
        None => {
            return Err(chain_err(
                0,
                "first solid op in a chain must produce a shape".into(),
            ));
        }
    }
    for (index, op) in ops_list.iter().enumerate().skip(1) {
        if op.boolean_op() == Some(BooleanOp::NewSolid) {
            return Err(chain_err(
                index,
                "only the first op in a chain may be NewSolid".into(),
            ));
        }
    }

    let mut model = Model::with_tolerances(tess::tolerances());
    let mut current: Option<Shape> = None;
    let op_tags = op_tags(ops_list, tags);
    let mut names = NameMap::default();
    let mut tools: Vec<Option<ToolSnapshot>> = Vec::with_capacity(ops_list.len());
    // The previewed feature: the body before it, after it, and its tools.
    let previewing = |index: usize| preview.as_ref().is_some_and(|r| r.contains(&index));
    let mut before: Option<Shape> = None;
    let mut after: Option<Shape> = None;
    let mut preview_tools: Vec<Shape> = Vec::new();
    let mut preview_cuts = false;
    let mut answers: Vec<Result<ProbeAnswer, String>> = probes
        .iter()
        .map(|p| {
            Err(if p.after_op == 0 {
                "there is no solid before it".to_string()
            } else {
                "it stands past the end of the history".to_string()
            })
        })
        .collect();

    for (index, solid_op) in ops_list.iter().enumerate() {
        let tag = op_tags[index];
        // What the op and the probes asked before it look up by name is
        // found in the solid as it stands.
        let named = naming::set_current(std::mem::take(&mut names));
        if index > 0
            && let Some(shape) = current.as_ref()
        {
            ask(&mut model, shape, index, probes, &mut answers);
        }
        let mut tool_names: Option<NameMap> = None;
        progress::context(format_args!(
            "{} {}/{}",
            progress::op_label(solid_op),
            index + 1,
            ops_list.len()
        ));
        let err = |message: String| ChainError {
            op_index: index,
            message,
        };
        progress::checkpoint().map_err(&err)?;
        let base = current.clone();
        if preview.as_ref().is_some_and(|r| r.start == index) {
            before = base.clone();
        }
        let mut tool_snapshot: Option<ToolSnapshot> = None;
        // Keep a feature's tool when it is the one previewed.
        let mut keep = |tool: &Shape, op: BooleanOp| {
            if previewing(index) {
                preview_tools.push(tool.clone());
                preview_cuts |= matches!(op, BooleanOp::Cut | BooleanOp::Common);
            }
        };

        let next = match solid_op {
            SolidOp::Shape { brep } => absorb_shape(&mut model, brep).map_err(&err)?,
            SolidOp::Sweep { profile, kind, op } => {
                let tool = ops::sweep::build_tool(&mut model, base.as_ref(), profile, kind)
                    .map_err(&err)?;
                tool_names = Some(sweep_names(&model, &tool, tag, profile));
                tool_snapshot = ToolSnapshot::of(solid_op, *op, None);
                keep(&tool, *op);

                combine(&mut model, base.as_ref(), tool, *op).map_err(&err)?
            }
            SolidOp::SweepFace { face, kind, op } => {
                let tool = ops::sweep::build_face_tool(&mut model, base.as_ref(), face, kind)
                    .map_err(&err)?;
                tool_names = Some(tool_names_fresh(&model, &tool, tag));
                tool_snapshot = ToolSnapshot::of(solid_op, *op, Some(tool.clone()));
                keep(&tool, *op);

                combine(&mut model, base.as_ref(), tool, *op).map_err(&err)?
            }
            SolidOp::Primitive {
                kind,
                placement,
                op,
            } => {
                let tool = ops::primitive::build_tool(&mut model, kind, placement).map_err(&err)?;
                tool_names = Some(tool_names_fresh(&model, &tool, tag));
                tool_snapshot = ToolSnapshot::of(solid_op, *op, None);
                keep(&tool, *op);

                combine(&mut model, base.as_ref(), tool, *op).map_err(&err)?
            }
            SolidOp::LoftThrough {
                sections,
                ruled,
                closed,
                op,
            } => {
                let tool = ops::loft_pipe::loft_through_tool(
                    &mut model,
                    base.as_ref(),
                    sections,
                    *ruled,
                    *closed,
                )
                .map_err(&err)?;
                tool_names = Some(tool_names_fresh(&model, &tool, tag));
                tool_snapshot = ToolSnapshot::of(solid_op, *op, Some(tool.clone()));
                keep(&tool, *op);

                combine(&mut model, base.as_ref(), tool, *op).map_err(&err)?
            }
            SolidOp::Loft {
                sections,
                ruled,
                closed,
                op,
            } => {
                let tool = ops::loft_pipe::loft_tool(&mut model, sections, *ruled, *closed)
                    .map_err(&err)?;
                tool_names = Some(match sections.first() {
                    Some(first) if !*closed => {
                        let plane = &first.plane;
                        naming::tool_names(
                            &model,
                            &tool,
                            tag,
                            &naming::profile_segments(first),
                            Point::new(plane.origin[0], plane.origin[1], plane.origin[2]),
                            Vector::new(plane.normal[0], plane.normal[1], plane.normal[2]),
                        )
                    }
                    _ => tool_names_fresh(&model, &tool, tag),
                });
                tool_snapshot = ToolSnapshot::of(solid_op, *op, None);
                keep(&tool, *op);

                combine(&mut model, base.as_ref(), tool, *op).map_err(&err)?
            }
            SolidOp::Pipe {
                profile,
                spine,
                frame,
                corner,
                sections,
                op,
            } => {
                let tool =
                    ops::loft_pipe::pipe_tool(&mut model, profile, spine, frame, *corner, sections)
                        .map_err(&err)?;
                tool_names = Some(tool_names_fresh(&model, &tool, tag));
                tool_snapshot = ToolSnapshot::of(solid_op, *op, None);
                keep(&tool, *op);

                combine(&mut model, base.as_ref(), tool, *op).map_err(&err)?
            }
            SolidOp::PipeThrough {
                profile,
                path,
                frame,
                corner,
                sections,
                op,
            } => {
                let tool = ops::loft_pipe::pipe_through_tool(
                    &mut model,
                    base.as_ref(),
                    profile,
                    path,
                    frame,
                    *corner,
                    sections,
                )
                .map_err(&err)?;
                tool_names = Some(tool_names_fresh(&model, &tool, tag));
                tool_snapshot = ToolSnapshot::of(solid_op, *op, Some(tool.clone()));
                keep(&tool, *op);

                combine(&mut model, base.as_ref(), tool, *op).map_err(&err)?
            }
            SolidOp::Fillet {
                radius,
                edges,
                follow_tangent,
            } => {
                let solid = base.ok_or_else(|| err("fillet needs an existing solid".into()))?;
                ops::dressup::fillet(&mut model, &solid, *radius, edges, *follow_tangent)
                    .map_err(&err)?
            }
            SolidOp::Chamfer {
                spec,
                flip,
                edges,
                follow_tangent,
            } => {
                let solid = base.ok_or_else(|| err("chamfer needs an existing solid".into()))?;
                ops::dressup::chamfer(&mut model, &solid, spec, *flip, edges, *follow_tangent)
                    .map_err(&err)?
            }
            SolidOp::Draft {
                angle_deg,
                neutral_point,
                neutral_normal,
                pull_dir,
                faces,
                face_names,
            } => {
                let solid = base.ok_or_else(|| err("draft needs an existing solid".into()))?;
                ops::dressup::draft(
                    &mut model,
                    &solid,
                    *angle_deg,
                    *neutral_point,
                    *neutral_normal,
                    *pull_dir,
                    &faces
                        .iter()
                        .enumerate()
                        .map(|(i, p)| (*p, face_names.get(i).copied().unwrap_or(0)))
                        .collect::<Vec<_>>(),
                )
                .map_err(&err)?
            }
            SolidOp::Refine => {
                let solid = base.ok_or_else(|| err("refine needs an existing solid".into()))?;
                ogeom::heal::unify_same_domain(&mut model, &solid, ops::tol())
                    .map(|(built, _)| {
                        naming::record(&built.history);
                        built.shape
                    })
                    .map_err(|e| err(format!("refine failed: {e}")))?
            }
            SolidOp::Thickness {
                value,
                open_faces,
                open_face_names,
                inward,
                join,
                both_sides,
            } => {
                let solid = base.ok_or_else(|| err("thickness needs an existing solid".into()))?;
                ops::dressup::thickness(
                    &mut model,
                    &solid,
                    *value,
                    open_faces,
                    open_face_names,
                    if *both_sides { None } else { Some(*inward) },
                    *join,
                )
                .map_err(&err)?
            }
            SolidOp::Transform {
                transforms,
                originals,
            } => {
                let solid = base.ok_or_else(|| err("pattern needs an existing solid".into()))?;
                let instances: Vec<pattern::ToolInstance> = if originals.is_empty() {
                    vec![pattern::ToolInstance {
                        tool: pattern::PatternTool::Solid(solid.clone()),
                        subtractive: false,
                    }]
                } else {
                    originals
                        .iter()
                        .map(|&orig| {
                            tools
                                .get(orig)
                                .and_then(|t| t.as_ref())
                                .map(|t| pattern::ToolInstance {
                                    tool: match &t.solid {
                                        Some(solid) => pattern::PatternTool::Solid(solid.clone()),
                                        None => pattern::PatternTool::Op(Box::new(t.op.clone())),
                                    },
                                    subtractive: t.subtractive,
                                })
                                .ok_or_else(|| {
                                    err("pattern references an op with no reusable tool solid"
                                        .into())
                                })
                        })
                        .collect::<Result<_, _>>()?
                };
                pattern::apply(&mut model, solid, &instances, transforms).map_err(&err)?
            }
            SolidOp::Boolean {
                tool_brep,
                kind,
                tool_transform,
            } => {
                let solid = base.ok_or_else(|| err("boolean needs an existing solid".into()))?;
                let tool =
                    external_tool(&mut model, tool_brep, tool_transform.as_ref()).map_err(&err)?;
                let cuts = *kind == BoolKind::Cut;
                keep(
                    &tool,
                    if cuts {
                        BooleanOp::Cut
                    } else {
                        BooleanOp::Fuse
                    },
                );
                ops::combine_solids(&mut model, &solid, &tool, *kind).map_err(&err)?
            }
        };

        // The result's faces take the names of the faces they came from.
        let (before_names, histories) = named.take_with_histories();
        names = match solid_op {
            SolidOp::Shape { .. } => tool_names_fresh(&model, &next, tag),
            _ => {
                let mut sources: Vec<&NameMap> = vec![&before_names];
                if let Some(tool) = &tool_names {
                    sources.push(tool);
                }
                NameMap::carry(&model, &next, &sources, tag, &histories)
            }
        };
        current = Some(next);
        if preview.as_ref().is_some_and(|r| r.end == index + 1) {
            after = current.clone();
        }
        tools.push(tool_snapshot);
    }

    let final_shape = current.expect("chain validated non-empty");
    let named = naming::set_current(names);
    ask(
        &mut model,
        &final_shape,
        ops_list.len(),
        probes,
        &mut answers,
    );
    let names = named.take();
    let mesh = tess::mesh_named(&model, &final_shape, detail, &names).map_err(|e| {
        chain_err(
            ops_list.len() - 1,
            format!("meshing the result failed: {e}"),
        )
    })?;
    if mesh.positions.is_empty() || mesh.indices.is_empty() {
        return Err(chain_err(
            ops_list.len() - 1,
            "solid-op chain produced an empty render mesh".into(),
        ));
    }
    let brep_blob = tess::write_blob(&model, &final_shape).map_err(|e| {
        chain_err(
            ops_list.len() - 1,
            format!("serializing the result failed: {e}"),
        )
    })?;

    let bounds_mm = tess::solid_bounds(&model, &final_shape, &mesh);
    // A preview that cannot be made leaves the build as it is.
    let preview = (!preview_tools.is_empty())
        .then(|| {
            let shown = if preview_cuts { after } else { before };
            feature_preview(&mut model, preview_tools, shown, preview_cuts, detail)
        })
        .flatten()
        .map(Box::new);
    Ok(SolidBuildResult {
        brep_blob,
        mesh,
        bounds_mm,
        preview,
        probes: answers,
    })
}

/// Answer the probes asked of the solid after `after_op` ops.
fn ask(
    model: &mut Model,
    shape: &Shape,
    after_op: usize,
    probes: &[ChainProbe],
    answers: &mut [Result<ProbeAnswer, String>],
) {
    for (probe, answer) in probes.iter().zip(answers.iter_mut()) {
        if probe.after_op == after_op {
            *answer = crate::probe::answer(model, shape, &probe.probe);
        }
    }
}

/// The preview of a feature: its tools meshed as one, and the body to show
/// beside them.
fn feature_preview(
    model: &mut Model,
    tools: Vec<Shape>,
    shown: Option<Shape>,
    cuts: bool,
    detail: &TessellationSettings,
) -> Option<FeaturePreview> {
    let tool = match tools.as_slice() {
        [one] => one.clone(),
        many => model.add_compound(many).ok()?,
    };
    let tool = tess::mesh_shape(model, &tool, &[], detail).ok()?;
    let shown = match shown {
        Some(shape) => {
            let mesh = tess::mesh_shape(model, &shape, &[], detail).ok()?;
            let brep_blob = tess::write_blob(model, &shape).ok()?;
            let bounds_mm = tess::solid_bounds(model, &shape, &mesh);
            Some(Box::new(SolidBuildResult {
                brep_blob,
                mesh,
                bounds_mm,
                preview: None,
                probes: Vec::new(),
            }))
        }
        None => None,
    };
    Some(FeaturePreview { shown, tool, cuts })
}

/// Combine a freshly built tool with the running solid.
fn combine(
    model: &mut Model,
    base: Option<&Shape>,
    tool: Shape,
    op: BooleanOp,
) -> Result<Shape, String> {
    match op {
        BooleanOp::NewSolid => Ok(tool),
        BooleanOp::Fuse => {
            let base =
                base.ok_or_else(|| "fuse requires existing material in the body".to_string())?;
            if !ops::bounds_overlap(model, base, &tool) {
                return ops::fuse_or_compound(model, base, &tool);
            }
            ops::combine_solids(model, base, &tool, BoolKind::Fuse)
        }
        BooleanOp::Cut => {
            let base =
                base.ok_or_else(|| "cut requires existing material in the body".to_string())?;
            ops::combine_solids(model, base, &tool, BoolKind::Cut)
        }
        BooleanOp::Common => {
            let base = base.ok_or_else(|| {
                "keeping the intersection requires existing material in the body".to_string()
            })?;
            ops::combine_solids(model, base, &tool, BoolKind::Common)
        }
    }
}

/// A native-format snapshot read into the model: the shape it holds.
pub(crate) fn absorb_shape(model: &mut Model, brep: &[u8]) -> Result<Shape, String> {
    let text =
        std::str::from_utf8(brep).map_err(|_| "solid snapshot is not valid UTF-8".to_string())?;
    let absorbed = ogeom::io::native::read_into(model, text)
        .map_err(|e| format!("importing the solid snapshot failed: {e}"))?;
    absorbed
        .shapes
        .first()
        .cloned()
        .ok_or_else(|| "solid snapshot holds no shape".to_string())
}

/// Another body's solid as a boolean's tool, where it sits relative to
/// this one.
fn external_tool(
    model: &mut Model,
    tool_brep: &[u8],
    tool_transform: Option<&[[f64; 4]; 4]>,
) -> Result<Shape, String> {
    let mut tool = absorb_shape(model, tool_brep)?;
    if let Some(matrix) = tool_transform {
        tool = pattern::moved(model, &tool, matrix)?;
    }
    Ok(tool)
}
