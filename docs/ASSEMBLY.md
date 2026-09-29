# Assembly

The Assembly workbench places bodies against each other with joints. A
joint belongs to the body it moves and names the body it holds against.
Joints solve when one is made or edited and when a body moves, and the
moves are ordinary edits: a joint and the moves it causes undo as one step.

## Making a joint

Pick a joint tool, click a face on the body to move, then a face on the
body it goes against. The first body moves; the second stays where it is.

| Joint | Takes | Leaves free |
| --- | --- | --- |
| Mate (M) | two flat faces | sliding on the face, turning about its normal |
| Align (A) | two axes | turning about the axis, sliding along it |
| Angle (N) | two flat faces | everything but the angle |
| Hinge (H) | two axes | turning about the axis |
| Slider (L) | two axes | sliding along the axis |
| Fix together (X) | any face on each | nothing: the body moves with the other |
| Parallel (R) | two flat faces | everything but the two tilts |
| Perpendicular (Shift+R) | two flat faces | everything but one tilt |
| Distance (D) | two flat faces | everything but the distance |
| Tangent (T) | a flat face and a round one | rolling and sliding on the flat face |

An axis is a round face (a hole, a pin, a boss) or an edge: a circular
edge gives its circle's axis, so a hole's rim works, and a straight edge
gives its own line. Settings a tool does not ask for start at what the
bodies make now (an angle, a distance), so making the joint moves nothing
it need not.

A datum plane or line selected in the tree while a joint is picked
counts as a face of its body. After the first pick the panel offers the
origin's planes and axes as the other end: the body is then held to the
world, which never moves (the nil id as `other` in a script, its face in
world space).

While a joint's faces are picked, every body but the one under the
cursor and the one picked first fades, so faces behind it can be seen and
clicked.

A joint's settings change its kind in place (Kind), from where the
bodies stand; a kind that takes other sorts of faces, or Pick faces again,
picks the two faces afresh while the joint keeps its name. `asm.set` takes
`kind`, `face`, `other` and `other_face` for the same.

Every joint's settings can turn its body about the joint's axis or
normal by an angle (Turn), or half a turn across it (Turn over, the body
the other way round); the joint takes the new place as its own, a driven
hinge keeping its angle. A fixed joint's shift, where the body sits from
the other along the other's axes, is three fields that take formulas.
`asm.turn` and `asm.flip` do the same from a script.

A selected joint is drawn in the view: a dot where each end takes hold,
a flat face's normal and a square in its plane, an axis as a dashed line,
and a dashed link between the ends with the joint's name.

Insert linked copies (Y) puts copies of the selected body in a row
beside it, as many as you ask, a step apart. Each copy takes the body's
shape and follows every change to it (an edit to the original's features,
a new import), is placed on its own and takes joints like any body, and
the parts list counts it with the original. A body with no joints is
dragged straight across the view to put it where it goes. `asm.copy` does
the same from a script.

Rigid group (U) locks several bodies together as they sit, in one
feature: click each body (a second click takes one out) and press OK. The
group moves as one, the first body the one the rest hold to; its settings
change the members or dissolve it. `asm.group` makes one from a script.

Ground (F) keeps a body where it is; the bodies joined to it are placed
against it. The first joint of an assembly grounds the body it holds to
when nothing is grounded yet. The status bar says how many motions the joints leave open,
and for the selected body which ones.

## Driving and limits

A hinge's angle and a slider's position can be driven: tick Drive in its
settings and give a value, or a formula (see [VARIABLES.md](VARIABLES.md)).
A hinge's angle counts from where it sat when the joint was made. Limits
keep the motion within a range while it is not driven; a joint resting
on a limit shows its motion as "one way, at its limit". Play sweeps a
driven joint through its limits (or a whole turn, or 25 mm either way) to
show the motion, and puts it back when stopped. Record saves the same
sweep, there and back, seen from the current view: as an animated PNG, a
GIF, or a folder of numbered PNG frames, by the kind of file chosen.

