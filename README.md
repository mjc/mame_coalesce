# mame_coalesce

`mame_coalesce` imports Logiqx DAT files, scans ROM sources, plans deterministic
builds, and writes verified ZIP/7z archives or directory outputs.

## Status

This project is pre-1.0. The primary verified development and handoff path is
the pinned devenv environment provided by this repository.

The crate is not currently being treated as a crates.io publishing artifact.
Release readiness here means a reviewed GitHub handoff with reproducible local
checks, CI coverage, and explicit release notes.
Normal CI and day-to-day verification do not run `cargo package`; packaging is
deferred until crates.io distribution becomes a goal.

## Workflow

The primary workflow is one-shot:

```sh
devenv shell -- cargo run -- build fixtures/test.dat /path/to/roms /path/to/out --jobs 8
```

Common options:

```sh
--layout parent-bundles
--layout per-game
--compression deflate
--compression store
--output-container zip
--output-container 7z
--output-container directory
--missing warn
--missing fail
--dry-run
--set "Game Set Name"
--reuse-verified
```

`--reuse-verified` leaves an existing ZIP, 7z, or directory artifact untouched
only when its complete logical contents match the current plan. Otherwise the
artifact is rebuilt as usual. The default remains replacement; source files
are still scanned/planned normally, and this option does not execute a saved
plan.

Build and audit commands accept repeatable `--set NAME` options to restrict
planning to exact set names in the selected DAT:

```sh
mame_coalesce build catalog.dat /roms /out --set "Game Set Name" --set "Another Set"
mame_coalesce --cache /tmp/coalesce.db audit "DAT Header Name" /roms --set "Game Set Name"
```

Set names are the DAT game/set names, not the DAT header title. Matching is
exact and case-sensitive, and selection is scoped to the chosen catalog even
when another imported DAT contains the same set name. Omitting `--set` keeps
the existing behavior and plans every set. Year, manufacturer, BIOS, and
`romof` are retained as catalog metadata but are not currently planner filters.

Explicit cache maintenance commands are available for advanced workflows:

Ordinary DAT imports and builds use the same immutable native catalog snapshots
as explicit catalog imports. A local DAT's canonical path identifies its catalog;
changed bytes publish a new snapshot instead of replacing another list with the
same header title. Cached builds accept an exact catalog key, source path or
unique display/header name and select the latest published snapshot. Ambiguous
names are errors; exact keys take precedence over paths, and retained source
paths remain queryable after their original files are removed. One-shot builds
pin the snapshot returned by their import, including when bytes revert to an
older edition or another import publishes during scanning.
Scanned files remain independent observations; scan summaries
count whole-file SHA-1 candidates once, without assigning a file to one list's
ROM row. The flat build interface does not flatten software-list items or
silently combine repeated set names.

```sh
mame_coalesce --cache /tmp/coalesce.db cache import fixtures/test.dat
mame_coalesce --cache /tmp/coalesce.db cache scan /path/to/roms --jobs 8
mame_coalesce --cache /tmp/coalesce.db cache scan /path/to/roms --reuse-unchanged
mame_coalesce --cache /tmp/coalesce.db cache scan /path/to/roms --reuse-unchanged --force-rehash /path/to/roms/suspect.rom
mame_coalesce --cache /tmp/coalesce.db cache build "DAT Header Name" /path/to/roms /path/to/out
mame_coalesce --cache /tmp/coalesce.db audit "DAT Header Name" /path/to/roms
mame_coalesce --cache /tmp/coalesce.db audit "DAT Header Name" /path/to/roms --format json
mame_coalesce --cache /tmp/coalesce.db audit "DAT Header Name" /path/to/roms --refresh --jobs 8
mame_coalesce --cache /tmp/coalesce.db cache backup /path/to/coalesce.backup.sqlite
mame_coalesce --cache /tmp/coalesce.db cache integrity
mame_coalesce --cache /tmp/coalesce.db cache restore /path/to/coalesce.backup.sqlite
mame_coalesce --cache /tmp/coalesce.db cache restore /path/to/coalesce.backup.sqlite --replace-existing
```

Backups consist of a SQLite snapshot made with `VACUUM INTO` and an adjacent
`<backup-path>.documents` directory containing retained source objects. Keep
both together when moving a backup; source bytes are never stored in SQLite.
Eligible whole-file SHA-1/SHA-256 source declarations share a persistent
16-byte file UUID without combining their catalog entries. Hash lookup joins
the original declarations; shared identities store no copied hashes or size.
Contradictions and ambiguous bridges remain unlinked, with references to the
specific source hash and size evidence. A disputed hash also blocks later sparse
claims until their exact conflicts receive published reviewed outcomes. The
`file_match_reviews` Rust API records append-only decisions with a rationale,
exact hash/size dispositions, and explicit UUID merges. Rejection excludes only
the named source assertion and cannot be undone by a later accept. A merge
preserves every issued UUID and source occurrence; old IDs resolve to the kept
UUID. Unlinked incoming entries are not assigned identities by a review.
Backup/restore preserves the
database-wide registry generation; a fresh rebuild starts a new generation.
UUID interchange uses 32 hexadecimal characters without dashes.

Relationship endpoints distinguish an unscoped declared digest (`ContentObject`),
an explicitly asserted observed whole-file digest (`ObservedContentIdentity`),
and an issued expected-file UUID (`SharedCatalogFile`). Equal digest bytes can
share one binary dictionary row without sharing these endpoint identities.
Observed endpoints never allocate or redirect catalog UUIDs. Relationship origin
and evidence describe the assertion; the endpoint alone does not certify a scan,
verification or physical location. CRC/MD5 observations do not become strong
catalog identity evidence. Both digest endpoint namespaces use the same checked
algorithm/byte-length predicate for insertion and publication.

Logiqx imports retain the original size/checksum spelling, empty declarations,
and text boundary spaces in native catalog fields. Uninterpretable declarations
remain queryable but are not usable matching evidence and cannot assign a shared
file UUID. Logical disk hashes never identify whole-container files. The default
`logiqx-declared-text-compat-v2` interpretation accepts sparse compatibility
documents; it is not strict DTD validation.

