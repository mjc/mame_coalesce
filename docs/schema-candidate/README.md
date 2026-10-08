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
The No-Intro fixture instead requires the composed DDL first:

```sh
{ python3 docs/schema-candidate/assemble.py --emit && cat docs/schema-candidate/no_intro_witnesses.sql; } | sqlite3 -bail :memory:
```

## Bounded review checkpoint — 2026-10-08 UTC

The integrated harness passes 20 tests in 3.727 seconds; all four native fixtures
pass. Assembly prepares 93 closed native kinds and 30 views with a clean
empty-schema FK check. Independent GPT-6.1 Sol medium review/fix/re-review loops
are CLEAR for the bounded core and native slices. Native re-review additionally
ran 52 adversarial assertions covering the six original findings and No-Intro
relationship-position corrections. These are not complete-design approval or
authentic importer/corpus/performance proof. The outstanding work below remains.

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

This artifact remains incomplete until independently verified all-format
field-presence/default/position closure, exact relationship-kind/position closure,
all root modes and diagnostic containment, independently parser-fed count seals,
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
