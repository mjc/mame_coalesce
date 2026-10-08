# Logiqx and ClrMamePro candidate

This is a design artifact, not production schema or approval. The native
fragment is [logiqx_cmp.sql](/home/mjc/projects/mame_coalesce/docs/schema-candidate/logiqx_cmp.sql).
It depends on `shared.sql` and `relationships.sql`; the parent assembler adds
cross-family owner, same-edition, mixed-order, closure and publication checks
using [logiqx_cmp-owners.tsv](/home/mjc/projects/mame_coalesce/docs/schema-candidate/logiqx_cmp-owners.tsv).
The installed shared field/value-position audit consumes the separate
[logiqx_cmp-field-presence.tsv](/home/mjc/projects/mame_coalesce/docs/schema-candidate/logiqx_cmp-field-presence.tsv)
and participates in the actual candidate integrity/publication gate.

Policy is taken from MAMEC-DOC-11 seq 38144 and MAMEC-DOC-21 seq 38132.
DOC-11's exact Logiqx owner/state/code ledgers, accepted root-field contract,
text-child cardinalities and CMP lexical-ownership section define the native
facts. DOC-21 defines global source-element IDs, common set/media identity,
physical-root diagnostics and the separate CMP identity/order exception.

## Table and field inventory

The fragment defines 31 native relations and 14 position relations plus the
CMP position-token view and local integrity view. Logiqx has 21 native relations and 11 attribute-position
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
virtual numeric projection is nullable. Strict DTD mode requires size.
Native hash-declaration/position presence is now checked by the shared
generated audit. Hash scope/interpretation and DTD-mode-specific required
text children are separate from this presence manifest; existing family
cardinality/state audits remain enabled.

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
The assembled harness already supplies edition/rules-family, registry
kind/edition/one-owner, mixed-order, coordinate and hash/relationship checks,
and now the shared field/value-position audit. Those executable candidate
checks do not establish real parser capture or independent source count
seals. Complete child-extent containment cannot follow from opening points.

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

### Field-inventory freeze at main e591363

The independent inventory is transcribed from MAMEC-DOC-11 **seq 38144**,
not inferred from the candidate's number of tables or populated TSV rows:

- Logiqx: 46 attributes in 11 local code domains; ten header text kinds;
  three game text kinds; repeated game comments; two compatible root text
  children. This is 62 field/text entries, including 15 defaulted enum fields
  with explicit presence (nine on option owners).
- CMP: 15 header, 12 set and 12 ROM codes (39 total); repeated scalar samples
  and document comments are separate, for 41 entries. Two additional lexical
  facts record retained `source_block` spelling on header/set forms; they are
  not extra scalar codes. Both `game` and `set` forms are accepted.

The resulting [field ledger](/home/mjc/projects/mame_coalesce/docs/schema-candidate/logiqx_cmp-field-coverage.tsv)
has 105 entries. Each `value_owner` is one actual SQL relation; `value_column`
contains only exact columns, optionally comma-separated. Joins to hash bytes
and relationship declarations are described in evidence/default rules, not
encoded as invented columns. XML attribute wire names carry `@`. Text children
have textual tag codes and `position_table=-`; their real typed child
`field_kind` numbers are given in evidence. They own their element placement
themselves and do not become attribute positions. CMP sample/comment/form
entries likewise have no invented scalar code. Numeric codes cover the 85
attribute/CMP positions; the 13 XML text kinds are separate closed domains.
There are 21 distinct field-owning table names and 32 distinct canonical
value-owner relations in this ledger; these are not registry-owner counts.
The unchanged manifest still has 21 registry-bearing native owners, plus the
two non-registry document roots described above. No new runtime owner,
value, order, ancestry, JSON, EAV or token-storage relation was introduced.

Required pinned DOC-11 sections were read completely: normative text-child
ownership; accepted root fields; device-reference correction; reconciled
Logiqx dictionary; complete Logiqx state/code ledger; field-owner
reconciliation; the ClrMamePro text section including exact CMP field/sample/
comment keys; exact Logiqx child/media crosswalk; current CMP comment-capture
boundary; and remaining dictionary gaps. DOC-21 seq 38132 remains the pinned
identity/order/root-diagnostic basis, with main's supplied common interface.

