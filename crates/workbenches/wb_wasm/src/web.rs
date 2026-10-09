//! Packages on a browser page. A worker (`web/package-worker.js`)
//! transpiles a package's component into a module with jco; its instance
//! runs on the page, so the host calls it as wasmtime's: synchronously,
//! with the guest's imports answered by [`Reach`] for the length of each
//! call. A trap replaces the instance as on a desktop; a page has no clock
//! to stop a call that runs too long, so a budget is not enforced here.
//! Jobs run in workers of their own (`jobs`).

pub(crate) mod jobs;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use js_sys::{Array, Object, Reflect, Uint8Array};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use crate::bench::WasmWorkbench;
use crate::exports::{Aftermath, Answer, Budget, Exports, Fault, STRIKES};
use crate::host::{Access, PackageInfo, Reach};
use crate::package::Package;
use jobs::JobBoard;

/// The name jco gives the transpiled files.
const NAME: &str = "bench";

#[wasm_bindgen(inline_js = r#"
let worker = null;
let next = 0;
const waiting = new Map();

function packageWorker() {
  if (!worker) {
    worker = new Worker("package-worker.js", { type: "module" });
    worker.onmessage = (event) => {
      const m = event.data;
      const asked = waiting.get(m.id);
      waiting.delete(m.id);
      if (m.error === undefined) asked?.resolve(m.files);
      else asked?.reject(new Error(m.error));
    };
  }
  return worker;
}

export async function pkg_prepare(bytes, name) {
  const files = await new Promise((resolve, reject) => {
    const id = next++;
    waiting.set(id, { resolve, reject });
    packageWorker().postMessage({ type: "transpile", id, bytes, name });
  });
  const table = new Map(files);
  const text = new TextDecoder().decode(table.get(`${name}.js`));
  const url = URL.createObjectURL(new Blob([text], { type: "text/javascript" }));
  const module = await import(url);
  URL.revokeObjectURL(url);
  const cores = new Map();
  for (const [file, data] of files) {
    if (file.endsWith(".wasm")) cores.set(file, await WebAssembly.compile(data));
  }
  return { module, cores, files, name };
}

export async function pkg_instantiate(prepared, host) {
  const wasi = await import(new URL("jco/wasi.js", document.baseURI).href);
  const imports = { ...wasi.wasiImports(), "printcad:workbench/host": host };
  const instance = await prepared.module.instantiate((name) => prepared.cores.get(name), imports);
  return instance.bench ?? instance["printcad:workbench/bench@0.1.0"];
}

export function pkg_call(bench, name, args) {
  try {
    return { ok: bench[name](...args) };
  } catch (error) {
    if (error && Object.prototype.hasOwnProperty.call(error, "payload")) {
      return { err: String(error.payload) };
    }
    return { trap: String(error?.message ?? error) };
  }
}
"#)]
extern "C" {
    fn pkg_prepare(bytes: &Uint8Array, name: &str) -> js_sys::Promise;
    fn pkg_instantiate(prepared: &JsValue, host: &JsValue) -> js_sys::Promise;
    fn pkg_call(bench: &JsValue, name: &str, args: &Array) -> JsValue;
}

fn js_error(error: JsValue) -> String {
    error
        .dyn_ref::<js_sys::Error>()
        .map(|e| String::from(e.message()))
        .or_else(|| error.as_string())
        .unwrap_or_else(|| format!("{error:?}"))
}

/// Load `package`, allowing it `granted` of what it asks for, and hand
/// `done` its workbench once the page has transpiled and started it.
pub fn load(
    package: Package,
    granted: bench_api::Capabilities,
    done: impl FnOnce(Result<WasmWorkbench, String>) + 'static,
) {
    wasm_bindgen_futures::spawn_local(async move {
        done(start(&package, &granted).await);
    });
}

async fn start(
    package: &Package,
    granted: &bench_api::Capabilities,
) -> Result<WasmWorkbench, String> {
    let bytes = package.component()?;
    let prepared = JsFuture::from(pkg_prepare(&Uint8Array::from(&bytes[..]), NAME))
        .await
        .map_err(|e| format!("{} did not transpile: {}", package.manifest.id, js_error(e)))?;
    let info = Arc::new(crate::bench::package_info(package, granted));
    let jobs = JobBoard::new(prepared.clone(), info.clone());
    let reach = Rc::new(RefCell::new(Reach::new(info.clone(), jobs.clone())));
    let host = host_imports(&reach);
    let bench = JsFuture::from(pkg_instantiate(&prepared, &host))
        .await
        .map_err(|e| format!("{} did not start: {}", package.manifest.id, js_error(e)))?;
    let guest = Guest {
        jobs,
        settings: None,
        package: info.clone(),
        reach,
        host,
        prepared,
        bench: Rc::new(RefCell::new(Some(bench))),
        strikes: 0,
    };
    crate::bench::assemble(package, info, guest)
}

