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
queries. Reviewed conflict outcomes and UUID redirects now have an atomic typed
storage API; remaining per-format coverage and observed-byte endpoints are
unfinished. This is not a claim of complete
DTD conformance.

### Reviewed shared-file decisions

Source conflict identity remains its exact `(occurrence_id, candidate UUID)` key,
not a copied hash tuple or a source name. `file_match_decisions` owns the rationale,
timestamp and closed keep-separate/merge action; `file_match_decision_conflicts`
permits one terminal outcome per source conflict. Hash and size disposition rows
reference the exact immutable conflict-evidence keys without copying values.
`file_match_decision_publications` activates the complete reviewed action and
freezes every decision child. The application creates and consumes a draft in
one immediate transaction; it does not expose durable draft editing.

`merged_file_ids` preserves old issued IDs and points them to the kept ID in the
same registry. Redirects require actual reviewed source-conflict bridges and
cannot self-link or cycle. Merge publication checks retained component facts,
including prior redirect ancestors and undispositioned evidence. Only a specific
published rejection excludes its underlying hash or qualified native size;
another accept cannot restore it. Settling one conflict never settles another.
Incoming entries remain unlinked: reviewing their evidence is not a new source
declaration or observed-byte proof.

Bulk occurrence results expose immutable source-issued and derived canonical
UUIDs separately. Keyset pages accept any issued ID, seek its reverse redirect
component, and pin the requested ID, registry generation and review revision.
Any later review requires restarting the cursor, preventing omissions when a
merge adds earlier occurrence IDs. The corpus service uses its frozen earlier
schema/executable; its preserved database is not upgraded by this source change.

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

### Native software metadata queries

`catalog_software::lists_for_snapshot` pages an exact published software
document's native lists. `titles_for_list` pages that snapshot's numeric list
owner and returns each selected title's scalar facts and positions, info,
shared features, parts, part features, DIP switches/values and data/disk areas.
Source names are values, not identities; repeated lists, titles, parts and
areas remain separate rows. Metadata-only titles and empty plural envelopes
are queryable without any file occurrence. `SoftwareEnvelope` distinguishes
single lists from plural envelopes with absent or empty build text, and
`SoftwareAreaFields` prevents invented data-area fields on disk areas.

These transient Rust results read existing authoritative tables, not catalog
JSON or another persisted projection. Each area carries occurrence IDs that
the native ROM/disk `catalog_files` reader resolves; it does not copy those
payloads into another table. Page-scoped provenance owns source/catalog,
original document, parser interpretation, registry generation and snapshot
keys once. Cursors pin the exact snapshot/generation and title-list owner;
historical published editions stay visible. Queries batch the selected owners
inside one read transaction. Page size bounds lists/titles, not the retained
children of each title; no silent child truncation is permitted. Source-order
gaps from vendor elements are preserved separately from each family's order.
Lexical attribute positions and approved executable loader interpretation
remain required for complete format acceptance.

| Input | Native structure and facts | Current implementation gap |
|---|---|---|
| MAME machine XML | Machine -> ROM/disk/sample, chips, displays, input/controls, switches/values/conditions, ports, devices, slots, driver/features, software-list references and RAM options. | Native families and numeric owners replace the wide stored union; complete specification presence/default and field witnesses remain open. |
| MAME software-list XML | List -> item -> part -> data/disk area -> ordered ROM/disk entries. File declarations and load/continue/reload/ignore uses are separate relations; fill has no file claim. | Numeric owners, separate declarations/operations and area-local loading chains exist; complete per-flag loading rules and native field witnesses remain open. |
| Logiqx XML / TOSEC | Document/header/options -> game -> comments, releases, BIOS sets, ROM/disk/sample claims, archive references and distinct parent declarations. | Native families, explicit defaults, declared size/hash text and scalar positions exist. Parent/device/media-merge declarations use native owners and shared reported identities. Complete pinned-DTD grammar, field/query and corpus acceptance remains open. |
| ClrMamePro text | Header/directives -> set -> ROM claims and scalar sample claims; native flags and sample-parent declarations. | Native CMP facts/directives and numeric set/occurrence owners exist; remaining specification fields and complete query witnesses remain open. |
| No-Intro flat DAT v3/v4 | Document/header/directives -> game -> scoped identifiers, categories, releases and ROM declarations; name-based and ID-based parents stay distinct. | Separate v3/v4 strict and observed-compatible interpretations stream into native typed owners. Raw size text, interned valid hashes, typed invalid hash literals, SHA-256 assertions, options, field positions and repeated children are retained. Full corpus/query/performance acceptance remains open; this is not a Logiqx envelope import or database-export coverage. |
| No-Intro database XML | Game -> archive, source histories and releases. A source or release owns its own details, serials and files; numeric file IDs may repeat under different owners. | The one-pass reader and native writer preserve the observed field ledger, both envelopes, field presence/order and distinct details/serial/file owners. Bulk file queries, scoped interned digests, typed archive references, NUL warning ownership and paired backup regressions exist. Native history compares all observed fields and ordered owners; source versions derive from native headers. All 275 exports pass reader verification; full native SQL corpus and producer-grammar acceptance remain open. |
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
664-file native baseline run was interrupted and is not complete acceptance;
its preserved artifact started with the previous guard revision. The separate
275-export SQL run uses a frozen profiling executable and a durable user service;
it must be audited after that service reaches a terminal result.

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

