# Catalog import profiling

## History SQL compilation fix, 2026-10-04

This is a history-reader optimization, not an import-writer or release-profile
benchmark. Diagnosis used signed main `3973183` and the real public software
import/diff APIs. A 14,537-byte relationship-review query took
0.337447–0.379859 seconds to prepare with actual published fixture keys, versus
0.000246–0.000887 seconds to execute with no reviews. A separate direct-SQLite
experiment showed that MAME history's temporary-table creation/deletion caused
a cached statement to reprepare. The raw SQL, sources and logs are preserved
in `/tmp/mame-coalesce-verification-perf.5eB68T/`.

Review hydration now starts from the selected numeric relationship owners,
looks up published review IDs, and applies the unchanged readiness checks only
to those IDs. Every 128-ID parameter batch and its tail is processed; this is
not an event cap. Owner/review deduplication preserves chronological events.
MAME attribute history uses snapshot-bound materialized owner CTEs instead of
temporary-schema DDL, and skips family queries when no MAME owners exist.
No stored facts, source positions, publication guards or schema are removed.

The unchanged `every_native_software_family_changes_history` case checks all
31 software field variants. Frozen all-feature debug executables ran with
`--test-threads=1`, alternating before/after, with no concurrent project build
or profiler. All six runs passed. GNU time includes process startup:

| Pair | Before wall seconds | After wall seconds | Before / after peak RSS, KiB |
| --- | ---: | ---: | ---: |
| 1 | 50.14 | 13.94 | 249,664 / 35,720 |
| 2 | 47.69 | 10.91 | 249,976 / 35,028 |
| 3 | 46.71 | 11.99 | 249,664 / 35,816 |

Median wall time is 47.69 versus 11.99 seconds, about 4.0 times faster for this
case. Other host workloads remained active. Earlier isolated baseline runs
were 20.76/21.70/20.83 seconds; they are not mixed into these paired medians.
Do not extrapolate a fixed factor to optimized imports or other formats. RSS
is whole-process memory, not heaptrack's live allocation measure.

The before executable SHA-256 is
`f94b7962d9eb1d14715d11801906f578dcd34c85089e2a96acfc936b002921df`;
after is `c80018dd650ce37c51b76bb2bb6c9a3d9c3d2b016eca219508eff8f83e577371`,
from reviewed source tree `9812eb4793a44e20eb425b947e13fcbf18b2c2d6`.
Frozen executables and paired logs remain beside the diagnosis artifacts.

Four observed RED regressions now pass. Additional checks cover all 32 actual
MAME family owner-query plans, stable temporary schema on repeated reads,
successful retry after an invalid owner read, and 641 published review events
across batch tails, preserving drafts/readiness and replacement chronology.
The 32 selected history/relationship integration tests also pass. Sol medium
adversarial review found no actionable issues in the production/test cut.

MAME rematerializes selected owners for each family; bounded indexed plans do
not prove a net MAME speedup. Raw import SQL is still uncached, and repeated
software hydration remains open. Fresh diagnostic perf caller graphs were
invalid (unknown or missing worker chains), so they are not used as CPU proof.
Existing optimized import profiles below retain their original provenance.

## Partial writer prepared-cache cut, 2026-10-06

Signed/pushed main commit `6a10ebd` replaces two repeated raw-SQL writer shapes:
Logiqx attribute positions (11 fixed owner/table layouts) and software ROM
entries (one fixed 17-bind layout) now use typed Diesel ASTs with static query
IDs. Tests observe prepared-statement cache hits for text, one-integer,
two-integer and software-ROM statements; they also check all 11 Logiqx IDs are
distinct and that a changed value and a nullable-to-NULL bind reach SQLite.
Existing importer, field, publication, rollback and source-order tests pass.

All-feature nextest passed 1,588 tests with three existing skips. The complete
`devenv test` gate passed, including strict Clippy and CLI smoke checks. Sol
medium's adversarial review found no concrete issue.

This is cache-admission evidence, not an import-performance result: no completed
corpus, profiling-mode before/after run, CPU capture or heaptrack comparison was
performed. Other per-record native writes and relationship inserts/lookups are
still outside this cache cut. MAMEC-53 remains open and blocked on shared-schema
acceptance; recheck these writer shapes after MAMEC-55 / PLAN-3 step #1895.

## Streaming native Logiqx checkpoint, 2026-10-03

This capture uses the optimized, symbolized `profiling` build and the native
Logiqx streaming writer, not the historical schema below. Artifacts are in
`target/profiling/logiqx-streaming-20261003/`; the separate build is in
`target/logiqx-streaming-profile-build-20261003/`. Neither existing build output
nor source files were cleaned. `capture.sh`, input hashes, executable hash,
raw traces, SVGs, reports, databases and query-proof source are retained locally
and ignored by Git. Executable SHA-256:
`73e1667134d8e611b1ed6eda44de553074ebe3ea846ce2b3113681709766acdb`.

The four inputs are the downloaded TOSEC C64 D64 demos DAT, TOSEC-ISO Tomy
KISS-Site games DAT, TOSEC-PIX Macintosh iCreate magazines DAT, and PureDOS DAT.
Their byte-exact hashes are in `inputs.sha256`. The C64 input is 18,993,083
bytes and contains 61,454 games and 61,454 ROM declarations. The ISO, PIX and
PureDOS inputs contribute 19/57, 3/3 and 216/21,491 games/media respectively.
An XML `rom` element does not prove that its file is a ROM image: ISO/PIX lists
can describe other media, including documents.

CPU capture used the repository profiling environment, 997 Hz user-space
cycle sampling, 8 KiB DWARF stacks and 1,024 mmap pages. The initial perf
attempt failed before import because its memory-lock allowance was too small;
the empty trace and failure log are preserved. Raising
`kernel.perf_event_mlock_kb` from 516 to 8,192 enabled capture; it was restored
to 516 afterwards. `perf_event_paranoid=1` and `kptr_restrict=0` were unchanged.
Rendering used `scripts/render_catalog_flamegraph.sh`; analysis used our local
`scripts/parse_flamegraph`.

The fresh CPU database published all four documents (`succeeded=4 failed=0`),
with 61,692 games and 83,005 media declarations. SQLite occupies 106,602,496
logical bytes. SQLite quick/FK checks and application integrity passed. With
the external originals temporarily unavailable, the public native query API
returned the expected counts over 126 pages of up to 500 games, with matching
one-game page boundaries. The proof explicitly checked that original loading
failed before querying; originals were restored afterwards. This proves
source-independent interface/count consistency for these four files, not
field-by-field agreement for every real record or full 4,743-file acceptance.
The helper retains first/boundary pages and does not measure one-page-only
memory. Its `declared_positions` count excludes media attribute positions.

Perf recorded 358,617 samples. The collapsed SVG weights reflect event periods,
not that raw sample count: percentages below are event-weighted coverage, not
wall time. `[unknown]` has 0.01% inclusive coverage and `sqlite3_prepare_v3`
has 78.68%. Deepest-frame SQLite/Diesel category coverage is 55.37%; named
SQLite internals also occur in `Other`. Inclusive ancestors/descendants overlap
and must not be added. No before/after speedup is claimed.

Heaptrack used the same executable with a separate fresh database and only the
C64 input. It completed (`succeeded=1 failed=0`) in 600.51 seconds under the
profiler, with 269,775,522 allocation calls, 53.90 MB peak traced live heap
(53,904,458 bytes in `heap-peak.folded`) and 82.61 MB peak RSS including
profiler overhead. Units are decimal. The 83,943,424-byte database has exactly
one publication, 61,454 games and 61,454 media; SQLite quick/FK checks and
application integrity pass. This is a completed-import observation, not a
controlled throughput comparison with the four-input CPU run.

`heap-report.txt` lists the top 100 individual allocation-count, peak and
temporary-allocation stacks with backtrace merging disabled. The largest
peak stack is 33.55 MB through input `read_to_end`; the next is 8.39 MB growing
the compressed sidecar output, followed by 3.66 MB of zstd workspace. These
are acquisition/source-object costs, not a retained tree of every parsed game.
The top allocation-count stack is 5,838,130 calls through SQLite expression
duplication and trigger compilation during `logiqx_native::insert_positions`,
with zero bytes from that stack live at the global heap peak. Together with
the CPU profile this points to repeated statement preparation/compilation
churn; it does not justify truncating input or imposing an arbitrary catalog
memory cap.

