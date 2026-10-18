#!/usr/bin/env bash
# Verify the actual cgroup files, not just requested systemd properties.
# Kept separate so validation failures can be tested without allocating memory.
set -euo pipefail

toon_verify_limits() {
    local base=$1 file expected actual
    while read -r file expected; do
        if ! IFS= read -r actual < "$base/$file"; then
            echo "Cannot read $file; refusing to launch." >&2
            return 1
        fi
        if [[ "$actual" != "$expected" ]]; then
            echo "$file is $actual, expected $expected; refusing to launch." >&2
            return 1
        fi
    done <<'LIMITS'
memory.high 3221225472
memory.max 4294967296
memory.swap.max 536870912
memory.oom.group 1
LIMITS
}

toon_guard_main() {
    local mode=${1:-} cg
    case "$mode" in
        --check|--launch) shift ;;
        *) echo 'Use tools/run-toon-limited.sh to start EffectCraft.' >&2; return 1 ;;
    esac
    cg=$(awk -F: '$1 == "0" {print $3}' /proc/self/cgroup)
    if [[ "$cg" != /* || "$cg" != */effectcraft-limited.scope ]]; then
        echo 'Not inside the expected memory-limited scope; refusing to launch.' >&2
        return 1
    fi
    toon_verify_limits "/sys/fs/cgroup$cg" || return 1
    echo 'Verified kernel limits: 3 GiB throttle, 4 GiB maximum, 512 MiB swap; OOM kills the group.'
    if [[ "$mode" == --check ]]; then return 0; fi
    if (($# == 0)) || [[ ! -f "$1" || ! -x "$1" ]]; then
        echo 'EffectCraft executable is unavailable; refusing to launch.' >&2
        return 1
    fi
    exec "$@"
}

if [[ ${BASH_SOURCE[0]} == "$0" ]]; then
    toon_guard_main "$@"
fi
