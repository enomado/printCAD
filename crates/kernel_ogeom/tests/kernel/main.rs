//! The kernel adapter's integration tests, one program: each module a part of what the adapter does, run against the real kernel.
//!
//! One program rather than one per file: each would link the whole crate
//! and its dependencies again, which costs far more than running the tests.

mod annotations;
mod borrowed_geometry;
mod chain_cache;
mod datum_attachments;
mod design_stack;
mod dxf_import;
mod export;
mod extrude;
mod generated_profiles;
mod imported_base;
mod kernel_fillet_closed_edges;
mod kernel_orientation;
mod kernel_revolution_booleans;
mod kernel_scaled_revolution;
mod mesh_import;
mod naming;
mod print_checks;
mod probes;
mod progress;
mod queries;
mod scripted_part;
mod shape_health;
mod solid_ops;
mod step_import;
mod surface_ops;
mod surface_stack;
