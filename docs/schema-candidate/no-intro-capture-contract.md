# No-Intro source capture contract

Status: proposed MAMEC-62 design, not a reader implementation or approved
cutover. The [publication contract](publication-contract.md) owns private EOF
capabilities, independent checked counts, batch installation and rollback.
This document specifies format-specific capture facts. Original documents stay
external; SQLite stores typed catalog values, not XML or JSON payloads.

## Source-to-table boundaries

The closed maps are `no_intro-counts.tsv`, `no_intro-owners.tsv`,
`no_intro-field-coverage.tsv` and `no_intro-field-presence.tsv`. They own exact
field codes, actual parents, value/position routes and count selectors. Use those
rows rather than another handwritten inventory. An absent optional child has no
row; present-empty accepted text has one empty value on its typed owner. PCDATA
has no attribute-position row.

| Input | Completed record and final proof | Native placement |
|---|---|---|
| Flat DAT v3/v4, strict/compatible | Closed games may be handed off before EOF; the final proof carries the selected mode, root interval, 26 independent DAT counters and completed document-wide ID/IDREF checks, not an accumulated game collection | One unqualified datafile and required preceding header; games and supported children use their existing typed owners and actual parent-local mixed element order |
| Observed database export / NUL recovery | Closed games carry their existing parent-local witnesses; final proof carries envelope/header facts, whole-document and physical-root intervals, 18 independent edition totals and exhausted recovery warnings | Exactly one datafile; optional nested header before games, or exactly one preceding sibling header; one canonical header owner in either mode |
| Synthetic P/C fixture | Closed fixture records plus a final proof for the exact repository projection and ten fixture counters | Existing synthetic header/game/child/attribute routes only; this is not authentic DAT-o-MATIC P/C grammar |

These conceptual private handoffs distinguish a completed record from accepted
document EOF. A completed game may enter a checked SQL batch; it cannot publish
the edition. Source-event increments happen when the parser accepts the named
event and before SQL, not necessarily before constructing a typed value used to
validate that event. No expected total comes from inserted rows.

Every direct child element start advances its immediate parent's zero-based
element order, including ignored elements; text/comments do not. Physical
attribute order includes gaps from namespace declarations and ignored attributes.
Do not densify retained ordinals or copy edition/machine ancestry. XSI attributes
increment their own canonical counters in addition to the declaring element's
event. The fixture's `languages` position increments once when physically
present, even empty; its token counter increments per emitted nonempty
comma-split/trimmed token. Absent means neither position nor token.

## Flat DAT XSI capture

Capture the existing four retained XSI codes: `schemaLocation`,
`noNamespaceSchemaLocation`, `type` and `nil`, on the eleven typed declaring
owner relations. Namespace declarations are resolver state, not extra XSI rows.
Each existing XSI row stores its attribute value once, its lexical position,
and **non-NULL `source_qname` for every retained code**. The QName is the exact
decoded attribute-name spelling (for example `s:nil`), not the type value.

Only the four simple-text owner relations have `resolved_type_kind`. It is
non-NULL exactly for `field_kind='type'`; other codes have NULL. The seven
complex-owner relations have no such column and reject type declarations.
No new `resolved_builtin_type` column, duplicate XSI payload or generic
namespace-binding table is proposed.

Resolve an `xsi:type` attribute's value QName while the declaring element's
namespace context is live. Keep its original value on that same row and its
resolved XML Schema local kind in `resolved_type_kind`. The closed type set and
allowed derivations are those checked by `dat_xsi.py`: header integer narrowing
and the admitted string-derived families. An unbound or incorrectly bound prefix
cannot be accepted merely because its local spelling matches. Attribute QName
and type-value QName are distinct facts.

Schema-hint pairing/revision checks, strict rejection of any explicit nil,
compatible acceptance of collapsed false/0 only, scalar lexical rules, and strict
document-scoped case-sensitive ID/IDREF closure follow the executable XSI
contract. Forward references may resolve before EOF. Compatible mode retains
its accepted raw text without acquiring strict value/ID registration checks.
Unknown attributes/subtrees follow the selected reader policy; they do not gain
invented typed owners. Root hints do not replace declarations on nested owners.

ROM size is one raw text value. Its query-only numeric projection follows
`native_file_sizes.py`: strict unsignedInt lexical/value validation through
4294967295; compatible nonempty ASCII decimal digits through signed SQLite
INTEGER maximum. Compatible plus signs, surrounding whitespace, negatives,
NUL or overflow remain retained but unusable comparison evidence, not new
input rejection. No second stored parsed size is added.