The writer releases each completed game before proceeding and publishes only
after validated EOF within the same transaction. Original/decoded input bytes,
coordinate bookkeeping and the current game or ignored subtree can still
require input-dependent memory. Strict pinned-DTD conformance, remaining
formats and full-corpus field/loading/query/performance acceptance remain open.

## Historical captures, 2026-09-29

The seven CPU flamegraphs and their raw perf recordings are under
`target/profiling/catalog-import-flamegraphs-2026-09-29-userspace-all/`. The
machine XML heaptrack recording, peak-heap flamegraph, and text report are under
`target/profiling/catalog-import-heaptrack-2026-09-29/`. These local artifacts
are ignored by Git; keep the downloaded input corpus under
`target/profiling/external-catalogs-2026-09-29/` when cleaning build output.

## Method

- Host: Linux x86_64, AMD Ryzen 9 5950X, 62 GiB RAM, no swap.
- Rust: 1.98.1. `cargo clean --profile profiling` preceded a clean
  `--profile profiling` build. That profile inherits release optimization and
  has `debug = 2`, `strip = false`.
- CPU: `perf record --all-user --event cycles:u --freq 997 --call-graph
  dwarf,8192 --mmap-pages 1024`, followed by `perf script`,
  `inferno-collapse-perf`, and `inferno-flamegraph`. Sampling covers process
  startup, SQLite initialization, and import. The host's
  `kernel.perf_event_paranoid` and `kernel.kptr_restrict` were temporarily set
  to `-1` and `0`, then restored to their original `1` and `1`.
- Rendering removes only unresolved `ffffffff... [unknown] ([unknown])` kernel
  frames from user-space call chains. It retains every sample and every
  user-space frame. The unfiltered SVGs are retained in
  `flamegraphs/unfiltered/`, and the raw recordings in `perf/`.
- Each format uses a fresh database. The software-list process imports all 777
  downloaded MAME 0.289 list XML files into one database.

The following are the original captures, before the ownership fix below.

| Import | Result | CPU graph (`flamegraphs/`) | Largest unknown frame |
| --- | --- | --- | ---: |
| PureDOS Logiqx DAT | 1 snapshot | `logiqx-puredos.svg` | 0% |
| MAME software lists | 776 snapshots; one invalid CRC | `mame-softwarelists-all.svg` | 0.01% |
| ClrMamePro fixture | 1 snapshot | `clrmamepro-fixture.svg` | 0% |
| No-Intro P/C projection fixture | 1 snapshot | `no-intro-pc-xml-fixture.svg` | 0% |
| Real Atari 2600 ClrMamePro DAT | Failed: duplicate set name violates `snapshot_sets` uniqueness | `clrmamepro-atari2600.svg` | 0% |
| PC Engine No-Intro XML mirror | Rejected: not the supported synthetic P/C projection | `no-intro-pc-engine-mirror.svg` | 0.01% |
| Full MAME machine XML | Partial; no snapshot published | `mame-listxml-partial.svg` | 0.40% |

The machine import was stopped when host available memory reached about 5 GiB.
The graph covers the work up to that cutoff, including SQLite persistence, but
does **not** represent a completed machine import. Its raw recording is named
`perf/mame-listxml-partial.data`. The other raw recordings correspond to their
SVG basenames. The small ClrMamePro and No-Intro fixtures mostly measure SQLite
initialization; they are functional coverage, not useful throughput baselines.

The completed Logiqx and software-list CPU profiles spend substantial sampled
cycles in SQLite statement preparation and execution. This is a hotspot
hypothesis, not a measured speedup. The machine profile is partial and includes
different phases, so compare it separately.

## Machine import heap

Heaptrack ran the profiling binary against the full 326,688,140-byte MAME XML.
The 98.34-second trace was stopped at the host memory cutoff during parsing;
the database still held only the retained document. Heaptrack reported a
12.37 GB peak traced heap, 14.37 GB peak RSS including profiler overhead,
and 190,950,921 allocation calls. Its "leaked" total reflects interruption
while the import still owned its in-memory catalog; it is not evidence of a
memory leak after a completed import.

The largest concrete retained allocation stack is 2.31 GB across 3,658,095
calls through `serde_json::map::insert` and `mame::extension` at
`src/mame.rs:516`, where XML extension elements are converted to
`serde_json::Value`. Several adjacent large stacks follow the same conversion
through different serializer branches. The parser also accumulates all parsed
machines and extensions in `MameCatalog` before publishing. Heaptrack's `G`
and `M` units here are decimal: the original peak folded stacks sum to
12,373,918,224 bytes (11.52 GiB).

Artifacts: `mame-listxml.heaptrack.gz.zst` is the raw trace;
`mame-listxml-heap-report.txt` is the detailed allocation report;
`mame-listxml-heap-peak.folded` and `mame-listxml-heap-peak.svg` show retained
bytes at the recorded peak.

## Ownership fix and controlled comparison

The reader already borrowed its ordinary UTF-8 input. The large retained
ownership came later: each unknown `Element` subtree expanded into a second
tree of `serde_json::Value` maps, arrays, keys, and strings. The importer kept
those trees for every machine until parsing finished, then serialized them
again for snapshot extensions and diagnostics. Tagged text/element wrappers
made millions of small JSON objects. The original heap report also attributed
793.05 MB and 449.49 MB of peak allocations to growth of machine extension
vectors.

The importer now consumes each machine into the existing storage helpers before
reading the next record. `ExtensionValue` holds JSON encoded directly from the
XML subtree, preserving the previous canonical JSON bytes and mixed-content
order. Snapshot storage and import diagnostics borrow that encoding. Metadata
values move into storage records, and binary/text bind parameters borrow their
values. The whole-catalog collecting adapter exists only in tests.

`ValidatedMame<S>` is produced only after the closing root and EOF validate;
publication accepts that state. All record writes belong to the same transaction,
so late parse errors roll them back while retaining the acquired document and a
failed-run diagnostic. Storage errors remain storage errors. Forward merge
references resolve from persisted records after parsing, using database keyset
pagination and the shared relationship writer. XML is traversed once. These
changes introduce no input cap or truncation. The decoded document and duplicate
machine-name index still live for the import; parsed record payloads do not.

The controlled input is the first 1,000 complete machines from the downloaded
MAME 0.289 XML, with its declaration/DTD/root preserved and a closing `</mame>`:
7,891,100 bytes, SHA-256
`ff487b3d16d2df04be01285e1936c09bfaec48e369b26b85d8140160ca3f09b6`.
Both binaries use the same optimized profiling build and fresh databases.

| Measurement | Before | After |
| --- | ---: | ---: |
| Peak traced heap | 307.16 MB | 19.41 MB |
| Peak RSS, without heaptrack | 355.21 MiB | 20.62 MiB |
| Allocation calls, including SQLite | 10,282,455 | 8,354,735 |
| Wall time, without heaptrack | 10.20 s | 10.93 s |

These are single runs, not a throughput claim. Peak heap fell 93.7%, peak RSS
94.2%, and allocation calls 18.7%. The new largest peak stack is SQLite's copy of
the retained source document (7.89 MB), rather than accumulated extension trees.

Before/after persisted data were compared in both directions, including duplicate
occurrence counts and exact JSON bytes, excluding generated IDs and timestamps:
1,000 sets, 10,822 assets, 105,164 extensions, 23,396 relationships, and 105,164
diagnostics all match. Regression coverage includes a malformed tail after a
delivered record, late duplicate rollback, injected storage failure, forward
merges across database pages, namespace/entity/mixed-content preservation,
idempotent reimport, canonical JSON encoding, and invalid content before the root.
Root boundaries accept XML whitespace only (space, tab, CR, LF), rejecting
Unicode whitespace such as NBSP before or after the root. The streaming,
canonical-encoding, invalid-prefix, and XML-whitespace regressions each had an
observed failing result before their implementation fix.

Artifacts are in `target/profiling/xml-ownership-2026-09-29/`: the saved
`import-before` binary, `machines-1000.xml`, before/after databases, GNU time
reports, heaptrack traces/reports, and `before-peak.svg` / `after-peak.svg`.

