# Source-free catalog query and history contract

Status: proposed MAMEC-62 model/API contract, not production implementation.
This maps current shared and No-Intro export consumers to the actual candidate
owners. MAME/software and Logiqx/CMP/DAT/fixture consumers have their companion
crosswalks. Canonical field/count/owner TSVs remain the exact field inventories;
this document does not add stored query projections, document payloads or JSON.

## Identity and public API decisions

The fresh schema does not preserve an old database or an old opaque snapshot
key. The following are explicit proposed API changes, to be accepted with the
complete design, not silently described as backward compatibility:

| Current returned identity | Proposed owner and representation | Disposition |
|---|---|---|
| Publishing source/catalog keys and display names | `catalog_publishers.publisher_key/display_name` and `catalogs.catalog_key/display_name` | Retained, reached through the edition's catalog/publisher, not copied into native rows |
| `SnapshotKey` string | A typed `CatalogEditionId`, qualified by database registry generation when carried across calls | Reshaped; no stored snapshot-hash alias or acceptance of legacy keys |
| `DocumentKey` / document digest | `catalog_source_files.sha256`; render `sha256:` plus hex at the boundary if that presentation is needed | Derived presentation; original bytes remain external |
| Interpretation key formerly incorporating scope | `catalog_reading_rules.rules_key` plus the edition's separate `coverage_id` and its typed members | Reshaped; rules alone must not impersonate edition/scope identity |
| Occurrence/record integer IDs | Actual `catalog_media_entries.media_entry_id` / typed source-element ID | Reshaped to distinct newtypes; never a shared file UUID or ordinal |
| Diagnostic string key | Typed message identity qualified by import and registry generation | Reshaped; message ID/order stay on the existing message row, with no copied opaque-key column |
| Issued file identity | `shared_catalog_files.file_uuid`, 16-byte BLOB plus its registry FK | Retained; resolving a redirect does not rewrite the issued identity |

Preserve the existing operational generation rule: the reserved
`file_id_registries` row with `registry_id=1` supplies the database-wide current
generation UUID. Bootstrap creates it once; normal opens require it and do not
regenerate it. A fresh destructive rebuild gets a new generation; a paired
backup preserves it. Other registry identities cannot implicitly become the
current generation. No new generation table, per-list generation or persisted
cursor is introduced. SQL length/immutability guards and supported bootstrap/open
validation have distinct jobs; the candidate's existence alone is not that API.

Public provenance exposes selected format/dialect/specification/parser/rules
versions from reading rules, independently of an import's source format hint.
The former optional free-form `parser_name` is not a second persisted identity:
the new API exposes these typed rule facts; a UI may label the known reader.
This removal is an explicit proposed response-shape change. Receipt/fetch facts
remain separately queryable through their actual owners, not substituted for
source-declared file names, versions or hashes.

## Shared files and matching consumers

Current entry points are `catalog_files::occurrences_for_ids`,
`occurrences_for_content`, `file_match_reviews::resolve_file_id`,
`record_review`, and the bounded compatibility readers in `catalog_content`.

| Returned fact / operation | Exact candidate route and rule |
|---|---|
| Requested occurrences | Request IDs → common media/source element → exact native subtype → actual native parents → set/group/edition → catalog/publisher/rules. Validate the requested path; a surviving common base is not sufficient |
| Shared-file membership | Resolve the caller's issued UUID through published `file_match_uuid_redirects`; seek its canonical component's published native occurrences by media UUID/key. Keep issued versus canonical UUID distinct |
| Native payload and positions | Dispatch by the closed native kind/actual typed owner, using canonical field and hash-position ledgers. Return raw declared states/lexemes, effective defaults plus specified flags and exact provenance without reparsing |
| Compatibility hashes/sizes | Point-read maintained `shared_file_hashes`, `shared_file_sizes` and interned `hash_values`; do not replay native source-witness qualification for each incoming key |
| Explanation/review evidence | Exact native declared hash/size witness plus `file_match_conflicts`, typed witness decisions, publication and redirects. Query-only accepted evidence is for qualification/explanation/maintenance, not another hot-path fact cache |

Keep current request behavior: empty ID input returns empty; duplicate IDs are
deduplicated; the bounded bulk request permits at most 10,000 distinct IDs;
content pages accept 1..500 and fetch one lookahead. These are request limits,
not import-file limits or a way to hide query cost. Missing requested IDs retain
the existing absence behavior; a present requested common/native path that is
contradictory must not disappear through an inner join.

