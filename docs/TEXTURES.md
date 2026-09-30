# Surface textures

A surface texture presses a pattern into a body's faces for printing:
knurling on a grip, ribs, hexagons, a rough finish, or a picture of your
own. The exact solid stays as it is. The texture is drawn in the view and
pressed into the STL and 3MF files you export and the file sent to the
slicer. STEP export writes the solid without it.

## Adding one

Right click a body (in the tree or the view) and pick **Surface
texture…**. Opened on a face, the texture starts on that face; otherwise
on the whole body. The task in the right panel shows the changes in the
view as you make them. OK keeps them as one undo step, Cancel puts back
what was there.

- **Faces**: click faces in the view to add them. Each listed face has a
  × to leave it smooth again. **Whole body** textures every face.
- **Pattern**: pick one from the gallery, or **Picture…** for a greyscale
  PNG or JPEG, white standing highest. The picture is kept in the document.
- **Projection**: how the flat pattern is laid on the faces.
  - *Triplanar* blends three flat projections and suits any shape.
  - *Flat along X, Y or Z* projects straight along one axis.
  - *Round X, Y or Z* wraps the pattern round an axis, a whole number of
    tiles round, so it closes without a seam: knurling on a cylinder.
  - *Spherical* wraps round the texture's centre.
- **Tile**: the size of one repeat of the pattern.
- **Turn**: the pattern's rotation on the surface.
- **Depth**: how far the pattern's high points stand from the surface.
  **Into the surface** cuts it in instead.
- **Keep flat**: faces within this angle of facing straight up or down
  stay smooth, a print's top and the side it stands on. 0 textures them
  too.

A body can carry several textures on different faces: **Add** starts
another, and the tabs switch between them. **Remove this texture** takes
the one shown away. A textured body shows TEXTURED in the tree.

The texture follows its faces through edits: faces are kept by the names
the model gives them, so changing a pad's length keeps the grip knurled.

## Exporting

The export dialog's **Surface textures** switch (on by default, for STL
and 3MF) presses them in. The exported mesh is finer than the view's, and
capped at a few million triangles for very fine patterns on large parts.
Send to slicer always includes them.

## Scripts

`pc.doc.set_textures{body = b, textures = {...}}` sets all of a body's
textures at once; see the command reference in `SCRIPTING.md`.

```lua
pc.doc.set_textures{body = b, textures = {
  {texture = {pattern = "Knurl", projection = {Cylindrical = "Z"},
              tile_mm = 4, depth_mm = 0.8, keep_flat_deg = 10}},
}}
```
