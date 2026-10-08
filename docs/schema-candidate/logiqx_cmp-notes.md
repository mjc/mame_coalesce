# Logiqx and ClrMamePro candidate

This is a design artifact, not production schema or approval. The native
fragment is [logiqx_cmp.sql](/home/mjc/projects/mame_coalesce/docs/schema-candidate/logiqx_cmp.sql).
It depends on `shared.sql` and `relationships.sql`; the parent assembler adds
cross-family owner, same-edition, field-presence, mixed-order, closure and
publication checks from [logiqx_cmp-owners.tsv](/home/mjc/projects/mame_coalesce/docs/schema-candidate/logiqx_cmp-owners.tsv).

Policy is taken from MAMEC-DOC-11 seq 38144 and MAMEC-DOC-21 seq 38132.
DOC-11's exact Logiqx owner/state/code ledgers, accepted root-field contract,
text-child cardinalities and CMP lexical-ownership section define the native
facts. DOC-21 defines global source-element IDs, common set/media identity,
physical-root diagnostics and the separate CMP identity/order exception.

## Table and field inventory

The fragment defines 31 native relations and 14 position relations plus the
CMP position-token view and local integrity view. Logiqx has 21 native owners and 11 attribute-position
families. Its closed position codes cover 46 accepted attributes: document
build/debug; ClrMamePro and RomCenter options; game, release, BIOS, ROM, disk,
sample, archive and compatible device-reference attributes. The remaining
accepted fields have explicit typed owners: ten header text-child kinds, three
game text-child kinds, repeated game comments, and compatible root
`file_name`/`sha1` children. `catalog_sets` alone owns each game name and root
placement. ROM/disk hashes are declarations in `catalog_entry_hashes` with one
position FK; the root SHA-1 is a distinct non-media `hash_values` reference.

ClrMamePro has ten native relations and three position families. Their closed
ledgers cover 15 header, 12 set and 12 ROM fields. The header's five directive
values live in its keyed options facet. Set link literals and ROM merge
literals stay on their typed relationship owners. ROM CRC and CRC32 remain
separate source fields despite sharing the `crc32` algorithm. Hash values and
positions use the common hash declaration owner. Each sample is one media
entry; it has no inferred size, digest or file UUID. Comments use one global
source ID and an annotation owner in the document. They have exact text and
unique start coordinates, with no copied syntax parent, form-item order or
comment rank.

The owner manifest has 21 registry-bearing native owners. Common set bases
are not extra owners: `logiqx_games.set_id` and `clrmamepro_sets.set_id` are
their typed native owners, while `catalog_sets` supplies the actual set
placement and name. Position rows and payload facets are excluded. The
physical Logiqx and CMP document owners and virtual root groups have no
registry IDs; `logiqx_documents`, `clrmamepro_documents`, and their group
relations are recorded here as non-registry roots rather than synthetic source
elements. [logiqx_cmp-hash-positions.tsv](/home/mjc/projects/mame_coalesce/docs/schema-candidate/logiqx_cmp-hash-positions.tsv)
maps all nine media hash position codes to the shared declaration field. It
keeps Logiqx `crc` distinct from CMP `crc`/`crc32`; CMP reports its sole hash
link on the `Value` role of the composite position, never on both token roles.

Six position tables carry `relationship_id UNIQUE` FKs to
`reported_catalog_relationships(relationship_id)`: Logiqx game codes 3/4/5,
device-reference code 0, ROM code 5 and disk code 3; CMP set codes 1/6 and ROM
code 6. Each relationship code requires a non-NULL ID; all other codes forbid
one. The native link/merge alone owns the target literal. The integrity view
checks all nine code-to-native-relationship mappings, including missing,
unexpected and substituted position links. CMP exposes the reference ID only
on the `Value` anchor, like its hash link.

## State, keys and ordering

All ordinary Logiqx child IDs are the global source-element ID. ROM, disk and
sample IDs reuse that same ID as `media_entry_id`. A source placement has one
nonnegative integer order in its immediate parent's sequence. The Logiqx root
sequence contains compatible `file_name`, compatible SHA-1, the optional
header, and catalog sets. Header text and option elements share the header
sequence. Game text/comments/releases/BIOS sets/archive and device references
and ROM/disk/sample declarations share the game sequence. Nested position
ordinals remain local to their attribute-bearing owner and do not replace child
order.

