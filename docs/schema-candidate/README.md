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
separate shared field-presence audit described below. No-Intro XSI semantic
enforcement and independently accumulated parser counts remain open.

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
or EOF. Expected-count schemas and producers for the other formats remain open;
per-owner position counters are not added to ordinary pages.

Fresh GPT-6.1 Sol medium core/count and four-family native fix/re-review loops
are bounded CLEAR for this child/seal pass. Native child suites pass MAME 10,
software 10, Logiqx/CMP 18 and No-Intro 13 tests; the wiring and export suites
pass five and eight respectively. Independent Logiqx/CMP re-review also passes
all 14 presence tests, nine publication defect/repair controls and the unknown
orphan isolation control. None establishes full-volume acceptance, parser-fed
counts/EOF, XSI lexical semantics or complete-design approval.

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

## Still required

This artifact remains incomplete until complete parser-fed field/child capture,
full native relationship publication proof, all root modes and diagnostic
containment, independently parser-fed count seals,
qualified size/hash evidence and atomic review refresh, UUID eligibility,
ordinary-owner immutability, corruption witnesses and populated native plans are
complete. Read the format notes for concrete remaining gaps rather than inferring
coverage from the existence of a table or a green test count.

Missing parser capture and checked numeric/load interpretation are separate from
constructed SQL proof. No generated SQL can manufacture a verified EOF, complete
root extent, authentic producer grammar or source-byte mapping. Private Rust
writer transitions and real corpus/source-free query tests still have to prove
the supported publication path after design approval.

After reconciling the complete dictionaries and collector-facing diagrams with
this artifact, obtain a fresh complete-design Sol review and explicit approval
of an identified proposal revision. Then implement the greenfield cutover,
regression/corpus/performance checks, complete `devenv test`, signed commits and
push. Existing database fingerprints are rejected, not migrated.
