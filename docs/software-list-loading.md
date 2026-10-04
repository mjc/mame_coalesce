# Approved contract: MAME 0.289 software-list ROM loading

The user approved the checked contract on 2026-10-03: warn and process partial
byte groups like MAME, retaining the full padded group for bounds checks and
the high lane for a reversed residual. Imports retain all source facts even
when executable interpretation fails. This resolves the original proposal's
pending approval and partial-group choice; implementation acceptance is still
open. The historical proposal below records the reviewed source behavior.

This approved contract records behavior observed in the frozen `mame0289` source and defines a checked Rust interpretation. Approval is not a claim that implementation or whole-format acceptance is complete. The DTD admits the 14 `loadflag` values below ([pinned DTD](https://github.com/mamedev/mame/blob/mame0289/hash/softwarelist.dtd#L38)). `softlist.cpp` converts only five layout flags; the others either create control entries or fall through to an ordinary ROM entry with zero layout flags ([pinned parser](https://github.com/mamedev/mame/blob/mame0289/src/emu/softlist.cpp#L709-L772)).

## Ordinary file and stream model

An ordinary ROM entry starts a file group: its nonempty XML `name` identifies the file, `size` is the byte length of this segment, and `size` must be nonempty for the parser to emit an entry. A missing name or size produces a diagnostic and no ordinary entry. For a normal dump, absent CRC or SHA-1 produces a diagnostic, but the parser still emits a named ROM entry with empty hash data. Supplied checksum text is not validated by this branch. For `status="nodump"`, the parser emits only the no-dump marker: supplied checksums are ignored, with a diagnostic when both are nonempty. A physical file may be absent for `nodump`, but the XML name is still required. Absent `offset` becomes zero. The file is opened once for the group. If a physical file exists, its length is checked after the first load/continue/ignore run, before reload groups are replayed. Non-nodump hashes cover the whole file; nodump skips checksum comparison even when a physical file exists. CRC and SHA-1 are metadata on the base ROM entry; control entries do not define independent hashes. Hashing does not use the target region layout or just the bytes written by a segment. ([parser](https://github.com/mamedev/mame/blob/mame0289/src/emu/softlist.cpp#L709-L772), [file processing and verification](https://github.com/mamedev/mame/blob/mame0289/src/emu/romload.cpp#L938-L1036), [verification implementation](https://github.com/mamedev/mame/blob/mame0289/src/emu/romload.cpp#L555-L594))

The file cursor starts at byte 0. A ROM or continue segment reads its declared `size` bytes from the current cursor and writes them at its declared region offset using its effective layout flags. `ignore` advances the loader's expected-length counter but performs no read and no seek, so it does not advance the file cursor. At the end of the first run, MAME compares the actual whole-file size with the sum of the base ROM and its continue/ignore lengths. For non-nodump entries it compares whole-file hashes; nodump bypasses that checksum comparison. It then seeks to byte 0 before processing each reload group. ([`process_rom_entries`](https://github.com/mamedev/mame/blob/mame0289/src/emu/romload.cpp#L938-L1036), [`rom_file_size`](https://github.com/mamedev/mame/blob/mame0289/src/emu/romload.cpp#L360-L377), [`verify_length_and_hash`](https://github.com/mamedev/mame/blob/mame0289/src/emu/romload.cpp#L555-L594))

`rom_file_size` is a separate estimate: for the initial run and each immediately following reload run, it sums the base entry plus that run's continue/ignore entries, then returns the **maximum** run sum. The first run's verification instead uses its own sum. Consequently a smaller reload does not reduce the first-run expected file length; an oversized reload can increase progress accounting without changing the first-run verification sum. `ignore` contributes to both sums despite consuming no bytes. ([pinned implementation](https://github.com/mamedev/mame/blob/mame0289/src/emu/romload.cpp#L360-L377), [first-run verification](https://github.com/mamedev/mame/blob/mame0289/src/emu/romload.cpp#L1009-L1015))

## All DTD `loadflag` values

“Layout” describes byte placement for a segment, independently of that segment's input length. “Hash” identifies the file bytes covered by MAME's verification. `inherit` means MAME's ROM-entry layout flags carry forward to the next control segment. Source-observed behavior is in the table; the Rust recommendations follow below.

| DTD value | File-group start / name rule | Linkage / layout | Cursor and data effect | Length and hash behavior |
|---|---|---|---|---|
| *(omitted)* | Starts group; nonempty XML `name` required. Missing physical file tolerated for `nodump` | Ordinary linear load; no inherited layout | Read `size` at current cursor; write at `offset` (default 0) | Base segment contributes to first-run sum; whole file checked against first-run sum and base entry's CRC/SHA-1 when known |
| `load16_byte` | Starts group; nonempty XML `name` required. Missing physical file tolerated for `nodump` | `ROM_SKIP(1)`; byte-lane placement, not word swapping | Read `size`; scatter bytes with one skipped destination byte between writes | Same base-file verification as ordinary load |
| `load16_word` | Starts group; nonempty XML `name` required. Missing physical file tolerated for `nodump` | **Falls through to zero layout flags**; linear load | Read `size`; linear placement | Same base-file verification |
| `load16_word_swap` | Starts group; nonempty XML `name` required. Missing physical file tolerated for `nodump` | Group 2, reversed | Read `size`; reverse each 2-byte group into region | Same base-file verification |
| `load32_byte` | Starts group; nonempty XML `name` required. Missing physical file tolerated for `nodump` | `ROM_SKIP(3)`; byte-lane placement | Read `size`; scatter bytes with three skipped destination bytes between writes | Same base-file verification |
| `load32_word` | Starts group; nonempty XML `name` required. Missing physical file tolerated for `nodump` | Group 2, skip 2 | Read `size`; place 2-byte groups with two skipped destination bytes between groups | Same base-file verification |
| `load32_word_swap` | Starts group; nonempty XML `name` required. Missing physical file tolerated for `nodump` | Group 2, reversed, skip 2 | Read `size`; reverse each 2-byte group and place with two skipped destination bytes between groups | Same base-file verification |
| `load32_dword` | Starts group; nonempty XML `name` required. Missing physical file tolerated for `nodump` | **Falls through to zero layout flags**; linear load | Read `size`; linear placement | Same base-file verification |
| `load64_word` | Starts group; nonempty XML `name` required. Missing physical file tolerated for `nodump` | **Falls through to zero layout flags**; linear load | Read `size`; linear placement | Same base-file verification |
| `load64_word_swap` | Starts group; nonempty XML `name` required. Missing physical file tolerated for `nodump` | **Falls through to zero layout flags**; linear load | Read `size`; linear placement | Same base-file verification |
| `reload` | No; requires a preceding ROM load | Reload control; inherits previous layout flags | After prior run, seek to byte 0; read `size` and write at this entry's `offset` (default 0) | Its run participates in `rom_file_size` maximum. No second file hash/length verification; base file hashes remain whole-file hashes from the first run |
| `fill` | No | Fill control; no file-layout inheritance | Write `size` bytes of parsed `value` at `offset`; no file read or cursor movement | Not part of a file run; no file size/hash |
| `continue` | No; requires a preceding ROM load | Continue control; inherits previous layout flags | Read `size` from the current cursor; write at this entry's `offset` (default 0) | Adds to current run's expected-length sum; included in first-run verification when in the first run |
| `reload_plain` | No; requires a preceding ROM load | Reload control; **does not inherit** previous layout flags; entry uses its own flags (zero layout flags from the software-list parser) | After prior run, seek to byte 0; read `size` and write at this entry's `offset` (default 0), using non-inherited/default layout | Same reload sizing and no-reverification behavior as `reload` |
| `ignore` | No; requires a preceding ROM load | Ignore control; inherits previous layout flags, though no data is placed | No read, no explicit seek, no destination write; cursor stays where it was | Adds `size` to current run's expected-length sum and to `rom_file_size`; included in first-run length verification, and the file's whole bytes are hashed |

The five explicit layout translations are visible in the parser. The DTD-listed `load16_word`, `load32_dword`, `load64_word`, and `load64_word_swap` are not translated there: with a nonempty name and size they emit ordinary ROM entries with zero layout flags, even when incomplete hashes have produced a diagnostic. They are accepted vocabulary, not implemented layouts in this pinned software-list parser. Do not infer these modes from the DTD spelling. ([parser lines](https://github.com/mamedev/mame/blob/mame0289/src/emu/softlist.cpp#L754-L766), [destination mapping](https://github.com/mamedev/mame/blob/mame0289/src/emu/romload.cpp#L760-L871))

## Chain validity and Rust interpretation

In MAME, continue, ignore, or reload at the top level without a preceding file run is fatal. Controls are consumed only as part of the file-processing loops: continue and ignore belong to the current run; reload starts another run after it. A fill is its own entry, not a file or continuation. A control entry still needs `size` because the software-list parser checks it before dispatch; a fill's missing `value` becomes an empty string and is parsed as zero by the fill path. Control entries do not need a name or CRC/SHA-1 because the parser handles them before the ordinary-ROM checks. An ordinary-layout flag, including a DTD-listed but untranslated one, needs a nonempty XML name and size to emit an entry. Missing normal-dump hashes produce diagnostics without preventing emission; diagnostics must not be confused with strict rejection. ([parser](https://github.com/mamedev/mame/blob/mame0289/src/emu/softlist.cpp#L709-L772), [chain processing](https://github.com/mamedev/mame/blob/mame0289/src/emu/romload.cpp#L938-L1036), [fill behavior](https://github.com/mamedev/mame/blob/mame0289/src/emu/romload.cpp#L874-L901))

Approved checked Rust interpretation:

- Preserve every source attribute's raw CDATA exactly, including malformed numeric strings; never reject source import or drop a source fact because executable interpretation fails. Separately parse size, offset, and fill value into checked optional numeric interpretations. Mark malformed, negative, or out-of-range interpretations invalid and refuse to execute/load that entry, rather than accepting C++ `strtol` truncation/wrap behavior.
- Build explicit file groups. Require each continue, ignore, and reload to attach to a preceding ordinary ROM; do not let a control silently become an independent file. Preserve MAME's distinction between run sum, maximum run sum, and actual cursor advancement.
- Only validated, usable whole-file declarations may supply matching hashes. For a non-nodump recipe, hash the complete source file once using those base-entry assertions; nodump does not fabricate checksum verification. Check the physical file's actual length against the first run sum, while retaining the maximum run sum as a separate progress estimate. Never hash per interleaved segment or reload destination.
- Implement exactly the five observed software-list layouts. For the four untranslated DTD layout names, represent observed behavior as linear/default if strict MAME 0.289 compatibility is the requirement; keep any future intended interpretation as an explicit, separate compatibility change.
- Bounds-check every segment's target mapping, including grouped/skipped writes, before modifying the region. Reject zero-length ROM/fill segments and arithmetic or region-range failures before partial writes.

MAME's historical loader has additional edge behavior that should not be mistaken for a clean format rule: it sums `ignore` into expected size without reading, uses 32-bit `u32`/`int` length calculations in places, and the parser converts sizes/offsets via `strtol` into `u32`. The approved Rust path preserves the observed cursor, grouping, and hash rules while keeping raw source facts and marking unsafe executable interpretations invalid. Partial groups warn and process residual bytes like MAME. A reversed residual occupies the high lane of its full group; bounds checks reserve the padded group extent. ([MAME grouped-load behavior](https://github.com/mamedev/mame/blob/mame0289/src/emu/romload.cpp#L760-L871), [size conversion](https://github.com/mamedev/mame/blob/mame0289/src/emu/softlist.cpp#L724-L725))

## Catalog-backed recipe API

The `mame-softwarelist-declared-text-compat-v4` numeric contract accepts only
unsigned ASCII digits in decimal, leading-zero octal, or `0x`/`0X` hexadecimal.
It rejects signs anywhere, whitespace, trailing junk and values above SQLite's
signed-integer range. Raw CDATA remains stored even when its numeric projection
is absent. The parser, three virtual area/ROM/offset projections and executable
recipes use this same grammar; the former `+010`/`0x+2` sign-handling quirks are
not executable compatibility behavior.

`software_loading::SoftwareLoadPlan::from_area` interprets one data area's
complete native occurrences, obtained through `catalog_software` and
`catalog_files`. Pass both from the same catalog registry generation. It uses
the area's entry IDs and native component order, rejects duplicate, missing,
wrong-area and mixed-edition rows, and checks each control's declaration link.
Recipes borrow names and facts; they neither rewrite the catalog nor store ROM
bytes. Disk areas are not ROM byte-placement recipes.

Each `SoftwareLoadFile` identifies its actual declaration occurrence, not just
a filename. `expected_length` is the first-run sum, `progress_length` is the
maximum run sum, and `read_length` is the greatest actual read cursor. These
are transient derived facts, not duplicate SQLite columns. Matching derives
`software_rom_file_size` from the complete first base/continue/ignore run in
the native tables, not an individual segment or maximum reload run. Invalid
consumed lengths, overflow, gaps or broken owner/use links leave it unknown.
Reload and fill stop that first run before their lengths are considered.
Destination geometry does not determine whole-file size evidence; a valid
named non-nodump base hash may remain usable when size is unknown. This is
source-declared expected content, not observed-byte verification. Fill clears
file ownership even when its own value or placement cannot execute.

The importer resolves each immutable declaration using its complete source
first run and finishes that file group's native facts and conflict evidence
before considering the next declaration. Size conflicts and reviews point to
the declaration, from which the original ordered entries remain reachable.
Reviewed size rejection affects matching without deleting or rewriting those
entries. The former NULL-only declaration-size placeholder is removed, not
filled with a cached calculation. The v4 reading-rules identity qualifies
these size/chain semantics separately from earlier immutable interpretations.

`bind` borrows physical files keyed by declaration. It validates all required
reads before returning the opaque `ReadySoftwareLoad` state. Whole-file
CRC/SHA-1 and length mismatches produce typed, declaration-linked warnings;
malformed assertions or insufficient read bytes prevent execution. Missing
`nodump` files are explicit absent sources, leave their destination bytes
unchanged and never fabricate verification. Supplied `nodump` bytes may load
without checksum comparison. Fill writes belong to the area, not a source file.

`ReadySoftwareLoad::execute` accepts a caller-owned, already initialized region
of exactly the declared length. It performs ordered placement/fill steps,
preserving untouched lanes. It does not open files, allocate regions, choose
missing-ROM random bytes, perform device-bus post-processing, boot emulators,
or change observed-file identity. Returned warnings are transient; durable
import diagnostics and their saved source spans remain a separate API.

Whole-format, catalog-matching and corpus/performance acceptance remain open.