Sem traces checked current DTD attribute declarations, XML readers, CMP
`parse_header`/`parse_set_facts`/`parse_asset_facts` and `reader::read_with`,
production `SCHEMA`, current native SQL facts, `DocumentDetails`,
`insert_logiqx_set_facts`, `insert_set_facts` and actual source-free consumers
`hydrate_rows`, `sets_for_snapshot`, `load_parents`. The CMP writer trace
included its position and relationship callees in one context call (3 ms).
This confirms the current-to-candidate aliases and the accepted `game|set`
forms; it does not port any consumer to candidate SQL.

### Independent all-code field witness and checker

[logiqx_cmp_field_witnesses.sql](/home/mjc/projects/mame_coalesce/docs/schema-candidate/logiqx_cmp_field_witnesses.sql)
constructs typed owners independently of both the ledger and the older
representative witness. It executes every one of the 85 position codes and
all 13 text-child kinds, with actual linked hash/relationship declarations,
root filename/SHA-1 owners, samples and document/game comments. Required text
is explicitly empty, not NULL. It tests all 15 explicit versus omitted XML
defaults, grouped optional text/hash/link omission, quoted empty CMP text,
unknown/empty directives, and CMP's omitted `forcenodump` effective default.
SQL assertions execute inside savepoints before rollback; rollback does not
skip the assertion. It also reaches the intended CMP invalid-hash diagnostic
while retaining Logiqx's distinct empty/invalid declaration states.

[logiqx_cmp_field_check.py](/home/mjc/projects/mame_coalesce/docs/schema-candidate/logiqx_cmp_field_check.py)
is a stdlib SQLite unittest runner for this design only. Its nine tests:

- Compare an independently transcribed DOC-11 inventory to each ledger
  wire/code entry, and validate every real owner/value column using SQLite
  metadata.
- Compare actual DDL closed-code CHECKs and populated codes for all 14
  position tables and two typed text-child tables; reject out-of-range,
  fractional, malformed and NULL codes at the precise storage/check layer.
- Remove each populated position separately. The 83 value-backed positions
  produce `missing_position`; CMP's two flags are the declarations themselves
  and correctly have no independent missing-value report.
- Reject empty/invalid enum tokens and malformed presence bits for all 15
  defaulted XML fields. The independent SQL exercises their absence/default
  transitions.
- Reject NULL required text and malformed CMP sizes while preserving those
  same malformed/empty size texts in Logiqx with a NULL numeric projection.
- Reject CMP code/keyword disagreements and missing/nonpositive scalar
  coordinates; test both original-case set forms and the root SHA-1 update
  guard at its trigger layer.
- Insert a fresh, otherwise valid, unclaimed hash declaration and substitute
  it into a CMP position. The expected `cmp_hash_mapping` report carries the
  native CMP edition, not a coincidental UNIQUE rejection or media ID.
- Check three populated parent-order/annotation queries use index searches
  without temporary sort B-trees. This is bounded plan evidence, not a corpus
  performance measurement.
- Mutate each of the three row-selected field assertions to match nothing
  (selector 999) and to match multiple rows. All six mutated fixtures must
  fail at `field_assert`'s `ok=1` CHECK. The assertions use an aggregate that
  always emits one row and requires exactly one matching, true evaluation;
  they cannot silently succeed by inserting zero assertion rows.

Both SQL fixtures require a fresh empty SQLite database, repository-root CLI
execution and STRICT support (SQLite >= 3.37). They deliberately supply
minimal common-key/hash/relationship fixtures, not main's complete shared
schema. The Python runner expands only the native DDL include and can run
from any directory. Its extended STRICT datatype check requires SQLite error
code 3091 even on Python builds that name that code `unknown`.