Use catalog import's `--format logiqx-dtd15` to select the pinned Logiqx DTD
1.5 grammar instead. The Rust API uses
`CatalogDocumentFormat::Logiqx(LogiqxMode::StrictDtd15)`; ordinary DAT workflows
and `--format logiqx` retain `ObservedCompatible`. Strict and compatible
interpretations create distinct immutable editions over the same external
original and use the same native catalog tables and shared file registry.
Strict parsing checks required fields, child order and multiplicity, declared
attributes, enumerations, and EMPTY/PCDATA content rules during the existing
streaming pass. Required CDATA may be empty or numerically uninterpretable;
it is not silently converted to usable file evidence. Enumeration space
normalization retains original QName positions and explicit/default presence.
The fixed grammar does not fetch DTDs or expand external entities.
With an external DOCTYPE, `standalone="yes"` is accepted only when omitted
defaults, enumeration normalization, or container whitespace do not depend on
the external declaration. DOCTYPE names must match `datafile`; declarations
inside catalog elements are rejected rather than discarded.
Strict mode accepts a name-only DOCTYPE or a syntactically valid SYSTEM/PUBLIC
external reference. Internal subsets are unsupported and rejected explicitly;
document-supplied declarations cannot override the configured pinned grammar.

ClrMamePro ROM declarations likewise retain checksum case, leading-zero size
text, quoted/empty values, both CRC aliases and independent dump flags in native
columns. Conflicting declarations remain queryable but cannot assign a shared
UUID; an ambiguous digest never supplies a singular matching hash. Field
positions retain source spelling, quotation, order and location without copying
the values. Publication checks complete ownership and agreement with normalized
source assertions. Headers, directives and set values use fixed native columns
with separate closed-field positions. Set region, release-date components and
serial preserve their declared text; set serial is distinct from ROM serial.
Lexical semicolon comments have ordered document-owned rows, separate from the
header comment. The documented missing `forcenodump` default is `obsolete`;
explicit unknown directive values remain stored but have no effective mode.
History tracks native document and scalar/sample/ROM order without treating
repeated-name source positions as identity. The
`clrmamepro-declared-text-compat-v1` interpretation is an explicit compatibility
grammar, not proof of every CMP dialect. Each scalar sample now owns a distinct
media entry in mixed ROM/sample source order, including repeated and empty
names. Bulk catalog queries expose its native name, location and list/set
provenance; size and declared digests remain unknown and it cannot receive a
file UUID. Samples are not ROM build requirements. The full format witness
matrix remains unfinished.

The native `catalog_clrmamepro::sets_for_snapshot` API reads an exact published
CMP edition without opening its original. Set pages retain the optional header,
all fifteen ordered header fields and effective directives, document-owned
semicolon comments, and complete selected sets with mixed scalar/parent/ROM/sample
children. Three shared closed field ledgers preserve existing codes. ROM and
sample references embed the same native `ClrMameProFilePayload` models exposed by
`catalog_files`, including keyword spelling, quotation and actual token positions.
Valid raw size text is checked independently of SQLite's virtual cast; raw hashes,
CRC aliases and conflicting dump declarations remain distinct. Cursor anchors
are tied to actual owners, snapshot and registry generation. A set limit does not
truncate its children. The current schema has no sealed total set/media count;
the reader does not claim to discover wholly erased unseen owners from it.

MAME machine XML stores ROM size/hash/offset declaration text without losing
missing, empty, leading-zero or uninterpretable values. Numeric size is a checked
virtual decimal projection; offset queries follow MAME's hexadecimal spelling.
ROM and CHD disk rows have separate native shapes, and historical attributes
belong to separately qualified compatibility owners. The default interpretation
is `mame-observed-compat-declared-text-v2`, not strict DTD validation.
Uninterpretable supplied declarations prevent shared UUID association. No-dump
claims and historical loading fields without a pinned whole-file contract retain
their literals and unknown-scope hash evidence without receiving a UUID. CHD
header hashes never identify whole-container bytes. Publication requires native
source literals and normalized source assertions to agree in both directions.
The `catalog_files` API hydrates typed MAME ROM/disk/sample payloads from existing
machine media references without reparsing originals. History includes their raw
declarations, explicit defaults and compatibility fields.
All 125 attributes in the pinned machine specification and ten separately
qualified compatibility fields have position-only companions on their actual
native owners. Public machine and file queries expose closed field enums,
lexical attribute order, and decoded QName coordinates without reading originals.
An omitted default has no invented position; explicitly empty declarations do.
Native and compatibility fields share the opening tag's attribute order.
History compares relative recognized-field order, ignoring vendor gaps, layout
and coordinates. Publication and history share edition-scoped presence and
coordinate validation; these companions contain no copied values or XML.
Each filename-only sample owns a distinct occurrence, including repeated and
empty names, in mixed ROM/disk/sample source order. `mame_samples` stores its
name and position once; its machine is reached through the occurrence owner.
Samples have no inferred size, hash, extension or UUID and are not ROM build
requirements. Independently computed metadata does not become a declared hash.
Machine `cloneof`, `romof`, `sampleof` and ordered device references own their
declared text once, with real FKs to compact reported relationship identities.
Issued review keys survive reimport and paired backup; source references do not
copy endpoints into generic assertion rows. Empty targets and repeated machine
names retain distinct native owners. ROM/disk merges likewise own one native
declaration and issued review key, even without a parent or resolvable filename.
Absent and explicitly empty merge text remain different. `SourceMerge`
explanations identify the actual media occurrence and retain an unresolved
reference to its declaring machine, ROM/disk kind, optional parent and merge name.
They do not fabricate a resolved target or an exact-content identity assertion.
Resolution remains a separate assertion; matching a filename cannot resolve
an ambiguous parent name. Media IDs and owner positions cannot be replaced.
Reconciliation attaches merge context through the actual snapshot/media ID;
neither a same-named sibling nor an unresolved parent receives that evidence.
Merge context does not change hash/size matching or support exact identity.

No-Intro flat DAT XML has four explicit import interpretations:
`no-intro-dat-v3-strict`, `no-intro-dat-v3-compatible`,
`no-intro-dat-v4-strict`, and `no-intro-dat-v4-compatible`.
Use them with `cache catalog-import --format`; a declared schema location is
not evidence that the source satisfies that schema. Strict interpretations use
the pinned producer contracts; compatibility interpretations preserve the
observed reordered fields, repeated ROMs, nested game IDs, header comments,
ROM MIA/date fields and sparse declarations without claiming XSD conformance.
Both keep publisher IDs, categories, releases, directives, raw size text
and field presence in native relational tables. Valid hashes refer to one
interned binary value; query text is canonical lowercase hexadecimal, and the
original spelling remains in the external document. Invalid or empty hash
literals retain their exact text, not matching evidence. Any uninterpretable
supplied size/hash prevents UUID linking. Header-filter directives qualify digest scope;
they cannot silently assign a whole-file UUID. Parsing streams one game at a
time, and an error anywhere through EOF rolls back the pending catalog edition.
This flat DAT dialect is distinct from database-export and synthetic P/C XML.

