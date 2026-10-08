"""Literal constructed-fixture receipts, never counts inferred from SQLite.

This helper is test-only. Real writers must independently accumulate checked
source events and seal only after accepted EOF; these fixtures prove neither.
"""

import source_counts


def seal(connection, family, edition_id, events):
    """Insert one sparse literal event vector; omitted known counters are zero."""
    if family not in source_counts.ROOTS:
        raise ValueError(f'unknown fixture count family {family}')
    names = tuple(row['counter'] for row in source_counts.inventory()
                  if row['family'] == family)
    if missing := set(events) - set(names):
        raise ValueError(f'unknown fixture counters: {sorted(missing)}')
    if any(type(value) is not int or value < 0 for value in events.values()):
        raise ValueError('fixture event counts must be nonnegative integers')
    columns = ','.join(names)
    marks = ','.join('?' for _ in range(len(names) + 1))
    connection.execute(f'INSERT INTO {family}_source_count_seals(edition_id,{columns}) '
                       f'VALUES({marks})',
                       (edition_id, *(events.get(name, 0) for name in names)))