Check collisions through the motion, in a hinge's or a slider's
settings, steps the drive across its limits (a whole turn, or 25 mm either
way, without them) and lists each step where two bodies share more
material than where the joint stands, away from the window.
`asm.motion_clashes` does the same from a script.

An alignment leaves two motions, the turn about its axis and the slide
along it, and each can be driven or limited the same way (`turn_drive`,
`turn_limits`, `slide_drive` and `slide_limits` in a script).

## Gears, belts, racks and screws

Couple joints (K) ties two joints' motions: the driven joint follows the
driving one. It takes the selected hinge or slider as the driver and the
first joint it can tie to it, and its settings let you pick others:

| Kind | Ties | The driven joint moves |
| --- | --- | --- |
| Gears | two hinges | the other way, `ratio` turns per turn |
| Belt | two hinges | the same way, `ratio` turns per turn |
| Rack and pinion | a hinge and a slider | the pinion's pitch circle: 2π × radius a turn |
| Screw | a hinge and a slider | the lead for each whole turn |

For meshed gears the ratio is the driver's teeth over the driven gear's:
a 20-tooth gear driving a 40-tooth one is 0.5. Reverse turns the driven
motion the other way (a crossed belt, a left-hand screw). The ratio, the
pitch radius and the lead take formulas.

A coupling is made where the two joints stand, so making it, or changing
its ratio, moves nothing there. Drive or drag the driving joint and the
driven one follows; the status bar shows the driven body as placed. A
coupling counts its driver's whole turns: after one turn of a driver at
1:2 the driven gear is half a turn round, not back where it started.

## Dragging

Drag a jointed body with the left mouse button: it follows the mouse as
far as its joints let it, so a door swings on its hinge rather than
sliding off it. A grounded body, or one with no joints of its own, does
not drag; use Move body (G) for those. A drag is one undo step.

A drag stops where the body would run into another: it comes to rest
against it. Faces that only touch, as mated faces do, never stop it, and
bodies that already overlapped when the drag began may move as long as
they overlap no further. Stop drags at collisions (C) in the toolbar turns
this off.

## Checking interference

Check interference (I) looks at every pair of visible solid bodies and
lists those that share material, with how much. The shared material is
drawn in red over the whole scene, where the bodies would hide it, and
marked with its volume. The check runs beside the window, which stays
usable: the panel shows how many pairs are done, and Stop keeps what was
found so far. Only pairs whose boxes meet are checked. Mesh bodies are
left out; convert one to a solid to check it. With a body selected, the
check takes that body against every other; Check every pair in the panel
takes them all. Check clearance, with the clearance set in the panel,
looks instead for pairs nearer to each other than that: each drawn as a
line between the two nearest points, with the distance. `asm.interference`
takes a `clearance` to do the same.

## Exploded view and parts list

Exploded view (E) moves every body straight out from the middle of the
assembly by the spread you set. Nothing is kept: the bodies go back when
it closes.

Parts list (B) lists every part with how many there are, bodies of the
same shape counted together, and the size of each along its own axes.
Copy as CSV puts it on the clipboard for a spreadsheet; Save as CSV
writes it to a file. Number the parts gives each part an item number, kept
from then on; Bought marks a part bought rather than made, which leaves
its bodies out of an export of every visible body and out of Send to
slicer; Add column adds a column of your own (a part number, a supplier)
with a value per part. All of it is kept in the document as the Parts list
row of the tree, and `asm.part` sets it from a script.

Mass and centre of mass (W) measures every visible solid body at the
density you set (g/cm³), with each body's share, and marks the centre of
mass in the view: what a tip-over check needs. `asm.mass` answers the
same to a script.

## From a script

Every joint tool is a command (`asm.mate`, `asm.hinge`, ...) taking faces
as `pc.doc.faces` lists them. `asm.set` changes a joint's settings,
`drive` and `limits` included, and a coupling's joints, kind and ratio;
`asm.couple` ties two joints; `asm.travel` reads a hinge's angle or a
slider's position; `asm.freedom`, `asm.interference` and `asm.parts` read
the assembly. [SCRIPTING.md](SCRIPTING.md) lists them all.
