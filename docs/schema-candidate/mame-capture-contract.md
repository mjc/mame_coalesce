# MAME capture contract — candidate design

Status: proposed for the MAMEC-62 approval gate. This is a design contract,
not evidence of reader/writer implementation. Approval of this
contract and proof of a future implementation are separate decisions.

## Current evidence and target handoff

The public `mame::read_with` currently accepts `&[u8]`, builds an `Element`
tree for each direct root child, parses it, drops it, then invokes the record
callback. A later XML error can therefore follow earlier callbacks. This does
not prove event-level database writes, complete root bounds, or EOF sealing.
The current writer consumes parsed snapshot records and writes their owners in
separate phases; it is not a streaming XML sink. (sem_context: 2ms, reader
callback boundary and dependencies.)

The proposed capture keeps a typed, input-dependent current machine and the
nested owner/order state needed to parse it. As source events arrive, it updates
that state and independent source counters; it does not write partial machine
state to SQL. At machine close it validates the completed typed record and its
local ownership, then hands that checked record to the writer for SQL batch
installation. Release its transient typed state after installation. The
reader/input buffers and current machine may scale with input; make no constant
total-memory claim. Do not build a whole-document DOM, keep a catalog of
completed machines, or make an additional whole-source/payload copy.

## Canonical source mapping and order

The exact selector, counter, target table/key, and scope for each counted
source event are the rows of [mame-counts.tsv](mame-counts.tsv). The exact
owner, ID, immediate-parent FK, and ordering key are the rows of
[mame-owners.tsv](mame-owners.tsv). Field and presence behavior comes only from
[mame-field-coverage.tsv](mame-field-coverage.tsv) and
[mame-field-presence.tsv](mame-field-presence.tsv); hash and relationship
positions come from [mame-hash-positions.tsv](mame-hash-positions.tsv) and
[relationship-positions.tsv](relationship-positions.tsv). These files are the
closed vocabularies. There are no placeholder selectors or inferred rows in
this prose.

Every direct element child start consumes one ordinal in its immediate
parent's domain, whether recognized, ignored, or retained as an extension.
Text, comments, processing instructions, and CDATA do not. Thus ignored
machine children leave gaps in the shared machine order; nested owners each
have their own zero-based child order and use the actual immediate parent FK
listed in `mame-owners.tsv`. Ignored elements create no source-element owner or
count unless a canonical ledger row names one. Never filter before advancing
the ordinal or reconstruct order by sorting typed records.

## Values, positions, and identity

Follow every field-coverage/presence row for required and optional values,
present-empty values, defaults, lexical forms, enums, sparse owners, and
positions. The PCDATA children `description`, `year`, and `manufacturer` are
typed text owners in `mame_machine_text_elements`; description is required,
year and manufacturer optional. Store each text once. RAM-option PCDATA is
stored once on its `mame_ram_options` owner, including empty text. Do not create
attribute positions for text.

For DTD-defaulted attributes, an absent attribute stores its effective default
with `specified=0` and no position; an explicit value stores its effective
value with `specified=1` and its canonical position. Do not extend this rule to
CDATA defaults such as adjuster or RAM-option `default`. Keep sparse
`isconsumable` and compatibility disk `writeable` distinct from native disk
`writable`. The existing ledgers define the default fields and wire contexts.

`catalog_entry_hashes` remains the sole digest declaration/state/value owner.
Valid canonical bytes use `hash_id`/`hash_values.bytes`; the shared
declared-text relation reconstructs original spelling. Preserve invalid text
or lexical override only in `reported_text`, as the shared hash contract
requires. Use the distinct ROM, compatibility-MD5, and disk-SHA-1 position
routes in the canonical hash ledger. Do not infer file identity from a digest
label. Relationship values likewise use their shared owner and the exact
position routes in `relationship-positions.tsv`.

Attribute identity is the DDL/ledger field code, occurrence, owner key, and
source-order position. A namespaced attribute uses the reader's canonical
`{namespace-uri}local-name`; split URI/local name only for a field admitted by
the source contract. Unknown namespaced attributes cannot create field codes.
Positions own coordinates/order, not duplicate values.

## Reading identity and multiplicity

The proposed strict reading identity is exactly `strict-dtd`; the proposed
compatible identity retaining all accepted instances is exactly
`observed-v3`. They are design keys aligned with the candidate DDL, not newly
implemented runtime modes. `observed-v2` remains a historical identity for
first-instance behavior and its information loss; do not label it v3 or
strict. Strict-only integrity checks apply only to `strict-dtd`.

Repeated children are accepted only where the candidate owner model supports
them. In particular, the candidate has one switch condition per switch, one
condition per switch value, and one per adjuster; these remain singleton in
v3. Switch kind controls which location/value QNames are valid, and each
condition uses its actual owner and retains `tag`, `mask`, `relation`, and
`value`. The candidate device-instance owner is repeatable: v3 stores every
accepted `<instance>` with its `name`, `briefname`, opening location, and
device-local order. Strict DTD permits at most one instance and applies its
pinned sequence, cardinality, required-field, enum, and ordering rules. Do not
let strict-only rejection leak into v3. This differs from current parser
behavior, which rejects repeated switch conditions and retains one optional
device instance.

## Root extent and shared finalization

Capture the physical MAME root's half-open byte interval with
`extent_view='transport_decoded_xml_bytes'`: start at the opening `<`, end
immediately after the matching closing/self-closing `>`. Capture the corresponding
endpoint coordinates with `location_view='transport_decoded_xml_text'` and
`column_convention='one_based_unicode_scalar'`. Byte and coordinate view names
are distinct, not interchangeable. Exclude legal trailing whitespace/comments.
Validate the byte interval against the decoded byte view's length;
EOF is not the root end. Keep retained-original length and decoded-view length
with their existing shared source-view owners. Never infer byte length from
characters or claim an original-byte mapping without proof. Ordinary MAME
child rows retain opening line/column only; do not invent child extents.

The source-event and writer-row counters use only the exact keys and mappings
in `mame-counts.tsv`. Common checked arithmetic, EOF acceptance, independent
count reconciliation, SQL rollback, failure receipts, and commit outcomes are
defined by [publication-contract.md](publication-contract.md); this format
contract adds no second counter/failure protocol. Machine completion is a
local checked-record boundary, not document acceptance. Only a validated root
extent, legal trailing input, EOF, and final reconciliation can enter the
shared publication seal.

## Approval versus implementation proof

Approval accepts the concrete mappings and rules above; it does not require
future implementation tests to exist. After approval, implementation evidence
must establish:

- Checked complete-machine handoff followed by SQL batch installation, with
  no partial-machine SQL writes; typed current state is input-dependent and
  contains no full XML tree or duplicate source payload.
- Interleaved recognized/ignored child fixtures proving machine and nested
  ordinals, actual parent keys, and canonical owner/counter closure.
- Row-by-row value/presence/position behavior, including defaults,
  present-empty and invalid controls, hash and relationship ownership.
- Distinct strict-dtd, observed-v3, and historical observed-v2 behavior;
  repeated singleton conditions reject, all supported v3 device instances
  persist, and strict allows at most one.
- Root byte/coordinate endpoints verified in the named view, plus late parse
  failure after an earlier completed machine and successful EOF sealing.

The full witness details belong to implementation review and the shared
publication contract. The current parser/writer facts above are evidence only
for today's behavior, not proof of this proposed design.
