# Pinned specifications and synthetic field witnesses

## MAME machine XML

`mame-0.289.dtd` contains the declaration block emitted by MAME 0.289's
[`infoxml.cpp`](https://github.com/mamedev/mame/blob/mame0289/src/frontend/mame/infoxml.cpp#L151-L313).
It was extracted from authentic `-listxml` output with line endings normalized
to LF. Its SHA-256 is
`25dacf470cd966048e0aac8195e064f5360f0e7aba3c957c7dc3a2b1082acd43`.
These are upstream specification declarations, not the acquired machine catalog.

`mame-machine-fields.xml` is synthetic input covering all 125 declared
attributes and the ten separately qualified compatibility attributes. It
includes all three condition owners, DIP and configuration switches, explicit
and omitted defaults, empty values, Unicode, tabs, and interspersed vendor
attributes. It contains no ROM data.

The tests derive QName coordinates independently from the input tokens and
compare all fields against their real native owners and complete SQL keys.
UTF-8, CRLF, UTF-16 in both byte orders, and gzip inputs exercise decoded
coordinates while retained originals must remain byte-identical. The import
remains a single streaming pass; positions store no duplicate values or syntax.

## Software-list XML

`softwarelist-0.289.dtd` is an unmodified, byte-pinned copy of the
[MAME 0.289 software-list DTD](https://raw.githubusercontent.com/mamedev/mame/mame0289/hash/softwarelist.dtd).
Its SHA-256 is `3b14fa382113bc1c259b2a119346b0c7b4777ebdd52e6610bdc293008af4b549`.
The DTD is upstream specification material, not a ROM catalog acquired from a user.

`software-list-fields.xml` is synthetic test input. It supplies every one of the
36 declared attributes, every PCDATA field, both notes owners, a forward clone
reference, repeated names, absent/empty named values, explicit/defaulted data
areas and a metadata-only parent. Its child order follows the canonical DTD.
It contains no ROM data and is separate from the acquired metadata corpus.

The tests compare the pinned declaration inventory with native typed SQL
columns, public metadata/file queries and independently changed snapshot facts.
Required-field and enum mutations must preserve all already published native
facts and leave no new catalog facts. Attribute editing uses the actual XML
token's borrowed range, so repeated values cannot pick another owner by accident.
The native XML import still makes one streaming read; this test inventory adds
no parsing pass, JSON persistence or copied source syntax to production.
