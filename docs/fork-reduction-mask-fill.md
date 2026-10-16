# Mask Interpolation and Content-Aware Fill removal

Apply after both previous reduction patches, including the VR/Media Browser/Tracker patch.

Deletes both dedicated panel implementations, removes their menu entries and available panel IDs,
removes their tabs from current/saved/floating layouts, and blocks contentFill.*, layer.newContentAwareFill,
mask.interpolate and mask.interpolationOptions through command policy. Extends command and layout
regression coverage and updates tests that previously expected retired panels to exist.

Ordinary masks, mask-path keyframe animation, Wiggler, Smoother, Motion Sketch and existing generated
fill footage remain. Low-level algorithms and serialized settings remain for compatibility; this is
functional removal rather than a complete dependency purge. No preview precision or export changes.

Static source checks and patch apply/reversal are checked here. Cargo is unavailable in the authoring
environment, so compilation, UI execution and Rust tests require the user's machine.

Build: cargo run --locked -p effectcraft -j 2
Tests: cargo test --locked -p effectcraft-engine feature_policy
Tests: cargo test --locked -p effectcraft-ui-egui fork_reduction_tests
Before committing, run the repository's required cargo xtask ci gate.

Manual checks: neither panel appears after restoring a saved workspace; regular masks can still be
created and animated; importing, playback, saving and H.264/MP4 export work as before.

Rollback: git apply -R --check PATH_TO_PATCH && git apply -R PATH_TO_PATCH