No-Intro DAT and database-export attribute positions locate the first character
of the original qualified attribute name, not the opening element. Ordinals
include namespace declarations and vendor attributes. Coordinates are one-based
lines and Unicode-scalar columns in decoded XML, with CRLF counted once and tabs
counted as one character; they are not encoded-byte offsets. The external source
retains the exact original UTF-8, UTF-16 or gzip bytes. These coordinate rules use
the `no-intro-dat-xsd-v2`, `no-intro-dat-observed-compat-v2`,
`no-intro-database-observed-compat-v2` and `no-intro-database-nul-recovery-v2`
interpretations. A changed interpretation produces a new immutable snapshot;
existing snapshots are not rewritten or migrated.

The separate `no_intro_db_xml::read_with` Rust API streams No-Intro database
exports into typed games, archive descriptions, dump sources and releases.
Each dump source or release owns its own details, serials and files. Explicit
observed-compatible and NUL-recovery modes accept both single-root and sibling
header/datafile framing. Recovery changes only decoded U+0000 in the parse view;
warnings are generated lazily with original coordinates after validated EOF.
Clean input remains borrowed, and the header is moved to the caller once.
Both interpretations are also `cache catalog-import` formats. Native tables
retain archive descriptions, distinct dump-source/release owners, details,
serials, file fields and their order/locations. Valid hashes share interned
binary values; origin and NFO hashes are separate scoped evidence. Export
file hashes have unknown scope until a whole-file contract is established,
so they do not assign shared file UUIDs. Bulk file queries and paired backups
include these native owners. History compares every observed native field,
presence and ordered owner/child structure without treating generated IDs or
physical source positions as continuity. Unknown-scope file, origin and NFO
digests remain metadata, not invented ROM build requirements. History and
relationship provenance derive header versions from native owners: an export
has a singular reported version only when exactly one version child exists.
Repeated header declarations remain ordered native facts. Full corpus and
producer-grammar acceptance remain open.

`catalog_no_intro_dat::games_for_snapshot` reads an exact published flat DAT
in its actual v3/v4 strict or compatible mode, without opening original XML.
A checked `1..=500` game limit returns the complete native header and selected
games: ordered header text/directives, declared parent references, repeated
categories and publisher IDs, releases and ROM references. Raw sibling and
attribute ordinals retain vendor/namespace gaps; list and per-family orders
remain separate. Missing fields differ from present-empty fields, and the
effective `forcenodump` default never replaces the declared text. ROM references
embed the same payload used by `catalog_files`, loaded in the same read
transaction. That payload exposes stored evidence scope and the existing
virtual numeric size alongside the original size text. No new persistent
projection or source bytes are added. Snapshot/registry-pinned cursors validate
their actual native anchors; paired backups preserve them.
The bounded reader checks available native ownership, positions, sealed counts
and digest registries. It is not a replacement for global integrity checking:
native ROMs have no independent game key after their shared occurrence is
completely erased, and global descendant totals do not identify which game
lost a fully erased collection.

`examples/no_intro_dat_verify.rs` compares one source XML document with the
public pages of an exact published DAT snapshot:

```sh
cargo run --locked --profile profiling --example no_intro_dat_verify -- /path/catalog.sqlite sha256:SNAPSHOT_HEX /path/source.dat
```

Run this inside the repository's devenv environment. Snapshot keys parse through
the checked `SnapshotKey` API; parsing proves their canonical shape, not that
they exist. The verifier compares supported header/game/ROM values, presence,
defaults, mixed-child order, declared positions and digest scope through EOF.
It retains one parsed game and one 64-game query page; original/decoded source
buffers and coordinate bookkeeping remain input-dependent. It does not import or update catalog
facts; normal database opening still takes the application lock and validates
the schema. Verification is a same-parser storage/query round trip, not an
independent producer-specification check. Valid hash spelling/case, root QName,
schemaLocation attribute QName/position and ignored vendor content are outside
the native query contract. A mismatch or incomplete source exits unsuccessfully.

The MAME machine and Logiqx verifiers use the same existing-database safeguards:

```sh
cargo run --locked --profile profiling --example mame_native_query_verify -- /path/catalog.sqlite sha256:SNAPSHOT_HEX /path/mame.xml
cargo run --locked --profile profiling --example logiqx_xml_native_verify -- /path/catalog.sqlite sha256:SNAPSHOT_HEX /path/catalog.dat observed-compatible
```

For a strict Logiqx snapshot, select `strict-dtd15` instead. The verifiers check
the actual retained reading-rules version, not its opaque interpretation key.
They traverse the source once through valid EOF, compare typed native metadata,
declared values/default presence and positions, and require complete query-page
exhaustion. Normalized source-declared digest assertions are checked independently
of raw hash spelling, including their scope and multiplicity. Media requests are
batched without capping or dropping a game's children. MAME and Logiqx each retain
one parsed record and a 64-record native page; those records' children and the
original/decoded buffers remain input-dependent. Logiqx uses 256-occurrence SQL
batches; MAME uses the public 10,000-occurrence bulk limit.

These are storage/query round-trip checks, not independent producer grammar
proof or observed ROM-byte verification. Unsupported vendor extension payloads
remain in the external original, outside the native query comparisons. MAME's
existing compatibility parser is not a strict pinned-DTD validator. Logiqx keeps
raw valid hash spelling except the binary document-level SHA-1. Native Logiqx
device references retain element locations and mixed-child order separately
from their family ordinal and name-attribute QName positions; history detects
crossings while ignoring reindentation and vendor-only ordinal gaps.

The software-list verifier compares one original with its exact published edition:

```sh
cargo run --locked --profile profiling --example mame_softwarelist_native_verify -- /path/catalog.sqlite sha256:SNAPSHOT_HEX /path/software-list.xml
```

It uses the same existing-database safeguards and the software-list streaming
reader, without collecting a source catalog. Checks cover bare/plural envelopes,
wrapper build presence, all 36 recognized attributes and five text placements,
effective defaults and explicit presence, raw numeric/hash spelling, checked
numeric values, qualified normalized digest assertions, and native parent/order/
position provenance. ROM declaration and control-operation links are compared;
CHD-header hashes are not treated as whole-container UUID evidence. List notes
can appear after software items and are compared at list end. Actual trailing EOF
and complete list/title/media exhaustion are required.

Memory includes the original/decoded buffers and coordinate bookkeeping, one
complete source item, a 64-list page, and a 64-title page with complete children.
Media queries use 256-ID batches without dropping or capping an item's entries.
Vendor extensions are counted and reported, not compared as catalog fields.
This is a source-to-storage/query round trip, not independent producer-grammar,
ROM-byte, or executable-loader proof.

The ClrMamePro verifier compares all 39 declared header/set/ROM fields, lexical
comments, mixed ROM/sample order, quotation and source positions:

```sh
cargo run --locked --profile profiling --example clrmamepro_native_verify -- /path/catalog.sqlite sha256:SNAPSHOT_HEX /path/source.dat
```

