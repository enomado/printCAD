//! What the Surface commands take and what a caller should know of them:
//! one command per kind of step, with its fields as arguments, plus
//! `surface.check` and `surface.set`, each with notes, related commands
//! and a working example.

use core_document::{CommandSpec, ParamKind};

use crate::feature::{Kind, SurfaceFeature};
use crate::{CHECK_TOOL, CURVATURE_TOOL, SET_COMMAND, ZEBRA_TOOL};

const EDGE: &str = "{point, direction}, as pc.doc.edges lists an edge";
const CURVE: &str = "{Sketch = id}, or {Edge = {point, direction}} for an edge of the body";

/// The command that makes a kind of step.
pub fn step(kind: &Kind) -> CommandSpec {
    let feature = SurfaceFeature::for_tool(kind.tool).expect("every kind has a step");
    let mut spec = CommandSpec::new(kind.tool, kind.summary);
    spec = if feature.needs_body() {
        spec.param(
            "body",
            ParamKind::Id,
            "The surface body it works on, or a feature in it",
        )
    } else {
        spec.optional(
            "body",
            ParamKind::Id,
            "The surface body it goes in, or a feature in it; else its first sketch's body \
             when that holds only drawings and surfaces, else a new one",
        )
    };
    spec = spec.optional("name", ParamKind::String, "Its name in the tree");
    if let Some(doc) = sketches(kind.tool) {
        spec = spec.optional("sketches", ParamKind::List, doc);
    }
    for (name, param, doc) in fields(kind.tool) {
        spec = spec.optional(name, *param, doc);
    }
    explained(kind.tool, spec.returns("The new feature's id"))
}

