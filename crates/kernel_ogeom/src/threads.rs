//! The kernel's parallel stages on a browser page's threads: web workers
//! sharing the page instance's memory, run as rayon's pool
//! (`wasm-bindgen-rayon` starts them). Only a build with atomics has them.

/// rayon's global pool, lent to the kernel's parallel stages.
struct Rayon;

impl ogeom::core::parallel::Pool for Rayon {
    fn workers(&self) -> usize {
        rayon::current_num_threads()
    }

    fn broadcast(&self, copies: usize, job: &(dyn Fn() + Sync)) {
        rayon::scope(|scope| {
            for _ in 0..copies {
                scope.spawn(|_| job());
            }
        });
    }
}

/// Lend the kernel rayon's pool, once it has threads; how many it has.
pub fn lend_rayon() -> usize {
    let workers = rayon::current_num_threads();
    if workers > 1 {
        ogeom::core::parallel::set_pool(&Rayon);
    }
    workers
}
