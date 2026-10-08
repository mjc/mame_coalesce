# MAME candidate notes

This is the MAME machine XML slice of the design candidate. Its source policy is
MAMEC-DOC-10 at seq 38124, MAMEC-DOC-21 at seq 38132, and the relationship
position contract in MAMEC-DOC-23 at seq 38140. These pages remain
design drafts; the full design approval gate is open.

## Ownership and ordering

`mame_documents` is the physical document/root owner, keyed by the edition.
`catalog_sets` owns the root machine's common source-element identity, name,
group, and root placement; `mame_machines.set_id` reuses that identity. It has
no copied machine source order. `mame_machine_text_elements` owns description,
year, and manufacturer as actual text children. Description is required by the
pinned grammar; year and manufacturer are optional singleton children.

All direct machine child elements have a source-element ID, actual
`machine_id`, one `source_order`, and opening-tag location. Their one order
domain includes BIOS sets, text children, ROMs, disks, samples, device refs,
switches, and every hardware family. Nested children use the actual immediate
parent FK and a new parent-local order. No nested child copies machine ancestry.
The TSV manifest lists all 30 registry-bearing native owner families. It omits
the document root, the sparse compatibility owners, machine links, and media
merge rows: those are physical-root or attribute/relation owners, not ordinary
source-element placements. Media source identity is shared with
`catalog_media_entries.media_entry_id`.
Switch value kinds (`dipvalue` versus `confsetting`) derive from the parent
switch's closed `kind`; a mismatch is rejected by the selected grammar.

`mame_device_instances` deliberately has no unconditional unique constraint on
`device_id`. Under strict DTD rules there may be at most one instance; the
selected observed-v3 compatibility rules retain every accepted instance.
`candidate_mame_integrity_problems` reports strict-dialect duplicate instances
for publication rejection; it does not classify v2 or unknown dialects as
strict. The strict reading identity must use `dialect='strict-dtd'` in the
shared reading-rules row. The view's rule is candidate policy, not parser or
publication implementation evidence.
Both instances and extensions have real source identity and a single shared
device-child order. The selected v3 profile and old v2 behavior remain distinct
reading-rule identities; this is a candidate policy, not an implemented parser
change.

## Values, presence, and hashes

CDATA and numeric-looking hardware declarations remain source text. In
particular `mameconfig`, adjuster `default`, RAM option `default`, RAM PCDATA,
switch/condition masks and values, timings, counters, and MAME ROM `offset`
remain TEXT. ROM `size_text` is NULL only when absent; empty and malformed
supplied text are retained. A checked numeric byte length is a derived
projection only for nonempty ASCII decimal values through signed 64-bit max.
MAME offset interpretation is hexadecimal u64 with the parser's accepted
optional prefix and leading-plus behavior; it is not a stored numeric column.

Effective defaults and explicit presence are retained for the DTD's defaulted
fields. Unspecified defaults do not receive position rows. Compatibility
`isconsumable` exists only in its sparse owner when supplied. `writeable` is a
separate compatibility disk fact from native `writable`, including when the
two declarations disagree. The observed compatibility fields are not claimed
as DTD attributes.

`catalog_entry_hashes` is the sole MAME digest declaration/value owner:

| Position family | Declared hash field | Scope candidate |
|---|---|---|
| ROM native | `crc`, `sha1` | `whole_file` only under the pinned native-ROM contract; `unknown` for nodump or unproven loading semantics |
| ROM compatibility | `md5` | Compatibility-rule-qualified scope; never sole cross-list UUID evidence |
| Disk native | `sha1` | `chd_header_sha1`, or `unknown` for nodump; never whole-container bytes |

Hash field positions have a unique non-NULL `reported_hash_id`; every other
position has NULL there. Hash presence/state, invalid-declaration subtype,
canonical bytes, and lexical override follow DOC-21's shared contract. No raw
digest text is repeated in a MAME owner. Native and compatibility positions
remain separate named tables but share the same per-element lexical ordinal.
`mame-hash-positions.tsv` is the exact integration map from the four digest
position families to `catalog_entry_hashes`; the assembled candidate enforces
one-to-one position closure using these owner/code/occurrence columns. The
field-presence manifest additionally requires a position exactly when the
canonical source-field declaration exists, including empty/invalid states.

