# Editing workflow

How documents, bodies, sketches and features behave in the app today.

## Documents and tabs

- Each tab holds one document. **New** reuses the current tab if it is
  untouched, and opens a new tab otherwise.
- A new document is empty and opens in Design.
- The start page's New cards also create a first body. The "Empty sketch"
  card then opens an XY sketch in it, and the example cards load a ready-made
  model.

## The active body

- Clicking a body in the tree makes it the active body.
- Clicking a feature makes it the active object. Tools then work on that
  feature's body.
- Clicking in the viewport selects a face, or the whole body on a double
  click.

Tools are enabled only when their input exists:

| Tools | Need |
| --- | --- |
| New sketch, primitives, datums | A body |
| Pad, revolution, loft, pipe, helix | A sketch |
| Pocket, groove, hole | A sketch and an existing solid |
| Fillet, chamfer, patterns, booleans | A solid |

A feature added to an imported body goes into a new body, since an imported
solid has no history to add to.

## Sketches

**Creating one.** Design's New sketch switches to the Sketcher and shows
a plane picker:

- the face selected in the viewport, if any
- the base planes XY, XZ and YZ
- the body's datum planes
- the XY, XZ and YZ planes of each local coordinate system in the body

The sketch is added to the body, opened for editing, and the camera turns
square to its plane.

**Editing one.** Double clicking a sketch in the tree opens it the same way.

**While editing,** the view stays square to the sketch plane: pan, zoom and
roll work, orbit and standard views do not.

**Finishing.** Close in the task panel, or Enter or Escape, ends the edit and
returns to the workbench you came from.

## Design features

1. A tool such as Pad adds the feature, hides the sketch it uses, and opens
   the feature's task panel.
2. Changes in the panel apply to the model as you make them.
3. **OK** keeps the feature. **Cancel** removes a new feature, or restores an
   existing one to how it was when the panel opened.

Everything done in one task panel is one undo step.

**Imported and converted solids take features too.** The first feature
added to one (a pocket, a fillet) gives the body a **Base shape** feature
first: the imported solid, kept as what the body's history starts from.
The body stays one body; its history reads Base, then the feature. Cancel
on that first feature, or deleting the Base once nothing follows it,
makes the body the plain imported solid again. Repair shape on such a
body mends its base, and the features build again on it.

**Replace shape…** (a body's menu, in the tree or the view) reads the
shape from another file: its first solid becomes the base, and the
features after it build again on the new shape, finding their faces by
name. On a body without features it simply replaces the shape. It clears
the undo history, as an import does.

**Delete faces** takes picked faces out of the solid and closes each
opening from the faces around it: a bore, a boss or a round taken away,
on any solid, an imported one included. What the kernel does not close
yet it refuses by name, on the feature.

**Recognize holes** reads the body's solid for round holes: full bores
open at one end or both, ending flat or in a drill point. It deletes
their faces (one Delete faces feature) and drills them again as Hole
features, one per set of alike holes on one plane, from a hidden sketch
of their centres, so a recognized hole's diameter, depth and point are
numbers to change like any hole's. Bores it cannot describe (a
counterbore, a countersink, a slot) are left as they are and counted in
the log. On an imported solid it gives the body its base shape first.

## The tree

- Double clicking a feature opens it for editing in the workbench that owns
  it.
- The eye on a row shows or hides a feature, a body or an imported part.
- A feature's right-click menu offers edit, rename, its body's appearance
  and placement, suppress, hide, move up, down or after another, set or
  clear the tip, freeze the body, cut, copy, paste, delete, copy and paste
  formulas, recompute and properties. A body's menu, in the tree or the
  view, offers appearance, placement, freeze, make unselectable, linked copy
  and the rest. Features after the tip are shown muted and are not built.
- Appearance, placement and moving in history open in the task panel: the
  view shows each change at once, OK keeps it as one undo step and Cancel
  puts back what was there.
- Deleting a body removes its features and clears the undo history.

## Rebuilding

- Changing a feature marks it dirty, along with everything that depends on
  it. Editing a sketch dirties the features built from it.
- Each frame, every body with a dirty feature is rebuilt on the kernel
  thread.
- A failed rebuild marks its feature in the tree and in the task panel, and
  logs the error.
- Recompute All rebuilds every body.

## Undo

Undo steps back through the recorded edits by applying their inverses. Each
mouse gesture or command is one step, and the history keeps 64 steps.
Imports, deleting a body, converting a mesh and repairing a shape clear the
history.