/// `surface.check`.
pub fn check() -> CommandSpec {
    CommandSpec::new(
        CHECK_TOOL,
        "Measure how a body's faces meet at each shared edge",
    )
    .param("body", ParamKind::Id, "The body, or a feature in it")
    .returns(
        "a list of {point, gap, angle_deg, curvature, join}, one per shared edge; the view \
         labels them",
    )
    .read_only()
    .note(
        "One entry per edge two faces of the body share: `gap` in mm between them, \
         `angle_deg` the crease (0 where they meet tangent), `point` halfway along the edge. \
         Any body with a shape is checked, a Design solid too.",
    )
    .note(
        "`curvature` is the largest difference, in 1/mm, between how sharply the two faces \
         bend square to the edge (nil where it could not be read); `join` is how the view \
         labels it: G2 where they meet tangent and bend alike, G1 tangent, else the crease \
         angle, or the gap where they are apart.",
    )
    .note(
        "Sheets that only touch share no edge until sewn, so the seam between them is not \
         listed: two separate sheets give an empty list. A body not built yet is refused \
         (\"The body has no shape to check yet\").",
    )
    .see_also("surface.sew")
    .see_also("surface.fillet")
    .example(
        "A sharp corner, then rounded",
        r#"
        local s = pc.sketch.new{plane = "XY"}
        pc.sketch.polyline{sketch = s, points = {{10, 0}, {0, 0}, {0, 10}}}
        local walls = pc.surface.extrude{sketches = {s}, length = 5}
        local body = pc.doc.feature{id = walls}.body
        assert(#pc.doc.rebuild() == 0)
        local joins = pc.surface.check{body = body}
        assert(#joins == 1 and math.abs(joins[1].angle_deg - 90) < 1e-6, "one square crease")
        assert(joins[1].gap < 1e-9)
        pc.surface.fillet{body = body, radius = 2,
          edges = {{point = joins[1].point, direction = {0, 0, 1}}}}
        assert(#pc.doc.rebuild() == 0)
        joins = pc.surface.check{body = body}
        assert(#joins == 2, "the round meets each wall")
        for _, j in ipairs(joins) do
          assert(j.angle_deg < 0.01 and j.join == "G1", "tangent, the curvature jumps")
          assert(math.abs(j.curvature - 0.5) < 1e-3, "from flat to a 2 mm round")
        end
        "#,
    )
}

/// `surface.curvature`.
pub fn curvature() -> CommandSpec {
    CommandSpec::new(
        CURVATURE_TOOL,
        "Paint a body with how sharply its surfaces bend",
    )
    .param("body", ParamKind::Id, "The body, or a feature in it")
    .optional(
        "measure",
        ParamKind::String,
        "gaussian (the default), mean, max or min",
    )
    .optional(
        "limit",
        ParamKind::Number,
        "The curvature the colours reach at either end; 0 (the default) takes it from \
             the body",
    )
    .returns("{measure, low, high, unit}: the measure's range over the body")
    .read_only()
    .note(
        "Curvatures are signed against the faces' outward normals: negative where a \
             surface bulges out, positive in a hollow. `gaussian` is the product of the two \
             principal curvatures (1/mm²): positive on a dome or in a bowl, negative on a \
             saddle, zero on a plane and on what unrolls flat (a cylinder, a cone). `mean` \
             is their average, `max` and `min` each (1/mm).",
    )
    .note(
        "The view paints the body until the task closes, or until `surface.zebra` or \
             `surface.check` takes its place. A body not built yet is refused (\"The body has \
             no shape to paint yet\").",
    )
    .see_also("surface.zebra")
    .see_also("surface.check")
    .example(
        "A tube's side bends one way only",
        r#"
            local s = pc.sketch.new{plane = "XZ"}
            pc.sketch.line{sketch = s, x1 = 5, y1 = 0, x2 = 5, y2 = 10}
            local tube = pc.surface.revolve{sketches = {s}}
            assert(#pc.doc.rebuild() == 0)
            local body = pc.doc.feature{id = tube}.body
            local k = pc.surface.curvature{body = body}
            assert(math.abs(k.low) < 1e-9 and math.abs(k.high) < 1e-9, "unrolls flat")
            k = pc.surface.curvature{body = body, measure = "mean"}
            assert(math.abs(math.abs(k.low) - 0.1) < 1e-6 and math.abs(k.high - k.low) < 1e-6,
              "half of 1/5 everywhere")
            "#,
    )
}

/// `surface.zebra`.
pub fn zebra() -> CommandSpec {
    CommandSpec::new(ZEBRA_TOOL, "Paint a body with zebra stripes")
        .param("body", ParamKind::Id, "The body, or a feature in it")
        .optional(
            "stripes",
            ParamKind::Number,
            "Dark stripes per half turn of the surface (6), 1 to 64",
        )
        .optional(
            "axis",
            ParamKind::String,
            "X, Y or Z (the default): the way they run",
        )
        .returns("nothing")
        .read_only()
        .note(
            "Each stripe is a band of the way the surface faces, turned about `axis`. Across a \
             crease the stripes break, across a tangent join they bend sharply, and across a \
             curvature continuous one they run on smoothly. The view paints the body until \
             the task closes.",
        )
        .see_also("surface.curvature")
        .see_also("surface.check")
        .example(
            "Stripes on a tube",
            r#"
            local s = pc.sketch.new{plane = "XZ"}
            pc.sketch.line{sketch = s, x1 = 5, y1 = 0, x2 = 5, y2 = 10}
            local body = pc.doc.feature{id = pc.surface.revolve{sketches = {s}}}.body
            assert(#pc.doc.rebuild() == 0)
            assert(pc.surface.zebra{body = body, stripes = 8} == nil)
            local ok = pcall(pc.surface.zebra, {body = body, axis = "W"})
            assert(not ok, "X, Y or Z")
            "#,
        )
}

/// `surface.set`.
pub fn set() -> CommandSpec {
    CommandSpec::new(SET_COMMAND, "Change fields of a surface step")
        .param("feature", ParamKind::Id, "The surface step to change")
        .extra_args(
            "The fields to change, by name (`length`, `continuity`, `plane`, `sketches`…), as \
             the step's own command takes them",
        )
        .returns("nothing")
        .note(
            "It takes the fields the step's command takes, by the same names; `sketches` \
             replaces the curves it is built from. A name the step does not have is refused \
             (\"is not a field of this surface\"); the kind of step stays.",
        )
        .note(
            "The step keeps its place in the body's history: everything after it builds again \
             on the change at `pc.doc.rebuild()`.",
        )
        .example(
            "A wall made taller",
            r#"
            local s = pc.sketch.new{plane = "XY"}
            pc.sketch.line{sketch = s, x1 = 0, y1 = 0, x2 = 10, y2 = 0}
            local wall = pc.surface.extrude{sketches = {s}, length = 5}
            local body = pc.doc.feature{id = wall}.body
            assert(#pc.doc.rebuild() == 0)
            assert(math.abs(pc.doc.measure{body = body}.area - 50) < 1e-6)
            pc.surface.set{feature = wall, length = 8}
            assert(#pc.doc.rebuild() == 0)
            assert(math.abs(pc.doc.measure{body = body}.area - 80) < 1e-6)
            "#,
        )
}

/// What `sketches` is to a kind of step, for the kinds that read it.
fn sketches(tool: &str) -> Option<&'static str> {
    Some(match tool {
        "surface.extrude" | "surface.revolve" => {
            "The sketches it is built from: every chain of each, open or closed"
        }
        "surface.planar" => "The sketches whose closed loops it fills",
        "surface.fill" => "The sketches whose curves close the hole, end to end",
        "surface.ruled" => "Two sketches: the first curve, then the second",
        "surface.loft" => "The sections, a sketch each, in order: two or more",
        "surface.sweep" => "The profile's sketch, then the path's",
        "surface.split" => "The sketches whose curves cut the faces",
        _ => return None,
    })
}

/// The fields of a kind of step, as its command takes them.
fn fields(tool: &str) -> &'static [(&'static str, ParamKind, &'static str)] {
    use ParamKind::{Any, Bool, List, Number, String};
    match tool {
        "surface.extrude" => &[
            ("length", Number, "mm (10)"),
            (
                "direction",
                Any,
                "SketchNormal (the default), X, Y, Z or {Custom = {x, y, z}}",
            ),
            ("symmetric", Bool, "Half each way"),
            ("reversed", Bool, "The other way"),
            (
                "curves",
                List,
                "In place of `sketches`: each {Sketch = id}, or {Edge = {point, direction}} for an edge of the body",
            ),
        ],
        "surface.revolve" => &[
            ("angle_deg", Number, "Degrees (360)"),
            (
                "axis",
                Any,
                "SketchVertical (the default), SketchHorizontal, X, Y, Z or \
                 {Custom = {origin = {x, y, z}, direction = {x, y, z}}}",
            ),
            (
                "curves",
                List,
                "In place of `sketches`: each {Sketch = id}, or {Edge = {point, direction}} for an edge of the body",
            ),
        ],
        "surface.planar" => &[(
            "curves",
            List,
            "In place of `sketches`: each {Sketch = id}, or {Edge = {point, direction}} for an edge of the body",
        )],
        "surface.fill" => &[
            (
                "continuity",
                String,
                "G0 (the default), G1 or G2: how it meets the faces of edges in `boundary`",
            ),
            (
                "boundary",
                List,
                "In place of `sketches`: each {Sketch = id}, or {Edge = {point, direction}} for an edge of the body",
            ),
        ],
        "surface.ruled" => &[("first", Any, CURVE), ("second", Any, CURVE)],
        "surface.loft" => &[
            (
                "closed",
                Bool,
                "Run on from the last section back to the first",
            ),
            (
                "sections",
                List,
                "In place of `sketches`: each {Sketch = id}",
            ),
        ],
        "surface.sweep" => &[
            (
                "profile",
                List,
                "In place of `sketches`: the profile, each {Sketch = id}",
            ),
            (
                "path",
                List,
                "In place of `sketches`: the path, each {Sketch = id}",
            ),
        ],
        "surface.offset" => &[
            (
                "faces",
                List,
                "The faces to copy, each {point, normal} as pc.doc.faces lists a face",
            ),
            (
                "distance",
                Number,
                "mm along the faces' normals (1); negative the other way",
            ),
        ],
        "surface.extend" => &[
            (
                "edges",
                List,
                "The free edges to grow past, each {point, direction}, as pc.doc.edges lists an edge",
            ),
            ("length", Number, "mm (5)"),
            ("continuity", String, "G1 (the default), G0 or G2"),
        ],
        "surface.blend" => &[
            ("first", Any, EDGE),
            ("second", Any, EDGE),
            ("continuity", String, "G1 (the default), G0 or G2"),
        ],
        "surface.split" => &[
            (
                "faces",
                List,
                "The faces to cut, each {point, normal} as pc.doc.faces lists a face",
            ),
            ("curves", List, "In place of `sketches`: each {Sketch = id}"),
        ],
        "surface.sew" => &[(
            "gap",
            Number,
            "mm: edges this far apart are joined too (0, only edges that meet)",
        )],
        "surface.fillet" => &[
            (
                "edges",
                List,
                "The edges to round, each {point, direction}, as pc.doc.edges lists an edge",
            ),
            ("radius", Number, "mm (2)"),
        ],
        "surface.thicken" => &[
            (
                "thickness",
                Number,
                "mm along the faces' normals (2); negative the other way",
            ),
            ("both_sides", Bool, "Half each side"),
        ],
        "surface.trim" => &[
            (
                "plane",
                Any,
                "YZ (the default), XZ, XY or {Custom = {origin = {x, y, z}, normal = {x, y, z}}}, \
                 in the body's frame",
            ),
            ("offset", Number, "mm along the plane's normal"),
            ("flip", Bool, "Keep the side the normal points away from"),
        ],
        "surface.mirror" => &[
            (
                "plane",
                Any,
                "YZ (the default), XZ, XY or {Custom = {origin = {x, y, z}, normal = {x, y, z}}}, \
                 in the body's frame",
            ),
            ("offset", Number, "mm along the plane's normal"),
        ],
        _ => &[],
    }
}

/// A step's notes, related commands and example.
fn explained(tool: &str, spec: CommandSpec) -> CommandSpec {
    let spec = if SurfaceFeature::for_tool(tool).is_some_and(|f| f.needs_body()) {
        spec.note(
            "`body` is required, a body or any feature in it: the step works on the \
             surfaces the body holds before it.",
        )
    } else {
        spec.note(
            "It goes in `body` (a body, or any feature in it); without one, in its first \
             sketch's body when that holds only sketches, datums and surfaces, else in a new \
             body named Surface. A body Design builds is refused: surfaces go in a body of \
             their own, and `pc.sketch.new{body = id}` starts a sketch in one.",
        )
        .note(
            "An open sheet has an area and no volume: `pc.doc.measure` gives `volume` nil \
             until the body is sewn closed or thickened.",
        )
        .see_also("sketch.new")
    };
    match tool {
        "surface.extrude" => spec
            .note(
                "`length` is 10 mm when left out. `direction` is SketchNormal (square to the \
                 first sketch), X, Y, Z or {Custom = {x, y, z}}; \"Custom\" without its \
                 numbers is refused with the form it takes. `symmetric = true` runs half \
                 each way, `reversed = true` the other way.",
            )
            .note(
                "Built from edges (`curves`) it needs a direction of its own: SketchNormal \
                 fails at rebuild (\"the direction follows a sketch's plane\").",
            )
            .see_also("surface.check")
            .example(
                "Two walls from an open outline",
                r#"
                local s = pc.sketch.new{plane = "XY"}
                pc.sketch.polyline{sketch = s, points = {{10, 0}, {0, 0}, {0, 10}}}
                local walls = pc.surface.extrude{sketches = {s}, length = 5}
                assert(#pc.doc.rebuild() == 0)
                local body = pc.doc.feature{id = walls}.body
                assert(#pc.doc.faces{body = body} == 2, "a face per line")
                local m = pc.doc.measure{body = body}
                assert(math.abs(m.area - 100) < 1e-6 and m.volume == nil, "a sheet, open")
                assert(math.abs(m.max[3] - 5) < 1e-6)
                "#,
            ),
        "surface.revolve" => spec
            .note(
                "`angle_deg` is 360 when left out. `axis` is SketchVertical (the first \
                 sketch's vertical axis through its origin), SketchHorizontal, X, Y, Z or \
                 {Custom = {origin = {x, y, z}, direction = {x, y, z}}}.",
            )
            .note(
                "A curve lying along the axis fails at rebuild (\"the whole wire lies along \
                 the axis, so it revolves out nothing\").",
            )
            .see_also("surface.check")
            .example(
                "A cylinder's side from a line",
                r#"
                local s = pc.sketch.new{plane = "XZ"}
                pc.sketch.line{sketch = s, x1 = 5, y1 = 0, x2 = 5, y2 = 10}
                local tube = pc.surface.revolve{sketches = {s}}
                assert(#pc.doc.rebuild() == 0)
                local m = pc.doc.measure{body = pc.doc.feature{id = tube}.body}
                assert(math.abs(m.area - 2 * math.pi * 5 * 10) < 1e-3, m.area)
                assert(math.abs(m.min[1] + 5) < 1e-6 and math.abs(m.max[3] - 10) < 1e-6)
                "#,
            ),
        "surface.planar" => spec
            .note(
                "It fills closed flat loops, a loop inside another a hole. An open chain \
                 fails at rebuild (\"profile wire is not closed\").",
            )
            .see_also("surface.sew")
            .example(
                "A plate with a hole",
                r#"
                local s = pc.sketch.new{plane = "XY"}
                pc.sketch.rect{sketch = s, x = 0, y = 0, width = 20, height = 10}
                pc.sketch.circle{sketch = s, x = 10, y = 5, radius = 2}
                local plate = pc.surface.planar{sketches = {s}}
                assert(#pc.doc.rebuild() == 0)
                local body = pc.doc.feature{id = plate}.body
                assert(#pc.doc.faces{body = body} == 1)
                local area = pc.doc.measure{body = body}.area
                assert(math.abs(area - (200 - math.pi * 4)) < 1e-3, area)
                "#,
            ),
        "surface.fill" => spec
            .note(
                "The curves must meet end to end and close; ones that do not fail at rebuild \
                 (\"the boundary does not close\"). The loop may rise out of a plane, which \
                 `surface.planar` cannot fill.",
            )
            .note(
                "`continuity` (G0, G1 or G2) is how it meets the face of each edge given in \
                 `boundary`; a sketch's curve is only touched.",
            )
            .see_also("surface.planar")
            .example(
                "A disc filling a circle",
                r#"
                local s = pc.sketch.new{plane = "XY", offset = 20}
                pc.sketch.circle{sketch = s, x = 0, y = 0, radius = 5}
                local cap = pc.surface.fill{sketches = {s}}
                assert(#pc.doc.rebuild() == 0)
                local m = pc.doc.measure{body = pc.doc.feature{id = cap}.body}
                assert(math.abs(m.area - math.pi * 25) < 1e-3, m.area)
                assert(math.abs(m.min[3] - 20) < 1e-6 and math.abs(m.max[3] - 20) < 1e-6)
                "#,
            ),
        "surface.ruled" => spec
            .note(
                "Exactly two curves, the first sketch's and the second's; a third is \
                 refused. Their ends pair start to start, as each was drawn: draw both the \
                 same way round, or the sheet twists.",
            )
            .see_also("surface.loft")
            .example(
                "A slanted strip between two lines",
                r#"
                local a = pc.sketch.new{plane = "XY"}
                pc.sketch.line{sketch = a, x1 = 0, y1 = 0, x2 = 10, y2 = 0}
                local b = pc.sketch.new{plane = "XY", offset = 5}
                pc.sketch.line{sketch = b, x1 = 0, y1 = 3, x2 = 10, y2 = 3}
                local strip = pc.surface.ruled{sketches = {a, b}}
                assert(#pc.doc.rebuild() == 0)
                local area = pc.doc.measure{body = pc.doc.feature{id = strip}.body}.area
                assert(math.abs(area - 10 * math.sqrt(34)) < 1e-3, area)
                "#,
            ),
        "surface.loft" => spec
            .note(
                "Two sections or more, a sketch each, passed through in order; `closed = true` \
                 runs on from the last back to the first. Sections may differ: a square to a \
                 circle gives a face per side.",
            )
            .see_also("surface.ruled")
            .example(
                "A bulge through three circles",
                r#"
                local function ring(z, r)
                  local s = pc.sketch.new{plane = "XY", offset = z}
                  pc.sketch.circle{sketch = s, x = 0, y = 0, radius = r}
                  return s
                end
                local loft = pc.surface.loft{sketches = {ring(0, 5), ring(10, 8), ring(20, 5)}}
                assert(#pc.doc.rebuild() == 0)
                local body = pc.doc.feature{id = loft}.body
                assert(#pc.doc.faces{body = body} == 1)
                local m = pc.doc.measure{body = body}
                assert(math.abs(m.max[1] - 8) < 1e-3 and math.abs(m.max[3] - 20) < 1e-6)
                "#,
            ),
        "surface.sweep" => spec
            .note(
                "`sketches` is the profile's sketch, then the path's. A closed path fails at \
                 rebuild (\"a sweep surface along a closed spine is not built\"), which is \
                 what a circle profile and its path given the other way round meet.",
            )
            .see_also("surface.loft")
            .example(
                "A tube along a line",
                r#"
                local profile = pc.sketch.new{plane = "XY"}
                pc.sketch.circle{sketch = profile, x = 0, y = 0, radius = 2}
                local path = pc.sketch.new{plane = "XZ"}
                pc.sketch.line{sketch = path, x1 = 0, y1 = 0, x2 = 0, y2 = 20}
                local tube = pc.surface.sweep{sketches = {profile, path}}
                assert(#pc.doc.rebuild() == 0)
                local m = pc.doc.measure{body = pc.doc.feature{id = tube}.body}
                assert(math.abs(m.area - 2 * math.pi * 2 * 20) < 1e-3, m.area)
                assert(math.abs(m.max[3] - 20) < 1e-6)
                "#,
            ),
        "surface.offset" => spec
            .note(
                "`faces` are faces of the body as `pc.doc.faces` lists them (the entry \
                 itself will do; a curved face has no `normal` and needs none). The copy is \
                 added beside them, the faces kept. `distance` is 1 mm when left out.",
            )
            .see_also("doc.faces")
            .see_also("surface.thicken")
            .example(
                "A sheet copied 3 mm up",
                r#"
                local s = pc.sketch.new{plane = "XY"}
                pc.sketch.rect{sketch = s, x = 0, y = 0, width = 10, height = 10}
                local body = pc.doc.feature{id = pc.surface.planar{sketches = {s}}}.body
                assert(#pc.doc.rebuild() == 0)
                local face = pc.doc.faces{body = body}[1]
                pc.surface.offset{body = body, faces = {face}, distance = 3}
                assert(#pc.doc.rebuild() == 0)
                assert(#pc.doc.faces{body = body} == 2, "the sheet and its copy")
                assert(math.abs(pc.doc.measure{body = body}.max[3] - 3) < 1e-6)
                "#,
            ),
        "surface.extend" => spec
            .note(
                "`edges` are free edges of the body's sheets, each {point = e.point, \
                 direction = e.direction} from `pc.doc.edges`; each face grows past them by \
                 `length` (5 mm) and stays one face.",
            )
            .note(
                "`continuity` G1 (the default) and G0 run straight on, G2 along the face's \
                 own surface. Past a cylinder's straight side, a curved direction, only G2 \
                 builds; G1 and G0 fail at rebuild (\"a linear extension of an analytic \
                 surface across a curved parameter line would leave the surface\").",
            )
            .see_also("doc.edges")
            .example(
                "A sheet grown 5 mm past one side",
                r#"
                local s = pc.sketch.new{plane = "XY"}
                pc.sketch.rect{sketch = s, x = 0, y = 0, width = 10, height = 10}
                local body = pc.doc.feature{id = pc.surface.planar{sketches = {s}}}.body
                assert(#pc.doc.rebuild() == 0)
                local side
                for _, e in ipairs(pc.doc.edges{body = body}) do
                  if math.abs(e.point[1] - 10) < 1e-6 then side = e end
                end
                pc.surface.extend{body = body, length = 5,
                  edges = {{point = side.point, direction = side.direction}}}
                assert(#pc.doc.rebuild() == 0)
                local m = pc.doc.measure{body = body}
                assert(math.abs(m.area - 150) < 1e-6 and math.abs(m.max[1] - 15) < 1e-6)
                assert(#pc.doc.faces{body = body} == 1, "still one face")
                "#,
            ),
        "surface.blend" => spec
            .note(
                "`first` and `second` are edges of the body's sheets, each {point, direction} \
                 from `pc.doc.edges`; the blend is a new face between them, meeting each \
                 edge's face with `continuity` (G1 by default, G0 or G2).",
            )
            .see_also("doc.edges")
            .see_also("surface.fill")
            .example(
                "A bridge between two strips",
                r#"
                local a = pc.sketch.new{plane = "XY", offset = 0}
                pc.sketch.line{sketch = a, x1 = 0, y1 = 0, x2 = 10, y2 = 0}
                local low = pc.surface.extrude{sketches = {a}, length = 5, direction = "Y"}
                local body = pc.doc.feature{id = low}.body
                local b = pc.sketch.new{body = body, plane = "XY", offset = 10}
                pc.sketch.line{sketch = b, x1 = 0, y1 = 10, x2 = 10, y2 = 10}
                pc.surface.extrude{body = body, sketches = {b}, length = 5, direction = "Y"}
                assert(#pc.doc.rebuild() == 0)
                local first, second
                for _, e in ipairs(pc.doc.edges{body = body}) do
                  if math.abs(e.point[2] - 5) < 1e-6 and math.abs(e.point[3]) < 1e-6 then first = e end
                  if math.abs(e.point[2] - 10) < 1e-6 and math.abs(e.point[3] - 10) < 1e-6 then second = e end
                end
                pc.surface.blend{body = body,
                  first = {point = first.point, direction = first.direction},
                  second = {point = second.point, direction = second.direction}}
                assert(#pc.doc.rebuild() == 0)
                assert(#pc.doc.faces{body = body} == 3, "two strips and the bridge")
                assert(pc.doc.measure{body = body}.area > 200)
                "#,
            ),
        "surface.split" => spec
            .note(
                "`faces` are faces of the body as `pc.doc.faces` lists them. A sketch's curve \
                 lands on the face as seen square to the sketch's plane, so a sketch above \
                 the face cuts it too.",
            )
            .note(
                "Each curve must cross the face from edge to edge; one ending inside fails at \
                 rebuild (\"it must run from boundary to boundary\").",
            )
            .see_also("doc.faces")
            .see_also("surface.trim")
            .example(
                "A sheet cut in two along a line",
                r#"
                local s = pc.sketch.new{plane = "XY"}
                pc.sketch.rect{sketch = s, x = 0, y = 0, width = 10, height = 10}
                local body = pc.doc.feature{id = pc.surface.planar{sketches = {s}}}.body
                assert(#pc.doc.rebuild() == 0)
                local face = pc.doc.faces{body = body}[1]
                local cut = pc.sketch.new{body = body, plane = "XY", offset = 5}
                pc.sketch.line{sketch = cut, x1 = 4, y1 = -1, x2 = 4, y2 = 11}
                pc.surface.split{body = body, faces = {face}, sketches = {cut}}
                assert(#pc.doc.rebuild() == 0)
                local areas = {}
                for _, f in ipairs(pc.doc.faces{body = body}) do areas[#areas + 1] = f.area end
                table.sort(areas)
                assert(#areas == 2 and math.abs(areas[1] - 40) < 1e-3 and math.abs(areas[2] - 60) < 1e-3)
                "#,
            ),
        "surface.sew" => spec
            .note(
                "It joins every face of the body where their edges meet; `gap` (mm) joins \
                 edges up to that far apart too.",
            )
            .note(
                "A shell that closes becomes a solid, with a volume. One that does not stays \
                 a sheet, its faces now sharing edges: `surface.check` lists those joins and \
                 `surface.fillet` can round them.",
            )
            .see_also("surface.check")
            .see_also("surface.fillet")
            .see_also("surface.planar")
            .example(
                "Two walls joined at their corner",
                r#"
                local a = pc.sketch.new{plane = "XY"}
                pc.sketch.line{sketch = a, x1 = 0, y1 = 0, x2 = 10, y2 = 0}
                local wall = pc.surface.extrude{sketches = {a}, length = 5}
                local body = pc.doc.feature{id = wall}.body
                local b = pc.sketch.new{body = body, plane = "XY"}
                pc.sketch.line{sketch = b, x1 = 0, y1 = 10, x2 = 0, y2 = 0}
                pc.surface.extrude{body = body, sketches = {b}, length = 5}
                assert(#pc.doc.rebuild() == 0)
                assert(#pc.surface.check{body = body} == 0, "touching, not joined")
                pc.surface.sew{body = body}
                assert(#pc.doc.rebuild() == 0)
                assert(#pc.surface.check{body = body} == 1, "one shared edge")
                "#,
            ),
        "surface.fillet" => spec
            .note(
                "`edges` are edges two faces of the body share, each {point, direction} from \
                 `pc.doc.edges`; `radius` is 2 mm when left out.",
            )
            .note(
                "Separate sheets that only touch share no edge: the round fails at rebuild \
                 (\"Sew them first so they share it\"). Sew them, or draw the faces from one \
                 sketch.",
            )
            .see_also("surface.sew")
            .see_also("doc.edges")
            .example(
                "Two walls rounded at their corner",
                r#"
                local s = pc.sketch.new{plane = "XY"}
                pc.sketch.polyline{sketch = s, points = {{10, 0}, {0, 0}, {0, 10}}}
                local walls = pc.surface.extrude{sketches = {s}, length = 5}
                local body = pc.doc.feature{id = walls}.body
                assert(#pc.doc.rebuild() == 0)
                local corner
                for _, e in ipairs(pc.doc.edges{body = body}) do
                  if #e.faces == 2 then corner = e end
                end
                pc.surface.fillet{body = body, radius = 2,
                  edges = {{point = corner.point, direction = corner.direction}}}
                assert(#pc.doc.rebuild() == 0)
                assert(#pc.doc.faces{body = body} == 3, "two walls and the round")
                local area = pc.doc.measure{body = body}.area
                assert(math.abs(area - (100 - 4 * 5 + math.pi * 5)) < 1e-3, area)
                "#,
            ),
        "surface.thicken" => spec
            .note(
                "Each sheet becomes a solid: `thickness` (2 mm) along the faces' normals, \
                 negative the other way, or half each way with `both_sides = true`.",
            )
            .note(
                "A body already sewn into a solid is refused at rebuild (\"the body has no \
                 sheet to thicken\"): Design's Thickness hollows a solid.",
            )
            .see_also("surface.sew")
            .see_also("surface.offset")
            .example(
                "A sheet given 2 mm",
                r#"
                local s = pc.sketch.new{plane = "XY"}
                pc.sketch.rect{sketch = s, x = 0, y = 0, width = 10, height = 10}
                local body = pc.doc.feature{id = pc.surface.planar{sketches = {s}}}.body
                pc.surface.thicken{body = body, thickness = 2}
                assert(#pc.doc.rebuild() == 0)
                local m = pc.doc.measure{body = body}
                assert(math.abs(m.volume - 200) < 1e-6, m.volume)
                assert(#pc.doc.faces{body = body} == 6, "a box")
                "#,
            ),
        "surface.trim" => spec
            .note(
                "It keeps what lies on the side the plane's normal points to: YZ keeps +X. \
                 `flip = true` keeps the other side; `offset` moves the plane along its \
                 normal, YZ with 4 being x = 4.",
            )
            .note(
                "A plane that leaves nothing fails at rebuild (\"the plane leaves nothing of \
                 the body on the side kept; flip it or move it\"). It cuts a sewn solid too, \
                 which stays solid.",
            )
            .see_also("surface.split")
            .see_also("surface.mirror")
            .example(
                "A sheet cut at x = 4, either side kept",
                r#"
                local s = pc.sketch.new{plane = "XY"}
                pc.sketch.rect{sketch = s, x = 0, y = 0, width = 10, height = 10}
                local body = pc.doc.feature{id = pc.surface.planar{sketches = {s}}}.body
                local trim = pc.surface.trim{body = body, plane = "YZ", offset = 4}
                assert(#pc.doc.rebuild() == 0)
                local m = pc.doc.measure{body = body}
                assert(math.abs(m.area - 60) < 1e-6 and math.abs(m.min[1] - 4) < 1e-6)
                pc.surface.set{feature = trim, flip = true}
                assert(#pc.doc.rebuild() == 0)
                assert(math.abs(pc.doc.measure{body = body}.area - 40) < 1e-6)
                "#,
            ),
        "surface.mirror" => spec
            .note(
                "It adds the body's reflection beside what the body holds, the original kept; \
                 the two are separate pieces of the one body.",
            )
            .note(
                "`plane` YZ (the default), XZ, XY or {Custom = {origin, normal}}, in the \
                 body's frame; `offset` moves it along its normal: YZ with 10 reflects in \
                 x = 10.",
            )
            .see_also("surface.trim")
            .example(
                "A sheet and its reflection",
                r#"
                local s = pc.sketch.new{plane = "XY"}
                pc.sketch.rect{sketch = s, x = 0, y = 0, width = 10, height = 10}
                local body = pc.doc.feature{id = pc.surface.planar{sketches = {s}}}.body
                pc.surface.mirror{body = body, plane = "YZ"}
                assert(#pc.doc.rebuild() == 0)
                local m = pc.doc.measure{body = body}
                assert(math.abs(m.area - 200) < 1e-6 and math.abs(m.min[1] + 10) < 1e-6)
                assert(#pc.doc.faces{body = body} == 2)
                "#,
            ),
        _ => spec,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feature::KINDS;
    use core_document::WorkbenchFeature;

    /// A step a tool makes records its command with every field by name,
    /// so each field is an argument its command takes.
    #[test]
    fn every_field_of_a_step_is_an_argument_of_its_command() {
        for kind in KINDS {
            let spec = step(kind);
            let feature = SurfaceFeature::for_tool(kind.tool).unwrap();
            let serde_json::Value::Object(outer) = feature.to_json() else {
                panic!("{} is an object", kind.tool);
            };
            if let Some(serde_json::Value::Object(fields)) = outer.values().next() {
                for name in fields.keys() {
                    assert!(
                        spec.params.iter().any(|p| &p.name == name),
                        "{} takes `{name}`",
                        kind.tool
                    );
                }
            }
            assert!(!spec.notes.is_empty(), "{} has notes", kind.tool);
            assert_eq!(spec.examples.len(), 1, "{} has an example", kind.tool);
        }
    }
}