## Relationship positions: exact integration cases

The four position tables below now carry `relationship_id INTEGER UNIQUE
REFERENCES reported_catalog_relationships(relationship_id)`. Their CHECKs
require a non-NULL ID exactly for the listed relationship codes and NULL for
every other field code. All listed fields have `field_occurrence=0`. MAME's
`field_kind` is the literal TEXT code shown here; current Rust enum ordinals
are not candidate SQL field codes.

| Typed literal declaration and key | Canonical position table and owner column | `field_kind` | Reported kind |
|---|---|---|---|
| `mame_machine_links(machine_id,link_kind='cloneof')` | `mame_machines_attribute_positions.set_id` | `cloneof` | `mame_cloneof` |
| `mame_machine_links(machine_id,link_kind='romof')` | `mame_machines_attribute_positions.set_id` | `romof` | `mame_romof` |
| `mame_machine_links(machine_id,link_kind='sampleof')` | `mame_machines_attribute_positions.set_id` | `sampleof` | `mame_sampleof` |
| `mame_device_references(source_element_id)` | `mame_device_references_attribute_positions.source_element_id` | `name` | `mame_device_ref` |
| `mame_rom_merges(media_entry_id)` | `mame_rom_claims_attribute_positions.media_entry_id` | `merge` | `mame_rom_merge` |
| `mame_disk_merges(media_entry_id)` | `mame_disk_claims_attribute_positions.media_entry_id` | `merge` | `mame_disk_merge` |

The four literal-owner tables retain their sole `target_name`, `name`, or
`merge_name` fact and existing unique relationship FK. Position rows add no
literal copy, and link/merge declarations add no attribute coordinates or
order. Device-reference placement coordinates/order remain on the actual
child; its `name` attribute coordinates/order remain on its position row.
The assembled candidate enforces matching relationship ID, owner key, TEXT
field code, occurrence, reported kind and edition in both directions, including
uniqueness across the four position families. The new field-presence routes
also enforce the corresponding typed source-literal/position obligation.
Local UNIQUE/FK/CHECK constraints alone do not prove this cross-table closure.

## Field and position ledgers

The position DDL provides the exact closed field-code ledger for each of the
33 native/compatibility position families. Every position has
`(owner,field_kind,field_occurrence)`, with singleton occurrence zero,
`source_order`, and positive QName coordinates. It stores no values, family
ranks, parent ancestry, or duplicate source order. Native XML attributes use
their literal names (including `sourcefile`, `flipx`, `pixclock`, `htotal`,
`hbend`, `hbstart`, `vtotal`, `vbend`, `vbstart`, `briefname`, `devname`,
`keydelta`, `nosoundhardware`, and `fixed_image`).

`mame-field-coverage.tsv` records all 125 pinned DTD attribute declarations
plus ten accepted compatibility attributes, with exact current SQL column to
candidate canonical-owner mappings. It contains 155 contextual rows: 151
attribute rows (the 135 declarations plus 16 additional placements of the
four condition attributes across five actual-parent contexts), and four
unpositioned PCDATA scalars. Description, year, manufacturer and RAM PCDATA
are explicitly text, not attributes. Every row cites DOC-10 seq 38124 and its
parser/macro evidence; native attributes additionally cite the pinned DTD.
Hash rows identify the canonical shared state owner and explain reconstruction
through `hash_values`/`declared_catalog_hash_text`, without copying digest text.

The source macro dictionary has 126 slots: 116 native and ten compatibility.
Its three shared switch/location/value enums expand to both source element
kinds, adding nine element-qualified declarations. The candidate DDL has 134
physical table/code pairs (124 native plus ten compatibility), because the
four shared condition codes occur in three typed position families. These
different inventories are checked independently, not equated. All six
switch/location/value element kinds and all five condition-parent contexts
have separately constructed owners, field assertions and actual-parent
controls. The 22 distinct stored default-presence fields are tested in all
24 wire contexts, including separate DIP/configuration defaults. Adjuster
default and RAM default remain CDATA rather than default-presence booleans;
RAM PCDATA remains a separate scalar. Native `writable` and compatibility
`writeable` have separate positions and a positive disagreement witness.