To reproduce after rebuilding with `cargo build --locked --profile profiling
--example catalog_import_profile`, enter the repository's profiling shell and
use fresh output database/trace paths:

```sh
heaptrack --record-only -o /path/to/new-trace \
  target/profiling/examples/catalog_import_profile machine \
  /path/to/new-database.sqlite3 /path/to/machines.xml
heaptrack_print -f /path/to/new-trace.zst --flamegraph-cost-type peak \
  -F /path/to/peak.folded
inferno-flamegraph --colors mem --countname bytes < /path/to/peak.folded \
  > /path/to/peak.svg
```

### Full machine corpus

The ownership-refactored profiling binary completed the full MAME 0.289 XML:
326,688,140 bytes, SHA-256
`340f4e9362ec1b5f208de43a6330dd3551dfa63b3d67bf198e540380b03fbeeb`.
It reported `format=mame-listxml succeeded=1 failed=0` and exited successfully.
GNU time measured 22m31.09s wall time and 642,772 KiB maximum RSS (627.7 MiB),
without heaptrack. The resulting database is 6,361,706,496 bytes. This is a
completed full-file measurement, not a projection from the controlled sample;
the earlier 12.37 GB heaptrack capture was interrupted and used a different
measurement method, so no full-file percentage comparison is claimed.

The committed database has one successful run and one published snapshot:

| Stored records | Count |
| --- | ---: |
| Machines | 50,368 |
| Assets | 373,154 |
| Extensions | 5,810,344 |
| Relationships | 1,014,360 |
| Diagnostics | 5,810,344 |

The machine count matches the source. Read-only `PRAGMA integrity_check` returned
`ok`, and `PRAGMA foreign_key_check` returned no violations.

The binary is the same ownership-refactor build used in the controlled
comparison; the subsequent invalid-root-content fixes were validated by the
regression tests and complete `devenv test` gate. Full-run artifacts are
`full-after.sqlite3` and `full-after.time` alongside the controlled traces.

## Reproduce

Build and profile in the repository's devenv:

```sh
devenv --profile profiling shell -- bash scripts/profile_catalog_imports.sh \
  target/profiling/external-catalogs-2026-09-29 \
  target/profiling/catalog-import-flamegraphs-new-run
```

The helper compiles into `target/catalog-import-profile-build`, separately from
the corpus and graphs. Clean only that build with:

```sh
cargo clean --profile profiling --target-dir target/catalog-import-profile-build
```

The original full machine import exceeded the memory available during the
baseline capture. The ownership comparison above measures the replacement
pipeline. The profiling source and renderer are
`examples/catalog_import_profile.rs`, `scripts/profile_catalog_imports.sh`,
and `scripts/render_catalog_flamegraph.sh`.

## Fresh full machine import: storage and CPU/heap profiles, 2026-09-30

A separate clean-database import of that same 326,688,140-byte MAME 0.289
machine XML completed successfully (`succeeded=1 failed=0`). The CPU-profiled
database file was 2,641,801,216 bytes (2.46 GiB); the compressed
retained-document sidecar was 15,305,382 bytes. Together they total
2,657,106,598 bytes, about 8.1 times the source XML. This is a fresh
measurement and replaces neither the earlier 6,361,706,496-byte run nor its
artifacts. The 2.66 GB result still fails the storage goal; these profiles
were captured before compacting source relationship assertions.

The CPU capture sampled 515,644 user-space cycle samples across a 568.6-second
sample interval. The SQLite/Diesel category accounts for 77.42% of exclusive
samples; `sqlite3_prepare_v3` accounts for 57.83% inclusive coverage. The
inclusive function percentages overlap. The dominant path is repeated SQLite
statement preparation/execution during relational inserts, making prepared
statement reuse the first CPU optimization to measure. The raw perf recording
is 4,343,461,784 bytes; the rendered flamegraph is `mame-final.svg`.

Heaptrack completed the same full import in a separate fresh database. It
recorded 772,859,173 allocations. The largest peak allocation stack is input
buffer growth in `std::io::default_read_to_end` via
`document_input::read_bounded` and `documents::retain_with_options`: 536.87 MB
across 15 `Vec` growth calls, for a 326.7 MB source. That points to avoidable
transient reallocations while retaining the source; it is not evidence that the
XML parser builds a second whole-document tree. SQLite's allocator dominated
call count, with 563,612,089 calls. The trace is 101,779,462 bytes compressed.

All artifacts are in
`target/profiling/mamec55-full-2026-09-30-8VXYGN/`: `mame-final.data`,
`mame-final.svg`, `mame-heap.trace.zst`, `mame-heap.report.txt`,
`mame-heap.peak.folded`, `mame-heap.peak.svg`, and the separate CPU/heap
databases. `scripts/parse_flamegraph ... summary` now classifies SQLite's
internal parser/B-tree symbols (`yy_reduce`, `exprDup`, `getRowTrigger`,
`getPageNormal`, `balance`, and related frames) with SQLite/Diesel instead of
leaving them in `Other`.

## Fresh full import after relationship compaction, 2026-09-30

Commit `9c44170` replaced JSON source-assertion payloads and serialized endpoint
keys with typed endpoint columns, relational evidence nodes, and support rows.
The profiling-build binary then imported the same source into a separate fresh
database at `target/profiling/mamec55-postcompact-2026-09-30-5ffqQN/`:

| Measurement | Result |
| --- | ---: |
| Source XML | 326,688,140 bytes |
| SQLite database | 2,040,233,984 bytes |
| Compressed source sidecar | 15,305,382 bytes |
| Database + sidecar | 2,055,539,366 bytes (6.29× source) |
| Wall time | 6:20.92 |
| Maximum RSS | 343,032 KiB |

The import succeeded. `PRAGMA integrity_check` returned `ok`; the foreign-key
check returned zero violations. Compared with the pre-compaction CPU-profiled
database plus sidecar (2,657,106,598 bytes), this saved 601,567,232 bytes
(22.6%). It is a material reduction but still far above the 326,688,140-byte
acceptance limit.

The remaining largest `dbstat` entries are live table/index data, not free
pages. Several primary-key tables duplicate substantial data in their indexes:
the database contains 1,079,384 relationship assertions, 1,830,737 switch
values, 845,760 machine specification elements, and 838,479 machine
dependencies.

| Table/index group | Bytes |
| --- | ---: |
| `relationship_assertions` table + source-snapshot index + primary-key index | 412,884,992 |
| `machine_switch_values` table + unique index | 384,258,048 |
| `mame_machine_spec_elements` table + type index + primary-key index | 317,296,640 |
| `mame_machine_dependencies` table + target index + primary-key index | 284,618,752 |
| `machine_switches` table + primary-key and tag indexes | 165,292,272 |
| `mame_machine_slot_options` table + primary-key index | 124,309,504 |
| `asset_requirements` table + primary-key and name indexes | 121,663,488 |

These results make the next storage work concrete: the relationship table still
stores over one million source assertions, while high-cardinality machine
children repeat the 36-byte snapshot key and set name in both table rows and
indexes. Compact parent IDs, narrower/clustered child tables, and only
query-required indexes are the next candidates; the size target remains open.

## Fresh full import with clustered composite keys, 2026-09-30

The current schema stores 31 composite-primary-key catalog tables as
`WITHOUT ROWID`. `snapshot_publications` remains rowid-backed because disk audit
selects the latest publication by `rowid`; `mame_machine_conditions` remains
rowid-backed because its tagged owner key intentionally contains nullable
columns. A schema test checks all 31 clustered tables.

The profiling-build importer loaded the same 326,688,140-byte source
(SHA-256
`340f4e9362ec1b5f208de43a6330dd3551dfa63b3d67bf198e540380b03fbeeb`)
into a new database. That database was then deleted and reimported once more
from the same source, as requested. The final fresh database is
1,585,537,024 bytes and its retained sidecar is 15,305,382 bytes: 1,600,842,406
bytes combined, about 4.90× the source. Compared with the prior post-compaction
fresh import, this saves 454,696,960 bytes (22.1%). The reimport took 9m35.10s
(6m22.89s user, 2m31.08s system); `PRAGMA quick_check` returned `ok`, with zero
foreign-key violations.

