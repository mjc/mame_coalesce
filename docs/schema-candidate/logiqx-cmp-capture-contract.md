# Logiqx and CMP capture contract — candidate design

Candidate-only design for [logiqx_cmp.sql](logiqx_cmp.sql). It defines
the proposed source-capture target for review; it does not change production
readers, DDL, or publication code and does not claim the contract is
implemented. Contract approval and proof of a future implementation are
separate decisions. Common EOF, counter, failure, rollback, and commit rules
are in [publication-contract.md](publication-contract.md).

## Canonical mappings and handoff

The exact source event, family/counter, and target table/key mappings are the
rows in [logiqx_cmp-counts.tsv](logiqx_cmp-counts.tsv). Parent IDs, immediate
FKs, and source-order domains are in [logiqx_cmp-owners.tsv](logiqx_cmp-owners.tsv).
Field and presence semantics come from [logiqx_cmp-field-coverage.tsv](logiqx_cmp-field-coverage.tsv)
and [logiqx_cmp-field-presence.tsv](logiqx_cmp-field-presence.tsv). These are
closed inventories; this prose adds no selectors, counters, or aliases.
Candidate ownership context is summarized in [logiqx_cmp-notes.md](logiqx_cmp-notes.md).

The selected existing reading identity governs parsing: Logiqx uses the pinned
compatible interpretation or pinned DTD 1.5 strict mode; CMP remains pinned
compat-v1. Compatible Logiqx retains accepted root `file_name` and declared
`sha1` children. Strict mode rejects those compatible-only children. A root
SHA-1 is a source declaration, not a computed document digest. CMP has no new
vendor grammar.

The proposed producer hands the writer a document-start fact, then one checked
completed typed record at a time: a Logiqx root child/game, or a CMP header/set
form. Each record retains supported values once, its actual parents, and
canonical positions. CMP comments are separate annotation events emitted at
their lexical occurrence. A checked record is installed as a SQL batch and
released after its bindings no longer borrow it. Current CMP parsing buffers a
complete `Form`; future state may depend on the input/current record, but must
not accumulate the catalog, create another whole-source copy, or serialize
raw tokens/payload JSON. Do not claim constant total memory or introduce a
fixed batch cap. Parser/writer errors prevent a successful EOF seal; common
failure handling is specified in the publication contract.

## Logiqx capture

Each direct `<datafile>` child element consumes one root-child ordinal,
including ignored compatible children; comments, processing instructions,
and inter-element text do not. Accepted root fields, header, and games retain
their actual ordinal, including gaps. Ignored children create no owner or
payload. A game is handed off only after its subtree is parsed and validated;
its mixed child order, values, hashes, relationships, and field positions use
the canonical ledgers and actual parent keys.

The physical XML root extent is the `<datafile>` element from its opening `<`
through the byte after its matching closing `>` (or accepted empty-root
delimiter). It excludes prolog and trailing material. Store endpoint
coordinates with their named view and capture the closing endpoint at the
reader event; never substitute the last child or EOF. For byte extent use
`retained_original_bytes` or `transport_decoded_xml_bytes` only when both
endpoints have a verified mapping into that view. The XML location/extent view
names and field owners remain those in the candidate DDL and DOC-21.

## CMP capture

Preserve document-form order across all top-level forms, including ignored or
extension forms accepted by compat-v1. The common `catalog_sets` row remains
the sole name, placement, and identity owner for each set. Preserve accepted
header/set values once on their typed owners. A CMP pair consumes its existing
field/item order; capture the original keyword coordinate separately from the
value coordinate, along with original spelling, quoted state, and empty quoted
values. Preserve original form spelling in `source_block`. `crc` and `crc32`
remain distinct. Keyword-only `nodump`/`baddump` flags have a keyword anchor
but no value text, value coordinate, or synthetic empty value. Preserve
repeated sample declarations as distinct media owners with their shared
set-item order and their existing token anchors.

Effective CMP ROM status is derived source-free from the stored `status_text`
and canonical `nodump`/`baddump` flag rows, following the existing
`parse_asset_status`/DOC-11 rule: status alone yields its text; exactly one
flag without status yields that flag's status; no declaration and conflicting
status/flag combinations yield no effective status. Duplicate flags and
duplicate status continue to follow the existing parser's rejection rule. Do
not add a cached status or duplicate flag-presence fields.

Every lexer-recognized semicolon comment, including comments between a keyword
and value or inside an unsupported nested form, is stored once in
`clrmamepro_comments` with exact text and decoded-text start coordinate. It is
a document annotation, not a syntax child: do not invent a parent, item order,
comment rank, neighbor ID, or token stream. Query lexical interleaving from
unique coordinates. Coordinates use `decoded_dat_text`, after accepted BOM
handling, one-based Unicode-scalar columns; LF advances a line, CRLF leaves CR
in comment text, and bare CR is not a line break. A keyword anchor is the
original token start, never reconstructed from the value.

The CMP byte extent is the complete source document, including comments,
material outside forms, and any BOM. Its only permitted byte view is
`retained_original_bytes`, and only when a verified mapping from decoded DAT
boundaries to retained-original byte offsets supplies both endpoints. It
covers the full retained source interval, including BOM bytes; it is not the
first decoded character through EOF with the BOM silently omitted. Store
coordinates separately in `decoded_dat_text` with the candidate's
one-based-Unicode-scalar convention. Do not label decoded offsets as original
bytes or infer a mapping through decoding. If mapping is unavailable, leave
the byte extent absent; retain only endpoint facts the named coordinate view
actually supports. This follows DOC-21 and the distinct `extent_view` and
`location_view` columns in the candidate DDL.

## Ownership and future proof

The field ledgers close supported values, present-empty states, positions,
and source forms; do not repeat their inventory here. Each value has one
canonical owner, each supported position its existing position relation,
and hashes/relationships their shared owners and mapped position FKs. Actual
parent FKs establish ancestry. Do not copy names, ancestry, presence mirrors,
stored ranks, effective values, raw payload, token slices, or syntax trees.
Keep Logiqx game child order, CMP document-form order, CMP set-item/field
order, token coordinates, and comment coordinate ordering as distinct facts.

Approval accepts these target rules; the following implementation proof comes
later. It must show raw Logiqx root ordinals with ignored-child gaps; closure
against every existing count key and field/position mapping; strict versus
compatible root-child behavior; root extent endpoints; CMP form/item order,
keyword/value anchors, quoted/empty values, flags and samples; comment text,
coordinates and interleaving; and CMP full-source byte mapping including BOM.
Failure after earlier complete records must produce no successful seal or
partial publication, as required by the shared publication contract. Source-
free reads must derive effective CMP status from canonical rows. These are
implementation witnesses, not prerequisites for approving this design.

Current code confirms the limits of existing evidence. The Logiqx reader
streams completed games but drops ignored root-child ordinals; current metadata
does not prove root order. CMP emits completed headers/sets but buffers each
form, strips an accepted BOM before parsing, and currently captures comment
text/start but not form context or keyword coordinates. XML transport/UTF-16
decoding does not by itself prove an original-byte endpoint mapping. The
candidate ledgers and DDL establish target shape, not producer capture or a
source-free status-query witness.