The evidence review used sem MCP on `read_with`, `parse_machine`,
`parse_machine_children`, `parse_machine_switch`, `parse_machine_bios_set`,
`parse_device_reference`, `parse_asset`, `parse_mame_asset_attributes`,
`known_asset_attribute`, `known_child_attribute`, `parse_condition`,
`specification::parse_element` and attribute `select`. Macro invocations and
SQL declarations were read as data because sem does not index their fields.
DOC-10's complete selected sections were the exact hardware/media crosswalk,
field-state completion, compatibility ownership, document/machine/source,
ROM/disk/sample, specification families, switches/values/conditions, closed
enums, position-only keys, truth of presence, and lexical-order policy.

## Root diagnostic extent contract

`mame_documents` reserves the DOC-21 physical-root diagnostic columns:
`extent_view`, `extent_start`, `extent_end`, `location_view`, `start_line`,
`start_column`, `end_line`, `end_column`, and `column_convention`. Byte extents
and decoded-text coordinates use distinct views. The candidate row CHECK
requires at least one complete nonempty extent or full coordinate range, so
the opening `source_line/source_column` pair alone cannot serve as root-span
proof. DOC-21 integration must additionally verify ranges against the named
source view, source-view length and containment. Existing code exposes the
opening root line/column only; current parser capture therefore remains an
explicit gap, and no existing opening coordinate may be backfilled as the end
of the root.

Ordinary native children currently expose opening locations only. This schema
does not invent end positions or extents for them, so containment cannot be
claimed for those children until the parser captures complete spans.

## Bounded witness behavior

`mame_witnesses.sql` builds a small in-memory common-key fixture and executes
the candidate DDL. Its assertions cover full-u64 ROM size/offset lexical
retention, a three-table device child order, a matching CRC declaration and
position, sparse compatibility absence, rejection of a made-up field code,
duplicate singleton-position rejection, integer QName/order storage classes,
and clean foreign-key closure. It inserts a second device instance and shows
that the integrity view is quiet under observed-v3, reports the owner after
the fixture's reading identity is switched to strict DTD, then becomes quiet
again when v3 is restored. A root with opening coordinates but no complete
span is rejected. The fixture prints an index-backed ROM parent/order plan.
The relationship fixture supplies all six literal declarations and matching
positions. Starting from these valid rows, negative updates test required IDs
for all six codes, forbidden IDs on nonrelationship fields in all four
position families, and duplicate machine-position relationship IDs. A
savepoint defers an isolated missing-reported-relationship FK probe, checks the
violation explicitly, and rolls it back before the final clean FK assertion.
This is a bounded construction witness, not exhaustive field dictionary,
cross-table ownership, or real-source extent proof.

The inventory counts different identities: 125 DTD `(element, attribute)`
declarations plus ten compatibility declarations give 135. The source Rust
macros have 126 enum/code slots because switch, location and value enums each
serve two XML kinds (nine additional DTD declarations). The candidate has 134
table/code pairs: the four condition codes occur in three typed position
tables, adding eight pairs to the macro count. Conditions also occur under five
actual wire parents (DIP switch, configuration, DIP value, configuration setting
and adjuster), so four DTD fields expand to twenty contextual declarations:
135 + 16 = 151 attributes, plus four PCDATA scalars = 155 ledger rows. The 126
count is not a count of physical candidate position pairs.

`mame_field_witnesses.sql` adds independent constructed execution for every
contextual ledger row and every closed position code. Required NULL and enum
empty-value controls, optional absent/present-empty controls, all 24 explicit
default/absent-default contexts, sparse compatibility/relationship states,
and absent/empty/invalid/value hash states execute against typed owners.
Savepoints restore the baseline after each state probe; rolled-back assertion
rows are observed through SQLite tracing by the Python checker. A populated
display parent/order query uses `mame_displays_parent_order`.

