# Scripts

## Public-domain ROM test data

`fetch_public_domain_test_data.sh` downloads small public-domain ROM bundles
from archive.org, extracts archives where needed, generates a focused Logiqx DAT
from the downloaded bytes, and runs the one-shot `mame_coalesce run` workflow in
an isolated temp directory.

```sh
nix develop -c bash scripts/fetch_public_domain_test_data.sh
```

The default `--catalog-tier curated` includes:

- `rs32_20200909`: NES/SNES/GBA PD bundles with archive.org Public Domain Mark metadata.
- `Chip-8RomsThatAreInThePublicDomain`: CHIP-8 pack whose archive.org title and Zophar source identify it as public domain.
- `pdrc2_5-submissions`: PDRoms Coding Competition homebrew bundle.
- `github.com-DerekTurtleRoe-N64-PD-ROMS_-_2023-10-31_17-21-10`: N64 PD ROM repository bundle, with obvious commercial-property derivative names filtered out.

Use strict archive.org license metadata only:

```sh
nix develop -c bash scripts/fetch_public_domain_test_data.sh --catalog-tier metadata
```

Use every collected ROM entry instead of the default cap:

```sh
nix develop -c bash scripts/fetch_public_domain_test_data.sh --max-roms 0
```

The script does not download abandonware, commercial ROM-set mirrors,
translations/patches of commercial games, or obvious derivative demos using
commercial game properties.

## CPU flamegraphs

Generate a symbol-rich flamegraph for the full `run` workflow:

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

Summarize the generated SVG:

```sh
nix develop -c bash scripts/parse_flamegraph target/profiling/flamegraphs/run-jobs-1.svg summary
```

## Run benchmarks

Use `benchmark_run.sh` to capture repeated wall-clock samples for the full
`run` workflow. The script cleans only the selected database and output
directory under `target/profiling` before each measured run and writes a
Markdown plus JSON report under `target/profiling/reports/`.

```sh
nix develop -c bash scripts/benchmark_run.sh \
  --dat tmp/perf-public-domain/dats/public-domain-roms.dat \
  --source tmp/perf-public-domain/source-roms \
  --out-root target/profiling/perf-out-jobs-1 \
  --jobs 1 \
  --runs 5
```

Pass `--compression store` to benchmark the opt-in stored-ZIP output path. The
default CLI path remains `--compression deflate`.