The associated `perf` capture has 306,343 samples and lost four chunks while
recording; its flamegraph is
`target/profiling/mamec55-without-rowid-2026-09-30-b.svg` and the capture is
`target/profiling/mamec55-without-rowid-2026-09-30-b.data`. The database was
reimported after capture into the same schema/source; its final size differs
from the captured run by only 81,920 bytes.

| Table/index group | Before | Clustered | Change |
| --- | ---: | ---: | ---: |
| `machine_switch_values` + primary-key index | 384,258,048 | 210,259,968 | -173,998,080 |
| `mame_machine_spec_elements` + type and primary-key indexes | 317,296,640 | 241,262,592 | -76,034,048 |
| `mame_machine_dependencies` + target and primary-key indexes | 284,618,752 | 215,293,952 | -69,324,800 |
| `mame_machine_slot_options` + primary-key index | 124,309,504 | 71,913,472 | -52,396,032 |
| `machine_switches` + tag and primary-key indexes | 165,292,272 | 163,053,568 | -2,238,704 |
| `asset_requirements` + name and primary-key indexes | 121,663,488 | 124,354,560 | +2,691,072 |

This confirms `WITHOUT ROWID` removes much of the duplicated primary-key B-tree
for tables without expensive secondary indexes. It can be neutral or negative
when secondary indexes must carry the full composite key: `machine_switches`
and `asset_requirements` show that tradeoff. Even this broad pass leaves the
database almost five times larger than the source and is not the acceptance
solution. `sem_context` traced the high-volume writers to
`insert_snapshot_set`, `insert_machine_switches`, and
`insert_mame_machine_specification`; these insert the repeated snapshot/set
strings directly into every child row, and the specification writer rebuilds
its SQL text per element. The next substantial reduction is normalized integer
parent identity for snapshot sets and their children, followed by remeasuring
the required secondary indexes and JSON-free fact representation.

## Fresh full import with compact dependency identity, 2026-09-30

The MAME dependency facts now store the parent `set_id` instead of repeating
`snapshot_key` and `set_name`. Import, dependency resolution, merge lookup, and
snapshot-history reconstruction join that ID back to `snapshot_sets`; source
locations and public set names remain unchanged. The same MAME 0.289 input was
imported into a separate fresh profiling database at
`target/profiling/mamec55-set-id-2026-09-30-c/`.

| Measurement | Result |
| --- | ---: |
| Source XML | 326,688,140 bytes |
| SQLite database | 1,445,314,560 bytes |
| Compressed source sidecar | 15,305,382 bytes |
| Database + sidecar | 1,460,619,942 bytes (4.47× source) |
| CPU samples | 577,864 |

`PRAGMA quick_check` returned `ok`; the foreign-key check found zero
violations. Compared with the previous fresh import, the database plus sidecar
is 140,222,464 bytes (8.8%) smaller. The dependency table and target index
shrank from 215,293,952 to 69,677,056 bytes, a 145,616,896-byte reduction.
The new unique `snapshot_sets(set_id)` index costs 5,136,384 bytes; the net
database saving is therefore slightly smaller than the dependency-table gain.

The new CPU flamegraph is `mame.svg` in the artifact directory. The local
`parse_flamegraph` summary classifies 76.93% of exclusive cycle samples as
SQLite/Diesel and 63.86% as `sqlite3_prepare_v3`; statement preparation remains
the largest CPU target.

| Current table/index group | Bytes |
| --- | ---: |
| relationship assertions + source-snapshot and primary-key indexes | 412,827,648 |
| switch values | 210,259,968 |
| machine spec elements + type index | 241,262,592 |
| machine switches + tag index | 163,053,568 |
| slot options | 71,913,472 |
| compact machine dependencies + target index | 69,677,056 |

This is a measured reduction, not acceptance: the database is still 4.47 times
the source size. Continue normalizing high-cardinality set-owned child tables,
remove remaining JSON persistence/encoded identities, and replace duplicate
source assertions with queryable typed facts before the next full-file size
check.

## Fresh full import with compact MAME specification identity, 2026-09-30

The MAME specification element, input-control, analog, device-extension, and
slot-option tables now store `set_id` and local element order rather than
repeating the snapshot key and set name. History reconstruction joins through
`snapshot_sets`; importer results and public set names are unchanged. The full
MAME 0.289 source was reimported to a new database at
`target/profiling/mamec55-spec-id-2026-09-30-e/`.

| Measurement | Result |
| --- | ---: |
| Source XML | 326,688,140 bytes |
| SQLite database | 938,496,000 bytes |
| Compressed source sidecar | 15,305,382 bytes |
| Database + sidecar | 953,801,382 bytes (2.92× source) |
| CPU samples | 515,519 |
| Import elapsed (profiling run) | 9m 41.6s |

`PRAGMA quick_check` returned `ok`, foreign-key violations were zero, and the
database had no free pages. Compared with the preceding fresh import, database
plus sidecar is 204,828,672 bytes (17.7%) smaller. The specification element
table and type index fell from 241,262,592 to 97,845,248 bytes; slot options
fell from 71,913,472 to 23,334,912 bytes. Together those groups save
191,995,904 bytes.

| Current table/index group | Bytes |
| --- | ---: |
| relationship assertions and their indexes | 412,930,048 |
| asset requirements and asset-name index | 124,354,560 |
| switches, values, locations, conditions, and their indexes | 114,843,648 |
| MAME specification elements and type index | 97,845,248 |
| compact machine dependencies and target index | 69,677,056 |
| MAME asset facts | 46,653,440 |
| slot options | 23,334,912 |

The flamegraph is `mame.svg`; the raw capture is `mame.data`. `parse_flamegraph`
classifies 74.90% of exclusive cycle samples as SQLite/Diesel. SQLite statement
preparation remains dominant (`sqlite3_prepare_v3`: 59.63%).

The database is still 2.92 times the source, and database plus sidecar exceeds
the source-size acceptance limit by 627,113,242 bytes. The next structural
targets are the duplicated relationship-assertion payload/indexes and repeated
snapshot/set identities in asset requirements; JSON persistence and encoded
keys also remain unaddressed.

## Accepted size baseline before cross-format redesign, 2026-09-30

After source-dependency assertion compaction, unused index removal, combined
asset-row identity, and derived MAME dependency keys, a fresh profiling import
completed in
`target/profiling/mamec55-derived-dependency-keys-2026-09-30-i/`.
The earlier failed partial run in the `-h/` directory is not this measurement.

| Measurement | Result |
| --- | ---: |
| Source XML | 326,688,140 bytes |
| Fresh SQLite database, migration 27 | 471,031,808 bytes |
| Compressed source object | 15,305,382 bytes |
| Database + source object | 486,337,190 bytes |
| CPU samples | 172,587 |

Source SHA-256 is
`340f4e9362ec1b5f208de43a6330dd3551dfa63b3d67bf198e540380b03fbeeb`.
The completed run has `quick_check=ok`, zero FK violations, zero freelist pages,
812,532 dependency rows and 812,532 distinct derived dependency assertion keys.
The source object's expanded length is the exact source length above. The raw
capture is `perf/mame.data`; the rendered graph is `flamegraphs/mame.svg`.

| Largest table/index object | Allocated bytes |
| --- | ---: |
| MAME machine specification elements | 84,180,992 |
| Generic source relationship assertions | 58,474,496 |
| Combined asset requirement rows | 56,004,608 |
| Machine switch values | 50,348,032 |
| MAME machine dependencies | 42,790,912 |
| Machine switches | 33,480,704 |
| Slot options | 23,334,912 |
| Switch tag/name index | 20,697,088 |
| Relationship source-snapshot index | 18,747,392 |
| Machine specification type index | 13,664,256 |
| Relationship primary-key index | 10,125,312 |

The 810,448 machine specification rows use a 60-column union. Switch values
have 1,783,403 rows; switch definitions have 609,206 rows. Native fields, row
headers, source locations, repeated text and lookup indexes explain the size;
there is no large pool of free pages to reclaim. This is diagnostic evidence,
not a requirement to drop fields or normalize every string.