The pinned MAME 0.289 software-list DTD now has an explicit 36-attribute native
SQL/public-query witness and all five PCDATA owner placements, including both
notes owners. Independent single-field editions compare exact qualified title
identities and native values; unchanged parent titles are checked separately.
Sixteen required attributes, eight enum declarations, and missing/duplicate
required item text fields exercise rollback without leaking native facts.
The document history value compares the complete ordered list tree, so a title
edit also changes that document value; it does not imply another title changed.
Original XML syntax remains in external retained documents, not copied into a
second field/value model in SQLite.

ROM hashes on all five control operations, unnamed/empty-name entries and
`nodump` ROMs remain queryable source assertions with unknown scope and no
shared UUID. Ordinary named good/baddump file declarations remain eligible.
The writer and typed file reader share a pure evidence classification. Native
publication additionally checks load flag/claim/use agreement and rejects
forged whole-file evidence or UUIDs independently of foreign-key enforcement.
This does not implement executable loading recipes or complete the remaining
formats and whole-corpus acceptance matrix.

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

Relationship decision evidence has no generic JSON/EAV tree. The public
`RelationshipEvidence` enum distinguishes a rationale, a catalog comparison
and native source-reference/merge facts. `catalog_relationship_rationales` owns only a
reason; `catalog_relationship_comparisons` owns the recorded comparison status, with
ordered closed-field assessments in `catalog_relationship_comparison_fields`. It
copies no expected source size/hash payload. Source variants are produced from
the existing native owners, never persisted as another evidence projection.

`catalog_relationship_evidence_publications` is the atomic completion boundary for
non-source decisions. It requires exactly one matching evidence subtype and
contiguous field/support orders. Explanations, support and reviews require
published evidence; source decisions instead require their catalog snapshot
publication. Evidence cannot be replaced or extended after publication, even
with SQLite foreign-key and recursive-trigger enforcement disabled. Integrity
and backup reject unfinished non-source decisions. There is no upgrade or
legacy evidence conversion. Inferred/manual endpoint subtypes now have actual
native ownership. Immutable observed-file endpoints remain a separate
unfinished shared-model requirement.

Evidence, rationale, comparison and review children reference integer
`relationship_id` owners, not repeated external assertion keys. Comparison
fields reference their actual comparison; ordered supports reference existing
published relationships and retain repeated support at distinct positions.
The external relationship key occurs once in `catalog_relationships`.

`catalog_relationship_reviews` stores the issued review key, predecessor FK,
decision, note and time once. `replaced_catalog_relationships` stores only its
review FK and replacement relationship FK. A consumed draft publishes review,
optional replacement and `catalog_relationship_review_publications` seal in
one immediate transaction. Unsealed reviews are invisible to explanations,
but integrity and backup enumerate them as unfinished durable work. Both
relationships must already be complete and published; cross-catalog reviews
retain each endpoint's actual edition rather than imposing equal editions.

The active replacement graph selects the greatest published review ID for each
relationship before inspecting its decision. Withdrawal, acceptance and
rejection deactivate an older replacement without deleting history. Newer
unsealed reviews do not change the active graph; sealing an older review adds
history without activating its historical edge. Self-replacement and active
cycles are rejected. Traversal seeks the latest sealed review by owner index
for each visited relationship, not a materialized global graph. Scoped review
readers, publication and integrity share the same canonical readiness rules.

