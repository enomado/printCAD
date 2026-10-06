# printcad-kernel-api

printCAD's geometry interface as plain data: meshes, profiles and the
solid operations (`SolidOp`: extrusions, revolutions, lofts, pipes,
booleans, fillets and the rest) a body's history builds from. It holds no
geometry code; printCAD's kernel adapter runs the operations.

Workbench packages meet it through
[`printcad-bench-sdk`](https://crates.io/crates/printcad-bench-sdk), which
re-exports it as `printcad_bench_sdk::api::kernel_api`; there is no need to
depend on it directly. The library is named `kernel_api`.

Its versions follow the workbench contract: 0.1.x speaks
`printcad:workbench@0.1`, and a patch release only adds. See
[Versions](https://github.com/gilbertorconde/printCAD/blob/master/docs/PLUGINS.md#versions).

Licensed under MIT or Apache-2.0, at your option.
