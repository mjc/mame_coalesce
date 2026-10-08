# Catalog import publication contract

Status: proposed design for MAMEC-62, not a Rust implementation or production
cutover. This makes the writer obligations in DOC-25 concrete. The existing
candidate SQL checks relational closure; it cannot establish parser EOF, source
capture or the outcome of an arbitrary SQL COMMIT.

## Identity and ownership

An edition is identified by the existing tuple `(catalog_id, source_file_id,
reading_rules_id, coverage_id)`. An import attempt has a fresh `import_key`,
including when it reuses an edition. Acquisition receipts remain separate. Do
not add another snapshot, import identity, stored interpretation hash or batch
table. File UUIDs remain issued 16-byte BLOBs; source element IDs and occurrence
order do not become file identity.

Before importing, retain and verify the immutable external original and its
source-file identity, selected reading rules, catalog and any acquisition
receipt. Retention/identity metadata may commit independently of the catalog
transaction. The hash/length describe the retained original, not a producer's
declared checksum, transport-decoded XML or a repaired view. A failed catalog
import does not delete the original or rewrite that identity. The selected
source-file ID, rules and coverage are fixed for the parser/writer session.
Select the XML repair and whole-file byte-contract facets, including their
absence, before an edition or import attempt references its reading-rules
identity. Once used, that identity cannot acquire either policy late; changed
repair or qualification policy requires new reading rules.

One import owns one connection and one immediate write transaction for all new
native facts, provisional file matching, conflicts, publication, import messages
and terminal successful attempt. Preflight retention and a later confirmed
failure-recording transaction are separate operations, not partial catalog
commits. No other import shares the writer connection. Public readers use
committed published editions, not its provisional facts.

The source is semantically parsed once. Retention/decompression/hash checking
may perform source I/O; this is not permission to parse XML again to recover
discarded fields, rebuild a DOM or derive expected source counts from SQL.
Borrow decoded values during the current record's lifetime where possible;
SQL bindings cannot outlive their backing input. Input/decoder buffers remain
input-dependent and require their separate performance work. Do not claim
constant total memory merely because completed native records are released.

## Private states and capabilities

The names below describe the target Rust API, not existing exported types.
Constructors and transitions stay private to parser/import modules. Distinct
ID and coordinate newtypes prevent unrelated integer or source-view substitution;
an enum distinguishes a new edition from verification of a reused publication.
No public boolean, SQL row, generic metadata map or caller-created counter vector
can construct an accepted-EOF or completed-batch proof.

```text
retained original + fixed rules/coverage + fresh attempt key
                     |
          +----------+-----------+
          |                      |
     new edition          reused published edition
     native draft         read/verify only; no native writes
          |                      |
  completed batches              |
  installed + frozen             |
          +----------+-----------+
                     | parser accepts EOF and finish checks
             EOF-checked import
                     | closure / counts / final diagnostics
             prepared commit
                     | COMMIT and outcome resolution
          committed | rolled back | unresolved
```

| Target capability | Permitted work | Transition and what is consumed |
|---|---|---|
| `ImportDraft` | Own the transaction, receive completed parser records and insert native facts for a new edition | `finish` consumes the selected parser and its independently checked events; it flushes the last pending completed records and returns `EofCheckedImport` only on accepted EOF |
| `CheckedBatch` | Carry complete native records, actual owner/order/position facts and their source-event deltas | Only the reader/checked adapter creates it after those records close; installation consumes it before matching is allowed |
| `ReusedEditionVerification` | Parse/verify the selected source and collect this attempt's messages against the exact existing edition | Cannot allocate replacement native owners, change its counts, publication time, previous-edition link or file assignments; accepted EOF joins the same finalization path |
| `EofCheckedImport` | Complete global reader checks, seal independent expected totals and check native closure | Exposes finalization, not another native-content mutation method; finalization yields `PreparedCommit` |
| `PreparedCommit` | Own the publication/attempt/message writes and affected-component checks in the still-uncommitted transaction | Only commit/outcome resolution remains; no success report escapes before confirmation |
| `CommitOutcome` | `Committed`, `RolledBack`, or `Unresolved`, with the original error and any secondary errors where applicable | An unresolved branch carries the attempt identity needed for resolution, not permission to replay or record a contradictory failed attempt |

An installed batch is never reopened to append attributes, children, hashes or
positions. Parser-global checks may still reject the whole import at EOF. Local
batch completion therefore is not document acceptance. Software matching waits
for the complete native first-run boundary and checked length/operation chain;
closing one ROM tag is not sufficient. Later complete records cannot silently
change an already issued file UUID or a frozen source witness.

The EOF-checked state is entered only after final pending records have been
installed. This distinguishes ordinary completed-batch flushing during parsing
from the final flush: waiting until EOF to write every record is not required.
No API exposes the connection for arbitrary native writes after this transition.
State types complement, rather than replace, actual SQLite guards.

## Completed-batch matching

Installing a batch has an ordered boundary:

