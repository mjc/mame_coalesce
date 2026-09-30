#!/usr/bin/env bash
set -euo pipefail

corpus_root=${1:-target/profiling/external-catalogs-2026-09-29}
output_root=${2:-target/profiling/catalog-import-flamegraphs-2026-09-29}
profile_build_root=target/catalog-import-profile-build
binary="$profile_build_root/profiling/examples/catalog_import_profile"

for profiling_tool in perf inferno-collapse-perf inferno-flamegraph; do
  if ! command -v "$profiling_tool" >/dev/null 2>&1; then
    echo "$profiling_tool is required; run this through devenv's profiling profile" >&2
    exit 127
  fi
done
if [[ ! -f "$corpus_root/logiqx/PureDOSDAT.xml" ||
      ! -f "$corpus_root/mame-full-test/mame-listxml/mame0289.xml" ||
      ! -d "$corpus_root/softwarelists" ||
      ! -f "$corpus_root/clrmamepro/Atari-2600.dat" ||
      ! -f "$corpus_root/no-intro-pc/pc-engine.xml" ||
      ! -f fixtures/catalog/clrmamepro/sample.dat ||
      ! -f fixtures/catalog/no-intro/pc-xml-synthetic.xml ]]; then
  echo "profiling corpus is incomplete: $corpus_root" >&2
  exit 2
fi
if [[ -e "$output_root" ]]; then
  echo "refusing to overwrite existing profiling output: $output_root" >&2
  exit 1
fi

mkdir -p "$output_root/flamegraphs" "$output_root/db" "$output_root/perf"
cargo build --locked --profile profiling --target-dir "$profile_build_root" \
  --example catalog_import_profile

profile_import() {
  local format=$1
  local graph=$2
  local database=$3
  shift 3
  echo "Profiling format=$format documents=$# graph=$graph"
  local perf_data="$output_root/perf/$graph.data"
  local record_status=0
  perf record \
    --all-user \
    --event cycles:u \
    --freq 997 \
    --call-graph dwarf,8192 \
    --mmap-pages 128 \
    --output "$perf_data" \
    -- "$binary" "$format" "$database" "$@" || record_status=$?
  bash scripts/render_catalog_flamegraph.sh \
    "$perf_data" "$output_root/flamegraphs/$graph.svg" "Catalog import: $format"
  return "$record_status"
}

profile_import logiqx logiqx-puredos \
  "$output_root/db/logiqx.sqlite3" \
  "$corpus_root/logiqx/PureDOSDAT.xml"
shopt -s nullglob
software_lists=("$corpus_root"/softwarelists/*.xml)
if [[ ${#software_lists[@]} -eq 0 ]]; then
  echo "no software-list XML inputs found under $corpus_root/softwarelists" >&2
  exit 2
fi
profile_import software-list mame-softwarelists-all \
  "$output_root/db/mame-softwarelists.sqlite3" \
  "${software_lists[@]}"
profile_import clrmamepro clrmamepro-fixture \
  "$output_root/db/clrmamepro.sqlite3" \
  fixtures/catalog/clrmamepro/sample.dat
profile_import no-intro-pc-xml no-intro-pc-xml-fixture \
  "$output_root/db/no-intro-pc-xml.sqlite3" \
  fixtures/catalog/no-intro/pc-xml-synthetic.xml
profile_import clrmamepro clrmamepro-atari2600 \
  "$output_root/db/clrmamepro-atari2600.sqlite3" \
  "$corpus_root/clrmamepro/Atari-2600.dat" ||
  echo "real ClrMamePro import failed; its CPU graph was retained" >&2
profile_import no-intro-pc-xml no-intro-pc-engine-mirror \
  "$output_root/db/no-intro-pc-engine-mirror.sqlite3" \
  "$corpus_root/no-intro-pc/pc-engine.xml"
profile_import machine mame-listxml-full \
  "$output_root/db/mame-listxml.sqlite3" \
  "$corpus_root/mame-full-test/mame-listxml/mame0289.xml"

printf '\nFlamegraphs: %s/flamegraphs\n' "$output_root"
printf 'Raw perf data: %s/perf\n' "$output_root"
printf 'Profile databases: %s/db\n' "$output_root"
