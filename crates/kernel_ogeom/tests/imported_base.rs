//! An imported solid takes features: the first one gives the body a base
//! shape, the imported solid, and builds on it, as a primitive's body
//! builds on the primitive.

use core_document::{
    BodyId, CommandArgs, CommandError, CommandResult, CommandSpec, Document, DocumentService,
    ImportedGeometry, WorkbenchId, WorkbenchRuntimeContext,
};
use kernel_api::{Kernel, TessellationSettings};
use kernel_ogeom::OgeomKernel;
use scripting::ScriptEngine;
use wb_design::DesignFeature;

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

/// The bundled STEP box, in a document as an import leaves it.
fn imported_box() -> (Document, BodyId, [f32; 3], [f32; 3]) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/box_native.step");
    let model = OgeomKernel::new()
        .import_step(&path, &TessellationSettings::default())
        .unwrap();
    let imported = &model.bodies[0];
    let mut document = Document::new("imported");
    let body = document.create_body(Some("Box".into()));
    let mesh = std::sync::Arc::new(imported.mesh.clone());
    let (lo, hi) = mesh.bounds().unwrap();
    document.set_imported_geometry(
        body,
        ImportedGeometry {
            bounds_mm: mesh.bounds(),
            mesh,
            // Any asset: what matters is that the import stamped one.
            source_asset: Some(body.0),
            revision: 0,
            brep_blob_path: None,
            face_colors_path: None,
            health: None,
        },
    );
    document.set_imported_brep_data(body, imported.brep_blob.clone(), Vec::new());
    (document, body, lo, hi)
}

fn benches(document: Document) -> Benches {
    let mut registry = DocumentService::default();
    registry
        .register_workbench(Box::new(wb_sketch::SketchWorkbench::default()))
        .unwrap();
    registry
        .register_workbench(Box::new(wb_design::DesignWorkbench::default()))
        .unwrap();
    Benches { registry, document }
}

#[test]
fn a_pocket_on_an_imported_box_builds_on_the_box() {
    let (document, body, lo, hi) = imported_box();
    assert!(document.body_solid_is_imported(body));
    let mut host = benches(document);
    let (cx, cy) = ((lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0);
    let out = ScriptEngine::new().run_script(
        &format!(
            r#"
            local s = pc.sketch.new{{body = "{body}", plane = "XY", offset = {top}}}
            pc.sketch.rect{{sketch = s, x = {x}, y = {y}, width = 2, height = 2}}
            pocket = pc.design.pocket{{sketch = s, depth = 1}}
            "#,
            body = body.0,
            top = hi[2],
            x = cx - 1.0,
            y = cy - 1.0,
        ),
        "pocket.lua",
        &mut host,
    );
    assert_eq!(out.error, None);

    // The body is the one imported, its history the box and then the
    // pocket.
    let document = &host.document;
    assert_eq!(document.bodies().len(), 1, "no second body");
    assert!(document.has_base_solid(body));
    let features = wb_design::design_features_of_body(document, body);
    assert!(matches!(features[0].1, DesignFeature::Base {}));
    assert!(matches!(features[1].1, DesignFeature::Pocket { .. }));

    let ops = wb_design::body_build_ops(document, body).unwrap().ops;
    let built = OgeomKernel::new()
        .execute_solid_chain(&ops, &TessellationSettings::default())
        .unwrap();
    let (blo, bhi) = built.mesh.bounds().unwrap();
    for axis in 0..3 {
        assert!((blo[axis] - lo[axis]).abs() < 1e-3 && (bhi[axis] - hi[axis]).abs() < 1e-3);
    }
    let whole = OgeomKernel::new()
        .execute_solid_chain(&ops[..1], &TessellationSettings::default())
        .unwrap();
    let volume = |r: &kernel_api::SolidBuildResult| {
        OgeomKernel::new()
            .physical_properties(&r.brep_blob)
            .unwrap()
            .volume_mm3
            .expect("a closed solid")
    };
    let cut = volume(&whole) - volume(&built);
    assert!((cut - 4.0).abs() < 1e-3, "a 2 × 2 × 1 pocket: {cut}");
}

#[test]
fn the_base_goes_last_and_takes_the_body_back_to_its_import() {
    let (document, body, _, hi) = imported_box();
    let mut host = benches(document);
    let out = ScriptEngine::new().run_script(
        &format!(
            r#"
            local s = pc.sketch.new{{body = "{body}", plane = "XY", offset = {top}}}
            pc.sketch.rect{{sketch = s, x = 0, y = 0, width = 1, height = 1}}
            pocket = pc.design.pocket{{sketch = s, depth = 0.5}}
            "#,
            body = body.0,
            top = hi[2],
        ),
        "pocket.lua",
        &mut host,
    );
    assert_eq!(out.error, None);
    let features = wb_design::design_features_of_body(&host.document, body);
    let (base, pocket) = (features[0].0, features[1].0);
    assert!(
        !wb_design::delete_feature(&mut host.document, base),
        "the pocket builds on it"
    );
    assert!(wb_design::delete_feature(&mut host.document, pocket));
    assert!(wb_design::delete_feature(&mut host.document, base));
    assert!(!host.document.has_base_solid(body));
    assert!(
        host.document.body_solid_is_imported(body),
        "the imported box again"
    );
    assert!(host.document.imported_brep_blob(body).is_some());
}
