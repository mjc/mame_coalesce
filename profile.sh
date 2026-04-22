#!/usr/bin/env bash
set -euo pipefail

: "${DAT_PATH:?set DAT_PATH to a directory containing .dat files}"
: "${ROM_PATH:?set ROM_PATH to the ROM source directory}"
: "${OUT_PATH:?set OUT_PATH to the output directory}"

find "${DAT_PATH}" -name '*.dat' -type f | sort | while read -r dat; do
    name="$(basename "${dat}")"
    destination="${OUT_PATH}/${name%.dat}"
    echo "Profiling ${dat} -> ${destination}"
    RUSTFLAGS="-Cprofile-generate=/tmp/pgo-data -Awarnings -C link-arg=-s" \
        nix develop -c cargo run --release -- run \
            --dat "${dat}" \
            --source "${ROM_PATH}" \
            --out "${destination}" \
            --mode parent-bundles
done
