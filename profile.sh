#!/usr/bin/env bash

# loop through all datfiles
# match them against every file in the folder

fd '.*\.dat$' "${DAT_PATH}" | while read -r d; do
    echo "$d"
    fd . "${ROM_PATH}" -t d | while read -r r; do
        RUSTFLAGS="-Cprofile-generate=/tmp/pgo-data -Awarnings -C link-arg=-s" cargo run --release "${d}" "${r}" "${OUT_PATH}"
    done
done