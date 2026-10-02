//! Surfaces through the real benches: a script sketches, the Surface
//! bench's commands make steps from the sketches, and the kernel builds
//! what the body's plan says.

use core_document::{
    BodyId, CommandArgs, CommandError, CommandResult, CommandSpec, Document, DocumentService,
    WorkbenchId, WorkbenchRuntimeContext,
};
use kernel_api::TessellationSettings;
use kernel_ogeom::OgeomKernel;
use ogeom::topo::{Model, ShapeType, explore_unique};
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

fn benches() -> Benches {
    let mut registry = DocumentService::default();
    for bench in [
        Box::new(wb_sketch::SketchWorkbench::default()) as Box<dyn core_document::Workbench>,
        Box::new(wb_design::DesignWorkbench::default()),
        Box::new(wb_surface::SurfaceWorkbench::default()),
    ] {
        registry.register_workbench(bench).unwrap();
    }
    Benches {
        registry,
        document: Document::new("surfaces"),
    }
}

fn run(host: &mut Benches, script: &str) {
    let out = ScriptEngine::new().run_script(script, "surface.lua", host);
    assert_eq!(out.error, None, "{:?}", out.printed);
    host.registry.evaluate(&mut host.document);
}

/// The body the surface steps went in, and how many faces and shells its
/// built shape has.
fn built(host: &Benches) -> (BodyId, usize, usize, usize) {
    let body = host
        .document
        .feature_tree()
        .all_nodes()
        .find(|(_, n)| n.workbench_id.as_str() == "wb.surface")
        .and_then(|(_, n)| n.body)
        .expect("a surface body");
    let plan = wb_surface::build::body_plan(&host.document, body).unwrap();
    let result = OgeomKernel::new()
        .execute_solid_chain(&plan.ops, &TessellationSettings::default())
        .unwrap();
    let mut model = Model::new();
    let text = std::str::from_utf8(&result.brep_blob).unwrap();
    let shape = ogeom::io::native::read_into(&mut model, text)
        .unwrap()
        .shapes[0]
        .clone();
    let count = |kind| explore_unique(&model, &shape, kind).unwrap().len();
    (
        body,
        count(ShapeType::Face),
        count(ShapeType::Shell),
        count(ShapeType::Solid),
    )
}

#[test]
fn an_open_sketch_extrudes_into_a_surface_body() {
    let mut host = benches();
    run(
        &mut host,
        r#"
        local s = pc.sketch.new{plane = "XY"}
        pc.sketch.polyline{sketch = s, points = {{0, 0}, {10, 0}, {10, 10}}}
        pc.surface.extrude{sketches = {s}, length = 5}
        "#,
    );
    let (body, faces, _, solids) = built(&host);
    assert_eq!((faces, solids), (2, 0));
    let sketch_body = host
        .document
        .feature_tree()
        .all_nodes()
        .find(|(_, n)| n.workbench_id.as_str() == "wb.sketch")
        .and_then(|(_, n)| n.body);
    assert_eq!(
        sketch_body,
        Some(body),
        "the surface goes beside its sketch"
    );
}

/// Walls and a floor sewn: an open box, one shell of five faces.
#[test]
fn walls_and_a_floor_sew_into_one_shell() {
    let mut host = benches();
    run(
        &mut host,
        r#"
        local s = pc.sketch.new{plane = "XY"}
        pc.sketch.polyline{sketch = s, points = {{0, 0}, {10, 0}, {10, 10}, {0, 10}, {0, 0}}}
        local walls = pc.surface.extrude{sketches = {s}, length = 5}
        pc.surface.planar{body = walls, sketches = {s}}
        pc.surface.sew{body = walls}
        "#,
    );
    let (_, faces, shells, solids) = built(&host);
    assert_eq!((faces, shells, solids), (5, 1, 0));
}

/// A Design feature asked for on a surface body goes into a body of its
/// own; the surface body keeps building from its surfaces.
#[test]
fn a_design_feature_never_lands_in_a_surface_body() {
    let mut host = benches();
    run(
        &mut host,
        r#"
        local s = pc.sketch.new{plane = "XY"}
        pc.sketch.polyline{sketch = s, points = {{0, 0}, {10, 0}, {10, 10}}}
        pc.surface.extrude{sketches = {s}, length = 5}
        "#,
    );
    let (surface_body, ..) = built(&host);
    let design_in_it = host
        .document
        .feature_tree()
        .all_nodes()
        .any(|(_, n)| n.workbench_id.as_str() == "wb.design" && n.body == Some(surface_body));
    assert!(!design_in_it);
    assert!(wb_surface::SurfaceWorkbench::takes_surfaces(
        &host.document,
        surface_body
    ));
}
