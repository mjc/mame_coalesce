# Optional managed content storage decision

## Decision

Keep source inventory in place and retained catalog documents in the existing
SQLite document store. Do not migrate ROM collections or add a managed-object
backend to normal import, scan, build, or mount paths. Revisit an opt-in CAS
only when a measured serving/materialization workload shows that stable local
copies pay for their additional disk, backup, and recovery complexity.

The repository already deduplicates immutable retained documents by SHA-256
while preserving distinct acquisition and import records. ROM inventory is
location evidence, not a reason to copy an entire collection into an object
store. The test-only prototype in
[`src/storage/managed_storage_prototype.rs`](../src/storage/managed_storage_prototype.rs)
explores publication and restore invariants; it is not a production API or
migration.

## Identity and metadata boundaries

- Keep each acquired source artifact (including a ZIP/7z/RAR container) intact
  and address it by the digest of its exact bytes. Container headers, comments,
  descriptors, ordering, and compression choices therefore remain recoverable.
- Address an extracted payload by its exact bytes and length. A payload digest
  can deduplicate equal members across containers, but must not merge their
  source artifacts, archive-member identities, or acquisition records.
- Keep acquisition, source location, catalog/document metadata, member path and
  ordinal, and verification evidence in separate durable records. A CAS key is
  not provenance and must never become the catalog identity.
- Give derived representations their own identity over a versioned canonical
  recipe, implementation/build identity, parameters, and verified output
  digest. Never infer equivalence between different transformations from equal
  labels or filenames.

## Publication, export, and failure behavior

For any future opt-in write, stream into a uniquely named temporary file on the
same filesystem as the object directory while hashing and counting bytes. Sync
and verify the staged object before an atomic no-clobber publication. Only
after publication succeeds may a database transaction add the object and its
provenance references. If the process dies between file publication and the
database commit, the result is an unreferenced orphan; reconciliation may
report it, and garbage collection may remove it only after a grace period and
under an exclusive store lock. A failed or partial copy never removes or
rewrites the source.

Export/restore must stage beside the destination, verify the complete object,
then atomically publish it. A missing or corrupt managed object is a hard,
visible integrity failure; do not silently substitute another archive member
or source location. Keep existing sources until a separately specified and
verified retention policy explicitly permits removal. Backups must either
include the complete object set and a checked manifest or identify a durable,
verified external backup; a database-only backup must not claim a managed
store is restorable.

If implementation is ever approved, migration is additive: retain originals,
build and verify managed copies, commit references only after verification,
and roll back by clearing managed references rather than deleting source data.
No database/object-store multi-file transaction is assumed.

## Synthetic fixture measurement

The test-only spike creates two valid ZIPs containing the same 24-byte ROM and
different archive comments. The byte counts come from the generated fixture
itself (the test asserts distinct archive bytes and equal extracted payloads):

| Strategy | Kept objects | Payload bytes |
| --- | ---: | ---: |
| In-place inventory; do not copy the ROMs | 0 new objects | 0 additional |
| Managed source archives only | 2 | 324 (162 + 162) |
| Managed source archives and one shared extracted payload | 3 | 348 (162 + 162 + 24) |
| Naively duplicate both archives and extracted members | 4 | 372 |

CAS saves 24 bytes (6.5% of the naive 372-byte fixture) only when the extracted
member is also retained. The absolute savings are tiny; the result proves the
identity boundary, not a meaningful performance or real-library storage win.
Identical catalog-document bytes already have a content-derived key in the
current SQLite design, so moving those BLOBs to files has no demonstrated
deduplication benefit in this sample. No latency, throughput, large-corpus, or
filesystem-fragmentation benchmark is claimed.

## Prototype evidence and remaining decision gate

The test-only prototype verifies that metadata-distinct containers remain
separate while byte-identical members share one object, interrupted reads and
expected-digest mismatches publish nothing, and restore verifies before
replacing a destination while leaving the managed source untouched. These
tests establish failure semantics only. Before a production decision, measure
realistic collection size distributions, concurrent serving throughput,
backup/restore time and space, orphan rates, and filesystem behavior against
the current in-place design. If those measurements do not justify the
operational burden, retain the current architecture.