`catalog_relationships` issues every source, inferred and manual key once.
`inferred_catalog_relationships` and `manual_catalog_relationships` own the
directed relation and two target FKs; an inferred row also references
`catalog_relationship_rules`, whose key, revision and description are explicit.
Reusing a revision with different metadata is rejected atomically.
`relationship_assertions` is a read-only display projection, not persisted
nullable endpoint payloads and not an insertion compatibility layer.
Reported, inferred and manual payload tables use `WITHOUT ROWID`: their
integer primary keys are foreign owners, not additional identity issuers.
NULL/omitted IDs cannot pass a negative-ID guard and then attach to a different
automatically allocated relationship. Explicit valid owners remain supported.

Each interned `catalog_relationship_targets` identity has one closed subtype:

| Target table | Stored ownership or declaration |
| --- | --- |
| `catalog_set_targets` | Actual set/software-item integer FK; name, list and edition derive from that owner. |
| `catalog_media_entry_targets` | Actual occurrence FK; no copied name, media order or edition. |
| `no_intro_archive_targets` | Actual database-export archive FK, independent of repeated publisher numbers. |
| `shared_file_targets` | Existing issued 16-byte file UUID FK; redirects do not rewrite the issued endpoint. |
| `declared_digest_targets` | Interned binary digest FK, explicitly unscoped and not an observed-file identity. |
| `unresolved_catalog_targets` | Snapshot plus a closed set/software/media literal shape; named fields preserve absent versus empty values. Names and ordinals do not resolve an owner. |
| `external_catalog_targets` | Declared external namespace and key, distinct from catalog owners. |

Installed guards reject orphan owners, replacement through primary or unique
identity keys, dual subtypes and mutation/late children with foreign keys and
recursive triggers disabled. Publication additionally requires actual catalog
edition seals. The shared requested-key readiness fragment also builds the
integrity view, so an issued identity with no payload cannot disappear from
backup checks. Scoped queries walk edition owners and then directed target
indices; single-origin projections avoid materializing all endpoint pairs.
Reconciliation candidates carry actual media-entry IDs when known. Source
relationships stay with native declarations; the generic source writer and
fallback importer have been removed. None of this implements immutable local
file-scan endpoints or proves complete per-format acceptance.

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

The native machine implementation stores the header in `mame_document_facts`;
its build is not copied to `catalog_snapshots.declared_version`.
`catalog_snapshot_versions` joins the native owner instead. Required
`config_version` represents source `mameconfig`, and required BIOS descriptions
are checked while streaming. Defaulted header, machine, BIOS, media, display,
input/control, driver and slot attributes retain explicit-presence facts.
Switch masks and setting values are source text, not coerced integers.

The bounded `catalog_machines` reader returns the native hierarchy from an exact
published snapshot, using numeric owners rather than machine names. Media are
occurrence references, not copied payloads. The same requested-owner child-order
query serves publication, native history and the public reader. Each union branch
seeks its requested machine before reading native rows; no globally materialized
child projection is stored. Publication checks required owners, matching media
payloads, singleton cardinality and unique cross-family/nested source positions.
Native insert/immutability guards also operate with SQLite foreign-key and
recursive-trigger enforcement disabled.

ROM declarations now retain size, CRC, SHA-1 and offset source text. Checked
numeric size is virtual rather than a stored copy. Native ROM/disk shapes are
separate, and historical machine/ROM/disk attributes have actual native-parent
compatibility owners qualified by `mame-observed-compat-declared-text-v2`.
ROM MD5 text belongs to that compatibility owner. Public file payloads hydrate
typed native media fields from existing occurrence references; history compares
raw declaration spelling as well as normalized evidence.

Publication checks both directions of the native-literal/source-assertion
relationship using requested-owner lookups. Uninterpretable supplied size/hash
fields cannot assign a UUID. No-dump and unproven historical loading facts retain
unknown-scope evidence instead of an invented whole-file claim, while CHD header
hashes never assign a whole-container UUID. MAME offsets are hexadecimal even
when their source text contains only decimal-looking digits. Referenced parser
interpretations cannot be replaced or deleted beneath their native facts, and
reinterning a digest preserves the existing digest owner ID.

Position-only companions now retain all 125 declared attributes and ten qualified
compatibility fields on 33 actual native owners. Numeric owner keys, closed field
codes, source order and positive integer QName coordinates are the only columns;
no values or source syntax are copied. Conditions keep their complete actual
parent key, including condition order, with one shared field enum across three
native condition tables. Native and compatibility fields occupy one opening-tag
ordinal domain. Publication and source-free history share edition-scoped
validation, while public readers check the complete requested owners.

