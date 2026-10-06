//! Real packages through the host: the Gear and CAM examples and a
//! package that misbehaves on request, all built from `sdk/` for
//! wasm32-wasip2.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use core_document::{Document, DocumentService, FeatureInfo, WorkbenchId, WorkbenchRuntimeContext};
use serde_json::{Value, json};
use wb_wasm::{Capabilities, Package};

/// The SDK workspace's release build for wasm32-wasip2, built once.
fn guests() -> &'static Path {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT.get_or_init(|| {
        let sdk = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../sdk");
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let status = std::process::Command::new(cargo)
            .current_dir(&sdk)
            .args([
                "build",
                "--release",
                "--target",
                "wasm32-wasip2",
                "-p",
                "gear",
                "-p",
                "cam",
                "-p",
                "rogue",
            ])
            .env_remove("RUSTFLAGS")
            .env_remove("CARGO_ENCODED_RUSTFLAGS")
            .status()
            .expect("run cargo for the SDK");
        assert!(
            status.success(),
            "the SDK's packages build for wasm32-wasip2"
        );
        sdk.join("target/wasm32-wasip2/release")
    })
}

/// A fresh folder under the system's temporary one.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "printcad-wasm-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Package folder `source` (under `sdk/`) with its component, packed into
/// an archive under a fresh folder.
fn archive(source: &str, component: &str, name: &str) -> PathBuf {
    let sdk = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../sdk");
    let work = scratch(name);
    let folder = work.join("src");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::copy(
        sdk.join(source).join("bench.toml"),
        folder.join("bench.toml"),
    )
    .unwrap();
    std::fs::copy(guests().join(component), folder.join("bench.wasm")).unwrap();
    let icons = sdk.join(source).join("icons");
    if icons.is_dir() {
        std::fs::create_dir_all(folder.join("icons")).unwrap();
        for entry in std::fs::read_dir(icons).unwrap().flatten() {
            std::fs::copy(entry.path(), folder.join("icons").join(entry.file_name())).unwrap();
        }
    }
    let archive = work.join("package.pcbench");
    wb_wasm::pack(&folder, &archive).expect("packs");
    archive
}

/// [`archive`], installed under a fresh root beside it.
fn installed(source: &str, component: &str, name: &str) -> Package {
    let archive = archive(source, component, name);
    wb_wasm::install(&archive, &archive.parent().unwrap().join("installed")).expect("installs")
}

fn registry_with(package: &Package, granted: Capabilities) -> DocumentService {
    let bench = wb_wasm::load(package, &granted).expect("loads");
    let mut registry = DocumentService::default();
    registry
        .register_workbench(Box::new(bench))
        .expect("registers");
    registry
}

fn run(
    registry: &mut DocumentService,
    document: &mut Document,
    bench: &str,
    command: &str,
    args: Value,
) -> Result<Value, String> {
    let mut ctx =
        WorkbenchRuntimeContext::new(document, [0.0, 0.0, 100.0], [0.0; 3], (0, 0, 800, 600));
    let args = args.as_object().cloned().unwrap_or_default();
    registry
        .workbench_mut(&WorkbenchId::new(bench))
        .unwrap()
        .run_command(command, &args, &mut ctx)
        .map_err(|e| e.to_string())
}

fn volume(ops: &[kernel_api::SolidOp]) -> f64 {
    let mut kernel = kernel_ogeom::OgeomKernel::new();
    let built = kernel
        .execute_solid_chain(ops, &kernel_api::TessellationSettings::default())
        .expect("the plan builds");
    kernel
        .physical_properties(&built.brep_blob)
        .expect("measures")
        .volume_mm3
        .expect("a closed solid")
}

/// The registry's commands, run from a script against one document.
struct Scripted<'a> {
    registry: &'a mut DocumentService,
    document: Document,
}

impl scripting::Host for Scripted<'_> {
    fn commands(&self) -> Vec<core_document::CommandSpec> {
        self.registry
            .commands()
            .into_iter()
            .map(|(_, c)| c.clone())
            .collect()
    }

    fn call(&mut self, id: &str, args: core_document::CommandArgs) -> core_document::CommandResult {
        let (bench, spec) = self
            .registry
            .command(id)
            .ok_or_else(|| core_document::CommandError::Unknown(id.to_string()))?;
        spec.check(&args)?;
        let mut ctx = WorkbenchRuntimeContext::new(
            &mut self.document,
            [0.0, 0.0, 100.0],
            [0.0; 3],
            (0, 0, 800, 600),
        );
        self.registry
            .workbench_mut(&bench)
            .unwrap()
            .run_command(id, &args, &mut ctx)
    }
}

