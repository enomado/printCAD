# printCAD

Parametric CAD for designing 3D-printed parts. Linux, Windows and macOS; Rust and Vulkan.

![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)
![Rust](https://img.shields.io/badge/rust-1.98%2B-orange)
![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20Windows%20%7C%20macOS-lightgrey)

> **Early development.** The full path from sketch to printable file works,
> but expect rough edges.

## Features

- **Sketcher:** lines, arcs, circles, ellipses, conics, weighted splines,
  slots, text and more, with geometric and dimensional constraints solved
  live. Solid edges, other sketches and datums come in as references, and
  generators draw gears, sprockets and shafts from their numbers.
- **Design:** pad, pocket, revolve, loft, pipe, helix, holes to thread
  standards, fillets, chamfers, draft, thickness, patterns and booleans, all
  editable in a feature tree, with datums and geometry borrowed from other
  bodies.
- **Assembly:** joints of every common kind (mate, align, hinge, slider,
  ball, cam, gears and more), components that move as one, linked copies
  and parts linked from other files, motion over time, interference and
  clearance checks, exploded views and a parts list.
- **Import:** STEP and IGES as solids, with the dimensions, tolerances,
  datums, notes and layers they carry; STL, OBJ, 3MF, PLY, glTF and VRML
  as meshes that can be converted to solids.
- **Export:** STEP, STL and 3MF, or straight to your slicer.
- **Documents:** `.prtcad` files, one tab each, with undo and redo.
- **View:** GPU picking of faces and edges, a clipping plane, a measure
  tool, body and face colours, and 6-DoF mouse support.
- **Scripting:** a Lua console that reaches every command of the
  application and the workbenches.
- **Variables and formulas:** any number can follow named variables and
  other dimensions, with units checked, and configurations switch a model
  between sizes.
- **AI agents:** chat with ACP agents that work the document through the
  same commands, and an MCP server for any other client.

The geometry kernel, [ogeom](https://github.com/gilbertorconde/ogeom-rs), is
pure Rust. No system CAD libraries are needed.

## Build and run

printCAD runs on Linux (Wayland or X11), Windows 10 or later, and macOS 11
or later. Ready-made downloads are on the
[releases page](https://github.com/gilbertorconde/printCAD/releases). On
Linux, unpack the archive and run `./install.sh`: it puts printCAD in your
applications menu and `printcad` on your path (`./install.sh --remove`
takes it away). The macOS and Windows builds are not signed yet, so the
first launch needs right-click › Open on macOS, and "Run anyway" on
Windows.

To build it you need Rust 1.98 or later and a GPU with Vulkan drivers. On
macOS Vulkan runs over Metal through MoltenVK (`brew install molten-vk`);
the released app carries its own. The shaders are compiled with shaderc:
`glslc`/`libshaderc` from your distribution on Linux, `brew install shaderc`
on macOS (then set `SHADERC_LIB_DIR=$(brew --prefix shaderc)/lib`); on
Windows it builds from source, which needs CMake and Python.

```bash
git clone https://github.com/gilbertorconde/printCAD.git
cd printCAD
cargo build --release
./target/release/printcad
```

Build the whole workspace, not only `app_shell`. The app starts a document
server binary from its own folder, and without it the app falls back to
plain file access.

A debug build (`cargo run -p app_shell`) is fine for development.

To use a 6-DoF mouse on Linux, install and start
[spacenavd](https://spacenav.sourceforge.net/). On Windows and macOS printCAD
reads the device directly. Either way it finds the device on its own.

## Controls

### Mouse

| Action | Control |
| --- | --- |
| Orbit | Middle drag |
| Set the orbit pivot | Middle click on the model, or **H** |
| Pan | Right drag |
| Zoom | Wheel |
| Select a face or edge | Left click (Ctrl adds) |
| Select a whole body | Left double click |
| Body menu | Right click on a body |
| Box select in a sketch | Left drag |
| Fit the model | **F** |
| Standard views | Click the orientation cube, or **0** to **6** |

### Keyboard

Every shortcut can be changed in Preferences › Keyboard. The defaults:

| Action | Keys |
| --- | --- |
| Command palette | Ctrl+K |
| New, Open, Save, Save As | Ctrl+N, Ctrl+O, Ctrl+S, Ctrl+Shift+S |
| Import, Export | Ctrl+I, Ctrl+E |
| Send to slicer | Ctrl+P |
| Undo, Redo | Ctrl+Z, Ctrl+Shift+Z or Ctrl+Y |
| New tab, Close tab | Ctrl+T, Ctrl+W |
| Next, Previous tab | Ctrl+Tab, Ctrl+Shift+Tab |
| Cut, Copy, Paste sketch geometry or features | Ctrl+X, Ctrl+C, Ctrl+V |
| Preferences | Ctrl+, |
| Recompute all | Ctrl+R |
| Delete the selected tree row | Delete |
| Show or hide the selected tree row | Space |
| Rename the selected tree row | F2 |
| Properties of the selected tree row | Alt+Enter |

View keys:

| Action | Keys |
| --- | --- |
| Fit all, fit the selection | F, Shift+F |
| Isometric, front, top, right | 0, 1, 2, 3 |
| Rear, bottom, left | 4, 5, 6 |
| Orthographic, perspective | O, P |

### Row and body menus

Right click a row of the tree, or a body in the view, for what can be
done to it. The menus are flat lists; anything that takes numbers or
choices opens as a task in the right panel, where edits show in the view
as they are made, OK keeps them as one undo step and Cancel puts back
what was there.

- A feature: Edit, Rename, its body's Appearance and Placement, Suppress,
  Hide, Set as tip, Move up, Move down, Move after (the task lists the
  body's features to click), Freeze body, Cut, Copy, Paste, Delete, Copy
  formulas and Paste formulas (onto a feature of the same kind),
  Recompute, Send to console, Properties.
- A body: Select, Rename, Hide, Show only this, Show all, Appearance,
  Placement, Freeze (its features are not rebuilt until it thaws; the row
  shows FROZEN), Make unselectable (clicks in the view pass through it),
  Linked copy, Select the original of a copy, Cut, Copy, Paste, Delete,
  Recompute, Send to console, Properties.
- In the view, the body's entries, and Face colour for the face under the
  pointer.

The Appearance task holds the body's colour (a palette, your own colours,
which you keep or forget from the custom colour picker and which every
body offers, and the colour it came with), how much shows through, the
colours of single faces (click a face in the view; a face's colour
follows it through a rebuild) and its material, from a list or typed in
with its density, which gives its mass in the Physical group. The
Placement task moves and turns the body by numbers, negative ones too;
the bodies of its rigid component move with it.

`doc.set_body`, `doc.set_face_color`, `doc.linked_copy`, `doc.move_after`
and `doc.recompute` do the same from a script.

A workbench's keys apply while it is active and win over the view keys.
A plain letter picks a tool; Shift and a letter picks its partner.

| Sketcher | Keys |
| --- | --- |
| Point, line, polyline, arc, circle | O, L, P, A, C |
| Ellipse, B-spline, rectangle, polygon, slot | E, B, R, G, S |
| Trim, external geometry, construction | T, X, N |
| Switch a polyline between lines and arcs | M |
| Coincident, point on object | Shift+C, Shift+O |
| Horizontal, vertical | Shift+H, Shift+V |
| Parallel, perpendicular, tangent | Shift+P, Shift+N, Shift+T |
| Equal, symmetric, block, lock | Shift+E, Shift+S, Shift+B, Shift+K |
| Dimension, radius, angle | Shift+D, Shift+R, Shift+A |
| Horizontal, vertical distance | Shift+L, Shift+I |

While a length is being typed, number keys go to the length.

| Design | Keys |
| --- | --- |
| Body, sketch | B, S |
| Pad, pocket | E, Shift+E |
| Revolution, groove | R, Shift+R |
| Loft, subtractive loft | L, Shift+L |
| Pipe, subtractive pipe | W, Shift+W |
| Hole | Shift+H |
| Fillet, chamfer, mirrored | U, C, M |

| Assembly | Keys |
| --- | --- |
| Mate, align, angle, hinge, slider | M, A, N, H, L |
| Fixed, parallel, perpendicular, distance, tangent | X, R, Shift+R, D, T |
| Ball, universal, pin in a slot, path, cam, width | Shift+B, Shift+U, Shift+S, Shift+P, Shift+C, Shift+W |
| Couple joints, rigid group, ground | K, U, F |
| Move a body, solve | G, S |
| Interference, collisions on drag, mass | I, C, W |
| Exploded view, parts list | E, B |
| Linked copies, replace a body | Y, Shift+Y |

## Scripting

Scripts are Lua. Every command of the application and the workbenches is
a function under `pc`, called with named arguments:

```lua
local s = pc.sketch.new{plane = "XY"}
pc.sketch.rect{sketch = s, x = 0, y = 0, width = 30, height = 20}
local pad = pc.design.pad{sketch = s, length = 12}
pc.design.set{feature = pad, length = 20}
```

Run them in the console (Windows › Console), from the Scripts menu and
toolbar button (every `.lua` file in `~/.config/printcad/scripts`), or
without a window: `printcad --script build.lua`. See
[Scripting](docs/SCRIPTING.md) for the guide and every command.

## Settings

Settings are stored in `~/.config/printcad/settings.json`. Change them in
Preferences (Ctrl+,).

## Reporting an import problem

1. Turn on **Preferences › General › Diagnostics › Write a report for every
   STEP import**.
2. Import the file again. The log shows where the report was written, under
   `/tmp/printcad/import-reports/`.
3. Open an issue on the
   [kernel tracker](https://github.com/gilbertorconde/ogeom-rs/issues) with
   the report, and the STEP file if you can share it.

## Project layout

| Crate | Purpose |
| --- | --- |
| `app_shell` | The application: window, frame loop, UI and input |
| `core_document` | Documents, feature tree, undo, file format |
| `doc_server` | The document server and its client |
| `kernel_api` | The geometry interface: meshes, profiles, solid operations |
| `kernel_ogeom` | That interface implemented with ogeom |
| `render_vk` | Vulkan renderer |
| `settings` | User settings |
| `ui_kit` | Colours, widgets, icons and fonts |
| `axes` | Axis presets, so no code assumes which way is up |
| `workbenches/wb_sketch` | Sketcher |
| `workbenches/wb_design` | Design |
| `workbenches/wb_assembly` | Assembly: joints between bodies |
| `workbenches/wb_wasm` | Workbench packages, run sandboxed |
| `workbenches/fixtures` | Ready-made scenes for tests and demos |
| `bench_api` | What a workbench package and the app exchange |
| `local_ipc` | Local sockets and helpers the app's processes talk through |
| `scripting` | The Lua engine scripts and the console run in |
| `agents` | Agent Client Protocol client and MCP server core |

More detail in [docs](docs/):

- [Architecture](docs/plan.md)
- [Editing workflow](docs/WB_IMP.md)
- [Document model](docs/DOCUMENT_MODEL.md)
- [Writing a workbench](docs/WORKBENCH_GUIDE.md)
- [Scripting](docs/SCRIPTING.md)
- [Variables and formulas](docs/VARIABLES.md)
- [Assembly](docs/ASSEMBLY.md)
- [Holes](docs/HOLES.md)
- [Workbench packages](docs/PLUGINS.md)
- [AI agents](docs/AI.md)
- [What is left to build](docs/FEATURE_GAPS.md)
- [Camera](camera_system.md)

## Roadmap

What each workbench still lacks, feature by feature, is in
[docs/FEATURE_GAPS.md](docs/FEATURE_GAPS.md). Features for printing
(filament use, print layout, nut traps) come next.

## License

MIT or Apache 2.0, at your option. See [LICENSE-MIT](LICENSE-MIT) and
[LICENSE-APACHE](LICENSE-APACHE).
