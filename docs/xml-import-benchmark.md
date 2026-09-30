# XML import benchmark

These measurements record the earlier quick-xml parser refactor. The subsequent
real-corpus ownership fix and heaptrack comparison are documented in
[Catalog import profiling](catalog-import-profiling.md#ownership-fix-and-controlled-comparison).
The reproduction commands below run the current implementation.

This is an end-to-end parse-plus-import comparison for Logiqx DAT, MAME
`-listxml`, and MAME software-list XML. Each measured run creates a fresh
SQLite database, imports the same deterministic 1,000-record input, and
reports allocator counters and peak resident memory. Results are observations,
not thresholds; this host showed meaningful run-to-run timing variance.

## Reproduce the refactored measurements

From the repository root in the repository's devenv:

```sh
scripts/generate_xml_import_benchmark_corpus.sh target/profiling/xml-import-corpus
cargo build --release --locked --example xml_import_bench
devenv shell -- bash scripts/benchmark_xml_import.sh \
  target/release/examples/xml_import_bench \
  target/profiling/xml-import-corpus 5
```

The corpus contains 1,000 machines, software items, or DAT games per file.
SHA-256 identities:

| Input | SHA-256 |
|---|---|
| `logiqx.dat` | `23cf98bed3a4db82ad5c19a49bad0e288171d2e0469b6f6ec5bcbcc6b27c189e` |
| `machine.xml` | `436d752a768a6e09b36dcd43c8e49daa234ed61cca61d14cb4dbf7073bd80521` |
| `software-list.xml` | `c7ecc93e0de655cd3648ca43c7bddfdb39b752a01d08581ff88e5f3f6e0d24e7` |

## Comparison

Five fresh-database runs per format, baseline first and refactored second,
using the same corpus, benchmark harness, allocator instrumentation, Rust
toolchain, and host. Time and peak RSS show the median of five runs. Allocation
count and allocated bytes are deterministic across these runs. “Allocated
bytes” is `stats_alloc`'s allocation-byte counter; reallocated bytes are
reported separately by the executable but omitted from this compact comparison.

| Format | Median time baseline → refactored | Allocation count baseline → refactored | Allocated bytes baseline → refactored | Median peak RSS baseline → refactored |
|---|---:|---:|---:|---:|
| Logiqx DAT | 118.5 → 104.1 ms (-12.1%) | 302,483 → 96,218 (-68.2%) | 31.95 → 10.73 MB (-66.4%) | 8.34 → 8.46 MiB (+1.5%) |
| MAME machine XML | 120.1 → 100.1 ms (-16.7%) | 139,220 → 90,199 (-35.2%) | 17.93 → 10.40 MB (-42.0%) | 10.82 → 8.54 MiB (-21.0%) |
| MAME software-list XML | 159.5 → 152.2 ms (-4.5%) | 208,233 → 143,211 (-31.2%) | 28.37 → 17.97 MB (-36.7%) | 12.97 → 9.07 MiB (-30.0%) |

Baseline parser revision: `d8ccd3852b9758989c5cb8c05abb10eee425a897`.
The benchmark harness and pinned `stats_alloc` dev dependency were applied to
an isolated detached worktree at that revision; parser/runtime source was left
unchanged. Refactored measurements were taken from this change before commit.
Both versions used Rust `1.98.1 (48a229cea 2026-09-01)`, NixOS
`26.11.20260928.b9d401c`, Linux x86_64, and an AMD Ryzen 9 5950X. The benchmark
binary runs in release mode. Peak RSS is GNU `time` maximum resident set size.

The measured refactor removes full-document position pre-indexing: source
positions advance as the parser consumes events. The three adapters share the
borrowing event reader, while retaining only the current record subtree where
format interpretation needs ordered extension data. These synthetic corpora
are useful for a controlled before/after comparison but do not represent every
real-world catalog; timing gains are modest and host-specific, while the
allocation reductions are consistent.
