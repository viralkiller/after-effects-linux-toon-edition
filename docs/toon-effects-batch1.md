# Toon Edition: effect catalogue reduction, batch 1

This batch hides 44 effects from the normal built-in catalogue through `legacy_only`.
The registry, ID lookup and render implementations remain available for existing
projects. It does not remove shader code or promise faster rendering or smaller builds.

It builds on the five-effect Blur & Sharpen menu. The import-dialog fix is untouched.
Colour correction, other distortion effects, time effects, audio, expression controls,
paint, basic mattes, rain/snow and particle systems remain unchanged. Camera and layer
transforms remain available; hiding 3D Channel effects does not disable multiplane work.

## 3D render-pass effects

- 3D Channel Extract (`ec.3d.channelextract`)
- Cryptomatte (`ec.3d.cryptomatte`)
- Depth Matte (`ec.3d.depthmatte`)
- Depth of Field (`ec.3d.depthoffield`)
- EXtractoR (`ec.3d.extractor`)
- Fog 3D (`ec.3d.fog3d`)
- ID Matte (`ec.3d.idmatte`)
- IDentifier (`ec.3d.identifier`)

## Live-action repair and complex keying

- Rolling Shutter Repair (`ec.distort.rollingshutterrepair`)
- Advanced Spill Suppressor (`ec.key.advancedspill`)
- CC Simple Wire Removal (`ec.key.ccsimplewireremoval`)
- Color Difference Key (`ec.key.colordifference`)
- Difference Matte (`ec.key.differencematte`)
- Inner/Outer Key (`ec.key.innerouter`)
- Key Cleaner (`ec.key.keycleaner`)
- Screen Key (`ec.key.screen`)
- Key Light (`ec.keying.keylight`)

## Heavy simulations

- CC Hair (`ec.sim.cchair`)
- CC Mr. Mercury (`ec.sim.ccmrmercury`)
- Foam (`ec.sim.foam`)
- Caustics (`ec.sim.caustics`)
- Wave World (`ec.sim.waveworld`)
- Card Dance (`ec.sim.carddance`)
- Particle Playground (`ec.sim.particleplayground`)
- Shatter (`ec.sim.shatter`)

## Surface treatments

- CC Glass (`ec.stylize.ccglass`)
- CC Plastic (`ec.stylize.ccplastic`)
- Bevel Alpha (`ec.perspective.bevelalpha`)
- Bevel Edges (`ec.perspective.beveledges`)
- Emboss (`ec.stylize.emboss`)
- Color Emboss (`ec.stylize.coloremboss`)

## Decorative transitions

- Block Dissolve (`ec.transition.blockdissolve`)
- CC Glass Wipe (`ec.transition.ccglasswipe`)
- CC Grid Wipe (`ec.transition.ccgridwipe`)
- CC Image Wipe (`ec.transition.ccimagewipe`)
- CC Jaws (`ec.transition.ccjaws`)
- CC Light Wipe (`ec.transition.cclightwipe`)
- CC Line Sweep (`ec.transition.cclinesweep`)
- CC Radial ScaleWipe (`ec.transition.ccradialscalewipe`)
- CC Scale Wipe (`ec.transition.ccscalewipe`)
- CC Twister (`ec.transition.cctwister`)
- CC WarpoMatic (`ec.transition.ccwarpomatic`)
- Card Wipe (`ec.transition.cardwipe`)
- Venetian Blinds (`ec.transition.venetian`)

## Validation

Patch application and reversal were checked against checkpoint 774e0ce with the
previous blur-menu edit reconstructed. Every removed ID was checked against the
checkpoint catalogue. Compilation and interactive testing on Linux Mint are pending.

After building, check the reduced menus, import a PNG through the fixed chooser,
and open a disposable copy of an older project containing one of the hidden effects.
Do not overwrite the original project during that compatibility check.
