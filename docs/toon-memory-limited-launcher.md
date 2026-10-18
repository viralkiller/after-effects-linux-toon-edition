# Memory-limited Linux launcher

Adds a launch option for Linux Mint with systemd 255 and cgroup v2. Independent of
the cache-before-playback patch; it can be applied before or after that patch.
No Rust files, existing launcher entries, caches, project files or licences change.

## Use

Close existing EffectCraft instances and save your work. From the repository root:

```bash
# Verify real kernel limits without opening the application:
bash tools/run-toon-limited.sh --check

# Start the existing debug build:
bash tools/run-toon-limited.sh

# Or the existing release build:
bash tools/run-toon-limited.sh --release
```

The launcher does not rebuild. It expects target/debug/effectcraft or
target/release/effectcraft. If using a custom Cargo target directory, adjust the
exe assignment in the launcher. Application arguments follow `--`, for example
`bash tools/run-toon-limited.sh -- --control 9877`.

Limits apply only to instances started through this launcher. Starting the binary
directly, using cargo run, or using an existing desktop shortcut bypasses them.
The fixed unit name rejects a second concurrent launch through this script.
No sudo, permanent system setting, or automatic unrestricted fallback is used.

## Behaviour and limits

The entire launched process group receives MemoryHigh=3G, MemoryMax=4G,
MemorySwapMax=512M and OOMPolicy=kill. Here G/M use powers of 1024. Above the high
threshold Linux applies reclaim/throttling. If memory cannot be kept under the
maximum, a cgroup OOM kill terminates the group. Swap has its own separate cap.
These are conservative starting values for the reported 13 GiB usable RAM,
not performance measurements or a guarantee of free memory elsewhere.

Before executing EffectCraft, the guard verifies its scope membership and all
four kernel control-file values. Missing controllers, wrong values, an unavailable
user systemd session, or a systemd error prevent launch. The --check option verifies
configuration; it does not force an OOM or exercise the driver.

An OOM kill can lose unsaved changes. This launcher does not isolate rendering from
the UI process or add recovery saving. It limits cgroup-accounted memory, not every
GPU/driver allocation, and cannot guarantee prevention of OS crashes. It applies to
exports as well as previews while the app runs through it. It does not alter GPU
settings, FP16 behaviour, or application cache budgets.

While running, inspect usage in a second terminal:

```bash
systemctl --user show effectcraft-limited.scope \
  -p ControlGroup -p MemoryCurrent -p MemoryHigh -p MemoryMax \
  -p MemorySwapCurrent -p MemorySwapMax -p OOMPolicy
```

## Validation

```bash
bash -n tools/run-toon-limited.sh tools/toon-memory-guard.sh
python3 tools/test_toon_memory_guard.py
```

Seven synthetic guard tests, Bash syntax, and patch application/reversal were
checked during preparation. Real systemd-user launch and kernel enforcement need
verification on the Linux Mint machine with --check; no overload test was performed.
No Rust rebuild is needed for this patch.

Documentation used:
- https://www.freedesktop.org/software/systemd/man/255/systemd.resource-control.html
- https://www.freedesktop.org/software/systemd/man/255/systemd.scope.html
- https://www.freedesktop.org/software/systemd/man/255/systemd-run.html
