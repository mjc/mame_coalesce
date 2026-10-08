"""SQLite-only lexical helpers for XML text, names, QNames, and languages."""


_NAME_START_RANGES = (
    (0x41, 0x5A), (0x5F, 0x5F), (0x61, 0x7A),
    (0xC0, 0xD6), (0xD8, 0xF6), (0xF8, 0x2FF),
    (0x370, 0x37D), (0x37F, 0x1FFF), (0x200C, 0x200D),
    (0x2070, 0x218F), (0x2C00, 0x2FEF), (0x3001, 0xD7FF),
    (0xF900, 0xFDCF), (0xFDF0, 0xFFFD), (0x10000, 0xEFFFF),
)
_NAME_EXTRA_RANGES = (
    (0x2D, 0x2E), (0x30, 0x39), (0xB7, 0xB7),
    (0x300, 0x36F), (0x203F, 0x2040),
)


def _expression(expression):
    if not isinstance(expression, str) or not expression.strip():
        raise ValueError("expression must be a non-empty SQL expression")
    return f"({expression})"


def _glob_class(ranges, allow_colon=False):
    members = [chr(first) if first == last else f"{chr(first)}-{chr(last)}"
               for first, last in ranges]
    if allow_colon:
        members.append(":")
    return "[" + "".join(members) + "]"


def _name_valid_sql(value, allow_colon, require_start):
    start = _glob_class(_NAME_START_RANGES, allow_colon)
    characters = _glob_class(
        sorted(_NAME_START_RANGES + _NAME_EXTRA_RANGES), allow_colon
    )
    conditions = [
        f"typeof({value}) = 'text'",
        f"length({value}) > 0",
        f"instr(CAST({value} AS BLOB), X'00') = 0",
        f"({value}) NOT GLOB '*[^{characters[1:-1]}]*'",
        # SQLite GLOB compares these three-byte sequences imprecisely in a
        # range ending at U+FFFD, so exclude the next two code points by bytes.
        f"instr(CAST({value} AS BLOB), X'EFBFBE') = 0",
        f"instr(CAST({value} AS BLOB), X'EFBFBF') = 0",
    ]
    if require_start:
        conditions.append(f"substr({value}, 1, 1) GLOB '{start}'")
    return "(" + " AND ".join(conditions) + ")"


def xml_collapse_sql(expression):
    """Return SQL collapsing XML tab, LF, CR, and ASCII space runs to spaces.

    NUL and every other character are preserved. Non-TEXT SQL values pass
    through unchanged so a subsequent validator can reject them by type.
    A recursive replacement halves each remaining adjacent-space run; it has
    no application-defined iteration or input-length limit.
    """
    value = _expression(expression)
    return f"""(
WITH RECURSIVE
source(raw) AS (VALUES ({value})),
normalized(value) AS (
    SELECT CASE WHEN typeof(raw) = 'text'
        THEN replace(replace(replace(raw, char(9), ' '), char(10), ' '), char(13), ' ')
        ELSE raw END
    FROM source
),
collapsed(value) AS (
    SELECT trim(value, ' ') FROM normalized WHERE typeof(value) = 'text'
    UNION ALL
    SELECT replace(value, '  ', ' ')
    FROM collapsed
    WHERE instr(value, '  ') > 0
)
SELECT value FROM collapsed WHERE instr(value, '  ') = 0
UNION ALL
SELECT value FROM normalized WHERE typeof(value) <> 'text'
)"""


def xml_name_valid_sql(expression, allow_colon=False, require_start=True):
    """Return SQL yielding 1 for a valid XML Name, otherwise 0.

    The input must already be XML-whitespace-collapsed. Unicode ranges match
    ``xml_name_start_char`` and ``xml_name_char`` in ``src/xml_reader.rs``.
    """
    if type(allow_colon) is not bool or type(require_start) is not bool:
        raise TypeError("allow_colon and require_start must be booleans")
    value = _expression(expression)
    predicate = _name_valid_sql("value", allow_colon, require_start)
    return f"""(
WITH source(value) AS (VALUES ({value}))
SELECT CASE WHEN coalesce({predicate}, 0) THEN 1 ELSE 0 END
FROM source
)"""


def xml_qname_valid_sql(expression):
    """Return SQL yielding 1 for an unprefixed NCName or one-colon QName."""
    value = _expression(expression)
    ncname = lambda part: _name_valid_sql(part, False, True)
    colon = "instr(value, ':')"
    prefix = f"substr(value, 1, {colon} - 1)"
    local = f"substr(value, {colon} + 1)"
    return f"""(
WITH source(value) AS (VALUES ({value}))
SELECT CASE
    WHEN typeof(value) <> 'text' OR length(value) = 0
      OR instr(CAST(value AS BLOB), X'00') <> 0 THEN 0
    WHEN {colon} = 0 THEN CASE WHEN {ncname('value')} THEN 1 ELSE 0 END
    WHEN instr(substr(value, {colon} + 1), ':') <> 0 THEN 0
    WHEN {ncname(prefix)} AND {ncname(local)} THEN 1
    ELSE 0
END
FROM source
)"""


def xml_language_valid_sql(expression):
    """Return SQL yielding 1 for an ASCII XML language tag.

    The primary segment has 1-8 ASCII letters. Every hyphenated following
    segment has 1-8 ASCII letters or digits. Segment count is unlimited.
    """
    value = _expression(expression)
    return f"""(
WITH RECURSIVE
source(value) AS (VALUES ({value})),
parts(segment, rest, ordinal) AS (
    SELECT
        CASE WHEN instr(value, '-') = 0 THEN value
             ELSE substr(value, 1, instr(value, '-') - 1) END,
        CASE WHEN instr(value, '-') = 0 THEN NULL
             ELSE substr(value, instr(value, '-') + 1) END,
        0
    FROM source
    WHERE typeof(value) = 'text'
      AND length(value) > 0
      AND instr(CAST(value AS BLOB), X'00') = 0
    UNION ALL
    SELECT
        CASE WHEN instr(rest, '-') = 0 THEN rest
             ELSE substr(rest, 1, instr(rest, '-') - 1) END,
        CASE WHEN instr(rest, '-') = 0 THEN NULL
             ELSE substr(rest, instr(rest, '-') + 1) END,
        ordinal + 1
    FROM parts
    WHERE rest IS NOT NULL
),
checked(ok) AS (
    SELECT count(*) > 0 AND min(
        length(segment) BETWEEN 1 AND 8
        AND segment NOT GLOB CASE WHEN ordinal = 0
            THEN '*[^A-Za-z]*' ELSE '*[^A-Za-z0-9]*' END
    )
    FROM parts
)
SELECT CASE WHEN coalesce((SELECT ok FROM checked), 0) THEN 1 ELSE 0 END
)"""