A content cursor retains the caller's **issued** UUID, current database
generation, settlement revision (published decision count) and last media ID.
Validate those before resolving and continuing. Publication of a settlement
invalidates a prior cursor even if its caller UUID still resolves. Use indexed
reverse component traversal from the selected canonical root and actual media
keys; do not materialize every file's redirect walk. Only published native
occurrences are public. Matching during import has the different private
completed-batch boundary in `publication-contract.md`.

Source origin, CHD/logical-disk, NFO companion, unknown and complete-file hashes
remain different evidence. CRC/MD5 alone do not bridge file identity. Invalid,
empty, absent and valid declarations remain queryable; only qualified accepted
facts join shared membership. Current `merged_file_ids(old_content_uuid,
kept_content_uuid)` is **reshaped** to the candidate's actual
`file_match_uuid_redirects(old_file_uuid, kept_file_uuid, decision_id)`, not a
second redirect table or an alternate candidate name.

## No-Intro export pages and file hydration

Current `catalog_no_intro_database::games_for_snapshot` returns a consistent
read-transaction view: publication provenance, document/header facts, bounded
games, complete ordered native children and references for file hydration.
Its 18 edition and nine parent-local count kinds are the DOC-24 contract;
ordinary pages do not rerun all edition integrity.

| Returned family | Candidate owner / selection |
|---|---|
| Exact publication provenance | `published_catalog_editions` and its matching `catalog_editions` tuple; source/catalog/rules/coverage joins above. Requested edition must be published and selected rules must be the export family |
| Document/header | `no_intro_export_documents`, actual `no_intro_export_datafiles`, canonical `no_intro_export_headers` and ordered `no_intro_export_header_fields`. Preserve envelope kind, independent header presence and absent versus present-empty header |
| Game ID/name/opening/extent/name position | `catalog_sets` joined to actual `no_intro_export_games` and `no_intro_export_game_field_positions`; group membership and registry kind agree. Optional byte extent and admitted coordinate endpoints retain exact view identity |
| Ordered archive/source/release children | `no_intro_archive_descriptions`, `no_intro_dump_sources`, `no_intro_releases`, each keyed to the actual game and selected by `(set_id,source_order,owner_id)` |
| Details/serials/files | Concrete dump-source or release child tables through their real parent IDs, using the existing parent-local order and independent singleton/count witnesses. Dump/release details retain independent `opening_end_line/opening_end_column`, in addition to element start/end; start < opening end ≤ element end. Do not reconstruct the opening endpoint from a closing tag or decoded attribute values |
| File payload / hashes / positions | `no_intro_dump_files` or `no_intro_release_files` → common media/source element, `catalog_entry_hashes`/`hash_values`/declared-text projection and corresponding native position table. No copied game ID on a file |
| NFO hashes | `no_intro_release_nfo_hashes` → actual release-details owner and its existing position; never a fabricated media occurrence or required whole-file asset |
| Archive clone/merge | Distinct marker versus typed clone link, separate merge link and actual reported relationship/position. `P`, empty literal and publisher number do not collapse |

The full field state/ownership ledger, including all observed export fields,
applies to these rows. Keep `nfo_size/nfosize`, NFO aliases, source/release field
differences and origin fields distinct. Size text is a single source value;
the existing numeric projection supplies usable comparisons without inventing
an integer for malformed text. File position order is not digest algorithm order.

Root pagination keysets on `(catalog_sets.source_order,set_id)` in the actual
root group. For this grammar only, derive public game `list_order` as
`source_order - nested_header_placement_count` (0 or 1). A sibling header does
not subtract one. Validate the exact native cursor anchor, root/group/kind/name
position and admitted containment, not only its common set row. Reject deleted,
reparented, retagged or reordered anchors. Keep the current non-serialized,
generation/edition-bound cursor and 1..500 page limit.

Hydrate all descendants for selected games in bounded request-driven sets
(existing descendant batches of 400). Derive each game's public flattened file
`occurrence_order` while traversing archive/source/release children in mixed
order, and each source/release's details/serials/files in its own mixed order.
Do not store a game-only rank or file-wide rank. Current `child_files`,
`history_orders` and `file_orders` demonstrate those distinct sequences.