## Export envelope, counts and recovery

The export document owns the complete input envelope. Its datafile owns that
actual XML root. The canonical header is distinct in both envelope modes:
nested headers have actual datafile placement/order; sibling headers have no
invented cross-root ordinal. `header_present` remains the one independent
zero/one fact, including present-empty headers. Repair policy comes from the
selected reading rule and repair relation, not a copied document flag.

The existing 18 edition totals and nine parent-local count kinds remain the
closed export contract. Preserve source-observed zero values explicitly. Do not
add a game-name-position total or another header count. A later malformed tail,
extra root, invalid envelope, missed recovery warning or checked overflow
invalidates the entire import even after earlier completed games were written.

Recovery replaces decoded U+0000 with U+FFFD only in the parser's view. Original
bytes remain unchanged. Unicode-scalar positions are preserved by that
replacement, but UTF-8 byte width changes from one byte to three: parse-buffer
byte offsets are therefore **not** canonical decoded/original byte offsets.
Track the mapping to the unrepaired decoded view, or leave byte facts unavailable
and use proven coordinates. Never tag sanitized bytes as retained-original or
transport-decoded evidence.

## Exact source views and extents

Use the existing vocabularies; there is no stored repaired-byte view.
`catalog_decoded_xml_views` identifies reproducible **unrepaired**
transport-decoded XML bytes, independent of reading rules. Their byte extents
need no original-byte mapping. An extent labelled `retained_original_bytes`
does need a verified mapping to that original encoding/compressed transport.
Byte lengths and view identity remain single-owned by the existing shared rows.

XML root/header intervals start at the opening `<` and end exclusively after
the matching closing/self-closing `>`. Whole-document intervals cover the
named input from beginning to EOF, including permitted prolog, outer whitespace
and sibling-root gaps. Root close and EOF are not interchangeable.

| Owner | Captured interval |
|---|---|
| Flat DAT document | Actual datafile root only; excludes prolog/trailer. Do not invent a second whole-document owner or extent |
| Export document | Whole envelope; it contains independently observed datafile and header intervals in comparable views |
| Export datafile | Actual datafile root; opening-only evidence does not supply an end |
| Export header | Its actual header element; nested containment or sibling-before-datafile relation follows the selected envelope |
| Synthetic P/C document | Actual fixture datafile root in the existing decoded XML view, not a claimed authentic producer-wide view |

Choose `transport_decoded_xml_text` with one-based Unicode-scalar columns for
new XML capture and retain the exact newline convention. The export
document/header's already-admitted original-text alternative may be used only
with proof of that view and its mapping. Ordinary owners retain only the
positions their existing tables admit; do not fabricate child end ranges.

Unknown extents stay NULL. Cross-view containment requires a verified mapping.
Diagnostic excerpts are exact BLOB windows of retained-original or unrepaired
decoded bytes; their zero-based end-exclusive highlight is relative to that
saved BLOB. Recovery warnings identify the original NUL evidence, not a
replacement character copied from the sanitized parser buffer. A zero-length
EOF anchor at an owner's exclusive end cannot claim containment in that owner.

## Evidence and post-approval tests

Current `sem_context` traces show flat DAT returns `ValidatedNoIntroDat` only
after EOF and ID/IDREF finish. It resolves names/types while namespace context
exists but does not carry every new capture fact through its public model.
Export's reader distinguishes envelopes and exhausts recovery warnings at EOF;
its current NUL scan plus parser are not proof of the target fused capture.
Input and current-record buffers may be input-dependent; one semantic parse
does not prove constant total memory or zero copying.

Existing candidate field/presence/count/cardinality/XSI/export checks exercise
constructed rows and publication refusal/repair. They do not prove parser-fed
counts, namespace bindings, complete physical intervals or the typed writer.
The observed export ledger has 135 fields; the reported 137-field CSV remains
unlocated, so no two unnamed fields are invented. Authentic P/C remains outside
the supported grammar.

After approval, real-reader tests must cover all four DAT modes; every canonical
event and XSI declaring owner; exact attribute QName versus resolved type value;
namespace rebinding and ordinal gaps; forward/unresolved/duplicate identities;
export nested/absent/present-empty/sibling headers; late/truncated/extra roots;
UTF-8/UTF-16, CRLF, multibyte text and one/multiple recovered NULs; exact
unrepaired excerpts and mapped offsets; fixture-only language positions/tokens;
and checked overflow. Assert completed-record callbacks may occur before a
late failure, but no EOF proof, publication or durable partial import escapes.
The shared publication contract owns atomic failure and commit-outcome tests.