`candidate_logiqx_cmp_integrity_problems` is the family-local executable
report. It checks expected versus actual closed field-position rows, rules
family/version, strict DTD versus compatible requirements and extension
owners, root-group kind/edition, CMP header/comment cardinality seals, keyed
payload facets, typed-parent mixed-order collisions, CMP source-order versus
opening/keyword coordinate inversions, comment/token coordinate collisions,
keyword-before-value order, canonical hash-code/Value-role mapping and
relationship-position agreement. Hash mapping diagnostics resolve edition
through the actual native position owner and its source-element registry row;
a declaration's media ID is never used as an edition ID. The
assembler still owns cross-family registry-kind/edition/one-owner checks,
decoded-source bounds, document/child containment, publication gates, and
whole-catalog closure. The view is diagnostic, not a trigger or a claim that
invalid rows cannot be staged.

Logiqx present-empty ordinary text remains `''`; absent optional text has no
child row. Release/BIOS/status/debug/directive enums are closed. The nine
defaulted option-presence bits and `isbios_was_present`/`status_specified` are
retained; presence for nullable header/plugin text derives from NULL versus
text. Compatible ROM serial is sparse in `logiqx_rom_compatibility`. Supplied
ROM size remains exact text even when empty, nonnumeric or out of range; its
virtual numeric projection is nullable. Strict DTD mode requires size. Hash
presence/scope/value consistency, DTD-mode-specific required children and
accepted hash interpretation need edition-aware closure checks.

CMP preserves document form order, header field order, set-item order across
field pairs/ROMs/samples, and ROM field/flag order. One field pair consumes one
source order; the `Keyword` and `Value` token anchors are roles over its
composite `(parent, field_kind, field_occurrence)` position identity. The
view exposes those roles and coordinates without storing token rows or value
copies; value text is joined from the owning typed field. Singleton
field_occurrence is zero. CMP flags 10/11 have keyword coordinates only and
derive quoted=false. Sample media identity carries its item order and its own
keyword/value coordinates. Comments are ordered by decoded source line/column;
they do not consume item order and do not claim a syntactic parent. The full
document comment vector and `comment_count` seal remain part of the contract.

Defaulted Logiqx enums use their pinned defaults only when omitted; invalid
supplied enums reject. Strict enum normalization follows the DOC-11
standalone-external-DTD rule. CMP unknown directive/status text remains stored
as supplied; recognized effective directive modes are derived, and ROM dump
status is derived from `status_text` plus canonical flag declarations. Do not
add separate dump-presence booleans or a cached effective status.

## Root diagnostics and capture limits

`logiqx_documents` and `clrmamepro_documents` own root/document extent and
location columns under DOC-21. The names and vocabularies follow main's
diagnostic contract: `extent_view`, `extent_start`, `extent_end`,
`location_view`, `start_line`, `start_column`, `end_line`, `end_column`, and
`column_convention`. Byte ranges are paired, nonnegative and nonempty; each
available coordinate endpoint is a paired positive one-based Unicode-scalar
line/column. A declared location view requires a non-NULL
`column_convention = 'one_based_unicode_scalar'`. Start and end are nullable
independently. Logiqx permits
`retained_original_bytes`/`transport_decoded_xml_bytes` and their corresponding
text views. CMP uses `retained_original_bytes` and `decoded_dat_text` for its
location view. Main must enforce available-source bounds and diagnostic
containment and require at least one verified extent/location system before
linking a root diagnostic.

These are capture requirements, not claims about current parser output. The
Logiqx candidate needs the actual `<datafile>` root end extent and closing
coordinates; its current capture evidence does not authorize synthesizing them
from a root opening point. CMP needs the full decoded-document extent,
including annotations, plus end coordinates; current comment/owner records do
not establish those document boundaries. Ordinary child owners retain their
opening locations only. Those points cannot establish full child containment.
Edition-aware rules-family checks, mandatory-field/presence closure, registry
kind/edition/one-owner checks, cross-table mixed-order and coordinate
collisions, hash/relationship agreement, and count-seal validation belong to
the assembled harness/finalizer.

The registry manifest intentionally marks `logiqx_games` and
`clrmamepro_sets` with `sequence=-`: their order is stored once on the common
`catalog_sets` row. Main's mixed-order generator must fold those inherited
placements into the Logiqx root sequence and CMP document-form sequence. CMP
set scalar/flag positions likewise consume the set-item `source_order` but
have composite keys and no registry identity, so they are deliberately absent
from the owner manifest. The CMP mixed-order check must union
`clrmamepro_set_field_positions.source_order`, `clrmamepro_roms.source_order`,
and `clrmamepro_samples.source_order` under the actual `set_id`; adding position
rows as registry owners or copying order onto `clrmamepro_sets` would violate
the selected identity contract. CMP header field positions and ROM field
positions have their own attribute-local order domains.