It streams the supplied source through validated EOF and requires complete native
page exhaustion. Header placement, absent/empty values, raw checksum aliases,
normalized declared digests, effective directives and ineligible UUID links are
checked separately. Queries do not open the retained original. The original
buffer, one complete source form/set, document comments and selected sets' full
children remain input-dependent; native reads use 64-set pages and 256-ID media
batches without truncation. Unsupported vendor extensions are counted, not
compared. This is a same-parser storage/query check, not independent producer
grammar or observed ROM-byte verification.

`catalog_no_intro_database::games_for_snapshot` reads an exact published export
without opening its original document. A checked page limit bounds games;
every selected game's archive, dump-source and release histories are complete,
including optional details/serials, file references and typed attribute positions.
The native metadata API and existing `catalog_files` payloads together expose
all 129 attributes in the observed export ledger. File references use the same
occurrence IDs and preserve both game-wide file order and each history owner's
mixed details/serials/file order. Empty, absent and repeated declarations remain
distinct. Continuations belong to the exact snapshot and database registry;
paired backups retain them, while fresh rebuilds reject them. Namespace
declarations may leave gaps in lexical attribute ordinals; header and child
ordering rules still apply. This API does not promote unknown-scope export
hashes to whole-file evidence or establish a producer grammar.
Saved game/header counts and dense mixed child/file order are checked before
returning a page. File ownership is checked from both the selected game's
occurrences and native files; two game-keyed indexes keep those reverse checks
bounded. Archive links must retain their reported relationship provenance.
Missing owners, dangling digest IDs and unknown digest field codes are errors,
not omitted metadata.

`examples/no_intro_database_profile.rs` exercises that reader with the
`no-intro-database-xml-compatible` or `no-intro-database-xml-nul-compatible`
interpretation and XML paths. It reports complete record counts and recovery
locations without retaining a catalog tree or writing a database.

The Rust `catalog_files` API exposes bulk occurrence lookup and keyset-paginated
file membership across published catalog editions. Results keep each owner and
its source/list provenance, with digest assertions as separate children.
Pagination cursors belong to one file UUID and registry generation.
Software ROM/load-operation and disk occurrences also expose typed native
payloads through both bulk lookup and UUID pages. They retain original numeric
and checksum text, checked segment sizes/offsets, explicit-default presence,
source order/location and area-local file-declaration links. Empty or invalid
declarations remain queryable without inventing usable matching evidence.
Software load-operation hashes, hashes on unnamed/empty-name ROM entries, and
hashes declared on `nodump` ROMs retain unknown scope and cannot issue a shared
file UUID. Publication checks the actual load flag, claim kind and file-use
operation together; changing a label cannot promote an operation to a file.
Disk payloads keep CHD-header hashes separate from whole-container identity.
These payloads describe source facts, not validated executable loading recipes;
the `catalog_software` API separately pages the lists in an exact published
software-list snapshot and the titles in one numeric list owner. Title pages
include native scalar positions, info/shared features, parts, switches and
data/disk areas. Area entries reference the existing `catalog_files` occurrence
IDs rather than copy ROM/disk fields. Repeated names remain distinct owners,
absent/empty values and explicit defaults remain distinct, and source order
includes gaps from vendor elements. Cursors pin the registry generation,
snapshot and (for title pages) list; older published editions stay queryable.
Each page reads one SQLite snapshot and carries document/catalog provenance
once. Software areas have compact owners and separate data-area/disk-area
detail tables: disk areas store no size, width or endianness placeholders.
Publication requires exactly one matching detail row; source order stays with
that detail and remains unique across data/disk areas in the same part.
The `mame-softwarelist-declared-text-compat-v4` interpretation retains all 36
pinned software-list attributes and the compatibility wrapper's `build`
attribute in 13 position-only native child tables. Public metadata and media
results expose closed, typed attribute-position vectors without reading the
source document. Positions identify the attribute QName in decoded XML;
omitted defaults have no synthetic position. History compares recognized
attribute order, ignoring vendor gaps and physical formatting. Executable
loading recipes are available through `software_loading`: a checked plan
borrows complete native data-area occurrences, binds declaration-keyed source
bytes, and only then permits region writes. Partial groups warn and process
as in the [approved MAME 0.289 contract](docs/software-list-loading.md).
First-run verification, maximum-run progress and actual read lengths remain
distinct. Whole-file size matching derives the first base/continue/ignore run
from the native entries; it never uses a segment or maximum reload length.
Malformed or overflowing runs have unknown size, while their raw facts remain
queryable. Fill ends file ownership. Software/DAT size disagreements retain
the declaring occurrence and its reachable chain as reviewable conflict evidence.
No ROM bytes or calculated sizes are stored in SQLite. Emulator integration
and full-format acceptance remain open.
Results distinguish the immutable source-issued UUID from its current canonical
UUID. Published reviews also version cursors: after another review, restart
pagination rather than silently skip newly merged members.

Snapshot publication verifies that linked source and canonical UUIDs were
actually issued. All eight native whole-file roles share the same source-size
consistency check, including declarations whose own length is unknown.
Published redirects combine retained source facts; only an exact published
size rejection excludes a length. Equal lengths, unknown-only components and
unlinked conflicting source entries remain publishable. No identity stores a
copied size or a cached consistency flag.
This does not upgrade unknown-scope database-export declarations to whole-file
evidence or establish the synthetic P/C parser's producer conformance.

Relationship evidence uses a closed Rust enum and native relational owners,
not a generic JSON tree. User/rule rationales and ordered comparison-field
assessments have separate typed tables; comparisons do not copy expected
sizes or digests. Imported source evidence is reconstructed from its native
facts. Publication atomically seals the evidence and support rows before
explanation, review or backup; incomplete decisions are integrity failures.
Inferred and manual relationships share the same once-issued identity registry
as imported declarations. Their endpoints reference actual set, media-entry,
archive or issued shared-file IDs through typed foreign keys. An unresolved
name remains a literal reference, never an automatically resolved owner.
Rules declare their key, revision and description separately. Unscoped digest
references reuse the binary digest table; they do not prove observed bytes or
issue file UUIDs. The former generic assertion payload is a read-only projection.

Relationship evidence, ordered supports and reviews use integer owner FKs;
issued external keys remain stored once. A review stores its note and time once,
with an optional replacement edge and an atomic publication seal. Unsealed
reviews remain invisible to explanations but fail integrity and backup checks.
Only the latest published review activates a replacement; withdrawal preserves
history while removing that active edge. Self-replacement and active cycles are
rejected, including when a newer draft is still unsealed. Publishing an older
review adds history without changing the latest decision. These FK-owned
reported/inferred/manual payloads and evidence/review children cannot allocate
a different owner when an ID is missing or NULL.