The first integration run failed at the `unspecified-nondefault:mame:debug`
negative assertion near original line 948: `UPDATE OR IGNORE` accepted
`debug=1,debug_specified=0`. This exposed a missing local default implication,
not a weakened test. `mame_documents` now has
`CHECK(debug_specified=1 OR debug=0)`, and the unchanged negative passes.
No public columns or relationship interfaces were renamed.

The fixture's TEMP `mame_fixture_presence_problems` is explicitly a test-only
detecting layer, not an installed candidate publication view. It checks
retained typed value/specified-bit/sparse-declaration presence against exact
owner/code positions. Constructed attacks show deletion of a position,
absent value with a present position, an invented default position, and an
equal-position-count move between two owners. Joint deletion of an optional
value and its position deliberately produces no problem: detecting that loss
requires an independent parser/source count seal or equivalent source evidence.
This ledger is a dictionary inventory, not such a runtime seal. Required
child existence, full parser fidelity and publication closure are not proven
by the TEMP audit. That historical fixture remains test-only: the installed
design-candidate closure described below comes from the shared generator, not
from promoting this TEMP view or substituting its thin common tables.

`mame_field_check.py` independently reads the pinned DTD and current Rust macro
dictionary, verifies exact contextual identities (including equal-count
mutation controls), checks current and candidate table/column references,
compares all actual closed DDL codes with the ledger and populated fixture,
and verifies executed field/default/hash probes. Its six tests inspect only
isolated SQLite declarations and fixture data; they do not run a production
parser, import or build.

Focused commands, all passing in the repository devenv:

```sh
devenv shell -- python3 docs/schema-candidate/mame_field_check.py
devenv shell -- bash -c 'sqlite3 -bail :memory: < docs/schema-candidate/mame_field_witnesses.sql'
devenv shell -- bash -c 'sqlite3 -bail :memory: < docs/schema-candidate/mame_witnesses.sql'
```

## Installed candidate field-presence closure

`mame-field-presence.tsv` is a build-time manifest, not a runtime catalog table.
Its 134 unique physical `(position_table,field_code)` routes cover all 33
native/compatibility position families. They are not 151 wire-context routes:
the shared switch, location and value tables use one actual base owner per
code, with the same presence rule under either switch kind. Conditions retain
three separate typed owners/position tables. No CASE is needed because these
shared contexts have identical presence policies; no duplicate route is added.
The exact six-column header names the real position FK target/key and a closed
Boolean expression on `owner`, never `NEW` or a copied ancestor/value.

Nullable literals use `IS NOT NULL` (including empty/malformed text); required
NOT NULL literals also use that Boolean test. Defaults use only the explicit
`*_specified=1` bit. Machine name is checked through its common set owner;
machine links and media merges use exact typed declaration rows and link kinds.
All compatibility routes start at the actual machine/ROM/disk owner, not its
optional payload facet. Sparse booleans use facet-row existence; each of the
seven ROM payload fields uses its own nullable facet column. MD5 instead uses
the sole `catalog_entry_hashes` declaration and needs no ROM compatibility
payload row. All four hash routes match media ID, exact source hash spelling
and occurrence zero, independently of valid/invalid/empty state. No relationship
ID semantics or public SQL names changed.

Main's `field_presence_routes`/`field_presence_sql` install
`candidate_field_presence_problems(problem,owner_id,edition_id)` in the actual
composed schema and include it in `candidate_integrity_problems` and publication
closure. Problems are `field_presence:POSITION_TABLE:FIELD_CODE`. Draft values
and positions may temporarily disagree; they need not be inserted atomically.
Publication requires agreement, and the existing published-fact guards freeze
native values, facets and positions afterward. General position-presence checks
therefore are installed in the design candidate, not merely requested work.
This is not production parser/schema/publication implementation approval.

