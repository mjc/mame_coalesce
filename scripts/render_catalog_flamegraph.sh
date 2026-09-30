#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 3 ]]; then
  echo "Usage: scripts/render_catalog_flamegraph.sh <perf-data> <svg> <title>" >&2
  exit 2
fi

perf_data=$1
svg_path=$2
title=$3
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)

perf script -i "$perf_data" |
  awk -f "$script_dir/filter_perf_user_stacks.awk" |
  inferno-collapse-perf |
  inferno-flamegraph --deterministic --title "$title" >"$svg_path"
