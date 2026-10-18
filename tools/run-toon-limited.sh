#!/usr/bin/env bash
# Linux-only launcher; no automatic build or unrestricted fallback.
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
profile=debug
check_only=false
while (($#)); do
    case "$1" in
        --release) profile=release; shift ;;
        --check) check_only=true; shift ;;
        --help|-h)
            echo 'Usage: bash tools/run-toon-limited.sh [--release] [--check] [-- APP_ARGS...]'
            echo '--check verifies live kernel limits without starting EffectCraft.'
            exit 0 ;;
        --) shift; break ;;
        *) break ;;
    esac
done

if [[ $(uname -s) != Linux ]] || [[ $(stat -fc %T /sys/fs/cgroup) != cgroup2fs ]]; then
    echo 'Requires Linux with cgroup v2; refusing to launch without memory limits.' >&2
    exit 1
fi
command -v systemd-run >/dev/null || {
    echo 'systemd-run is unavailable; refusing to launch.' >&2
    exit 1
}

exe="$root/target/$profile/effectcraft"
if ! "$check_only" && [[ ! -f "$exe" || ! -x "$exe" ]]; then
    echo "Executable missing: $exe" >&2
    echo 'Build the selected profile first. No build was started.' >&2
    exit 1
fi

mode=--launch
if "$check_only"; then mode=--check; fi
# A fixed unit name prevents two copies started by this launcher sharing separate
# 4 GiB allowances. Close any instance started outside this launcher first.
cd -- "$root"
exec systemd-run --user --scope --unit=effectcraft-limited \
    --expand-environment=no \
    -p MemoryAccounting=yes \
    -p MemoryHigh=3G \
    -p MemoryMax=4G \
    -p MemorySwapMax=512M \
    -p OOMPolicy=kill \
    /bin/bash "$root/tools/toon-memory-guard.sh" "$mode" "$exe" "$@"
