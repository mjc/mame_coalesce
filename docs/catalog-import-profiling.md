# Catalog import profiling, 2026-09-29

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