`mame_presence_check.py` assembles the real candidate once per class, reuses
only concrete INSERTs from the earlier field fixture, and never loads its stub
schema or TEMP presence audit. Its independent inventory test compares the
134 identities with pinned-DTD/coverage identities, actual closed SQL CHECKs,
and actual position FKs. Savepoint probes remove every physical code (including
every populated shared-kind owner), require the exact installed/aggregate
finding and reject publication; test every nullable native field, all 22
default bits in 24 contexts, all compatibility facets and MD5 without payload,
empty/invalid hashes, exact relationships, empty required CDATA, equal-count
owner substitution, and baseline publication followed by mutation rejection.
Constructed byte ranges are synthetic source-view fixtures, not authentic
parser span capture or corpus evidence.

Normal detached-registry deletion is rejected by generated FK guards. The
explicit reverse-guard-bypass probe preserves the native presence finding even
without registry scope. Its publication rejection is required by the test;
the initial shared gate accepted `(owner_id=205,edition_id=NULL)` because it
filtered only the proposed edition. This is a shared-generator/publication
integration finding, not a missing MAME route. The initial composed run also
reported an unclosed connection from `assemble()`'s connection context manager;
main fixed that cleanup, and the subsequent ResourceWarning-as-error run is
warning-free. New test connections have explicit cleanup.

Initial checkpoint: 11 tests executed in 12.043s, ten GREEN and one RED for
detached-registry publication (`IntegrityError not raised`). Main corrected
the ancestry audit to retain the actual parent's edition when the child's
registry scope is missing. The unchanged suite then passed all 11 tests in
12.208s, warning-free. Every normal-scope physical-route deletion is found by
the installed and aggregate views and rejects publication. This is bounded
constructed publication proof, not complete-model or parser acceptance.

### NativeSol P2 CRC predicate controls

The original eleven tests did not distinguish CRC presence from a retained
SHA1 declaration or from an occurrence-one-only CRC declaration. Two new
composed-schema tests remove the CRC position and occurrence-zero declaration
while retaining SHA1 and its exact canonical position. The first requires the
CRC-specific presence result to stay empty, verifies clean aggregate/FK state,
and successfully publishes that valid CRC-absent draft. The second adds only
`catalog_entry_hashes(...,source_hash_field='crc',field_occurrence=1,...)` and
again requires the occurrence-zero CRC presence result to stay empty.

The occurrence-one row is synthetic out-of-MAME-contract source data, not an
accepted MAME attribute or position. The actual shared hash table permits
nonnegative occurrences for multiple formats; this insertion requires no
CHECK, trigger, FK or UNIQUE bypass. Its independent `hash_position_count`
problem still rejects publication. The native presence assertion executes
before that separate closure check, so a publication rejection cannot mask a
bad CRC predicate. All probes roll back to the original baseline.

The checker adds a test-only `--crc-presence-mutant` option that patches only
the CRC route's in-memory generator input. It does not change any manifest,
DDL, shared file, setup rows or storage guards. Three individually executed
mutants are RED at the new native-presence assertions: selecting SHA1 instead
of CRC; removing the source-field filter; and removing the occurrence-zero
filter. Each produces the false CRC finding `[(201,1)]` instead of `[]`, with
one assertion failure and zero setup/storage errors. The actual unmodified
manifest passes all 13 tests in 17.164s, warning-free; that concurrent test-run
duration is not a performance benchmark. Sem traced the real generator/hash
guard interfaces; shared SQL CHECKs were directly inspected as data.

Focused command (active matching devenv, shell `login:false`):

```sh
python3 -Werror::ResourceWarning docs/schema-candidate/mame_presence_check.py
```

Reproduce the three expected RED runs independently (each exits 1):

```sh
python3 -Werror::ResourceWarning docs/schema-candidate/mame_presence_check.py --crc-presence-mutant sha1 ComposedPresence.test_crc_declaration_removal_does_not_select_sha1
python3 -Werror::ResourceWarning docs/schema-candidate/mame_presence_check.py --crc-presence-mutant any-hash ComposedPresence.test_crc_declaration_removal_does_not_select_sha1
python3 -Werror::ResourceWarning docs/schema-candidate/mame_presence_check.py --crc-presence-mutant any-occurrence ComposedPresence.test_crc_nonzero_declaration_does_not_supply_occurrence_zero
```