/// The guest's `host` imports, each answered by `reach`.
fn host_imports(reach: &Rc<RefCell<Reach>>) -> JsValue {
    let imports = Object::new();
    let set = |name: &str, f: JsValue| {
        let _ = Reflect::set(&imports, &name.into(), &f);
    };
    macro_rules! import {
        ($name:literal, $r:ident => |$($arg:ident: $ty:ty),*| $(-> $ret:ty)? $body:block) => {{
            let shared = reach.clone();
            set(
                $name,
                Closure::<dyn Fn($($ty),*) $(-> $ret)?>::new(move |$($arg: $ty),*| {
                    let $r = &mut *shared.borrow_mut();
                    $body
                })
                .into_js_value(),
            );
        }};
    }
    import!("log", r => |level: String, message: String| { r.log(level, message) });
    import!("feature", r => |id: String| -> Option<String> { r.feature(id) });
    import!("features", r => | | -> String { r.features() });
    import!("bodies", r => | | -> String { r.bodies() });
    import!("bodyMesh", r => |body: String| -> JsValue {
        match r.body_mesh(body) {
            Some((positions, indices)) => {
                let mesh = Object::new();
                let _ = Reflect::set(
                    &mesh,
                    &"positions".into(),
                    &js_sys::Float32Array::from(&positions[..]),
                );
                let _ = Reflect::set(
                    &mesh,
                    &"indices".into(),
                    &js_sys::Uint32Array::from(&indices[..]),
                );
                mesh.into()
            }
            None => JsValue::UNDEFINED,
        }
    });
    import!("bodyShape", r => |body: String| -> Option<Vec<u8>> { r.body_shape(body) });
    import!("profile", r => |feature: String| -> Option<String> { r.profile(feature) });
    import!("call", r => |command: String, args: String| -> Result<String, JsValue> {
        r.call(command, args).map_err(JsValue::from)
    });
    import!("request", r => |request: String| { r.request(request) });
    import!("redraw", r => | | { r.redraw() });
    import!("progress", r => |done: u64, total: u64| { r.progress(done, total) });
    import!("cancelled", r => | | -> bool { r.cancelled() });
    import!("helper", r => |name: String, input: Vec<u8>| -> Result<Vec<u8>, JsValue> {
        r.helper(name, input).map_err(JsValue::from)
    });
    imports.into()
}

/// The bench's own instance on the page.
pub(crate) struct Guest {
    pub jobs: Arc<JobBoard>,
    /// Settings last given, put back into a fresh instance.
    pub settings: Option<String>,
    package: Arc<PackageInfo>,
    reach: Rc<RefCell<Reach>>,
    host: JsValue,
    prepared: JsValue,
    /// The instance's exports; `None` while a fresh one starts.
    bench: Rc<RefCell<Option<JsValue>>>,
    strikes: u32,
}

// SAFETY: a browser page runs this build on one thread (the kernel's
// workers are instances of their own, sharing no memory), so nothing here
// is ever reached from another.
unsafe impl Send for Guest {}

impl Guest {
    /// Whether the guest misbehaved often enough to be turned off.
    pub(crate) fn disabled(&self) -> bool {
        self.strikes >= STRIKES
    }

    /// Call into the guest. `None` when it is turned off, starting again
    /// or trapped; the reason is logged and a trapped instance replaced.
    pub(crate) fn call<R>(
        &mut self,
        _budget: Budget,
        access: Access,
        f: impl FnOnce(&mut dyn Exports) -> Answer<R>,
    ) -> Option<(R, Aftermath)> {
        if self.disabled() {
            return None;
        }
        let bench = self.bench.borrow().clone()?;
        self.reach.borrow_mut().access = access;
        let result = f(&mut Bound { bench: &bench });
        let aftermath = {
            let mut reach = self.reach.borrow_mut();
            reach.access = Access::None;
            Aftermath {
                requests: std::mem::take(&mut reach.requests),
                redraw: std::mem::take(&mut reach.redraw),
            }
        };
        match result {
            Ok(value) => Some((value, aftermath)),
            Err(fault) => {
                self.strike(&fault);
                None
            }
        }
    }

    fn strike(&mut self, fault: &Fault) {
        self.strikes += 1;
        let id = self.package.id.clone();
        let Fault::Trapped(error) = fault;
        let what = format!("stopped with an error: {error}");
        if self.disabled() {
            tracing::error!(
                target: "printcad.bench",
                package = %id,
                "the workbench {what}; turned off for this session after {} failures",
                self.strikes
            );
            return;
        }
        tracing::error!(target: "printcad.bench", package = %id, "the workbench {what}; restarting it");
        // Calls answer nothing until the fresh instance is in.
        self.bench.borrow_mut().take();
        let (slot, prepared, host) = (self.bench.clone(), self.prepared.clone(), self.host.clone());
        let settings = self.settings.clone();
        wasm_bindgen_futures::spawn_local(async move {
            match JsFuture::from(pkg_instantiate(&prepared, &host)).await {
                Ok(bench) => {
                    if let Some(settings) = settings {
                        let _ = Bound { bench: &bench }.apply_settings(&settings);
                    }
                    *slot.borrow_mut() = Some(bench);
                }
                Err(e) => tracing::error!(
                    target: "printcad.bench",
                    package = %id,
                    "cannot restart the workbench: {}",
                    js_error(e)
                ),
            }
        });
    }
}

