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
