# Target profile decision: MAME 0.289

Status: selected for the first snapshot-pinned MAME view.

## Target and expected behavior

The profile is the standalone MAME core at version `0.289`. It describes the
MAME ROM-path naming and lookup contract; it does not claim that a selected
image boots successfully or that every device/media instruction is supported.
MAME 0.289 was released on 31 July 2026 and is the latest release listed by
MAMEdev as of this decision on 28 September 2026.

MAME searches software-list ROMs by list/item short names with parent-item and
item-only fallbacks; CHD placement follows a separate directory convention.
This ticket's manifest currently projects machine ROM-set layouts only. The
existing `LogicalEntry` model does not preserve software-list item, part, area,
or component identities, so this profile makes no claim about representing or
materializing software-list media. A future software-list projection must add
those typed identities and follow MAME's media-specific path rules rather than
flattening components. MAME's own audit/load result is separate compatibility
evidence, not implied by this manifest.

References: [MAME 0.289 release](https://www.mamedev.org/?p=3),
[MAME asset search behavior](https://docs.mamedev.org/usingmame/assetsearch.html),
[MAME software-list usage](https://docs.mamedev.org/usingmame/usingmame.html).

## Manifest contract

The manifest pins one exact `SnapshotKey`, selected roots, the target profile,
layout and policy choices, and their independent non-zero policy versions. Its
JSON envelope has format version `1`; readers reject unknown versions and
non-canonical ordering rather than guessing. Constructing or importing a newer
snapshot cannot mutate an already-created manifest.

The initial policy set includes the runtime dependency closure, MAME ROM-path
short-name conventions, preservation of catalog component representations,
and snapshot requirements plus observed-content evidence. These are plan
inputs, not permission to execute source load instructions or perform arbitrary
transformations. `MameSetLayoutPolicy` remains distinct from the legacy
`parent-bundles`/`per-game` CLI choice.

The manifest is descriptive and format-neutral. It does not include ZIP,
directory-writer, archive-handle, or output-compression settings. Existing CLI
grouping and output-container adapters remain unchanged; a future materializer
may adapt pinned logical groups to either output backend and must revalidate the
selected content before writing.

## Tests added with the profile

- Profile identity tests pin the MAME release and stable serialized ID.
- Serialization tests round-trip version `1`, reject unknown and non-canonical
  fields at the envelope and nested levels, and reject unknown format and policy
  versions, reject non-canonical order, and reject roots that disagree with the
  roots captured by the planned layout; reordered diagnostics and duplicate
  groups, entries, and provenance records are rejected. The decoder rejects
  input over 64 MiB before parsing and checks canonical serialization without
  allocating a second complete encoding. Snapshot-bearing diagnostics must
  match the manifest's pinned snapshot; stale extra-closure diagnostics still
  round-trip when they belong to an unselected root. Imported output groups
  must have matching path-validation diagnostics, and nested provenance
  requirements reject reordered or duplicate values and evidence that does
  not agree with the selected output entry.
- Snapshot tests show a newer selected snapshot cannot retarget an existing
  manifest.
- Planner permutation tests reorder selected roots, dependency closures, and
  resolved inventory sets, then compare both manifest values and serialized
  bytes.
- Boundary tests ensure ZIP and directory-writer choices do not leak into the
  pure layout manifest and policy versions cannot be zero.