/// An instance's exports, for one call.
struct Bound<'a> {
    bench: &'a JsValue,
}

impl Bound<'_> {
    /// Call export `name` (as jco names it) with `args`: its value, or the
    /// error text of one whose result is an error.
    fn call(&self, name: &str, args: &[JsValue]) -> Answer<Result<JsValue, String>> {
        let answer = pkg_call(self.bench, name, &args.iter().collect());
        let field = |key: &str| {
            Reflect::get(&answer, &key.into())
                .ok()
                .filter(|v| !v.is_undefined())
        };
        if let Some(trap) = field("trap") {
            return Err(Fault::Trapped(trap.as_string().unwrap_or_default()));
        }
        if let Some(err) = field("err") {
            return Ok(Err(err.as_string().unwrap_or_default()));
        }
        Ok(Ok(
            Reflect::get(&answer, &"ok".into()).unwrap_or(JsValue::UNDEFINED)
        ))
    }

    /// An export answering without an error result.
    fn value(&self, name: &str, args: &[JsValue]) -> Answer<JsValue> {
        self.call(name, args)?
            .map_err(|e| Fault::Trapped(format!("{name} answered an error: {e}")))
    }

    fn text(&self, name: &str, args: &[JsValue]) -> Answer<String> {
        let value = self.value(name, args)?;
        value
            .as_string()
            .ok_or_else(|| Fault::Trapped(format!("{name} answered no text")))
    }

    fn flag(&self, name: &str, args: &[JsValue]) -> Answer<bool> {
        Ok(self.value(name, args)?.as_bool().unwrap_or(false))
    }
}

impl Exports for Bound<'_> {
    fn describe(&mut self) -> Answer<String> {
        self.text("describe", &[])
    }
    fn feature_info(&mut self, node: &str) -> Answer<String> {
        self.text("featureInfo", &[node.into()])
    }
    fn parameters(&mut self, node: &str) -> Answer<String> {
        self.text("parameters", &[node.into()])
    }
    fn settle(&mut self, node: &str) -> Answer<String> {
        self.text("settle", &[node.into()])
    }
    fn rebuild(&mut self, request: &str) -> Answer<String> {
        self.text("rebuild", &[request.into()])
    }
    fn run_command(&mut self, id: &str, args: &str) -> Answer<Result<String, String>> {
        Ok(self
            .call("runCommand", &[id.into(), args.into()])?
            .map(|v| v.as_string().unwrap_or_default()))
    }
    fn input(&mut self, input: &str) -> Answer<bool> {
        self.flag("input", &[input.into()])
    }
    fn frame(&mut self, pointer: &str) -> Answer<String> {
        self.text("frame", &[pointer.into()])
    }
    fn panel_event(&mut self, slot: &str, event: &str) -> Answer<()> {
        self.value("panelEvent", &[slot.into(), event.into()])
            .map(drop)
    }
    fn task_close(&mut self, accept: bool) -> Answer<Option<String>> {
        Ok(self.value("taskClose", &[accept.into()])?.as_string())
    }
    fn menu_items(&mut self, scope: &str) -> Answer<String> {
        self.text("menuItems", &[scope.into()])
    }
    fn menu_command(&mut self, id: &str, scope: &str) -> Answer<bool> {
        self.flag("menuCommand", &[id.into(), scope.into()])
    }
    fn delete_feature(&mut self, id: &str) -> Answer<bool> {
        self.flag("deleteFeature", &[id.into()])
    }
    fn settings_panel(&mut self) -> Answer<String> {
        self.text("settingsPanel", &[])
    }
    fn settings(&mut self) -> Answer<Option<String>> {
        Ok(self.value("settings", &[])?.as_string())
    }
    fn apply_settings(&mut self, settings: &str) -> Answer<()> {
        self.value("applySettings", &[settings.into()]).map(drop)
    }
    fn suspend(&mut self) -> Answer<Option<Vec<u8>>> {
        let value = self.value("suspend", &[])?;
        Ok(value.dyn_ref::<Uint8Array>().map(Uint8Array::to_vec))
    }
    fn resume(&mut self, state: Option<&[u8]>) -> Answer<()> {
        let state = state.map_or(JsValue::UNDEFINED, |s| Uint8Array::from(s).into());
        self.value("resume", &[state]).map(drop)
    }
}
