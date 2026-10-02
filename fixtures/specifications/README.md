# Software-list field witnesses

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
