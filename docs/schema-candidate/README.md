# Executable catalog schema candidate

This is an isolated design artifact for
[PLAN-3's model gate](https://lific.mjc.lol/MAMEC/issues/MAMEC-62), not the
application schema, a migration, a database conversion, or an approved cutover.
Do not apply it to an existing catalog database. The checks create in-memory
databases only; no corpus, original, database or profiling output is removed.

The format notes identify the exact Lific revisions, field/code inventories,
current-to-target decisions, parser capture gaps and still-unproved contracts.
Unsupported OfflineList, PureDOS extension grammar and authentic DAT-o-MATIC
P/C grammar are not promoted to supported formats. The P/C fragment is explicitly
the existing synthetic fixture adapter.

## Run the design checks

Run directly in this repository's active devenv environment:

```sh
python3 docs/schema-candidate/assemble.py
python3 docs/schema-candidate/check.py
```

`assemble.py --emit` prints the fully composed, concrete SQL. It reads the
four closed native-owner manifests and emits actual per-table FK, reverse-FK,
identity-collision, typed-owner and mixed-order checks. The manifests are design
inputs, not SQLite data tables or a runtime polymorphic owner model. Values and
actual parent FKs stay on native tables; common bases and payload facets do not
count as additional source declarations. Registry kind literals are closed over
the manifest, not accepted from arbitrary runtime configuration.

`check.py --core` is deliberately narrower: it tests only shared tables. A passing
core check is not evidence that native fragments assemble, that source fields
have complete witnesses, or that any importer implements this candidate.
Family `*_witnesses.sql` files state their own scope; some use minimal shared
stubs to isolate a format. They are not substitutes for the assembled checks.

Run MAME, software and Logiqx/CMP witnesses directly with
`sqlite3 -bail :memory: < docs/schema-candidate/FAMILY_witnesses.sql`.
The No-Intro fixtures require the composed DDL and a statement runner that
asserts their expected rejections before checking independently corrupted rows:

```sh
python3 docs/schema-candidate/no_intro_field_check.py
```

## Initial bounded review checkpoint — 2026-10-08 UTC

The integrated harness passes 20 tests in 3.727 seconds; all four native fixtures
pass. Assembly prepares 93 closed native kinds and 30 views with a clean
empty-schema FK check. Independent GPT-6.1 Sol medium review/fix/re-review loops
are CLEAR for the bounded core and native slices. Native re-review additionally
ran 52 adversarial assertions covering the six original findings and No-Intro
relationship-position corrections. These are not complete-design approval or
authentic importer/corpus/performance proof. The outstanding work below remains.

## Relationship routing and independent field crosswalks

Routing/field-inventory checkpoint: 25 tests passed in 4.045 seconds; assembly prepared
93 closed native kinds and 34 views with a clean empty-schema FK check. Core
routing/reference and all four native field reviews are bounded CLEAR after
fix/re-review. Logiqx/CMP assertions require exactly one selected row; six
missing/multiple-selector mutations fail at the intended assertion CHECK.
No complete-design or production approval is implied.

`relationship-positions.tsv` maps DOC-23's 22 kinds to their concrete typed
declaration and canonical position. Generated checks reject the wrong owner,
field, occurrence or edition; reverse checks protect already-linked positions;
global audits diagnose missing/mismatched positions. Both No-Intro `P` marker
branches have separate presence and mutual-exclusion rules. Query-only unions
share this routing definition with the relationship-kind audit, without copying
source values or ancestry into persisted tables.

`check.py` now exercises every relationship route and all 32 hash mappings with
thin typed owners, plus an actual composed MAME ownership regression. Integer
field codes use INTEGER SQL literals: trigger `NEW` expressions do not inherit
table-column comparison affinity. Positive tests prevent valid native values
from being rejected by a superficially strict guard. The actual populated MAME
identity lookups use indexed searches across both routing views.

The four `*-field-coverage.tsv` inventories identify source fields, actual value
owners/columns, canonical position codes, accepted states/defaults and evidence.
`field_coverage` checks real SQL references (including generated columns), not
specification completeness or presence consistency. Independent family field
witnesses exercise their stated slices; their notes identify remaining gaps.
The TSV files are build-time design evidence, not generic runtime catalog data.

Run the independent field checks separately from the shared routing suite:

```sh
python3 docs/schema-candidate/mame_field_check.py
python3 docs/schema-candidate/software_field_check.py
python3 docs/schema-candidate/logiqx_cmp_field_check.py
python3 docs/schema-candidate/no_intro_field_check.py
```

The current field evidence is bounded as follows:

| Family | Inventory represented | Focused evidence |
|---|---|---|
| MAME | 155 contextual rows: 151 attribute contexts and four PCDATA scalars | Six tests; independent DTD/macro identity comparison and constructed state probes |
| Software | 42 rows: 37 attributes and five text scalars | Seven tests; all 13 position families and seven default-presence pairs |
| Logiqx/CMP | 105 rows; 46 Logiqx and 39 CMP attribute/field codes, plus native text/form facts | Nine tests; all 85 codes, 13 text-child kinds and 15 XML default-presence fields |
| No-Intro | 269 source paths; 201 distinct position table/code pairs | 284 field assertions / 24 expected rejections, plus legacy 18 / 4 |

MAME's 125 DTD plus ten compatibility declarations expand into 151 actual-parent
attribute contexts; source macros have 126 enum/code slots while the candidate
has 134 table/code pairs. These are different inventories, not conflicting
counts. None of the 571 crosswalk rows is an independent runtime source count.
The earlier MAME TEMP view remains test-only. The candidate now installs the
separate shared field-presence audit described below. The later No-Intro XSI
checkpoint installs candidate declaration/value/identity checks. Namespace
capture and independently accumulated parser counts remain open.

## Executable No-Intro XSI checks — 2026-10-08 UTC

`dat_xsi.py` adds query-only projections over the eleven existing typed XSI
relations and four simple-text owner families. Twenty-two local insert/update
guards reject invalid attribute QName spelling, nil/type declarations and schema
hints. They seek only the actual native owner, edition and reading rules. Local
guards and the global audit share their declaration predicates; hint tokenization
is scoped to one attribute, not a global recursive seed. No source values,
effective types, collapsed text, URI pairs or document IDs are stored twice.

The publication audit also checks strict selected scalar values, including the
default header xs:int when no type is declared. Compatible mode skips these
value checks, but still checks type QName/derivation and hints. Strict ID/IDREF
closure uses one edition-partitioned identity pass: duplicates fail and forward
references may be completed before publication. A matching ID in another edition
does not resolve a reference. Published native facts retain their existing freeze.

`xml_numeric_sql.py` is shared by strict DAT file sizes and narrowed scalar
integers. It checks significant decimal digits before casting, including signed
minimum, overflow and negative zero. `xml_text_sql.py` supplies XML-only whitespace
collapse, Unicode XML Name/QName and language checks using SQLite built-ins.
Neither helper introduces a UDF or application-defined input-size cutoff.

```sh
python3 -Werror::ResourceWarning docs/schema-candidate/xml_numeric_sql_check.py
python3 -Werror::ResourceWarning docs/schema-candidate/xml_text_sql_check.py
python3 -Werror::ResourceWarning docs/schema-candidate/dat_xsi_check.py
```

These are constructed candidate checks, not an imported-source proof. Prefix
spelling cannot prove namespace binding. The future checked reader must capture
the XSI attribute QName and resolved built-in type while namespace context exists,
independently accumulate accepted declarations/counts, reach accepted EOF, and
atomically finalize the edition. Skipped vendor subtrees do not acquire invented
typed owners. Full-model review, user approval and production cutover remain open.

Current constructed receipts: twelve XSI methods pass in 11.824 seconds, including
local guard rollback and real publication refusal/repair/freeze. Five numeric,
five text, six native-size and eight native-qualification methods pass; the
integrated 29-method checker passes in 23.026 seconds. Independent No-Intro field
witnesses remain 284 assertions/24 expected rejections plus legacy 18/4, and five
audit-wiring checks pass. Assembly prepares 93 closed native kinds and 70 views.
The 512/1,024 unrelated ID/IDREF-pair regression checks fixed-edition simple/XSI/
identity work and global identity scaling; it is not a whole-catalog latency or
authentic import benchmark. The corruption harness previously took 124.979
seconds because rollback after trigger DDL repeatedly reset SQLite's schema
cache; the separate committed in-memory copy fixes that harness cost without
removing cases. No production import speedup is inferred from these test times.

## Executable field-position publication checks

Four `*-field-presence.tsv` manifests map 457 distinct physical table/code pairs
to closed native presence predicates: MAME 134, software 37, Logiqx/CMP 85 and
No-Intro 201. These build-time definitions do not become catalog rows. Values,
default-presence flags, hash declarations and relationship literals remain on
their typed owners; the generated audit is query-only.

`candidate_field_presence_problems` compares each retained native field with
the exact owner/code position count and occurrence. Its results feed the
edition publication gate. Drafts may temporarily mismatch and be repaired;
publication rejects missing, invented, extra or noncanonical positions for
independently retained fields, then
existing immutability guards freeze the accepted facts and positions. Joined
registry/group scopes permit edition-filtered indexed reads while the global
audit still visits detached owners with an unknown edition.

Thirty-seven No-Intro XSI routes have value and position in the same typed row.
Their actual `(owner,field_kind)` primary key supplies implicit occurrence zero;
no synthetic occurrence column is added. This structural closure does not
interpret `nil`, `type` or selected reading-rule semantics. Nor can any predicate
infer an optional source field erased together with its position; independent
source counts/revalidation have their separate, documented limits.
CMP token-only flags and present-empty P/C `languages` likewise use the position
as their sole presence marker, not a fabricated duplicate value or presence bit.

Run generator attacks and each family's native presence checks:

```sh
python3 docs/schema-candidate/presence_check.py
python3 docs/schema-candidate/mame_presence_check.py
python3 docs/schema-candidate/software_presence_check.py
python3 docs/schema-candidate/logiqx_cmp_presence_check.py
python3 docs/schema-candidate/no_intro_presence_check.py
```

The main integrated regressions exercise actual missing/invented root positions,
repair-before-publication, published immutability and isolation from another
edition's incomplete draft. The generator tests additionally cover empty versus
absent, explicit defaults, equal-count owner substitution, extra/nonzero
occurrences, detached owners, invalid manifest references and inventories above
SQLite's compound-select limit. The retained-field checkpoint passed 29 integrated
tests in 6.280 seconds with 41 views. The later child/seal snapshot prepares
45 views and passed 29 integrated tests in 5.748 seconds. Ten generator tests
and 55 native presence test methods pass, with hundreds of route/state mutations.
An ancestry contradiction is reported to both known editions; an edition
identified only by the surviving parent is retained, while truly unknown
orphans remain global-only findings.
MAME and software controls distinguish the exact CRC declaration from SHA-1,
other hash fields and nonzero occurrences. Deliberately broken predicates fail
the native-presence assertions without unrelated FK/UNIQUE setup failures.
Fresh GPT-6.1 Sol medium core and four-family native fix/re-review loops are
bounded CLEAR for this field-position pass. Neither approves complete source
capture/counts, PCDATA cardinality, XSI semantics, corpus or production cutover.

## Native child cardinality and export count seals

The assembler explicitly loads all four `*_cardinality.sql` fragments and
requires their exact `(problem,owner_id,edition_id)` view interfaces. Missing or
misspelled views, wrong columns and unresolved native dependencies fail assembly;
each family audit participates in the actual edition publication gate. Existing
format integrity views and local singleton keys remain part of that gate.

The MAME audit requires machines and each machine's description. Strict DTD
machine/switch/device child order and required ROM size are checked separately
from observed-compatible order and optional size. Its prior-maximum windows
avoid pairwise sibling comparisons. Software
list/title/area cardinalities move from the older integrity view to their one
canonical audit, preserving empty required text, optional singleton notes and
compatible empty areas/switches. CMP requires at least one game/set form;
Logiqx's existing mode-specific text rules remain in its format integrity view;
its strict root/header/game child order has its own candidate checks. CMP
lexical ordering uses adjacent ordered anchors instead of quadratic pairwise
joins, including nested fields. No-Intro strict DAT ROMs require their
size/CRC/MD5/SHA-1 declarations; all four DAT modes require the header before
games, while strict header/game child order does not constrain compatible
children. V3 compatible mode correctly permits trademarks/piracy. Existing DAT/P-C/export
child rules remain in their native keys and integrity view. These are not new
claims of strict XSI, numeric lexical validation or complete parser capture.

Run the independent composed cardinality and seal checks:

```sh
python3 docs/schema-candidate/cardinality_wiring_check.py
python3 docs/schema-candidate/mame_cardinality_check.py
python3 docs/schema-candidate/software_cardinality_check.py
python3 docs/schema-candidate/logiqx_cmp_cardinality_check.py
python3 docs/schema-candidate/no_intro_cardinality_check.py
python3 docs/schema-candidate/export_count_check.py
```

`export_count_check.py` independently specifies the existing 18 edition counters
and nine game/source/release counters. It perturbs each expected count, removes
each seal, tests exact nonnegative integers, deletes native relations to expose
same-sized swapped query mappings, and exercises actual publication refusal,
repair and immutability. Removing a complete optional header child and its
registry row still leaves the independent seal reporting source loss. The
mapping controls intentionally bypass deletion guards and test the count audit
alone after their explicit expected-count adjustment; they do not publish the
damaged graph or derive expectations with SELECT from inserted rows. These
constructed fixtures do not prove real parser accumulation, checked arithmetic
or EOF. The integrated count contract below covers the other formats;
checked parser producers and accepted EOF remain open.
Per-owner position counters are not added to ordinary pages.

Fresh GPT-6.1 Sol medium core/count and four-family native fix/re-review loops
are bounded CLEAR for this child/seal pass. Native child suites pass MAME 10,
software 10, Logiqx/CMP 18 and No-Intro 13 tests; the wiring and export suites
pass five and eight respectively. Independent Logiqx/CMP re-review also passes
all 14 presence tests, nine publication defect/repair controls and the unknown
orphan isolation control. None establishes full-volume acceptance, parser-fed
counts/EOF, XSI lexical semantics or complete-design approval.

## Independent source-count contract layer

The ordinary `assemble.py` compiler includes the source-count contract.
`source_counts.py` delegates to that same assembler, including for `--emit`.
Four closed `*-counts.tsv` inventories define six format-specific
wide tables, one seal row per edition, with named exact nonnegative integer
columns. No `(kind,value)` catalog data or per-owner position counters are
stored. Counts are validation facts and are excluded from semantic history.

The current inventory has 158 counters: MAME 63, software 26, Logiqx 27, CMP 6,
flat DAT 26 and synthetic P/C 10. Physical source owners and attribute-position
tables count once. Software's canonical list counts its bare or wrapped source
element, not its wrapper-placement representation a second time. Existing CMP
header-presence/comment seals remain canonical. Fixed roots need no duplicate
counter. P/C language tokens have a separate canonical-value count because
losing a token need not erase its attribute position; this is not a second wire
attribute or a claim of authentic DAT-o-MATIC P/C support.

The compiler checks count coverage against closed native owners and physical
position/XSI tables, validates actual PK components and prepares every edition
scope. It reuses the base FK/reverse, replacement and published-immutability
generators for the new tables. The main `candidate_integrity_problems` audit
includes the count audit, and the single aggregate publication-closure trigger
checks both count and existing ownership/field/cardinality failures. Existing
native publication, identity and immutability guards are retained.
Scope expressions describe persisted comparisons only: expected totals must
come from independently checked parser events, never SELECT from retained rows.

```sh
python3 -Werror::ResourceWarning docs/schema-candidate/source_counts.py
python3 -Werror::ResourceWarning docs/schema-candidate/source_count_check.py
python3 -Werror::ResourceWarning docs/schema-candidate/mame_count_mapping_check.py
python3 -Werror::ResourceWarning docs/schema-candidate/software_count_mapping_check.py
python3 -Werror::ResourceWarning docs/schema-candidate/logiqx_cmp_count_mapping_check.py
python3 -Werror::ResourceWarning docs/schema-candidate/no_intro_count_mapping_check.py
```

At the count-consolidation checkpoint both entry points emitted the same
complete 48-view candidate; the software interpretation additions below add
four query-only views. The independent MAME controls
use literal expected events, exercise actual publication refusal/repair and
detect a coherently erased optional value and position that the older audit
cannot infer. Thin compiler controls separately cover equal-sized mappings,
integer domains, FK-off ownership, replacement and edition isolation.
The four native mapping suites independently pin counter names, literal event
vectors and physical deletion targets, with populated rows for all 158 routes.
Each guard-bypassed deletion must leave the intended count mismatch; adjusting
only that expected counter then makes the source-count audit quiet. These
damaged graphs are not published. Separate positive publication controls retain
the original closure. A process-local ROM/disk SQL-body swap after valid MAME
fixture setup is detected at mapping assertions, with no SQL/setup errors.
`candidate_source_count_problems` remains available for focused diagnostics;
integrity consumers no longer need a separate audit pass to find count failures.

Fresh independent GPT-6.1 Sol medium technical and claim reviews are bounded
CLEAR after fixing both findings. All 23 focused tests pass: 12 compiler and
publication controls, three MAME, two software, two Logiqx/CMP and four No-Intro
mapping tests. One SQL-only ROM/disk mapping mutation is killed without SQL or
setup errors; its cascading assertion failures are not separate mutations.

The subsequent consolidation removes the separate assembly/audit/publication
path. Constructed fixtures supply sparse literal source-event vectors through
the test-only `count_fixtures.seal` helper, never SQLite-derived expectations.
The default-artifact regression checks all six seal tables, alias delegation
and one aggregate closure gate; optional whole-field erasure must now appear
in the main integrity audit as well as the focused count audit.

Fresh GPT-6.1 Sol medium technical and local claim re-reviews are bounded CLEAR
for this consolidation. Both fixture findings were fixed and independently
rechecked. Final normal receipts: 29 integrated tests, 14 count-compiler and
publication controls, five audit-wiring and eight export-count tests; 57 unique
MAME/software/standalone software-field tests; Logiqx/CMP 14 presence, 18
cardinality and two mapping tests; No-Intro 16 presence, 13 cardinality and four
mapping tests, plus 284 field assertions/24 expected rejections and legacy 18/4.
The software mapping suite's final two-test replay also preserves the full
integrity baseline before its deliberately destructive mapping controls.

This layer is not complete-design approval. Checked parser producers and
accepted EOF remain open.
Ordinary reads do not rescan every edition; publication and
global integrity scopes remain separate. Compensating edits that preserve a
counter still require independent source revalidation, not stronger claims
about these aggregate seals.

## Query-only software numbers and first verification lengths

The composed candidate includes one operation classifier, area-size and
ROM-size/offset numeric projections, and a first-verification-run length view.
All four are query-only. The source `loadflag` and exact numeric text remain
owned once by their native rows; no derived size, operation string or source
bytes are added to storage. The operation classifier is shared with the native
chain integrity audit.

The checked number grammar is software-specific: decimal, leading-zero octal
and `0x`/`0X` hex, no signs/whitespace/non-ASCII digits/NUL, bounded by i64::MAX.
Conversion cannot accept a partial token or SQLite's clamped overflow value.
Octal/hex accumulation strips leading zeroes and walks only bounded significant
digits. File length sums the first base/continue/ignore run, stopping at reload,
reload_plain, fill or the next base load. Invalid consumed lengths, broken
required-file/load-step ownership and overflowing sums project NULL rather
than a usable prefix. A draft result is provisional, not a parser EOF receipt.

```sh
python3 -Werror::ResourceWarning docs/schema-candidate/numeric_sql_check.py
python3 -Werror::ResourceWarning docs/schema-candidate/software_numbers_check.py
python3 -Werror::ResourceWarning docs/schema-candidate/software_file_lengths_check.py
```

These constructed checks cover literal boundaries, exact native lexeme
retention, operation/ownership corruptions and indexed point-read plans. SQLite
instruction controls test that unrelated native declarations/chains do not
increase point-query work. They do not establish real-import timing, all-format
numeric equivalence, checked source capture/EOF, executable recipe behavior or
qualified shared UUID assignment. See `software-notes.md` for the remaining
qualification and publication obligations.

## What the checks establish

The shared witnesses exercise binary UUID width/type; edition identity including
coverage; repeated import attempts; hash algorithm/byte lengths; empty, invalid
and valid digest states with lossless source spelling; receipt ancestry; nullable
diagnostic facts and byte-indexed UTF-8 excerpt highlights; fractional integer
rejection; and forward/reverse/alternate-key replacement attacks with foreign
keys both enabled and disabled. Populated shared-fact lookups check their actual
index plans, not source-history projections. They do not time a real importer.

The assembler prepares every view and checks FK targets on an empty composed
schema. Native manifests give a concrete kind-to-owner/parent inventory. The
global reverse audit enumerates each declared FK independently of registry seeds,
so detached common bases, facets and positions are not automatically hidden by
a missing registry entry. Its unknown edition remains unknown. Publication
checks are edition-scoped; they are not a claim to intercept arbitrary COMMIT.

Collision guards reject primary and alternate unique-key collisions before
SQLite can run REPLACE's implicit deletion, including when recursive triggers
are disabled. Candidate batch writers must deduplicate with set-based
`INSERT ... SELECT ... WHERE NOT EXISTS`; blind `INSERT OR IGNORE` and UPSERT
are not a substitute for that identity policy. This is a candidate writer
contract requiring performance/implementation review, not a claim that the
current application already follows it.

## Source capture and publication design

The proposed [publication contract](publication-contract.md) makes the private
writer states, completed-batch visibility, independent source counts, accepted
EOF, reuse and confirmed/uncertain commit outcomes explicit. Format-specific
capture is specified in [MAME](mame-capture-contract.md),
[Logiqx/CMP](logiqx-cmp-capture-contract.md) and
[No-Intro](no-intro-capture-contract.md); software's existing
[notes](software-notes.md) retain its native load-chain and physical-root
contract. Exact fields, counter names and parent keys remain single-owned by
their canonical TSV inventories, not recopied into these documents.

These specifications are reviewable design, not runtime proof. In particular,
DAT retains every XSI attribute's source QName on its existing row; resolved
type kind exists only on the four simple-owner relations. A NUL-repaired parser
buffer is not another stored byte view. Root ends require actual closing
events, and successful prefix batches never substitute for accepted EOF.
No reader, writer or schema has been cut over by these documents.

Two independent GPT-6.1 Sol medium reviews cover the publication/No-Intro and
MAME/Logiqx/CMP contract slices. Fix/re-review makes MAME's byte versus coordinate
view names and half-open root end explicit. Strict-only DAT identity checks and
new-edition-only publication insertion are explicit, not imposed on compatible
mode or reused editions. Assembly still prepares 93 native kinds and 70 views;
local Markdown links and whitespace checks pass. This documentation checkpoint
does not rerun parser tests, imports, profiles or the production gate, and is not
a complete-design clearance.

## Source-free consumer reconciliation

[The shared/export query contract](query-contract.md) maps issued file identity,
generation-bound cursors, compatibility reads, native export hydration, build/
audit/reconciliation/dependency consumers, diagnostics, history, integrity,
paired backup and explicit external source recovery to actual candidate owners.
Its identity/response-shape changes and publication-time ordering are explicit
proposals, not backward-compatibility or implemented-reader claims.
[MAME/software](mame-software-consumers.md) and
[Logiqx/CMP/DAT/synthetic P/C](logiqx-cmp-dat-consumers.md) map each native returned
family, field/default/position meaning, parent-local sequence and semantic
history disposition without duplicating the canonical field ledgers.

Independent GPT-6.1 Sol medium review/fix/re-review is bounded CLEAR for these
consumer contracts. Corrections retain export details' independently observed
opening-tag endpoints on the two actual details owners; fix source-order page
keys, software notes ownership/area naming, and current-versus-new CMP/root-SHA1
capture; and account for catalog name selectors, build metadata/parents and
repeated-root-name rejection. The two new endpoint regressions went RED for
missing columns then GREEN, including checked coordinates and publication
freeze. All ten export-count methods and sixteen No-Intro presence methods
pass, plus 284/24 field and 18/4 legacy assertions/expected rejections. The full
count suite takes 51.488 seconds; this is recorded evidence, not an acceptable
runtime performance claim or a reason to reduce its cases.

These are design/constructed checks only. They do not prove authentic parser
capture, current application use of the target, all-format query/history parity,
production import performance or complete-design approval.

Run the constructed export access-path witness with:

```sh
python3 -Werror::ResourceWarning docs/schema-candidate/export_query_check.py
```

Its twelve methods pass in 6.400 seconds (6.283 seconds class setup), retaining
512 unrelated native games with dump-source and release-file owners, `ANALYZE`,
all guards during positive population and separate committed corruption-copy
DDL. Game-page work is 203→214 SQLite VM instructions before/after population;
other owner/position/hash/file-ancestry branches have indexed/PK plan checks,
not independent VM-scaling claims. No new index or copied ancestry is needed
for these bounded queries. Publication's cursor-evidence lookup uses an edition-
PK probe rather than a planner-selected publication-table scan.

`GAME_ANCHOR` and `REQUESTED_MEDIA` are evidence projections, not complete public
validators: they do not check game-name positions/all intermediate native kinds,
and the latter does not join common media. Tests prove missing/reparented
evidence remains visible, not that a full reader rejects every corrupt path.
Commit before SQLite backup prevents busy retry; cleanup is registered as each
connection opens; 500 visible rows continue after the last visible row, not the
lookahead. Independent Sol fix/re-review is bounded CLEAR for the witness.

## Native diagnostic containment and independent byte bounds

`diagnostic_links.py` shares one half-open containment predicate with physical
root links. Its closed eleven-route export projection checks actual typed
ancestry and format, rather than accepting registry FKs alone. Every available
comparable proof must agree. Link insert/update and scoped reverse checks reject
invalid linked draft rewrites; the link-seeded global audit also detects missing
owners and mistyped intermediate ancestors after guards are bypassed. Export
games regain their independently observed optional paired end coordinates;
their opening position remains owned once by `catalog_sets`. Opening-only
owners cannot borrow parent intervals. A separate raw-native coordinate audit
rejects supplied empty/reversed intervals even without diagnostic links; its
LEFT JOIN ancestry preserves globally unattributable corrupt rows.

`physical_extents.py` independently validates supplied byte extents on all
twelve native extent-bearing tables against the selected original/decoded
length. Publication does not depend on a diagnostic happening to reference a
bad range. All-NULL unavailable extents remain unavailable; missing decoded
length cannot fall back to original length. This adds a query-only audit, not
stored bytes or another extent/ancestry table.

```sh
python3 -Werror::ResourceWarning docs/schema-candidate/ordinary_diagnostics_check.py
python3 -Werror::ResourceWarning docs/schema-candidate/physical_extents_check.py
```

These are constructed containment/publication controls, not authentic capture,
all-format child intervals, runtime timing or full-model approval. The bounds
test's twelve-table assertion verifies generated audit membership; behavioral
publication/boundary cases exercise the populated software fixture. The ordinary
point-read control retains 512 unrelated games, sources, files, details and
serials with guards enabled and `ANALYZE`; it is a bounded instruction/plan
check, not a production benchmark.

## Sealed relationship meaning and receipt provenance

`relationship_closure.py` supplements the existing source-relationship routing
with user/derived assertion closure. Evidence sealing and accepted reviews need
the origin-appropriate payload and exactly one compatible typed endpoint for
each target. Actual set/media targets require matching native kinds and a
published edition; other endpoint facts retain their real typed references.
Sealing freezes assertion/evidence children, endpoint meaning and referenced
inference rules, including late insertion and draft-to-sealed child moves.
An unresolved target need not have a published edition, but sealing freezes
that edition's defining context; unsealed draft context remains editable.
The separate audit finds broken sealed payload/endpoint closure after guard
bypass. It does not infer the truth of a user's relationship assertion.

`receipt_ancestry.py` preserves receipt/source/publisher agreement in both
reference directions and freezes receipt provenance once used by a published
edition or terminal import. Terminal imports cannot detach, downgrade, delete
and reattach to evade that freeze; running attempts may finish normally.
Unreferenced receipts remain editable. Existing shared ancestry audits are
reused rather than copied, with reverse indexes for receipt-reference lookups.

```sh
python3 -Werror::ResourceWarning docs/schema-candidate/relationship_closure_check.py
python3 -Werror::ResourceWarning docs/schema-candidate/receipt_ancestry_check.py
```

These constructed lifecycle witnesses are not production finalizer/recovery
proof. Any thin or publication-bypassed fixture is explicitly labelled and
cannot establish complete native publication closure.

This checkpoint's main receipts: 14 ordinary diagnostic tests (14.425s), six
byte-bound tests (10.554s), eight receipt tests (4.893s), and thirteen relationship
tests (0.429s). Independent configured GPT-6.1 Sol medium fix/re-reviews are
bounded CLEAR for those slices, including normal linked-owner rename/deletion,
terminal-import detach/downgrade/delete and unresolved-edition context bypasses.
Assembly prepares 93 native kinds and 75 views. The integrated 29-method suite
passed in 24.659s before the last small guard deltas; later independent fix
controls and final assembly cover those deltas, not a rerun of the full gate.
No-Intro presence remains 16/16 (17.637s), export queries 12/12 (7.536s), and the
focused export-count publication/freeze control 1/1 (6.736s). These constructed
timings include schema/fixture setup and concurrent work, not import acceptance.

## Still required

Before design approval, settle every supported field/owner/state/order contract,
typed keys and ancestry, count events/scopes, required parser capture, checked
accumulation and EOF/failure-transition specifications, qualification/UUID rules,
diagnostics and query semantics. Prove composed candidate closure with
independent constructed witnesses, corruption controls and populated target
plans. Read the format notes for concrete remaining gaps rather than inferring
coverage from the existence of a table or a green test count.

One newly confirmed consumer gap remains: the current application has real
`no_intro_archive_targets` and source-free archive endpoint round trips, but the
candidate's seven-type endpoint registry omits that native owner. Reconcile the
eighth typed endpoint and its publication/closure/freeze/query contracts before
complete-model clearance; repeated publisher archive numbers must not collapse
actual archive owners. Do not silently replace them with set/name endpoints.

The shared-file evidence layer now has candidate hash and size projections,
native-file qualification and shared-fact refresh triggers; the earlier gaps
described here are historical. The candidate records one immutable byte-
contract role per reading-rules row, projects eight native size sources, and
refreshes hash and size membership for affected canonical components and their
aliases. These SQL proposals still need completed qualification review and a
production writer; they do not establish parser-fed facts or authorize UUID
assignment in the application.
Historical checkpoint before the 2026-10-08 native-file candidate addition:
the shared-file evidence layer was incomplete. Its accepted
hash view relied too heavily on a `whole_file` tag and excluded `whole_asset`
without the native byte-boundary qualification required by the dictionaries.
At that checkpoint there was no complete native source-size witness projection
or size-membership refresh. The shared UUID FK alone did not prove native file
eligibility, and
the review refresh still needed exact affected-component/redirected-alias
cleanup and hash/size consistency controls. The software length view was an
input to that work.

Missing parser capture and checked numeric/load interpretation are separate from
constructed SQL proof. No generated SQL can manufacture a verified EOF, complete
root extent, authentic producer grammar or source-byte mapping. After design
approval, a Rust writer must implement and verify the complete batch publication
path, including parser capture, checked producers, accepted EOF, finalizer/
rollback behavior, source mapping, qualification and shared-fact maintenance.
Full-model guards, authentic capture/EOF evidence, corpus and performance
acceptance remain open. No production schema, importer or writer has changed as
part of this isolated candidate work.

After reconciling the complete dictionaries and collector-facing diagrams with
this artifact, obtain a fresh complete-design Sol review and explicit approval
of an identified proposal revision. Then implement the greenfield cutover,
regression/corpus/performance checks, complete `devenv test`, GPG-signed commits and
push. Existing database fingerprints are rejected, not migrated.

## Native file byte facts — isolated candidate, 2026-10-08

`catalog_file_byte_contracts` binds a reading-rules row to one of six closed
roles: MAME machine ROM, MAME software file, complete declared Logiqx file,
ClrMamePro declared asset, unfiltered No-Intro DAT file, or the synthetic
No-Intro P/C fixture asset. The immutable contract is an explicit trusted
producer assertion, not a certification or a dialect/version checker.

`candidate_native_file_sizes` exposes eight native size sources: MAME ROM,
software file length, ClrMamePro ROM, Logiqx ROM, flat DAT ROM, synthetic P/C
ROM, database-export dump file and database-export release file. Each row keeps
the native field selector, a queryable byte length when representable, and an
`omitted`, `value` or `unusable` state. The original lexeme remains on its
native owner. Numeric projections are bounded by SQLite's signed 64-bit
integer; a valid unsigned value above `i64::MAX` is therefore `unusable` in
this projection, not a native parser rejection. Strict flat-DAT sizes use the
candidate `xs:unsignedInt` projection. Compatible DAT and export size
projections use conservative ASCII-decimal comparison while preserving the
native text. These projections do not replace native validation.

`candidate_native_file_byte_coverage` requires a matching immutable byte
contract and eligible native owner. `candidate_qualified_file_hashes` exposes
each individually valid, position-backed whole-file/whole-asset hash, even
when another supplied field is malformed. The stricter automatic eligibility
view, `candidate_native_file_qualification`, also requires a usable/omitted size
and all supplied hashes to be valid and mutually consistent. Thus a malformed
size or CRC blocks automatic identity but does not erase a valid SHA-1 review
witness. Published match decisions and edition publication
rebuild both shared hash and size membership over the affected canonical
component, including issued aliases, and check that stored membership equals
the accepted witnesses. These are composed SQL candidate contracts, not a
Rust batch writer or production autoassignment path.

Run the focused candidate checks with the repository's active devenv Python:

```sh
python3 docs/schema-candidate/native_file_sizes_check.py
python3 docs/schema-candidate/native_file_qualification_check.py
python3 docs/schema-candidate/shared_file_facts_check.py
```

The native-size checker passes six tests. A separate Sol size review ran 4,284
additional lexical, range and full-view assertions. That bounded review was
CLEAR across all six
policy families, 30 cross-family negative controls, and immutability/retrofit
controls. The native qualification suite now includes separate media-ID,
reported-hash-ID and UUID-only lookup controls over actual native rows. Its
focused plan regression passes after 1,024 unrelated MAME ROM/hash/position
declarations: hash/media 1,585→1,586 instructions; hash/reported 1,545→1,545;
strict/media 9,296→9,299; accepted hash/UUID 1,577→1,578; accepted size/UUID
4,102→4,102. The role view uses one keyed media root with closed typed-owner
checks, not a union that can become a global coroutine. CMP `nodump` preserves
its existing qualified-digest behavior. The final eight-method replay passes
in 19.613 seconds and native Sol re-review is bounded CLEAR. Independent reads
remain bounded with 2,048 unrelated declarations and a target inserted last;
these constructed MAME plans are not all-format corpus timing.
The shared-facts suite has fourteen tests using thin size/qualification adapters
and took 1.461 seconds.
They include incoming-hash rollback, explicit rejection of a contradictory
component merge, independently injected cross-registry edges, refusal to
extend a corrupt redirect cycle and the published-source review boundary. Its
1024-unrelated-witness control measured point-query VM work from 980 to 981
instructions and publication work from 7519 to 6725. These figures show
bounded maintenance-adapter work only; they do not measure composed native
qualification, a Rust writer, a production import, corpus behavior or runtime.

Separate current checkpoints: the integrated candidate has 29 passing tests in
22.931 seconds, source-count has 14 in 22.988 seconds, and wiring has five in
0.004 seconds. Wall times vary with concurrent activity and are not performance
claims. Full-model guards, producer capture, accepted EOF, complete-batch Rust
writing after approval, corpus gates and production/performance evidence remain
open. Candidate evidence views retain draft facts for intended completed-batch
matching; private Rust batch isolation/freezing remains unproved. Review
publication requires every affected
linked occurrence, conflict incoming occurrence and explicit hash/size witness
owner to belong to a published edition. Draft collection remains allowed; a
refused decision may publish after its participating editions publish. This
separates review from mutable drafts without hiding completed-batch matching
facts. Private Rust batch capture freezing and parser/EOF proof remain open.
These checks do not alter the application schema or importer.
