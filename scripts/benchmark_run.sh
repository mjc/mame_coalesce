#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/benchmark_run.sh --dat <path> --source <path> --out-root <path> --jobs <N> [options]

Options:
  --runs <N>                  Number of hyperfine runs (default: 5)
  --mode <parent-bundles|per-game>
                              Build mode (default: parent-bundles)
  --compression <deflate|store>
                              ZIP compression (default: deflate)
  --strict                    Pass --strict (default)
  --no-strict                 Do not pass --strict
  --db <path>                 Database path (default: target/profiling/<out-root-basename>.db)
  --report <path>             Markdown report path (default: target/profiling/reports/run-jobs-<N>.md)
  --json <path>               hyperfine JSON path (default: target/profiling/reports/run-jobs-<N>.json)
  -h, --help                  Show this help

Run through Nix:
  nix develop -c bash scripts/benchmark_run.sh --dat <path> --source <path> --out-root target/profiling/perf-out-jobs-1 --jobs 1
USAGE
}

dat_path=
source_path=
out_root=
jobs=
runs=5
mode=parent-bundles
compression=deflate
strict_flag=(--strict)
db_path=
report_path=
json_path=

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
    --out-root)
      require_value "$1" "${2:-}"
      out_root=$2
      shift 2
      ;;
    --jobs)
      require_value "$1" "${2:-}"
      jobs=$2
      shift 2
      ;;
    --runs)
      require_value "$1" "${2:-}"
      runs=$2
      shift 2
      ;;
    --mode)
      require_value "$1" "${2:-}"
      mode=$2
      shift 2
      ;;
    --compression)
      require_value "$1" "${2:-}"
      compression=$2
      shift 2
      ;;
    --strict)
      strict_flag=(--strict)
      shift
      ;;
    --no-strict)
      strict_flag=()
      shift
      ;;
    --db)
      require_value "$1" "${2:-}"
      db_path=$2
      shift 2
      ;;
    --report)
      require_value "$1" "${2:-}"
      report_path=$2
      shift 2
      ;;
    --json)
      require_value "$1" "${2:-}"
      json_path=$2
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

if [[ -z "$dat_path" || -z "$source_path" || -z "$out_root" || -z "$jobs" ]]; then
  echo "--dat, --source, --out-root, and --jobs are required" >&2
  usage >&2
  exit 2
fi

if [[ ! "$jobs" =~ ^[0-9]+$ ]]; then
  echo "jobs must be a non-negative integer" >&2
  exit 2
fi

if [[ ! "$runs" =~ ^[1-9][0-9]*$ ]]; then
  echo "runs must be a positive integer" >&2
  exit 2
fi

case "$mode" in
  parent-bundles|per-game) ;;
  *)
    echo "invalid --mode: ${mode}" >&2
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

for command in cargo git hyperfine jq nproc; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "missing required command: ${command}" >&2
    exit 127
  fi
done

case "$out_root" in
  target/profiling/*|./target/profiling/*|"$(pwd)"/target/profiling/*) ;;
  *)
    echo "--out-root must be under target/profiling for safe cleanup: ${out_root}" >&2
    exit 2
    ;;
esac

out_root="${out_root%/}"
out_base="$(basename "$out_root")"

if [[ -z "$db_path" ]]; then
  db_path="target/profiling/${out_base}.db"
fi

case "$db_path" in
  target/profiling/*|./target/profiling/*|"$(pwd)"/target/profiling/*) ;;
  *)
    echo "--db must be under target/profiling for safe cleanup: ${db_path}" >&2
    exit 2
    ;;
esac

mkdir -p target/profiling/reports "$(dirname "$out_root")" "$(dirname "$db_path")"

if [[ -z "$report_path" ]]; then
  report_path="target/profiling/reports/run-jobs-${jobs}.md"
fi

if [[ -z "$json_path" ]]; then
  json_path="target/profiling/reports/run-jobs-${jobs}.json"
fi

mkdir -p "$(dirname "$report_path")" "$(dirname "$json_path")"

strict_display=0
if [[ ${#strict_flag[@]} -gt 0 ]]; then
  strict_display=1
fi

run_command=(
  cargo run --quiet --profile profiling --
  --database-path "$db_path"
  run
  --dat "$dat_path"
  --source "$source_path"
  --out "$out_root"
  --jobs "$jobs"
  --mode "$mode"
  --compression "$compression"
  "${strict_flag[@]}"
)

command_string="$(printf '%q ' "${run_command[@]}")"
command_string="${command_string% }"

prepare_command=$(printf 'rm -rf %q %q %q %q %q' \
  "$out_root" \
  "$db_path" \
  "${db_path}-wal" \
  "${db_path}-shm" \
  "${db_path}-journal")

echo "Benchmarking: ${command_string}" >&2
echo "Preparing each run with: ${prepare_command}" >&2
echo "Prebuilding profiling binary" >&2
cargo build --quiet --profile profiling --bin mame_coalesce

hyperfine \
  --runs "$runs" \
  --export-json "$json_path" \
  --prepare "$prepare_command" \
  "$command_string"

mean=$(jq -r '.results[0].mean' "$json_path")
stddev=$(jq -r '.results[0].stddev' "$json_path")
min=$(jq -r '.results[0].min' "$json_path")
max=$(jq -r '.results[0].max' "$json_path")
git_ref="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
dirty="clean"
if ! git diff --quiet --ignore-submodules -- 2>/dev/null || ! git diff --cached --quiet --ignore-submodules -- 2>/dev/null; then
  dirty="dirty"
fi

cat >"$report_path" <<REPORT
# mame_coalesce run benchmark

- DAT: \`$dat_path\`
- Source: \`$source_path\`
- Output root: \`$out_root\`
- Database: \`$db_path\`
- Jobs: \`$jobs\`
- Runs: \`$runs\`
- Mode: \`$mode\`
- Compression: \`$compression\`
- Strict: \`$strict_display\`
- Git: \`${git_ref} (${dirty})\`
- CPU cores: \`$(nproc)\`
- JSON: \`$json_path\`

## Command

\`\`\`sh
${command_string}
\`\`\`

## Timing

| metric | seconds |
| --- | ---: |
| mean | $mean |
| stddev | $stddev |
| min | $min |
| max | $max |

REPORT

echo "Benchmark JSON: ${json_path}" >&2
echo "Benchmark report: ${report_path}" >&2