For a lookup seeded by a file ID, begin with that requested ID and probe each
concrete dump/release file branch by its primary key, retaining missing-parent
evidence with outer joins. Follow source/release → game → group → edition.
Require one compatible file branch and the common media/source-element owner;
wrong/missing ancestry is an error. A selected-game page validates only its
reachable descendants, not an unrelated orphan elsewhere. Edition and global
integrity enumerate disconnected rows separately. An outer join is not permission
to accept NULL ancestry, and expected IDs must not be silently removed by filters.

## Build, audit, reconciliation and dependency consumers

These consumers also read the catalog; a page/history-only crosswalk would miss
them. Current evidence is `BuildCatalogRepository::load`,
`load_published_catalog`, `catalog_reconciliation::{reconcile,
snapshot_requirements,software_requirements}`, `machine_dependencies::load_catalog`
and `app::{audit_disks,load_published_disk_requirements}`.

| Current surface | Candidate route and preserved boundary |
|---|---|
| Build/plan/audit catalog selection | Preserve precedence: exact catalog key → retained-source path → distinct-catalog union of display-name and latest-publication Logiqx/flat-DAT header-name matches. Source paths join catalog → published edition → receipt → fetch attempt URI with local-key preference. Reject ambiguous names/paths; do not reopen XML or create an alias table. Latest selection pins one exact published edition before scanning/planning |
| Build ROM requirements | Shared sets plus exact native ROM/file declaration, checked family size/status/merge fields and usable declared digests. Return all root set names, including empty sets, separately from requirements. The current root build path rejects software-list catalogs and filters role ROM; keep that limitation explicit rather than silently flatten software titles |
| Catalog reconciliation | Two explicit published editions → root or software physical requirements via actual owners, rules and coverage; compare declared evidence, not local filesystem inventory or issued UUID equality. Preserve algorithm/scope/provenance, merge/status, serial/date and structural record-key boundaries |
| MAME dependency resolution | Exact edition → root sets and MAME machine flags, machine links/device-reference relationships, or Logiqx game flags/set links. Carry complete/filtered/partial/unknown coverage and distinct BIOS/device/parent roles; preserve the current supported MAME/Logiqx family restriction |
| Disk audit | Pinned published edition → `mame_disks`, `logiqx_disks` and `software_disks` through actual list/title/part/disk-area owners; typed SHA-1 declarations and scope, names/merge links and family clone links supply locations and parent context. Logical CHD header hashes remain disk evidence, not complete-file shared identity |
| Explicit source recovery | Edition → `catalog_source_files.object_key` and original digest/length → verified external object. This operation intentionally reads the retained original; it is not how any metadata query, build requirement or history serializer recovers missing facts |

Catalog display-name matches require at least one published edition. Header-name
matches use only each catalog's latest published edition, not an older matching
header: join its actual root group to `logiqx_headers` →
`logiqx_header_text_elements` (name field), or `no_intro_dat_headers` →
`no_intro_dat_header_text_children` (`field_kind='name'`). Deduplicate catalog
identities across those branches before enforcing exactly one match. Names are
selectors, never identities or header aliases stored on `catalogs`. An exact
key that also looks like a path wins; an explicit edition selector bypasses
name/path/latest selection and must itself be published. Reuse the one proposed
publication ordering below for both header-name matching and latest selection;
stale header names cannot select a catalog after its newest edition changes.

Root build requirements also return set metadata and parent context, not only
ROM digests. Their candidate source owners are:

| Returned build context | Candidate owner / policy |
|---|---|
| Set name and parent name | Shared root `catalog_sets.set_name`; native Logiqx/MAME/CMP clone declarations supply the literal parent. Synthetic P/C resolves its clone archive ID, then merge archive ID fallback, to the same group's game/set name; `P` is not a parent. Do not choose an ambiguous matching target |
| Source file and BIOS | Logiqx `sourcefile/isbios` or MAME `sourcefile/isbios`, rendering MAME's boolean as yes/no; synthetic P/C `bios_text` is its existing fallback. Keep NULL/empty and effective/default presence on the native owners |
| ROM/sample parents and device references | `logiqx_set_links` / `mame_machine_links`; CMP sample parent from `clrmamepro_set_links`. Ordered references from `logiqx_device_references` / `mame_device_references`; no copied parent or device array |
| Board/rebuild target | `logiqx_games.board/rebuildto`; CMP `clrmamepro_sets.rebuildto` fallback |
| Description/year/manufacturer | Typed `logiqx_game_text_elements`, `mame_machine_text_elements`, CMP `clrmamepro_sets` values; synthetic P/C description from `no_intro_pc_game_descriptions.description_text`. Do not manufacture unavailable family metadata |
| Requirement identity/order | Exact root owner plus asset name/role and checked component order, catalog/set requirement key and expected evidence. Reject a non-root owner or a returned owner name inconsistent with its native set |