The synthetic P/C adapter also issues native relationship keys for numeric
`clone` and `mergeof` declarations. Their literal archive references retain
leading zeros and remain unresolved in explanations, even when build planning
finds a matching archive number. `clone="P"` is a marker, not a relationship;
it can coexist with a merge declaration. Clone projects a source-parent relation
and merge an alternate-representation relation under this synthetic interpretation.
This does not establish authentic DAT-o-MATIC P/C wire-format support.

The synthetic adapter stores its optional header, repeated names/descriptions,
version, game description and ordered ROM declarations in native tables.
Missing and empty fields remain different. Header versions are read from their
native owner, not copied into snapshot rows. ROM size spelling is retained;
valid unsigned sizes beyond SQLite's signed range remain queryable without
inventing a signed size or assigning a shared UUID. Bulk file queries expose
the native P/C ROM payload with typed attribute positions. Nine game and five
ROM attributes have position-only native owners: lexical order and one-based
Unicode-scalar QName coordinates, not copies of their values. Publication
requires every present attribute's position and exactly one source hash per
hash position. Clone/merge explanations point at their declaring attribute.
The named `no-intro-pc-synthetic-provenance-v2` interpretation detects recognized
attribute reordering while ignoring vendor-only gaps and reindentation.
Authentic producer grammar remains separate unfinished work.

Logiqx retains all 44 DTD 1.5 attributes in their native value owners, with
position-only tables for explicit declarations. Closed field enums distinguish
these from named compatibility fields such as ROM serials and device references.
Coordinates point at attribute QNames in decoded Unicode scalars; transport BOMs
do not count as columns. Omitted defaults acquire no invented positions.
The `logiqx-declared-text-compat-v2` interpretation tracks recognized attribute
reordering, independently of child order, vendor-only gaps and reindentation.
Bounded file queries, history and relationship explanations use these native
facts without reading the original document. Publication requires complete
attribute positions and actual native ancestry; standalone integrity checks also
find orphaned draft values. This does not establish strict DTD conformance or
complete format/corpus acceptance.

Import errors can retain an exact byte excerpt with a start-inclusive,
end-exclusive highlight relative to that saved excerpt, not the whole file.
`problem_start_byte` and `problem_end_byte` index the stored `source_excerpt`:
for `name='bad'`, `[6,9)` identifies `bad`. These are byte offsets, not display
columns; rendering or decoding the excerpt must map them to displayed text.
XML character and encoding failures capture their actual bytes, including
NUL, invalid UTF-8 and UTF-16. Gzip excerpts name the decoded XML view and do
not claim compressed-file offsets. Unknown ranges stay absent. The schema
checks paired integer bounds and links each diagnostic to its run and source
document; failed catalog facts still roll back. Database-export NUL recovery
persists the original encoded bytes and exact excerpt-relative ranges after
validated EOF. Its warnings link to the most specific stored XML element:
document, header, header field, game, archive, dump source or release, and
their details, serials or file declarations. Each link uses the actual native
record's foreign key and complete element range. Failed imports retain their
error diagnostic, without links to rolled-back catalog records.

`import_diagnostics::for_run` reads persisted run metadata, ordered diagnostics,
optional saved excerpts and typed native owners without opening the source
document. Pages contain 1–500 diagnostic rows; continuation cursors are bound
to the actual run and database registry. A page limit does not truncate an
excerpt or discard owner links. Unknown byte anchors and highlights remain
unknown when saved evidence is loaded or clipped. The run summary comes from
its first diagnostic; there is no duplicate message in the run table.
Additional semantic token/field highlights and other formats' native diagnostic
owners remain open.

Format version 1 is marked in the SQLite header and includes the cache database,
acquisitions, catalog snapshots, assertions, reviews, diagnostics, and
rebuildable inventory. Backup creation never replaces an existing backup file.
Restore validates a same-directory staging copy against the running program's
bundled DDL before publishing it; an existing cache requires
`--replace-existing`, and restore is refused while the cache is open by this
application or SQLite sidecar files exist. Cache files with multiple hard links
are refused because they bypass application locking. New databases are created
from the bundled DDL, and existing databases are accepted only when their
stored schema digest and SQLite schema objects match it. The application does
not upgrade or repair a different schema; create a new cache and reimport its
catalogs when the schema changes. `cache integrity` never modifies the inspected
SQLite database; backup, restore, and integrity operations may create adjacent
`.lock` files on both the cache and backup paths. It reports durable
catalog/document failures separately from rebuildable inventory problems.
Inventory problems do not invalidate a backup or prevent restore. Where the
platform cannot sync the containing directory, publication succeeds with a
warning that crash durability could not be confirmed. The integrity command
returns a failing exit status when either category contains issues.

Build, audit, and cache scan also accept repeatable `--source-root DIR` options.
The positional source directory stays first; roots are canonicalized and exact
aliases are scanned once. Earlier roots take precedence over later roots, then
the existing within-root bare-file/archive/path/member order applies. Physical
members reached through overlapping roots resolve once, attributed to the
earliest matching root; a nested root still has its own cached provenance and
can take precedence when listed first. A multi-root refresh scans every root
before changing inventory, then replaces all requested scopes in one SQLite
transaction. If scanning any root fails, none of those scopes are replaced.

For example, `mame_coalesce build catalog.dat /roms/primary out
--source-root /roms/secondary --source-root /roms/overrides` searches the
primary tree first, followed by secondary and overrides. Repeat the same ordered
options for `audit` or `cache scan` to use the same scope and precedence.

Cache scans rehash every source by default. For a single source root, the
explicit `cache scan --reuse-unchanged` option may reuse observations for bare
files when their persisted Unix file identity, size, modification time, and
change time still match. Archives are always read and fingerprinted. Use the
repeatable `--force-rehash FILE` option to bypass reuse for selected files;
files with absent or mismatched cache stamps also fall back to a full hash.
This metadata check is an opt-in performance heuristic, not integrity evidence:
privileged metadata manipulation or filesystem behavior outside the platform
stamp can make it stale. Builds still verify source bytes against catalog
evidence before writing output. Non-Unix platforms safely disable reuse.

The reusable library operations in `app` do not initialize logging or terminal
progress. `plan_build` reads the selected catalog and cached scan observations
and returns a logical plan without creating outputs. `build` reads the cache and
writes requested artifacts. `import_dat` and `scan_source` persist catalog and
inventory changes, respectively; `run` performs those imports/scan updates
before building. The `*_with_roots` operations accept an ordered
`SourceRootSelection`; the original operations remain one-root conveniences. A
one-shot `--dry-run` or strict-missing build still imports the
DAT and refreshes the scan cache, but does not write ROM outputs. The CLI owns
human-readable reports, progress bars, and mapping build outcomes to process
exit codes. `build_with_container` and `run_with_container` select ZIP, 7z, or
directory output without changing request types; their ordered-root variants
offer the same choice.