Local DDL corrections exposed during this pass: accept both CMP set-form
keywords; bind each CMP keyword to its exact field code case-insensitively
without erasing original case; guard root SHA-1 hash-reference updates as
well as inserts; report CMP empty/invalid hash declarations as
`cmp_invalid_hash_state`. Public table/column names, position codes,
`relationship_id` semantics, owner manifest and hash manifest are unchanged.
The root update guard does not by itself prove shared hash-value immutability.

Freeze checks (all exit 0, no production build/import/gate):

```sh
sqlite3 :memory: < docs/schema-candidate/logiqx_cmp_field_witnesses.sql
python3 docs/schema-candidate/logiqx_cmp_field_check.py
sqlite3 :memory: < docs/schema-candidate/logiqx_cmp_witnesses.sql
```

The Python suite reports nine passing tests. The new field SQL reports
`Logiqx/CMP independent field witness: passed`; the existing family witness
continues to pass. Changed files in this pass are the native SQL, coverage
TSV, independent field SQL, Python field checker and these notes. No other
family, main-owned integration file, Lific page or production file was edited.

Banach's zero-row assertion finding is fixed in the three field SQL assertions
for quoted empty header text, unknown/empty directives and omitted
`forcenodump`. This re-freeze changes only the field SQL, Python checker and
notes; native DDL and field/owner/hash inventories are unchanged. The ninth
test covers missing and multiple selector matches independently for all three
assertions; the unmutated SQL fixture still passes.

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

The field ledger enumerates the pinned accepted-field contract; it is not
blanket all-field approval or complete parser-to-target proof. General
field/value-position presence is installed in the actual candidate
integrity/publication gate; it is no longer an installation gap. Real raw
root-order capture, actual root/document extents, CMP keyword-coordinate
capture, independent source count seals, production source-free hydration
and representative corpus-scale target query plans remain open. Full parser
proof of strict versus compatible root/content rules and source-text/hash
scope semantics is not established by these SQL tests. No authentic TOSEC
grammar or whole-file digest semantics are inferred. PLAN-3 design approval
remains open.

The all-code fixture proves constructed native storage/positions, not that
the real parser captured all supplied optional fields. Erasing an optional
value and its position together can remain internally consistent. Losing a
CMP flag, sample or optional text child can likewise leave no independent
native value from which to reconstruct presence. Independent parser-side
count seals, coverage inventories and source comparisons must catch those
losses; no test here claims otherwise. Full strict DTD child ordering,
standalone-external enum normalization, XML shape/CDATA/whitespace behavior,
root SHA-1 lexical parsing and alias-conflict/status interpretation are not
proved by constructing SQL rows. The CMP effective ROM-status rule is stated
in the ledger but a complete source-free status-query witness is still open.
Bounded assembled FK-off auditing and publication rejection are now tested
below; this is not complete model-wide publication or parser implementation
proof. Hash lexical-override interpretation, complete source coverage and
root/child extent containment require their own selected-rule/source proofs.
No authentic TOSEC, PureDOS or broader nested CMP grammar is inferred.
No NoIntro or software proof is added here: the 135 versus unlocated 137
evidence contradiction stays unresolved, authentic P/C remains unsupported,
and software numeric/load qualification still needs its independently checked
derivation chain.

## Executable field/value-position presence freeze

Source policy was reread through Lific's complete Logiqx state/code and exact
CMP field/sample/comment sections at DOC-11 seq 38144. The sem skill/MCP
provided the route reader, presence generator, edition-scope path, assembler
and existing fixture loader structure. The family SQL audit lookup returned
not found, so its existing expected-position CTEs were read directly as the
documented SQL fallback; no native SQL semantics were changed.

The new six-column presence manifest contains exactly the independent DOC-11
seq 38144 dictionaries: 46 Logiqx attributes and 39 CMP fields/flags across
14 physical position tables. Each `(position_table, field_code)` occurs once.
Its owner is the position's real typed FK target, never a payload facet used
as a surrogate parent. `field_kind` is the canonical code and all singleton
`field_occurrence` values are zero. Predicates are closed build-time SQL over
alias `owner`; they do not create runtime EAV/owner/value tables or copies.

