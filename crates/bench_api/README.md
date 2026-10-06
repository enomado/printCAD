# printcad-bench-api

What a printCAD workbench package and the app exchange: the manifest, the
registration (tools, commands, actions), document nodes, rebuild plans,
input, the frame a bench draws, declared panel widgets and their events,
menus and requests. The library is named `bench_api`.

The component interface is the WIT world in `wit/workbench.wit`
(`printcad:workbench@0.1`, also `bench_api::WIT`); its structured values
cross as JSON of this crate's types. A package written in Rust uses
[`printcad-bench-sdk`](https://crates.io/crates/printcad-bench-sdk), which
re-exports this crate as `printcad_bench_sdk::api`. Another language that
builds WebAssembly components implements the WIT world directly.

Versions: 0.1.x speaks `printcad:workbench@0.1`, and a patch release only
adds. See
[Versions](https://github.com/gilbertorconde/printCAD/blob/master/docs/PLUGINS.md#versions).

Licensed under MIT or Apache-2.0, at your option.