The user accepted this size baseline and requested a redesign from all input
schemas. The earlier smaller-than-source acceptance statements in this document
describe prior goals. The current field/ownership/query design and cross-format
validation sequence are in
[Catalog schema redesign from all input formats](catalog-schema-redesign.md).
Future comparisons must measure native field completeness and query correctness
across the corpus as well as SQLite size and profiling results.

## Native software-list import baseline, 2026-10-01

A fresh database imported the retained NES, SNES, Atari 2600 and 3DO software
lists with the current greenfield software source-fidelity schema. The importer
was built with `cargo build --locked --profile profiling --example
catalog_import_profile`; the executable retains debug symbols. This is a timed
release-profile import, **not a CPU flamegraph or heaptrack capture**.

| Measurement | Result |
| --- | ---: |
| Successful imports / failures | 4 / 0 |
| Software titles / parts | 9,868 / 9,868 |
| Source ROM entries / disk entries | 14,916 / 5 |
| File declarations / file uses | 13,896 / 14,921 |
| Shared file UUIDs | 13,183 |
| SQLite database | 21,549,056 bytes |
| External document directory, including metadata | 1,098,201 bytes |
| Elapsed time | 58.09 seconds |
| Peak resident memory | 46,664 KiB |

`quick_check` returned `ok`; `foreign_key_check` returned no violations. Every
registry UUID is a 16-byte BLOB. All five disk entries retain CHD-header SHA-1
scope and have no whole-file UUID. The source entries include 734 fill and 286
reload operations; these are uses, not additional file declarations. Wrapper
version text has no copied `catalog_snapshots.declared_version` value.

The complete artifacts are retained at
`/tmp/mame-coalesce-software-native-final.ls9nCc/`: `catalog.sqlite`, its external
document directory, `import.log` and `import.time`. This four-list baseline does
not prove complete format coverage, exact per-flag loader interpretation, or
full-corpus query/CPU/heap acceptance. Those requirements remain open.

### CPU and heap cross-check of the same four lists

Separate fresh CPU and heap databases completed the same four imports with
`succeeded=4 failed=0`; both passed SQLite integrity and foreign-key checks.
Artifacts are retained in `/tmp/mame-coalesce-software-cpu-heap.2ociw0/`, including
the frozen optimized executable `importer` (SHA-256
`8bf9af5a74fc08c071ccd1baa58255d0c4823347b6263d363fa3970b51f89670`),
`software.data`, `software.svg`, `software.heaptrack.zst`, `heap-report.txt`,
allocation stacks and `allocations.svg`.

The user-space CPU capture used 997 Hz cycle sampling and 32 KiB DWARF stacks.
It collected 93,395 samples, with **11 lost chunks** reported by perf; this is a
usable diagnostic capture, not a lossless measurement. `[unknown]` has 0.02%
inclusive coverage. `sqlite3_prepare_v3` has 83.36% inclusive coverage and
generated-column compilation has 46.25%; these nested coverages must not be
added. Deepest-frame SQLite/Diesel coverage is 70.74%. The `Other` category also
contains named SQLite internals, not unresolved symbols.

Heaptrack reports 324,458,691 allocation calls, 38.00 MB peak live heap and
89.65 MB peak RSS including profiler overhead. Its first allocation-count stack
has 6,533,208 calls through SQLite string/opcode construction and statement
preparation into `software_native::insert_rom_entry`, with zero bytes from that
stack live at the global heap peak. This corroborates **statement-compilation
churn**, not hundreds of millions of retained catalog objects. The raw report
lists the top 100 individual stacks for calls, peak bytes and temporary
allocations, with backtrace merging disabled to avoid misleading merged peaks.

Heaptrack finished the capture before its optional automatic GUI launch crashed;
`heaptrack_print` successfully read and analyzed the saved trace. The local
flamegraph parser currently accepts the `samples` title unit, not `allocations`;
its allocation-graph analysis failed explicitly. Use the saved heap report and
allocation SVG rather than treating that failed analysis as measurement proof.
Prepared-statement reuse is the measured next optimization, but no speedup or
allocation reduction is claimed here. Exact loader interpretation, complete
field/query witnesses and full-corpus acceptance remain open.

## Bounded MAME writer investigation — 2026-10-07

Repeated full-file runs were stopped after the initial bulk-write patch and two
later intermediate builds each exceeded the 600-second limit. None establishes
successful full-file acceptance. Their existing artifacts remain under
`target/profiling/mame-bulk-import-20261007`, `-r2`, and `-r3`.

The investigation now uses the existing 1,000-machine sample at
`target/profiling/xml-ownership-2026-09-29/machines-1000.xml` (7,891,100 bytes,
SHA-256 `ff487b3d16d2df04be01285e1936c09bfaec48e369b26b85d8140160ca3f09b6`).
Optional `CATALOG_SQL_TIMINGS=1` on the profiling example records query count,
aggregate duration and average duration by SQL shape, reporting the top twenty.
This diagnostic adds formatting/aggregation overhead; disable it for elapsed-time
comparisons. Times overlap transaction boundaries and must not simply be summed.

A diagnostic debug run before the final specification-writer cache conversion
completed in 23.131 seconds. The leading statements were publication (5.393s,
one call), identity size checks (1.512s, 6,012 calls), and slot-option inserts
(1.489s, 37,980 calls). Full output is retained in
`target/profiling/sql-1000-20261007.log`.

The optimized intermediate executable before cached recursive identity, size and
digest lookups exceeded 30 seconds on this sample. After those changes and fixed
MAME specification/root-set statement reuse, the optimized profiling executable
completed in **13.252 seconds for import, 13.489 seconds including startup**.
Both used fresh databases and no diagnostic instrumentation. These are two
intermediate working-tree cuts, not repeatable committed-revision medians; the
older executable was not frozen, so this is bounded diagnostic evidence only.
The successful measured executable has SHA-256
`4e04287fb34962cb3e461d9fb1feab0b3db6e84e7b4bceb0e3cc808b02ef12fe`.
Database `target/profiling/sql-release-after-1000-20261007.sqlite3` contains
1,000 machines, 11,157 occurrences and one publication; SQLite `quick_check`
returns `ok`, with no foreign-key violations.

Changes reuse prepared statements for native MAME attributes, claims,
relationships and identity lookups; remove a redundant per-asset source-kind
lookup; and batch switches, locations and values in groups of 64. A shared
private fixed-SQL adapter keys statements by SQL text and binds values afresh.
Specification insert SQL is constructed at compile time, not per record.
Runtime regressions cover cache admission, changed nullable values, distinct SQL
shapes, bind counts, native row/order/default-presence fidelity and bounded batch
statement counts. No schema, guard or catalog-fact removal is involved.

The next measured algorithmic fix changes two publication predicates, not the
data model: materialize distinct touched canonical UUIDs before component-wide
size validation, and force snapshot/native-owner qualification before recursive
No-Intro identity checks. This preserves the same source evidence, published
redirects/review exclusions, and separate issued-UUID guard. A query-plan
regression fails on the old per-occurrence size guard and checks both identity
deduplication and native-owner-first ordering/scoped seeks in the replacement.
The trigger definitions change the bundled schema digest; measurements after
this cut must use fresh databases, not silently upgrade an existing cache.

With both publication fixes, a fresh optimized run with no other project checks
running completed in **10.227 seconds for import, 10.452 seconds including
startup** (9.31s user, 0.99s system, 98% CPU). It produced the same 1,000 machines,
11,157 occurrences and one publication, with clean SQLite quick/FK checks.
Artifacts are `target/profiling/sql-release-publication-idle-1000-20261007.sqlite3`
and frozen executable `target/profiling/mame-importer-publication-20261007`
(SHA-256 `09ab8ebd816936d12f42f74f4407a821a7ebeb766a241979f6031bb48c3030f4`).
An earlier run of this same executable overlapping compilation took 19.690s
wall time (60% CPU); it is retained at
`target/profiling/sql-release-publication-after-1000-20261007.sqlite3` but is not
used as a comparable latency measurement. These remain individual sample runs,
not committed-revision benchmark medians or full-file acceptance.

Full-file latency, current-corpus CPU/heap evidence, publication cost and
repeated-identity scaling remain open. Do not extrapolate this sample into a
claim that the full 326,688,140-byte input meets the ten-minute ceiling.

### Attribute presence validation and bounded inserts

