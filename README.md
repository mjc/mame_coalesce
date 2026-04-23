# mame_coalesce

`mame_coalesce` imports Logiqx DAT files, scans ROM sources, plans deterministic builds, and writes merged ZIP outputs.

## Status

This project is pre-1.0. The primary verified development and handoff path is
the Nix shell provided by this repository.

The crate is not currently being treated as a crates.io publishing artifact.
Release readiness here means a reviewed GitHub handoff with reproducible local
checks, CI coverage, and explicit release notes.

## Workflow

```sh
nix develop -c cargo run -- dat import fixtures/test.dat
nix develop -c cargo run -- source scan /path/to/roms --jobs 8
nix develop -c cargo run -- build --dat fixtures/test.dat --source /path/to/roms --out /path/to/out
```

After import, `build --dat` accepts either the imported DAT file path or the DAT header name.

The one-shot pipeline is:

```sh
nix develop -c cargo run -- run --dat fixtures/test.dat --source /path/to/roms --out /path/to/out --jobs 8
```

Build modes:

```sh
--mode parent-bundles
--mode per-game
```

ZIP compression defaults to deflate for compatibility. Use `--compression store`
when profiling or when faster, larger ZIP output is preferred.

Defaults:

- `--mode parent-bundles`
- `--compression deflate`
- missing ROMs are reported without failing
- `--strict` exits `2` and writes nothing when required ROMs are missing
- duplicate source matches are resolved deterministically

## External smoke test

To test against downloaded public-domain ROM bundles:

```sh
nix develop -c bash scripts/fetch_public_domain_test_data.sh
```

The script downloads only archive.org items whose metadata is Public Domain Mark
or CC0 by default when `--catalog-tier metadata` is used. Its default curated
catalog also includes archive.org items whose title, description, or upstream
source explicitly describes the ROMs as public-domain/PD ROMs. It generates a
focused Logiqx DAT from the downloaded bytes and runs the one-shot workflow with
`--strict`. It writes all temporary data under `tmp/public-domain-rom-test/`.

Use `--max-roms 0` to include every collected ROM entry.

## Maintenance

Use the Nix shell as the development environment. Plain `cargo test` may fail on systems without `pkg-config`, `libarchive`, and SQLite development libraries.

### CPU Flamegraphs

The profiling helpers mirror the workflow used in `nntp-proxy`, with
`mame_coalesce`-specific categories for hashing, archive work, scan walking,
planning, SQLite/Diesel, and writing. Always run them through Nix.

Baseline single-thread run:

```sh
nix develop -c bash scripts/profile_flamegraph.sh \
  --dat fixtures/<dat>.dat \
  --source <source-dir> \
  --out target/profiling/out-jobs-1 \
  --jobs 1
```

If perf permissions block sampling, retry with `--root`:

```sh
nix develop -c bash scripts/profile_flamegraph.sh \
  --dat fixtures/<dat>.dat \
  --source <source-dir> \
  --out target/profiling/out-jobs-1 \
  --jobs 1 \
  --root
```

Parallel comparison:

```sh
nix develop -c bash scripts/profile_flamegraph.sh \
  --dat fixtures/<dat>.dat \
  --source <source-dir> \
  --out target/profiling/out-jobs-8 \
  --jobs 8
```

Summarize or compare generated flamegraphs:

```sh
nix develop -c bash scripts/parse_flamegraph target/profiling/flamegraphs/run-jobs-1.svg summary
nix develop -c bash scripts/parse_flamegraph target/profiling/flamegraphs/run-jobs-1.svg top 30 0.5
nix develop -c bash scripts/parse_flamegraph target/profiling/flamegraphs/run-jobs-1.svg search planner
nix develop -c bash scripts/parse_flamegraph target/profiling/flamegraphs/run-jobs-1.svg diff target/profiling/flamegraphs/run-jobs-8.svg
```

Benchmark the full `run` workflow with repeated wall-clock samples:

```sh
nix develop -c bash scripts/benchmark_run.sh \
  --dat tmp/perf-public-domain/dats/public-domain-roms.dat \
  --source tmp/perf-public-domain/source-roms \
  --out-root target/profiling/perf-out-jobs-1 \
  --jobs 1 \
  --runs 5
```

Add `--compression store` to measure the opt-in stored-ZIP write path while
keeping default CLI behavior unchanged.

Optional raw perf-data analysis, if `perf.data` is retained or captured
manually:

```sh
nix develop -c sh -lc 'perf script 2>/dev/null | bash scripts/parse_perfdata'
```

## Verification

Required local gate:

```sh
nix develop -c shellcheck scripts/fetch_public_domain_test_data.sh scripts/profile_flamegraph.sh scripts/benchmark_run.sh scripts/parse_flamegraph scripts/parse_perfdata
nix develop -c cargo fmt --check
nix develop -c cargo test
nix develop -c cargo clippy --all-targets --all-features -- -D warnings
nix develop -c cargo package
```

Dependency and maintenance checks:

```sh
nix develop -c cargo update
nix develop -c cargo tree -d
nix develop -c cargo audit
nix develop -c cargo deny check
nix develop -c cargo-udeps udeps --all-targets
```

## Known Operational Constraints

- Running outside Nix requires system `pkg-config`, `libarchive`, SQLite, zlib,
  and related development libraries.
- Current dependency majors include crates that declare MSRVs newer than Rust
  `1.85`; the Nix shell currently builds with Rust `1.92`.
- `Cargo.toml` still declares `rust-version = "1.85"` until the MSRV policy is
  intentionally revised.
- `cargo deny check` may report duplicate dependency warnings under the current
  policy, but the check exits successfully.
