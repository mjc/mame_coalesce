"""Focused lexical and range checks for XML integer SQL projections."""

import sqlite3
import unittest
from contextlib import closing

from xml_numeric_sql import I64_MAX, I64_MIN, xml_integer_sql


class XmlIntegerSqlChecks(unittest.TestCase):
    def assert_projection(self, text, expected, minimum, maximum):
        expression = xml_integer_sql("input.value", minimum, maximum)
        query = f"WITH input(value) AS (VALUES (?)) SELECT {expression} FROM input"
        with closing(sqlite3.connect(":memory:")) as connection:
            self.assertEqual(connection.execute(query, (text,)).fetchone(), (expected,))

    def test_xml_int_lexical_and_range(self):
        cases = (
            (None, None), (1, None), ("", None), (" ", None),
            ("+", None), ("-", None), ("++1", None), ("--1", None),
            ("1+", None), ("1-", None), ("  +0012\t", 12),
            ("\t\n\r  -00012 \r\n", -12), ("-0", 0), ("+000", 0),
            ("1 2", None), ("1\t2", None), ("1\n2", None),
            ("1\r2", None), ("1\v", None), ("1\f", None),
            ("١", None), ("１", None), ("1x", None), ("x1", None),
            ("1\x002", None), ("1\x00", None), ("2147483647", 2147483647),
            ("2147483648", None), ("-2147483648", -2147483648),
            ("-2147483649", None),
        )
        for text, expected in cases:
            with self.subTest(text=text):
                self.assert_projection(text, expected, -2147483648, 2147483647)

    def test_narrow_and_unsigned_schema_ranges(self):
        matrices = (
            (-32768, 32767, ("-32768", -32768), ("32767", 32767),
             ("-32769", None), ("32768", None)),
            (-128, 127, ("-128", -128), ("127", 127),
             ("-129", None), ("128", None)),
            (0, 4294967295, ("0", 0), ("+4294967295", 4294967295),
             ("-0", 0), ("-1", None), ("4294967296", None)),
        )
        for minimum, maximum, *cases in matrices:
            for text, expected in cases:
                with self.subTest(range=(minimum, maximum), text=text):
                    self.assert_projection(text, expected, minimum, maximum)

    def test_i64_extremes_and_ranges(self):
        for text, expected in (
            (str(I64_MIN), I64_MIN), (str(I64_MAX), I64_MAX),
            ("-09223372036854775808", I64_MIN),
            ("+09223372036854775807", I64_MAX),
            ("9223372036854775808", None),
            ("-9223372036854775809", None),
        ):
            with self.subTest(text=text):
                self.assert_projection(text, expected, I64_MIN, I64_MAX)
        self.assert_projection(str(I64_MAX), None, I64_MIN, I64_MAX - 1)

    def test_giant_leading_zeroes_and_significant_digit_bounds(self):
        zeros = "0" * 12000
        self.assert_projection(zeros + "00042", 42, -100, 100)
        self.assert_projection("-" + zeros + "00042", -42, -100, 100)
        self.assert_projection(zeros + "9" * 200, None, -100, 100)

    def test_requested_range_and_arguments_are_validated(self):
        for minimum, maximum in ((2, 1), (I64_MIN - 1, 0), (0, I64_MAX + 1)):
            with self.subTest(bounds=(minimum, maximum)), self.assertRaises(ValueError):
                xml_integer_sql("value", minimum, maximum)
        for minimum, maximum in ((True, 1), (0, 1.0)):
            with self.subTest(bounds=(minimum, maximum)), self.assertRaises(TypeError):
                xml_integer_sql("value", minimum, maximum)
        for expression in ("", "  ", None):
            with self.subTest(expression=expression), self.assertRaises(ValueError):
                xml_integer_sql(expression, 0, 1)


if __name__ == "__main__":
    unittest.main(verbosity=2)