## Remaining design gaps

- The 134 installed routes close retained source-field/position agreement;
  they do not provide real-source/parser count seals. Joint deletion of an
  optional value and its position is still undetectable without independent
  source evidence, and has an explicit quiet-boundary witness.
- Required description child existence/cardinality and total PCDATA-child
  counts are outside this attribute-position manifest. Text remains solely
  owned by its typed child/RAM owner; no attribute position is fabricated.
  XSI vocabulary/meaning, source-count capture and parser reading semantics
  are not established by these 134 native/compatibility presence predicates.
- DOC-10 specifies strict-DTD behavior but does not name the shared
  `catalog_reading_rules.dialect` token. The integrity view uses the explicit
  candidate marker `strict-dtd`; its canonical reading-rule identity still
  needs policy confirmation rather than being treated as source-established.
- Relationship identity/edition, native ownership/media-kind, mixed order,
  and field/default-position guards are shared assembled-candidate checks,
  not still-uninstalled MAME work. Their bounded constructed tests do not
  prove the complete model or a production publication implementation.
- Root complete extents require new parser capture; current opening-only
  coordinates are insufficient. Ordinary-child extents remain unavailable.
- ROM size/offset checked interpretation is documented, not implemented by
  these literal-retention tables. Complete UUID/hash linking, diagnostic/source
  span, cross-table mixed order and publication proof remain unapproved.
- Both SQL fixtures provide populated index-backed query-plan witnesses;
  neither is a production performance measurement or complete approved design.

The presence implementation introduced `mame-field-presence.tsv`; the bounded
NativeSol P2 follow-up edits only `mame_presence_check.py` and these notes.
The manifest, native SQL, coverage and shared files remain untouched. The two
follow-up files are re-frozen for NativeSol re-review. The formerly RED
shared-gate regression also remains intact; it was not weakened to obtain GREEN.
The design approval gate remains open; no production build/gate, commit, push
or external-document edits were performed.

## Native child and PCDATA cardinality — candidate 2026-10-08

`mame_cardinality.sql` supplies the family view
`candidate_mame_cardinality_problems(problem,owner_id,edition_id)`. The main
assembler explicitly includes it in the cardinality audit discovery and
publication closure. Root-without-machine is attributed to the real
`mame_documents.edition_id`; a missing required machine description is
attributed to its actual `mame_machines.set_id` and registry edition. Detached
orphan ancestry remains the assembler's separate ownership audit.

The view enforces the parser's accepted requirement that every MAME document
contain at least one machine, independent of dialect: `parse_machine_records`
returns `MAME document has no machine records` when it sees none. Each machine
must have its singleton `description` text child. Empty description, year,
manufacturer and RAM-option PCDATA remain valid values; optional year and
manufacturer absence remains valid. Existing typed constraints already cap
machine text kinds and singleton cardinality, condition/value ownership and
other locally unique optional children. The view does not repeat those local
UNIQUE/CHECK/FK rules. RAM PCDATA remains a non-NULL field on each RAM-option
row, preserving present-empty text.

The strict `strict-dtd` reading mode additionally checks the pinned machine
child sequence, switch condition/location/value sequence, and device
instance-before-extension sequence. The machine sequence ranks are exactly
description, year, manufacturer, BIOS set, ROM, disk, device reference,
sample, chip, display, sound, input, DIP switch, configuration, port,
adjuster, driver, feature, device, slot, software list, RAM option. These
strict-only scans filter to strict owners before applying a prior-maximum
window; they do not pairwise compare every sibling. Observed compatibility
keeps source orders the parser accepts. The separate existing integrity view
continues to report a second device instance only for strict DTD; it is not
duplicated here. Shared mixed-sibling source-order collision detection also
remains in the assembler, not this family view.

