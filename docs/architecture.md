# Architecture and compatibility

This document describes the implementation through the explicit MAME layout-policy increment. It
supersedes the earlier proposed target architecture where the two differ. The
crate remains pre-1.0; the compatibility promises here describe the command-line
workflow and existing cache behavior, not a semver-stable public Rust API.

## Dependency direction

The normal flow is:

```text
CLI (clap, terminal rendering, exit mapping)
  -> app (reusable workflows and request/report types)
    -> domain + operations
      -> format adapters / sources / resolution / build planning
        -> storage repositories and SQLite
      -> build validation and writer
```

`domain` owns typed catalog identities, retained-document and scan evidence,
matching/build requests, logical plans, and reports. Format adapters turn
Logiqx, MAME listxml, MAME software-list XML, ClrMamePro, and No-Intro PC XML
into the common snapshot model; `storage::catalog_import` persists normalized
snapshots and parser extensions. `document_input` bounds and validates input;
`storage::documents` retains immutable bytes and acquisition metadata.

`operations::scan` discovers and fingerprints source files and archive
members. `sources` is the backend boundary for bare files and supported archive
formats. `storage::repositories` persists completed observations and reads
cached inventory. `resolution` is the format-neutral matching policy layer;
`build::planner` converts selected evidence into logical output groups, while
`build::validation` rejects unsafe paths, collisions, and source/output
overlap. `build::writer` reopens and verifies selected sources, stages archive
members once per source session, and writes ZIP or directory artifacts. The
public `app` functions compose these layers without CLI logging, rendering, or
process exits. The binary owns those presentation concerns.

`build::mame_layout` is a separate pure operation over one snapshot's typed
dependency closures and already-resolved set contents. Its explicit non-merged
policy makes each selected machine group self-contained by including the
resolved BIOS, device, and other runtime dependency contents, plus inherited
parent ROMs with child merge declarations applied as overrides. Clone ancestry
is not treated as a runtime-dependency edge. The planner reports incomplete
dependencies, missing assets, and content/path conflicts before any writer is
involved; neither legacy CLI layout is reinterpreted.

The crate root deliberately exposes `app`, `build`, `database`, `disk`,
`domain`, `error`, `hashes`, `logiqx`, and `resolution`, plus the retained
document store types. Parsing adapters, scan operations, source backends, and
storage implementation details remain crate-private. This keeps reusable
requests and typed plans observable without making persistence tables or
parser-internal representations the public contract.

## Compatibility, cache, and workflow semantics

- Existing one-source CLI positionals and defaults remain: ZIP output,
  `parent-bundles`, deflate compression, and permissive missing-content
  reporting. Additional source roots and directory output are opt-in.
- The library's snapshot-aware non-merged MAME policy is an explicit planning
  operation and does not change the meanings or CLI availability of
  `parent-bundles` and `per-game`.
- The database runs embedded forward migrations when it is opened. The tested
  additive migrations preserve populated legacy DAT/game/ROM/source rows.
  Legacy source rows whose bytes or archive-member identity were never retained
  remain present but unavailable for operations that require that evidence;
  migration does not invent missing identity or rescan source directories.
- Retained catalog documents are content-addressed and immutable. Re-imports
  record acquisitions/import runs and normalized snapshots; an import's
  snapshot publication is transactional, so a failed parse or persistence
  operation does not publish a partial snapshot.
- Catalog and version selection is explicit. `CatalogKey` is a caller-supplied
  stable source/catalog identity, not a display name, header name, or declared
  version. Multiple catalogs with the same human-readable name remain distinct
  when they have different keys.
- A `SnapshotKey` identifies one catalog, retained document, and parser
  interpretation. Re-importing the same document under the same interpretation
  is idempotent; changed bytes or a changed interpretation produce a separate
  immutable snapshot and import history. Earlier snapshots are retained rather
  than silently replaced.
- A declared version is descriptive source metadata, not a unique key or an
  ordering rule. It may be absent or repeated. Callers choose the exact
  `SnapshotKey` to inspect or compare; there is no implicit “latest version”
  selection and no change to the existing caller-supplied `CatalogKey`
  behavior.
