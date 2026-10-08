"""Focused checks for the query-only native file-size projection."""

import sqlite3
import unittest

import native_file_sizes


MAX_I64 = 9223372036854775807


class NativeFileSizesCheck(unittest.TestCase):
    def setUp(self):
        self.db = sqlite3.connect(":memory:")
        for table in ("mame_roms", "no_intro_dat_rom_claims",
                      "no_intro_dump_files", "no_intro_release_files"):
            self.db.execute(
                f"CREATE TABLE {table} (media_entry_id INTEGER PRIMARY KEY, size_text TEXT)"
            )
        for table, plus in (("clrmamepro_roms", False), ("logiqx_roms", False),
                            ("no_intro_pc_file_claims", True)):
            value = native_file_sizes._decimal_i64_sql("size_text", allow_plus=plus)
            virtual_column = "size_i64" if table == "no_intro_pc_file_claims" else "size_value"
            self.db.execute(
                f"CREATE TABLE {table} (media_entry_id INTEGER PRIMARY KEY, "
                f"size_text TEXT, {virtual_column} INTEGER GENERATED ALWAYS AS ({value}) VIRTUAL)"
            )
        self.db.executescript("""
CREATE TABLE catalog_media_entries (media_entry_id INTEGER PRIMARY KEY);
CREATE TABLE candidate_software_file_lengths (
    media_entry_id INTEGER PRIMARY KEY,
    byte_length INTEGER
);
CREATE TABLE catalog_reading_rules (
    reading_rules_id INTEGER PRIMARY KEY,
    format_family TEXT NOT NULL,
    dialect TEXT NOT NULL
);
CREATE TABLE catalog_editions (
    edition_id INTEGER PRIMARY KEY,
    reading_rules_id INTEGER NOT NULL
);
CREATE TABLE catalog_source_elements (
    source_element_id INTEGER PRIMARY KEY,
    edition_id INTEGER NOT NULL,
    element_kind TEXT NOT NULL
);
""")
        for edition, dialect in enumerate((
            "no-intro-dat-v3-strict",
            "no-intro-dat-v4-strict",
            "no-intro-dat-v3-compatible",
            "no-intro-dat-v4-compatible",
            "ordinary",
        ), start=1):
            self.db.execute(
                "INSERT INTO catalog_reading_rules VALUES (?, ?, ?)",
                (edition, "no_intro_dat" if edition < 5 else "other", dialect),
            )
            self.db.execute(
                "INSERT INTO catalog_editions VALUES (?, ?)", (edition, edition)
            )
        self.db.execute(native_file_sizes.sql())

    def tearDown(self):
        self.db.close()

    def add(self, table, media_id, text):
        kinds = {
            "mame_roms": "mame_rom",
            "clrmamepro_roms": "clrmamepro_rom",
            "logiqx_roms": "logiqx_rom",
            "no_intro_dat_rom_claims": "no_intro_dat_rom",
            "no_intro_pc_file_claims": "no_intro_pc_rom",
            "no_intro_dump_files": "no_intro_export_source_file",
            "no_intro_release_files": "no_intro_export_release_file",
        }
        self.db.execute("INSERT INTO catalog_media_entries VALUES (?)", (media_id,))
        self.db.execute(
            "INSERT INTO catalog_source_elements VALUES (?,?,?)",
            (media_id, 5, kinds[table]),
        )
        self.db.execute(
            f"INSERT INTO {table}(media_entry_id,size_text) VALUES (?,?)",
            (media_id, text),
        )

    def add_dat(self, media_id, edition, text):
        self.db.execute("INSERT INTO catalog_media_entries VALUES (?)", (media_id,))
        self.db.execute(
            "INSERT INTO catalog_source_elements VALUES (?,?,?)",
            (media_id, edition, "no_intro_dat_rom"),
        )
        self.db.execute("INSERT INTO no_intro_dat_rom_claims VALUES (?,?)", (media_id, text))

    def result(self, media_id):
        return self.db.execute(
            "SELECT source_size_field,byte_length,size_state "
            "FROM candidate_native_file_sizes WHERE media_entry_id=?",
            (media_id,),
        ).fetchone()

    def test_native_family_literal_positive_and_negative_controls(self):
        fixtures = (
            ("mame_roms", 101, "0007", ("size", 7, "value")),
            ("mame_roms", 102, None, ("size", None, "omitted")),
            ("mame_roms", 103, "+7", ("size", None, "unusable")),
            ("mame_roms", 104, str(MAX_I64 + 1), ("size", None, "unusable")),
            ("mame_roms", 105, str(MAX_I64), ("size", MAX_I64, "value")),
            ("clrmamepro_roms", 201, "000", ("size", 0, "value")),
            ("clrmamepro_roms", 202, None, ("size", None, "omitted")),
            ("clrmamepro_roms", 203, "1.0", ("size", None, "unusable")),
            ("clrmamepro_roms", 204, str(MAX_I64 + 1), ("size", None, "unusable")),
            ("clrmamepro_roms", 205, str(MAX_I64), ("size", MAX_I64, "value")),
            ("logiqx_roms", 301, "0009", ("size", 9, "value")),
            ("logiqx_roms", 302, None, ("size", None, "omitted")),
            ("logiqx_roms", 303, " 9", ("size", None, "unusable")),
            ("logiqx_roms", 304, str(MAX_I64 + 1), ("size", None, "unusable")),
            ("logiqx_roms", 305, str(MAX_I64), ("size", MAX_I64, "value")),
            ("no_intro_pc_file_claims", 401, "+0009", ("size", 9, "value")),
            ("no_intro_pc_file_claims", 402, None, ("size", None, "omitted")),
            ("no_intro_pc_file_claims", 403, "+", ("size", None, "unusable")),
            ("no_intro_pc_file_claims", 404, str(MAX_I64 + 1), ("size", None, "unusable")),
            ("no_intro_pc_file_claims", 405, str(MAX_I64), ("size", MAX_I64, "value")),
            ("no_intro_dump_files", 501, "00011", ("size", 11, "value")),
            ("no_intro_dump_files", 502, None, ("size", None, "omitted")),
            ("no_intro_dump_files", 503, "11x", ("size", None, "unusable")),
            ("no_intro_dump_files", 504, str(MAX_I64 + 1), ("size", None, "unusable")),
            ("no_intro_dump_files", 505, str(MAX_I64), ("size", MAX_I64, "value")),
            ("no_intro_release_files", 601, "12", ("size", 12, "value")),
            ("no_intro_release_files", 602, None, ("size", None, "omitted")),
            ("no_intro_release_files", 603, "-0", ("size", None, "unusable")),
            ("no_intro_release_files", 604, str(MAX_I64 + 1), ("size", None, "unusable")),
            ("no_intro_release_files", 605, str(MAX_I64), ("size", MAX_I64, "value")),
        )
        for table, media_id, raw, expected in fixtures:
            with self.subTest(table=table, raw=raw):
                self.add(table, media_id, raw)
                self.assertEqual(self.result(media_id), expected)
                self.assertEqual(
                    self.db.execute(
                        f"SELECT size_text FROM {table} WHERE media_entry_id=?",
                        (media_id,),
                    ).fetchone(),
                    (raw,),
                )

    def test_software_full_file_length_is_never_optional(self):
        for media_id in (701, 702, 703):
            self.db.execute("INSERT INTO catalog_media_entries VALUES (?)", (media_id,))
            self.db.execute(
                "INSERT INTO catalog_source_elements VALUES (?,5,'software_rom_entry')",
                (media_id,),
            )
        self.db.executemany(
            "INSERT INTO candidate_software_file_lengths VALUES (?,?)",
            ((701, 123), (702, None), (703, MAX_I64)),
        )
        self.assertEqual(self.result(701), ("file_length", 123, "value"))
        self.assertEqual(self.result(702), ("file_length", None, "unusable"))
        self.assertEqual(self.result(703), ("file_length", MAX_I64, "value"))

    def test_strict_dat_v3_and_v4_use_xs_unsigned_int_contract(self):
        controls = (
            (801, 1, " \t+00042\r", ("size", 42, "value")),
            (802, 1, "-000", ("size", 0, "value")),
            (803, 1, "4294967295", ("size", 4294967295, "value")),
            (804, 1, "4294967296", ("size", None, "unusable")),
            (805, 1, "-1", ("size", None, "unusable")),
            (806, 1, "1 2", ("size", None, "unusable")),
            (807, 1, None, ("size", None, "omitted")),
            (808, 2, "\n+7\t", ("size", 7, "value")),
            (809, 2, "+", ("size", None, "unusable")),
        )
        for media_id, edition, raw, expected in controls:
            with self.subTest(edition=edition, raw=raw):
                self.add_dat(media_id, edition, raw)
                self.assertEqual(self.result(media_id), expected)

    def test_compatible_dat_v3_and_v4_use_descriptive_decimal_projection(self):
        controls = (
            (901, 3, "00042", ("size", 42, "value")),
            (902, 3, "+42", ("size", None, "unusable")),
            (903, 3, " 42", ("size", None, "unusable")),
            (904, 4, "42", ("size", 42, "value")),
            (905, 4, None, ("size", None, "omitted")),
        )
        for media_id, edition, raw, expected in controls:
            with self.subTest(edition=edition, raw=raw):
                self.add_dat(media_id, edition, raw)
                self.assertEqual(self.result(media_id), expected)

    def test_lookup_uses_each_native_primary_key_and_has_constant_bytecode(self):
        self.add("mame_roms", 101, "7")
        query = (
            "SELECT source_size_field,byte_length,size_state "
            "FROM candidate_native_file_sizes WHERE media_entry_id=?"
        )
        plan = "\n".join(
            row[3]
            for row in self.db.execute("EXPLAIN QUERY PLAN " + query, (1,)).fetchall()
        )
        self.assertIn("SEARCH media USING INTEGER PRIMARY KEY", plan)
        for lookup in (
            "SEARCH source USING INTEGER PRIMARY KEY",
            "SEARCH media USING INTEGER PRIMARY KEY",
            "SEARCH mame_roms USING INTEGER PRIMARY KEY",
            "SEARCH clrmamepro_roms USING INTEGER PRIMARY KEY",
            "SEARCH logiqx_roms USING INTEGER PRIMARY KEY",
            "SEARCH rom USING INTEGER PRIMARY KEY",
            "SEARCH no_intro_pc_file_claims USING INTEGER PRIMARY KEY",
            "SEARCH no_intro_dump_files USING INTEGER PRIMARY KEY",
            "SEARCH no_intro_release_files USING INTEGER PRIMARY KEY",
        ):
            self.assertIn(lookup, plan)
        self.assertNotIn("MATERIALIZE", plan)

        def vm_steps():
            steps = 0

            def count():
                nonlocal steps
                steps += 1
                return 0

            self.db.set_progress_handler(count, 1)
            try:
                self.result(101)  # warm statement preparation/cache outside measurement
                steps = 0
                self.assertEqual(self.result(101), ("size", 7, "value"))
            finally:
                self.db.set_progress_handler(None, 0)
            return steps

        before = vm_steps()
        self.db.executemany(
            "INSERT INTO catalog_media_entries VALUES (?)",
            ((media_id,) for media_id in range(10_000, 11_024)),
        )
        self.db.executemany(
            "INSERT INTO catalog_source_elements VALUES (?,5,'mame_rom')",
            ((media_id,) for media_id in range(10_000, 11_024)),
        )
        self.db.executemany(
            "INSERT INTO mame_roms VALUES (?,?)",
            ((media_id, "1") for media_id in range(10_000, 11_024)),
        )
        after = vm_steps()
        self.assertEqual(after, before)
        self.assertIsNone(self.result(999_999))

    def test_view_is_query_only_with_the_closed_four_column_interface(self):
        self.assertEqual(
            tuple(row[1] for row in self.db.execute(
                "PRAGMA table_info(candidate_native_file_sizes)")),
            ("media_entry_id", "source_size_field", "byte_length", "size_state"),
        )
        self.assertEqual(
            self.db.execute(
                "SELECT type FROM sqlite_schema WHERE name='candidate_native_file_sizes'"
            ).fetchone(),
            ("view",),
        )


if __name__ == "__main__":
    unittest.main()
