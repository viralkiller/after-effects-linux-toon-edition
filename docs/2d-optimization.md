# 2D preview optimizations

Target: the reduced EffectCraft fork after the Learn/community, tracking/AI,
VR/Media Browser, and mask-interpolation/content-aware-fill removals, with the
previous CPU FP16 cache experiment rolled back. No dependencies or project file
formats change.

## Default improvements

- Cache processed still layers across frames; placement changes reuse pixels.
  Masks, effects, source reloads, proxies, scale and color settings remain keyed.
  Movies/sequences retain time-dependent caching. Custom footage providers opt
  in through `cache_revision`; independent providers must use distinct tokens.
- Cache reduced still images at preview dimensions. A cold reduced request does
  not also retain the full image. Move uniquely owned image buffers instead of
  cloning them where possible.
- Feather masks with single-channel FP32 buffers, preserving the previous RGBA
  blur's running sums, radii, pass order and edge rules.
- Keep supported ordinary layer effect chains on the GPU; reuse masked-source
  uploads and stable processed GPU results. Unsupported steps retain CPU
  fallbacks. Masks and layer styles retain their existing rendering paths.
- Composite small normal-blend layers using just their affected rectangle when
  the canvas is uniquely owned, placement is one affine transform, quantization
  matches, and no minification prefilter is needed. Other cases use the existing
  full-frame path.
- Share the preview budget between finished frames and GPU working caches.
  Include display mipmaps and decoded spare buffers in cache accounting. Limit
  native simultaneous frame jobs to one through four according to the budget;
  each frame can still parallelize pixel processing. Bound queued GPU work.

Budgets are not hard process-RAM or GPU-allocation limits. Active frames,
decoders, UI resources and drivers also use memory. `GpuContext::memory_stats()`
reports retained working-cache categories and textures awaiting safe reuse.

## Optional half-float GPU preview storage

Use GPU acceleration in project settings, then launch:

```bash
EFFECTCRAFT_GPU_FP16=1 cargo run --locked -p effectcraft -j2
```

This stores intermediate preview pixels in RGBA16Float (8 bytes/pixel instead
of 16), with **FP32 shader arithmetic and accumulated sums**. Uploads stay FP32;
conversions happen on the GPU, avoiding CPU FP16 packing/unpacking.

The initial gate allows native 2D display previews in 8/16-bpc projects without
composition motion blur, using Gaussian/Fast Box/Directional Blur, Drop Shadow,
Levels, Curves, Hue/Saturation, or Invert. Other frames stay FP32. Unsupported
half-float pipelines fall back to FP32. The first eligible preview compiles the
extra pipelines. Half storage is lossy: inspect gradients and soft transparency.
Exports retain the full-precision render path and normal project quantization.
Restart without the environment variable to disable the option.

## Apply, test and reverse

From the reduced checkout with the patch saved in Downloads:

```bash
cd "$HOME/Projects/_Forked_/my-light-effects"
git apply --check "$HOME/Downloads/my-light-effects-2d-optimization.patch" &&
git apply "$HOME/Downloads/my-light-effects-2d-optimization.patch"
cargo test --locked --lib -p effectcraft-raster -p effectcraft-render -p effectcraft-media -p effectcraft-gpu -p effectcraft-ui-egui optimization -j2 -- --nocapture
cargo run --locked -p effectcraft -j2
```

If the apply check fails, stop and inspect the differences; do not force it or
reset the checkout. Do not run `cargo clean`. GPU comparisons explicitly report
when no adapter is available; shader validation runs without an adapter.

To reverse only this patch, if later edits do not overlap:

```bash
git apply --reverse --check "$HOME/Downloads/my-light-effects-2d-optimization.patch" &&
git apply --reverse "$HOME/Downloads/my-light-effects-2d-optimization.patch"
```

## Measure on the laptop

Compare the same 720p project, frame range, preview scale, acceleration mode and
cache budget. Record cold performance after purging, then warm performance on a
second pass. Use scenes with moving stills, feathered masks and supported effects.
Compare playback FPS, time to cache the range, peak memory, and exported frames.
Try the default patch before optional half-float storage. Speed improvements
are workload and driver dependent; compiler checks do not establish an FPS gain.
