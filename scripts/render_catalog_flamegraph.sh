#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 3 || $# -gt 4 ]]; then
	echo "Usage: scripts/render_catalog_flamegraph.sh <perf-data> <svg> <title> [same-boot-kallsyms]" >&2
	exit 2
fi

perf_data=$1
svg_path=$2
title=$3
kallsyms=${4-}
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)

if [[ -n $kallsyms && ! -r $kallsyms ]]; then
	echo "kallsyms file is not readable: $kallsyms" >&2
	exit 2
fi

perf script -i "$perf_data" |
	awk -v kallsyms="$kallsyms" -f "$script_dir/filter_perf_user_stacks.awk" |
	inferno-collapse-perf --addrs |
	inferno-flamegraph --deterministic --minwidth 0 --title "$title" >"$svg_path"