1. Receive records whose local grammar, fields, native parents, raw ordinals,
   positions and closed-child facts have been checked. Independently accumulated
   source events accompany the records; they are not counts of successful SQL
   inserts. A compatibility-accepted malformed value remains a source fact, not
   an invented usable integer/hash or a reason to erase its position.
2. Bulk-install common identities, actual native payloads, positions, declared
   hashes and relationships. New media begin without an assigned file UUID.
   No matching query runs halfway through installation. SQL failure aborts the
   import; statement ABORT or a successful subset does not complete a batch.
3. After native closure, qualify complete-file evidence using the selected
   immutable byte contract and existing native hash/size rules. Resolve whole
   incoming key sets in bulk, including within-batch duplicates/bridges, before
   recording assignments or conflicts. Use the canonical witness policy, not
   filename equality, CRC/MD5 alone or a supplied scope label.
4. Freeze source witnesses and expose the installed completed batch to later
   matching on the same connection. Retain source facts and unresolved conflict
   evidence even when they cannot issue a file UUID. Release record buffers when
   no binding or callback still borrows them.

Matching's allowed evidence is exactly published source evidence plus completed,
frozen batches belonging to this active new edition. Exclude unrelated
unpublished editions and the batch being installed. The private writer owns this
boundary; the candidate's unrestricted draft views do not themselves prove it.
Use the active edition and completed-owner capabilities as query inputs, not a
persisted duplicate ancestry flag or batch-status column. A reused-edition
verification adds no provisional native matching evidence.

Bulk writes use set-based identity deduplication and stable prepared statement
shapes. Required existing indices start at actual incoming IDs/digest keys and
requested components; ordinary compatibility reads do not rescan the edition's
full count contract per owner. No per-row full-integrity audit, shrinking batch
to hide query cost, arbitrary document cap or whole-document serialization is
part of this design. Throughput and allocation claims require the eventual
optimized corpus measurements, not this specification.

## Independent source counts and accepted EOF

The four `*-counts.tsv` files are the closed six-family event inventories; export
uses its existing 18 edition counters and nine parent-local counters. They are
build-time contracts, not a runtime EAV table. The future producer uses typed
format-specific vectors, with every known counter explicit including zero.
Do not count common identity/facet rows as additional source elements. Physical
XSI/native positions and emitted normalized tokens follow their distinct events;
an absent attribute is not a present-empty one.

Increment only the selected parser event specified by the canonical inventory,
before handing the accepted fact to SQL. Every increment and ordinal advance
uses checked arithmetic. Expected count/order values must fit nonnegative SQLite
INTEGER before sealing/binding; exceeding its representation is an explicit
unrepresentable-input failure, never saturation, wraparound or truncation. This
is a storage-domain limit, not a tunable cap on ordinary import files.

The reader returns its private EOF proof only after the actual transport-decoded
stream ends under the selected format: all required roots/children have closed,
trailing material is accepted/rejected according to that grammar, decoder and
decompressor errors have been checked, and pending identity/reference checks
finish. Export framing and the permitted NUL repair are selected rules, not a
second parser pass. An end tag, empty batch, callback return, correct SQL count
or source byte length cannot substitute for EOF.

Source extents are captured in the existing retained-original or unrepaired
transport-decoded byte views while tokens and closing positions exist. A repaired
parser buffer is not a third stored byte view: map its offsets back to a named
view or leave byte evidence unavailable. Do not derive a complete root or
child interval from opening coordinates. Cross-view containment needs a proven
mapping; an excerpt's zero-based end-exclusive byte highlight is relative to
that saved BLOB, not a source line/column. Missing capture cannot be synthesized
by SQL or supplied by a different document. Native capture contracts give the
format-specific event and view boundaries.

## Finalization and reused editions

After accepted EOF, the writer performs these operations in the same transaction:

1. Verify the source/rules/coverage/session identity and final independent event
   vectors. Finish queued diagnostics with exact saved excerpts/highlights and
   actual ordinary/root links; a warning with no proven link remains source-only.
2. For a new edition, insert its expected-count seals and run the canonical
   native field/owner/order/hash/relationship/count closure for that edition.
   Include request-attributable errors; globally unattributable orphans remain
   global integrity findings rather than guessed membership in this import.
3. For a new edition, insert the publication marker. No native source row or position can change
   after this point. The marker is transaction-local until commit.
4. Run/check accepted hash and size membership maintenance for affected canonical
   file components/aliases. Candidate publication triggers implement part of
   this step. Source UUIDs remain issued identities; redirects do not rewrite
   them. Unexpected set mismatch or contradiction aborts finalization.
5. Finalize the fresh attempt as succeeded with the exact edition and messages,
   then prepare commit. Reset/finish pending write statements. No success is
   reported externally until commit is confirmed.

