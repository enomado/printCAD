# printcad-bench-sdk

Write [printCAD](https://github.com/gilbertorconde/printCAD) workbenches as
WebAssembly components. A workbench package adds tools, feature kinds with
parametric solids, task panels, viewport drawing, commands for scripts and
agents, and a Preferences page, and runs sandboxed on every system printCAD
runs on.

The SDK speaks the contract `printcad:workbench@0.1`, which its version
follows: a patch release only adds, so a package keeps building on any
later 0.1.x, and a printCAD of the same contract loads it.

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
printcad-bench-sdk = "0.1"
```

```rust
use printcad_bench_sdk::{Bench, api::*, bench, host};

#[derive(Default)]
struct Hello;

impl Bench for Hello {
    fn describe(&self) -> Registration {
        Registration {
            label: "Hello".into(),
            tools: vec![Tool {
                id: "acme.hello.wave".into(),
                label: "Wave".into(),
                behavior: ToolBehavior::Action,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn input(&mut self, input: &Input) -> bool {
        if matches!(input.event, Event::ToolActivated) {
            host::info("Hello");
            return true;
        }
        false
    }
}

bench!(Hello);
```

Build it with `cargo build --release --target wasm32-wasip2` and pack the
component with its `bench.toml`. The
[guide](https://github.com/gilbertorconde/printCAD/blob/master/docs/PLUGINS.md)
covers the manifest, packing, publishing and every part of the interface,
and [PrintCAD-example-wb](https://github.com/gilbertorconde/PrintCAD-example-wb)
is a repository to start from.

Licensed under MIT or Apache-2.0, at your option.