The next diagnostic, using the same frozen executable and 1,000-machine input,
identified 76,610 slot-position, 73,538 ROM-position and 45,483 switch-value
position statements. Instrumented import time was 19.113s; that includes SQL
timing overhead and is not a normal-import comparison. Its fresh database and
query totals are retained at
`target/profiling/remaining-sql-baseline-20261007.sqlite3` and the matching `.log`.

Read-only execution of the production attribute-presence predicate on that
database took 2.919s. Replacing two materialized sets and their symmetric
`EXCEPT` comparisons with one grouped presence comparison took 0.629s. Both
returned the same valid result. Each side is already snapshot-filtered, so the
group key contains only the six integer owner/field coordinates, not repeated
snapshot text. The query still sorts once; it does not bypass validation.

The new query-plan regression was observed failing before the replacement.
Correctness tests compare all 1,024 pairs of five-key presence subsets, including
NULL and malformed-field keys, unrelated snapshots, asymmetric duplicate counts,
every owner coordinate and the separate ordinal-collision guard. These four
tests complete in 0.11s. Existing native-owner, publication, corruption,
source-free hydration and history coverage also passes (28 focused tests total).

Attribute-position writes now insert up to 64 rows per statement for one owner,
borrowing source locations and using a fixed stack buffer for converted field
and ordinal binds. Even four-key owners use at most 512 bind parameters. Diesel
caches by generated SQL text, distinguishing both table and batch length; values
are rebound on each execution. All positions and row-level guards remain intact,
and import errors still roll back the enclosing transaction. Tests cover key
arities one through four, empty input, 130 rows across full and tail batches,
changed bindings, and statement counts. Sol 6.1 medium's read-only adversarial
review found no actionable issue in the production query/batching delta.

One matched uninstrumented pair, with no project builds/tests running, took
10.404s before versus 9.483s after for import; process totals including startup
were **10.69s versus 9.73s** (about 9% lower). Before/after user time was
9.30/8.76s and system time 1.02/0.76s. Both fresh databases contain 1,000 machines,
11,157 occurrences and one publication, with identical snapshot keys, clean
`quick_check` results and no foreign-key violations. These are individual runs,
not medians; the full input has not been retried.

Paired databases are `target/profiling/attribute-paired-before-1000-20261007.sqlite3`
and `attribute-paired-after-1000-20261007.sqlite3`; the latter's matching `.log`
retains stdout and process timings. The frozen after executable is
`target/profiling/mame-importer-attribute-batch-20261007`, SHA-256
`bc99764e707d5071050b46e6724e538dd9fb5e3e2f3174302294ca1dafe3e2cf`.

A separate bounded instrumented after run completed in 10.514s, with totals in
`target/profiling/attribute-timed-after-1000-20261007.log`. Publication remained
largest at 2.249s for one statement; identity-size checks took 1.461s across 6,012
calls. Timing instrumentation overhead changes with statement count, so its
19.113s-before/10.514s-after totals must not be presented as normal-import speedup.
Further gains need investigation of those remaining production costs, not
smaller tests or reduced field retention. A supplementary regression verifies
all 33 families have distinct cached SQL and reuse it when their binds change.

A read-only bidirectional `EXCEPT` comparison of all 59 attribute-position
tables found no changed rows between the paired databases (including empty
other-format tables). This checks owner keys, field codes, source ordinals,
lines and columns, not just counts. Results remain in
`target/profiling/attribute-paired-positions-20261007.log`; this is not a
field-by-field comparison of every other native fact table.

The latest complete-gate attempt used `timeout 480 devenv test`: all 681 library
tests passed in 237.63s and subsequent integration targets reported no failures,
but the limit expired during the 77-test `catalog_import` target. The full gate
therefore did **not** pass. Its log is
`target/profiling/attribute-batch-devenv-test-20261007.log`; the unchanged broad
test-runtime problem remains tracked separately in MAMEC-64. No whole-suite
speedup is claimed by this bounded import optimization.

### Cross-machine bulk writes and bounded identity resolution — 2026-10-07

The next cut replaces per-owner writes with batches spanning completed machines,
without removing native facts, source positions, conflict evidence or publication
checks. The streaming writer flushes at 64 machines or 4,096 structural nodes;
an individually larger machine is processed alone, not rejected. Asset identity
resolution works in pages of 64. Digest values use bulk `ON CONFLICT DO NOTHING`,
and digest assertions join their dictionary IDs inside SQL rather than fetching
each ID separately. Machine, specification, switch, BIOS, relationship, asset and
position rows are inserted in parent-before-child batches.

Identity lookup preserves source order: conflicting input ends the current
prefix before any later evidence is written. Candidate fanout stays in SQLite
TEMP tables with file-backed temporary storage; Rust receives bounded summaries,
and SQL persists every conflict candidate and its evidence. There is no new
acceptance cap or whole-catalog Rust cache. Batch statement-cache admission is
bounded; identity fact reads pad UUID bindings to power-of-two arities, with at
most 16 cached read templates per connection.

The repository's SQLite 3.53.3 build previously enabled
`SQLITE_ENABLE_STMT_SCANSTATUS`, adding opcode cycle-accounting overhead even
without import instrumentation. Removing that compile option, without changing
SQLite version or application guards, reduced the frozen pre-bulk executable's
sample import from 9.483s to 6.704s. The bulk writer must be compared against that
6.704s control to separate application gains from the native build change.

| Same 1,000-machine input, fresh databases | Import seconds | Process wall seconds |
| --- | ---: | ---: |
| Frozen pre-bulk writer, original SQLite build | 9.483 | 9.73 |
| Same frozen writer, scanstatus disabled | 6.704 | 6.93 |
| Bulk writer, indexed identity hydration | 6.577 | 6.79 |
| Bulk writer, reusable identity read plans | 5.676 | 5.90 |

The final run used 4.75s user, 0.97s system and 69,120 KiB peak RSS. This is about
15% less import time than the native-build-matched control, not a benchmark
median. The final binary SHA-256 is
`9afa54da646df184a55f65eb4da394ca8147c6efd9230829ee108575494150b3`;
its build path is
`target/mamec63-profile-build-20261007/profiling/examples/catalog_import_profile`.
Timings are retained in `target/profiling/bulk-cached-1000-20261007.log` alongside
its fresh `.sqlite3` database.

An instrumented intermediate bulk run executed 12,288 SQL statements; the old
writer's top 20 query groups alone accounted for 282,334 executions. These are
not two complete before/after totals, and instrumented timings are not mixed
with the table above. An initial bulk implementation regressed to 16.778s:
digest hydration joined canonical roots too broadly. Rooted occurrence/assertion
index seeks corrected that regression. A subsequent CPU profile attributed
19.18% inclusive samples to SQLite parsing, motivating the reusable read plans.
Its 6,644 samples had 0.24% unknown stacks. That profile predates the final
read-plan cache change; artifacts are `bulk-indexed-1000-20261007.perf.data`,
`.svg`, `.analysis.txt` and `.summary.txt` under `target/profiling/`.

The native parity check passed all 80 comparisons across 69 tables, including
all 33 MAME position families, against the native-build-matched pre-bulk database.
Both databases contain 1,000 machines, 11,157 asset occurrences, 4,705 content
identities, 21,454 digest assertions and 24,896 relationship facts; UUIDs and
dictionary IDs are compared through their represented facts, not random bytes.
Both passed `quick_check` and foreign-key checks. Reproduction SQL and results
are `target/profiling/bulk-native-parity-20261007.{sql,log,json}`. This comparison
predates only the read-plan caching change.

Focused verification passed 36 affected library tests, then 21 cache/identity
tests including the real query-plan/cache regression, plus 54 integration tests
across ten affected targets. Regressions cover cross-machine SQL call counts,
EOF rollback after a flushed batch, cross-page deduplication, conflicting size
evidence, wide candidate fanout and native field/position retention. Strict
library/test Clippy passed. Two Sol reviewers reported no remaining actionable
production findings after their findings were fixed. No full `devenv test` gate
was run for this cut, and no new full-file or heaptrack measurement is claimed.

