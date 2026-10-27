# Plain GIMP XCF import

Save an RGB, built-in sRGB, 8-bit or 16-bit integer XCF with ungrouped Normal pixel
layers. Apply masks and filters in GIMP before import. This first patch deliberately
rejects advanced features rather than silently omitting them.

File → Import → File accepts `.xcf` and offers the existing options:

- **Composition**: separate layers padded to document dimensions.
- **Composition – Retain Layer Sizes**: separate layers with their own bounds/offsets.
- **Footage**: flatten visible layers to one still.

Names, stacking, offsets, visibility, opacity and transparent pixels are preserved.
Text is raster footage. Drag import follows the existing default drag-import preference.
The command interface uses `file.import` with `importAs` equal to `footage`,
`composition` or `compositionLayerSizes`. Footage may optionally select `layer` by
zero-based top-to-bottom index or unique exact name.

Layers remain linked to the XCF. Saving the project retains source paths and layer
identifiers. Reload Footage and existing automatic reload invalidate media/render
caches. GIMP tattoos identify layers across renaming and reordering; files without
these identifiers require unique layer names. A deleted layer never falls back to
an unrelated layer at its old index.

Timeline animation, order, visibility, opacity and position remain project-owned on
reload. Geometry changes or added source layers may require reimport/repositioning;
this is not automatic synchronization of the entire GIMP document.

A composition uses the project's existing colour settings. If GIMP's compositing
spaces differ, the importer warns without changing global settings. For all-linear
Normal layers, Blend Colors Using 1.0 Gamma is the corresponding project option.
Mixed spaces cannot be reproduced by one global setting; Footage uses each source
layer's recorded space when flattening. Unsupported profiles require conversion to
GIMP's built-in sRGB first.

## Verification

```bash
CARGO_BUILD_JOBS=2 cargo test --locked -p effectcraft-xcf
CARGO_BUILD_JOBS=2 cargo test --locked -p effectcraft-engine -p effectcraft-media --lib xcf
CARGO_BUILD_JOBS=2 cargo test --locked -p effectcraft-ui-egui --lib xcf_import
CARGO_BUILD_JOBS=2 cargo build --locked -p effectcraft
```

Synthetic tests cover decoding, hostile files, import geometry, save/undo, source
identity and cache refresh. Real GIMP artwork and a visual check on Linux remain
final acceptance checks; this is not a claim of full GIMP/XCF compatibility.

Preparation validation (2026-10-21): all 13 reader tests passed; an independent
ImageMagick decode of the original 80×80 layered fixture matched the flattened
RGBA bytes exactly. `cargo check --locked -p effectcraft-engine -p effectcraft-media
--tests` passed (only an existing unused `tmp` warning). Patch application was
checked on the published source and with the earlier Keylight/PSD/cursor patch.
The broader GUI/build and engine/media runtime-test builds were not completed here;
run the commands above on Linux before treating this as a fully validated build.