The independent pinned inventory and synthetic all-field fixture exercise SQL
owners and public queries, including decoded UTF-16 and compressed inputs.
This is not completion of the broad pinned MAME contract: grammar, executable
loading semantics and full acquired-corpus acceptance remain open.
Configuration `conflocation` is declared by
the [pinned 0.289 embedded DTD](https://github.com/mamedev/mame/blob/mame0289/src/frontend/mame/infoxml.cpp):
`name` and `number` are required, and `inverted` defaults to `no`. Its attribute
presence and defaults are covered by the same native witnesses as the other
declared machine fields; its coordinates are not borrowed from the switch tag.

## MAME software-list native relations

The source-fidelity cutover adds native `software_documents` and plural-only
`software_wrapper_headers`; wrapper build has no copied snapshot-version value.
`catalog_snapshot_versions` derives it from the native owner. Software defaults
retain explicit-presence flags for supported, width, endianness, ROM/disk status,
disk writeable and DIP-value default. Numeric CDATA remains source text once,
with checked decimal/octal/hex virtual interpretations; unknown values are NULL,
not saturation or invented zero. Raw checksum text distinguishes missing, empty,
invalid and usable declarations independently of the normalized assertion rows.
Numeric source owners preserve repeated names and every occurrence.

Position-only scalar children and cross-family source ordinals preserve native
order and locations without another value store. Software snapshot history uses
the existing relationship-key representation `[list_name,item_name]`; missing
title classification uses qualified software coverage, not root-set membership.
History reads native owners, normalizes known-child ranks across families, and
compares complete same-name owner multisets rather than matching generated IDs
or physical positions. Repeated-name list contexts are cached once per parent;
exact structural content defines comparison-local classes, not persisted IDs.
Title comparisons do not copy or serialize the whole parent subtree per title.
A source spelling/default-presence edit is a metadata edit;
usable declaring-file hash changes are separately reported. Loading operations
do not become additional required files. Exact per-flag loader/length/hash proof,
complete typed native query interfaces and full-corpus performance acceptance
remain open; these source-fidelity changes are not complete format acceptance.
The public `catalog_files` bulk and UUID-page APIs now expose native software
ROM/load-operation and disk payloads. These are transient typed query results
over the authoritative native rows, not another persisted projection. Raw
numeric/hash spelling and empty/invalid distinctions survive alongside checked
segment interpretations, explicit default presence, component/source order,
source locations and the actual file-declaration owner. Requested IDs bound
both native payload branches; no catalog-wide software union is materialized.
The existing provenance retains list/title/part/area ownership. The public
`catalog_software` pages include complete native title/part scalar, info,
feature, switch and area metadata. The new
`mame-softwarelist-declared-text-compat-v2` interpretation retains all 36 pinned
attributes, plus the compatibility wrapper's `build`, in 13 native position-only
child tables. Each row belongs to its actual numeric native owner and stores a
closed field code, source ordinal and decoded QName line/column; attribute values
remain stored only in their native field. The plural-only wrapper header has an
integer owner ID and a unique snapshot key. Empty or malformed raw hash text
still has a lexical position, independently of usable digest evidence. Omitted
defaults have no synthetic position. Typed metadata and media APIs hydrate these
vectors from SQLite without the original document, and history compares
recognized attribute ranks without treating vendor gaps or physical formatting
as changes. GPT-6.1 Sol medium review/fix/re-review is clear. A full-gate
follow-up restores native-area error precedence while retaining complete
lexical-witness checks; all 30 focused area, attribute and history tests pass.
The complete devenv gate passes 1,306 tests with one existing ignored test;
all-feature nextest passes 1,309 tests with three existing optional skips.
Strict Clippy and warning-denied all-feature documentation also pass.
Executable loading recipes and full-corpus acceptance remain separate
unfinished work.
The [pinned loader decision table](software-list-loading.md) separates observed
MAME 0.289 behavior from the proposed checked Rust interpretation. Approval and
the executable interpretation remain separate gates; retaining source facts
never depends on whether their loading recipe can be executed.

Pin the upstream 0.289 `hash/softwarelist.dtd`, independently from machine XML.
The accepted plural `<softwarelists>` wrapper is an application compatibility
dialect, not the canonical DTD root.

The following table describes the current physical relations, including the
separate data-area/disk-area detail tables proposed in MAMEC-DOC-12. That document
remains the design and acceptance checklist; this alignment is not full format
or executable-loader acceptance.

| Current native relation | Fields and nesting |
|---|---|
| `software_lists` | name, description?, notes?; provides a record namespace |
| `software_items` | native record FK, cloneof?, supported, description, year, publisher, notes? |
| `software_item_info`, `software_item_shared_features` | item FK, occurrence order, declared name, optional value |
| `software_parts` | item FK, order, name, interface |
| `software_part_features` | part FK, order, declared name, optional value |
| `software_areas` | compact part/item owner FKs, kind=data/disk and area order; exactly one matching kind-specific detail row is required at publication |
| `software_data_areas` | area PK/FK, name, source order/location, required raw size text and virtual checked size, width/endianness and explicit-default presence |
| `software_disk_areas` | area PK/FK, name and source order/location only; no nullable size, width or endianness placeholders |
| `software_rom_entries` | area FK, operation order, optional name/size/CRC/SHA-1/offset/value/loadflag, status |
| `software_file_declarations` | claim FK, unique declaring-entry FK; identifies the native entry that declares a file without copying its name/hashes |
| `software_file_uses` | entry FK, file-declaration FK, load/continue/reload/ignore/fill/disk use kind; several ordered entries may refer to one file |
| `software_disk_entries` | area FK, order, claim FK, name, SHA-1?, status, writeable |
| `software_part_dipswitches`, `software_part_dip_values` | part FK; switch name/tag/mask; ordered value name/value/default |

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
explicit-versus-default presence. The opt-in `logiqx-dtd-1.5-v1` interpretation
checks the pinned content grammar in the existing completed-game streaming
pass. It has its own interpretation/edition identity but reuses the same native
owners, source store and file registry. The default compatibility interpretation
continues accepting sparse and named producer fields. Full real-corpus
field/query acceptance remains distinct from the grammar witnesses; these
tables alone do not establish every producer dialect's conformance.

The strict interpretation validates supported name-only and SYSTEM/PUBLIC
DOCTYPE syntax without resolving external identifiers. Internal subsets are
unsupported and rejected; this is the pinned format grammar, not an arbitrary
document-supplied DTD interpreter. External `standalone="yes"` cannot depend
on omitted defaults, changed enumeration normalization or container whitespace.

The default interpretation is named `logiqx-declared-text-compat-v2`. ROM size,
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
| `cmp_rom_claims`, `clrmamepro_rom_merges` | actual media-entry ID; name, size?, CRC/CRC32 alias, MD5?, SHA-1?, declared nodump/baddump flags and status; the named compatibility merge literal has its own native relationship owner |
| `cmp_samples`, `clrmamepro_set_links` | one occurrence-keyed scalar sample declaration with unknown size/digests; cloneof and sampleof literals remain distinct set-owned relationships |

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
size text, both CRC/CRC32 declarations, MD5/SHA-1 text, date/serial/status
and independent nodump/baddump presence in `cmp_rom_claims`. The merge literal
belongs only to `clrmamepro_rom_merges`; its keyword, quote state, order and
location derive from the existing `cmp_rom_field_positions` row. Numeric size and
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

Parent declarations have native owners too. `no_intro_dat_set_links` owns each
present `cloneof` or `cloneofid` literal once and links it to a once-issued
reported relationship identity. The game row does not repeat either value.
Existing game field-position rows retain their attribute order and location.
Attribute locations identify the first QName character, including any prefix,
not the opening tag. The lexical ordinal counts every attribute, including
namespace and vendor declarations. Lines and Unicode-scalar columns are
one-based in decoded XML; CRLF is one newline and a tab is one column. UTF-16
coordinates are not original encoded-byte offsets. Both strict and compatible
DAT modes use v2 rules for this contract; a reimport creates a different parser
interpretation without rewriting an earlier published edition. Database-export
observed and NUL-recovery interpretations likewise use v2 attribute positions
on their existing closed native owners. Exact originals stay external.
A name-based parent stays an unresolved set-name reference; a publisher-ID
parent stays an unresolved `NoIntroDatIdReference` on its actual declaring set.
Empty strings and leading zeroes are significant. Both declarations can exist
on the same game without becoming the same assertion or resolved identity.

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

Numeric `clone` and `mergeof` declarations now own once-issued reported
relationship IDs on `no_intro_pc_clone_links` and `no_intro_pc_merge_links`.
Their exact digit-text archive references are stored once, including arbitrarily
long leading zeros; empty text, NULs, non-ASCII digits and non-text storage are
rejected. A game's own archive ID is not required to declare a target.
`clone="P"` remains a separate marker and may coexist with merge text.
All owner views and explanations follow the actual set, native game and snapshot.

The synthetic `no-intro-pc-synthetic-provenance-v2` interpretation stores only
lexical order and QName coordinates in `no_intro_pc_game_attribute_positions`
(name, id, namealt, region, languages, version, bios, clone, mergeof) and
`no_intro_pc_rom_attribute_positions` (name, size, crc, md5, sha1). Closed field
codes and native owner IDs form each WITHOUT ROWID primary key; values stay
with their existing game, claim, language, link or normalized digest owner.
Namespace/vendor attributes count toward ordinals but have no native rows.
Coordinates are one-based decoded Unicode scalars, with CRLF folded once and
tabs counting one column, including UTF-16 input.

Positions are immutable, require pending native owners and cannot be replaced
or appended after publication. Publication closes attribute presence in both
directions; insertion and publication require exactly one source-declared
whole-asset digest per hash position. Computed or unrelated assertions cannot
stand in for source fields. Clone/merge explanations derive their QName
coordinates from game positions. Public bulk ROM queries seek requested
occurrence IDs and expose typed positions; history compares relative recognized
attribute order, not vendor gaps or whitespace. Authentic P/C, remaining native
XML position owners, loading policy and full corpus/profile proof stay open.
Targets remain unresolved archive-number references even when build planning
finds an archive with the same number. The synthetic interpretation projects
clone as source-parent and merge as alternate-representation evidence, not
an exact-content assertion. Element provenance is retained; exact attribute
positions/order remain unfinished.

Native `no_intro_pc_documents` and `no_intro_pc_headers` now retain optional
header presence, root/child order and original element locations. Separate
`no_intro_pc_header_names` / `no_intro_pc_header_descriptions` rows retain every
accepted repeated value, including explicit empty text. Version text and its
position live once on the header; public version provenance derives from it.
Native games retain document and description child order. ROM claims retain
`size_text` and original game-child order, with a checked virtual signed-size
projection; the existing synthetic unsigned parser accepts optional `+`.
Valid unsigned declarations above signed SQLite range remain stored and
queryable without UUID association. Normalized hashes remain separate scoped
source assertions, not inline copied binary columns. Unsupported media merge
and dump-status columns were removed from this synthetic ROM shape.

P/C ROM payloads are FK-owned `WITHOUT ROWID` rows; native owner/publication
guards reject orphan, replacement and late-child writes independently of FK
enforcement. Bulk queries hydrate typed native ROM size/order payloads through
the existing bounded join. Document, game-child and ROM-relative history use
native owners and ignore vendor gaps/reindentation without erasing source order.
This native synthetic cut is not authentic DAT-o-MATIC wire-format acceptance
or exact attribute lexical-position proof.

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

Native MAME samples now use actual `asset_occurrences` IDs plus `mame_samples`
payload rows: name, full machine-child source order and original location. The
machine owner derives from the occurrence FK path; no second set/order sample
table is stored. Duplicate and explicitly empty names remain separate media
entries. Typed machine and bulk-file APIs project that same native owner,
including the existing machine-specification sample view. Publication rejects
missing payloads, declared sample hashes, mismatched owners, duplicate child
positions and late/replaced facts; independently computed metadata is distinct.
History sample hydration uses the actual occurrence key rather than materializing
sample rows from unrelated snapshots. Exhaustive pinned-DTD field/grammar/query
witnesses remain unfinished.

Machine link attributes (`cloneof`, `romof`, `sampleof`) and device references
now own compact reported relationship IDs with actual kind-qualified FKs and
same-edition publication closure. The registry retains each issued external
review key once; native owners retain the literals and source positions once.
The former generic dependency table and copied MAME clone assertions are gone;
common dependency/explanation shapes are read-only query projections. Reimport,
source-free queries, history, support/reviews and paired backups use the issued
identities. Repeated machine names and empty source references remain distinct,
not assumed unique by a parser-wide name set. `mame_rom_merges` and
`mame_disk_merges` now own their merge literal/location once, keyed to the actual
matching native media occurrence and one compact reported relationship ID.
The ROM/disk payloads no longer copy merge text. Even parentless, empty or
ambiguous references receive an identity and remain explainable, reviewable and
usable as ordered support. `SourceMerge` describes the source declaration:
its subject is a typed native `OccurrenceId`, not a serialized name/order tuple;
its target is an unresolved ROM/disk reference with the actual declaring machine
and optional parent context. No resolved target or exact-content identity is
fabricated. Parent context derives from existing native machine links; it is
not copied into each merge. The Logiqx and CMP reported-relationship cutover uses
one shared typed identity issuer and media-merge writer to replace
format-specific registry implementations and the former resolved-merge pass.
`logiqx_set_links`, `logiqx_device_references` and `logiqx_file_merges` own Logiqx
declarations. `clrmamepro_set_links` and `clrmamepro_rom_merges` own CMP literals
without copying its existing field positions. The shared registry and readiness
contract apply to all three formats. GPT-6.1 Sol medium review/fix/re-review is
clear. The complete devenv gate passes 1,110 tests with one existing ignore,
strict Clippy, script/format checks and CLI smoke. Fifteen new regressions cover
native ownership and once-issued keys, empty/parentless references, actual CMP
field provenance, review/support/reimport/backup/rollback, mutation/replacement
seals and independent wrong-edition/claim-kind readiness. Canonical requested-key
plans remain native-row-bounded after importing 300 unrelated sets across all
three formats and running `ANALYZE`; metadata scaling across many catalog groups
and complete corpus/performance acceptance are not established by this witness.
Locked all-feature nextest passes 1,113 tests with three existing optional skips;
all-feature documentation builds with warnings denied.
Software-list and flat No-Intro DAT parents extend that shared registry to
18 closed reported kinds. One native parent writer selects the actual value
owner with an enum; one catalog-record endpoint constructor is shared by source
and generic readers. Software clone values live only in `software_clone_links`,
with their list namespace derived from the actual item owner. Name-based and
publisher-ID-based DAT parents live only in `no_intro_dat_set_links`, with
attribute provenance derived from the existing field positions. Their issued
keys replace the former synthetic DAT key namespace. Unresolved declarations
remain distinct from resolved identity; no target-name lookup is performed.
The shared exclusion guard blocks generic copies both before and after
publication. This software/flat-DAT milestone passed Sol medium review/fix/
re-review, the complete devenv gate (1,128 tests, one existing ignore), locked
all-feature nextest (1,131 tests, three existing optional skips), strict Clippy
and warning-denied docs. Eighteen added tests share the native regression
fixture. They cover issued keys, unresolved name/ID literals, source-free
queries, reimport/reviews/support/backup/rollback, generic-copy exclusion,
independent ownership/publication seals and wrong-edition readiness. Canonical
projection, inverse-closure and readiness plans remain native-row-bounded after
500 unrelated owners across five formats and `ANALYZE`, including checks against
scanning all 11 physical native relationship-owner tables. This is not metadata
scaling or full corpus/CPU/heap acceptance. Other No-Intro reported families and
derived/user/observed endpoint registries remain pending cutovers; this does not
finish the shared model.

Database-export archive clone and merge declarations extend the same registry
to 20 closed reported kinds. `no_intro_archive_clone_links` and
`no_intro_archive_merge_links` own each literal and once-issued integer
relationship ID on the actual archive FK. Their kind-qualified registry FKs
cannot attach a different declaration kind. Attribute order and coordinates
derive from existing `no_intro_archive_field_positions` codes 30 and 31;
link and `P` marker rows no longer copy them. A `P` marker has no relationship
identity. Empty values, leading zeros and repeated archive numbers remain exact
unresolved source text, not names or resolved ordinal-based identities.

One `ReferenceOwner` enum carries the actual software item, flat DAT set or
database-export archive newtype into the shared identity issuer and native
writer. Issued-ID proofs are not copied. The canonical explanation projection,
reverse ownership closure, readiness, reviews and ordered support all use those
same native owners; the former generic archive assertion payload and archive
endpoint column are removed. Generic source copies are rejected independently
of endpoint shape, before and after publication in both export interpretations.
New regressions cover exact literals/provenance, source-free queries,
reimport/reviews/support/backup/late-EOF rollback, immutable native owners,
independent EOF and publication seals for both clone and merge, orphan identities,
missing field positions and actual-owner edition readiness. Populated query-plan
witnesses include 600 unrelated native owners across six formats and all 13
physical relationship-ID tables. Synthetic P/C reported ownership and the
derived/user/observed endpoint cutovers remain open, as do complete field,
query, loader and full-corpus CPU/heap acceptance.

GPT-6.1 Sol medium review/fix/re-review is clear for this archive cutover.
The initial once-issued-identity test failed before implementation. Sol's
generic-copy finding also had observed failing tests: otherwise-valid set-shaped
copies could bypass a guard that checked only archive endpoint shapes. The
corrected guard excludes these native source fields regardless of endpoint
shape. The final independent-seal tests remove the other seal inside a rolled-back
fixture savepoint, preserving actual owners/positions and positive controls.
The frozen final code passes the complete devenv gate (1,140 tests, one existing
ignore), strict all-target/all-feature Clippy, format/scripts/CLI checks, locked
all-feature nextest (1,143 tests, three existing optional skips), and
warning-denied all-feature documentation. Twelve added integration tests extend
the shared regression fixture; not every test is claimed as an observed TDD cycle.

A fresh optimized/debug-symbol profiling build imported the acquired Atari 2600
source-code, Seta Aleck64 and Fairchild Channel F database exports: succeeded=3,
failed=0; 53 games/archives, nine clone identities, 44 `P` markers and no generic
assertions. These specimens contain no merge declarations; merge behavior is
covered by the independent regression fixtures, not this authentic sample.
SQLite quick/FK and application integrity checks are clean. SQLite is 2,240,512
logical bytes; the external original-object directory is 9,070 bytes for 38,781
input bytes. Including startup: 0.55 seconds wall and 23,308 KiB peak RSS. The
frozen importer and logs are preserved in
`/tmp/mame-coalesce-native-db-archive-corpus.HQS5ne`; importer SHA-256
`43f22802bbbd098f77d848886097021a35438e6369bd08231c459b6723eff196`.
This bounded sample is not complete corpus or new CPU/heap profiling acceptance.
Existing databases, originals, corpus files and profiling artifacts were not
deleted, converted or committed.

Reconciliation carries the actual occurrence ID in transient root requirements.
It indexes native evidence by snapshot and occurrence, attaching a declaration
only to its full source record. Unresolved references do not attach evidence to
parent records or same-owner siblings. Source-merge context changes neither
matching status nor exact-identity candidate support; CHD-header evidence remains
separate from whole-container identity. No stored logical-name projection is added.

Replacement seals cover occurrence IDs and owner positions, set and group identities, and their native
children, even when foreign keys and recursive triggers are disabled. Readiness
independently checks each declaration's actual owner edition. Published device
reference ordinals are dense, so projected attribute dependencies cannot collide
with device-reference order. Machine pages, snapshot dependency/history queries
and build device-reference queries start from requested owners before seeking
their native rows; the read-only union is not materialized across other editions.
Relationship explanation payloads likewise seek the selected source group, set,
occurrence and native media keys. Support hydration reads only returned derived
candidate keys, preserving declared support order; source-only catalogs issue no
support lookups.

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

Every reviewable source declaration uses `relationship_id UNIQUE NOT NULL` as an
FK to a source-origin `catalog_relationships` identity. Its issued external
`assertion_key` is retained once for reviews and ordered support. The identity
records its closed native
declaration kind and snapshot FK; that kind must agree with exactly one owning
native row. Owner identity/field/occurrence is unique. Source targets may be
unresolved literals; resolution is separate from the existence of the source
declaration. Snapshot consistency follows the subject's native record and is
checked before publication, including any optional resolved target.

| Source declaration owner | Owned declaration and assertion link |
|---|---|
| `mame_machine_links`, `mame_device_references` | cloneof/romof/sampleof or device_ref, including the reference tag |
| `mame_rom_merges`, `mame_disk_merges` | owning claim FK and declared merge name; no second merge-name copy in the asset payload |
| `software_clone_links` | actual item FK, once-issued relationship FK and declared cloneof name; optional singleton, scoped to its software list |
| `logiqx_set_links`, `logiqx_device_references`, `logiqx_file_merges` | actual game/media-entry FKs and distinct cloneof/romof/sampleof/device/merge source declarations |
| `clrmamepro_set_links`, `clrmamepro_rom_merges` | actual set/media-entry FKs and cloneof/sampleof/compatibility merge literals; provenance is read from existing native field-position rows |
| `no_intro_dat_set_links` | actual game FK, once-issued relationship FK and distinct cloneof-name/cloneofid-publisher-ID literals; provenance comes from existing game field positions |
| `no_intro_archive_clone_links`, `no_intro_archive_merge_links` | actual database-export archive FK, once-issued integer reported relationship FK and literal number/merge token; provenance derives from existing archive attribute positions 30/31. Clone and merge remain distinct unresolved references. `P` has a separate marker owner and no relationship identity. |

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