/// A package's commands carry their notes and examples to the host, and
/// each example runs, from an empty document, as it says.
#[test]
fn a_package_s_command_examples_reach_the_host_and_run() {
    let cam = [
        "example.cam.pocket",
        "example.cam.toolpath",
        "example.cam.gcode",
    ];
    for (source, component, commands) in [
        ("examples/gear", "gear.wasm", &["example.gear.make"][..]),
        ("examples/cam", "cam.wasm", &cam[..]),
    ] {
        let package = installed(source, component, &format!("{component}-examples"));
        let mut registry = registry_with(&package, Capabilities::default());
        for command in commands {
            let (_, spec) = registry.command(command).unwrap();
            assert!(!spec.notes.is_empty(), "{command}'s notes");
            let examples = spec.examples.clone();
            assert!(!examples.is_empty(), "{command}'s examples");
            for example in examples {
                let mut host = Scripted {
                    registry: &mut registry,
                    document: Document::new("example"),
                };
                let out = scripting::ScriptEngine::new().run_script(
                    &example.script,
                    &example.title,
                    &mut host,
                );
                assert_eq!(out.error, None, "{command}, {}", example.title);
            }
        }
    }
}

#[test]
fn a_gear_package_installs_registers_and_builds_a_parametric_gear() {
    let package = installed("examples/gear", "gear.wasm", "gear");
    assert_eq!(package.manifest.id, "example.gear");
    let mut registry = registry_with(
        &package,
        Capabilities {
            save_dialog: true,
            ..Default::default()
        },
    );
    let id = WorkbenchId::new("example.gear");

    let tools = registry.tools_for(&id).unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].id, "example.gear.new");
    assert_eq!(tools[0].icon, Some("example.gear/gear"));
    assert!(
        ui_kit::icon::exists("example.gear/gear"),
        "the package's icon is drawable"
    );
    assert!(registry.command("example.gear.make").is_some());

    let mut document = Document::new("gears");
    let made = run(
        &mut registry,
        &mut document,
        "example.gear",
        "example.gear.make",
        json!({"teeth": 20, "module": 2.0, "thickness": 8.0, "bore": 5.0}),
    )
    .expect("makes a gear");
    let feature = core_document::FeatureId(made["feature"].as_str().unwrap().parse().unwrap());
    let node = document.get_feature_meta(feature).unwrap().clone();
    assert_eq!(node.workbench_id.as_str(), "example.gear.gear");
    assert_eq!(node.made_by.as_deref(), Some("example.gear 0.1.0"));

    let info = registry.feature_info(&node).unwrap();
    assert_eq!(info.kind_label, "Spur gear, 20 teeth");
    assert_eq!(info.icon, "example.gear/gear");
    assert_eq!(registry.parameters(&node).len(), 4);

    let jobs = registry.rebuild_jobs(&mut document);
    assert_eq!(jobs.len(), 1, "one body to build");
    let plan = jobs.into_iter().next().unwrap().plan.expect("a plan");
    assert_eq!(plan.op_features, vec![feature]);
    let v20 = volume(&plan.ops);
    // Between the root circle and the tip circle, less the bore.
    let (root, tip, bore) = (20.0 - 2.5, 22.0, 2.5f64);
    let pi = std::f64::consts::PI;
    assert!(v20 > pi * (root * root - bore * bore) * 8.0, "{v20}");
    assert!(v20 < pi * (tip * tip - bore * bore) * 8.0, "{v20}");
    assert!(
        registry.rebuild_jobs(&mut document).is_empty(),
        "the plan settled the dirty flags"
    );

    // A formula drives it like any built-in feature.
    document
        .set_feature_formula(feature, "/teeth", Some("20 + 4".into()))
        .unwrap();
    let jobs = registry.rebuild_jobs(&mut document);
    assert_eq!(jobs.len(), 1, "the formula marked it for rebuilding");
    let v24 = volume(&jobs.into_iter().next().unwrap().plan.unwrap().ops);
    assert!(v24 > v20 * 1.2, "more teeth, a bigger gear: {v20} → {v24}");

    // A double click on its row opens its task; selecting it does not.
    let mut ctx =
        WorkbenchRuntimeContext::new(&mut document, [0.0, 0.0, 100.0], [0.0; 3], (0, 0, 800, 600));
    let bench = registry.workbench_mut(&id).unwrap();
    assert!(bench.task(&ctx).is_none());
    bench.edit_feature(&mut ctx, feature);
    assert_eq!(
        bench.task(&ctx).map(|t| t.title),
        Some("Gear".into()),
        "a double click opens the gear"
    );
}

/// The moves of a G-code program: whether rapid, and where the tool is
/// after each line that moves it.
fn moves(program: &str) -> Vec<(bool, [f64; 3])> {
    let mut at = [f64::NAN; 3];
    let mut out = Vec::new();
    for line in program.lines() {
        let code = line.split('(').next().unwrap_or("");
        let mut rapid = None;
        let mut moved = false;
        for word in code.split_whitespace() {
            let (letter, value) = word.split_at(1);
            let Ok(v) = value.parse::<f64>() else {
                continue;
            };
            match letter {
                "G" if v == 0.0 || v == 1.0 => rapid = Some(v == 0.0),
                "X" | "Y" | "Z" => {
                    at["XYZ".find(letter).unwrap()] = v;
                    moved = true;
                }
                _ => {}
            }
        }
        if let (Some(rapid), true) = (rapid, moved) {
            out.push((rapid, at));
        }
    }
    out
}

