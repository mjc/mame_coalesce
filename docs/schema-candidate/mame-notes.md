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
position families to `catalog_entry_hashes`; the main assembler should enforce
one-to-one presence/state closure using these owner/code/occurrence columns.

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
Main must enforce the matching relationship ID, owner key, TEXT field code,
occurrence, reported kind and edition in both directions, including uniqueness
across the four position families. The local UNIQUE/FK/CHECK constraints do
not prove that cross-table closure.

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
by the TEMP audit.

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

## Remaining design gaps

- Constructed field coverage is complete for the stated 135 attribute
  declarations and four text scalars, not real-source/parser count-seal proof.
  Optional value-plus-position joint erasure remains undetectable without
  independent source evidence. The field TEMP audit is not a publication guard.
- DOC-10 specifies strict-DTD behavior but does not name the shared
  `catalog_reading_rules.dialect` token. The integrity view uses the explicit
  candidate marker `strict-dtd`; its canonical reading-rule identity still
  needs policy confirmation rather than being treated as source-established.
- Relationship identity/edition agreement, media-kind agreement, required
  description/default-presence rules, and all cross-table ownership checks
  remain in the main assembler's generated checks.
- Root complete extents require new parser capture; current opening-only
  coordinates are insufficient. Ordinary-child extents remain unavailable.
- ROM size/offset checked interpretation is documented, not implemented by
  these literal-retention tables. Complete UUID/hash linking, diagnostic/source
  span, cross-table mixed order and publication proof remain unapproved.
- Both SQL fixtures provide populated index-backed query-plan witnesses;
  neither is a production performance measurement or complete approved design.

MAME-owned files are frozen for NativeSol/Banach review after these focused
checks. The design approval gate remains open; no production build/gate,
commit, push or external-document edits were performed.
