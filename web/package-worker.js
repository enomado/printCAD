// Workbench packages away from the page (crates/workbenches/wb_wasm/src/
// web.rs is the page's side). A worker transpiles a package's component
// into a module the page runs (jco), and a worker of its own runs each of
// a package's jobs, ended to stop it. Messages are objects; bytes cross as
// typed arrays.
import { transpile } from "./jco/transpile.js";
import { wasiImports } from "./jco/wasi.js";

// The module and core modules of transpiled `files`.
async function load(files, name) {
  const table = new Map(files);
  const text = new TextDecoder().decode(table.get(`${name}.js`));
  const url = URL.createObjectURL(new Blob([text], { type: "text/javascript" }));
  const module = await import(url);
  URL.revokeObjectURL(url);
  const cores = new Map();
  for (const [file, data] of files) {
    if (file.endsWith(".wasm")) cores.set(file, await WebAssembly.compile(data));
  }
  return { module, cores };
}

// What a job reaches: no document, its progress, a log line.
function jobHost(post) {
  const refused = (command) => {
    throw command === "job.start"
      ? "a job cannot start another job"
      : `\`${command}\` changes the document, which this call may only read`;
  };
  return {
    log: (level, message) => post({ type: "log", level, message }),
    feature: () => undefined,
    features: () => "[]",
    bodies: () => "[]",
    bodyMesh: () => undefined,
    bodyShape: () => undefined,
    profile: () => undefined,
    call: refused,
    request: () => {},
    redraw: () => {},
    progress: (done, total) => post({ type: "progress", done: Number(done), total: Number(total) }),
    cancelled: () => false,
    helper: () => {
      throw "helpers run in the desktop application";
    },
  };
}

onmessage = async (event) => {
  const m = event.data;
  if (m.type === "transpile") {
    try {
      const files = await transpile(m.bytes, m.name);
      postMessage({ type: "transpiled", id: m.id, files });
    } catch (error) {
      postMessage({ type: "transpiled", id: m.id, error: String(error?.message ?? error) });
    }
  } else if (m.type === "job") {
    try {
      const { module, cores } = await load(m.files, m.name);
      const imports = { ...wasiImports(), "printcad:workbench/host": jobHost(postMessage) };
      const instance = await module.instantiate((name) => cores.get(name), imports);
      const bench = instance.bench ?? instance["printcad:workbench/bench@0.1.0"];
      try {
        postMessage({ type: "done", ok: bench.jobRun(m.entry, m.input) });
      } catch (error) {
        const failed = Object.prototype.hasOwnProperty.call(error ?? {}, "payload");
        postMessage({ type: "done", err: failed ? String(error.payload) : String(error?.message ?? error) });
      }
    } catch (error) {
      postMessage({ type: "done", err: String(error?.message ?? error) });
    }
  }
};
