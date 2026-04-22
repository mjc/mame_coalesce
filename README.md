# mame_coalesce

`mame_coalesce` imports Logiqx DAT files, scans ROM sources, plans deterministic builds, and writes merged ZIP outputs.

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

Defaults:

- `--mode parent-bundles`
- missing ROMs are reported without failing
- `--strict` exits `2` and writes nothing when required ROMs are missing
- duplicate source matches are resolved deterministically

## Maintenance

Use the Nix shell as the development environment. Plain `cargo test` may fail on systems without `pkg-config`, `libarchive`, and SQLite development libraries.

Required green checkpoint:

```sh
nix develop -c cargo fmt --check
nix develop -c cargo test
nix develop -c cargo clippy --all-targets --all-features -- -D warnings
```

Dependency and maintenance checks:

```sh
nix develop -c cargo update
nix develop -c cargo tree -d
nix develop -c cargo audit
nix develop -c cargo deny check
nix develop -c cargo-udeps udeps
```

Current dependency majors include crates that declare MSRVs newer than Rust `1.85`; the Nix shell currently builds with Rust `1.92`.
