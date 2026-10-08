# MAME software-list candidate

This is a design artifact only. The candidate reads against `shared.sql`; it
does not alter the application schema or assert that the design is approved.

Policy sources are [MAMEC-DOC-12](https://lific.mjc.lol/MAMEC/pages/95), seq
38114, and [MAMEC-DOC-21](https://lific.mjc.lol/MAMEC/pages/138), seq 38132.
DOC-12's exact root/title/part crosswalk, exact area/media crosswalk, field
state ledger, all thirteen position ledgers, and ROM chain rules govern these
tables. DOC-21 supplies the software root diagnostic owner mapping and common
identity/edition/registry pattern.

## Owners and ordering

`software.sql` defines 21 native relations: document, wrapper root, canonical
list, wrapped-list placement, title, list notes, title text, title info, shared
features, parts, part features, switches, switch values, areas, data-area
details, disk-area details, ROM entries, disks, required files, load steps,
and clone links. The native source-owner manifest has 13 registry-bearing
owners; its ID, actual parent FK, parent key, placement sequence and media bit
are in `software-owners.tsv`. The title owner uses the common `catalog_sets`
key and inherits placement there, so its manifest sequence is blank. The
remaining roots, subtype facets and derived/relationship owners are listed
below and do not get invented source-element identities.

Sequence labels identify actual-parent domains, not the `source_order` column
name: wrapper lists, list children, title children, part children, switch
values, data-area entries and disk-area entries remain distinct. Main must
combine list notes with common `catalog_sets` placements in the list domain;
the title manifest row intentionally has no native order column.

The 37 accepted attributes have one native value owner each:

| Owner | Closed field codes in source order |
|---|---|
| wrapper | 0 build |
| list | 0 name, 1 description |
| title | 0 name, 1 cloneof, 2 supported |
| title info | 0 name, 1 value |
| shared feature | 0 name, 1 value |
| part | 0 name, 1 interface |
| part feature | 0 name, 1 value |
| switch | 0 name, 1 tag, 2 mask |
| switch value | 0 name, 1 value, 2 default |
| data area | 0 name, 1 size, 2 width, 3 endianness |
| disk area | 0 name |
| ROM entry | 0 name, 1 size, 2 crc, 3 sha1, 4 offset, 5 value, 6 status, 7 loadflag |
| disk | 0 name, 1 sha1, 2 status, 3 writeable |

Those codes are implemented by thirteen position-only relations. Each position
has field occurrence zero, one QName source order per owner, positive source
coordinates, and no copied attribute value. Only ROM crc/sha1 and disk sha1
positions require their unique `reported_hash_id`; other position rows require
NULL. Title attribute code 1 additionally requires the unique
`relationship_id` for the clone declaration; codes 0 and 2 require NULL. This
is a typed relation key, not a copied clone target. The common hash declaration
owns presence (`empty`, `invalid`, `value`), scope, lexical override and
normalized hash link. Omission has no hash row.

The software contribution to the count-seal inventory is all 13 manifest
element kinds/typed owners (2 media owners), all 13 attribute-position tables
with the code domains above (37 accepted field codes across their ledgers),
one optional list-notes singleton, four title text kinds with required counts
`description=1`, `year=1`, `publisher=1`, `notes=0..1`, and one load-step per
ROM media owner. A bare list has zero wrapper-placement rows; plural-list
placements count each actual child list. Registry-to-native closure and these
mode/cardinality outcomes are cross-table seals, not fabricated constant row
counts.

Text children have two canonical owner families. `software_list_notes` is an
optional singleton list child with empty text preserved. `software_title_text_elements`
has closed codes 0 description, 1 year, 2 publisher and 3 notes; description,
year and publisher are required once, notes is optional, and empty strings are
valid values. Each has one actual-parent mixed `source_order`; a list's notes
and catalog titles share the group sequence, while title text, info,
sharedfeat and parts share the title sequence. Part features, switches and
areas share the part sequence; switch values use the switch sequence; ROMs and
disks share the actual area sequence only with siblings of their same area
kind. Main's manifest-driven checks enforce mixed-table collisions and
same-edition ancestry.

No family ordinal, flattened media rank, duplicate parent/title ID, duplicate
list placement, copied attribute text, or source-name uniqueness is added.
List names and wrapper build, title clone target, nullable named-value fields,
area text and media text preserve absent versus present-empty as specified.
Canonical `software_lists` stores the list name/optional description and its
root location; a wrapper child placement stores only its one wrapper-local
order. Bare-root softwarelists have no wrapper or placement. A plural wrapper
may contain any number of lists allowed by the grammar; no unconditional
single-list or single-wrapper assumption is encoded.

## Enum and interpretation states

Closed SQL domains match DOC-12: supported is yes/partial/no (omitted yes),
width is 8/16/32/64 (omitted 8), endianness is little/big (omitted little),
status is good/baddump/nodump (omitted good), writeable is false/true (omitted
false), and switch default is false/true (omitted false). Each effective value
has its explicit-specified bit and a check requiring the documented omitted
default. Invalid supplied enum tokens are rejected by the importer contract;
there is no invalid enum row state.

Data-area size and optional ROM size/offset retain exact source text once.
Their checked numeric interpretation is unsigned ASCII decimal, multi-character
leading-zero octal, or `0x`/`0X` hexadecimal in `0..i64::MAX`. Malformed,
empty, signed, whitespace-padded or overflowing supplied text remains stored
with a NULL numeric interpretation; absent optional text remains NULL and has
no position. SQLite's built-in casts cannot implement this grammar exactly.
The candidate keeps the source lexeme and installs query-only projections:
`candidate_software_data_area_numbers(area_id,byte_length)` and
`candidate_software_rom_numbers(media_entry_id,byte_length,byte_offset)`.
Both use `numeric_sql.software_integer_sql`, which emits SQLite built-ins,
not an application UDF or a persisted numeric copy. It checks the complete
ASCII token, NUL exclusion and range before decimal conversion or bounded
octal/hex accumulation. Leading zeroes are removed before accumulation; a
long zero prefix cannot create an unbounded recursive walk. The projections
return NULL for absent or malformed text without changing the retained
lexeme. A numeric zero is valid at this layer; operation-specific rules may
require a positive length. These are not generic number rules for other
formats: flat DAT's strict XML `unsignedInt` and P/C's Rust `u64` contracts
have different lexical/sign/range rules.

ROM loadflag accepts exactly `reload`, `fill`, `continue`, `reload_plain`,
`ignore`, `load16_byte`, `load16_word`, `load16_word_swap`, `load32_byte`,
`load32_word`, `load32_word_swap`, `load32_dword`, `load64_word`, and
`load64_word_swap`, or NULL. The literal `load` is not a source token. Its
operation is derived into exactly six cases: load (omitted or one of nine
ordinary-layout flags), continue, reload, reload_plain, ignore, or fill. There is one step row
per ROM; ordinary loads have their own required-file row and self-link, controls
link to the active preceding declaration or remain orphaned, and fill has a
NULL link and clears the active declaration. The four query link states are
`DeclaresFile`, `UsesFile`, `OrphanControl`, and `NoFile`. Operation and link
state are not stored again. `software_disks` itself is the typed disk
requirement; disks have no ROM requirement or load-step rows. Same-area chain
transitions and load-step/required-file completeness need main's
publication/integrity checks.

The approved process decision is warning-and-processing for partial groups.
Retaining their list, title, text and file facts does not depend on whether a
loading recipe is executable; recipe validation and progress/read sizes remain
separate. The parser also accepts the plural wrapper as an application
compatibility dialect, allows an empty plural aggregate, requires at least one
software item in each parsed individual list, rejects duplicate list notes,
and routes extra accepted/retained source content through its existing
extension path. The candidate adds no unconditional list count or database
rule that would prevent warning-and-processing partial groups.

## Root diagnostics and evidence gaps

The physical-root diagnostic relations required by DOC-21 are
`software_document_import_messages(edition_id → software_documents.edition_id)`,
`software_list_root_import_messages` with `set_group_id → software_lists`,
plus `(set_group_id,edition_id) → catalog_set_groups(set_group_id,edition_id)`,
and
`software_wrapper_import_messages((wrapper_id,edition_id) →
software_wrapper_headers(wrapper_id,edition_id))`. The document owner captures
the whole document extent; the canonical list owner captures the actual
singular `<softwarelist>` root extent; the wrapper owner captures the actual
plural `<softwarelists>` root extent. The DDL uses the shared proposed names
`extent_view`, `extent_start`, `extent_end`, `location_view`, `start_line`,
`start_column`, `end_line`, `end_column`, and `column_convention`. It requires
one complete byte extent or complete text-coordinate extent, enforces paired
values, positive ordered coordinates and increasing byte offsets, and does
not claim source-length validation. `candidate_software_integrity_problems`
checks same-view containment for document→root, document→wrapper and
wrapper→nested-list edges; when byte and coordinate views both match, both
checks apply. It also audits singular/plural closure, individual-list title
presence, area subtype closure, title required text, ROM chain links, clone
position/relationship agreement, and canonical software media hash positions.
The candidate does not link diagnostic rows itself; DOC-21's exact root-link
relations remain main's diagnostics integration.

The current software parser exposes opening-element line/column only for its
roots and children; it does not expose the complete root/document byte extents,
location end coordinates or a location view. Therefore the new root columns
are candidate capture requirements, not values the existing importer can
currently populate. Ordinary child position/owner rows retain opening
coordinates only; those rows cannot prove containment. Main should link root
diagnostics only after the selected evidence system is actually captured and
validated. No extent is synthesized from the opening point.

## Integration dependencies

- `relationships.sql` defines the typed `reported_catalog_relationships`
  relation and closed `software_cloneof` kind used by these clone FKs. The
  software witness loads that fragment instead of stubbing the relationship
  registry. The clone target literal belongs only to `software_clone_links`;
  title attribute field 1 points to the matching relationship identity.
- At signed main `e591363`, common `catalog_sets` owns title opening
  `source_line/source_column`. Native `software_titles` has no duplicate.
- The assembler consumes the thirteen software manifest kinds and includes
  `candidate_software_integrity_problems` by its explicit
  `problem,owner_id,edition_id` interface. List root, wrapper root and document
  remain physical owners without registry rows. The new constructed fixture
  exercises actual subtype ancestry and mixed ordering in that assembled DDL;
  the title manifest row deliberately has no duplicate order.
- `software-field-presence.tsv` now supplies all 37 attribute/code routes to
  the shared actual `candidate_field_presence_problems` audit and assembled
  publication gate. This includes all seven explicit-default pairs and exact
  typed clone/hash facts, not just the earlier family-view subsets. Drafts may
  temporarily disagree; native values and positions need not be inserted in
  one statement. The parser-to-row count seal remains absent: typed owner
  closure and retained-field consistency do not prove that every parser-counted
  source element or optional source field was persisted.
- Registry completeness, table-kind agreement, typed same-edition ancestry
  and mixed-order collision checks are cross-table responsibilities for main's
  manifest-generated guards. These artifacts do not claim arbitrary raw-SQL
  commit closure or full design approval.

`software-hash-positions.tsv` inventories all three canonical software media
hash slots for manifest consumers: ROM crc, ROM sha1 and disk sha1. The bounded
SQL witness exercises singular and plural roots, nested-list extent containment,
a representative item/part/area/ROM chain, explicit/default states, raw
malformed numeric retention, closed enum rejection, clone and hash-position
agreement, and a populated ordered query plan. It uses minimal shared identity
stubs and loads the real `relationships.sql`; it is not a replacement for
main's cross-family assembled candidate checks.

## Field coverage and historical constructed witnesses — e591363

`software-field-coverage.tsv` has exactly 42 source-field rows, independently
transcribed from DOC-12 seq 38114: the 36 pinned DTD attributes plus compatible
wrapper build (37 attributes in 13 position families), and five element-text
scalars (list notes and title description/year/publisher/notes). This inventory
comes from the dictionary, not the candidate's table or code count. There are
seven default-presence pairs: supported, dipvalue default, dataarea width and
endianness, ROM status, disk status and disk writeable. The required title
description/year/publisher children must exist once but may have empty text;
notes children remain optional and can also be empty.

The complete sections read for this cut were the pinned grammar boundary;
Lists, items and parts (including the exact root/title/part crosswalk); Areas
and source entries (including reconciliation and the exact area/media
crosswalk); Complete software field-state rules; Native attribute provenance
contract (all its subsections); Checked unsigned numeric interpretation; ROM
file-chain relations and derived states; and All DTD loadflag values. All reads
returned seq 38114. Evidence labels in the TSV abbreviate these named sections.
The current-to-candidate column dispositions are in each row's `current=`
evidence, with its actual parser entity. sem traced current
`mame_softwarelist::{software_document_header,parse_list,parse_item,parse_part,
parse_dipswitch,parse_area,parse_rom,parse_disk,named_value}`, native writers,
`catalog_software::queries`, `catalog_files::software::rom_select`, and
`snapshot_history::software::{load_document,load_requirements}`. The production
`SCHEMA` composition in `src/storage/db/ddl.rs` still includes its existing
schema/guard/position fragments; this cut changes none of them.

Ledger conventions:

- `owner_table` is the actual candidate typed owner of the source construct,
  even when the sole value belongs to a common set or clone relation.
- `value_owner` is one existing table/view and `value_column` is an exact column
  name or comma-separated actual columns. Defaulted attributes list both the
  effective-value and specified columns. For digest scalars the existing
  `declared_catalog_hash_text.declared_text` view supplies the lossless source
  spelling from canonical `catalog_entry_hashes` state/override/hash link and
  `hash_values.bytes`; it adds no persisted copy. Their positions identify
  the same canonical declaration, never a copied media digest.
- Attribute codes are the actual integer `field_kind` codes. Text scalars use
  `position_table='-'`: list notes uses `field_code='text'`, and title scalars
  use description/year/publisher/notes tags. Their actual child discriminators
  0/1/2/3 are stated in `default_rule`; opening location/order is on the child
  itself, not another attribute companion.
- Presence/default text states the required policy. The new shared audit now
  enforces `position-iff-present` for retained attributes at integrity checks
  and candidate publication, not as a same-statement draft insertion rule.
  NULL, empty and invalid raw numeric text remain distinct from checked numeric
  interpretation; no inferred execution qualification is in the ledger.

`software_field_witnesses.sql` is independent of the older minimal family
fixture. Its stated prerequisite is a fresh in-memory database with the full
assembled candidate DDL. It constructs valid typed owners and real parents,
inserts every accepted attribute code, and exercises required-empty text,
optional omission versus empty, all seven omitted versus explicit-default
pairs, an empty clone literal with stable relationship identity, malformed
numeric retention, and all four declaration states for each canonical hash
slot (absent/empty/invalid/value). It asserts required-child absence is audited
and ends with clean assembled-integrity and FK checks. Hash scopes intentionally
grant no whole-file/UUID eligibility; disk valid SHA-1 remains CHD-header scope.
Its TEMP assertion table is historical test scaffolding only, never a runtime
catalog relation or the implementation of the new shared presence audit.

`software_field_check.py` has a separately written expected 42-field mapping.
It compares exact wire names, typed/value/position owners, columns and codes;
removing any row, substituting its wire name without changing row count, or
substituting its value mapping must fail (126 in-memory mutations). It also
checks actual DDL tables/views/columns, typed position FKs, all 13 closed code
domains and constructed rows for every code. Valid-row mutations isolate
required NULL rejection, illegal/fractional position codes, closed enums, and
each of the seven omitted-nondefault contradictions. All fourteen accepted
loadflag tokens are tested only for scalar storage. Schema SQL is emitted once;
one class database/fixture is prepared, and each test uses a rollback savepoint.
No executescript runs inside those per-test savepoints.

Historical focused commands and observed results (before the shared presence
integration; not a claim about current publication coverage):

```sh
python3 docs/schema-candidate/software_field_check.py
# 7 tests passed in 4.139 seconds, including one-time assembly/preparation.
{ python3 docs/schema-candidate/assemble.py --emit && cat docs/schema-candidate/software_field_witnesses.sql; } | sqlite3 -bail :memory:
# software field witnesses passed
sqlite3 -bail :memory: < docs/schema-candidate/software_witnesses.sql
# software witnesses passed; populated ROM area lookup uses its covering index.
```

Proof boundaries remain concrete. Exact field inventory and constructed state
execution do not prove complete parser-fed field/element counts, source byte
coordinates, EOF or publication closure for arbitrary SQL. In particular, an
optional value and its position erased together look like legitimate omission;
these checks cannot detect that without an independent parser-fed count seal.
The shared audit closes those retained attribute/position equivalences; it does
not supply the missing independent source counts. Required description/year/
publisher child existence is already audited and per-kind singleton constraints
prevent duplicates; empty text is allowed. Optional list/title notes are also
singletons. These five PCDATA scalars have no attribute-position routes, and
parser-fed PCDATA event/count completeness is not proved by this manifest.
SQL INTEGER affinity on width cannot itself prove exact wire-token recognition
(for example, a bound text `08` can become integer 8); yes/no wire booleans also
need the pinned parser boundary. The witness's constructed coordinates are not
new parser capture evidence.

The query-only numeric and first-run views described below supersede the
earlier missing-projection/first-run gap. The isolated native-file candidate
now also defines byte-contract-backed file/hash qualification and shared
hash/size publication maintenance; see the README's dated section and focused
checks. Eight composed qualification methods pass and scoped Sol re-review is
CLEAR. Review publication requires published source owners; candidate evidence
views retain draft facts for intended completed-batch matching, while private
Rust batch isolation/freezing remains unproved. Maximum-run progress,
actual cursor behavior, partial-group warning/processing and complete recipe
preflight still require their pinned interpretation proof. Testing all
loadflag strings does not test those derived facts. Source-declaration/step
checks do not turn a retained malformed or unnamed declaration into an
executable recipe. No production build/gate/import/profile was run for these
design checks. Full design approval remains open.

### Query-only first verification run — candidate

`candidate_software_file_lengths(media_entry_id,byte_length)` derives the first
ordinary load's verification length from native ROM/load-step/required-file
owners. It adds consecutive `continue` and `ignore` lengths and stops at the
first reload, reload_plain, fill or next ordinary load. It does not use the
largest reload run, data-area size, offset, destination extent or progress
estimate as the file length. Base and continue lengths must be positive;
ignore may be zero. An invalid consumed length, signed-range overflow or
broken owner/step chain yields NULL, never the accumulated valid prefix.

The view adds no stored length or extra source declaration. Numeric projection,
load-chain structure, byte-coverage qualification and accepted shared evidence
are separate checks. A result over draft rows is provisional: absence of a
later row is not a parser EOF receipt. The supported finalizer must first
establish complete source-count/native ownership closure for the selected
edition. The view alone does not qualify a malformed/unnamed/nodump file or
promote control-operation hashes. The separate native-file candidate now
defines qualification and atomic shared hash/size maintenance, but neither is
wired to a Rust batch writer; implementation follows design approval. Full
model guards, producer capture, accepted EOF, corpus and production gates
remain open.

Focused constructed controls are in `numeric_sql_check.py`,
`software_numbers_check.py` and `software_file_lengths_check.py`. They exercise
literal numeric boundaries, retained native lexemes, operation/owner corruption,
and point-query plans. They are not authentic parser, corpus, EOF or runtime
performance acceptance.

## Retained-field presence closure — current isolated candidate

`software-field-presence.tsv` has one build-time route per distinct physical
position table/code: 37 routes across 13 families. The independent inventory
in `software_presence_check.py` spells out the exact wire names and codes,
rather than treating a matching row count as completeness. Predicates use the
actual position FK's native owner: 16 required constants, ten nullable scalar
checks, seven specified-bit checks, one typed clone-facet existence check and
three canonical media-hash declaration existence checks. Required title name
belongs only to `catalog_sets`; the required constant does not copy that value
or replace its FK/NOT NULL enforcement. Empty clone targets and empty/invalid
hash declarations remain present independently of usable targets/digest bytes.
Hash identity and typed relationship agreement remain separate shared audits.

The shared generator validates complete owner primary keys and exact
single-column typed position FK groups, then compares the Boolean predicate
with the exact owner/code position count and rejects nonzero occurrence. These
routes are design inputs, not EAV data, and add no stored value or ancestry.
The generated views are included in actual `candidate_integrity_problems` and
the actual candidate publication trigger. Publication freezes the existing
native values and positions; draft mismatches remain observable and repairable.
Production parser/writer/finalizer adoption and full design approval remain
unimplemented/unapproved; passing this SQL candidate is not production clearance.

For this closure, DOC-12 seq 38114's complete software field-state rules,
Presence/location/query/history, and both exact root/title/part and area/media
crosswalks were reread completely. sem traced the shared generator, scope
resolver and assembly/publication wiring, including the native area-facet FK
path. Fixture INSERT statements require direct SQL reading because sem's SQL
entity index does not expose that DML as entities.

The independent suite prepares the actual composed candidate and existing thin
family field fixture once per class, then uses test/case rollback savepoints;
no executescript runs inside them. It covers all 37 missing-position codes,
ten absent/empty scalar transitions, seven explicit/omitted defaults, all three
hash slots in empty/invalid/value states, stable typed clone identity, equal-row-
count owner substitution, repair→actual publication→immutability, and detached
registry/group/area-parent cases. Deliberate corruption temporarily drops the
affected table's guards only in rollback savepoints with SQLite FK enforcement
off; ordinary guard rejection is checked first. Registry/group-detached rows
remain visible with NULL edition. For an area facet, its actual parent FK is
the issued source ID: surviving registry identity still truthfully attributes
edition 1 when only the native parent payload is lost, and publication is
blocked. Missing registry, alone or with its parent payload, gives NULL in the
field audit. This is not proof that an edition-filtered publication catches
arbitrary deliberately detached unknown-edition corruption.

Both actual software area-facet audit branches also have populated edition-
filtered plan regressions, extracted from the installed shared view definitions
rather than a handwritten approximation. They require indexed native owner
and registry lookups on the actual parent FK, no owner/registry scan, and test
parent-only, registry-only and combined detachment. The initial chained
parent→registry LEFT JOIN was RED with `SCAN owner` for both facets; the shared
resolver now joins the registry directly using that issued source identity,
without copying ancestry or hiding missing native-parent payload audits.

Before installation, deleting part 200's required interface position left the
actual integrity audit empty and publication incorrectly succeeded (RED).
The new suite requires that same isolated mismatch to be the sole integrity
problem, blocks actual publication, then repairs and publishes successfully.
Earlier focused runs against actual composed DDL after the scope correction
(before the CRC occurrence review follow-up):

```sh
python3 -Werror::ResourceWarning docs/schema-candidate/software_presence_check.py
# 11 tests passed in 18.809 seconds; both actual facet plans and all six
# parent-only/registry-only/combined facet detachment cases passed.
python3 -Werror::ResourceWarning docs/schema-candidate/software_field_check.py
# 7 tests passed in 3.413 seconds.
```

The earlier presence run was RED only for the two chained-join facet plans
(11 tests, two failing subtests); its behavioral checks were green. Both recorded
commands exited zero without connection ResourceWarnings. The final exact-FK-
group/complete-PK inventory assertion was additionally rerun independently.

### NativeSol P2: CRC declaration occurrence isolation

`test_crc_nonzero_occurrence_only_does_not_invent_canonical_presence` removes
ROM 400's CRC position and occurrence-0 declaration 600, then successfully
inserts fresh declaration 609 for the same media/CRC at occurrence 1 only.
This uses the actual composed schema and normal DML: no dropped guards,
constraint weakening or expected insert rejection. The test verifies the
exact retained declaration, absence of a CRC position and a clean FK check.
The actual native field-presence view must remain empty: occurrence 1 cannot
invent presence of the absent canonical occurrence-0 attribute.

This is a staged audit control, not a publishable software document. The
assembled integrity view separately reports exactly `hash_position_count`
and `software_hash_without_matching_position` for declaration 609. Those
expected independent hash-position findings are checked, but are not the
oracle for CRC predicate correctness or a substitute for the native-presence
assertion.

The targeted real test passed (1 test, 3.105 seconds). Mutation verification
used `python3 -Werror::ResourceWarning` and an in-memory `unittest.mock` patch
of `assemble.field_presence_routes`, removing only
` AND hash.field_occurrence=0` from the ROM CRC code-2 predicate before actual
assembly. Running the same new test was RED (1 assertion failure, no errors,
3.346 seconds): the native view falsely returned exactly
`('field_presence:software_rom_attribute_positions:2',400,1)`. The verification
driver required that exact failure. No manifest or persistent DDL was changed.

Final bounded follow-up command:

```sh
python3 -Werror::ResourceWarning docs/schema-candidate/software_presence_check.py
# 12 tests passed in 22.563 seconds, with no connection ResourceWarnings.
```

Only `software_presence_check.py` and these notes changed in this follow-up;
both are frozen again for review. The source-count, PCDATA, XSI, checked-load
and production-publication proof boundaries below remain unchanged.

No software XSI field is invented: namespace/schema-location storage elsewhere
does not prove XML namespace or schema semantics, and this family suite makes
no such claim. Source event/count seals, genuine byte/coordinate capture and
EOF evidence remain separate, as do the numeric/checked-chain/complete-first-
run/qualification gaps above. Erasing an optional value and its position
together cannot be detected without independent parser-fed evidence.

## Native child cardinality candidate — 2026-10-08

`software_cardinality.sql` owns the exact `problem,owner_id,edition_id` view
`candidate_software_cardinality_problems`. Its three bounded audits cover a
list with no typed title, missing required title description/year/publisher
PCDATA children (empty text is present and valid), and data/disk area subtype
closure. Required title text is exactly one of each: the view checks the
minimum and exact count, while the native `UNIQUE(set_id,field_kind)` key
enforces the maximum in ordinary writes. The same native unique keys enforce
optional list/title notes as zero-or-one. Area subtype diagnostics preserve
every area with outer joins. They use the parent source element's edition
first, fall back to the area's own source-element edition when the parent
registry is missing, and retain NULL when both scopes are unknown; native
ownership/ancestry audits continue to report the detached rows independently.

The existing `candidate_software_integrity_problems` no longer duplicates
these three rules. Envelope closure remains there: a bare root has exactly one
canonical list; the compatible plural wrapper requires its physical wrapper
and placements for present lists, while zero nested lists is accepted. The
importer requires each present list to contain at least one software item and
requires each item's description/year/publisher, while accepting empty PCDATA.
It accepts parts, areas, switches, and recognized children with zero children;
specifically, `parse_dipswitch` accepts no `dipvalue` even though the pinned
upstream DTD spells that child with `+`. Unknown children in supported scopes
are retained as extensions and are not treated as typed native children. The
candidate follows these importer-compatible outcomes rather than imposing
stricter DTD validation.

`software_cardinality_check.py` composes the current assembler output once
and checks its exact view interface without re-creating the view. Rollback
savepoints cover erasing each required PCDATA kind, rejecting an extra
singleton, an empty list, an area/subtype mismatch, and repair. Positive
controls include empty required text, optional notes present-empty/absent,
empty dipswitch and area children, a bare single-list document, and a plural
wrapper with zero nested lists. The suite checks parent-derived edition
attribution, a detached parent with own-registry fallback, fully unknown NULL
scope, indexed known-parent/facet query plans, and the real assembled
publication rejection/repair path. It remains constructed-SQL evidence: it
does not establish parser/corpus proof, source event counts, or independent
count seals. No count seals are added in this child/PCDATA slice.

## Parser event count inventory — candidate

`software-counts.tsv` contains 13 native source-owner counts and one count for
each of the 13 physical attribute-position tables. It counts the canonical
list and the other native source owners in the owner manifest, excluding the
wrapper-list placement as a duplicate representation of that list. The fixed
document root is not counted: envelope mode fixes its cardinality, and in
plural mode `software_documents` and `software_wrapper_headers` describe the
same physical root. A bare `<softwarelist>` and each wrapped `<softwarelist>`
increment the independent list event once; `software_list_wrapper_entries`
records placement but is not a second count of that element. The list count
remains independent evidence if both the canonical list owner and its wrapper
placement are erased.

The proposed increment labels identify the reader root/list callbacks and the
typed elements represented by each parsed item and its nested parts, switches,
areas, media declarations, and text children. Recognized token-only attributes
such as `supported`, `default`, `width`, and `endianness` contribute their one
physical position row; their specified/value flags do not add owners. Required
title PCDATA children and optional title/list notes count as source-element
owners, while values sharing an owner row remain scalar fields. Load steps,
required-file projections, clone links, and other derived or relation rows are
excluded.

These are proposed build-time parser events, not implemented producers or
validated seals. `scope_sql` resolves each listed native row's edition through
the envelope, `catalog_set_groups`, or `catalog_source_elements`; it does not
derive expected totals by selecting the candidate rows. This inventory does
not establish that the callbacks feed counters, that all owner/position events
are captured, or that totals are sealed only after accepted input completion.
Physical root/element span capture and parser-to-row completeness therefore
remain open evidence gaps.

`software_count_mapping_check.py` exercises the candidate's constructed
count-to-row comparison with an independent literal vector keyed by counter.
All 26 routes have positive rows in the constructed software fixture; deleting
each target-edition route inside a savepoint yields only its named counter
mismatch, and changing only that seal to zero makes the source-count audit
quiet. The values are fixture mapping receipts, not parser event totals,
parser-fed seals, or EOF/source-capture proof.
