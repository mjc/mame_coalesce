# mame_coalesce

`mame_coalesce` imports Logiqx DAT files, scans ROM sources, plans deterministic builds, and writes merged ZIP outputs.

## Status

This project is pre-1.0. The primary verified development and handoff path is
the pinned devenv environment provided by this repository.

The crate is not currently being treated as a crates.io publishing artifact.
Release readiness here means a reviewed GitHub handoff with reproducible local
checks, CI coverage, and explicit release notes.
Normal CI and day-to-day verification do not run `cargo package`; packaging is
deferred until crates.io distribution becomes a goal.

## Workflow

The primary workflow is one-shot:

```sh
devenv shell -- cargo run -- build fixtures/test.dat /path/to/roms /path/to/out --jobs 8
```

Common options:

```sh
--layout parent-bundles
--layout per-game
--compression deflate
--compression store
--missing warn
--missing fail
--dry-run
```

Explicit cache maintenance commands are available for advanced workflows:

```sh
mame_coalesce --cache /tmp/coalesce.db cache import fixtures/test.dat
mame_coalesce --cache /tmp/coalesce.db cache scan /path/to/roms --jobs 8
mame_coalesce --cache /tmp/coalesce.db cache build "DAT Header Name" /path/to/roms /path/to/out
```

ZIP compression defaults to deflate for compatibility. Use `--compression store`
when profiling or when faster, larger ZIP output is preferred.

Defaults:

- `--layout parent-bundles`
- `--compression deflate`
- missing ROMs are reported without failing
- `--missing fail` exits `2` and writes nothing when required ROMs are missing
- duplicate source matches are resolved deterministically

## External smoke test

To test against downloaded public-domain ROM bundles:

```sh
devenv shell -- bash scripts/fetch_public_domain_test_data.sh
```

The script downloads only archive.org items whose metadata is Public Domain Mark
or CC0 by default when `--catalog-tier metadata` is used. Its default curated
catalog also includes archive.org items whose title, description, or upstream
source explicitly describes the ROMs as public-domain/PD ROMs. It generates a
focused Logiqx DAT from the downloaded bytes and runs the one-shot workflow with
`--missing fail`. It writes all temporary data under `tmp/public-domain-rom-test/`.

Use `--max-roms 0` to include every collected ROM entry.

## Maintenance

Use devenv 2.3.1 or newer with Nix. The repository owns `devenv.nix`,
`devenv.yaml`, and `devenv.lock`; no machine-local devshell imports are needed.
The Rust module selects latest stable (currently pinned to 1.98.1), including
Clippy, rustfmt, rust-analyzer and Rust sources. Nix supplies the compiler/linker,
SQLite, OpenSSL, zlib, CMake and pkg-config; sccache caches compilation.

```sh
devenv shell                 # interactive development shell
devenv shell -- build        # cargo build --locked
devenv tasks run project:check  # formatting, scripts, tests, strict Clippy
devenv test                  # the same gate, Git hooks and CLI smoke test
```

Inside an already activated shell, run `cargo` or `build` directly.
Do not nest `nix develop` inside devenv. For automatic activation, configure
`devenv hook` for your shell and run `devenv allow` after reviewing the checkout.
The project does not require `.envrc` or automatically load `.env`.

| Feature | Usage |
| --- | --- |
| Named verification tasks | `devenv tasks list`; `devenv tasks run project:format` |
| Git checks installed on shell entry | Rust formatting, Nix formatting, ShellCheck |
| Faster test runner | `devenv shell -- cargo nextest run --locked` (the main gate also runs doctests) |
| Profiling tools | `devenv --profile profiling shell` for flamegraphs, perf on Linux and hyperfine |
| Maintenance tools | `devenv --profile maintenance shell` for audit, deny, outdated and machete |
| Toolchain refresh | `devenv update rust-overlay`, then `devenv test`; review and commit the lockfile |

Normal builds and tests use `--locked`; entering a shell does not update
Cargo dependencies or run the full test suite. No services are needed for this
CLI: tests use temporary SQLite databases and synthetic archive fixtures.

The older `nix develop -c ...` entrypoint remains available for compatibility,
but devenv is the primary development and CI gate.

