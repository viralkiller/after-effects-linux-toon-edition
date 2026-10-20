# Grouped keyframes and source replacement

## Copy Position and Scale together

Right-click a keyframe and choose **Select All Keys at This Frame**. This selects
keys at that composition frame across every property of the selected, unlocked
layers, including collapsed properties. Press Ctrl+C, move the playhead to the
destination frame, and press Ctrl+V. Shift-clicking or marquee-selecting keys also
supports copying multiple properties together.

Explicit key selection copies only those keys, without adding keys from stale
property highlights. Relative timing and keyframe interpolation are retained.
Selecting a property or Transform group still copies its values and animation.

## Replace a layer's source

Select the destination layer in the Timeline. Drag an item from the Project
panel, hold Alt, and drop onto that highlighted layer's row. You can start the
drag before pressing Alt. The destination row is outlined during the drop.

Replacement changes only the source reference. It preserves the layer ID, name,
stack position, timing, transforms, keys, effects and other layer settings. It
keeps existing scale and anchor values; it does not fit differently sized sources.
Undo restores the previous source. Locked layers, unsupported sources, and
composition cycles are rejected. An Alt-drop outside a selected layer does not
insert a new layer.

## Validation

Prepared against commit 7f627c3. The patch is separate from the preview-defaults
and removals patch. Rust compilation and tests must be run locally:

```bash
CARGO_BUILD_JOBS=2 cargo test --locked -p effectcraft-engine --lib tests_timeline
CARGO_BUILD_JOBS=2 cargo test --locked -p effectcraft-engine --lib copy_a_selected_property_pastes_onto_the_same_property
CARGO_BUILD_JOBS=2 cargo test --locked -p effectcraft-ui-egui --test ui_keyframes --test ui_drag_drop
CARGO_BUILD_JOBS=2 cargo build --locked -p effectcraft
```
