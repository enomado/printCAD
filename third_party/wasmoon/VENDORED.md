# wasmoon 1.16.0

Lua 5.4 compiled to WebAssembly, from the npm package `wasmoon` 1.16.0
(MIT, `LICENSE` here): `dist/index.js` and `dist/glue.wasm`, unchanged.

The browser build's console runs Lua through it in a worker of the page's
own (`web/lua-worker.js`, `crates/scripting/src/thread_web.rs`);
`scripts/build-web.sh` copies both files beside the app. A desktop build
does not use it: its console is mlua.
