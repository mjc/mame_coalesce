"""Focused checks for SQLite XML text and name projections."""

import sqlite3
import unittest
from contextlib import closing

from xml_text_sql import (
    xml_collapse_sql,
    xml_language_valid_sql,
    xml_name_valid_sql,
    xml_qname_valid_sql,
)


class XmlTextSqlChecks(unittest.TestCase):
    def project(self, helper, value, *arguments, **keywords):
        expression = helper("?", *arguments, **keywords)
        with closing(sqlite3.connect(":memory:")) as connection:
            return connection.execute(f"SELECT {expression}", (value,)).fetchone()[0]

    def test_collapse_only_xml_whitespace(self):
        cases = (
            ("", ""), (" \t\n\r ", ""),
            ("a\tb\nc\rd", "a b c d"),
            ("a  \t\n\r  b", "a b"),
            ("\v\f\u00a0", "\v\f\u00a0"),
            ("a\x00  b", "a\x00 b"), ("a  \x00", "a \x00"),
            ("\x00  a", "\x00 a"), ("a \x00", "a \x00"),
            (None, None), (7, 7),
        )
        for value, expected in cases:
            with self.subTest(value=value):
                self.assertEqual(self.project(xml_collapse_sql, value), expected)
        self.assertEqual(self.project(xml_collapse_sql, "a" + " " * 5001 + "b"), "a b")

    def test_name_start_and_character_unicode_boundaries(self):
        start_ranges = (
            (0x41, 0x5A), (0x5F, 0x5F), (0x61, 0x7A),
            (0xC0, 0xD6), (0xD8, 0xF6), (0xF8, 0x2FF),
            (0x370, 0x37D), (0x37F, 0x1FFF), (0x200C, 0x200D),
            (0x2070, 0x218F), (0x2C00, 0x2FEF), (0x3001, 0xD7FF),
            (0xF900, 0xFDCF), (0xFDF0, 0xFFFD), (0x10000, 0xEFFFF),
        )
        char_ranges = tuple(sorted(start_ranges + (
            (0x2D, 0x2E), (0x30, 0x39), (0xB7, 0xB7),
            (0x300, 0x36F), (0x203F, 0x2040),
        )))
        merged = []
        for low, high in char_ranges:
            if merged and low <= merged[-1][1] + 1:
                merged[-1] = (merged[-1][0], max(merged[-1][1], high))
            else:
                merged.append((low, high))

        for low, high in start_ranges:
            for scalar in (low, high):
                with self.subTest(start_boundary=hex(scalar)):
                    self.assertEqual(self.project(xml_name_valid_sql, chr(scalar)), 1)
        for (_, previous_high), (next_low, _) in zip(start_ranges, start_ranges[1:]):
            for scalar in (previous_high + 1, next_low - 1):
                if previous_high < scalar < next_low:
                    if not 0xD800 <= scalar <= 0xDFFF:
                        with self.subTest(start_hole=hex(scalar)):
                            self.assertEqual(self.project(xml_name_valid_sql, chr(scalar)), 0)

        for low, high in char_ranges:
            for scalar in (low, high):
                with self.subTest(character_boundary=hex(scalar)):
                    self.assertEqual(self.project(xml_name_valid_sql, "a" + chr(scalar)), 1)
        for (_, previous_high), (next_low, _) in zip(merged, merged[1:]):
            for scalar in (previous_high + 1, next_low - 1):
                if previous_high < scalar < next_low:
                    if not 0xD800 <= scalar <= 0xDFFF:
                        with self.subTest(character_hole=hex(scalar)):
                            self.assertEqual(self.project(xml_name_valid_sql, "a" + chr(scalar)), 0)
        for scalar in (0xFDD0, 0xFFFE, 0xFFFF, 0xF0000):
            with self.subTest(noncharacter_or_above_end=hex(scalar)):
                self.assertEqual(self.project(xml_name_valid_sql, chr(scalar)), 0)

    def test_name_options_and_rejections(self):
        self.assertEqual(self.project(xml_name_valid_sql, "a:b"), 0)
        self.assertEqual(self.project(xml_name_valid_sql, "a:b", allow_colon=True), 1)
        self.assertEqual(self.project(xml_name_valid_sql, "1name"), 0)
        self.assertEqual(self.project(xml_name_valid_sql, "1name", require_start=False), 1)
        for value in (None, 0, "", "\x00", "a\x00b", "a b", "a\v", "a\f", "a\u00a0"):
            with self.subTest(value=value):
                self.assertEqual(self.project(xml_name_valid_sql, value), 0)
        for options in ((1, True), (False, 0)):
            with self.subTest(options=options), self.assertRaises(TypeError):
                xml_name_valid_sql("value", *options)

    def test_qnames_are_one_or_two_nc_names(self):
        cases = (
            ("root", 1), ("π:名", 1), ("𐀀:𐀀name", 1),
            ("a:b:c", 0), (":local", 0), ("prefix:", 0),
            ("1prefix:local", 0), ("prefix:1local", 0),
            ("a b", 0), ("a\x00:b", 0), (None, 0), (4, 0), ("", 0),
        )
        for value, expected in cases:
            with self.subTest(value=value):
                self.assertEqual(self.project(xml_qname_valid_sql, value), expected)

    def test_language_segments_and_ascii_rules(self):
        cases = (
            ("a", 1), ("abcdefgh", 1), ("abcdefghi", 0),
            ("en-US", 1), ("i-klingon-419", 1), ("a-12345678", 1),
            ("a-123456789", 0), ("-en", 0), ("en-", 0), ("en--US", 0),
            ("1en", 0), ("en-US-", 0), ("en_US", 0), ("en US", 0),
            ("é", 0), ("en-ß", 0), ("\x00en", 0), (None, 0), (5, 0), ("", 0),
            ("en" + "-a" * 1500, 1),
        )
        for value, expected in cases:
            label = value if isinstance(value, str) and len(value) < 40 else "long/nontext"
            with self.subTest(value=label):
                self.assertEqual(self.project(xml_language_valid_sql, value), expected)


if __name__ == "__main__":
    unittest.main(verbosity=2)
