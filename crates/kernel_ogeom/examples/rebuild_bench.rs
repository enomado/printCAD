//! Time the rebuilds an edit makes: a plate with pockets, bosses, a
//! patterned slot and a fillet, built through the workbenches' commands as
//! a script would, then built by the kernel as the application builds it:
//! from scratch, after an edit to its last feature, after one in the middle
//! of its history, and again unchanged.
//!
//! ```text
//! cargo run --release -p kernel_ogeom --example rebuild_bench [-- --ops]
//! ```
//!
//! `--ops` prints each feature's time and the meshing as the chain logs
//! them (target `printcad.chain`).

use core_document::{
    CommandArgs, CommandError, CommandResult, CommandSpec, Document, DocumentService, FeatureId,
    WorkbenchId, WorkbenchRuntimeContext,
};
use kernel_api::{SolidBuildResult, TessellationSettings};
use kernel_ogeom::OgeomKernel;
use scripting::ScriptEngine;

struct Benches {
    registry: DocumentService,
    document: Document,
}

impl scripting::Host for Benches {
    fn commands(&self) -> Vec<CommandSpec> {
        self.registry
            .commands()
            .into_iter()
            .map(|(_, c)| c.clone())
            .collect()
    }

    fn call(&mut self, id: &str, args: CommandArgs) -> CommandResult {
        let (bench, spec): (WorkbenchId, _) = self
            .registry
            .command(id)
            .ok_or_else(|| CommandError::Unknown(id.to_string()))?;
        spec.check(&args)?;
        let id = spec.id.clone();
        let wb = self.registry.workbench_mut(&bench).unwrap();
        let mut ctx =
            WorkbenchRuntimeContext::new(&mut self.document, [0.0; 3], [0.0; 3], (0, 0, 1, 1));
        wb.run_command(&id, &args, &mut ctx)
    }
}

const PLATE: &str = r#"
local s = pc.sketch.new{plane = "XY"}
pc.sketch.rect{sketch = s, x = 0, y = 0, width = 160, height = 100}
pc.design.pad{sketch = s, length = 10}
"#;

/// The rest, on the plate's body (`BODY` stands for its id).
const PART: &str = r#"
for i = 0, 11 do
    local h = pc.sketch.new{body = "BODY", plane = "XY", offset = 10}
    pc.sketch.circle{sketch = h, x = 15 + (i % 6) * 26, y = 15 + math.floor(i / 6) * 70, radius = 4}
    pc.design.pocket{sketch = h, depth = 6}
end
middle = nil
for i = 0, 5 do
    local b = pc.sketch.new{body = "BODY", plane = "XY", offset = 10}
    pc.sketch.circle{sketch = b, x = 25 + i * 22, y = 50, radius = 6}
    local boss = pc.design.pad{sketch = b, length = 8}
    if i == 2 then middle = boss end
end
local p = pc.sketch.new{body = "BODY", plane = "XY", offset = 10}
pc.sketch.rect{sketch = p, x = 10, y = 30, width = 6, height = 8}
pc.design.pocket{sketch = p, depth = 4}
pc.design.linear_pattern{body = "BODY", length = 130, occurrences = 6}
pc.design.fillet{body = "BODY", radius = 1, face_point = {80, 50, 0}, face_normal = {0, 0, -1}}
local t = pc.sketch.new{body = "BODY", plane = "XY", offset = 18}
pc.sketch.circle{sketch = t, x = 80, y = 50, radius = 3}
last = pc.design.pad{sketch = t, length = 4}
"#;

fn main() {
    if std::env::args().any(|a| a == "--ops") {
        tracing_subscriber::fmt()
            .with_env_filter("printcad.chain=debug")
            .with_target(false)
            .without_time()
            .init();
    }
    let mut registry = DocumentService::default();
    registry
        .register_workbench(Box::new(wb_sketch::SketchWorkbench::default()))
        .unwrap();
    registry
        .register_workbench(Box::new(wb_design::DesignWorkbench::default()))
        .unwrap();
    let mut host = Benches {
        registry,
        document: Document::new("bench"),
    };
    let mut engine = ScriptEngine::new();
    let out = engine.run_script(PLATE, "plate.lua", &mut host);
    assert_eq!(out.error, None, "the plate builds");
    let body = host.document.bodies()[0].id;
    let part = PART.replace("BODY", &body.0.to_string());
    let out = engine.run_script(&part, "part.lua", &mut host);
    assert_eq!(out.error, None, "the part builds");
    let ids = engine.run_script("print(last) print(middle)", "ids.lua", &mut host);
    let feature = |i: usize| FeatureId(ids.printed[i].trim().parse().expect("a feature id"));
    let (last, middle) = (feature(0), feature(1));
    let detail = TessellationSettings::default();

    let build = |host: &mut Benches, label: &str| {
        host.registry.evaluate(&mut host.document);
        let plan = wb_design::body_build_ops(&host.document, body).unwrap();
        let tags: Vec<_> = plan
            .op_features
            .iter()
            .map(|f| kernel_api::naming::name_of_id(f.0.as_bytes()))
            .collect();
        let started = std::time::Instant::now();
        let built: SolidBuildResult = OgeomKernel::new()
            .execute_solid_chain_named(&plan.ops, &tags, &detail, None, &[])
            .unwrap();
        println!(
            "{label:<28} {:>4} ops  {:>8.1} ms  {} triangles",
            plan.ops.len(),
            started.elapsed().as_secs_f64() * 1000.0,
            built.mesh.indices.len() / 3
        );
    };
    let mut set = |host: &mut Benches, feature: FeatureId, length: f64| {
        let out = engine.run_script(
            &format!(
                "pc.design.set{{feature = \"{}\", length = {length}}}",
                feature.0
            ),
            "set.lua",
            host,
        );
        assert_eq!(out.error, None);
    };

    build(&mut host, "from scratch");
    set(&mut host, last, 5.0);
    build(&mut host, "last feature edited");
    set(&mut host, last, 6.0);
    build(&mut host, "last feature edited again");
    set(&mut host, middle, 9.0);
    build(&mut host, "a middle feature edited");
    set(&mut host, middle, 10.0);
    build(&mut host, "the same one again");
    build(&mut host, "nothing changed");
}
