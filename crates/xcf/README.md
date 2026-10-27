# effectcraft-xcf

Native plain-pixel-layer XCF reader, independently implemented from GIMP's public
[XCF specification](https://developer.gimp.org/core/standards/xcf/), revision dated
2026-07-28, consulted 2026-10-21. No GIMP/third-party decoder source or artwork was
used. GIMP is not a runtime dependency. Synthetic fixtures are original Rust code.

Supported: RGB/RGBA, 8/16-bit integer, linear/nonlinear samples, Normal and legacy
Normal, names, stacking, signed offsets, visibility, integer/float opacity, raw/RLE/
zlib tiles, 32/64-bit pointers, and stable layer tattoos. Text uses its saved pixels.
Versions 0–3 and 7–26 are accepted only for features in this subset; development-era
16-bit files before v12 are excluded. Embedded matrix sRGB profiles are recognized.

Unsupported features return an error with preparation instructions: groups, masks
(including disabled masks), floating selections, non-Normal blending, non-Union
compositing, live filters, vector/linked layers, float precision, indexed/grayscale
colour, non-sRGB profiles, unknown layer properties, and future versions. Outer
.xcf.gz/.xcf.bz2/.xcf.xz compression is not supported. Editing-only metadata, saved
selections and paths do not affect the imported artwork.

The reader borrows source bytes and decodes tiles lazily. Validation does not allocate
whole images. `layer_pixels` returns straight FP32 sRGB without layer opacity or
visibility (these belong to the timeline). `composite` flattens bottom to top using
each layer's saved linear/nonlinear Normal-over space, with one canvas and one tile.
No persistent decoded-document cache or CPU FP16 cache is added.

Bounds: 256 MiB file, 1024 layers, 16384 pixels per axis, 16 megapixels per canvas or
layer, and 64 megapixels across all layers. Strings, properties, hierarchy lists and
decompression are bounded. Native .xcf reads check file size before allocation and
while reading. These limits do not guarantee total application memory or OS stability.

Run `cargo test --locked -p effectcraft-xcf`. Application tests share the synthetic
fixture generator in `tests/support/mod.rs`.
