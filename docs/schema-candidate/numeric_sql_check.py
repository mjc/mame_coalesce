#!/usr/bin/env python3
"""Literal boundary checks for the MAME software integer SQL fragment."""

from contextlib import closing
import sqlite3
import unittest

from numeric_sql import software_integer_sql


class SoftwareIntegerSqlChecks(unittest.TestCase):
    def test_literal_parse_number_matrix(self):
        cases = (
            (None, None),
            ("", None),
            ("0", 0),
            ("00", 0),
            ("010", 8),
            ("012", 10),
            ("0000000000000000000000000000000000000007", 7),
            ("10", 10),
            ("08", None),
            ("09", None),
            ("0x0", 0),
            ("0XfF", 255),
            ("0x7fffffffffffffff", 9223372036854775807),
            ("0X8000000000000000", None),
            ("0x", None),
            ("0xg", None),
            ("0x+1", None),
            ("+1", None),
            ("1+0", None),
            ("-1", None),
            (" 1", None),
            ("1 ", None),
            ("1x", None),
            ("١", None),
            ("0\x001", None),
            ("1\x00", None),
            ("9" * 40, None),
            (1, None),
        )
        value_sql = software_integer_sql("input.value")
        query = (
            "WITH input(value) AS (VALUES (?)) "
            f"SELECT ({value_sql}) FROM input"
        )

        with closing(sqlite3.connect(":memory:")) as connection:
            for source, expected in cases:
                with self.subTest(source=source):
                    self.assertEqual(
                        connection.execute(query, (source,)).fetchone(),
                        (expected,),
                    )

    def test_rejects_signed_range_overflow_in_octal_and_hex(self):
        cases = (
            ("0777777777777777777777", 9223372036854775807),
            ("01000000000000000000000", None),
            ("0x7fffffffffffffff", 9223372036854775807),
            ("0x8000000000000000", None),
        )
        value_sql = software_integer_sql("input.value")
        query = (
            "WITH input(value) AS (VALUES (?)) "
            f"SELECT ({value_sql}) FROM input"
        )

        with closing(sqlite3.connect(":memory:")) as connection:
            for source, expected in cases:
                with self.subTest(source=source):
                    self.assertEqual(
                        connection.execute(query, (source,)).fetchone(),
                        (expected,),
                    )

    def test_empty_expression_is_rejected(self):
        with self.assertRaises(ValueError):
            software_integer_sql(" ")


if __name__ == "__main__":
    unittest.main()