The full local MAME 0.289 input contains 50,368 machines and 401,889 XML
ROM/disk/sample declarations; the sample covers roughly 2% of machines, not the
full corpus. Millions of child facts and relationships make machine count alone
a poor cost estimate. The full 326,688,140-byte import still needs a fresh
measurement; these sample timings do not establish the ten-minute ceiling.
Other input formats have not been converted to this cross-owner bulk path.

## Full MAME retry and SQLite page-cache pressure — 2026-10-07

The 50,368-machine count extrapolated from the 5.676-second sample suggested
about 286 seconds. The actual profiling-build full import did not complete
within its 600-second limit. Including recorder shutdown, it ran 609.43 seconds
(221.11 user / 386.54 system) with 1,327,236 KiB process-tree peak RSS. It captured
287,599 samples with zero reported lost samples. This is a stopped import, not
full-file throughput or publication proof. SQLite recovered its hot rollback
journal on normal reopening; `quick_check` returned `ok`, with zero machines
and zero publications. Existing successful artifacts were not overwritten.

Artifacts under `target/profiling/` use the prefix
`bulk-cached-full-20261007`: `.log`, `.perf.data`, `.kallsyms`, `.boot-id`,
`.uptime`, and recovered `.sqlite3`. The original executable is preserved as
`mame-importer-cache2m-20261007`, SHA-256
`9afa54da646df184a55f65eb4da394ca8147c6efd9230829ee108575494150b3`.
It has no ELF build ID; `perf buildid-cache --add` could not cache it. Keep this
frozen executable for analysis of the recording rather than assuming the
rebuildable example executable still matches.

The old user-stack filter discarded unresolved kernel frames, so its `.svg`
and associated analysis exclude kernel-only samples and must not be used as a
whole-run cost breakdown. Same-boot kernel-symbol resolution instead exposes
heavy `pread64` / ZFS read stacks. The preliminary complete-stack rendering is
`.complete.svg`; it still needs the sample/period reconciliation described by
the corrected renderer before its percentages are treated as whole-run totals.
The suspected mechanism is repeated SQLite page-cache misses: connections used
the default `cache_size=-2000`, approximately 2 MiB. Read-only Luna audits found
indexed identity lookup and native per-row guard plans, not a confirmed missing
index or catalog-wide scan. XML source locations advance a cursor rather than
rescanning the document prefix.