For DTD-required attributes, the field crosswalk identifies `rom@size` as the
only required value intentionally nullable in the shared projection: observed
compatibility permits absence, while strict DTD does not. The cardinality view
therefore requires non-NULL `mame_roms.size_text` only for `strict-dtd`; it
does not validate decimal syntax, range, or interpretation, and accepts
present-empty or malformed retained text. The other DTD-required attribute
values, including root `mameconfig` and machine `name`, are stored in NOT NULL
columns. Existing field-presence publication checks require a position for
each retained required value. Required description and the parser-accepted
nonempty root are handled as child/root cardinality above. Erasing an entire
optional or repeated source record together with its registry entry can remain
undetectable when the surviving parent still meets its cardinality. That needs
independent source/parser evidence, not a count inferred from surviving rows.

`mame_cardinality_check.py` uses `assemble.assemble()` directly and requires
the installed view, so it cannot double-create it. Its constructed in-memory
cases cover complete roots with multiple machines, present-empty PCDATA,
required-description deletion/repair, zero-machine root rejection/repair,
strict machine/switch/device sequence rejection and repair, observed-order
positive controls, edition attribution, and publication rejection followed by
repair. A complete strict publication fixture jointly erases ROM size text and
its matching position, confirms the ordinary presence audit is quiet, and
proves strict cardinality rejects publication until both are restored. A
separate observed-mode publication control proves the same consistent
absence remains accepted. No positive assertion is inferred from persisted
row totals. This is SQL execution evidence only: it does not prove production
parser or corpus cardinality, nor detect jointly erased optional children
without an independent source count seal.

The installed scope intentionally leaves child-count seals for optional or
repeatable fields to separate work. Nested content not represented by typed
candidate rows, child-local uniqueness already enforced by the DDL, exact
source/parser correspondence, and corpus-wide conformance remain unproven.
The empty-root behavior is parser-established; the strict content sequences
are candidate policy for the named strict dialect and do not claim that the
current compatibility parser validates the upstream DTD sequence.

## Parser event count inventory — candidate

`mame-counts.tsv` is a closed design inventory: 30 native source-owner counts
from `mame-owners.tsv` and one count for each of the 33 physical
attribute-position tables. Each owner row is counted once, and each position
row is counted once per table. The physical root's fixed cardinality is not a
counter or fabricated registry. Position totals include recognized token-only
attributes such as explicit boolean/default flags; they do not create separate
owner counts.
Description, year, and manufacturer are child source-element rows in
`mame_machine_text_elements`. RAM-option PCDATA is a scalar on its
`mame_ram_options` owner row, not a second child owner. Root `mameconfig` and
other attribute CDATA remain attributes, not PCDATA child events.

The `source_event` values name proposed increment points in the existing MAME
reader/dispatch (root start, `parse_record`, machine-child or specification
dispatch, and the typed attribute selector). They describe candidate wiring;
no parser-fed count-seal comparison or accepted-EOF integration is claimed.
Each `scope_sql` maps the row identified by `key` to its edition through the
physical root, `catalog_sets`, or `catalog_source_elements`; it is edition
scope only, not a query that constructs expected counts from persisted rows.

The inventory does not certify capture completeness. Current opening-tag
locations do not supply complete element spans. The parser's filtered nested
walks also lack actual source ordinals for conditions, input controls, port
analogs, slot options, and device instance/extension order; unknown children
may leave gaps. Those order/span gaps do not change which recognized typed
owners or attribute-position rows this inventory names. Parser-fed totals,
their comparison with candidate rows, and the point at which totals become
eligible for sealing after accepted input completion remain unimplemented and
unproved.

`mame_count_mapping_check.py` exercises the candidate's constructed
count-to-row comparison with a literal, counter-keyed fixture vector. All 63
routes have positive rows in the constructed fixture; route-by-route deletion
inside savepoints produces only the named counter mismatch, and changing only
that literal seal to zero makes the source-count audit quiet. A second
constructed ROM makes the same-scope ROM/disk table-body swap fail at the
baseline mapping assertion. These are fixture mapping receipts, not parser
event totals, parser-fed seals, or EOF/source-capture proof.