The accepted compatible Logiqx raw root ordinal is also new capture work: the
current reader does not retain gaps for ignored direct-root elements. Strict
mode rejects the compatible-only filename and SHA-1 children. Do not use the
filename as acquisition metadata or treat the declared root SHA-1 as a digest
of the retained XML bytes. For Logiqx media hashes, use `unknown` scope unless
the specific edition's reading rules establish complete-file coverage; TOSEC
labels and the old `whole_asset` tag do not prove it.

## Existing schema and consumers

`sem_context(SCHEMA)` confirmed that current `src/storage/db/ddl.rs::SCHEMA`
still composes `logiqx.sql`, `logiqx_attribute_positions.sql`, and the legacy
CMP fragments. These candidate tables are not wired into that production
constant. Import consumers currently enter through
`src/storage/catalog_import/logiqx_native.rs` and
`src/storage/catalog_import/cmp_native.rs`. Source-free Logiqx reads use
`src/storage/catalog_logiqx/document.rs`, `games.rs`, `media.rs`, and
`positions.rs`, with the shared media projection in
`src/storage/catalog_files/logiqx.rs`, history in
`src/storage/snapshot_history/logiqx.rs`, and the independent
`examples/logiqx_xml_native_verify` verifier. CMP reads use
`src/storage/catalog_clrmamepro/reader.rs::sets_for_snapshot`, then its
document/header, positions, sets, and query modules; native payloads are also
consumed by `examples/clrmamepro_native_verify`. The candidate mapping keeps
their documented source-free fields but replaces snapshot keys/family ranks
with edition/native identities and typed parent-local order. This is a design
crosswalk, not a claim that those consumers have been ported.

## Witnesses and branch test explanation

`logiqx_cmp_witnesses.sql` is a bounded SQLite-only candidate witness. It seeds
one Logiqx edition and one CMP edition, then checks representative root and
native ownership, defaults and explicit states, a quoted/empty CMP scalar,
sample identity/order, hash declaration-to-position linkage, keyword/value
anchors, comment coordinate ordering, rejection of closed-code, duplicate
singleton, malformed-size and duplicate-coordinate rows, integrity-view
reports for mixed-order collisions, missing positions, token/comment coordinate
collisions, source-order inversion and strict-mode game count, and index-backed
parent-order/comment-coordinate query plans on populated rows. It does not
execute Rust importers, scan/import corpora, prove cross-family
ownership/edition/source-bound checks, or replace the parent assembler's full
candidate check.

The witness also covers Banach review
`01a119fa-da75-7e93-8230-478660ef464c`: NULL/nonpositive CMP scalar and hash
value coordinates reject while flags retain keyword-only coordinates; a
declared root location with NULL convention rejects; all nine relationship
codes have valid native/position links, require IDs and forbid IDs on other
codes. A real but substituted relationship ID reaches the agreement report.
Wrong hash mappings use fresh declarations and unoccupied position keys/orders,
so they must insert successfully and then produce the intended integrity-view
diagnostic. A cross-edition declaration-owner substitution verifies that the
report uses the native position owner's edition. These cases do not reuse an
already claimed hash ID to obtain an unrelated UNIQUE rejection.

The other branch-added SQL witnesses exercise distinct candidate slices:

- `mame_witnesses.sql` checks one machine with mixed ROM/disk/sample/device
  children, nested device instance/extension order, a hash-position link,
  invalid enum and duplicate-position rejection, integer provenance, and a
  populated parent-order query plan.
- `software_witnesses.sql` checks a part/area/ROM file chain, default and text
  states, malformed numeric retention, closed enum and position uniqueness,
  and a hash-position FK.
- `no_intro_witnesses.sql` checks DAT, synthetic P/C and observed export
  owners, hash-position links, non-media NFO hash separation, nested versus
  sibling header identity, unknown extents, and populated owner/position query
  plans.
- `relationships_witnesses.sql` is the shared relationship guard fragment:
  source/user/derived origin agreement, one origin payload, append-only review
  and evidence seals, conflict-side ownership, and replacement-cycle checks.

These are bounded design SQL witnesses/guards, not production tests. The
repository's application tests remain separate from this artifact.

## Known gaps

The source contract specifies accepted data and the candidate relations cover
it, but no complete parser-to-target proof exists here. In particular, fresh
raw root-order capture, actual document/root extents, CMP keyword-coordinate
capture, hash/relationship/presence/count publication enforcement, source-free
hydration, and populated target query plans remain open. DTD strict versus
compatible root/content rules and source-text/hash-scope projections need
edition-aware validation. No authentic TOSEC grammar or whole-file digest
semantics are inferred. PLAN-3 design approval remains open.
