# Toon preview defaults and removals

Based on checkpoint `7f627c3`.

- Main Snapping starts off on launch and when opening a project; the toolbar can enable it.
- The composition viewport and every Preview shortcut start at Half resolution. Both controls
  remain editable and are saved with the project, including custom resolution and per-shortcut
  options. Older projects without these fields use Half. Global preferences no longer override
  project preview settings. Export resolution and output settings are unchanged.
- All 18 Simulation effects are removed, including CPU implementations, GPU kernels, particle
  backends and simulation caches. Existing projects containing them cannot render those effects
  in this edition; keep the previous build for those projects. Card Wipe and Curl Noise retain
  their CPU/GPU implementations and render regression tests in independent modules.
- Metadata is removed from panels, menus and commands. Old workspace layouts discard its tabs.
  Existing project/item comments and import-required file metadata are preserved.

This patch does not incorporate the separately supplied cache-before-playback patch or change
memory limits. It does not claim to prevent operating-system or GPU-driver failures.

Validation before the workspace reset: 27 project tests passed, Card Wipe and Curl Noise
render tests passed (3 tests), and both shader storage variants validated 218 entry points.
The changes were reconstructed after that reset. The full UI/GPU compile check was interrupted
and is not claimed as passed. The delivered patch was checked against the base commit again.

Run these checks on Linux:

```bash
CARGO_BUILD_JOBS=2 cargo test --locked -p effectcraft-project --lib preview::tests
CARGO_BUILD_JOBS=2 cargo test --locked -p effectcraft-effects --lib toon_catalog
CARGO_BUILD_JOBS=2 cargo test --locked -p effectcraft-effects --test card_wipe_golden --test curl_noise_golden
CARGO_BUILD_JOBS=2 cargo test --locked -p effectcraft-engine --lib preview::tests
CARGO_BUILD_JOBS=2 cargo test --locked -p effectcraft-ui-egui --lib old_metadata_tabs_load_and_are_removed
CARGO_BUILD_JOBS=2 cargo test --locked -p effectcraft-ui-egui --test ui_preview_align
CARGO_BUILD_JOBS=2 cargo test --locked -p effectcraft-gpu --lib full_and_half_storage_shader_variants_validate_without_a_gpu
CARGO_BUILD_JOBS=2 cargo test --locked -p effectcraft-gpu --lib tests_fx_cardwipe
CARGO_BUILD_JOBS=2 cargo build --locked -p effectcraft
```

Manually open an older workspace with Metadata, change both resolution controls independently,
save, close and reopen the project. Confirm the resolutions restore, new projects use Half,
Snapping starts off, and Simulation is absent from Effects & Presets.