`audit` has no output destination and never invokes an output writer. It uses
an already-imported DAT and cached source observations by default and labels
them as cached rather than freshly checked bytes. Opening the cache validates
its schema against the bundled DDL; that does not refresh source observations. `--refresh`
explicitly rescans and persists the selected source root before resolving;
`--verify-selected` instead reads and verifies only the resolved source members,
reports stale or unavailable selections, and leaves the inventory unchanged;
it cannot be combined with `--refresh`.
`--matching-policy evidence-aware` opts into the
resolver's evidence-aware conflict/ambiguity classifications. Human reports
include expected and observed evidence and the selected or competing sources.
To keep adversarially large inventories from multiplying report memory by the
number of requirements, resolution retains at most 256 detailed assessments and
256 duplicate-source examples per operation; omitted counts remain explicit in
JSON and human reports.
`--format json` writes version 2 audit documents to stdout and keeps logs and
progress on stderr; the reader remains compatible with version 1 reports.
Audit exits `0` when all requirements match and `1` when
requirements remain unresolved (operational failures also exit `1`). These
audit statuses do not change build's existing `0` success, `1` execution-failure
and `2` strict-missing behavior.

Evidence-aware matching ranks SHA1 above MD5 above CRC-plus-size. It may fall
back to a weaker digest only when the observed stronger digest does not
contradict the catalog; a stronger contradiction vetoes that candidate. A
unique CRC-plus-size candidate is classified as weak evidence, while collisions
remain ambiguous unless consistent stronger evidence establishes equivalent
copies. The default SHA1-compatibility policy is unchanged and does not use
those fallback matches.

ZIP compression defaults to deflate for compatibility. Use `--compression store`
when profiling or when faster, larger ZIP output is preferred.
`--output-container` is independent of layout and compression; it defaults to
`zip`, `directory` writes each logical group as an uncompressed directory, and
`7z` writes each group as a 7z archive using the pinned `r7z` LZMA2 defaults.
`--compression` applies only to ZIP output; it is ignored for 7z and directory
output.

Defaults:

- `--layout parent-bundles`
- `--compression deflate`
- `--output-container zip`
- missing ROMs are reported without failing
- `--missing fail` exits `2` and writes nothing when required ROMs are missing
- duplicate source matches are resolved deterministically

Building from archive members stages selected bytes on disk before assembling
each output ZIP or 7z archive. Its private staging directory is created beside
the output, so ensure that filesystem has up to 16 GiB of free space for one
output archive.

Logical output paths use a conservative portable naming profile: ASCII only,
case-insensitive collision checks, no trailing spaces or dots, path traversal,
Windows-reserved device names, or platform-forbidden characters. Unicode names
are rejected rather than relying on filesystem-specific normalization. The
one-shot workflow rejects source and destination roots that are equal, nested,
or aliased through existing symlinks; it checks before scanning and again before
writing. Linux and macOS artifact writing use handle-relative directory access
and do not follow symlink components. These checks contain untrusted catalog and
archive paths; concurrent local processes modifying the destination are outside
the threat model. Other platforms are rejected before creating or truncating
artifacts. Sources are never modified.

Each ZIP or 7z artifact is built in a temporary file beside its destination.
Source bytes are streamed and checked against the selected catalog evidence and
scanned source identity; archive fingerprints are checked before and after
member reads. The finished archive is flushed and synced before replacement. On
Linux and macOS, renaming that same-filesystem temporary file replaces one
artifact atomically;
the containing directory is synced afterward to ask the filesystem to persist
the new entry. This does not guarantee survival of sudden power loss on every
filesystem or storage device (notably, macOS `fsync` may not flush drive caches).
A multi-artifact build is not atomic as a whole: execution stops at the first
failure and reports which artifacts completed, failed, or were not attempted.
Atomic replacement is currently supported on Linux and macOS only.

Each directory artifact is materialized in a private sibling staging directory.
Replacing an existing real directory first renames it into a unique sibling
backup, then installs the staged directory. If installation fails, the writer
attempts to restore the backup; if rollback also fails, the report includes the
recoverable backup path. A successful install whose backup cleanup fails is
reported as completed with a warning and retains that backup. Directory rename
is not treated as a universally atomic replacement: a process or machine crash
between moving the old directory aside and installing the new one can leave the
destination absent, with the old output under its hidden backup name. A
multi-group build is not atomic as a whole. Output sources are read-only.

Source scans intentionally skip hidden files and directories below the source
root. Non-UTF-8 paths and traversal or archive-read errors fail the scan, so an
incomplete inventory cannot replace the last successful cache contents.

## External smoke test

To test against downloaded public-domain ROM bundles:

```sh
devenv shell -- bash scripts/fetch_public_domain_test_data.sh
```

The script downloads only archive.org items whose metadata is Public Domain Mark
or CC0 by default when `--catalog-tier metadata` is used. Its default curated
catalog also includes archive.org items whose title, description, or upstream
source explicitly describes the ROMs as public-domain/PD ROMs. It generates a
focused Logiqx DAT from the downloaded bytes and runs the one-shot workflow with
`--missing fail`. It writes all temporary data under `tmp/public-domain-rom-test/`.

Use `--max-roms 0` to include every collected ROM entry.

## Maintenance

For module boundaries, compatibility and cache behavior, change
recipes, and refactor test/measurement evidence, see
[`docs/architecture.md`](docs/architecture.md).

Use devenv 2.3.1 or newer with Nix. The repository owns `devenv.nix`,
`devenv.yaml`, and `devenv.lock`; no machine-local devshell imports are needed.
The Rust module selects latest stable (currently pinned to 1.98.1), including
Clippy, rustfmt, rust-analyzer and Rust sources. Nix supplies the compiler/linker,
SQLite, OpenSSL, zlib, CMake and pkg-config; sccache caches compilation.

```sh
devenv shell                 # interactive development shell
devenv shell -- build        # cargo build --locked
devenv tasks run project:check  # formatting, scripts, tests, strict Clippy
devenv test                  # the same gate, Git hooks and CLI smoke test
```

Inside an already activated shell, run `cargo` or `build` directly.
For automatic activation, configure `devenv hook` for your shell and run
`devenv allow` after reviewing the checkout.
The project does not require `.envrc` or automatically load `.env`.