Required attributes use `1`; nullable text uses `IS NOT NULL` so supplied
empty text remains present; all 15 defaulted Logiqx enums use their explicit
presence bits, not their effective default values. Merge/serial/options/detail
payloads and set links use keyed typed EXISTS predicates. Set-link kinds are
distinguished. Hash predicates key the real media owner, source hash field
and occurrence zero, never merely equal digest bytes. Logiqx empty/invalid
hash declarations still require positions. CMP invalid hash interpretation
remains rejected by the existing family audit, not erased by a presence
predicate. Original-case CMP `game` and `set` spelling does not change the
set's closed field domain.

The two CMP flags are deliberately different: their positions ARE their
declarations. Their predicates read their own exact owner/code/occurrence
rows; there is no duplicate flag bit or value facet. The shared audit can
check position identity/occurrence consistency but cannot prove a lost flag
event from its absence. Independent parser-side flag coverage remains needed.
This is an explicit limit, not 85 independently value-backed fields.

[logiqx_cmp_presence_check.py](/home/mjc/projects/mame_coalesce/docs/schema-candidate/logiqx_cmp_presence_check.py)
uses the actual `assemble.assemble()` output, including the real shared
schema, relationship identities, generated field audit and publication gate.
It compiles that schema/generator once per class, uses one explicitly closed
in-memory SQLite connection and rolls mutations back with savepoints. Only
the constructed native INSERT portion of the existing family fixture is
reused; its placeholder common schema is NOT used. Exact fixture boundaries
are checked, shortened shared rows are replaced with real common identities,
and all nine reported relationships have real source identities/kinds.
The existing SQL witness and its nonvacuous assertions are unchanged.

The 14 tests independently compare physical DDL code domains, coverage TSV
and route keys, then exercise all 83 independently value-backed missing
positions; absent versus empty optional fields; all 15 omitted versus
explicit-default pairs; nullable CMP payload facets; typed set-link/merge/
compatibility facets; all nine hash fields and occurrence qualification;
equal-byte CRC/CRC32 aliases; Logiqx empty/invalid declarations; both CMP form
spellings; token-only flags; equal-count owner substitution; detached-registry
audit visibility; and actual missing/invented-position publication rejection
followed by published-position immutability. The baseline has no assembled
integrity or FK problems. Relationship declaration deletion already has a
stronger existing guard: tests verify that rejection, remove the position
first, then verify the exact facet route becomes absent. They do not weaken
or remove that guard.

The detached-registry case is explicitly simulated pre-existing corruption:
FK enforcement is off and only registry triggers are removed inside a
rollback-only test. A missing native ROM position still reports owner 107
with NULL edition rather than disappearing through an INNER JOIN. Ordinary
guarded writes are not claimed to permit detaching that row. Registry/FK
closure is separate from presence identity and is not replaced by this test.

RED evidence: a process-local mutation replaced the root `build` predicate
with constant `1`. The optional absent/invented-position test produced exactly
one expected failure, with no setup errors. Restoring the real manifest is
GREEN; no tracked file was mutated for that RED probe. The suite command is:

```sh
python3 -Werror::ResourceWarning docs/schema-candidate/logiqx_cmp_presence_check.py
```

GREEN: all 14 tests passed on the actual assembled schema (13.123 s in the
recorded focused run), with no ResourceWarning. The unchanged nine-test field
checker and nonvacuous field SQL fixture also pass. These are candidate tests,
not a production build, importer run, corpus verification or complete gate.

Required PCDATA child cardinality, root filename/SHA-1 children, sample/comment
source-event counts and full parser/coverage seals are outside this position
manifest; they keep their typed native owners and existing family checks.
Erasing an optional value and its position together still cannot be detected
without independent source evidence. XSI semantics are outside this family's
DOC-11 code dictionaries: this suite adds no XSI fields and grants no
cross-family nil interpretation proof. The installed candidate publication
gate is tested, but complete production parser/finalizer integration and
model-wide publication approval remain open. Only this manifest, its new
test runner and these notes change in this pass; native SQL, coverage,
owner/hash manifests, previous witnesses and main's harness are untouched.