- Snapshot history and cross-catalog comparison are separate operations.
  History diffs compare snapshots of the same `CatalogKey` and use catalog
  scope to distinguish removals from unknown absence. Requirement
  reconciliation accepts two explicit, distinct snapshots, including snapshots
  from different catalog keys; it compares expected asset evidence and related
  source assertions without consulting local inventory or asserting possession.
  Incompatible evidence scopes remain unknown, weak matches remain candidates,
  and shared assets do not imply that their containing sets are identical.
- `plan_build` and audit consume the selected catalog plus cached scan evidence
  without writing output. Audit is cached by default; explicit refresh scans
  and persists first. A one-shot build (including dry-run or strict-missing)
  imports the DAT and refreshes scan evidence before planning. Dry-run and
  incomplete strict plans write no ROM artifacts; strict missing content keeps
  the established CLI exit code `2`.
- Multi-root scans canonicalize and deduplicate roots, scan all requested roots
  before mutation, then replace their inventory scopes in one SQLite
  transaction. A failed root scan leaves all requested scopes untouched. A
  single-root scan has the same scan-completely-before-replace rule.
- Incremental reuse is explicit and single-root only. The default always
  rehashes; opted-in reuse is restricted to bare files whose persisted Unix
  identity/size/mtime/ctime stamp matches, while archives always get rescanned.
  Missing or changed stamps and `--force-rehash` paths hash normally. The stamp
  is a freshness hint, never a content digest; build-time source-byte
  verification remains authoritative, and unsupported platforms disable reuse.
- Outputs are not a single transaction across groups. Each ZIP is written to a
  sibling temporary file, synced, then replaced atomically on Unix. Each
  directory is built in a private sibling stage; replacement backs up an
  existing real directory and attempts rollback if installation fails. The
  report marks completed, failed, unattempted, and recoverable-backup outcomes.
  A process crash during directory replacement can leave the old tree under
  its hidden backup name and the destination temporarily absent. Later groups
  are not attempted after the first failure. Source trees are never modified.

## Change recipes

| Change | Primary home | Coupled boundary / evidence |
| --- | --- | --- |
| Add a catalog input format | Add an adapter and format hint/dispatch; map records into the common typed snapshot; add fixture coverage and import compatibility tests. | `storage::catalog_import` owns normalized persistence; test source locations, unsupported-extension retention, malformed input rollback, and idempotent import. |
| Add a source/archive backend | Implement the backend in `sources`; keep discovery and selected-member identity in the scan/source model. | Exercise listing/read errors, identity/fingerprint changes, duplicate names, and writer verification; preserve scan-before-cache-replace. |
| Change matching | Add typed evidence/policy behavior in `domain` and pure resolution logic in `resolution`. | Test permutations, partial/conflicting evidence, ambiguity, roots/catalog isolation, and audit/build agreement; keep SQL and CLI out unless their contracts actually change. |
| Change logical layout | Add or evolve a pure policy in `build::mame_layout` or `build::planner`; keep MAME snapshot dependencies distinct from legacy build modes. | Test explicit group contents, dependency diagnostics, input permutations, path collisions, and provenance before writer integration. |
| Add an output container | Add the container choice to `domain`, route it in `app`/CLI, and implement it behind the shared validation and verified writer boundary. | Test staging, replace/rollback failures, stale-source rejection, and byte-tree equivalence; document per-artifact and multi-artifact guarantees. |
| Add a user workflow or frontend | Compose operations in `app`; keep terminal rendering and exit-code mapping in the CLI. | Test the public request/report behavior and ensure reusable operations do not initialize terminal state. |

The dependency graph is intentionally not a claim that every change is
one-file-only. It identifies the narrowest owning boundary and the contracts
that a change must preserve.

## Change-propagation evidence

The original baseline had scan, matching, planning, and writing concerns
co-located in the command-oriented implementation; changing audit or source
selection could therefore cross CLI, matching, and writer paths. In the
implemented proof slices, the observed diffs are narrower:

| Proof slice | Files changed | Observed propagation |
| --- | ---: | --- |
| Standalone audit (`6c7657b`) | 10 | app/domain/CLI reporting and options; no writer, source backend, or SQL changes. |
| Ordered roots (`7a1ef8c`) | 12 | app/domain/CLI, planner/resolution, repository queries, and dedicated roots coverage; no parser or output-writer changes. |
| Directory output (`f264943`) | 10 | app/domain/CLI and shared writer/validation/docs/tests; no matching, parser, or SQL changes. |