Terminal succeeded/failed import rows are immutable: they cannot detach their
receipt, downgrade to running, or be deleted and recreated to rewrite provenance.
Receipts used by a published edition or terminal attempt retain their identity
and fetch/source association. Corrections create a new attempt; unreferenced
draft receipts remain editable. A running attempt may transition once to its
terminal state after its final facts are installed.
The fetch parent is already immutable when its receipt is retained; this
stronger existing rule remains intact. Publication or terminal use additionally
seals its ordered headers and declared hash expectations, and the attempt's diagnostics
(payload, native/root links and external evidence). UPDATE cannot reparent facts
into or out of a sealed aggregate; new children cannot be appended. A running
attempt may finish constructing its diagnostics before terminalization.

For reuse, verify the exact existing published edition tuple, selected-source
parse and applicable event/message agreement. Do not reseal or repair its native
content, predecessor or stored counts. Record only this fresh import attempt and
its diagnostics/receipt association. The old edition's publication marker is
not proof that this new attempt committed.

## Failure and commit outcomes

Input rejection, representation overflow, relational validation failure,
cancellation and infrastructure failure remain distinguishable errors. A
candidate/writer invariant failure is not automatically labelled malformed XML.
Warnings about compatibility-accepted values do not turn them into rejected
input. Preserve the primary error and all secondary rollback/recording errors.

Before commit, any rejected parse/EOF, failed SQL batch, closure or membership
check requires rollback of the entire incoming catalog transaction. The
supported API cannot commit its successful prefix. On confirmed rollback and a
healthy database, a separate transaction may record this attempt as failed with
NULL edition and source-only messages/excerpts. Re-establish verified retained
identities if needed; rolled-back native IDs cannot be used for links. If failure
recording fails, return that secondary error without claiming a durable receipt.
Failure recording constructs the fresh attempt as running inside that separate
transaction, adds its final source-only diagnostics and evidence, then changes
it to failed last and commits atomically. It must not insert a terminal failed
row and subsequently append messages. No intermediate running receipt is
committed as the failure result.

| Observation | Required result/action |
|---|---|
| COMMIT succeeds | Return the exact durable successful attempt/edition; release the transaction capability |
| COMMIT returns busy while the transaction is still active | Retry only COMMIT under the connection's explicit busy/cancellation policy, or confirm whole rollback; do not replay parse/writes or report success |
| Error leaves a live transaction before successful commit | Attempt whole rollback; confirm its outcome before failure recording |
| SQLite has already rolled the transaction back | Confirm the absence of this fresh attempt's committed success; record failure only if the database is usable |
| COMMIT/connection outcome is uncertain | Inspect transaction state where usable and resolve on a healthy connection using this fresh import_key and exact source/catalog/rules/edition/message linkage; an old publication marker or an absence in a stale read transaction is insufficient |
| Resolution is unavailable or contradictory | Return `Unresolved`, preserve errors and identities, and prohibit replay/contradictory failed receipt until resolved; quarantine an unusable connection |

Resolution uses a new read transaction after the writer is known finished or a
healthy connection can establish its outcome; absence while the original writer
may still be committing is not confirmed rollback. A failed attempt is not
inserted while its original success might still commit. Do not weaken SQLite
journaling/durability to make this logic appear faster. These distinctions follow
[SQLite transaction errors](https://www.sqlite.org/lang_transaction.html) and
[autocommit state](https://www.sqlite.org/c3ref/get_autocommit.html); current driver
integration and fault-injection proof remain implementation work.

## Required implementation witnesses

| Area | Evidence required after approval |
|---|---|
| Capability misuse | Compile-fail tests for public EOF/count construction, cross-session proofs, native writes after EOF/publication and reuse writes; positive streaming/reuse/forward-reference cases |
| Independent counts | Parser-event tests for every canonical counter, accepted zero/present-empty cases, erased optional value+position, mocked arithmetic overflow and actual late EOF; no SQL-derived expected totals |
| Batch visibility | Same-batch and next-batch duplicates/bridges, exclusion of incomplete/current and unrelated drafts, software first-run completion, malformed fields and immutable prior witnesses |
| Atomic import | Inject failures after registry/native/hash/relationship/count/marker/membership/message steps; directly enumerate no surviving incoming rows or UUID mutations after confirmed rollback |
| Commit resolution | Busy retry without replay, automatic rollback, lost/unusable connection, durable success found by fresh import_key, stale-read absence, genuinely unresolved outcome, and secondary rollback/failure-recording failures |
| Source fidelity | Every capture contract's independent original-to-query checks, source-free query/history/paired backup and exact view-relative diagnostic highlights |
| Performance | Optimized real large-input import/CPU/heap and requested-key plans/VM scaling; no per-owner whole-edition audit or per-row lookup loop hidden by smaller batches |

Current sem evidence at e1fbb97 traced `import_no_intro_database`,
`start_streaming_import`, `finish_streaming_import`, `finish_streaming_result`,
`record_failed_import` and `prepare_snapshot` in `src/storage/catalog_import.rs`.
It shows the existing validated-reader transaction and separate parse-failure
path; storage errors currently return directly. It does not implement this new
typed writer or generalized commit resolution. The existing candidate's source
counts, closure, qualification and affected-component tests prove their stated
SQL slices only. Full model approval and all implementation witnesses above
remain open.
