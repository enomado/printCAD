# egui-winit 0.36.2, patched

The published crate with one change, in `src/dropped_file.rs`: on
`wasm32` its file type implements `bytes_async`, the method egui's
`DroppedFile` trait asks for in a browser, rather than `bytes`, which the
trait has only on other targets. Unpatched, the crate does not build for
`wasm32-unknown-unknown`. Native builds compile exactly the published code.

The workspace `Cargo.toml` points `[patch.crates-io]` here. Remove this
folder and that line once a release builds for the browser.
