//! Workbenches from packages. A package is a WebAssembly component
//! speaking `printcad:workbench` (`bench_api`), a manifest and its icons;
//! [`load`] turns one into a [`WasmWorkbench`], which the registry takes
//! like any built-in bench. See `docs/PLUGINS.md`.

#[cfg(feature = "runtime")]
mod bench;
#[cfg(feature = "runtime")]
mod convert;
#[cfg(feature = "runtime")]
mod engine;
#[cfg(feature = "runtime")]
mod guest;
#[cfg(feature = "runtime")]
mod host;
#[cfg(feature = "runtime")]
mod jobs;
pub mod package;
pub mod remote;
pub mod store;

#[cfg(feature = "runtime")]
pub use bench::{WasmWorkbench, load};
pub use bench_api::{Capabilities, Manifest};
#[cfg(feature = "runtime")]
pub use host::{ProfileSource, set_profile_source};
pub use package::{Package, discover, install, pack, uninstall};
