# Printing

What printCAD offers for the step from a model to a printer: the parts
list's printing columns, nut traps for captive nuts, and the print
layout that lays every part flat on the bed for export or the slicer.
The bed itself (its size, where its origin is, whether it is drawn) and
the slicer command are set in Preferences › Printing.

## Printing columns

The Assembly's Parts list (B) carries three columns for printing beside
the count and size of each part:

- **Print**: how many of the part to print. It starts at one per body of
  the part (a part that is bought starts at none); type another count in
  the cell to print spares or fewer. Clearing the cell goes back to one
  per body. The count is the whole part's, however the list is grouped.
- **Volume**: the volume of one piece. A solid's is the kernel's exact
  measure, worked out away from the window and kept until the part's
  shape changes (the cell shows … meanwhile); a mesh body's is the
  volume its triangles enclose.
- **Mass**: the filament one piece takes, its volume at the density of
  the material it is printed in.

**Printed in** under the list picks the material: PLA (1.24 g/cm³), PETG
(1.27), ABS (1.04), ASA (1.07), TPU (1.21), Nylon (1.14) or PC (1.20), or
Custom with a density of your own. It is kept with the document. A body
given a material of its own (Appearance, or `doc.set_body`) is weighed at
that material's density instead.

The line under the list adds it up: how many pieces, their volume and the
filament they take. Copy as CSV and Save as CSV carry the Print, Volume
(cm³) and Mass (g) columns too.

From a script, `asm.parts` answers `print`, `volume`, `mass` and
`density` for each part, `asm.part{body = ..., print = 3}` sets a count
and `asm.print_material{name = "PETG"}` the material (a name of your own
needs its `density`).