/// Wait out the CAM bench's jobs, then let it hear how they ended, as its
/// next frame would; what it logged.
/// Wait for the CAM bench's jobs to end and deliver them; `says` when the
/// ending is one the bench logs (a stop, a failure), which can land a
/// moment after the job stops being busy.
fn finish_jobs(registry: &mut DocumentService, document: &mut Document, says: bool) -> Vec<String> {
    let bench = registry
        .workbench_mut(&WorkbenchId::new("example.cam"))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    while bench.busy() {
        assert!(Instant::now() < deadline, "the job ends");
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut ctx =
        WorkbenchRuntimeContext::new(document, [0.0, 0.0, 100.0], [0.0; 3], (0, 0, 800, 600));
    let mut logs = Vec::new();
    loop {
        bench.on_frame(0.016, &mut ctx);
        logs.extend(ctx.drain_logs().into_iter().map(|l| l.message));
        if !says || !logs.is_empty() || Instant::now() > deadline {
            return logs;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn feature_of(made: &Value) -> core_document::FeatureId {
    core_document::FeatureId(made["feature"].as_str().unwrap().parse().unwrap())
}

/// A square sketch, `side` across from its corner at the origin, on a body
/// placed at `at`.
fn square_sketch(document: &mut Document, side: f32, at: [f32; 3]) -> core_document::FeatureId {
    use wb_sketch::sketch::{GeometryElement, Line, Point, Sketch, Vec2D};
    let body = document.create_body(Some("Stock".into()));
    document.set_body_placement(
        body,
        core_document::BodyPlacement::new(glam::Quat::IDENTITY, glam::Vec3::from_array(at)),
    );
    let mut sketch = Sketch::new("Outline");
    let corners: Vec<_> = [[0.0, 0.0], [side, 0.0], [side, side], [0.0, side]]
        .into_iter()
        .map(|[x, y]| sketch.add_geometry(GeometryElement::Point(Point::new(Vec2D::new(x, y)))))
        .collect();
    for i in 0..4 {
        sketch.add_geometry(GeometryElement::Line(Line::new(
            corners[i],
            corners[(i + 1) % 4],
        )));
    }
    let plane = sketch.plane;
    document
        .add_feature_in_body(
            wb_sketch::SketchFeature::new(sketch, plane),
            "Outline".into(),
            Some(body),
        )
        .unwrap()
}

#[test]
fn a_cam_package_works_out_a_pocket_in_a_job_and_writes_its_g_code() {
    wb_wasm::set_profile_source(wb_sketch::profile::closed_profile);
    let package = installed("examples/cam", "cam.wasm", "cam");
    assert_eq!(package.manifest.id, "example.cam");
    let mut registry = registry_with(
        &package,
        Capabilities {
            save_dialog: true,
            ..Default::default()
        },
    );
    let id = WorkbenchId::new("example.cam");
    let tools = registry.tools_for(&id).unwrap();
    assert_eq!(tools[0].id, "example.cam.new");
    assert!(ui_kit::icon::exists("example.cam/pocket"));

    // A 40 mm square sketched on a body standing at (100, 0, 10).
    let mut document = Document::new("cam");
    let sketch = square_sketch(&mut document, 40.0, [100.0, 0.0, 10.0]);
    let (radius, stepover, step_down) = (3.0, 2.4, 2.0);
    let made = run(
        &mut registry,
        &mut document,
        "example.cam",
        "example.cam.pocket",
        json!({
            "sketch": sketch.0.to_string(),
            "depth": 5.0,
            "tool_diameter": 2.0 * radius,
            "stepover": stepover,
            "step_down": step_down,
        }),
    )
    .expect("makes a pocket");
    let feature = feature_of(&made);
    let node = document.get_feature_meta(feature).unwrap().clone();
    assert_eq!(node.workbench_id.as_str(), "example.cam.pocket");
    assert_eq!(node.body, None, "an operation sits on no body");
    assert_eq!(node.data["top"], json!(10.0), "the sketch's height");
    let info = registry.feature_info(&node).unwrap();
    assert!(!info.builds_solid);
    assert_eq!(registry.parameters(&node).len(), 9);
    let toolpath = |registry: &mut DocumentService, document: &mut Document| {
        run(
            registry,
            document,
            "example.cam",
            "example.cam.toolpath",
            json!({"id": feature.0.to_string()}),
        )
        .unwrap()
    };
    assert_eq!(toolpath(&mut registry, &mut document)["state"], "none");

    // Opening it starts the job, away from the window.
    {
        let mut ctx = WorkbenchRuntimeContext::new(
            &mut document,
            [0.0, 0.0, 100.0],
            [0.0; 3],
            (0, 0, 800, 600),
        );
        let bench = registry.workbench_mut(&id).unwrap();
        bench.edit_feature(&mut ctx, feature);
        assert_eq!(bench.task(&ctx).map(|t| t.title), Some("Pocket".into()));
    }
    finish_jobs(&mut registry, &mut document, false);
    let path = toolpath(&mut registry, &mut document);
    assert_eq!(path["state"], "ready", "{path}");
    let levels: Vec<f64> = serde_json::from_value(path["levels"].clone()).unwrap();
    assert_eq!(levels.len(), 3, "5 mm in passes of at most 2");
    assert!(
        (levels[2] - 5.0).abs() < 1e-9,
        "down to the floor: {levels:?}"
    );
    let runs: Vec<Vec<[f64; 2]>> = serde_json::from_value(path["runs"].clone()).unwrap();
    let points: Vec<[f64; 2]> = runs.into_iter().flatten().collect();
    // Every point the tool's centre reaches keeps the tool inside the
    // square: the square less the tool's radius.
    let (low, high) = ([100.0 + radius, radius], [140.0 - radius, 40.0 - radius]);
    for p in &points {
        for axis in 0..2 {
            assert!(
                p[axis] >= low[axis] - 1e-6 && p[axis] <= high[axis] + 1e-6,
                "{p:?} leaves the square less the tool"
            );
        }
    }
    // And it reaches every side of it, in rows no further apart than the
    // stepover.
    let reach = |axis: usize, f: fn(f64, f64) -> f64, from: f64| {
        points.iter().map(|p| p[axis]).fold(from, f)
    };
    assert!(reach(0, f64::min, f64::MAX) - low[0] < 1e-3);
    assert!(high[0] - reach(0, f64::max, f64::MIN) < 1e-3);
    assert!(reach(1, f64::min, f64::MAX) - low[1] < 1e-3);
    assert!(high[1] - reach(1, f64::max, f64::MIN) < 1e-3);
    let mut rows: Vec<f64> = points.iter().map(|p| p[1]).collect();
    rows.sort_by(f64::total_cmp);
    rows.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    assert!(rows.windows(2).all(|w| w[1] - w[0] <= stepover + 1e-9));

    // The G-code: millimetres, cuts inside the square less the tool at
    // the passes' depths, rapids only above the stock.
    let program = run(
        &mut registry,
        &mut document,
        "example.cam",
        "example.cam.gcode",
        json!({"id": feature.0.to_string()}),
    )
    .unwrap();
    let program = program.as_str().unwrap();
    assert!(program.contains("G21"), "millimetres");
    assert!(program.contains("M3") && program.contains("M30"));
    let (top, safe) = (10.0, 15.0);
    let mut depths = Vec::new();
    for (rapid, [x, y, z]) in moves(program) {
        if rapid {
            assert!(
                (z - safe).abs() < 1e-9,
                "a rapid at Z{z}, not the clearance"
            );
            continue;
        }
        assert!(z < top, "a feed move cuts");
        assert!(x >= low[0] - 1e-3 && x <= high[0] + 1e-3, "X{x}");
        assert!(y >= low[1] - 1e-3 && y <= high[1] + 1e-3, "Y{y}");
        if !depths.contains(&z) {
            depths.push(z);
        }
    }
    depths.sort_by(f64::total_cmp);
    assert_eq!(depths.len(), 3, "{depths:?}");
    assert_eq!(depths[0], 5.0, "the floor");
    assert!(top - depths[2] <= step_down + 1e-3);
    assert!(depths.windows(2).all(|w| w[1] - w[0] <= step_down + 1e-3));
}

#[test]
fn a_pocket_job_stops_when_its_task_is_cancelled() {
    let package = installed("examples/cam", "cam.wasm", "cam-cancel");
    let mut registry = registry_with(&package, Capabilities::default());
    let id = WorkbenchId::new("example.cam");
    let mut document = Document::new("cam");
    // A round pocket of many edges in very fine rows: long work.
    let circle: Vec<[f64; 2]> = (0..4000)
        .map(|i| {
            let a = std::f64::consts::TAU * i as f64 / 4000.0;
            [1000.0 * a.cos(), 1000.0 * a.sin()]
        })
        .collect();
    let made = run(
        &mut registry,
        &mut document,
        "example.cam",
        "example.cam.pocket",
        json!({"outline": [circle], "depth": 1.0, "tool_diameter": 6.0, "stepover": 0.0101}),
    )
    .unwrap();
    let feature = feature_of(&made);
    let bench = registry.workbench_mut(&id).unwrap();
    let mut ctx =
        WorkbenchRuntimeContext::new(&mut document, [0.0, 0.0, 100.0], [0.0; 3], (0, 0, 800, 600));
    bench.edit_feature(&mut ctx, feature);
    std::thread::sleep(Duration::from_millis(200));
    assert!(bench.busy(), "still working");
    let escape = core_document::WorkbenchInputEvent::KeyPress {
        key: core_document::runtime::KeyCode::Escape,
    };
    bench.on_input(&escape, None, &mut ctx);
    assert!(bench.task(&ctx).is_none(), "Escape closed the task");
    drop(ctx);
    let stopped = Instant::now();
    let mut logs = finish_jobs(&mut registry, &mut document, false);
    // Uncancelled, the job runs for minutes; a stop is seen at its next
    // row, which a loaded machine can still take seconds to reach.
    assert!(
        stopped.elapsed() < Duration::from_secs(30),
        "it stopped long before it would have finished"
    );
    let state = |registry: &mut DocumentService, document: &mut Document| {
        run(
            registry,
            document,
            "example.cam",
            "example.cam.toolpath",
            json!({"id": feature.0.to_string()}),
        )
        .unwrap()["state"]
            .clone()
    };
    let deadline = Instant::now() + Duration::from_secs(30);
    while state(&mut registry, &mut document) == "running" && Instant::now() < deadline {
        logs.extend(finish_jobs(&mut registry, &mut document, false));
        std::thread::sleep(Duration::from_millis(10));
    }
    let now = state(&mut registry, &mut document);
    // On a loaded machine a call can run past its frame budget, which
    // starts the package afresh with nothing running: the job ended
    // either way, and never as a finished toolpath.
    assert!(now == "stopped" || now == "none", "{now} {logs:?}");
    if now == "stopped" {
        assert!(logs.iter().any(|l| l.contains("stopped")), "{logs:?}");
    }
}

#[test]
fn a_formula_drives_a_pocket_s_tool() {
    let package = installed("examples/cam", "cam.wasm", "cam-formula");
    let mut registry = registry_with(&package, Capabilities::default());
    let mut document = Document::new("cam");
    let square = json!([[[0.0, 0.0], [40.0, 0.0], [40.0, 40.0], [0.0, 40.0]]]);
    let made = run(
        &mut registry,
        &mut document,
        "example.cam",
        "example.cam.pocket",
        json!({"outline": square, "depth": 3.0, "tool_diameter": 6.0, "stepover": 2.0}),
    )
    .unwrap();
    let feature = feature_of(&made);
    let nearest = |registry: &mut DocumentService, document: &mut Document| {
        let program = run(
            registry,
            document,
            "example.cam",
            "example.cam.gcode",
            json!({"id": feature.0.to_string()}),
        )
        .unwrap();
        moves(program.as_str().unwrap())
            .into_iter()
            .filter(|(rapid, _)| !rapid)
            .map(|(_, p)| p[0].min(p[1]).min(40.0 - p[0]).min(40.0 - p[1]))
            .fold(f64::MAX, f64::min)
    };
    assert!((nearest(&mut registry, &mut document) - 3.0).abs() < 1e-3);

    document
        .set_feature_formula(feature, "/tool_diameter", Some("2 * 5 mm".into()))
        .unwrap();
    registry.evaluate(&mut document);
    assert_eq!(
        document.feature_values(feature).unwrap()["tool_diameter"],
        json!(10.0)
    );
    assert!(
        (nearest(&mut registry, &mut document) - 5.0).abs() < 1e-3,
        "a 10 mm tool keeps 5 mm from the walls"
    );
}

#[test]
fn a_bench_that_overruns_traps_or_overgrows_costs_the_app_nothing() {
    let package = installed("tests/rogue", "rogue.wasm", "rogue-limits");
    let mut registry = registry_with(&package, Capabilities::default());
    assert!(
        registry
            .tools_for(&WorkbenchId::new("test.rogue"))
            .unwrap()
            .is_empty(),
        "a tool whose id is not the package's is left out"
    );
    let mut document = Document::new("rogue");
    let mut call = |command: &str| {
        run(
            &mut registry,
            &mut document,
            "test.rogue",
            command,
            json!({}),
        )
    };
    assert_eq!(call("test.rogue.count"), Ok(json!(1)));

    let started = Instant::now();
    assert!(
        call("test.rogue.spin").is_err(),
        "an endless loop is stopped"
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "within its budget"
    );
    assert_eq!(
        call("test.rogue.count"),
        Ok(json!(1)),
        "a fresh instance answers"
    );

    assert!(call("test.rogue.panic").is_err());
    assert_eq!(call("test.rogue.count"), Ok(json!(1)));

    assert!(
        call("test.rogue.grow").is_err(),
        "memory stops at the package's cap"
    );
    assert!(
        call("test.rogue.count").is_err(),
        "three failures turn the bench off"
    );
}

#[test]
fn a_bench_reaches_its_own_folder_and_nothing_else() {
    let package = installed("tests/rogue", "rogue.wasm", "rogue-files");
    let mut registry = registry_with(&package, Capabilities::default());
    let mut document = Document::new("rogue");
    let answer = run(
        &mut registry,
        &mut document,
        "test.rogue",
        "test.rogue.files",
        json!({}),
    )
    .expect("files");
    assert_eq!(answer["back"], "kept");
    assert_eq!(
        answer["outside"], false,
        "the file system beyond its folder is not there"
    );
    assert_eq!(
        std::fs::read_to_string(package.data_dir().join("note.txt")).unwrap(),
        "kept"
    );
}

#[test]
fn a_bench_changes_only_its_own_kinds() {
    let package = installed("tests/rogue", "rogue.wasm", "rogue-kinds");
    let mut registry = registry_with(&package, Capabilities::default());
    let mut document = Document::new("rogue");
    let id = run(
        &mut registry,
        &mut document,
        "test.rogue",
        "test.rogue.add",
        json!({}),
    )
    .expect("its own kind");
    assert!(id.as_str().is_some());
    let refused = run(
        &mut registry,
        &mut document,
        "test.rogue",
        "test.rogue.outside",
        json!({}),
    )
    .unwrap_err();
    assert!(refused.contains("does not own"), "{refused}");
    assert_eq!(document.feature_tree().all_nodes().count(), 1);
}

#[test]
fn a_bench_removes_the_body_it_made_but_not_one_holding_anothers_feature() {
    let package = installed("tests/rogue", "rogue.wasm", "rogue-bodies");
    let mut registry = registry_with(&package, Capabilities::default());
    let mut document = Document::new("rogue");
    let made = run(
        &mut registry,
        &mut document,
        "test.rogue",
        "test.rogue.body",
        json!({}),
    )
    .expect("a body with a thing on it");
    let body = made["body"].as_str().unwrap().to_string();
    assert_eq!(document.bodies().len(), 1);
    let removal = |id: &str| json!({"command": "doc.remove_body", "args": {"id": id}});
    // Its own body, with its own feature on it, goes: what a cancelled
    // part leaves behind must not stay in the tree.
    run(
        &mut registry,
        &mut document,
        "test.rogue",
        "test.rogue.call",
        removal(&body),
    )
    .expect("its own body");
    assert!(document.bodies().is_empty());
    assert_eq!(document.feature_tree().all_nodes().count(), 0);
    let gone = run(
        &mut registry,
        &mut document,
        "test.rogue",
        "test.rogue.call",
        removal(&body),
    )
    .unwrap_err();
    assert!(gone.contains("no body"), "{gone}");

    // A body holding another workbench's feature is not the package's to
    // remove.
    let other = document.create_body(Some("Other".into()));
    document.add_feature_of_kind(
        WorkbenchId::new("wb.design"),
        "Pad".into(),
        Some(other),
        Vec::new(),
        json!({}),
        core_document::FeatureOrigin::default(),
    );
    let refused = run(
        &mut registry,
        &mut document,
        "test.rogue",
        "test.rogue.call",
        removal(&other.0.to_string()),
    )
    .unwrap_err();
    assert!(refused.contains("does not own"), "{refused}");
    assert_eq!(document.bodies().len(), 1);

    // Nor is an imported part, which holds no feature at all.
    let imported = document.create_body(Some("Imported".into()));
    let asset = document.add_asset_with_data(
        core_document::AssetReference::new(
            "assets/part.step",
            core_document::AssetType::Step,
            json!({}),
        ),
        b"ISO-10303-21;".to_vec(),
    );
    document.set_imported_geometry(
        imported,
        core_document::ImportedGeometry {
            mesh: Default::default(),
            source_asset: Some(asset),
            revision: 0,
            bounds_mm: None,
            brep_blob_path: None,
            face_colors_path: None,
            health: None,
        },
    );
    let refused = run(
        &mut registry,
        &mut document,
        "test.rogue",
        "test.rogue.call",
        removal(&imported.0.to_string()),
    )
    .unwrap_err();
    assert!(refused.contains("an import"), "{refused}");
    assert_eq!(document.bodies().len(), 2);
}

#[test]
fn a_job_runs_away_from_the_window_and_reports_when_the_bench_is_active() {
    let package = installed("tests/rogue", "rogue.wasm", "rogue-job");
    let mut registry = registry_with(&package, Capabilities::default());
    let mut document = Document::new("rogue");
    let job = run(
        &mut registry,
        &mut document,
        "test.rogue",
        "test.rogue.job",
        json!({"steps": 300}),
    )
    .expect("starts");
    assert_eq!(job, json!(1));
    let bench = registry
        .workbench_mut(&WorkbenchId::new("test.rogue"))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    while bench.busy() {
        assert!(Instant::now() < deadline, "the job ends");
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut ctx =
        WorkbenchRuntimeContext::new(&mut document, [0.0, 0.0, 100.0], [0.0; 3], (0, 0, 800, 600));
    bench.on_activate(&mut ctx);
    bench.on_frame(0.016, &mut ctx);
    let logs: Vec<String> = ctx.drain_logs().into_iter().map(|l| l.message).collect();
    assert!(
        logs.iter().any(|l| l.starts_with("job 1: Ok(")),
        "the bench is told how its job ended: {logs:?}"
    );
}

/// Run the rogue's helper job and answer how it ended, as the bench was
/// told. The helper is a shell script, so this runs where one does.
#[cfg(unix)]
fn helper_job(granted: Capabilities, name: &str) -> String {
    let package = installed("tests/rogue", "rogue.wasm", name);
    let helpers = package.dir.join("helpers").join(format!(
        "{}-{}",
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    std::fs::create_dir_all(&helpers).unwrap();
    let script = helpers.join("upper");
    std::fs::write(&script, "#!/bin/sh\ntr a-z A-Z\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mut registry = registry_with(&package, granted);
    let mut document = Document::new("rogue");
    run(
        &mut registry,
        &mut document,
        "test.rogue",
        "test.rogue.helper",
        json!({}),
    )
    .unwrap();
    let bench = registry
        .workbench_mut(&WorkbenchId::new("test.rogue"))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    while bench.busy() {
        assert!(Instant::now() < deadline, "the job ends");
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut ctx =
        WorkbenchRuntimeContext::new(&mut document, [0.0, 0.0, 100.0], [0.0; 3], (0, 0, 800, 600));
    bench.on_activate(&mut ctx);
    bench.on_frame(0.016, &mut ctx);
    ctx.drain_logs()
        .into_iter()
        .map(|l| l.message)
        .find(|m| m.starts_with("job "))
        .expect("the bench heard of its job")
}

#[cfg(unix)]
#[test]
fn a_helper_runs_only_when_the_user_allowed_it() {
    let allowed = helper_job(
        Capabilities {
            helper: true,
            ..Default::default()
        },
        "rogue-helper-on",
    );
    assert_eq!(allowed, "job 1: Ok(\"HELLO\")");
    let refused = helper_job(Capabilities::default(), "rogue-helper-off");
    assert!(refused.contains("not been allowed"), "{refused}");
}

#[test]
fn a_package_writing_a_feature_stamps_its_own_version_on_it() {
    let package = installed("tests/rogue", "rogue.wasm", "rogue-stamp");
    let mut registry = registry_with(&package, Capabilities::default());
    let mut document = Document::new("rogue");
    let id = run(
        &mut registry,
        &mut document,
        "test.rogue",
        "test.rogue.add",
        json!({}),
    )
    .unwrap();
    let feature = core_document::FeatureId(id.as_str().unwrap().parse().unwrap());
    // As an older version of the package left it.
    document
        .set_feature_origin(
            feature,
            core_document::FeatureOrigin::new("test.rogue 0.0.9", None),
        )
        .unwrap();
    run(
        &mut registry,
        &mut document,
        "test.rogue",
        "test.rogue.touch",
        json!({ "id": id }),
    )
    .unwrap();
    let node = document.get_feature_meta(feature).unwrap();
    assert_eq!(node.data, json!({"touched": true}));
    assert_eq!(
        node.made_by.as_deref(),
        Some("test.rogue 0.1.0"),
        "the writer's version"
    );
    assert_eq!(node.package_source, None, "installed from a file");
}

#[test]
fn a_feature_whose_package_is_missing_names_it_and_keeps_its_data() {
    let package = installed("tests/rogue", "rogue.wasm", "rogue-missing");
    let mut registry = registry_with(&package, Capabilities::default());
    let mut document = Document::new("rogue");
    run(
        &mut registry,
        &mut document,
        "test.rogue",
        "test.rogue.add",
        json!({}),
    )
    .unwrap();
    let node = document
        .feature_tree()
        .all_nodes()
        .next()
        .unwrap()
        .1
        .clone();

    let without = DocumentService::default();
    assert!(without.feature_info(&node).is_none());
    assert_eq!(
        FeatureInfo::unowned(&node).family_label,
        "Needs test.rogue 0.1.0"
    );
    assert!(
        FeatureInfo::missing_package(&node)
            .unwrap()
            .contains("not installed")
    );
}

#[test]
fn reinstalling_keeps_the_data_folder_and_uninstalling_removes_it() {
    let package = installed("tests/rogue", "rogue.wasm", "rogue-reinstall");
    let root = package.dir.parent().unwrap().to_path_buf();
    std::fs::create_dir_all(package.data_dir()).unwrap();
    std::fs::write(package.data_dir().join("keep.txt"), "mine").unwrap();
    let archive = root.parent().unwrap().join("package.pcbench");
    let again = wb_wasm::install(&archive, &root).expect("reinstalls");
    assert_eq!(
        std::fs::read_to_string(again.data_dir().join("keep.txt")).unwrap(),
        "mine"
    );
    assert_eq!(wb_wasm::discover(&root).len(), 1);
    wb_wasm::uninstall(&root, "test.rogue").unwrap();
    assert!(wb_wasm::discover(&root).is_empty());
}

/// GitHub as the tests see it: one repository whose latest release holds
/// `asset` with `bytes`, and a record of what was asked.
struct FakeGithub {
    tag: std::cell::RefCell<String>,
    bytes: std::cell::RefCell<Vec<u8>>,
    digest: std::cell::RefCell<Option<String>>,
}

impl FakeGithub {
    fn publish(&self, tag: &str, bytes: Vec<u8>, digest: Option<String>) {
        *self.tag.borrow_mut() = tag.into();
        *self.bytes.borrow_mut() = bytes;
        *self.digest.borrow_mut() = digest;
    }
}

impl wb_wasm::remote::Fetch for FakeGithub {
    fn json(&self, url: &str) -> Result<Value, String> {
        let tag = self.tag.borrow().clone();
        let known = url == "https://api.github.com/repos/acme/gears/releases/latest"
            || url == format!("https://api.github.com/repos/acme/gears/releases/tags/{tag}");
        if !known {
            return Err(format!("{url} was not found"));
        }
        Ok(json!({
            "tag_name": tag,
            "assets": [
                {"name": "notes.txt", "browser_download_url": "https://example.invalid/notes"},
                {
                    "name": "gears.pcbench",
                    "browser_download_url": format!("https://example.invalid/{tag}/gears.pcbench"),
                    "digest": *self.digest.borrow(),
                }
            ]
        }))
    }

    fn bytes(&self, url: &str, _limit: u64) -> Result<Vec<u8>, String> {
        assert!(url.ends_with("/gears.pcbench"), "{url}");
        Ok(self.bytes.borrow().clone())
    }
}

fn sha256(bytes: &[u8]) -> String {
    use sha2::Digest;
    let hex: String = sha2::Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("sha256:{hex}")
}

#[test]
fn a_package_installs_from_a_github_release_and_takes_its_updates() {
    use wb_wasm::remote;
    let gear = std::fs::read(archive("examples/gear", "gear.wasm", "github-gear")).unwrap();
    let rogue = std::fs::read(archive("tests/rogue", "rogue.wasm", "github-rogue")).unwrap();
    let root = scratch("github-root");
    let github = FakeGithub {
        tag: Default::default(),
        bytes: Default::default(),
        digest: Default::default(),
    };
    github.publish("v0.1.0", gear.clone(), Some(sha256(&gear)));

    let package = remote::install_from_github(&github, "https://github.com/acme/gears", &root)
        .expect("installs from the release");
    assert_eq!(package.manifest.id, "example.gear");
    let source = remote::source_of(&package).unwrap();
    assert_eq!(
        (source.repo.as_str(), source.tag.as_str()),
        ("acme/gears", "v0.1.0")
    );
    assert_eq!(remote::check(&github, &package), Ok(None), "up to date");
    std::fs::create_dir_all(package.data_dir()).unwrap();
    std::fs::write(package.data_dir().join("tools.json"), "[]").unwrap();

    // A release whose bytes do not match its checksum is refused.
    github.publish("v0.2.0", gear.clone(), Some(sha256(b"something else")));
    let release = remote::check(&github, &package)
        .unwrap()
        .expect("a newer release");
    assert_eq!(release.tag, "v0.2.0");
    let refused = remote::update(&github, &package, &release).unwrap_err();
    assert!(refused.contains("checksum"), "{refused}");

    // A repository that starts shipping another package is refused.
    github.publish("v0.3.0", rogue.clone(), None);
    let release = remote::check(&github, &package).unwrap().unwrap();
    let refused = remote::update(&github, &package, &release).unwrap_err();
    assert!(refused.contains("not example.gear"), "{refused}");
    assert_eq!(wb_wasm::discover(&root).len(), 1, "nothing was replaced");

    github.publish("v0.4.0", gear.clone(), Some(sha256(&gear)));
    let release = remote::check(&github, &package).unwrap().unwrap();
    let updated = remote::update(&github, &package, &release).expect("updates");
    assert_eq!(remote::source_of(&updated).unwrap().tag, "v0.4.0");
    assert_eq!(
        std::fs::read_to_string(updated.data_dir().join("tools.json")).unwrap(),
        "[]",
        "the package's data stays"
    );
    assert_eq!(remote::check(&github, &updated), Ok(None));

    // The features it makes say where to get it.
    let mut registry = registry_with(&updated, Capabilities::default());
    let mut document = Document::new("gears");
    let made = run(
        &mut registry,
        &mut document,
        "example.gear",
        "example.gear.make",
        json!({"teeth": 12, "module": 1.0}),
    )
    .unwrap();
    let feature = core_document::FeatureId(made["feature"].as_str().unwrap().parse().unwrap());
    let node = document.get_feature_meta(feature).unwrap();
    assert_eq!(node.package_source.as_deref(), Some("acme/gears"));
    assert_eq!(
        FeatureInfo::needed_package(node),
        Some(("example.gear".into(), Some("acme/gears".into())))
    );
    assert!(
        FeatureInfo::missing_package(node)
            .unwrap()
            .contains("github.com/acme/gears")
    );

    // A repository with no such release says so.
    let missing = remote::install_from_github(&github, "acme/other", &root).unwrap_err();
    assert!(missing.contains("no published release"), "{missing}");
}

#[test]
fn a_running_package_is_replaced_in_place_and_rebuilds_what_it_owns() {
    let package = installed("examples/gear", "gear.wasm", "gear-reload");
    let mut registry = registry_with(&package, Capabilities::default());
    let id = WorkbenchId::new("example.gear");
    let mut document = Document::new("gears");
    run(
        &mut registry,
        &mut document,
        "example.gear",
        "example.gear.make",
        json!({"teeth": 12, "module": 2.0}),
    )
    .unwrap();
    assert_eq!(registry.rebuild_jobs(&mut document).len(), 1);

    // Taken out: its gear stays in the document, owned by nothing.
    let old = registry.unregister_workbench(&id).expect("was registered");
    drop(old);
    let node = document
        .feature_tree()
        .all_nodes()
        .next()
        .unwrap()
        .1
        .clone();
    assert!(registry.feature_info(&node).is_none());
    assert!(registry.command("example.gear.make").is_none());
    assert!(registry.rebuild_jobs(&mut document).is_empty());

    // A fresh load takes its place, and rebuilds the gear it finds.
    let bench = wb_wasm::load(&package, &Capabilities::default()).expect("loads again");
    registry
        .register_workbench(Box::new(bench))
        .expect("registers again");
    assert!(registry.command("example.gear.make").is_some());
    registry
        .workbench(&id)
        .unwrap()
        .invalidate_all(&mut document);
    let jobs = registry.rebuild_jobs(&mut document);
    assert_eq!(jobs.len(), 1, "the new instance plans the gear");
    assert!(jobs[0].plan.is_ok());
}