Preserve `load_root_sets`' repeated-root-name rejection for build layout even
though catalog pages/history may represent repeated names. Check all root sets,
including empty ones, before constructing name-keyed requirements; do not
silently collapse them into a set or select one by generated ID. Shared set
metadata remains a transient response assembled from these owners, not another
stored projection. Explicit ambiguous P/C parent rejection is a stronger target
closure policy, not a claim that today's raw joins implement it.

Build source-path resolution must not depend on whichever reimport receipt was
last seen: edition lineage/receipt stays immutable, and separately retained
fetch/receipt associations remain queryable as acquisition evidence. An unretained
fetch cannot satisfy a retained-source selector. The new publication ordering
below is an explicit proposed change from current rowid latest-selection, with
the exact chosen edition carried through the workflow rather than reselected
mid-build.

Native declared requirements are not synonymous with qualified shared-file
membership. An unknown-scope or malformed declaration may still be a queryable
requirement/fact with unusable evidence; do not drop its owner merely because no
file UUID can be issued. Software continue/reload/ignore/fill are interpreted
through the checked native chain, not one requirement per load segment. Disks
and samples are not ROM byte-copy requirements. Build/verification retains its
existing role/status/evidence policy; no source-origin or NFO digest is promoted
to an expected whole-file checksum.

Dependency literal lookup and relationship target identity are different:
preserve unresolved/ambiguous names and coverage-aware missing-parent outcomes,
not a guessed edge selected by first matching set name. Typed relationship
assertions, ordered support, comparisons/rationales and published review or
replacement status remain available to `record_relationship`,
`review_relationship` and `explain_relationships`. Their canonical owners are
`catalog_relationships`, the reported/manual/inferred subtype relations,
`catalog_relationship_targets` and its exact typed subtypes, and the evidence/
review/publication tables in `relationships.sql`. No generic parent-ID pair,
copied source literal or stored explanation payload is introduced.

These are model routes and explicit behavioral boundaries, not evidence that
the current application consumes the candidate. Implementation acceptance must
exercise build/plan/audit, dependency closure, reconciliation and relationship
explanations with originals unavailable to metadata readers, and verify the
selected edition remains pinned across each workflow.

## Semantic history

Current `snapshot_history::{history,diff}` and its native serializers are
source-free. Preserve these semantic rules rather than persisting their current
transient JSON output or loading retained XML:

* Compare only editions of the same catalog; root versus software-list contexts
  remain incompatible. Coverage participates independently: complete coverage
  or equal filtered memberships can establish comparable scope. Missing records
  outside known coverage yield Unknown/OutOfScope, not invented removals.
* Group duplicate set names and compare fact multisets. Unique-name groups may
  get requirement deltas; exact-fact duplicate groups and ambiguous groups do
  not acquire arbitrary ID/name pairing. Generated IDs, list positions and file
  UUID allocation are not semantic correspondence.
* Document metadata includes the actual format declarations, native header
  presence, repeated ordered header values and envelope kind. Export declared
  version is descriptive and returns a value only when exactly one version
  declaration exists; repeated equal versions still do not become one value.
  No stored version projection is needed.
* Export game facts traverse actual native ancestry and mixed child sequences.
  Retain empty elements, repeated fields, source field identities and recognized
  attribute order. Normalize valid digest case via canonical bytes; retain
  malformed/empty distinctions. NFO and origin hash changes do not invent
  whole-file requirements. Formatters/coordinates are not catalog-value changes.
* Normalize known retained field/child order transiently where the current family
  history does so; vendor-only gaps alone are not edits. Never densify persisted
  source order. Family crosswalks specify their exact normalizations.
* Preserve reported relationships, manual/inferred assertions, ordered support,
  comparison/rationale and published review/replace status. Use actual typed
  endpoints in `relationships.sql`; don't treat unknown parser families as empty
  scopes, merge unresolved literals or expand empty root relationships.
* Exclude operational source lengths, excerpts, coordinates, database IDs,
  independent count seals, acquisition times, parser execution/repair warnings
  and UUID settlement bookkeeping from semantic catalog facts. They remain
  separately queryable provenance/diagnostic evidence. Source-declared textual
  values that resemble times/IDs remain catalog facts.

