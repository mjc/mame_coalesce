#!/usr/bin/env bash
set -euo pipefail

corpus_root=${1:-target/profiling/xml-import-corpus}
if [[ -e "$corpus_root" ]]; then
  echo "benchmark corpus already exists: $corpus_root" >&2
  exit 1
fi

mkdir -p "$corpus_root"
logiqx="$corpus_root/logiqx.dat"
machine="$corpus_root/machine.xml"
software_list="$corpus_root/software-list.xml"
printf '<datafile><header><name>XML import benchmark</name><description>Generated version 1</description><version>1</version><author>mame_coalesce</author></header>\n' >"$logiqx"
printf '<mame build="xml-import-benchmark-v1">\n' >"$machine"
printf '<softwarelist name="benchmark" description="Generated version 1">\n' >"$software_list"

for ((index = 1; index <= 1000; index++)); do
  name=$(printf '%05d' "$index")
  game="record-$name"
  rom="rom-$name.bin"
  printf '<game name="%s"><description>%s</description><year>2000</year><manufacturer>Benchmark</manufacturer><rom name="%s" size="1024" crc="aabbccdd" md5="900150983cd24fb0d6963f7d28e17f72" sha1="a9993e364706816aba3e25717850c26c9cd0d89d"/></game>\n' \
    "$game" "$game" "$rom" >>"$logiqx"
  printf '<machine name="%s"><description>%s</description><year>2000</year><manufacturer>Benchmark</manufacturer><rom name="%s" size="1024" crc="aabbccdd" md5="900150983cd24fb0d6963f7d28e17f72" sha1="a9993e364706816aba3e25717850c26c9cd0d89d"/></machine>\n' \
    "$game" "$game" "$rom" >>"$machine"
  printf '<software name="%s"><description>%s</description><year>2000</year><publisher>Benchmark</publisher><part name="cart" interface="cart"><dataarea name="rom" size="1024"><rom name="%s" size="1024" crc="aabbccdd" md5="900150983cd24fb0d6963f7d28e17f72" sha1="a9993e364706816aba3e25717850c26c9cd0d89d"/></dataarea></part></software>\n' \
    "$game" "$game" "$rom" >>"$software_list"
done

printf '</datafile>\n' >>"$logiqx"
printf '</mame>\n' >>"$machine"
printf '</softwarelist>\n' >>"$software_list"
sha256sum "$logiqx" "$machine" "$software_list"
