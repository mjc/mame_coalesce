# Relationship candidate — isolated design

Status: incomplete candidate. This fragment is not production implementation,
does not describe a migration, and does not close MAMEC-62. Its shared-table
names and types are read from `shared.sql` as it exists in this checkout.
`relationships.sql` owns relationship identities, typed endpoint identities,
append-only relationship reviews, issued file-UUID redirects, and the
relationship worker's hash-evidence qualification/review logic.
`relationships_guards.sql` is a candidate guard fragment. The main assembler
loads both fragments; neither is wired into the application schema.

Source literals remain on the typed native declarations authored by the format
workers. Their `relationship_id` points to `reported_catalog_relationships`;
the shared registry stores only the closed reported kind and source edition.
Canonical QName/token positions remain position-only facts owned by the format
tables and point back through `relationship_id`; they do not copy the
literal. This design adds no generic owner discriminator, serialized endpoint,
copied literal, digest bytes, UUID payload, or per-list hash/size fact.

## Source pages and sequences read

| Page | Sequence | Sections used |
|---|---:|---|
| [MAMEC-DOC-8](https://lific.mjc.lol/MAMEC/pages/91) | 37903 | ROM sets/media/shared files and accepted witnesses; relationship model; reviewed file-match settlement/UUID redirects |
| [MAMEC-DOC-21](https://lific.mjc.lol/MAMEC/pages/138) | 38132 | Common identity and publication; native identity/position conventions |
| [MAMEC-DOC-23](https://lific.mjc.lol/MAMEC/pages/140) | 38140 | All 22 reported kinds, current literal/position owners and source behavior; candidate owner/position mapping; closure gaps/review boundary |

Current table shapes were read with sem from `file_match_reviews.sql`,
`catalog_registry.sql`, `shared_file_facts.sql`, `reported_relationships.sql`,
`relationship_targets.sql`, `relationship_reviews.sql`,
`relationship_evidence.sql` and `mame_relationships.sql`. The three requested
design pages were read by bounded sections, not as whole-page dumps.

## Reported kind crosswalk: current owner to candidate owner

The target literal owner is the native typed declaration table in each row;
the shared registry does not copy the literal. For each XML source attribute,
the target canonical key is `(source_element_id, field_kind, field_occurrence=0)`.
CMP scalar positions instead use their actual set/media owner plus field code
and occurrence zero. A target position row carries
`relationship_id`. Native fragments now provide these typed FKs and selected
field-to-declaration checks; exhaustive bidirectional closure for all 22 kinds
remains required.

| Kind | Current literal owner → candidate typed declaration | Current canonical position → target position | Current behavior retained |
|---|---|---|---|
| `mame_cloneof` | `mame_machine_links(set_id,link_kind)` → MAME machine-link owner | `mame_machines_attribute_positions(set_id,'cloneof',0)` → machine source element/field | Absent means no row; empty and unresolved target text are retained. |
| `mame_romof` | Same table, kind `romof` → MAME machine-link owner | `mame_machines_attribute_positions(set_id,'romof',0)` → machine source element/field | Absent means no row; empty and unresolved target text are retained. |
| `mame_sampleof` | Same table, kind `sampleof` → MAME machine-link owner | `mame_machines_attribute_positions(set_id,'sampleof',0)` → machine source element/field | Absent means no row; empty and unresolved target text are retained. |
| `mame_device_ref` | `mame_device_references(source_element_id).name` → typed device-reference child | `mame_device_references_attribute_positions(source_element_id,'name',0)` → child name field; child placement/order stays separate | `name` and `tag` must be present; empty values are accepted; no target lookup. |
| `mame_rom_merge` | `mame_rom_merges(media_entry_id).merge_name` → typed MAME ROM merge owner | `mame_rom_claims_attribute_positions(media_entry_id,'merge',0)` → ROM media field | Absent means no row; empty text is retained; target is unresolved. Current line/column describe the element, not the merge QName. |
| `mame_disk_merge` | `mame_disk_merges(media_entry_id).merge_name` → typed MAME disk merge owner | `mame_disk_claims_attribute_positions(media_entry_id,'merge',0)` → disk media field | Same absent/empty/opaque behavior as ROM merge; current line/column are element coordinates. |
| `logiqx_cloneof` | `logiqx_set_links(set_id,link_kind).target_name` → Logiqx game-link owner | Logiqx game attributes code 3 → game source element/code 3 | Absent means no row; empty is retained under the selected XML mode; no target lookup. |
| `logiqx_romof` | Same table, kind `romof` → Logiqx game-link owner | Logiqx game attributes code 4 → game source element/code 4 | Absent means no row; empty is retained; XML mode rules apply; no target lookup. |
| `logiqx_sampleof` | Same table, kind `sampleof` → Logiqx game-link owner | Logiqx game attributes code 5 → game source element/code 5 | Absent means no row; empty is retained; XML mode rules apply; no target lookup. |
| `logiqx_device_ref` | `logiqx_device_references(set_id,reference_order).target_name` → typed device-reference child | Device-reference attributes code 0 → child source element/name field; placement/order is separate | `name` is required and may be empty; no target lookup. |
| `logiqx_rom_merge` | Current `logiqx_file_merges(occurrence_id,claim_kind=logiqx_rom)` → typed Logiqx ROM merge owner | Logiqx ROM attributes code 5 → media entry/code 5 | Absent means no row; empty is retained if accepted by XML mode; no target lookup. Retire claim-kind routing in the candidate. |
| `logiqx_disk_merge` | Current `logiqx_file_merges(...,claim_kind=logiqx_disk)` → typed Logiqx disk merge owner | Logiqx disk attributes code 3 → media entry/code 3 | Same absent/empty/mode behavior as ROM merge; no target lookup. |
| `clrmamepro_cloneof` | `clrmamepro_set_links(set_id,link_kind)` → typed CMP set-link declaration | Current set positions `(record_id,1)` → actual set owner/code 1/occurrence 0 | Absent means no row; token spelling, quoting and empty value are retained; malformed syntax fails; no target lookup. |
| `clrmamepro_sampleof` | Same current set-link table, kind `sampleof` → typed CMP set-link declaration | Current set positions `(record_id,6)` → actual set owner/code 6/occurrence 0 | Same token behavior; preserve token order/location along with value. |
| `clrmamepro_rom_merge` | `clrmamepro_rom_merges(occurrence_id).merge_name` → typed CMP ROM merge owner | Current ROM positions `(occurrence_id,6)` → media entry/code 6/occurrence 0 | Absent means no row; quoted empty is retained; malformed syntax fails. Set-body ROM placement remains a separate position. |
| `software_cloneof` | `software_clone_links(set_id).target_name` → typed software-title clone owner | `software_title_attribute_positions(set_id,1,0)` → title clone field, with `relationship_id` | Absent means no row; optional empty attribute is retained; no target lookup. |
| `no_intro_dat_cloneof` | `no_intro_dat_set_links(set_id,link_kind=cloneof).target_literal` → typed DAT game-link owner | `no_intro_dat_game_field_positions(set_id,'cloneof',0)` → game clone field | Absent means no row; optional literal including empty is retained; no target lookup. |
| `no_intro_dat_cloneofid` | Same table, kind `cloneofid` → distinct typed DAT link owner | `no_intro_dat_game_field_positions(set_id,'cloneofid',0)` → game cloneofid field | Absent means no row; opaque publisher ID including empty; never coerce to cloneof. |
| `no_intro_database_archive_clone` | `no_intro_archive_clone_links(archive_id).declared_target_number` → typed archive clone owner | `no_intro_archive_field_positions(archive_id,'clone',0)` → archive clone field | Absent means no row; `P` is a marker without relationship ID; all other text, including empty, remains literal. |
| `no_intro_database_archive_mergeof` | `no_intro_archive_merge_links(archive_id).declared_mergeof` → typed archive mergeof owner | `no_intro_archive_field_positions(archive_id,'mergeof',0)` → archive mergeof field | Absent means no row; text including empty is an alternate-representation literal, not media merge or numeric target. |
| `no_intro_pc_clone` | `no_intro_pc_clone_links(set_id).target_archive_id` → typed fixture-game clone owner | `no_intro_pc_game_attribute_positions(source_element_id,'clone',0)` → game clone field | Absent means no row; `P` is a marker without relationship ID; otherwise require nonempty ASCII digits and retain leading zeroes. |
| `no_intro_pc_mergeof` | `no_intro_pc_merge_links(set_id).target_archive_id` → typed fixture-game merge owner | `no_intro_pc_game_attribute_positions(source_element_id,'mergeof',0)` → game mergeof field | Absent means no row; require nonempty ASCII digits and preserve spelling; distinct from clone and media merge. |

The same-edition ancestry follows each actual typed parent to its set group and
edition. Media merge ancestry follows the typed media owner, then its actual
machine/game/set parent. Child-element order and coordinates are not copied
into attribute-position rows. None of these source literals is resolved to a
target record by this design. The relationship audit view verifies registry,
typed literal owner, kind, and edition parity. Native position FKs now use
`relationship_id`; their existence is not proof of exhaustive required-position,
literal-owner, kind and edition closure for all 22 kinds.

## Endpoint and identity choices

- `catalog_relationships` is the shared identity issuer for source, derived,
  and user assertions. The stable caller key is stored once. Source identities
  have an edition; derived and user identities do not fabricate source
  editions.
- `reported_catalog_relationships` carries one of the 22 closed kinds. Native
  declarations own literal values; no generic EAV/source-owner table is added.
- Target identity and payload use a closed kind registry with typed subtype
  tables for actual set, media entry, issued shared-file UUID, declared hash,
  observed-content hash, unresolved catalog literal, and external record.
  Equal hash bytes can use one `hash_values` row while declared and observed
  endpoint identity stays distinct.
- A shared-file target keeps its originally issued UUID. Redirect resolution
  is separate. An observed digest never allocates or aliases a file UUID;
  SHA-1/SHA-256 alone qualify as strong whole-file identity evidence. CRC32 and
  MD5 can remain hash membership facts but are not identity bridges.
- Inferred/manual assertions have typed `from_target_id` and `to_target_id`;
  rationales, comparisons, ordered support, and append-only reviews remain
  separate relations. Review visibility follows the latest published review,
  so a newer unfinished draft does not hide it.

## Shared file facts and review semantics

`shared_catalog_files`, `file_id_registries`, `hash_values`,
`catalog_entry_hashes`, `shared_file_sizes`, and `shared_file_hashes` are owned
by `shared.sql`; this fragment does not redeclare them. The accepted hash view
qualifies only valid, whole-file source declarations with an issued UUID and
excludes only the exact published rejection. The SHA-1/SHA-256 identity view
is narrower. Duplicate witnesses collapse only when deriving membership; the
source declarations remain available for explanation.

File-match decisions preserve the append-only keep-separate/merge model:
immutable conflict evidence, exact hash/size witness dispositions, one
terminal published outcome per conflict, same-registry UUID redirects, and
cycle-free resolution through issued aliases. A merge never rewrites a source
UUID or gives an unlinked incoming entry an identity. A published rejection
permanently excludes only that named source assertion; later accepts cannot
restore it. Hash memberships are rebuilt for the affected canonical redirect
component within publication from the full accepted-evidence view, including
qualifying facts in earlier completed editions as well as the current batch.
Edition publication also adds that edition's qualified facts through the
current canonical UUID. Redirects retain the old issued UUID and follow chains
rather than rewriting alias history. Hash membership differences are reported
by `catalog_shared_fact_mismatches` and surfaced by
`candidate_relationship_integrity_problems`.
Before decision publication, every hash and size witness attached to each
settled conflict must have its own exact accept/reject disposition. This keeps
the final witness from being silently omitted; the integrity view reports the
same issue if a published row set is damaged outside that guard.

The size half is intentionally not claimed complete. `shared.sql` currently
has no common source-size witness relation, and the format fragments expose
raw sizes on different typed owners (software also has a derived complete
first-load length). The candidate's conflict selector is not enough to prove
the actual native owner/field or reconstruct qualified evidence. No generic
source-owner table or duplicate size fact was added. The size evidence view,
per-component `shared_file_sizes` rebuild, and exact typed owner guards must be
designed with the native-owner manifest before this is a complete candidate.
Software has an additional complete-first-run boundary: its facts must not
qualify shared membership before its first source load is complete. The current
generic edition-publication trigger does not establish that boundary, so
software gating and the first-run backfill remain open.

## Remaining closure and proof gaps

- `candidate_relationship_integrity_problems` uses the assembler's local audit
  contract `(problem, owner_id, edition_id)` and checks source identity/kind
  presence, exactly one typed literal owner, owner kind and edition parity,
  derived/user evidence readiness, evidence payload presence, and shared hash
  membership mismatches. It does not prove each identity has exactly one
  matching canonical position or complete owner ancestry. Main's
  manifest/harness must enumerate all 22 native relations in both directions;
  each position row carries `relationship_id` without duplicating the source
  literal. The native fragments now expose these FKs, including the conditional
  No-Intro clone-marker case. Exhaustive position closure remains an integration
  dependency, not a missing-column claim.
- Endpoint subtype completeness, actual set/media edition closure, and
  published endpoint readiness need the same explicit manifest and corruption
  checks. The typed FKs here prove subtype key existence, not cross-table
  edition equality.
- The SHA-1/SHA-256 identity-evidence view separates strong evidence from
  CRC32/MD5, but the candidate does not yet guard every assignment to
  `catalog_media_entries.file_uuid` against that view. Software's complete
  first-load eligibility and all format-specific UUID-qualification rules need
  to be included in the source-owner finalizer.
- Relationship evidence seal completeness and the active replacement graph's
  publication-time cycle rule need the production-parity closure witnesses.
- Accepted incoming hash/size facts still need a published consistency check
  against the retained component before a merge can seal. The current guards
  establish witness ownership and redirect shape but do not prove that every
  explicitly accepted incoming value agrees with retained evidence.
- File-match size qualification, exact native size-field closure, rejection
  lookup and atomic size-membership rebuild remain open as described above.
- The integrated assembler now loads both relationship fragments, prepares all
  30 views and passes its empty-schema FK check. The earlier No-Intro composite
  header-key mismatch is fixed. The 20 shared/assembled tests and four bounded
  native fixtures pass, but they do not provide populated settlement/evidence
  guard coverage. No build, import, profile or production gate was run for this
  isolated design task.

## Branch-added test explanations

This branch adds `docs/schema-candidate/check.py` and format-specific SQL
witness fragments; they exercise the isolated candidate, not authentic imports
or the production gate. In `check.py`, the shared-core cases cover binary
16-byte UUIDs, edition identity versus repeated imports, coverage membership,
algorithm-specific digest lengths, integral size/count constraints, FK-off
forward/reverse/replace protections, receipt/publisher/source ancestry,
lossless diagnostic coordinates and excerpts, hash-state overrides, immutable
publication/hash meaning, reverse-import ancestry protection, cycle-free
edition lineage, and indexed shared-fact lookups. Its assembled-schema case
prepares every view and checks declared FK targets. Native `*_witnesses.sql`
fragments add local format owner/hash/position assertions; their own notes list
their bounded fixtures. `relationships_guards.sql` adds relationship-origin,
append-only review/evidence publication, conflict-side witness ownership,
settlement, same-registry redirect/cycle, and transaction-scoped hash-refresh
guards, including exact per-hash/per-size review coverage so no final conflict
witness can be omitted. Both membership rebuilds use setwise deduplication and
`NOT EXISTS`, avoiding `INSERT OR IGNORE` so pre-insert collision guards do not
abort on a duplicate witness. The complete assembled schema plus relationship
guard DDL prepared in memory; no populated relationship/file-match guard
behavior was executed. None of these cases
proves canonical relationship-position closure, complete software first-run
gating, authentic importer behavior, or the production gate.

The format SQL witnesses are bounded constructed fixtures: MAME checks
device-child order, mixed ROM/disk/sample children, hash-position links,
invalid/duplicate positions and a populated parent-order plan; software checks
root/list and part/area/ROM ownership, defaults/text/malformed numerics, closed
enums, clone/hash-position linkage and an ordered plan; Logiqx/CMP checks root
and native ownership, explicit defaults, quoted-empty scalar preservation,
sample order, hash-position linkage, invalid/duplicate rows and populated
parent/comment-coordinate plans; No-Intro checks DAT/P-C/export ownership,
hash-position links, non-media NFO hash separation, header nesting/sibling
rules, unknown extents and populated owner/position plans. Companion notes
delimit the cross-table/parser properties each fixture does not establish. The
software fixture uses minimal common identity stubs but loads the actual
`relationships.sql`; these fixtures do not prove production writer transitions.

Relevant existing test files already in the repository exercise these contracts:

| Existing test file | What it covers |
|---|---|
| `tests/native_reported_relationships.rs` | Closed reported kinds, typed native owners, registry/owner consistency, publication closure, and indexed native relationship queries. |
| `tests/native_relationship_targets.rs` | Typed endpoint subtypes, digest endpoint separation, UUID target identity, resolution rules, and bounded endpoint lookups. |
| `tests/file_match_reviews.rs` | Exact conflict settlement, accepted/rejected hash and size witnesses, UUID merges/aliases, append-only publication, and rollback/error cases. |
| `tests/catalog_shared_file_facts.rs` | Source-backed shared hash/size memberships, deduplication, and retained witness behavior. |
| `tests/observed_content_endpoints.rs` | Observed whole-file digest endpoints remain distinct from declarations and never allocate catalog UUIDs. |