| Feature | Usage |
| --- | --- |
| Named verification tasks | `devenv tasks list`; `devenv tasks run project:format` |
| Git checks installed on shell entry | Rust formatting, Nix formatting, ShellCheck |
| Faster test runner | `devenv shell -- cargo nextest run --locked` (the main gate also runs doctests) |
| Profiling tools | `devenv --profile profiling shell` for flamegraphs, perf on Linux and hyperfine |
| Maintenance tools | `devenv --profile maintenance shell` for audit, deny, outdated and machete |
| Toolchain refresh | `devenv update rust-overlay`, then `devenv test`; review and commit the lockfile |

Normal builds and tests use `--locked`; entering a shell does not update
Cargo dependencies or run the full test suite. No services are needed for this
CLI: tests use temporary SQLite databases and synthetic archive fixtures.

## MAME XML catalog import

The machine `-listxml` adapter imports machine records, clone relationships, ROM
and disk declarations, BIOS sets, and every machine-child family declared by the
MAME 0.289 DTD, including displays, input controls, switch conditions, drivers,
features, devices, slots, software-list references, and RAM options. DTD defaults
and required device-reference tags are stored as typed facts, with ordered nested
rows and snapshot-diff coverage. Required `mameconfig` and BIOS descriptions are
checked before publication. The native header owns build text once; snapshots do
not copy it. Effective default values retain separate declared-presence facts.
Machine-child ordinals preserve source order across families; snapshot history
compares known-child order without treating vendor-only gaps as edits. Switch
masks and settings keep their declared text, including hexadecimal spelling.

The `catalog_machines::machines_for_snapshot` Rust API returns bounded pages from
an exact published edition, with numeric machine identities, native flags and
scalar locations, BIOS sets, dependencies, switches and every specification
family. ROM/disk/sample references identify existing occurrences without copying their
payloads. Repeated machine names remain distinct, and continuations are pinned
to their snapshot and registry generation. Queries read stored native facts, not
the source XML. Raw ROM size/offset spelling and qualified compatibility owners
are retained. Source relationships from six verified parser families and
inferred/manual relationships now use native owners. Authentic P/C reported
identity, immutable observed-file endpoints and exhaustive pinned-DTD
field/grammar/query coverage remain unfinished.

The separate software-list adapter imports list-scoped items, parts,
data/disk areas, component evidence, and load instructions as source data. It
retains omitted versus explicit defaults, empty values, original size/offset/hash
text, native child order and scalar locations. Checked numeric values are virtual
SQL projections, not additional stored copies. Single-list roots and plural
wrappers have distinct native document owners; wrapper build text is stored once.
Native snapshot history compares list-qualified titles and their complete child
hierarchy, without treating generated IDs or changed physical coordinates as edits.
Repeated-name list contexts are shared within the comparison instead of copied
into every title's facts.
File uses refer to their declaring entry instead of copying its name, offset or
value. Exact per-flag loader/hash/whole-file length proof remains open; the adapter
does not execute loading instructions or expand dependencies. Both adapters
retain unrecognized XML in the external original document, not a generic
SQLite field table. Imports accept retained documents and
gzip-expanded XML up to 384 MiB. XML tree adapters are limited to 200,000
elements; the record-streaming MAME machine and software-list adapters allow up
to 6,000,000 elements to accommodate official full catalogs and larger lists.
Nesting is limited to 256 levels. These bounds limit retained input, parser
work, and per-record trees; larger documents are rejected.

Real catalog import CPU flamegraphs and a partial full-machine-XML heap trace are
documented in [catalog import profiling](docs/catalog-import-profiling.md).
The repeatable CPU helper is `scripts/profile_catalog_imports.sh`.

### CPU Flamegraphs

The profiling helpers mirror the workflow used in `nntp-proxy`, with
`mame_coalesce`-specific categories for hashing, archive work, scan walking,
planning, SQLite/Diesel, and writing. Always run them through Nix.

Baseline single-thread run:

```sh
devenv --profile profiling shell -- bash scripts/profile_flamegraph.sh \
  --dat fixtures/<dat>.dat \
  --source <source-dir> \
  --out target/profiling/out-jobs-1 \
  --jobs 1
```

If perf permissions block sampling, retry with `--root`:

```sh
devenv --profile profiling shell -- bash scripts/profile_flamegraph.sh \
  --dat fixtures/<dat>.dat \
  --source <source-dir> \
  --out target/profiling/out-jobs-1 \
  --jobs 1 \
  --root
```

Parallel comparison:

```sh
devenv --profile profiling shell -- bash scripts/profile_flamegraph.sh \
  --dat fixtures/<dat>.dat \
  --source <source-dir> \
  --out target/profiling/out-jobs-8 \
  --jobs 8
```

Summarize or compare generated flamegraphs:

```sh
devenv shell -- bash scripts/parse_flamegraph target/profiling/flamegraphs/run-jobs-1.svg summary
devenv shell -- bash scripts/parse_flamegraph target/profiling/flamegraphs/run-jobs-1.svg top 30 0.5
devenv shell -- bash scripts/parse_flamegraph target/profiling/flamegraphs/run-jobs-1.svg search planner
devenv shell -- bash scripts/parse_flamegraph target/profiling/flamegraphs/run-jobs-1.svg diff target/profiling/flamegraphs/run-jobs-8.svg
```

Benchmark the full `build` workflow with repeated wall-clock samples:

```sh
devenv --profile profiling shell -- bash scripts/benchmark_run.sh \
  --dat tmp/perf-public-domain/dats/public-domain-roms.dat \
  --source tmp/perf-public-domain/source-roms \
  --out-root target/profiling/perf-out-jobs-1 \
  --jobs 1 \
  --runs 5
```

Add `--compression store` to measure the opt-in stored-ZIP write path while
keeping default CLI behavior unchanged.
The benchmark helper prebuilds the profiling binary and runs it directly by
default; pass `--runner cargo` only when measuring the older `cargo run` path.

Optional raw perf-data analysis, if `perf.data` is retained or captured
manually:

```sh
devenv --profile profiling shell -- sh -c 'perf script 2>/dev/null | bash scripts/parse_perfdata'
```

## Test coverage

Run the complete repository gate with `devenv test`. The database
and catalog regressions cover these behaviors:

- `database_initialization` checks creation from bundled DDL, acceptance of a
  matching schema, and rejection of schema drift without repair.
- `catalog_content_disputes` checks conflicting source evidence, later sparse
  claims against disputed hashes, immutable published assertions, guarded
  replacement writes, cross-format file membership and registry-preserving
  backup/restore versus a fresh generation.
- `catalog_files` unit tests check published-only bulk and paginated membership,
  repeated owners, native filenames and locations, software part/area ownership,
  scoped digest children, corruption errors, temporary-table cleanup and indexed
  owner, digest and payload lookup without scanning the entire catalog.
