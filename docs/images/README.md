# Pictures in the guides

Pictures of models (`bracket-fillet`, `holes-section`, `nut-trap`,
`generators`, `surface-shade`, `hinged-arm`) are drawn headless by
`scripts/doc-pictures.sh`, which runs each scene's Lua and ends it in
`pc.doc.picture`: run it again after a change that shows in them.

The others are crops of the running app: its window captured on a
1600 x 1000 output of its own, so nothing else on the screen is in them,
and cropped to the panel or the part of the view a guide talks about.
