// The console's Lua on a browser page: wasmoon (Lua 5.4) in a worker of
// its own, so a long script never holds the page. Each command a script
// calls goes to the page as a message and the script waits for the answer
// (crates/scripting/src/thread_web.rs is the page's side). Messages are
// JSON text.
importScripts("wasmoon/index.js");

let lua = null;
let printed = [];
let nextCall = 0;
const waiting = new Map();
const post = (message) => postMessage(JSON.stringify(message));

// Lua's error text without wasmoon's prefix or the stack traceback.
const message = (error) =>
  String(error?.message ?? error)
    .replace(/^Lua Error\([^)]*\):\s*/, "")
    .split("\nstack traceback:")[0];

onmessage = async (event) => {
  const m = JSON.parse(event.data);
  if (m.type === "init") {
    const factory = new wasmoon.LuaFactory("wasmoon/glue.wasm");
    lua = await factory.createEngine();
    lua.global.set("__pc_print", (line) => {
      const text = String(line);
      printed.push(text);
      post({ type: "print", text });
    });
    lua.global.set(
      "__pc_ask",
      (id, args) =>
        new Promise((resolve, reject) => {
          const call = nextCall++;
          waiting.set(call, { resolve, reject });
          post({ type: "call", call, id, args });
        }),
    );
    await lua.doString(m.webPrelude);
    await lua.doString(m.prelude);
    post({ type: "ready" });
  } else if (m.type === "reply") {
    const asked = waiting.get(m.call);
    waiting.delete(m.call);
    if (m.error === undefined) asked?.resolve(m.answer);
    else asked?.reject(m.error);
  } else if (m.type === "run") {
    printed = [];
    lua.global.set("__pc_source", m.source);
    lua.global.set("__pc_name", m.name);
    lua.global.set("__pc_line", m.line);
    try {
      await lua.doString("__pc_value, __pc_returned = __pc_run(__pc_source, __pc_name, __pc_line)");
      const value = lua.global.get("__pc_value");
      const returned = lua.global.get("__pc_returned");
      post({ type: "done", printed, value: value ?? null, returned: returned ?? null, error: null });
    } catch (error) {
      post({ type: "done", printed, value: null, returned: null, error: message(error) });
    }
  }
};