- `native_catalog_model` checks scoped record identity and rejects occurrences
  whose claim kind or content identity conflicts with their native record;
  it also checks that no second mutable DAT model exists.
- `native_build_catalog` checks native publication through ordinary imports,
  exact external source recovery, optional hashes and large ROM sizes,
  path-scoped catalogs, name ambiguity, snapshot replacement without lost
  history, native build/audit queries and scan rollback.
- `native_build_selection` checks exact-key priority over an existing path,
  retained source-path lookup without the input file, reverted-byte imports
  and immutable build/audit selection across a competing publication.
- `catalog_shared_model` checks relational scope/set storage, separate owners
  for repeated names, owner-aware relationship explanations, publication
  immutability, and P/C empty fields, ordered languages and archive-ID tokens.
- `catalog_history_ownership` compares complete same-name owner fact multisets,
  preserves unchanged permutations and reports ambiguous changes without
  inventing continuity between entries.
- `snapshot_history::no_intro_database_consumers` checks every observed export
  owner field through value and absent/present-empty changes using isolated
  in-memory catalogs with the production DDL and external source store. Its
  relationship regressions preserve duplicate-owner decisions, inference
  supports, reviews, incoming links and mixed parser editions while proving
  empty root-relationship queries avoid the full explanation plan.
- `catalog_extension_ownership` checks exact external-document recovery of
  unknown game/ROM fields even when both game and ROM names repeat.
- Coverage unit tests check exact scope reuse, root/software qualification,
  selected-but-absent sets, partial unknown members and immutable referenced
  coverage rows.
- `logiqx_native_model` checks parser defaults and repeated ROM, disk, sample,
  release and BIOS-set fields, plus headerless native imports and snapshot diffs.
- `logiqx_dtd15_contract` independently checks the pinned grammar, all 44
  attributes and 15 enumerations, lexical content and normalization boundaries,
  decoded QName coordinates, declaration/BOM source ranges, and separation
  from permissive compatibility.
- `logiqx_dtd15_import` checks interpretation identity, idempotence and shared
  UUIDs, actual SQL-before-late-error rollback, normalized native query values
  and original positions without promoting invalid size declarations. Its
  complete-field witness queries every DTD attribute and text field from
  native storage.
- `native_logiqx_specification` checks native header options and repeated game
  children, explicit/default presence, empty fields, ordering and owner guards.
- `catalog_logiqx_persistence` checks separate ROM/disk/sample occurrences,
  original size text, qualified disk identity and immutable published owners.
- `native_logiqx_history` checks option and repeated-child edits, native order
  across media families, default presence, vendor-only insertions, whitespace
  changes and repeated-owner permutations without false size/hash changes.
- `logiqx_attribute_fields` checks every DTD attribute's native field code,
  lexical order and exact QName in UTF-8, UTF-8 with BOM and UTF-16, plus named
  compatibility fields, omitted/explicit defaults and empty/NULL distinctions.
  It checks source-free payloads, history and explanations after backup/restore,
  SQL-fault rollback after a position write, and independent late-EOF failure.
- `logiqx_attribute_guards` checks native parents, immutable positions and all
  value-owner REPLACE paths with foreign keys and recursive triggers off. It
  independently checks orphaned drafts, publication closure and BIOS ancestry.
  `logiqx_attribute_provenance` checks recognized attribute reordering without
  false changes from vendor attributes or reindentation.
- `catalog_ancestor_guards` checks stable publisher/catalog/document keys while
  permitting label edits, and confirms the actual Logiqx publication guard
  seeks the requested snapshot instead of scanning other native owners.
- `native_mame_specification` checks ordered machine specification facts and
  their stored field values.
- `clrmamepro_native_model` checks native CMP header directives, set/sample
  facts, and ROM date, serial, and status provenance.
- `cmp_declared_fields` checks raw ROM declarations, conflicting evidence,
  source assertion agreement and complete immutable ROM field ownership.
- `cmp_set_provenance` checks all header/set values, date-component text,
  separate parent declarations, lexical comments and native order in history,
  including header crossings and unchanged repeated-name owner permutations.
- `cmp_native_publication` checks missing native rows and field positions,
  invalid ordinals, foreign-format owners and immutable publication boundaries.
- `software_native_model` checks shared file UUIDs without confusing load
  segments with file lengths, area-local file owners, and scope-correct digests.
- `catalog_import` exercises public imports, including format-specific facts,
  idempotent publication, relationships, diagnostics, and equivalence between
  native and shared catalog fields.
- `cache_backup` checks backup and restore behavior against the current schema.

The integration suite also generates and reads synthetic 7z archives through
`r7z`, pinned at revision `bfef3198696add8045ad34581dd977d671ae9daa`; it does not
require an external `7z` executable. This checks the pinned library's
writer/reader and the application read path, not compatibility with every
external encoder or codec. Independent cross-implementation checks are outside
the default gate. To run the ignored interoperability test with a compatible
executable installed, set `MAME_COALESCE_7Z` if its name is not `7z`:

```sh
devenv shell -- env MAME_COALESCE_7Z=7z cargo test --locked --test integration external_7z_extracts_r7z_builder_archive -- --ignored
```

`cargo package` is intentionally not part of the normal local or CI gate while
the project depends on the git-only `r7z` crate.

Dependency and maintenance checks:

```sh
devenv shell -- cargo tree -d
devenv --profile maintenance shell -- cargo audit
devenv --profile maintenance shell -- cargo deny check
devenv --profile maintenance shell -- cargo machete
```

## Known Operational Constraints

- Running outside Nix requires system `pkg-config`, SQLite, zlib, and related
  development libraries.
- The crate declares `rust-version = "1.98.1"`, matching the latest stable
  Rust release selected by devenv. The project currently tests against that
  release rather than maintaining a separately validated older MSRV.
- `cargo package` requires `r7z` to be published on crates.io; until then the
  crate uses a pinned `mjc/r7z` git dependency.
- `cargo deny check` may report duplicate dependency warnings under the current
  policy, but the check exits successfully.

## Optional read-only FUSE view

On Linux, build with the optional `fuse` feature and mount a serialized MAME
0.289 view manifest:

```sh
cargo run --features fuse -- mount \
  --manifest /path/to/view.json \
  --mountpoint /path/to/empty-mountpoint \
  --spool-root /path/to/separate-spool
```

The mount is read-only. Startup validates source locations, selected archive
members, and known lengths; every open then verifies the pinned content before
exposing bytes. Same-length stale content therefore fails on open rather than
being served or retargeted. The spool must be an existing directory outside
both the source tree and mountpoint, with at least 512 MiB available.
The first adapter is Linux-only; FUSE remains optional, so catalog and other
commands do not depend on it. Other operating systems do not yet have an adapter.
