# Key Light, PSD preview proportions and timeline trim cursors

Prepared against `d1f6754`; the affected source files are unchanged since `b0fd83c`.

- Key Light is retained in the Keying menu and searchable by its Keylight aliases.
  The earlier shell fix searched `legacy_only`, but the exclusion was actually
  in `toon_catalogue_legacy_only`.
- When decoded still dimensions differ from footage metadata, previews retain
  the uniform source mapping used by export rather than squeezing new pixels
  into an old rectangle. Nominal dimensions are part of the media cache key,
  preventing reuse of a stale native-size fallback after metadata refresh.
  Matching dimensions retain the reduced-size cache optimization.
- Trim handles extend six pixels beyond each bar edge and also accept clicks,
  giving hover feedback from both sides. Short bars split their inside handle
  areas at the midpoint. Dragging retains the horizontal resize cursor.

## Validation status

The Key Light catalogue/search regression passed before the workspace reset.
The reconstructed patch passed Rust syntax and whitespace checks. The remaining
compiled regression runs did not finish before the reset; no success is claimed
for them. Run these checks before building/installing:

```bash
CARGO_BUILD_JOBS=2 cargo test --locked -p effectcraft-effects --lib toon_keylight_is_visible_and_searchable
CARGO_BUILD_JOBS=2 cargo test --locked -p effectcraft-media --lib psd_preview_reload_tests
CARGO_BUILD_JOBS=2 cargo test --locked -p effectcraft-render --lib tests_footage_size
CARGO_BUILD_JOBS=2 cargo test --locked -p effectcraft-ui-egui --test ui_timeline_zoom trim_edges_show_resize_cursor_from_both_sides_and_during_drag
```

The media test generates a layered PSD, changes its pixels and dimensions,
invalidates the source, and tests stale and refreshed metadata. The renderer
regression compares the layer-space footprint at Half and Full resolution.
The UI regression uses pointer hover and a real trim drag.

The user's particular PSD and Radeon viewport still require visual verification:
edit/save a PSD layer, reload footage, and compare Half preview with export.
This patch addresses dimension-mismatch distortion, not every possible PSD issue.
Close and reopen the application after rebuilding. No Cargo clean is needed.
