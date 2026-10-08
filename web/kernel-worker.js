// A kernel worker: the same module as the page, serving only the kernel.
// The page sends it jobs once it says it is ready (kernel_pool.rs). Its
// address names the build the page runs (`threads`, built with shared
// memory, or `plain`) and how many threads the worker may use: a
// threaded build starts that many web workers as the kernel's pool.
const options = new URL(self.location.href).searchParams;
const threaded = options.get("build") === "threads";
const threads = Number(options.get("threads") ?? 1);
const printcad = await import(threaded ? "./threads/printcad.js" : "./printcad.js");
await printcad.default();
const pool = threaded && threads > 1 && typeof printcad.initThreadPool === "function";
if (pool) await printcad.initThreadPool(threads);
printcad.kernel_worker_main(pool ? threads : 1);
