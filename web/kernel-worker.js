// A kernel worker: the same module as the page, serving only the kernel.
// The page sends it jobs once it says it is ready (kernel_pool.rs).
import init, { kernel_worker_main } from "./printcad.js";

await init();
kernel_worker_main();
