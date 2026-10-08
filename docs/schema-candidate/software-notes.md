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
This DDL therefore keeps the source lexeme and leaves the checked numeric
projection to the pinned software-reading-rule implementation; it does not
store a potentially inconsistent second numeric value. This is a known
candidate DDL/query gap until the design selects a deterministic SQL function
or an explicitly validated projection mechanism.

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
- DOC-12 assigns title opening location to `catalog_sets`, but current shared
  candidate `catalog_sets` has no location columns. Keep that title location
  on the common set row; do not duplicate it on `software_titles`.
- Add all thirteen software `element_kind` strings from `software-owners.tsv`
  to the assembler's `/* SOURCE_ELEMENT_KINDS */` closed ledger. List root,
  wrapper root and document are physical owners, not registry rows.
- `assemble.py` must union `candidate_software_integrity_problems` into the
  assembled publication audit. Its generic owner resolver also needs to walk
  media ancestry through `software_data_areas`/`software_disk_areas` to the
  registered `software_areas` owner. For mixed ordering, merge the
  `list_children` manifest domain with common `catalog_sets` placements; the
  title manifest row deliberately has no duplicate order.
- Full attribute-presence↔position one-to-one checks for all 37 codes and
  explicit-default↔attribute-position checks are still unproven. Table-local
  checks enforce each stored effective value/presence pair, and the software
  view checks the clone/hash subsets; they do not prove absence/extraneous
  position closure for every field. The parser-to-row count seal is also not
  represented: manifest owner closure proves one typed owner per listed source
  element, not that all parser-counted source elements were persisted.
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
