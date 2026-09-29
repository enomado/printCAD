# Holes

Part Design's Hole drills at every circle centre and lone point of a
sketch. The task panel sets the rows below; a script sets the same fields
by name (`pc.part.hole`, `pc.part.set`).

## Size

**Standard** is Custom (a plain diameter) or a thread standard:

| Standard | Sizes | Form |
| --- | --- | --- |
| ISO metric coarse, fine | M1.6 to M48, M3x0.35 to M36x3 | 60° |
| UNC, UNF, UNEF | #0 to 1 inch | 60° |
| BSW, BSF | 1/8 to 1 inch | 55° |
| BSP parallel (G) | 1/16 to 2 | 55° |
| BSP taper (Rc) | 1/16 to 2 | 55°, 1:16 taper |
| NPT | 1/16 to 2 | 60°, 1:16 taper |

A sized hole drills the tap drill when **Threaded** is on, otherwise the
clearance of its **Fit** (close, normal, loose: ISO 273 for metric sizes,
ASME B18.2.8 for unified ones; the other standards drill the major
diameter), or **Custom**, a clearance diameter of your own. A tapped taper thread drills the thread's minor diameter at the
face and narrows 1:16 on the diameter from there.

**Class** is the internal thread's class: 4H to 8H and 4G to 8G for metric
(a G class moves the diameters out by the ISO 965-1 allowance,
(15 + 11 P) µm), 1B to 3B for unified, Medium or Normal for Whitworth. Pipe
threads have none. **Left-hand thread** turns a modeled thread the other
way.

**Modeled thread** cuts the thread itself into the wall, for printing it
rather than tapping it: the standard's flank angle, out to its major
diameter, along a cone for a taper thread. **Thread length** is the
**Thread depth** given, the **Whole hole**, or the hole **Less the
run-out** of three pitches a tap leaves at the bottom.

## Depth and bottom

**Through all** or a **Depth**. A blind hole's **Drill point** is flat or
angled (118°, 135° or any **Point angle**); **Point within the depth**
makes the depth run to the tip rather than to the end of the wall.
**Taper** leans the wall in toward the bottom (a tapped taper thread uses
its own).

## Hole cut

- **Counterbore:** a wider bore, diameter and depth.
- **Spotface:** a shallow counterbore, to face a seat.
- **Countersink:** a cone, diameter at the face and included angle.
- **Counterdrill:** a wider bore ending in a cone down to the hole:
  diameter, depth and the cone's angle.
- **ISO 4762 seat:** the counterbore for a socket head cap screw of the
  hole's metric size (DIN 974-1 diameters).
- **ISO 10642 seat:** the 90° countersink for a countersunk socket screw of
  the hole's metric size.
- **ISO 7380 seat:** the counterbore for a button head screw.
- **ISO 2009 seat** and **ISO 7046 seat:** the 90° countersink for a
  slotted or cross recessed countersunk screw.
- **DIN 7984 seat:** the counterbore for a low head cap screw.
- **ISO 4762 + washer seat:** the counterbore for a socket head cap screw
  on an ISO 7089 washer (DIN 974-1's wider row).
- **ISO 4017 seat:** the counterbore for a hex head screw, with room for a
  socket wrench (DIN 974-2).

## Your own cuts

Cuts you use often go in `hole_cuts.json` in the application's
configuration folder (`~/.config/printcad/` on most systems, beside
`settings.json`). It is read when the application starts; its profiles are
listed under the standard cuts, and picking one copies its sizes into the
hole.

```json
{
  "profiles": [
    { "name": "M3 heat insert", "cut": { "Counterbore": { "diameter": 4.2, "depth": 5.0 } } },
    { "name": "Deburr 6", "cut": { "Countersink": { "diameter": 6.0, "angle_deg": 90.0 } } },
    { "name": "Stepped 8", "cut": { "Counterdrill": { "diameter": 8.0, "depth": 3.0, "angle_deg": 118.0 } } },
    { "name": "Face 12", "cut": { "Spotface": { "diameter": 12.0, "depth": 0.5 } } }
  ]
}
```

## Scripts

```lua
pc.part.hole{
  sketch = s,
  depth = 8,
  thread = {standard = "Unc", size = "1/4-20", class = "3B", left_handed = false},
  threaded = true,
  drill_point = {Angled = {angle_deg = 118}},
  cut = {Counterdrill = {diameter = 9, depth = 2, angle_deg = 90}},
}
```

`standard` is one of `IsoMetricCoarse`, `IsoMetricFine`, `Unc`, `Unf`,
`Unef`, `Bsw`, `Bsf`, `BspParallel`, `BspTaper` or `Npt`; `size` is the
designation the panel lists. A screw seat is `cut = {Seat = {seat =
"SocketHead"}}`, or `"Countersunk"`, `"ButtonHead"`, `"SlottedCountersunk"`,
`"CrossCountersunk"`, `"LowHeadCap"`, `"CapScrewWithWasher"`, `"HexHead"`.
`clearance` sets a clearance of your own; `thread_length` is `"Given"`,
`"HoleDepth"` or `"RunOut"`.