The connection setup now requests `main.cache_size=-65536`. This is an on-demand
64 MiB suggested page-cache budget **per connection**, not a process-wide memory
limit. Ten populated pool connections can approach 640 MiB of main-page caches,
plus other allocations. It neither changes input acceptance nor retains input
records in Rust. Foreign keys, file-backed TEMP storage and FULL synchronous
durability are unchanged. [SQLite documents the units, on-demand allocation,
and connection-local lifetime](https://www.sqlite.org/pragma.html#pragma_cache_size).

One fresh, sequential, uninstrumented 1,000-machine pair measured:

| Main cache | Import | Process | User / system | Peak RSS |
| --- | ---: | ---: | ---: | ---: |
| Default, approximately 2 MiB | 5.268s | 5.55s | 4.54s / 0.75s | 69,376 KiB |
| 64 MiB | 4.918s | 5.13s | 4.39s / 0.46s | 125,252 KiB |

These are single runs, not medians or a full-file speedup. Artifacts are
`cache-control-1000-20261007.{sqlite3,log}` and
`cache64m-1000-20261007.{sqlite3,log}`. The 64 MiB frozen executable is
`mame-importer-cache64m-20261007`, SHA-256
`5cbfef01944fd5f3d21f9f207095d8153e4e01a130b490455b70c3cbfc3ae6d6`.

The new file-backed pool test failed first with the original `-2000` setting,
then passed with `-65536`. It holds two distinct physical connections and checks
the cache setting, foreign keys, TEMP policy and synchronous durability on both.
All 23 focused connection/cache/identity tests passed in 0.267 seconds. Sol
medium found no actionable correctness issue in the connection change. This
does not replace the complete development gate or full-corpus acceptance.

The matched 64 MiB full-file retry also hit the 600-second limit: 611.77 seconds
including recorder shutdown, 534.51 user / 75.79 system, and 1,523,224 KiB
process-tree peak RSS. It captured 291,417 samples with zero reported lost
samples. Its artifacts use `cache64m-full-20261007`; normal SQLite recovery
again left zero machines and zero publications, with `quick_check=ok`.
The cache changed the observed workload from predominantly system time to
predominantly SQLite CPU work; it did **not** establish acceptable full-file
throughput. Do not reuse the 286-second extrapolation as an estimate.

The corrected call-stack rendering is `.lossless.svg` / `.lossless.folded`.
Its folded periods sum to **2,368,461,491,546**, exactly the recording's period
sum. Previously, 32,509 header-only records (249,527,002,332 periods, **10.5354%**)
disappeared in the collapser. They now appear explicitly as
`[unavailable_stack]`, rather than being discarded or assigned invented callers.
An additional 0.1372% of the complete recording has an unresolved frame.
Kernel naming requires the explicitly supplied, same-boot `.kallsyms` snapshot;
the renderer never substitutes the current machine's symbols for old recordings.

Those header-only records still contain a sampled instruction pointer.
A separate `perf script -G -F period,ip,sym,dso` extraction recovers these leaf
samples without pretending to recover their callers. The `.leaf.folded` /
`.leaf.svg` artifacts account for the same **2,368,461,491,546** periods, with
zero unresolved **leaf symbols** after same-boot kernel resolution. This is a
leaf-only view, not a reconstructed call graph. `sqlite3BtreeTableMoveto` alone
accounts for 604,852,959,053 periods (25.54%) there. The `.lossless.analysis.txt`
and `.leaf.analysis.txt` reports use this repository's flamegraph parser.

A deliberately bounded, SQL-instrumented 120-second diagnostic on the same full
input uses `cache64m-sql-progress-20261007.{log,sqlite3}` and the frozen executable
`mame-importer-sql-progress-20261007` (SHA-256
`9089935a1e44eac6fd195b23ec04927eadbaab4c54fccee3170003954add48f0`).
It stopped after 120.03 seconds (108.80 user / 10.46 system, 447,584 KiB peak
RSS). The last completed checkpoint contained 180,018 calls across 4,088 SQL
shapes; this is not a completed import or a count of all queries before exit.
The largest candidate-staging digest-query shape consumed 22.247 seconds across
1,195 calls, averaging 18.617 ms per call. Its earlier average was 11.496 ms,
evidence that the cost grows as more matching source occurrences accumulate.
Other arities of the same lookup add further cost.

The captured staging query requested the canonical UUID through the shared
qualification view, invoking a correlated redirect traversal for each matching
prior occurrence; the following staging phase resolves redirects again after
deduplication. The lookup now keeps that qualification view but selects the
issued UUID from the native occurrence owner. Existing staging then resolves
redirects after deduplication. This removes repeated normalization without
duplicating the qualification rules or dropping accepted/rejected evidence,
published redirects, source order or file-backed candidate fanout. Repeated
digest probes and compatibility-fact hydration remain possible next targets;
there is no successful full-corpus result yet.

SQL timing mode now emits cumulative checkpoints every 10,000 completed queries
per physical connection, so an intentionally stopped diagnostic retains query
evidence. This mode formats queries and stores timing shapes; its timings are
diagnostic, not directly comparable to uninstrumented import throughput. The
real-connection callback test failed before this change and passed afterward;
all three profiling-example tests passed in 1.23 seconds. The profiling shell
now supplies GNU awk and Inferno, plus a renderer regression task covering
64-bit kernel addresses, code/data and module boundaries, unknown preservation,
header-only records and period conservation.

## Deduplicated redirect normalization — matched 5,000 machines

A larger prefix exposes growth hidden by the 1,000-machine sample. The input
`target/profiling/machines-5000-20261007.xml` contains 5,000 machines and
39,080,186 bytes (SHA-256
`1b2b60cb81d8e6df9874cd01621d0edc83763934d389663ac618d105e52829b9`).
Both sequential runs use the same optimized profiling build configuration,
native dependencies, 64 MiB connection cache and fresh file-backed databases,
without perf or SQL instrumentation:

| Identity lookup | Import | Process | User / system | Peak RSS |
| --- | ---: | ---: | ---: | ---: |
| Per-occurrence canonical UUID | 45.134s | 45.56s | 36.61s / 4.95s | 221,128 KiB |
| Issued UUID, normalize after staging deduplication | 37.125s | 37.41s | 32.36s / 4.26s | 234,516 KiB |

That is an 8.009-second / 17.7% lower import time for this pair, not a median,
isolated cold-cache experiment or a full-file speedup. Both imported successfully
with zero diagnostics and the same snapshot key. Artifact prefixes are
`cache64m-before-staging-5000-20261007` and
`staged-issued-after-5000-20261007`. The candidate executable is frozen as
`mame-importer-staged-issued-20261007`, SHA-256
`1912f94038bd34c500998b82377095eccefebf18d978ee29180ecc9751acbf71`.

On the baseline database, a frequently repeated SHA-1 has 249 qualifying
occurrences. Selecting their canonical UUID lengths used 23,459 SQLite VM steps;
selecting issued owner UUID lengths used 14,248 (39.3% fewer), with both returning
3,984 bytes. The wall timings of this read-only pair are cache-order-confounded
and are not presented as a speedup. A regression first failed on the old
correlated recursive plan, then passed on the issued-owner plan while still
resolving the candidate to its published redirect root. All 24 focused
connection/cache/bulk-identity tests subsequently passed in 0.377 seconds,
including wide candidates and complete conflict evidence. Sol medium found no
actionable issue in the production lookup change. These checks do not establish
the full-corpus time ceiling or complete the shared catalog-model acceptance.

The 80 typed native parity comparisons across 69 tables all passed on this
5,000-machine pair, including all 33 attribute-position families. Both databases
contain 62,376 occurrences, 25,106 identities and 112,625 digest assertions,
with one publication each; random UUID bytes are compared through represented
facts rather than byte equality. Results are
`target/profiling/staged-issued-parity-5000-20261007.json`, using the existing
`bulk-native-parity-20261007.sql` with its two attached database paths changed
to this pair. Both databases passed `quick_check` and foreign-key checks.

Final focused verification passed 24 library tests (0.363 seconds), 93
integration tests across import, review, MAME evidence publication and attribute
provenance (34.382 seconds), and all three profiling-example tests (7.21
seconds). Strict library/test/example Clippy, Rust formatting, shell checks,
renderer regression fixtures and Nix formatting passed. A full `devenv test`
was not run. None of these results is a completed 50,368-machine import or a
new heaptrack comparison.

## Deduplicated digest probes — 2026-10-07

Candidate staging now materializes the distinct `(algorithm,digest)` keys in
each bounded parameter page, resolves each key once, and joins those matches
back to every original request index. Both lookup branches anchor requested
keys before `digest_values` using `CROSS JOIN`; this changes join ordering as
well as removing repeated probes. It retains the shared accepted-source
qualification view and issued owner UUIDs. Deduplication includes the disputed
flag, so an accepted and disputed match to the same UUID are not conflated.
Existing file-backed staging still resolves published redirects, validates
terminal roots and accumulates complete candidate/evidence fanout. There is no
whole-catalog Rust cache, schema change or reduced integrity policy.

On the same 5,000-machine / 39,080,186-byte prefix and fresh file-backed
databases, the previous issued-UUID executable took 37.125 seconds to import;
the deduplicated-probe executable took **27.268 seconds**, a 26.6% reduction
for this single sequential comparison. The new process took 27.50 seconds
(23.51 user / 3.45 system), with 234,352 KiB peak RSS, zero diagnostics and the
same snapshot key. Neither run used perf or SQL timing. These are not medians,
an isolated cold-cache experiment or a full-file estimate.

The new artifacts use `target/profiling/dedup-probes-after-5000-20261007`.
The optimized, symbolized executable is frozen as
`target/profiling/mame-importer-dedup-probes-20261007`, SHA-256
`75a2378bac67318593a916bb4000f0648a5b05964dd09d98d72546ee779e4617`.

A separate SQL-instrumented run of the previous executable completed the
5,000-machine prefix with 63,439 calls across 3,471 shapes. The largest digest
staging shape took 3.029 seconds over 467 calls; snapshot publication alone
took 7.357 seconds. This diagnostic is not the uninstrumented timing above.
Read-only predicate probes identified native MAME publication checks as a
substantial remaining cost. Each side of the attribute-presence comparison
contains **1,660,556 positions** on this sample; checking all of them is real
work, not evidence of duplicated source storage.

Materializing the native position CTE, materializing attribute presence, and
replacing the grouped presence comparison with two materialized `EXCEPT`
queries all measured worse than their respective current predicates. None of
these experimental rewrites was adopted. Existing duplicate, NULL-owner,
coordinate, snapshot-isolation and publication checks remain intact. Probe
SQL and logs are retained under `target/profiling/`, including
`publication-guard-costs-5000-20261007` and the
`mame-publication-*-probe-5000-20261007` /
`mame-attribute-*-probe-5000-20261007` prefixes.

### Completed full-file capture

The same executable completed the full **326,688,140-byte MAME 0.289 source**
under perf: **538.481 seconds importing** and **564.90 seconds including
recorder shutdown**, 400.08 user / 160.73 system seconds and 1,333,408 KiB
process-tree peak RSS. It reported success with zero diagnostics. This is the
first successful full-file run after the two timed-out controls above, not a
paired full-file speedup or a median. It meets the ten-minute ceiling for this
run; further performance work is still warranted.

The database contains 50,368 machines, 401,889 occurrences, 158,361 identities,
731,502 digest assertions and one publication. Its snapshot key is
`sha256:8523a3bad9dacf27d6370c85374cedac032908e98937407bebb7f0eb7bc3a3cf`.
`quick_check` returned `ok` and `foreign_key_check` returned no violations.
The matched 5,000-machine pair also passed all 80 typed native comparisons
across 69 tables, including 33 position families; both databases passed
`quick_check`, and the candidate passed the foreign-key check. The parity
artifact is `dedup-probes-parity-5000-20261007.json`.

Full-run artifacts use `target/profiling/dedup-probes-full-20261007`:
`.sqlite3`, `.log`, `.perf.data`, `.kallsyms`, `.boot-id`, `.uptime`,
`.lossless.folded` / `.lossless.svg`, `.leaf.folded` / `.leaf.svg`,
`.sample-periods.txt` and both `.analysis.txt` reports. The kernel-symbol
snapshot was captured before recording, on the same boot. Existing inputs,
databases, frozen executables and profiles were preserved.

Perf captured **256,272 samples**, with zero reported lost samples and
**1,593,821,862,642 sampled periods**. The complete-stack and leaf-only folded
views each sum to exactly that denominator. The complete-stack view explicitly
marks 22.8981% whose callers are unavailable. Another 0.205282% of periods
(3,271,832,601, across 106 folded stacks) contain unresolved frames. The
leaf extraction parsed all 256,272 records and has zero unknown symbols.
Leaf symbols do not reconstruct unavailable callers or unresolved frames.

Using this repository's parser, the leaf-only category breakdown attributes
56.05% to SQLite/Diesel and 12.27% to memory operations. Individual leaf costs
include `sqlite3VdbeExec` (21.03%), `sqlite3BtreeIndexMoveto` (6.78%),
`sqlite3BtreeTableMoveto` (5.78%) and `pcache1Fetch` (4.92%). The call-stack view
also shows statement preparation at 4.89% inclusive coverage. These percentages
are sampled CPU periods, not wall-time attribution, and inclusive caller and
callee percentages overlap. Repeated compatibility-fact hydration across pages
and uncached staging-query preparation are candidates for further bounded
measurement, not newly demonstrated full-file query-cost rankings.

This cut does not include a new heaptrack comparison or the complete
`devenv test` gate, and does not close the other-format writer or shared-model
acceptance work.

Focused verification passed 25 connection/cache/bulk-identity library tests
(0.555 seconds) and 93 import/review/MAME-evidence/attribute-provenance
integration tests (25.658 seconds), after compilation. Strict library, test and
example Clippy and Rust formatting passed. The new repeated-digest regression
first failed against the old plan and now checks expansion to both eligible
requests while excluding an ineligible request, incoming member evidence,
stored member-only evidence and a published rejected whole-file assertion.
Shared query-tracing helpers retain the earlier published-redirect regression;
existing wide-candidate, complete-evidence and dispute tests also pass.
