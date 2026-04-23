#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'USAGE'
Usage: scripts/profile_flamegraph.sh --dat <path> --source <path> --out <path> [options]

Options:
  --jobs <N>                  Worker count to pass to mame_coalesce (default: 1)
  --layout <parent-bundles|per-game>
                              Output layout (default: parent-bundles)
  --compression <deflate|store>
                              ZIP compression (default: deflate)
  --frequency <Hz>            perf sampling frequency (default: 997)
  --db <path>                 Database path (default: target/profiling/flamegraph-run-jobs-<N>.db)
  --svg <path>                SVG output path (default: target/profiling/flamegraphs/run-jobs-<N>.svg)
  --root                      Forward --root to cargo flamegraph
  --title <text>              SVG title (default: mame_coalesce build jobs=<N>)
  --dry-run                   Forward --dry-run to mame_coalesce
  --missing <warn|fail>       Missing ROM policy (default: fail)
  -h, --help                  Show this help

Run through Nix:
  nix develop -c bash scripts/profile_flamegraph.sh --dat <path> --source <path> --out <path>
USAGE
}

dat_path=
source_path=
out_path=
jobs=1
layout=parent-bundles
compression=deflate
frequency=997
root_flag=()
title=
dry_run_flag=()
missing=fail
db_path=
svg_path=

require_value() {
    local option=$1
    local value=${2:-}
    if [[ -z "$value" || "$value" == --* ]]; then
        echo "missing value for ${option}" >&2
        usage >&2
        exit 2
    fi
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dat)
            require_value "$1" "${2:-}"
            dat_path=$2
            shift 2
            ;;
        --source)
            require_value "$1" "${2:-}"
            source_path=$2
            shift 2
            ;;
        --out)
            require_value "$1" "${2:-}"
            out_path=$2
            shift 2
            ;;
        --jobs)
            require_value "$1" "${2:-}"
            jobs=$2
            shift 2
            ;;
        --layout)
            require_value "$1" "${2:-}"
            layout=$2
            shift 2
            ;;
        --compression)
            require_value "$1" "${2:-}"
            compression=$2
            shift 2
            ;;
        --frequency)
            require_value "$1" "${2:-}"
            frequency=$2
            shift 2
            ;;
        --db)
            require_value "$1" "${2:-}"
            db_path=$2
            shift 2
            ;;
        --svg)
            require_value "$1" "${2:-}"
            svg_path=$2
            shift 2
            ;;
        --root)
            root_flag=(--root)
            shift
            ;;
        --title)
            require_value "$1" "${2:-}"
            title=$2
            shift 2
            ;;
        --dry-run)
            dry_run_flag=(--dry-run)
            shift
            ;;
        --missing)
            require_value "$1" "${2:-}"
            missing=$2
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "unknown argument: $1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

if [[ -z "$dat_path" || -z "$source_path" || -z "$out_path" ]]; then
    echo "--dat, --source, and --out are required" >&2
    usage >&2
    exit 2
fi

if ! command -v cargo-flamegraph >/dev/null 2>&1; then
    cat >&2 <<'MSG'
cargo flamegraph is not available.
Run this through: nix develop -c bash scripts/profile_flamegraph.sh ...
MSG
    exit 127
fi

case "$layout" in
    parent-bundles|per-game) ;;
    *)
        echo "invalid --layout: ${layout}" >&2
        exit 2
        ;;
esac

case "$missing" in
    warn|fail) ;;
    *)
        echo "invalid --missing: ${missing}" >&2
        exit 2
        ;;
esac

case "$compression" in
    deflate|store) ;;
    *)
        echo "invalid --compression: ${compression}" >&2
        exit 2
        ;;
esac

if [[ -z "$title" ]]; then
    title="mame_coalesce build jobs=${jobs}"
fi

flamegraph_dir=target/profiling/flamegraphs
if [[ -z "$svg_path" ]]; then
    svg_path="${flamegraph_dir}/run-jobs-${jobs}.svg"
fi
if [[ -z "$db_path" ]]; then
    db_path="target/profiling/flamegraph-run-jobs-${jobs}.db"
fi
mkdir -p "$flamegraph_dir" "$(dirname "$svg_path")"
mkdir -p "$(dirname "$out_path")"
mkdir -p "$(dirname "$db_path")"
rm -rf "$out_path" "$db_path" "${db_path}-wal" "${db_path}-shm" "${db_path}-journal"

echo "Writing flamegraph to ${svg_path}" >&2

set +e
cargo flamegraph \
    --profile profiling \
    --bin mame_coalesce \
    --freq "$frequency" \
    --deterministic \
    --output "$svg_path" \
    --title "$title" \
    "${root_flag[@]}" \
    -- \
    --cache "$db_path" \
    build \
    "$dat_path" \
    "$source_path" \
    "$out_path" \
    --jobs "$jobs" \
    --layout "$layout" \
    --compression "$compression" \
    --missing "$missing" \
    "${dry_run_flag[@]}"
status=$?
set -e

if [[ $status -ne 0 ]]; then
    cat >&2 <<MSG
cargo flamegraph failed with exit status ${status}.
If this is a perf permissions error, retry with:
nix develop -c bash scripts/profile_flamegraph.sh --dat "${dat_path}" --source "${source_path}" --out "${out_path}" --jobs "${jobs}" --root
MSG
    exit "$status"
fi

echo "Flamegraph written: ${svg_path}" >&2