These counts describe those ticket commits, not the full refactor. From the
MAMEC-3 baseline commit `1375390` through the directory-output commit
`f264943`, the cumulative history changes 78 files (+17,571 / -1,276 lines),
including catalog-format, evidence, scan, and build milestones. It should not be
read as the size of any single proof slice.

## Test evidence and remaining gaps

The branch-added tests are organized around behavioral contracts rather than
module line coverage:

- Migration tests populate pre-migration caches and check legacy row identity,
  retained-document unavailability when bytes are absent, preservation of ROM
  requirements, idempotent forward migrations, and reversal of the newer
  snapshot/document extensions.
- Scan tests establish that incomplete inventories do not replace prior
  observations; multi-root tests establish canonical alias deduplication,
  precedence for overlapping roots, root-scoped cache isolation, and all-roots
  scan-before-transaction behavior.
- Resolution and audit tests vary input order, partial and conflicting
  evidence, catalog/root scope, duplicate candidates, and missing requirements;
  they check deterministic choices and agreement between audit and build
  reports.
- Plan/writer tests reject unsafe paths, case-folded collisions and
  file/directory overlaps; inject read, write, finalize, and replace failures;
  verify stop/rollback/report outcomes; detect stale source or archive identity;
  and compare ZIP contents with directory trees under both supported layouts.
- Adapter tests use synthetic fixtures to cover supported format records,
  source locations, extensions, malformed input, and catalog import behavior.

The MAMEC-16 final `devenv test` gate passed after this document's changes:
224 Rust tests (168 library, 1 CLI, 4 audit, 4 catalog-fixture, 13
catalog-import, 32 integration, 2 multi-root), strict all-target/all-feature
Clippy, repository formatting/script checks, and CLI help smoke. The explicit
`cargo check --locked --workspace --all-targets --all-features` and
`RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --all-features
--no-deps` checks also passed. `cargo-machete` reported four candidates:
`infer`, `libz-sys`, `md-5`, and `sha-1`. The latter three are expected naming
or intentional-feature false positives (`libz-sys` is directly retained as a
documented zlib-ng workaround); `infer` has no source reference and is a
plausible stale direct dependency. It was not removed because doing so requires
a lockfile regeneration, contrary to this ticket's locked-resolution/no
blanket-update constraint. `cargo tree -d` reports duplicate low-level
versions along ZIP/r7z/hash/test dependency paths; no broad update was made.

The synthetic 100-ROM benchmark compares the unchanged benchmark harness at
the MAMEC-3 baseline (`1375390`) with the completed MAMEC-15 implementation
(`f264943`): same DAT/source corpus, 32-core host, stable Rust 1.98.1, binary
runner, `parent-bundles`, `--missing fail`, and 5 runs per case (9 for the
parallel deflate repeat). Baseline means were 157.4 ms (1 job/deflate), 162.7
ms (1/store), 158.2 ms (8/store), and 161.9 ms (8/deflate, 9-run repeat).
Current means were 719.7 ms, 712.7 ms, 654.0 ms, and 722.6 ms respectively;
the last sample is the 9-run repeat (722.6 ± 61.9 ms, range 666.7–852.6 ms).
This small-corpus end-to-end result is roughly 4.0–4.6x slower and is a
material performance regression, not evidence of a speedup. It measures
startup, parsing/import, scan/cache, planning, and output together and does not
identify the cause. Treat it as a follow-up profiling item; do not infer that
one architectural boundary caused the difference. The input scripts are
unchanged, and old user profiling artifacts from another dirty revision were
excluded from the baseline.

Prioritized follow-up gaps:

1. Profile the measured small-corpus regression with phase-level timing before
   optimizing; retain the identical corpus and benchmark harness.
2. Keep broader multi-platform directory replacement guarantees explicit; the
   current atomic ZIP replacement statement is Unix-specific.
3. Expand compatibility fixtures when additional real-world catalog dialects
   or archive backends are added. Current behavior is validated primarily with
   focused synthetic fixtures, not a broad external corpus.
4. Revisit crate publication/MSRV policy only when external Rust API stability
   or crates.io distribution becomes a project goal.
