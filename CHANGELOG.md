# Changelog

## Unreleased

- Modernized the crate for Rust 2024 and tightened linting around unsafe code,
  unwrap/expect usage, TODOs, and clippy warnings.
- Added the app-level DAT import, source scan, build, and one-shot run workflow
  exposed through the current CLI.
- Cleaned DAT reimport behavior so old games and ROMs are removed before new
  rows are inserted for the same DAT.
- Canonicalized scanned source roots and fixed source-root boundary matching so
  similarly prefixed paths do not cross-match.
- Skipped directory entries while scanning ZIP and 7z archives.
- Refreshed scanned ROM file links after DAT import so scan-before-import
  workflows build correctly.
- Propagated corrupt or unreadable source file, ZIP, and 7z scan errors
  instead of silently dropping bad inputs.
- Replaced libarchive-backed archive handling with `r7z` for 7z archives and
  the `unrar` crate for RAR archives.
- Added end-to-end 7z workflow coverage, including p7zip extraction of archives
  produced by `r7z`.
- Validated build output ZIP file names and ZIP entry names before writing, and
  rejected duplicate output ZIP names and duplicate entry names.
- Added a public-domain ROM smoke-test script and documentation for generating
  a focused Logiqx DAT from archive.org sources.
- Added argument validation for the public-domain smoke-test script.
- Added profiling and benchmark helpers, plus an opt-in `--compression store`
  ZIP write mode after public-domain end-to-end profiling showed deflate as the
  dominant measured hotspot; default output remains deflated.
- Updated the benchmark helper to run the prebuilt profiling binary directly by
  default so measurements do not include `cargo run` overhead.
- Updated profiling parser categories for the current ZIP, 7z, and RAR archive
  implementation.
- Refreshed Cargo and Nix dependencies, including raising the declared Rust
  version to `1.88` for the current dependency graph.
- Aligned the GitHub CI gate with the local release-readiness checks.
- Verified the current handoff with formatting, tests, clippy, package,
  shellcheck, audit, deny, and udeps checks.
