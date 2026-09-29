# Catalog identity schema

The identity tables introduced by migration
`2026-09-24-000000_create_catalog_identity_schema` keep catalog provenance
separate from the legacy import cache. Keys are opaque, caller-assigned durable
identifiers; display names, URLs, and SQLite row IDs are descriptive data, not
identity. Reusing a key for a different entity is a primary-key conflict and
must be resolved by the writer, not by silently merging records.

Declared catalog versions are nullable labels and are intentionally not unique.
Scope is also explicit: `unknown` means the available metadata cannot establish
scope; it is not equivalent to a complete catalog. Filtered and partial scopes
retain caller-provided JSON details. Snapshots capture immutable catalog
interpretations, while import runs record separate execution attempts, so a
parser/rules change does not overwrite prior history.
When acquisition provenance is present, snapshots and runs are constrained to
the acquired document they reference.
Snapshot parents are limited to the same catalog; cross-catalog continuity and
derivation belong to typed relationship assertions rather than snapshot history.

The migration is additive. Existing `data_files`, `games`, `roms`, and
`rom_files` rows remain intact, but no identity rows are synthesized from their
names or local IDs because the old cache does not record publisher/catalog
provenance. Such legacy provenance remains unknown. Future importer integration
must assign explicit keys and is handled separately from this schema migration.