### CPU Flamegraphs

The profiling helpers mirror the workflow used in `nntp-proxy`, with
`mame_coalesce`-specific categories for hashing, archive work, scan walking,
planning, SQLite/Diesel, and writing. Always run them through Nix.

Baseline single-thread run:

```sh
devenv --profile profiling shell -- bash scripts/profile_flamegraph.sh \
  --dat fixtures/<dat>.dat \
  --source <source-dir> \
  --out target/profiling/out-jobs-1 \
  --jobs 1
```

If perf permissions block sampling, retry with `--root`:

```sh
devenv --profile profiling shell -- bash scripts/profile_flamegraph.sh \
  --dat fixtures/<dat>.dat \
  --source <source-dir> \
  --out target/profiling/out-jobs-1 \
  --jobs 1 \
  --root
```

Parallel comparison:

```sh
devenv --profile profiling shell -- bash scripts/profile_flamegraph.sh \
  --dat fixtures/<dat>.dat \
  --source <source-dir> \
  --out target/profiling/out-jobs-8 \
  --jobs 8
```

Summarize or compare generated flamegraphs:

```sh
devenv shell -- bash scripts/parse_flamegraph target/profiling/flamegraphs/run-jobs-1.svg summary
devenv shell -- bash scripts/parse_flamegraph target/profiling/flamegraphs/run-jobs-1.svg top 30 0.5
devenv shell -- bash scripts/parse_flamegraph target/profiling/flamegraphs/run-jobs-1.svg search planner
devenv shell -- bash scripts/parse_flamegraph target/profiling/flamegraphs/run-jobs-1.svg diff target/profiling/flamegraphs/run-jobs-8.svg
```

Benchmark the full `build` workflow with repeated wall-clock samples:

```sh
devenv --profile profiling shell -- bash scripts/benchmark_run.sh \
  --dat tmp/perf-public-domain/dats/public-domain-roms.dat \
  --source tmp/perf-public-domain/source-roms \
  --out-root target/profiling/perf-out-jobs-1 \
  --jobs 1 \
  --runs 5
```

Add `--compression store` to measure the opt-in stored-ZIP write path while
keeping default CLI behavior unchanged.
The benchmark helper prebuilds the profiling binary and runs it directly by
default; pass `--runner cargo` only when measuring the older `cargo run` path.

Optional raw perf-data analysis, if `perf.data` is retained or captured
manually:

```sh
devenv --profile profiling shell -- sh -c 'perf script 2>/dev/null | bash scripts/parse_perfdata'
```

## Verification

Required local gate: `devenv test`. It also runs the normally ignored p7zip
interoperability test because devenv supplies `7z`. The component commands are:

```sh
devenv shell -- shellcheck scripts/fetch_public_domain_test_data.sh scripts/profile_flamegraph.sh scripts/benchmark_run.sh scripts/generate_synthetic_benchmark_corpus.sh scripts/parse_flamegraph scripts/parse_perfdata
devenv shell -- cargo fmt --check
devenv shell -- cargo test --locked
devenv shell -- cargo test --locked --test integration p7zip_extracts_r7z_builder_archive -- --ignored --exact
devenv shell -- cargo clippy --locked --all-targets --all-features -- -D warnings
```

`cargo package` is intentionally not part of the normal local or CI gate while
the project depends on the git-only `r7z` crate.

Dependency and maintenance checks:

```sh
devenv shell -- cargo tree -d
devenv --profile maintenance shell -- cargo audit
devenv --profile maintenance shell -- cargo deny check
devenv --profile maintenance shell -- cargo machete
```

## Known Operational Constraints

- Running outside Nix requires system `pkg-config`, SQLite, zlib, and related
  development libraries.
- The crate currently declares `rust-version = "1.88"`; devenv builds
  with latest stable Rust from the locked `rust-overlay` input. Updating the
  development toolchain does not by itself change the declared MSRV.
- `cargo package` requires `r7z` to be published on crates.io; until then the
  crate uses a pinned `mjc/r7z` git dependency.
- `cargo deny check` may report duplicate dependency warnings under the current
  policy, but the check exits successfully.
