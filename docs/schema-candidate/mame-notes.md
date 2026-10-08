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

The pinned DTD inventory has 125 ATTLIST declarations, but this candidate does
not claim all 125 are independently proven. The 30 native owner ledgers
currently encode 116 distinct field codes. The typed `mame_switches`,
`mame_switch_locations`, and `mame_switch_values` each serve two DTD elements
(`dipswitch`/`configuration`, `diplocation`/`conflocation`, and
`dipvalue`/`confsetting`); those nine repeated declaration slots are
disambiguated by the owning element kind, not duplicated field values. The
three compatibility ledgers add ten closed codes: machine `isconsumable`; ROM
`md5`, `soundonly`, `dispose`, `loadflag`, `value`, `inverted`, `ovha`,
`nothread`; disk `writeable`. This is a declaration-count seal of 125 + 10,
not 135 distinct attribute spellings. The three compatibility position
families encode ten additional observed-compatibility codes; these are not DTD
attributes. The nine repeated declaration slots are accounted for in the DTD
inventory, but independent per-kind source-enum/field witnesses are still
missing. Thus `116 + 10` is the implemented candidate code count, not proof
coverage of `125 + 10`; do not report the nine slots as proven by shared
tables merely because their values have the same names.

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

## Remaining design gaps

- The bounded SQL witness checks strict-vs-v3 instance diagnostics and root
  span completeness, but does not prove all declaration-state relationships,
  field presence, or cross-table mixed order.
- The nine shared switch-family DTD declaration slots need an independent
  field inventory witness under both element kinds; they are not nine extra
  value columns or position rows on one source owner.
- DOC-10 specifies strict-DTD behavior but does not name the shared
  `catalog_reading_rules.dialect` token. The integrity view uses the explicit
  candidate marker `strict-dtd`; its canonical reading-rule identity still
  needs policy confirmation rather than being treated as source-established.
- Relationship identity/edition agreement, media-kind agreement, required
  description/default-presence rules, and all cross-table ownership checks
  remain in the main assembler's generated checks.
- Root complete extents require new parser capture; current opening-only
  coordinates are insufficient. Ordinary-child extents remain unavailable.
- No populated query-plan witness or complete approved design exists yet.
