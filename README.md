# mame_coalesce

`mame_coalesce` imports Logiqx DAT files, scans ROM sources, plans deterministic
builds, and writes verified ZIP/7z archives or directory outputs.

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
--output-container zip
--output-container 7z
--output-container directory
--missing warn
--missing fail
--dry-run
--set "Game Set Name"
--reuse-verified
```

`--reuse-verified` leaves an existing ZIP, 7z, or directory artifact untouched
only when its complete logical contents match the current plan. Otherwise the
artifact is rebuilt as usual. The default remains replacement; source files
are still scanned/planned normally, and this option does not execute a saved
plan.

Build and audit commands accept repeatable `--set NAME` options to restrict
planning to exact set names in the selected DAT:

```sh
mame_coalesce build catalog.dat /roms /out --set "Game Set Name" --set "Another Set"
mame_coalesce --cache /tmp/coalesce.db audit "DAT Header Name" /roms --set "Game Set Name"
```

Set names are the DAT game/set names, not the DAT header title. Matching is
exact and case-sensitive, and selection is scoped to the chosen catalog even
when another imported DAT contains the same set name. Omitting `--set` keeps
the existing behavior and plans every set. Year, manufacturer, BIOS, and
`romof` are retained as catalog metadata but are not currently planner filters.

Explicit cache maintenance commands are available for advanced workflows:

Ordinary DAT imports and builds use the same immutable native catalog snapshots
as explicit catalog imports. A local DAT's canonical path identifies its catalog;
changed bytes publish a new snapshot instead of replacing another list with the
same header title. Cached builds accept an exact catalog key, source path or
unique display/header name and select the latest published snapshot. Ambiguous
names are errors; exact keys take precedence over paths, and retained source
paths remain queryable after their original files are removed. One-shot builds
pin the snapshot returned by their import, including when bytes revert to an
older edition or another import publishes during scanning.
Scanned files remain independent observations; scan summaries
count whole-file SHA-1 candidates once, without assigning a file to one list's
ROM row. The flat build interface does not flatten software-list items or
silently combine repeated set names.

```sh
mame_coalesce --cache /tmp/coalesce.db cache import fixtures/test.dat
mame_coalesce --cache /tmp/coalesce.db cache scan /path/to/roms --jobs 8
mame_coalesce --cache /tmp/coalesce.db cache scan /path/to/roms --reuse-unchanged
mame_coalesce --cache /tmp/coalesce.db cache scan /path/to/roms --reuse-unchanged --force-rehash /path/to/roms/suspect.rom
mame_coalesce --cache /tmp/coalesce.db cache build "DAT Header Name" /path/to/roms /path/to/out
mame_coalesce --cache /tmp/coalesce.db audit "DAT Header Name" /path/to/roms
mame_coalesce --cache /tmp/coalesce.db audit "DAT Header Name" /path/to/roms --format json
mame_coalesce --cache /tmp/coalesce.db audit "DAT Header Name" /path/to/roms --refresh --jobs 8
mame_coalesce --cache /tmp/coalesce.db cache backup /path/to/coalesce.backup.sqlite
mame_coalesce --cache /tmp/coalesce.db cache integrity
mame_coalesce --cache /tmp/coalesce.db cache restore /path/to/coalesce.backup.sqlite
mame_coalesce --cache /tmp/coalesce.db cache restore /path/to/coalesce.backup.sqlite --replace-existing
```

Backups consist of a SQLite snapshot made with `VACUUM INTO` and an adjacent
`<backup-path>.documents` directory containing retained source objects. Keep
both together when moving a backup; source bytes are never stored in SQLite.
Eligible whole-file SHA-1/SHA-256 source declarations share a persistent
16-byte file UUID without combining their catalog entries. Hash lookup joins
the original declarations; shared identities store no copied hashes or size.
Contradictions and ambiguous bridges remain unlinked, with references to the
specific source hash and size evidence. A disputed hash also blocks later sparse
claims until reviewed settlement is implemented. Backup/restore preserves the
database-wide registry generation; a fresh rebuild starts a new generation.
UUID interchange uses 32 hexadecimal characters without dashes.

Logiqx imports retain the original size/checksum spelling, empty declarations,
and text boundary spaces in native catalog fields. Uninterpretable declarations
remain queryable but are not usable matching evidence and cannot assign a shared
file UUID. Logical disk hashes never identify whole-container files. The default
`logiqx-declared-text-compat-v1` interpretation accepts sparse compatibility
documents; it is not strict DTD validation.

ClrMamePro ROM declarations likewise retain checksum case, leading-zero size
text, quoted/empty values, both CRC aliases and independent dump flags in native
columns. Conflicting declarations remain queryable but cannot assign a shared
UUID; an ambiguous digest never supplies a singular matching hash. Field
positions retain source spelling, quotation, order and location without copying
the values. Publication checks complete ownership and agreement with normalized
source assertions. Headers, directives and set values use fixed native columns
with separate closed-field positions. Set region, release-date components and
serial preserve their declared text; set serial is distinct from ROM serial.
Lexical semicolon comments have ordered document-owned rows, separate from the
header comment. The documented missing `forcenodump` default is `obsolete`;
explicit unknown directive values remain stored but have no effective mode.
History tracks native document and scalar/sample/ROM order without treating
repeated-name source positions as identity. The
`clrmamepro-declared-text-compat-v1` interpretation is an explicit compatibility
grammar, not proof of every CMP dialect. Scalar sample rows are native query
data; shared media-entry API coverage and the full format witness matrix remain
unfinished.

The Rust `catalog_files` API exposes bulk occurrence lookup and keyset-paginated
file membership across published catalog editions. Results keep each owner and
its source/list provenance, with digest assertions as separate children.
Pagination cursors belong to one file UUID and registry generation.

Format version 1 is marked in the SQLite header and includes the cache database,
acquisitions, catalog snapshots, assertions, reviews, diagnostics, and
rebuildable inventory. Backup creation never replaces an existing backup file.
Restore validates a same-directory staging copy against the running program's
bundled DDL before publishing it; an existing cache requires
`--replace-existing`, and restore is refused while the cache is open by this
application or SQLite sidecar files exist. Cache files with multiple hard links
are refused because they bypass application locking. New databases are created
from the bundled DDL, and existing databases are accepted only when their
stored schema digest and SQLite schema objects match it. The application does
not upgrade or repair a different schema; create a new cache and reimport its
catalogs when the schema changes. `cache integrity` never modifies the inspected
SQLite database; backup, restore, and integrity operations may create adjacent
`.lock` files on both the cache and backup paths. It reports durable
catalog/document failures separately from rebuildable inventory problems.
Inventory problems do not invalidate a backup or prevent restore. Where the
platform cannot sync the containing directory, publication succeeds with a
warning that crash durability could not be confirmed. The integrity command
returns a failing exit status when either category contains issues.

Build, audit, and cache scan also accept repeatable `--source-root DIR` options.
The positional source directory stays first; roots are canonicalized and exact
aliases are scanned once. Earlier roots take precedence over later roots, then
the existing within-root bare-file/archive/path/member order applies. Physical
members reached through overlapping roots resolve once, attributed to the
earliest matching root; a nested root still has its own cached provenance and
can take precedence when listed first. A multi-root refresh scans every root
before changing inventory, then replaces all requested scopes in one SQLite
transaction. If scanning any root fails, none of those scopes are replaced.

For example, `mame_coalesce build catalog.dat /roms/primary out
--source-root /roms/secondary --source-root /roms/overrides` searches the
primary tree first, followed by secondary and overrides. Repeat the same ordered
options for `audit` or `cache scan` to use the same scope and precedence.

Cache scans rehash every source by default. For a single source root, the
explicit `cache scan --reuse-unchanged` option may reuse observations for bare
files when their persisted Unix file identity, size, modification time, and
change time still match. Archives are always read and fingerprinted. Use the
repeatable `--force-rehash FILE` option to bypass reuse for selected files;
files with absent or mismatched cache stamps also fall back to a full hash.
This metadata check is an opt-in performance heuristic, not integrity evidence:
privileged metadata manipulation or filesystem behavior outside the platform
stamp can make it stale. Builds still verify source bytes against catalog
evidence before writing output. Non-Unix platforms safely disable reuse.

The reusable library operations in `app` do not initialize logging or terminal
progress. `plan_build` reads the selected catalog and cached scan observations
and returns a logical plan without creating outputs. `build` reads the cache and
writes requested artifacts. `import_dat` and `scan_source` persist catalog and
inventory changes, respectively; `run` performs those imports/scan updates
before building. The `*_with_roots` operations accept an ordered
`SourceRootSelection`; the original operations remain one-root conveniences. A
one-shot `--dry-run` or strict-missing build still imports the
DAT and refreshes the scan cache, but does not write ROM outputs. The CLI owns
human-readable reports, progress bars, and mapping build outcomes to process
exit codes. `build_with_container` and `run_with_container` select ZIP, 7z, or
directory output without changing request types; their ordered-root variants
offer the same choice.

`audit` has no output destination and never invokes an output writer. It uses
an already-imported DAT and cached source observations by default and labels
them as cached rather than freshly checked bytes. Opening the cache validates
its schema against the bundled DDL; that does not refresh source observations. `--refresh`
explicitly rescans and persists the selected source root before resolving;
`--verify-selected` instead reads and verifies only the resolved source members,
reports stale or unavailable selections, and leaves the inventory unchanged;
it cannot be combined with `--refresh`.
`--matching-policy evidence-aware` opts into the
resolver's evidence-aware conflict/ambiguity classifications. Human reports
include expected and observed evidence and the selected or competing sources.
To keep adversarially large inventories from multiplying report memory by the
number of requirements, resolution retains at most 256 detailed assessments and
256 duplicate-source examples per operation; omitted counts remain explicit in
JSON and human reports.
`--format json` writes version 2 audit documents to stdout and keeps logs and
progress on stderr; the reader remains compatible with version 1 reports.
Audit exits `0` when all requirements match and `1` when
requirements remain unresolved (operational failures also exit `1`). These
audit statuses do not change build's existing `0` success, `1` execution-failure
and `2` strict-missing behavior.

Evidence-aware matching ranks SHA1 above MD5 above CRC-plus-size. It may fall
back to a weaker digest only when the observed stronger digest does not
contradict the catalog; a stronger contradiction vetoes that candidate. A
unique CRC-plus-size candidate is classified as weak evidence, while collisions
remain ambiguous unless consistent stronger evidence establishes equivalent
copies. The default SHA1-compatibility policy is unchanged and does not use
those fallback matches.

ZIP compression defaults to deflate for compatibility. Use `--compression store`
when profiling or when faster, larger ZIP output is preferred.
`--output-container` is independent of layout and compression; it defaults to
`zip`, `directory` writes each logical group as an uncompressed directory, and
`7z` writes each group as a 7z archive using the pinned `r7z` LZMA2 defaults.
`--compression` applies only to ZIP output; it is ignored for 7z and directory
output.

Defaults:

- `--layout parent-bundles`
- `--compression deflate`
- `--output-container zip`
- missing ROMs are reported without failing
- `--missing fail` exits `2` and writes nothing when required ROMs are missing
- duplicate source matches are resolved deterministically

Building from archive members stages selected bytes on disk before assembling
each output ZIP or 7z archive. Its private staging directory is created beside
the output, so ensure that filesystem has up to 16 GiB of free space for one
output archive.

Logical output paths use a conservative portable naming profile: ASCII only,
case-insensitive collision checks, no trailing spaces or dots, path traversal,
Windows-reserved device names, or platform-forbidden characters. Unicode names
are rejected rather than relying on filesystem-specific normalization. The
one-shot workflow rejects source and destination roots that are equal, nested,
or aliased through existing symlinks; it checks before scanning and again before
writing. Linux and macOS artifact writing use handle-relative directory access
and do not follow symlink components. These checks contain untrusted catalog and
archive paths; concurrent local processes modifying the destination are outside
the threat model. Other platforms are rejected before creating or truncating
artifacts. Sources are never modified.

Each ZIP or 7z artifact is built in a temporary file beside its destination.
Source bytes are streamed and checked against the selected catalog evidence and
scanned source identity; archive fingerprints are checked before and after
member reads. The finished archive is flushed and synced before replacement. On
Linux and macOS, renaming that same-filesystem temporary file replaces one
artifact atomically;
the containing directory is synced afterward to ask the filesystem to persist
the new entry. This does not guarantee survival of sudden power loss on every
filesystem or storage device (notably, macOS `fsync` may not flush drive caches).
A multi-artifact build is not atomic as a whole: execution stops at the first
failure and reports which artifacts completed, failed, or were not attempted.
Atomic replacement is currently supported on Linux and macOS only.

Each directory artifact is materialized in a private sibling staging directory.
Replacing an existing real directory first renames it into a unique sibling
backup, then installs the staged directory. If installation fails, the writer
attempts to restore the backup; if rollback also fails, the report includes the
recoverable backup path. A successful install whose backup cleanup fails is
reported as completed with a warning and retains that backup. Directory rename
is not treated as a universally atomic replacement: a process or machine crash
between moving the old directory aside and installing the new one can leave the
destination absent, with the old output under its hidden backup name. A
multi-group build is not atomic as a whole. Output sources are read-only.

Source scans intentionally skip hidden files and directories below the source
root. Non-UTF-8 paths and traversal or archive-read errors fail the scan, so an
incomplete inventory cannot replace the last successful cache contents.

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

For module boundaries, compatibility and cache behavior, change
recipes, and refactor test/measurement evidence, see
[`docs/architecture.md`](docs/architecture.md).

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
For automatic activation, configure `devenv hook` for your shell and run
`devenv allow` after reviewing the checkout.
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

## MAME XML catalog import

The machine `-listxml` adapter imports machine records, clone relationships, ROM
and disk declarations, BIOS sets, and every machine-child family declared by the
MAME 0.289 DTD, including displays, input controls, switch conditions, drivers,
features, devices, slots, software-list references, and RAM options. DTD defaults
and required device-reference tags are stored as typed facts, with ordered nested
rows and snapshot-diff coverage. The separate software-list adapter imports list-scoped items, parts,
data/disk areas, component evidence, and load instructions as source data; it
does not execute those instructions or expand dependencies. Both adapters
retain unrecognized XML in the external original document, not a generic
SQLite field table. Imports accept retained documents and
gzip-expanded XML up to 384 MiB. XML tree adapters are limited to 200,000
elements; the record-streaming MAME machine and software-list adapters allow up
to 6,000,000 elements to accommodate official full catalogs and larger lists.
Nesting is limited to 256 levels. These bounds limit retained input, parser
work, and per-record trees; larger documents are rejected.

Real catalog import CPU flamegraphs and a partial full-machine-XML heap trace are
documented in [catalog import profiling](docs/catalog-import-profiling.md).
The repeatable CPU helper is `scripts/profile_catalog_imports.sh`.

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

## Test coverage

Run the complete repository gate with `devenv test`. The database
and catalog regressions cover these behaviors:

- `database_initialization` checks creation from bundled DDL, acceptance of a
  matching schema, and rejection of schema drift without repair.
- `catalog_content_disputes` checks conflicting source evidence, later sparse
  claims against disputed hashes, immutable published assertions, guarded
  replacement writes, cross-format file membership and registry-preserving
  backup/restore versus a fresh generation.
- `catalog_files` unit tests check published-only bulk and paginated membership,
  repeated owners, native filenames and locations, software part/area ownership,
  scoped digest children, corruption errors, temporary-table cleanup and indexed
  owner, digest and payload lookup without scanning the entire catalog.
- `native_catalog_model` checks scoped record identity and rejects occurrences
  whose claim kind or content identity conflicts with their native record;
  it also checks that no second mutable DAT model exists.
- `native_build_catalog` checks native publication through ordinary imports,
  exact external source recovery, optional hashes and large ROM sizes,
  path-scoped catalogs, name ambiguity, snapshot replacement without lost
  history, native build/audit queries and scan rollback.
- `native_build_selection` checks exact-key priority over an existing path,
  retained source-path lookup without the input file, reverted-byte imports
  and immutable build/audit selection across a competing publication.
- `catalog_shared_model` checks relational scope/set storage, separate owners
  for repeated names, owner-aware relationship explanations, publication
  immutability, and P/C empty fields, ordered languages and archive-ID tokens.
- `catalog_history_ownership` compares complete same-name owner fact multisets,
  preserves unchanged permutations and reports ambiguous changes without
  inventing continuity between entries.
- `catalog_extension_ownership` checks exact external-document recovery of
  unknown game/ROM fields even when both game and ROM names repeat.
- Coverage unit tests check exact scope reuse, root/software qualification,
  selected-but-absent sets, partial unknown members and immutable referenced
  coverage rows.
- `logiqx_native_model` checks parser defaults and repeated ROM, disk, sample,
  release and BIOS-set fields, plus headerless native imports and snapshot diffs.
- `native_logiqx_specification` checks native header options and repeated game
  children, explicit/default presence, empty fields, ordering and owner guards.
- `catalog_logiqx_persistence` checks separate ROM/disk/sample occurrences,
  original size text, qualified disk identity and immutable published owners.
- `native_logiqx_history` checks option and repeated-child edits, native order
  across media families, default presence, vendor-only insertions, whitespace
  changes and repeated-owner permutations without false size/hash changes.
- `native_mame_specification` checks ordered machine specification facts and
  their stored field values.
- `clrmamepro_native_model` checks native CMP header directives, set/sample
  facts, and ROM date, serial, and status provenance.
- `cmp_declared_fields` checks raw ROM declarations, conflicting evidence,
  source assertion agreement and complete immutable ROM field ownership.
- `cmp_set_provenance` checks all header/set values, date-component text,
  separate parent declarations, lexical comments and native order in history,
  including header crossings and unchanged repeated-name owner permutations.
- `cmp_native_publication` checks missing native rows and field positions,
  invalid ordinals, foreign-format owners and immutable publication boundaries.
- `software_native_model` checks shared file UUIDs without confusing load
  segments with file lengths, area-local file owners, and scope-correct digests.
- `catalog_import` exercises public imports, including format-specific facts,
  idempotent publication, relationships, diagnostics, and equivalence between
  native and shared catalog fields.
- `cache_backup` checks backup and restore behavior against the current schema.

The integration suite also generates and reads synthetic 7z archives through
`r7z`, pinned at revision `bfef3198696add8045ad34581dd977d671ae9daa`; it does not
require an external `7z` executable. This checks the pinned library's
writer/reader and the application read path, not compatibility with every
external encoder or codec. Independent cross-implementation checks are outside
the default gate. To run the ignored interoperability test with a compatible
executable installed, set `MAME_COALESCE_7Z` if its name is not `7z`:

```sh
devenv shell -- env MAME_COALESCE_7Z=7z cargo test --locked --test integration external_7z_extracts_r7z_builder_archive -- --ignored
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
- The crate declares `rust-version = "1.98.1"`, matching the latest stable
  Rust release selected by devenv. The project currently tests against that
  release rather than maintaining a separately validated older MSRV.
- `cargo package` requires `r7z` to be published on crates.io; until then the
  crate uses a pinned `mjc/r7z` git dependency.
- `cargo deny check` may report duplicate dependency warnings under the current
  policy, but the check exits successfully.

## Optional read-only FUSE view

On Linux, build with the optional `fuse` feature and mount a serialized MAME
0.289 view manifest:

```sh
cargo run --features fuse -- mount \
  --manifest /path/to/view.json \
  --mountpoint /path/to/empty-mountpoint \
  --spool-root /path/to/separate-spool
```

The mount is read-only. Startup validates source locations, selected archive
members, and known lengths; every open then verifies the pinned content before
exposing bytes. Same-length stale content therefore fails on open rather than
being served or retargeted. The spool must be an existing directory outside
both the source tree and mountpoint, with at least 512 MiB available.
The first adapter is Linux-only; FUSE remains optional, so catalog and other
commands do not depend on it. Other operating systems do not yet have an adapter.
