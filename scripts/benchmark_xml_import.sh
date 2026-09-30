#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "Usage: scripts/benchmark_xml_import.sh <benchmark-binary> <corpus-dir> [runs]" >&2
  exit 2
}

[[ $# -ge 2 && $# -le 3 ]] || usage
binary=$1
corpus_root=$2
runs=${3:-3}
[[ -x "$binary" ]] || { echo "benchmark binary is not executable: $binary" >&2; exit 2; }
[[ -f "$corpus_root/logiqx.dat" && -f "$corpus_root/machine.xml" && -f "$corpus_root/software-list.xml" ]] || {
  echo "benchmark corpus is incomplete: $corpus_root" >&2
  exit 2
}
[[ "$runs" =~ ^[1-9][0-9]*$ ]] || { echo "runs must be a positive integer" >&2; exit 2; }
time_bin=$(type -P time || true)
[[ -n "$time_bin" ]] || { echo "GNU time is required to measure peak RSS" >&2; exit 2; }

run_root=$(mktemp -d "${TMPDIR:-/tmp}/mame-xml-import.XXXXXX")
trap 'rm -rf -- "$run_root"' EXIT

for format in logiqx machine software-list; do
  case "$format" in
    logiqx) input="$corpus_root/logiqx.dat" ;;
    machine) input="$corpus_root/machine.xml" ;;
    software-list) input="$corpus_root/software-list.xml" ;;
  esac
  for ((run = 1; run <= runs; run++)); do
    database="$run_root/$format-$run.sqlite"
    timing="$run_root/$format-$run.time"
    echo "format=$format run=$run"
    "$time_bin" -f 'peak_rss_kib=%M' -o "$timing" \
      "$binary" "$format" "$database" "$input"
    cat "$timing"
  done
done