The new public history list is explicitly proposed to order published editions
by `(published_at,edition_id)` rather than the old opaque document/interpretation/
snapshot key lexicographic order. Reuse preserves publication time; a fresh
attempt is not a new edition/history entry. Latest-edition selection uses that
same publication order. This is a proposed ordering change, not a claim of exact
legacy ordering. Internal history facts should become closed typed comparables;
no stored JSON or runtime generic metadata table is introduced.

A history read is a separate full-edition operation: native missing/reparented
owners must not disappear from a serializer's inner-join assembly. Establish
edition closure/count policy and independently detect identifiable detached
owners before comparison. Global unattributable orphans remain global findings,
not guessed members of every edition. This is a stronger target closure
requirement than today's `load_games`; don't infer it from current code or a
green page query. History need not run a global audit for every set.

## Diagnostics, integrity, backup and scans

`import_diagnostics::for_run` remains source-free. Seek the exact `import_key`,
then messages by `(import_id,message_order)` with one lookahead. A continuation
is pinned to current generation/import and exact message ID/order; validate that
anchor before continuing. Preserve severity/code/message, NULL versus empty
record/field/offending text, partial source coordinates, exact BLOB excerpt and
its relative highlight, separate source/original ranges and actual source-view
identity. Ordinary and eleven physical-root link families retain real typed
FK ancestry; external comparison evidence is a separate published target.
Confirmed failed imports have no incoming edition/owner links. No SQL text
coercion or guessed owner may conceal malformed stored evidence.

Ordinary links use the closed native export-owner interval routes in
`diagnostic_links.py`, not registry membership alone. Verify every typed ancestor,
selected format and actual interval; every available comparable byte/coordinate
proof must contain the problem. A point at the exclusive end is not contained.
Opening-only owners remain source/root-only. The game end pair is retained on
its native game row, not synthesized from its name or its parent's end.
Scoped reverse guards preserve linked draft facts; the separate global audit
starts from links so missing or mistyped owners cannot disappear through joins.

The existing summary is the first diagnostic's message (confirmed by
`load_summary`), so derive it from message order zero, not a duplicate summary
column. The current retained/not-retained discriminator is reshaped: a
`catalog_source_files` row means verified externally retained bytes. Acquisition
failures/unretained responses belong to fetch attempts and cannot impersonate a
retained source-file row or an import with fabricated source identity.

Integrity is a dedicated operation: SQLite physical/FK checks, exact schema
fingerprint and canonical candidate global reverse/closure audits, plus
publication/relationship/review lifecycle checks. Enumerate all closure-bearing
relations, including wholly detached rows; cap displayed issues, not checks.
Catalog provenance is durable, distinct from independently rebuildable physical
scan inventory. Ordinary point reads never invoke this database-wide operation.

Backup/restore retains the SQLite catalog **and** external original-object sidecar
as a validated pair, including registry generation, rules, coverage, published
editions, diagnostics, file identities/redirects and relationship reviews.
Validate header/schema/closure and referenced objects before replacing an
existing destination; preserve current atomic staging/explicit overwrite policy
and reject unsafe sidecar states. Source-free querying can work without opening
original bytes; a complete backup/integrity operation still verifies the paired
originals. A rebuild without the registry invalidates cursors/old references.
No migration or automatic legacy database conversion is added.

`SourceRepository` physical scans remain independent: filesystem/archive path,
observed content digests and scan-run provenance are not catalog documents,
expected-file declarations or issued file UUIDs. Catalog matching may compare
observed evidence to accepted facts but never moves scanned payload into native
catalog rows or promotes an observed digest to declared source evidence.

## Evidence required

This contract is based on sem traces of the named current APIs plus candidate
DDL/ledgers and DOC-21/24/25/27. New response identities/order and reader policy
above are explicit proposed decisions for full review/approval, not implemented
compatibility claims. Constructed SQL plan witnesses can prove selected target
queries and corruption evidence, not real parser capture or a complete public API.

Implementation must verify every returned native family against actual source
fixtures without reopening originals; page/cursor/requested-path corruption and
header-presence cases; duplicate-name/scope/order/history and typed relationship
semantics; diagnostics NULL/empty/highlight/link cases; paired backup/cursor
lifecycle; and populated target query plans/VM scaling. No smaller batches,
unbounded per-owner edition audit, copied ancestry/rank or source-reparse fallback
can substitute for those proofs. Full-model review and explicit approval still
gate the production cutover.
