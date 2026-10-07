# transform_gizmo

Move, turn and scale handles for a 3D view, without a window: where the
handles are on screen, which one the pointer is over, what a drag does, and
what to draw.

This crate is shared: the same sources serve other applications, and it is
meant to become a common library. Keep it that way:

- It depends on `glam` and `emath` only and knows no printCAD type. A host
  converts its own types at the boundary: host space is `glam` in `f64`
  (points, unit directions, `DQuat`), angles are radians, the view's scale is
  host units per logical pixel, screen points are `emath::Pos2`.
- Colours are roles (`paint::Ink`), never values: the host maps them to its
  palette.
- A change here goes to the shared copy first, then comes back whole, so the
  copies do not drift. Formatting follows this repository.

## Use

A host keeps one `gizmo::Gizmo` per view and, each frame or input event:

1. `Gizmo::layout(view, set, mode)` lays out the handles of a `HandleSet`
   (the classic three axes: `HandleSet::placement_axes`; or any list of
   slides and turns, one per freedom).
2. `Layout::hit(pointer, style.hit_radius)` finds the handle under the
   pointer.
3. On a primary press over a handle, `Gizmo::press`; while it is held,
   `Gizmo::drag` with the pointer, then `Gizmo::release`, or `Gizmo::cancel`
   on Escape or when the target goes away.
4. `Gizmo::shapes(view, layout, highlighted)` lists what to draw, back to
   front.

`Event::Updated` and `Event::Finished` carry the delta from the press: apply
it to the target as it was at the press, record one undo step on `Finished`,
restore the press state on `Cancelled`. `hand` turns a delta into increments
of the freedoms a handle stands for.
