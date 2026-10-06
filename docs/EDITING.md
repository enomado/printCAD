# Editing workflow

How documents, bodies, sketches and features behave in the app.

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
- Clicking a solid in the viewport selects the face under the cursor; near
  an outline it selects the whole edge instead. Ctrl+click adds or removes
  faces of the selected body, and edges; the status bar counts them ("3
  faces, 2 edges"). A double click selects the whole body (one part of an
  assembly). The face or edge under the cursor is highlighted as you hover.

Tools are enabled only when their input exists:

| Tools | Need |
| --- | --- |
| New sketch, primitives, datums | A body |
| Pad | A sketch, or a flat face of the solid |
| Revolution, loft, pipe, helix | A sketch |
| Pocket | A solid, and a sketch or a flat face of it |
| Groove, hole | A sketch and an existing solid |
| Fillet, chamfer, draft, thickness, face tools, patterns, booleans | A solid |

A Design feature added to a surface body, or to an imported body that cannot
take features (a mesh not yet converted, a part linked from another file),
goes into a new body, and the log says so. Surface features go on a surface
body, or on one holding only sketches and datums; elsewhere they start a
new body ([Surfaces](SURFACES.md)).

## Sketches

**Creating one.** Design's New sketch switches to the Sketcher and shows
a plane picker:

- the face selected in the viewport, if any (the sketch then follows the
  face as the solid changes)
- the base planes XY, XZ and YZ
- a plane placed by an attachment mode on what is selected on the body, as
  a datum plane is
- the body's datum planes
- the XY, XZ and YZ planes of each local coordinate system in the body

The sketch is added to the body, opened for editing, and the camera turns
square to its plane.

**Editing one.** Double clicking a sketch in the tree opens it the same way.

**While editing,** the view stays square to the sketch plane: pan, zoom and
roll work, orbit and standard views do not.

**Finishing.** Close in the task panel, or Enter, ends the edit and returns
to the workbench you came from. Escape in the view drops what is in
progress first: a typed value, a half-drawn shape, the selection.

## Design features

1. A tool such as Pad adds the feature, hides the sketch it uses, and opens
   the feature's task panel.
2. Changes in the panel apply to the model as you make them.
3. **OK** keeps the feature. **Cancel** removes a new feature, or restores an
   existing one to how it was when the panel opened.

Everything done in one task panel is one undo step. While the panel is
open, the material the feature adds or takes away is drawn as a translucent
preview.

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

**Offset faces** pushes or pulls picked faces along their outward
normals (negative into the material; a bore offset in widens), and
**Move faces** shifts them, or turns them about an axis; in both the faces
around them follow on their own surfaces. The distance, the move and the
angle take formulas.

**Recognize holes** reads the body's solid for round holes: full bores
open at one end or both, ending flat or in a drill point. It deletes
their faces (one Delete faces feature) and drills them again as Hole
features, one per set of alike holes on one plane, from a hidden sketch
of their centres, so a recognized hole's diameter, depth and point are
numbers to change like any hole's. Bores it cannot describe (a
counterbore, a countersink, a slot) are left as they are and counted in
the log. On an imported solid it gives the body its base shape first.

**Borrowed geometry** brings another body's sketch, or faces and edges of
its solid, into this body, where the two bodies sit (Borrow geometry in Design;
`design.borrow`). It follows its source, or keeps a frozen copy. A
borrowed face is:

- a pad's or pocket's profile, when flat
- the face a pad or pocket stops on (Up to its name in the Type list)
- one of the faces an Up to shape side stops on (its Borrowed stop faces
  ticks), beside picked faces of the body's own solid; with only
  borrowed faces it needs no earlier material
- the face a revolution or groove turns until (Up to its name in the
  Type list), on the first feature too
- the plane of a new sketch (select the borrow, then New sketch), or of
  an existing one: a sketch's tree menu offers Map onto each flat face
  its body borrows (`design.map_sketch`). Either way the sketch follows
  the face as its body changes or moves.

A borrowed edge is an axis to turn about, a direction to pad along, or a
pipe's path.

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
- Selecting a feature shows its body as it stands there: the tip moves to
  it (back to the whole history when it is the last), later features are
  muted, and a feature made then goes in right after it. Moving through
  history this way is not an undo step.
- Appearance, placement and moving in history open in the task panel: the
  view shows each change at once, OK keeps it as one undo step and Cancel
  puts back what was there.
- Deleting a body removes its features. Unless the body was still empty,
  this clears the undo history.

## Rebuilding

- Changing a feature marks it dirty, along with everything that depends on
  it. Editing a sketch dirties the features built from it.
- Each frame, every body with a dirty feature is rebuilt on the kernel's
  threads. A body builds one plan at a time; changes made meanwhile wait,
  and only the latest one is built.
- A failed rebuild marks its feature in the tree and in the task panel, and
  logs the error. The body shows the history before the failure.
- Recompute All rebuilds every body.

## Undo

Undo steps back through the recorded edits by applying their inverses. Each
mouse gesture or command is one step, and the history keeps 64 steps. In a
sketch's editing session every edit (a line drawn, a constraint added) is a
step of its own. Imports, deleting a body that has anything in it, Replace
shape, converting a mesh, refining and repairing a shape clear the
history.
