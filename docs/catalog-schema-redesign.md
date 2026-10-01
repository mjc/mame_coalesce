# Catalog schema redesign from all input formats

Status: corpus-expanded and adversarially reviewed proposal, 2026-09-30. This
document specifies the next catalog storage model; it does not describe an
implemented migration. The original proposal received four Luna audits and a
repeated Sol review. The expanded Sol review identified the six Xbox 360 NULs,
the observed-vs-strict v3 distinction, and release/source ownership cases;
those findings are incorporated here. Existing SQLite imports and source
objects remain the comparison baseline.

Durable design: [MAMEC-DOC-7](https://lific.mjc.lol/MAMEC/pages/89).
Implementation sequence: [MAMEC-PLAN-3](https://lific.mjc.lol/MAMEC/plans/108).
The expanded proposal now records the adversarial review findings. Importer
implementation and field/query conformance testing remain outstanding.

## Decision and scope

Design from document entities, field meaning, cardinality, and actual queries.
Keep a small shared identity/provenance model and narrow tables for each native
entity. Share a fact only when its meaning is equivalent across formats. Expose
common catalog queries through typed projections over those facts, without
storing another copy of every projection.

Every field in a supported, pinned document specification belongs in relational
query storage. SQLite stores catalog facts only: declared names, sizes, hashes,
formats, provenance, and relationships. It never stores ROM, disc, image,
manual, CUE, or imported DAT/XML byte payloads. Exact input document bytes stay
in verified zstd source objects outside SQLite; actual media payloads remain
outside the catalog store entirely. Vendor fields outside the supported
dialect remain recoverable from the original external document. No JSON
columns, JSON composite keys, universal attribute/value tables, or catalog-sized
owned syntax trees are part of the proposed model.

The user accepted the latest 471,031,808-byte MAME SQLite import as a sufficient
size baseline. Its source object is 15,305,382 bytes; its original XML is
326,688,140 bytes. Further byte reduction is measured alongside field coverage
and queries, rather than treating a database smaller than XML as a prerequisite
for this redesign. No new memory cap or field truncation is proposed.

## Schema shape and implementation state

The greenfield cutover is underway, not a migration. Shared file/digest identity
does not establish complete field coverage for every input format. Native
`catalog_set_groups` and `catalog_sets` now own ordered groups and sets by
integer IDs, and retain source names as values. `snapshot_sets`, `records` and
`record_namespaces` are read-only consumer views, not stored owners. Native
MAME, Logiqx and P/C set details no longer copy snapshot keys and set names.
Coverage members are typed relational rows, not JSON. Source assertions follow
numeric set/occurrence owners, including repeated names. Unknown vendor fields
remain in the external source document, not a catch-all SQLite table.
History compares complete same-name fact multisets rather than treating source
positions as continuity. SQLite has no JSON columns or generic vendor-field
storage. Logiqx header options, comments, releases, BIOS sets and archive
references now have native tables; ROM, disk and sample occurrences retain
source order and default presence. ROM size text is retained once, with a
virtual numeric projection. Ordinary imports and builds now use published native
snapshots; the old mutable DAT/game/ROM/archive-file tables and scanned-file
association column are removed. Build queries derive expected evidence and
metadata from native owners in one read transaction. Shared file UUIDs now use
an immutable registry generation, source-backed hash/size views and native
conflict evidence; published cross-list membership has bounded bulk and keyset
queries. Remaining per-format coverage, reviewed conflict settlement/redirects
and observed-byte endpoints are unfinished; this is not a claim of complete
DTD conformance.

The target hierarchy uses integer IDs for ownership and retains names and
publisher IDs as source fields:

```text
snapshot -> set group -> ROM set or software item
                           |
                           +-- exactly one native record payload
                           |
                           +-- ordered native child facts
                           |
                           +-- asset/source-operation occurrence
                                 +-- exactly one native claim/operation payload
                                 +-- digest assertions (scope and provenance)
                                 +-- optional shared content UUID
```

Each row below describes a different source structure, with its own typed
columns and child relations. They meet at record/occurrence identity and digest
assertions; they are not reduced to a shared game name and ROM tuple.

| Input | Native structure and facts | Current implementation gap |
|---|---|---|
| MAME machine XML | Machine -> ROM/disk/sample, chips, displays, input/controls, switches/values/conditions, ports, devices, slots, driver/features, software-list references and RAM options. | Native families and numeric owners replace the wide stored union; complete specification presence/default and field witnesses remain open. |
| MAME software-list XML | List -> item -> part -> data/disk area -> ordered ROM/disk entries. File declarations and load/continue/reload/ignore uses are separate relations; fill has no file claim. | Numeric owners, separate declarations/operations and area-local loading chains exist; complete per-flag loading rules and native field witnesses remain open. |
| Logiqx XML / TOSEC | Document/header/options -> game -> comments, releases, BIOS sets, ROM/disk/sample claims, archive references and distinct parent declarations. | Partial document/game facts and ROMs exist; native options, releases, BIOS/disk/sample/archive families and specification defaults remain incomplete. |
| ClrMamePro text | Header/directives -> set -> ROM claims and scalar sample claims; native flags and sample-parent declarations. | Native CMP facts/directives and numeric set/occurrence owners exist; remaining specification fields and complete query witnesses remain open. |
| No-Intro flat DAT v3/v4 | Document/header/directives -> game -> scoped identifiers, categories, releases and ROM declarations; name-based and ID-based parents stay distinct. | Separate v3/v4 strict and observed-compatible interpretations stream into native typed owners. Raw size text, interned valid hashes, typed invalid hash literals, SHA-256 assertions, options, field positions and repeated children are retained. Full corpus/query/performance acceptance remains open; this is not a Logiqx envelope import or database-export coverage. |
| No-Intro database XML | Game -> archive, source histories and releases. A source or release owns its own details, serials and files; numeric file IDs may repeat under different owners. | The one-pass reader and native writer preserve the observed field ledger, both envelopes, field presence/order and distinct details/serial/file owners. Bulk file queries, scoped interned digests, typed archive references, NUL warning ownership and paired backup regressions exist. All 275 exports pass reader verification; full native SQL corpus/history acceptance remains open. |
| Synthetic No-Intro P/C projection | Archive record -> ordered language/native fields and file claims, with parent-marker/reference/merge-token distinctions. | Typed archive, region, alternate name, version, BIOS, ordered languages and distinct clone/merge tokens exist; this is not proof of authentic DAT-o-MATIC P/C conformance. |

TOSEC ISO/PIX assets retain their source `rom` declaration even when the file
is a CUE, image or manual. OfflineList and PureDOS-specific structures remain
explicit coverage gaps until their adapters and dialect contracts are supported.
The family inventories below specify fields, cardinalities and ordering in
detail. Completion requires those native tables and importer/query witnesses,
not just the common identity layer.

The No-Intro flat DAT physical table/field/position dictionary is recorded in
[MAMEC-DOC-14](https://lific.mjc.lol/MAMEC/pages/97). Its native writer and
consumers passed repeated GPT-6.1 Sol medium review/fix/re-review cycles,
the complete devenv gate, 835 all-feature tests (three existing optional skips),
and warning-denied all-feature documentation. A fresh profiling-build import of
four authentic SG-1000/Game Boy/NES DAT and parent-clone documents retained
11,923 games, 11,925 ROMs, 5,529 releases, 37,716 source hash fields/assertions,
and 65,897 ROM field positions. Two thousand UUIDs have membership in both
Game Boy lists; the filtered NES catalog's 7,383 ROMs have no whole-file UUID.
SQLite/foreign-key/application integrity checks found no issues. Originals
remain outside SQLite; no existing database or profiling artifacts were removed.
This is a bounded native-format milestone, not the full 664-file import/query
acceptance matrix or coverage of authentic P/C and database-export XML.

The separate database-export reader has now parsed all 274 pack files plus the
FDS specimen: 131,533 games/archives, 172,997 dump sources, 29,462 releases,
562,938 source-owned files and 101,831 release-owned files. The 170 single-root
and 105 sibling-header documents all reached valid EOF. Exactly six NULs were
recovered in the Xbox 360 document; their original one-based coordinates match
an independent scan. After removing repeated nested-header cloning, the rebuilt
profiling executable took 5.04 seconds and 55,936 KiB peak RSS including startup.
This proves parser coverage, not native SQL persistence or complete importer
acceptance. Recovery warnings iterate borrowed decoded originals rather than
owning another catalog-sized collection. Native persistence now uses that
single-pass reader rather than a second parsed catalog tree. Its regression
witnesses cover every observed owner field, empty/absent values, header and
mixed-child order, scoped digest interning, typed file queries, unresolved
archive references, late-EOF rollback and paired backups. Recovered NULs retain
exact UTF-8/UTF-16 bytes, separate gzip-view offsets and excerpt-local highlights;
details-opening bounds prove source/release diagnostic owner links. This does
not substitute for a fresh full native SQL corpus run or history acceptance.

A fresh profiling-build SQL import of the acquired FDS and Xbox 360 exports
published both documents: 17,853 games/archives, 14,544 dump sources, 4,751
releases, 31,938 source files and 5,045 release files. There are 115,136 scoped
file digest assertions and 116,478 interned digest values, including NFO
companions. Unknown export-file scope assigns no shared file UUID. All six Xbox
NUL diagnostics retain original byte offsets and a `[0,1)` highlight within
the stored `00` excerpt; independent original-file scanning matched every
offset and Unicode line/column. Five warnings link to actual release details
and one to dump-source details. SQLite quick/FK checks pass. Including startup,
the import took 216.61 seconds with 155,072 KiB peak RSS. SQLite logical size is
64,593,920 bytes; compressed external originals occupy 3,798,264 directory
bytes. This is native persistence evidence for two exports, not the complete
275-document SQL/query/performance acceptance matrix.

A native DAT corpus run exposed a publication bug: a later conflicting claim
could invalidate an earlier association and reject the whole document. The
dispute guard now rejects linking an already disputed hash when source evidence
is inserted; immutable earlier associations and conflict evidence remain
queryable, while subsequent resolution quarantines the alias. A shared
allocated-owner guard also prevents assertion-first insertion with deferred
foreign keys. The three formerly failing 3DS, N64 byte-swapped and encrypted DS
lists now import 10,991 ordered ROM occurrences into a fresh database, retaining
nine contradictory-evidence records with clear SQLite/FK checks. The complete
664-file native baseline run remains ongoing, not accepted as complete; it
started with the previous guard revision and needs final audit.

Additional schema-backed witnesses cover every strict DAT v3/v4 field with
distinct values and exact owner/field/ordinal/coordinates, including empty and
absent values, defaults, integer boundaries, hash interning and paired backup
recovery. Database-export witnesses check every observed ledger field with
distinct values, owner order/coordinates and current/origin/NFO digest roles.
Shared tests cover CR/LF/CRLF coordinates, UTF-16 and original-byte encoding
declarations; split text/reference/CDATA cannot bypass the scalar budget.
The complete devenv gate passed (851 tests, one existing optional skip),
all-feature nextest passed 854 tests with three existing optional skips, and
all-feature documentation passed with warnings denied. GPT-6.1 Sol medium
review/fix/re-review cycles were clear after the final parser/test refactors
and the diagnostic clipping-contract correction. This parser milestone did
not implement the excerpt/highlight/FK contract below.

## Evidence and corpus coverage

The CLI currently exposes six parser families: Logiqx XML, MAME machine XML,
MAME software-list XML, ClrMamePro text, the synthetic No-Intro P/C projection,
and No-Intro flat DAT XML with explicit v3/v4 interpretation modes.
Compression is a transport property, not another document schema. Publisher
identity (No-Intro, TOSEC, Redump, etc.) is independent of syntax and dialect.

A streaming structural pass over all unpacked XML/DAT files in
`target/profiling/external-catalogs-2026-09-29` found the following. The inspector
counted element paths and attribute names and cleared parsed elements; it did
not validate a DTD or invoke the application's import adapters.

| Corpus input | Observed structure | Coverage consequence |
|---|---|---|
| MAME 0.289 full machine XML, 326,688,140 bytes | 50,368 machines; 371,752 ROMs; 1,402 disks; 781,216 device references; 609,206 DIP/configuration switches; 1,783,403 switch values | The embedded DTD is a concrete, versioned machine-format contract. All its native families need tables. |
| 777 unique MAME software-list files | 778 file instances, including an identical extra `vgmplay.xml`; 145,856 software records across those instances, 303,562 parts, 303,586 ROM entries, 11,563 disk entries | Preserve list/item/part/area nesting and ordered load entries; file-instance totals are not deduplicated totals. |
| `logiqx/PureDOSDAT.xml`, 3,945,595 bytes | 216 games; 21,491 ROM entries; developer/comments/links; nested source, track, and patch metadata | This is a Logiqx-shaped extended dialect. Baseline Logiqx conformance does not prove its additional structures are supported. |
| `clrmamepro/Atari-2600.dat` | Real text DAT with header, game fields, ROMs, region, and release-date fields | The XML inspector reports a syntax error for this text input; the text adapter must be assessed separately. |
| `no-intro-pc/pc-engine.xml`, 213,911 bytes | `<dat>/<configuration>` and `<games>/<game>`; 425 records, ROM CRCs and separate image CRCs | It is an OfflineList-style document, not an authentic DAT-o-MATIC P/C XML specimen. The current synthetic No-Intro adapter cannot import it. |
| Checked-in TOSEC, Redump, and No-Intro examples | Synthetic metadata-only fixtures, identified as such in `fixtures/catalog/manifest.json` | Useful semantic regressions, not proof of production export coverage. Redump CUE is a separate companion document, not a currently supported import adapter. |

The duplicated `vgmplay.xml` instances both have SHA-256
`61bfa8546cd34be1ac24cf2087c150c0b1af5cbc94219398b459ff10e73a4a08`.
No ROM or disc data is required for this work.

The user added a larger local corpus under the ignored
`local/catalog-data/2026-09-30` directory (5,685 files; 1.1 GiB apparent size,
269 MiB allocated on the compression-enabled filesystem).
It is local research input, not a checked-in test fixture. A streaming audit
found several schemas within one publisher and file extension:

| New corpus | Files | First schema observation |
|---|---:|---|
| No-Intro DAT pack | 336 DAT + 1 index | No-Intro XML datafile schema v3/v4: 237,787 games and 1,526,732 ROMs. v3/v4 split: 210/126 files. |
| No-Intro PC pack | 328 DAT + 1 index | Same publisher family with a parent-clone catalog subset: 133,525 games, 450,115 ROMs and 52,473 releases. v3/v4 split: 202/126 files. |
| TOSEC complete pack | 4,743 DAT | All use the Logiqx XML envelope: 1,062,978 games and 1,333,246 `<rom>` entries, including ISO/CUE images and PDF manuals. Subsets are TOSEC (3,111), TOSEC-ISO (300), and TOSEC-PIX (1,332). |
| No-Intro database export | 274 XML + 1 index | Distinct `datafile/game/archive/source/details/serials/file` dialect; at least 115,527 games. 170 files have one root; 104 serialize sibling `header` and `datafile` roots. One Xbox 360 export contains six XML-illegal NUL characters in source/release details; narrow recovery permits the complete catalog to import. |
| Nintendo Family Computer Disk System export | 1 XML | Same observed archive/source export records: 408 games, 1,546 sources, 1,579 files; its header is a sibling of `datafile`. |

The official [No-Intro DAT XSD v3](https://datomatic.no-intro.org/stuff/schema_nointro_datfile_v3.xsd)
requires a header with typed ID/name/description/version/author, and defines
ordered game categories, descriptions, ROM claims, and releases. A ROM requires
name, unsigned 32-bit size, CRC, MD5, and SHA-1; SHA-256, status, serial, and
header are optional. The current Logiqx reader can parse the shared envelope,
but it does not satisfy this dialect's field contract: its generic extension
capture is not typed storage, and some unknown header children or game child
families are dropped. The producer's
[v4 XSD](https://datomatic.no-intro.org/stuff/schema_nointro_datfile_v4.xsd)
was recovered on 2026-10-01 (HTTP 200, 3,475 bytes). It adds optional header
`trademarks` and `piracy`, retaining the v3 one-ROM cardinality, required fields
and unsigned 32-bit size. Version 3 and 4 counts are from each file's
schemaLocation, not a claim that all documents passed XSD validation. In
particular, some v3-labelled records have multiple ROMs, missing required
digests, nested `game_id`, or sizes above the XSD's unsigned 32-bit limit. Keep
strict v3/v4 validation distinct from explicitly named observed compatibility
interpretation; do not silently relax the pinned XSD. Give each schema or
compatibility revision its own interpretation identity while reusing native
tables only where the meanings and cardinalities agree.

The database exports need a different model. A game can own an archive record
and multiple source histories; each source can own details, serials, and one or
more file records. Numeric IDs, clone markers, and revisions have scopes that
must be proved before using them as cross-snapshot keys. The captured exports
show fields such as dump/release dates and confidence flags, region, dumper,
project, origin, tools, comments, links, serials, formats, sizes, and several
digest algorithms. The existing `<datafile>/<game>/<rom>` P/C projection does
not parse these records. The current project-root HTML guidance describes many
No-Intro archive, source, and file conventions, but it does not establish this
database-export XML grammar as a versioned public schema. See the upstream
[archive naming conventions](https://wiki.no-intro.org/index.php?title=Naming_Convention),
[source conventions](https://wiki.no-intro.org/index.php?title=Source_Convention),
and [file conventions](https://wiki.no-intro.org/index.php?title=File_Convention);
keep the exported document itself as the wire evidence. In this corpus, export
headers also contain `version`, `author`, `url`, `trademarks`, and `piracy`.

The 274-file DB export corpus also has two top-level serialization forms: a
single `<datafile>` root, or sibling `<header>` and `<datafile>` elements. The
latter 104 files are not well-formed as a single-root XML document, even though
their records can be read by framing the sibling elements. The Xbox 360 file
contains six NUL characters in source and release details. Replacing all six
with U+FFFD after decoding, in the in-memory parse view, produces 17,445 games,
12,998 sources, and 35,404 file occurrences. The first NUL is at original-input
line 17,628, column 269 (one-based). Support must account for the sibling
envelope, recover only these U+0000 characters, record non-fatal recovery
diagnostics at original source locations, and continue importing. Decode before
replacement; replacing raw zero bytes could corrupt UTF-16. Preserve original
input bytes, digest, and external source object exactly. Publish a recovered
interpretation only after the remaining document passes strict XML-character,
grammar, EOF, and transaction checks; persist the recovery diagnostics and
identify the compatibility interpretation separately from strict parsing.
Other XML-forbidden characters remain errors. This is separate from the
No-Intro DAT v3 contract, which requires one root and a header inside
`datafile`.

The TOSEC sweep changes the shared query model too: `<rom>` is the native
Logiqx element name, but TOSEC-ISO and TOSEC-PIX use it for CUE/image assets and
PDF manuals. Keep the source-declared element/role separate from any inferred
media classification. The DAT does not declare track layouts or prove that a
CUE and ISO with similar names belong together. TOSEC also has header
`<clrmamepro/>` markers the current parser skips; those must be typed if their
semantics affect query behavior. Tokenizing bracketed descriptors out of game
names would be a separate, evidence-backed TOSEC rule, not part of XML parsing.

## Problems in the existing fit

- `SnapshotSet` combines all format adapters using optional MAME, No-Intro, and
  Logiqx fields plus JSON metadata. The shared storage shape is influencing what
  survives an import.
- `MachineSpecification` is already a Rust enum with native variant structs.
  SQLite flattens those variants into the 60-column
  `mame_machine_spec_elements` table. A port row carries the record header for
  display, driver, device, and other unrelated fields.
- Software-list child keys repeat snapshot/list/item/part/area strings. Integer
  parents should carry those identities once through foreign keys.
- A software-list ROM entry can be a load, reload, continue, fill, or ignore
  instruction. Its `size` can describe a segment rather than the entire input
  file. It cannot always be treated as a new ordinary file requirement.
- Logiqx fields have dedicated facts that equivalent ClrMamePro fields currently
  lack. No-Intro archive facts are divided between columns, JSON, and extensions.
- Generic source and derived relationship rows carry different kinds of
  endpoint information in one wide nullable shape. Source declarations should
  be queried from their authoritative facts rather than copied into a second
  explanation payload.
- The downloaded corpus contains dialects the named parser paths do not fully
  cover. A successful shallow import is not a field-completeness result.
- The MAME DTD audit found concrete contract mismatches to address with the
  redesign: required root `mameconfig` and BIOS-set `description` are optional
  in current models/storage. The software-list ROM parser already materializes
  the effective status default `good`. These checks are not a complete proof
  of every grammar rule.

## Shared identity and provenance

Use integer primary/foreign keys internally, with distinct Rust newtypes.
Persist external stable document/snapshot identities once at the relevant
parent. Never make names, declared versions, or local integer row IDs global
game/content identities.

| Relation | Owned data and constraints |
|---|---|
| `sources`, `catalogs` | Publisher identity, locator, collection identity and display name. |
| `documents` | SHA-256, exact byte length, compression codec and relative object key; source bytes are external. |
| `acquisitions`, `acquisition_attempts`, ordered transport-header children | URI/path, method, time, expected digest, verification/outcome and provenance. Repeated header names and order survive. |
| `interpretations` | Syntax family, supported dialect/spec revision, parser version and rules version. Real parser options use dedicated typed columns/relations. |
| `snapshots`, `snapshot_publications`, `import_runs` | Catalog/document/interpretation identity, lineage, publication state, declared version and import result. Immutable facts publish only after valid EOF and transaction completion. |
| `snapshot_scope`, `scope_member_names` | Unknown/complete/filtered/partial scope and ordered or set-valued members according to the actual scope contract. No serialized name list. |
| `record_namespaces` | A snapshot root namespace, or a software-list namespace. This keeps software-list item names scoped to their own list. |
| `records` | Namespace FK, native record kind, source order and source name; native facts attach by `RecordId`. Name lookup may be ambiguous unless the supported dialect guarantees uniqueness. |
| `asset_occurrences` | One source occurrence in a record: either a file/media claim or a digest-bearing source/load operation. It preserves owner and order; file/media claims may reference a shared content UUID. List-specific name, role, status and native semantics remain occurrence/native facts. |
| `catalog_contents` | One deduplicated catalog-level whole-file content identity with a persistent 128-bit UUID stored as its 16 canonical bytes in one SQLite BLOB; any number of asset occurrences can reference it. It contains no ROM bytes and no list-specific naming/role metadata. |
| `digest_values` / `occurrence_digest_assertions` | Interned algorithm and bytes, plus a source-attributed occurrence link carrying scope and provenance. This is the authoritative owner of common digest assertions; native subtype rows do not duplicate those values. |
| `catalog_content_digest_assertions` (view) | Qualified whole-file source assertions joined to their linked occurrences and catalog editions. No separately stored alias or digest copy. Candidate lookup deduplicates UUIDs, not source evidence rows. |
| `file_id_registries` | One immutable database-wide generation UUID; retained by backup/restore. Fresh destructive rebuilds without the registry start a new generation. |
| `assertions` | Compact identity and source/derived/user origin. Source-native declaration tables own source fields; derived/user subtype tables own explicit endpoints and rules. |

The direct-create SQLite schema implements shared identities and normalized
digest storage; there is no migration chain. `catalog_contents.content_uuid`
is a persistent 16-byte BLOB with a registry-generation FK and no copied size.
`digest_values` interns only `(algorithm, digest bytes)`;
`catalog_content_digest_assertions` derives eligible identity aliases from
qualified source occurrences. `asset_requirement_digest_assertions` and
`software_component_digest_assertions` retain each occurrence's native key,
digest, scope, and provenance. The physical occurrence rows no longer store
CRC/MD5/SHA-1 bytes. `asset_requirements` and `software_components` currently
derive the former single-hash columns as compatibility projections; direct
query consumers still need to move to long-form assertion queries as the
remaining schema migration proceeds. MAME software-list and machine imports
share eligible SHA-1 identities without collapsing their occurrence rows;
operation-only and scoped disk assertions remain unlinked. SHA-256 parser
coverage and explicit digest-bridge review/redirects remain outstanding.
UUID interchange is checked undashed 32-hex text, not stored text.
Conflict hash/size children point to their actual native owners; unresolved
disputes also block later sparse declarations. Published cross-list occurrence
queries keep source/catalog/edition provenance and separate digest children;
reverse membership pages bind their cursor to the UUID and registry generation.

Foreign keys must keep a record, its namespace, and its snapshot consistent.
Source name/order uniqueness is decided per pinned dialect, not imposed on every
catalog. Source-location columns live with the authoritative native fact; a
generic node/location row is not added for every XML token.

The minimal key shapes are `record_namespaces(namespace_id, snapshot_id, kind)`,
`records(record_id, namespace_id, kind, source_order, source_name)`,
`asset_occurrences(occurrence_id, record_id, occurrence_order,
native_claim_kind, content_uuid?)`,
and `catalog_contents(content_uuid BLOB PRIMARY KEY NOT NULL
CHECK(length(content_uuid) = 16))`.
The 16 bytes use canonical UUID octet order; this is a single exact BLOB key,
not SQLite `REAL`, decimal text, or a truncated integer. Textual interchange,
when needed, uses 32 lowercase hexadecimal characters without dashes and is not
the stored key. An optional occurrence FK is either NULL or exactly 16 bytes.
A native claim payload has its occurrence ID as primary key and FK; it stores
no second copy of the owning record's name or
snapshot key. Its discriminator must match its native subtype. Nested payload
owners must belong to that same record/namespace/snapshot, enforced with
composite FKs or specific checked insert rules, not merely Rust conventions.
`UNIQUE(record_id, occurrence_order)` preserves repeated declarations while
giving each its exact document-preorder position across nested parts, areas,
and file/media families. Native payload rows reference the occurrence ID and
matching discriminator; native area/operation ordinals are retained separately
for their scoped semantics. Common digest bytes are interned by
`(algorithm, value)`; `occurrence_digest_assertions` links those bytes to each
digest-bearing source occurrence and carries that occurrence's scope and
provenance. It is the single authoritative owner of common digest assertions.
Operation-only digest facts remain attached to the source operation occurrence
and are not promoted to a shared file identity.
`content_identity_digest_aliases(content_uuid, digest_id, scope)` links
eligible aliases to identities without enforcing global uniqueness. A
per-occurrence digest query returns multiple assertions without multiplying the
one-row-per-occurrence catalog projection.

Every published occurrence has exactly one native payload; a content identity
may have any number of occurrences, including occurrences from different
catalogs and formats. Assign its UUID once and preserve it through reimports,
backups and restores; never regenerate a different UUID for an already-known
identity. Digest aliases find an existing UUID or qualify a new identity, but
the digest itself is not used as the cross-list key. A destructive full rebuild
must either restore the identity registry or be explicitly treated as a new
catalog generation; it must not silently claim the old UUIDs remain stable.

Deduplicate identity, not source-list membership. Two identical ROM
declarations in different lists remain two occurrences, preserving each list's
record, source order, declared name/role/status, source location, explicit vs
defaulted fields, and source-attributed digest assertions. Both may reference
one `catalog_contents` row, so global content queries return one identity with
all source occurrences. Native query projections join those occurrences and
their authoritative native facts; they do not clone a ROM row per list.

This identity represents catalog-asserted expected whole-file content, not
verified physical bytes. A matching parsed source SHA-1 or SHA-256 explicitly
scoped to the whole file is eligible to share a UUID; this is an assertion-based
deduplication rule, not a cryptographic guarantee that source claims are true.
CRC32/MD5-only evidence, filenames, size, merge tokens, scoped CHD/track/
subcomponent digests and parser guesses never assign a shared UUID; they may
produce candidate matches. Exact byte identity requires separately observed
bytes and their own verified digest.

Before associating an occurrence, compare every supplied size and digest
assertion against the candidate identity. Any contradictory size or digest
assertion blocks automatic sharing and remains visible on the source
occurrence. If eligible aliases resolve to no identity, create one UUID. If
they resolve to exactly one compatible identity, reuse its UUID. If they
resolve to multiple UUIDs or conflict, leave the occurrence unlinked and
record the candidate/conflict; never silently merge or pick a UUID. UUIDs are
immutable; a later reviewed resolution explicitly selects the canonical UUID,
redirects the other UUIDs with audit history, and updates alias lookup without
reassigning any issued key. Import-order permutations must preserve the same
occurrences, source assertions and unresolved candidate relationships; only
the generated opaque UUID values may differ in a new identity registry.
Missing eligible digests leave the occurrence unlinked. Observed local bytes
and locations remain separate inventory entities. The shared identity
relation is catalog metadata; it never stores ROM/media payload bytes. Only a
file/media-claim occurrence may carry `content_uuid`; operation occurrences
remain unlinked and associate with a file only through validated native
declaration/use relations.

For example, `mame_chips(machine_id, chip_order, ...)` has a clustered composite
PK, and `mame_input_controls(machine_id, control_order, ...)` references the
singleton input owner. Software parts and areas use distinct integer parent
IDs; native operations key by `(area_id, operation_order)`. There is no general
`node_kind/key/value` table behind those keys.

Public references include snapshot identity and native namespace/record/
occurrence identity. Internal IDs can change on rebuild without silently
retargeting an external review or manifest. Idempotence still binds identical
document bytes, catalog identity, interpretation, and scope.

## Import diagnostics and exact source excerpts

The excerpt/range columns and run/document FKs are implemented for fatal XML
character/encoding diagnostics and database-export NUL-recovery warnings.
Checked Rust ranges capture original encoded bytes, and clipping rebases the
highlight or makes it NULL if only part of the problem remains. A recovered NUL
inside a details opening tag has a typed FK to its actual dump-source or release
details owner, validated against the import snapshot and Unicode coordinates.
Linked diagnostic evidence, runs and links cannot be rewritten or replaced.
Additional token/field highlights and other native catalog-owner links remain
required; this is not complete diagnostic acceptance. Diagnostics are
warnings or errors about actual malformed or failed input. A report table
exposes the message and a relevant import-file excerpt. Necessary nearby field
text may appear in the excerpt, but diagnostics are not a second catalog-field
store: do not emit messages to duplicate ordinary fields, whole parsed subtrees,
whole serialized records/documents, or EAV data. Unsupported input remains
recoverable from the retained source document and is reparsed on demand.

Each diagnostic may retain `source_excerpt BLOB` as an exact byte window from
one of two explicitly typed deterministic views: `retained_original_bytes` or
`transport_decoded_xml_bytes`. Record the excerpt's start byte in that view
when known. For gzip XML, the decoded view is the decompressed XML byte stream
reproducible from the retained compressed original; its offsets are never
described as offsets into the gzip file. A UTF-16 excerpt remains the original
encoded XML bytes (or those same encoded bytes in the decoded view after gzip
transport decoding). Do not persist normalized or sanitized parser bytes as
an excerpt. BLOB storage preserves NUL bytes and invalid text encoding.

The offending slice is always a half-open byte range inside the actual saved
excerpt BLOB: `problem_start_byte` is inclusive and `problem_end_byte` is
exclusive, with
`0 <= problem_start_byte <= problem_end_byte <= length(source_excerpt)`.
Equal bounds are valid, including `[length(source_excerpt), length(source_excerpt))`
for a missing-token/EOF anchor. Bounds are either both NULL when exact bytes
cannot be identified, or both integer values satisfying the range constraint;
when present, an excerpt must be present. Never substitute a whole field or
guessed span. Verify the excerpt against its named source view at the recorded
view anchor. Original-file offending byte start/end are separate nullable
coordinates and are populated only when an exact mapping to retained original
bytes is known. A transport-decoded range is not an original-file range. A
clipped excerpt must update its view anchor and rebase the highlight against
the final saved bytes. Dropping eight prefix bytes changes `[10,12)` to `[2,4)`
and advances the view anchor by eight. Retain the complete offending span
inside the excerpt or leave both excerpt-relative bounds NULL; do not highlight
only the retained fragment. The known full offending range in the source view
remains independent of clipping.

Source line and column are independent of both byte ranges and carry their
coordinate view plus the parser's declared column convention. UTF-8/UTF-16
decoding, gzip transport decoding, XML entities, normalization, and display
escaping require exact mappings before parser coordinates can be attributed
to either stored view or original-file bytes. In particular, UTF-8 parser
coordinates for UTF-16 input do not establish original encoded-byte offsets;
leave an unproven mapping NULL. Normalization is a mapping concern, never a
third excerpt view. None implies that display characters, Unicode columns,
source bytes, and excerpt bytes share coordinates. Capture ranges from
token/field/character parser evidence while parsing, never by searching later
for repeated text. Excerpt size budgets limit diagnostic context only: they
do not cap imports, warning/error counts, or retained diagnostic evidence.
Choose no fixed byte budget until it has been reviewed.

Every diagnostic links to its import/run and retained source document through
real foreign keys. Owner links use a distinct typed relation for each persisted
owner table, not a generic unchecked `kind/id` pair. For example,
`catalog_import_message_sets(import_id, list_order, edition_id, set_id)`
has a composite FK to the message, a composite FK `(import_id, edition_id)` to
the import's declared candidate key, and `(set_id, edition_id)` to the set's
declared same-edition candidate key; an entry-owner relation follows the same pattern for
`catalog_media_entries`. Format-specific relationship/evidence owners receive
their own typed relations and FKs to their actual owner rows. Each link row
names the diagnostic and concrete owner, and may carry the edition key needed
for composite provenance enforcement. A diagnostic may have multiple owner
rows when one warning concerns conflicting evidence. Cross-catalog evidence
uses its own typed link relation with a closed evidence role and a real FK to
the candidate/conflicting evidence row; it does not weaken same-run owner
checks. If EOF failure rolls back catalog facts, record the failed run and
diagnostic in a separate failure transaction linked to the run and source
document; do not FK to a rolled-back row or preserve a partial catalog row
solely to attach the diagnostic. Catalog-owner links exist only for
independently persisted owners.

Future tests must prove exact highlights and owner behavior for Unicode,
embedded NUL and invalid encoding bytes, gzip XML (decoded-view excerpt
reproduction without gzip-offset claims), UTF-16 excerpts preserving encoded
XML bytes with parser-coordinate mapping or NULL, XML entities, and unknown
original-file mappings. Check both-NULL or both-integer bounds, reject
non-integer/out-of-range bounds and bounds beyond excerpt length, and accept a
zero-length EOF/missing-token range. Also test prefix clipping that rebases
`[10,12)` to `[2,4)` after removing eight bytes, clipping that cannot retain the
whole problem span (NULL highlight), multiple owners for conflicting evidence,
and EOF failure with catalog-fact rollback
while the diagnostic and source links survive.

## MAME machine native relations

The field lists below describe logical source/query fields, not a requirement
to duplicate every common digest in each native table. Whole-file CRC/MD5/SHA
assertions are owned once by interned `digest_values` plus the source-scoped
`occurrence_digest_assertions` link. Native field queries join those facts.
Digest-bearing software load/source operations receive their own ordered source
occurrence; operation-specific digests remain attached there and never become a
file claim unless the pinned format semantics establish one.

The embedded DTD in the full 0.289 XML defines the baseline below. `*` families
retain occurrence order. Singletons retain absence. Each family has its own
narrow row, including source location and explicit/defaulted attribute presence
where that distinction affects source queries.

| Native relation | Fields beyond its integer parent/key |
|---|---|
| `mame_documents` | build, debug, mameconfig |
| `mame_machines` | sourcefile, description, year?, manufacturer?, isbios, isdevice, ismechanical, runnable |
| `mame_machine_links` | cloneof/romof/sampleof declarations as distinct source fields, target literal and assertion identity |
| `mame_device_references` | target name, required tag, source assertion identity |
| `mame_bios_sets` | name, description, default |
| `mame_roms` | claim ID; name, size, CRC, SHA-1, BIOS?, merge?, region?, offset?, status, optional |
| `mame_disks` | claim ID; name, SHA-1?, merge?, region?, index?, writable, status, optional |
| `mame_samples` | claim ID, declared name; audio-sample claims have unknown size/digests unless the source supplies them |
| `mame_chips` | name, tag?, CPU/audio kind, clock? |
| `mame_displays` | tag?, kind, rotation?, flipx, width?, height?, refresh, pixclock?, htotal?, hbend?, hbstart?, vtotal?, vbend?, vbstart? |
| `mame_sound` | channels; zero or one per machine |
| `mame_inputs`, `mame_input_controls` | input service/tilt/players/coins?; ordered controls with type, player?, buttons?, minimum?, maximum?, sensitivity?, keydelta?, reverse, ways?, ways2?, ways3? |
| `mame_switches` | DIP/configuration kind, name, tag, mask; integer switch identity |
| `mame_switch_locations`, `mame_switch_values` | location name/number/inverted; value name/value/default; ordered children with the native DIP/configuration distinction preserved |
| `mame_switch_conditions`, `mame_switch_value_conditions` | tag, mask, comparison enum, value; zero or one for each owner |
| `mame_ports`, `mame_analogs` | port tag; ordered analog masks |
| `mame_adjusters`, `mame_adjuster_conditions` | adjuster name/default; optional tag/mask/comparison/value condition |
| `mame_drivers` | status, emulation, cocktail?, savestate, requiresartwork, unofficial, nosoundhardware, incomplete |
| `mame_features` | feature type, status?, overall? |
| `mame_devices`, `mame_device_instances`, `mame_device_extensions` | device type/tag?/fixed_image?/mandatory?/interface?; optional instance name/briefname; ordered extension names |
| `mame_slots`, `mame_slot_options` | slot name; ordered name/devname/default options |
| `mame_softwarelist_references` | tag, name, original/compatible status, filter? |
| `mame_ram_options` | name, optional default attribute, PCDATA value |

Historical accepted attributes such as soundonly/dispose and alternate disk
spellings belong to an explicitly identified compatible dialect. They must not
be falsely attributed to the 0.289 DTD.

Keep native element order with an ordinal on each family. If cross-family order
is queried, a `UNION ALL` view joins those ordinals; avoid a redundant, globally
registered node row for each element. Conditions use typed owner tables so
foreign keys cannot point a switch condition at an unrelated display.

## MAME software-list native relations

Pin the upstream 0.289 `hash/softwarelist.dtd`, independently from machine XML.
The accepted plural `<softwarelists>` wrapper is an application compatibility
dialect, not the canonical DTD root.

| Native relation | Fields and nesting |
|---|---|
| `software_lists` | name, description?, notes?; provides a record namespace |
| `software_items` | native record FK, cloneof?, supported, description, year, publisher, notes? |
| `software_item_info`, `software_shared_features` | item FK, occurrence order, declared name, optional value |
| `software_parts` | item FK, order, name, interface |
| `software_part_features` | part FK, order, declared name, optional value |
| `software_data_areas` | part FK, order, name, declared size, width, endianness |
| `software_disk_areas` | part FK, order, name |
| `software_rom_entries` | area FK, operation order, optional name/size/CRC/SHA-1/offset/value/loadflag, status |
| `software_file_declarations` | claim FK, unique declaring-entry FK; identifies the native entry that declares a file without copying its name/hashes |
| `software_rom_file_uses` | entry FK, file-declaration FK, load/continue/reload/ignore use kind; several ordered entries may refer to one file |
| `software_disk_entries` | area FK, order, claim FK, name, SHA-1?, status, writeable |
| `software_part_switches`, `software_part_switch_values` | part FK; switch name/tag/mask; ordered value name/value/default |

`info`, `sharedfeat`, and `feature` are intentionally named-value records in this
specification. Their own typed, owner-constrained tables preserve repeated names,
NULL optional values and explicit empty values; they do not introduce a generic
catalog EAV storage mechanism.

Defaults include supported=yes, width=8, endianness=little, dump status=good,
writeable=no and DIP value default=no. Preserve omitted versus explicit defaults
using per-entity presence information, not fabricated source locations.

For file requirements, distinguish file declarations from load operations.
Reload/continue/fill/ignore remain ordered native entries. A continue segment's
length is not automatically the complete file size. Preserve unresolved or
underspecified operations as source facts; validated recipe types gate any
assembly. The real NES `10yardj1` entry supplies a named 16 KiB ROM followed by an
unnamed reload at offset `0x4000`, demonstrating why every `<rom>` cannot become
a new required file.

Every XML ROM entry, including an unusual combination of name/hash/loadflag,
has one `software_rom_entries` row that owns its original attributes. A separate
file-declaration row points at its declaring entry and supplies the FK identity
for common claims. Ordered use rows connect an initial load and its continuation,
reload or ignore entries to that declaration. A fill has no file declaration.
These relationships interpret source facts and never justify discarding a
DTD-valid source entry.

File-claim construction requires documented file-declaration semantics, a
consistent area-local chain and checked ownership. Attribute-free or otherwise
unclassifiable entries remain queryable with an explicit unclassified state;
they do not fabricate filename or byte-length requirements. The native
declaring entry is the source owner of the file's declared hashes; their values
are stored once in `digest_values` and linked from that occurrence by
`occurrence_digest_assertions`. Any additional digest on a later operation
remains that operation's source evidence until its scope is proved. The complete
expected file length is a derived query value only when
the pinned loading rules and complete chain determine it; otherwise it is
unknown. Segment sizes/offsets remain native values. Reload/fill lengths must
not be naively summed into physical file size. Tests cover load+continue,
load+reload, ignored bytes, fill, conflicting declarations and an unresolved
chain. No derived full-file hash or size is asserted as a source declaration.

## Logiqx and ClrMamePro native relations

Logiqx and ClrMamePro overlap semantically, but have distinct declared syntax,
headers, flags, and supported dialects. The complete pinned specification
inventory must cover more than the currently parsed field subset.

The baseline Logiqx contract is DTD revision 1.5, dated 2008-10-28, from the
producer's repository at commit
`1575f8da6706e159b51e3bb8b511f546909927cc` (DTD blob
`ab86446ee415761077dcec602c0a54cf0088d84d`). It allows an optional header followed
by one or more games. When present, the header requires name, description,
version and author in its declared sequence; each game requires description.
The default compatibility parser permits headerless catalogs and sparse native
values; it is not a strict validator of the DTD sequence and requiredness.

| Native Logiqx relation | Declared fields/cardinality |
|---|---|
| `logiqx_documents` | build?, debug default no |
| `logiqx_headers` | zero or one; name, description, category?, version, date?, author, email?, homepage?, URL?, comment? in DTD order |
| `logiqx_clrmamepro_options` | optional header singleton; header-definition filename?, forcemerging default split, forcenodump default obsolete, forcepacking default zip |
| `logiqx_romcenter_options` | optional header singleton; plugin?, rommode/biosmode default split, samplemode default merged, lockrommode/lockbiosmode/locksamplemode default no |
| `logiqx_games` | source name from record identity; sourcefile?, isbios default no, cloneof?/romof?/sampleof? declaration owners, board?, rebuildto?, description, year?, manufacturer? |
| `logiqx_game_comments` | ordered repeated PCDATA comments |
| `logiqx_releases` | repeated name/region, language?, date?, default no |
| `logiqx_bios_sets` | repeated name/description/default no |
| `logiqx_roms` | claim ID; required name/size, CRC?/SHA-1?/MD5?, merge?, date?, status default good including verified enum value |
| `logiqx_disks` | claim ID; required name, SHA-1?/MD5?, merge?, status default good including verified |
| `logiqx_samples` | claim ID; repeated required name, audio-sample role, unknown size/digests |
| `logiqx_archives` | repeated required archive name; named container references, distinct from contained file claims |

Each game sequence preserves comments, description, year?, manufacturer?, then
release/BIOS/ROM/disk/sample/archive families in the declared order. Fixed option
enums are checked against the DTD; no app policy silently rewrites the source
settings. Current accepted file-name/SHA-1 metadata, ROM serial fields and
device references are outside this pinned DTD and belong to an explicitly named
compatible dialect if retained as supported input. The implementation now has
native releases, BIOS sets, disk/sample/archive and option blocks, with
explicit-versus-default presence. Complete strict grammar and field-witness
coverage remain open; these tables alone do not establish DTD conformance.

The default interpretation is named `logiqx-declared-text-compat-v1`. ROM size,
CRC, MD5 and SHA-1, and disk MD5/SHA-1 retain their declared text on the native
claim. Missing, empty and uninterpretable values remain distinct. Numeric size
is a virtual projection only for ASCII decimal values within SQLite's signed
integer range; usable digests require exact-width ASCII hex. Derived state
views classify declarations without storing another copy. Any uninterpretable
ROM size/digest prevents shared-file UUID assignment before registry mutation;
independently usable assertions remain queryable. Disk-data evidence cannot
assign a whole-container UUID, including through direct publication writes.

`logiqx_header_text_positions` and `logiqx_game_text_positions` hold closed field
codes, source order and location only; scalar values stay in their native fixed
columns. Publication requires exactly one position for every present scalar,
including empty text. History ranks these fields with native option/media
children, ignoring vendor-only gaps but detecting cross-family order changes.
PCDATA boundary whitespace is preserved rather than trimmed. Originals remain
external; none of these relations store XML documents or catch-all values.

ClrMamePro's own documentation defines listinfo tags and examples, not a complete
DTD-equivalent grammar. Its documented tag order/case rules differ from XML.
Use these native tables with a pinned, explicit supported text grammar:

| Native CMP relation | Documented fields/structures |
|---|---|
| `cmp_header_facts` | name, description, version, author, comment; witnessed homepage and explicitly accepted category/date/email/URL compatibility values; source block, ordinal and location |
| `cmp_header_directives` | header-definition filename, forcemerging, forcezipping, forcenodump; exact source spelling and explicitness |
| `cmp_set_facts`, `clrmamepro_set_links` | numeric set owner; catalog-owned name, cloneof literal, description/year/manufacturer/rebuildto; witnessed region/release-date-component/set-serial text; original game/set spelling and document ordinal |
| `cmp_roms` | claim ID; name, size?, CRC/CRC32 alias, MD5?, SHA-1?, declared nodump/baddump flags; explicitly supported merge/status dialect fields |
| `cmp_samples`, `cmp_sample_parent_links` | one occurrence-keyed scalar sample declaration with unknown size/digests; separate set-owned sampleof literal |

`forcezipping` in documented text and Logiqx's `forcepacking` are distinct source
fields. BIOS/disk/resource engine aliases are documented transformation
vocabulary, not sufficient proof of a text block grammar; require a pinned
shape/specimen before declaring those forms supported. Real corpus region,
releaseyear/releasemonth/releaseday and set serial are supported under the named
compatibility contract, not presented as published listinfo fields. Leading
zeros and empty date components stay text, without invented calendar validation.
The published forcenodump default is obsolete; other undocumented defaults and
multiplicity rules are not inferred.
The current `clrmamepro-declared-text-compat-v1` implementation stores ROM name,
size text, both CRC/CRC32 declarations, MD5/SHA-1 text, merge/date/serial/status
and independent nodump/baddump presence in `cmp_rom_claims`. Numeric size and
effective dump status are virtual interpretations, not stored copies. Size is
nonempty ASCII decimal within signed 64-bit range; hashes are exact-width ASCII
hex. Duplicate singleton fields/flags and missing names are rejected by this
adapter. These rules are named compatibility choices, not an inferred complete
CMP specification. Unequal CRC aliases or competing dump markers remain native
query data but bypass UUID resolution. Independently valid hash assertions
remain queryable; singular root consumers use the derived, scope/provenance-
qualified `usable_occurrence_digest_assertions` view rather than picking an
arbitrary assertion.

`cmp_rom_field_positions` stores twelve closed native field codes with source
keyword spelling, order, quote state and location; it does not store values.
The old partial `cmp_rom_facts` copy is removed. Publication requires a native
claim for every CMP ROM occurrence and a position for every present declaration.
Native raw hashes and normalized source assertions must agree in both directions;
computed evidence cannot stand in for a source field. Linked UUIDs require an
unambiguous matching source SHA-1 in the native whole-file scope and no native
declaration conflicts. Native claims and positions cannot be appended after
publication. History keeps raw spelling and relative native-field order while
ignoring vendor-only gaps and reindentation.

Header values and directives have fixed native columns with fifteen closed
`cmp_header_field_positions` codes. Set scalar positions use twelve closed
`cmp_set_field_positions` codes, including name and separate cloneof/sampleof
literals. Positions never copy values. A mandatory `cmp_documents` row records
header presence and parsed lexical comment count; `cmp_comments` owns ordered
semicolon tokens and locations independently of the header's comment value.
`cmp_set_rom_positions` orders ROM forms alongside scalar fields and repeated
samples. Publication requires present-value position coverage, matching native
owners, declared comment count with contiguous comment ordinals and unique
native document/set ordinals. These checks enforce the importer's declaration,
not reconstruction of omitted input from a direct-SQL caller's false count.
All native facts are immutable, and publication rejects late inserts. Insert
guards reject conflicting primary keys and unique field-order keys even when
REPLACE deletion triggers are disabled, including unpublished draft rows. Effective
merging/zipping/nodump modes derive from declared values: missing forcenodump
means obsolete, while explicit unknown values disable the mode. Forcepacking
remains an independent named compatibility value, never an alias for forcezipping.
History compares structural document layout separately from owner fact multisets:
header crossings remain visible even with repeated names, but indistinguishable
repeated owners do not gain invented cross-snapshot ordinal identity. Native
scalar samples now use occurrence IDs rather than set/sample-order keys.
`cmp_samples` owns name, keyword spelling, quotation, source order and location;
set ownership and mixed-media order come only from `asset_occurrences`. Repeated
and empty sample names remain distinct entries. Their bulk API results include
native name/location and source/list/set provenance without assigning a UUID,
size or source-declared digest. Sample-only order for history is derived, not
stored again. Publication requires every sample payload and mixed ROM/sample
order matching the native source layout; draft media PK and set/order conflicts
abort before REPLACE handling. Samples remain outside ROM requirements. The
importer's native payload is one enum, replacing independently optional format
payloads. Complete format query witnesses remain unfinished.

A fresh import of the ignored authentic Atari-2600 corpus on 2026-10-01 retained
905 sets and 905 ROM occurrences, all 4,526 present ROM field positions and
2,715 normalized digest assertions. Six repeated set-name groups remain
separate native owners; there are no missing ROM field positions. All 905
eligible occurrences linked to expected-file UUIDs. SQLite integrity and
foreign-key checks passed. The 188,849-byte original remains an external source
object. The original ROM milestone did not accept its set-level region,
release-date components or serial fields; the later document/set milestone
adds those native field owners and provenance. Full format/query/performance
acceptance is still separate.

Preserve quoted strings and valid flags as typed values; CMP descriptions/year/
manufacturer no longer live in `metadata_json`. TOSEC naming conventions and
Redump publisher identity are adapter rules over their actual wire dialect, not
substitute XML grammars.

PureDOS declares a Logiqx DTD while adding developer/comments/link and nested
ROM source/track/patch structures. Those are observed dialect fields, not proof
that baseline Logiqx defines them. A supported PureDOS dialect would add explicit
`puredos_game_links`, `puredos_rom_sources`, `puredos_tracks`, and
`puredos_patches` with the observed typed fields and owner/order constraints.
Its `data` attribute and source/track hash scopes need producer semantics before
they can enter generic file matching. A shallow import must report partial
coverage until that adapter is specified.

## No-Intro exports and OfflineList

There are now three separate No-Intro shaped inputs to account for: flat DAT
XML using the No-Intro schema v3/v4, nested database-export XML, and the
repository's current synthetic P/C projection. They are not interchangeable
just because each contains `<game>` records.

### Flat No-Intro DAT XML

Use narrow relations for the document/header; ordered header directives;
games; repeated game identifiers; categories; ROM declarations; releases; and
parent declarations. In v3, game `id`/`cloneofid` and name-based `cloneof` have
different identity semantics; nested `<game_id>` values are additional IDs,
not duplicate archive names. Preserve zero-game documents, multiple ROMs per
game, optional status/serial/date/header/SHA-256 fields, and repeated release
rows. The PC pack alone has 52,473 release records. Resolve only a target key
whose scope is established for that schema revision; keep unresolved parent
literals queryable.

Both producer XSDs now supply their required/defaulted contracts. The v4
contract adds header `trademarks` and `piracy`; observed `comment`, nested
`game_id`, ROM `mia`/`date`, multiple ROMs, omitted required fields and larger
sizes belong to explicit compatibility interpretations, not relaxed strict
validation. Digests and status are `xs:string`: an empty or malformed digest
can conform to that datatype without becoming usable matching evidence.
The refreshed streaming audit counted 371,312 games, 1,976,847 ROMs and 52,473
releases across all 664 files without XML parse failures.

The native adapter streams one game at a time in one XML traversal. A private
validated-EOF result gates publication of the pending edition. Native ROM hash
fields identify CRC/MD5/SHA-1/SHA-256 with closed field codes and own either an
interned digest reference or an uninterpretable literal, never both. Valid hash
payloads are stored once in the digest dictionary; query text derives canonical
lowercase hexadecimal, while exact producer spelling remains in the external
original. Invalid/empty declarations and missing attributes stay distinct.
Usable independent assertions survive a malformed companion field, but any
supplied uninterpretable size/hash prevents a shared file UUID. Global header
filters and per-ROM header declarations make evidence scope unknown even when
the field is empty; filenames and extensions do not prove whole-file scope.

### No-Intro database-export XML

Model each export game, its archive metadata, optional repeated game releases,
and zero or more repeated source-history records. A source and a release can
each own details, serials, and file claims; do not force release fields under a
source or assume every game has a source. Preserve schema cardinalities rather
than promoting observed counts to constraints. Archive identity, source
revision identity, source provenance, serials, and file occurrences must remain
distinct. Numeric file IDs can repeat under different owners, so they are not
unique occurrence keys; preserve owner and order, and deduplicate only through
an explicit verified identity relation. A `<file>` row's `crc32` and SHA-256 are
not the same source fields as a flat DAT's `crc` and SHA-256, even where a
normalized digest projection can expose equivalent algorithms.

The Nintendo example has 408 games, one archive per game, 1,546 sources and
1,579 files. Other exports include games with only an archive and zero sources.
All archive `clone` values are either the `P` parent marker or a
numeric archive reference in that file; `regparent` is a separate text value.
Source IDs and archive numbers must not become cross-snapshot keys without a
verified scope contract. The large export directory contains at least 115,527
games, with one file excluded from that lower bound by a streaming parse error.
Keep the one-root and sibling-header/datafile envelope forms explicit in the
document interpretation, and isolate/report malformed input without dropping
valid records from unrelated files.

### Synthetic P/C projection and OfflineList

The existing No-Intro adapter accepts a synthetic `<datafile>/<game>/<rom>`
projection. Its persistent typed facts now cover archive ID, description,
alternate name, region, ordered languages, version, BIOS and distinct clone
and merge tokens beyond asset claims. Actual flat DAT
files use attributes and nested releases/game IDs that do not fit that
projection, and DB exports use nested archive/source/file records instead.

Design native No-Intro archive, ordered language, file, and source relations.
Archive IDs are snapshot/source-scoped identifiers, not global game identity.
`clone="P"` is a marker; numeric clone values are archive references;
`mergeof` is a distinct source declaration. Do not manufacture parent names or
infer exact merge semantics from those tokens.

The published archive vocabulary also includes development/additional/special
status, licensed/BIOS/display-language flags, distribution/physical/public/DAT
flags, regional parent, game ID and notes. Published file/source conventions
describe further hashes (including SHA-256), format/filename overrides, item,
extension, serial/bad/unique/merge fields and dumping/media provenance. Some
references are observational and lack exact XML names or nesting. These are
candidate explicit columns/child tables, not a claim of verified wire coverage.
Require an authentic production export and a versioned dialect contract before
claiming P/C XML conformance; UI defaults are not XML DTD defaults.

OfflineList receives its own proposed adapter and relations if it is added to
supported imports: document configuration, field-display configuration,
download/search instructions, and ordered game records. The observed game
fields are imageNumber, releaseNumber, title, saveType, romSize, publisher,
location, sourceRom, language, files/romCRC, im1CRC, im2CRC, comment, duplicateID.
Separate ROM CRCs from screenshot/image CRCs. The referenced `datas.xsd` is
missing from the downloaded corpus. Treat that missing specification as a
coverage gap, not evidence that the existing No-Intro parser handles this file.

## Common query contract and physical ownership

The authoritative owner of each native field is exactly one native table.
`asset_occurrences` supplies source-scoped identity for each file/media claim
and digest-bearing source operation. Format-native tables own the relevant
name/size/status or operation facts once. The normalized
`occurrence_digest_assertions` relation is the sole owner of common digest
assertions, keyed through interned algorithm/value bytes with scope and
provenance on each occurrence link.
`catalog_contents` supplies a separately qualified cross-catalog whole-file
identity, addressed by its persistent binary UUID, when the evidence supports
one. A `catalog_file_claims` projection returns one row per source occurrence
and its optional shared content identity. Digests are queried as a separate
one-to-many child relation, so a join does not multiply occurrence rows. It
never includes an
instruction-only fill/reload as another file and never equates a track digest,
CHD header SHA-1, logical disk digest, and whole-container digest. APIs,
history, and manifests remain snapshot/occurrence-scoped and return source
provenance even when exposing the shared identity; global content lookup returns
the identity plus paginated occurrences, never a list-specific declaration as
if it were global metadata. Existing relationship endpoints using the current
algorithm-plus-digest key migrate to a UUID only when an unambiguous whole-file
alias identifies one; otherwise their evidence remains unresolved with no
invented scope or target. Observed-byte identities remain a separate typed
endpoint (`ObservedContentDigest` with whole-file scope and verified
algorithm/value), never rewritten as catalog-assertion UUIDs. Source-native query rows join a single authoritative
digest-assertion owner rather than repeating the same digest in both native
payloads and a generic digest table.

Specify the persistent identity's exact key and digest-scope policy together;
the existing Rust `ContentIdentity` (algorithm plus digest) is insufficient to
represent differently scoped evidence. Existing relationship endpoints using
that type must migrate to a UUID only when an unambiguous whole-file alias maps
to one identity; unscoped or ambiguous legacy endpoints remain unresolved with
their original evidence rather than receiving an invented scope or UUID. Query
common cases in bulk: occurrence
to content identity, content identity to paginated occurrences, and candidate
matching from incomplete evidence. Measure their cardinalities, query plans,
and justified indexes in the import corpus so deduplication does not introduce
per-occurrence lookup loops or oversized redundant indexes.

Native sample declarations are expected audio-media claims with a `ClaimId`,
unknown size and unknown digests. This includes MAME and Logiqx sample names and
CMP scalar samples. A source sample name may be a logical audio name rather
than a complete filesystem filename; target-profile rules decide admissible
representations without inventing an extension during import. Common queries
expose their weak/unknown evidence instead of omitting samples or declaring
exact content identity. Archive names remain typed container-reference facts,
not fabricated size/hash claims for their contents.

Common APIs cover record enumeration/selection, expected file/media claims,
source dependencies/merges, relationship explanations and adjudication, scoped
snapshot diffs, catalog reconciliation, and pinned manifests. Namespaces and
integer FK joins replace concatenated/JSON composite endpoint strings. Native
fields remain individually queryable even if a generic query does not expose
them. Descriptive values only share a physical column when their source meaning
and cardinality truly agree; manufacturer and publisher are not silently
collapsed into one canonical organization assertion.

| Existing query | Required redesign behavior |
|---|---|
| Scoped history/diffs | Keep selected-but-absent scope names as names, not only FKs to records that happened to import. Removal requires coverage in both snapshots; location-only changes are excluded from semantic fact comparison. |
| Reconciliation | Require published snapshots; preserve claim roles and digest scopes, native software hierarchy, area kind and every relevant ordinal. |
| Machine dependencies | Keep clone, ROM-parent, device and sample declarations distinct; current resolver capabilities do not turn unsupported sample edges into resolved dependencies. |
| Relationship explanations/reviews | Preserve source provenance, derivation rules, support ordering, latest review and append-only history, review notes and supersession targets. |
| Pinned manifests/recipes | Preserve external snapshot/record references and canonical order; storage ID changes do not redefine a recipe or its verification. |
| Inventory/retrieval | Keep expected catalog claims separate from observed bytes, locations, archive member selectors, scan provenance and freshness. |
| Backup/restore | Update exact schema validation and durable/rebuildable classification; retain and verify the matching source-object directory. |

Versioned JSON interchange for CLI reports/manifests is independent of the
no-JSON SQLite contract. This redesign does not require changing those public
serialization envelopes or adding persistent layout tables without a consumer.

Source assertions keep compact identity and authoritative source-native facts.
Derived/user assertions have distinct tables with explicit record/asset/content/
external endpoint FKs and rule/evidence/support relations. A source declaration
does not require a second copy of its snapshot name, subjects, targets, or raw
payload merely to explain it. Reviews and supersession point at assertion
identity, and unresolved/ambiguous source targets remain queryable literals.

Every reviewable source declaration uses `assertion_id UNIQUE NOT NULL` as an
FK to a source-origin assertion identity. The identity records its closed native
declaration kind and snapshot FK; that kind must agree with exactly one owning
native row. Owner identity/field/occurrence is unique. Source targets may be
unresolved literals; resolution is separate from the existence of the source
declaration. Snapshot consistency follows the subject's native record and is
checked before publication, including any optional resolved target.

| Source declaration owner | Owned declaration and assertion link |
|---|---|
| `mame_machine_links`, `mame_device_references` | cloneof/romof/sampleof or device_ref, including the reference tag |
| `mame_rom_merges`, `mame_disk_merges` | owning claim FK and declared merge name; no second merge-name copy in the asset payload |
| `software_clone_links` | item FK and declared cloneof name; optional singleton |
| `logiqx_record_links`, `logiqx_asset_merges` | native record/claim FK and distinct cloneof/romof/sampleof/device/merge source field |
| `cmp_record_links`, `cmp_asset_merges` | native record/claim FK and documented parent/runtime/sample/merge declarations |
| `no_intro_archive_links`, `no_intro_file_merge_links` | declared archive-ID or merge token with its own known/unknown semantics; a parent marker remains a marker fact, not a fabricated target edge |

The attribute value listed in a native family inventory is stored in this
declaration owner when it is reviewable, and is exposed on that family's query
view by joining. It is not also copied into the owning machine/item/ROM row.
Derived/user assertion subtypes carry explicit endpoint FKs, rule/evidence and
ordered support. Publication rejects assertion identities missing a native
owner, assigned to multiple owners, of the wrong origin/kind, or attached to a
different snapshot. Source tables and identities are immutable after publication.

For history correspondence, occurrence ordinals identify a row only inside an
immutable snapshot. Prefer a publisher-declared stable key only when its pinned
dialect establishes its scope and continuity semantics. Otherwise unique
qualified names establish a match only when they are unique in both selected
snapshots. Duplicate-name groups are compared as fact multisets and may expose
unique exact fact matches, but ambiguous changed occurrences produce an explicit
ambiguous-correspondence result. Do not invent continuity, rename, or removal by
matching ordinal positions after an insertion/reorder. Reconciliation can
compare independent content evidence without asserting record continuity.
Reviews and manifests stay pinned to their original snapshot/source occurrence;
carrying a conclusion to another snapshot requires an explicit validated
association or supersession, not automatic name/position retargeting.

SQLite and every source object referenced by its captured state form one logical
backup unit. Capture the database first, enumerate references from that captured
database rather than a changing live connection, and verify each object's codec,
decompressed length and digest before reporting a complete backup. Restore into
staging, verify schema/foreign keys/publications and every captured object, and
reject a missing or mismatched object before publishing the restored database.
Finalization must publish verified objects before their database references;
it must not claim cross-filesystem atomicity that the platform cannot provide.
Source objects cannot be classified as rebuildable scan-cache data.
When replacing an existing restored database, publish verified content-addressed
objects append-only into the destination object store and retain every object
referenced by the old database until the new database is durable. Never replace
the entire object directory while the old database can still reference it.
Reclaim unreferenced objects only as a separate, explicit operation after the
replacement is durable. A crash during object publication leaves the old backup
unit usable; a crash after database replacement leaves the new references valid.

Hash columns use fixed-length binary values and checked scope/algorithm codes.
Closed format enums have stable documented storage codes and Rust enums.
Repeated free-text labels may be interned only after measuring repetition and
query cost across the corpus. Do not require every short string to use a
dictionary or claim a size win before a fresh import proves it.

Absent, explicitly empty, defaulted, and explicitly specified values remain
distinct when the source specification distinguishes them. PCDATA/CDATAs such
as year, mask, clock and refresh are not narrowed to arbitrary numeric domains
solely because the representative source happens to contain numbers. Validated
numeric views/newtypes can serve operations without losing the source fact.
For example, an optional digest that is explicitly empty uses presence state
alongside the nullable binary digest; it is distinguishable from an absent
attribute without storing a second copy of every valid hex digest. Presence
bits have documented field meanings and are decoded through native Rust enums.

## Rust boundaries and import state

Use `SnapshotId`, `RecordId`, `MachineId`, `SoftwareItemId`, `PartId`, `AreaId`,
`ClaimId`, and `AssertionId` newtypes; avoid interchangeable `i64` IDs across
writers. Keep native record enums and structs rather than an optional-field
`SnapshotSet` union. Share storage/iteration helpers for identical operations,
not semantics by calling unrelated records the same kind of game or ROM.

An `ImportSession<Parsing>` consumes one native record at a time. EOF and native
grammar validation produce `ImportSession<Validated>`; successful source-object
verification and transaction completion produce `PublishedSnapshot`. An enum
distinguishes syntax, dialect/field validation, storage, and source-object errors.
Invalid or partial streams cannot reach publication. Existing validated media
recipe typestates continue to gate assembly and verification.

## Implementation and verification sequence

1. Pin every supported format/dialect grammar and make a field-to-column/child
   coverage ledger, with required/optional/repeated/default rules. Label
   synthetic samples and unsupported production dialects honestly.
2. Implement shared compact identities—including persistent 16-byte content
   UUID BLOBs, source-attributed digest aliases and ordered occurrences—and
   source/snapshot/scope/option tables. Replace JSON metadata and endpoint
   persistence with the native typed owners.
3. Replace the MAME wide union with narrow native tables and compact parents;
   migrate software-list hierarchy and load records as one coherent change.
4. Complete Logiqx/TOSEC and ClrMamePro standard field coverage; add flat
   No-Intro DAT v3/v4 and database-export XML dialects separately from the
   synthetic P/C projection and OfflineList. Track MAMEC-56 through MAMEC-61
   against this design, without reviving archived plan 2.
5. Switch source explanations, reviews, history, reconciliation, and manifests
   to the common typed query contract. Remove superseded compatibility copies.
6. Reimport fresh databases across the entire metadata corpus. Compare each
   native field, defaults, ordering and ownership, not only record counts or a
   success exit code. Run the complete repository `devenv test` gate.

Use schema-backed minimal witnesses for every declared field family, including
valid empty values, duplicate name/value entries, repeated same-named areas,
missing optional attributes, explicit defaults, out-of-order references, and
load-only ROM entries. Add cross-format witnesses proving equal eligible
whole-file SHA-1/SHA-256 assertions share one content identity across catalogs while retaining
separate ordered/source-attributed occurrences; same filenames with different
digests do not merge; CRC-only, missing-digest, scoped-digest and conflicting
size/digest claims do not merge unsafely. Test the SHA-1-only, SHA-256-only,
then dual-digest bridge sequence in both import orders: the bridge to two
existing UUIDs remains an explicit unresolved candidate until reviewed, and a
reviewed merge redirects aliases without rewriting old UUIDs. UUID
binary/undashed-hex conversion, stable reimport, and backup/restore preserve
the same UUID; every occurrence assertion, field presence/default, source byte
stream and order still round-trips. Test
ambiguous target handling, digest-scope separation, idempotence, failed-EOF
rollback, snapshot diffs, review/supersession stability, exact source recovery
and backup/restore. Application tests should assert query facts and
relationships, not mirror private insert statements.

Add No-Intro regression witnesses for six NULs across source and release
details recovering after decoding; exact original-source bytes/hash retained;
all six diagnostics carrying original one-based coordinates; sibling-root
framing; and rejection/no publication for a different XML-forbidden character.
Add strict-v3 versus observed-compatibility witnesses for one valid record and
the corpus deviations (multiple ROMs, missing required digests, nested
`game_id`, and a size above `u32::MAX`). Add database-export ownership witnesses
for zero sources, release-owned details/serials/files, and repeated numeric
file IDs under distinct owners without occurrence collapse. Verify recovered
imports still require valid EOF and a successful transaction before
publication.

Record SQLite table/index bytes, native row counts, import time, peak heap/RSS
and query plans per format. Require justified indexes for real queries and
verify union projection plans. Size/profile comparisons never excuse missing
specification fields or a misidentified input dialect.

## Primary references and local evidence

- MAME 0.289 embedded machine DTD and source document:
  `target/profiling/external-catalogs-2026-09-29/mame-full-test/mame-listxml/mame0289.xml`.
- [MAME 0.289 software-list DTD](https://github.com/mamedev/mame/blob/mame0289/hash/softwarelist.dtd).
- [MAME software-list interpretation code](https://github.com/mamedev/mame/blob/mame0289/src/emu/softlist.cpp).
- [Pinned Logiqx revision 1.5 DTD](https://github.com/Logiqx/logiqx-dev/blob/1575f8da6706e159b51e3bb8b511f546909927cc/DatLib/datafile.dtd).
- [ClrMamePro DAT documentation](https://mamedev.emulab.it/clrmamepro/docs/htm/datfile.htm).
- [No-Intro naming convention](https://wiki.no-intro.org/index.php?title=Naming_Convention),
  [file convention](https://wiki.no-intro.org/index.php?title=File_Convention),
  [source convention](https://wiki.no-intro.org/index.php?title=Source_Convention).
- [No-Intro DAT schema v3](https://datomatic.no-intro.org/stuff/schema_nointro_datfile_v3.xsd)
  and [v4](https://datomatic.no-intro.org/stuff/schema_nointro_datfile_v4.xsd),
  both retrieved from the producer on 2026-10-01.
- User-provided No-Intro DAT/PC, TOSEC, and database-export corpus:
  `local/catalog-data/2026-09-30` (git-excluded, never imported in this audit).
- `fixtures/catalog/manifest.json` and `docs/catalog-format-assessment.md` state
  the synthetic fixture limits.
- sem inspected native MAME enums/structs, `SnapshotSet`/`SnapshotAsset`, and
  software-list persistence. Absolute checkout paths were necessary when bare
  entity lookup failed.
