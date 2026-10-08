"""SQLite-only SQL for the MAME software numeric grammar."""


def software_integer_sql(expression):
    """Return SQL yielding MAME software's checked integer value or NULL.

    This mirrors ``parse_number``: 0x/0X hexadecimal, multi-character
    leading-zero octal, otherwise decimal; plus signs and malformed text are
    rejected. Values above i64::MAX yield NULL. The expression must evaluate
    to TEXT. Radix accumulation is recursive only after leading zeroes are
    removed, so it is bounded to 16 hexadecimal or 21 octal digits.
    """
    if not isinstance(expression, str) or not expression.strip():
        raise ValueError("expression must be a non-empty SQL expression")

    value = f"({expression})"
    is_hex = f"substr(value, 1, 2) IN ('0x', '0X')"
    radix = (
        f"CASE WHEN {is_hex} THEN 16 "
        "WHEN length(value) > 1 AND substr(value, 1, 1) = '0' THEN 8 "
        "ELSE 10 END"
    )
    raw_digits = f"CASE WHEN {is_hex} THEN substr(value, 3) ELSE value END"
    digit_value = "(instr('0123456789abcdef', lower(substr(digits, position, 1))) - 1)"
    digit_fits = (
        f"{digit_value} >= 0 AND {digit_value} < radix AND "
        f"accumulator <= (9223372036854775807 - {digit_value}) / radix"
    )

    return f"""(
WITH RECURSIVE
source(value) AS (VALUES ({value})),
token AS (
    SELECT value,
           {radix} AS radix,
           {raw_digits} AS raw_digits
    FROM source
    WHERE typeof(value) = 'text'
      AND instr(CAST(value AS BLOB), X'00') = 0
      AND instr(value, '+') = 0
),
prepared AS (
    SELECT radix, ltrim(raw_digits, '0') AS significant
    FROM token
    WHERE raw_digits <> ''
      AND raw_digits NOT GLOB
          (CASE radix WHEN 16 THEN '*[^0-9a-fA-F]*'
                      WHEN 8 THEN '*[^0-7]*'
                      ELSE '*[^0-9]*' END)
),
bounded AS (
    SELECT radix, coalesce(nullif(significant, ''), '0') AS digits
    FROM prepared
    WHERE (radix = 10 AND
           (length(significant) < 19 OR
            (length(significant) = 19 AND significant COLLATE BINARY <= '9223372036854775807')))
       OR (radix = 8 AND length(significant) <= 21)
       OR (radix = 16 AND length(significant) <= 16)
),
fold(position, radix, digits, accumulator, valid) AS (
    SELECT 1, radix, digits, 0, 1
    FROM bounded
    WHERE radix IN (8, 16)
    UNION ALL
    SELECT position + 1,
           radix,
           digits,
           CASE WHEN valid = 1 AND {digit_fits}
                THEN accumulator * radix + {digit_value}
                ELSE accumulator END,
           CASE WHEN valid = 1 AND {digit_fits} THEN 1 ELSE 0 END
    FROM fold
    WHERE position <= length(digits)
),
fold_result AS (
    SELECT accumulator, valid
    FROM fold
    WHERE position = length(digits) + 1
)
SELECT CASE
    WHEN EXISTS (SELECT 1 FROM bounded WHERE radix = 10)
        THEN (SELECT CAST(digits AS INTEGER) FROM bounded WHERE radix = 10)
    WHEN EXISTS (SELECT 1 FROM fold_result WHERE valid = 1)
        THEN (SELECT accumulator FROM fold_result WHERE valid = 1)
    ELSE NULL
END
)"""
