#!/usr/bin/env bash
set -euo pipefail

corpus_root=target/profiling/synthetic-corpus
if [[ -e "$corpus_root" ]]; then
  echo "benchmark corpus already exists: $corpus_root" >&2
  exit 1
fi

source_root="$corpus_root/source"
dat_path="$corpus_root/synthetic.dat"
mkdir -p "$source_root"
printf '<datafile><header><name>Synthetic benchmark</name><description>Fixed benchmark corpus</description><version>1</version><author>mame_coalesce</author></header>\n' >"$dat_path"

for ((index = 1; index <= 100; index++)); do
  game_name=$(printf 'game-%03d' "$index")
  rom_name=$(printf 'rom-%03d.rom' "$index")
  rom_path="$source_root/$rom_name"
  printf 'synthetic-rom-%03d' "$index" >"$rom_path"
  size=$(wc -c <"$rom_path" | tr -d '[:space:]')
  sha1=$(sha1sum "$rom_path" | cut -d ' ' -f 1)
  md5=$(md5sum "$rom_path" | cut -d ' ' -f 1)
  printf '<game name="%s"><description>%s</description><rom name="%s" size="%s" crc="aabbccdd" md5="%s" sha1="%s"/></game>\n' \
    "$game_name" "$game_name" "$rom_name" "$size" "$md5" "$sha1" >>"$dat_path"
done

printf '</datafile>\n' >>"$dat_path"
printf 'Created 100 ROMs in %s\nDAT: %s\n' "$source_root" "$dat_path"
