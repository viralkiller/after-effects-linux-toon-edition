# My Light Effects: feature reduction

Base: the supplied EffectCraft source archive at commit 9a7d3e7.

## Scope

Removed the Learn workspace, Home Learn tab, tutorial coach and tutorial engine; Discord,
community/product promotional controls in the header, Home and About; corresponding Help
commands; and the demo command and normal desktop/web/CLI demo launch behavior.
Templates, ordinary workspaces, useful reference help and contributor credits remain.
The demo generator remains a development fixture for tests, snapshots and benchmarks.

Camera tracking/solving, Roto Brush/Refine Edge and face tracking are unavailable through
commands, menus, tools, effect catalogs and model settings. Ordinary point/mask tracking,
Warp Stabilizer, stabilization, 3D cameras, paint and puppet tools remain.
Generic edits cannot introduce new instances of the retired effects. Old saved Learn
layouts and tool slots migrate to retained controls.

## Compatibility and size

Existing retired effect data is preserved. Opening such a project displays a warning.
Compatibility renderers, shared tracking code, serialized data types and associated
backend dependencies remain. New analysis is blocked, trained models are not automatically
loaded, and retired custom editors are removed. This patch is a functional feature reduction,
not a complete dependency purge; it makes no promise about binary or memory savings.
No changes were made to the working native file dialog.

## Validation performed here

- Checked changed Rust files for balanced delimiters/strings and valid include paths.
- Checked removal references and preservation of ordinary tracking/menu entries.
- Added engine policy and UI absence regression tests; adapted remaining demo-based tests
  to call the fixture directly, and removed tests dedicated to retired workflows.
- Verified that the patch applies to the supplied original archive and reverses cleanly.

Cargo/rustc are unavailable in the authoring environment. Compilation, Rust tests, formatting
and visual validation have NOT been run. Do not treat this as a tested release.

## Apply on Linux Mint

Save my-light-effects-reduction.patch in Downloads. In your checkout:

```bash
cd "$HOME/Projects/_Forked_/my-light-effects"
git status --short
git apply --check "$HOME/Downloads/my-light-effects-reduction.patch"
git apply "$HOME/Downloads/my-light-effects-reduction.patch"
cargo run --locked -p effectcraft -j 2
```

Apply only to the clean source version used to create the archive. Stop if the check fails;
do not force the patch or use --reject. Keep target/ intact to reuse your build cache.

After it builds, check Home, header, Help, workspace selection, Tracker, mask tracking,
Effects & Presets and Settings. Test a normal import -> animate -> save -> reopen -> export
workflow. Test opening any old projects with retired effects on copies.

Before committing, run formatting, the focused regressions, then the repository's required gate:

```bash
cargo fmt --all
cargo test --locked -p effectcraft-engine feature_policy -j 2
cargo test --locked -p effectcraft-ui-egui --test ui_panels retired_controls_are_absent -j 2
cargo xtask ci
```

## Rollback

Before further source edits or formatting, undo exactly this patch with:

```bash
git apply --reverse --check "$HOME/Downloads/my-light-effects-reduction.patch"
git apply --reverse "$HOME/Downloads/my-light-effects-reduction.patch"
```

Stop if the reverse check fails; do not reset unrelated work. Rollback does not remove the
build cache. Re-run the same Cargo command to rebuild the original source if needed.
