"""SQLite-only projections for XML Schema integer lexical values."""


I64_MIN = -(2**63)
I64_MAX = 2**63 - 1


def xml_integer_sql(expression, minimum, maximum):
    """Return SQL yielding an XML integer value in the requested range or NULL.

    The expression must evaluate to TEXT. XML whitespace at the boundaries is
    trimmed (internal whitespace remains invalid), and one optional sign plus
    ASCII decimal digits are accepted. Significant digits are compared as text
    before casting, so long runs of leading zeroes are harmless and overflow
    never reaches SQLite's saturating integer cast.
    """
    if not isinstance(expression, str) or not expression.strip():
        raise ValueError("expression must be a non-empty SQL expression")
    if type(minimum) is not int or type(maximum) is not int:
        raise TypeError("minimum and maximum must be Python integers")
    if not I64_MIN <= minimum <= I64_MAX or not I64_MIN <= maximum <= I64_MAX:
        raise ValueError("minimum and maximum must fit SQLite INTEGER")
    if minimum > maximum:
        raise ValueError("minimum must not exceed maximum")

    value = f"({expression})"
    low_abs = str(abs(minimum))
    high_abs = str(abs(maximum))
    if minimum < 0:
        negative_lower = (
            f"(length(significant) < length('{low_abs}') OR "
            f"(length(significant) = length('{low_abs}') AND "
            f"significant COLLATE BINARY <= '{low_abs}'))"
        )
    else:
        negative_lower = "0"
    if maximum < 0:
        negative_upper = (
            f"(length(significant) > length('{high_abs}') OR "
            f"(length(significant) = length('{high_abs}') AND "
            f"significant COLLATE BINARY >= '{high_abs}'))"
        )
    else:
        negative_upper = "1"

    positive_lower = (
        "1" if minimum <= 0 else
        f"(length(significant) > length('{minimum}') OR "
        f"(length(significant) = length('{minimum}') AND "
        f"significant COLLATE BINARY >= '{minimum}'))"
    )
    positive_upper = (
        "0" if maximum < 0 else
        f"(length(significant) < length('{maximum}') OR "
        f"(length(significant) = length('{maximum}') AND "
        f"significant COLLATE BINARY <= '{maximum}'))"
    )

    negative_result = (
        f"CASE WHEN significant = '9223372036854775808' "
        f"THEN (-9223372036854775807 - 1) "
        f"ELSE -CAST(significant AS INTEGER) END"
    )
    zero_in_range = int(minimum <= 0 <= maximum)

    return f"""(
WITH
source(raw) AS NOT MATERIALIZED (VALUES ({value})),
trimmed(value) AS (
    SELECT trim(raw, char(9) || char(10) || char(13) || ' ')
    FROM source
    WHERE typeof(raw) = 'text'
      AND instr(CAST(raw AS BLOB), X'00') = 0
),
signed(value, negative, digits) AS (
    SELECT value,
           substr(value, 1, 1) = '-',
           CASE WHEN substr(value, 1, 1) IN ('+', '-')
                THEN substr(value, 2) ELSE value END
    FROM trimmed
),
parsed(negative, significant) AS (
    SELECT negative, coalesce(nullif(ltrim(digits, '0'), ''), '0')
    FROM signed
    WHERE digits <> '' AND digits NOT GLOB '*[^0-9]*'
),
accepted(negative, significant) AS (
    SELECT negative, significant
    FROM parsed
    WHERE CASE
        WHEN significant = '0' THEN {zero_in_range}
        WHEN negative THEN ({negative_lower}) AND ({negative_upper})
        ELSE ({positive_lower}) AND ({positive_upper})
    END
)
SELECT CASE
    WHEN significant = '0' THEN 0
    WHEN negative THEN {negative_result}
    ELSE CAST(significant AS INTEGER)
END
FROM accepted
)"""
