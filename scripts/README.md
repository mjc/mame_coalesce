# Scripts

## Public-domain ROM test data

`fetch_public_domain_test_data.sh` downloads small public-domain ROM bundles
from archive.org, extracts archives where needed, generates a focused Logiqx DAT
from the downloaded bytes, and runs the one-shot `mame_coalesce build` workflow in
an isolated temp directory.

```sh
devenv shell -- bash scripts/fetch_public_domain_test_data.sh
```

The default `--catalog-tier curated` includes:

- `rs32_20200909`: NES/SNES PD bundles with archive.org Public Domain Mark metadata.
- `Chip-8RomsThatAreInThePublicDomain`: CHIP-8 pack whose archive.org title and Zophar source identify it as public domain.
- `pdrc2_5-submissions`: PDRoms Coding Competition homebrew bundle.
- `github.com-DerekTurtleRoe-N64-PD-ROMS_-_2023-10-31_17-21-10`: N64 PD ROM repository bundle, with obvious commercial-property derivative names filtered out.

Use strict archive.org license metadata only:

```sh
devenv shell -- bash scripts/fetch_public_domain_test_data.sh --catalog-tier metadata
```

Use every collected ROM entry instead of the default cap:

```sh
devenv shell -- bash scripts/fetch_public_domain_test_data.sh --max-roms 0
```

The script does not download abandonware, commercial ROM-set mirrors,
translations/patches of commercial games, or obvious derivative demos using
commercial game properties.

## CPU flamegraphs

Generate a symbol-rich flamegraph for the full `build` workflow:

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

Summarize the generated SVG:

```sh
devenv shell -- bash scripts/parse_flamegraph target/profiling/flamegraphs/run-jobs-1.svg summary
```

`parse_flamegraph` uses Inferno's `fg:x`/`fg:w` sample ranges to count each
sample once per symbol, even when frames overlap or a symbol appears more than
once. `top`, `search`, and `diff` show *inclusive* function coverage; those rows
are not additive. `summary` attributes each sample range to its deepest visible
frame, so its category rows sum to 100%. `diff` is meaningful only for
comparable workloads. For symbolized exclusive on-CPU costs, use
`perf report --stdio --no-children -g none -i <perf.data>`;
the SVG cannot recover frames absent from its recorded call chains.

## Run benchmarks

Generate a repeatable 100-ROM synthetic corpus for baseline comparisons:

```sh
devenv shell -- bash scripts/generate_synthetic_benchmark_corpus.sh
```

The helper creates `target/profiling/synthetic-corpus/` and refuses to replace
an existing corpus. Pass its `synthetic.dat` and `source/` paths to
`benchmark_run.sh` for each jobs/compression combination.

Use `benchmark_run.sh` to capture repeated wall-clock samples for the full
`build` workflow. The script cleans only the selected cache database and output
directory under `target/profiling` before each measured run and writes a
Markdown plus JSON report under `target/profiling/reports/`.
It prebuilds `target/profiling/mame_coalesce` and runs that binary directly by
default so samples do not include `cargo run` overhead.

```sh
devenv --profile profiling shell -- bash scripts/benchmark_run.sh \
  --dat tmp/perf-public-domain/dats/public-domain-roms.dat \
  --source tmp/perf-public-domain/source-roms \
  --out-root target/profiling/perf-out-jobs-1 \
  --jobs 1 \
  --runs 5
```

Pass `--compression store` to benchmark the opt-in stored-ZIP output path. The
default CLI path remains `--compression deflate`.
Pass `--runner cargo` if you need to compare against the older cargo-driven
benchmark command.

## XML parse-and-import benchmarks

Generate the pinned 1,000-record inputs for Logiqx, MAME machine XML, and MAME
software-list XML:

```sh
devenv shell -- bash scripts/generate_xml_import_benchmark_corpus.sh
```

Build and run the end-to-end import harness. It reports elapsed time and
allocator counts/bytes for the import call; GNU `time` reports process peak RSS.
Run baseline and refactored binaries against the same generated files and use
fresh databases for every sample:

```sh
devenv shell -- cargo build --release --locked --example xml_import_bench
devenv shell -- bash scripts/benchmark_xml_import.sh \
  target/release/examples/xml_import_bench \
  target/profiling/xml-import-corpus 3
```

The generated corpus is deterministic. The generator prints SHA-256 digests so
the exact three inputs can be recorded alongside benchmark results. Use the
same Nix/devenv environment and storage location for both revisions; these
measurements are comparative evidence, not a pass/fail threshold. The recorded
baseline/refactored results and host/toolchain details are in
[`docs/xml-import-benchmark.md`](../docs/xml-import-benchmark.md).

## Catalog import flamegraphs

Generate one release-like CPU flamegraph for each supported catalog format with
`perf` user-space cycles sampling. The raw `perf.data` files are also retained.
Rendering removes unresolved high kernel-address frames that may appear in a
user-space call chain; no samples or user-space frames are removed.
The MAME software-list graph profiles the complete downloaded list corpus in a
single process. The No-Intro graph uses the repository's explicitly synthetic
projection fixture; the upstream mirror is not a compatible production export.
Separate graphs capture the failed attempts to import the real ClrMamePro DAT
and No-Intro XML mirror. The helper builds into `target/catalog-import-profile-build`;
clean that build with `cargo clean --profile profiling --target-dir target/catalog-import-profile-build`
to preserve downloaded inputs in `target/profiling`.
Profiles include process startup, database initialization, and import work. The
helper creates a new output directory under `target/profiling` and refuses to
overwrite an existing run.

```sh
devenv --profile profiling shell -- bash scripts/profile_catalog_imports.sh \
  target/profiling/external-catalogs-2026-09-29 \
  target/profiling/catalog-import-flamegraphs-2026-09-29
```

Inspect a completed import with `bash scripts/parse_flamegraph <svg> summary`
or `top 30 0.5`. The retained `perf/<name>.data` files provide symbolized
self-time through `perf report`; do not use the SVG alone to rank Rust parser
functions if its call chains omit their frames.
