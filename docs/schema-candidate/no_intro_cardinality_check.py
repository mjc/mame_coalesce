#!/usr/bin/env python3
"""Exercise No-Intro cardinality audits against assembled candidate DDL.

Run in the repository devenv:
  python3 -Werror::ResourceWarning docs/schema-candidate/no_intro_cardinality_check.py

Fixtures are constructed relational rows, not parser or corpus evidence.
"""
import pathlib
import sqlite3
import sys
import unittest

sys.dont_write_bytecode = True
import assemble
import count_fixtures

ROOT = pathlib.Path(__file__).resolve().parent


class NoIntroCardinality(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.db = sqlite3.connect(":memory:")
        cls.addClassCleanup(cls.db.close)
        cls.db.execute("PRAGMA foreign_keys=ON")
        cls.db.executescript(assemble.assemble())
        views = {row[0] for row in cls.db.execute(
            "SELECT name FROM sqlite_schema WHERE type='view'")}
        if "candidate_no_intro_cardinality_problems" not in views:
            raise AssertionError("assembled schema omitted the No-Intro cardinality audit")
        columns = tuple(row[1] for row in cls.db.execute(
            "PRAGMA table_info(candidate_no_intro_cardinality_problems)"))
        if columns != ("problem", "owner_id", "edition_id"):
            raise AssertionError(f"unexpected cardinality audit interface: {columns!r}")
        # Keep only the complete positive setup; the fixture's later corruption
        # directives are tested in their own dedicated witness runner.
        setup = (ROOT / "no_intro_witnesses.sql").read_text().split(
            "\nINSERT INTO no_intro_witness_assertions", 1)[0]
        cls.db.executescript(setup)
        cls.db.execute("RELEASE no_intro_candidate_witness")
        count_fixtures.seal(cls.db, "no_intro_dat", 50, {
            "header_count": 1, "header_text_child_count": 4,
            "game_count": 1, "game_description_count": 1, "category_count": 1,
            "rom_count": 1, "game_attribute_position_count": 3,
            "rom_attribute_position_count": 3,
        })
        count_fixtures.seal(cls.db, "no_intro_pc_fixture", 51, {
            "header_count": 1, "header_name_child_count": 1,
            "header_version_child_count": 1, "game_count": 2,
            "language_token_count": 1, "game_description_count": 1,
            "rom_count": 1, "game_attribute_position_count": 6,
            "rom_attribute_position_count": 2,
        })

    def setUp(self):
        self.db.execute("SAVEPOINT cardinality_case")
        for edition, revision, mode in (
            (54, 3, "strict"), (55, 4, "strict"),
            (56, 3, "compatible"), (57, 4, "compatible"),
        ):
            self.make_dat_edition(edition, revision, mode)

    def tearDown(self):
        self.db.execute("ROLLBACK TO cardinality_case")
        self.db.execute("RELEASE cardinality_case")

    def make_dat_edition(self, edition, revision, mode):
        strict = mode == "strict"
        base, group = edition * 1000, edition + 100
        source_file, rules = edition + 1000, edition + 2000
        self.db.execute(
            "INSERT INTO catalog_source_files VALUES (?,?,NULL,100,?,'zstd')",
            (source_file, bytes([edition]) * 32, f"witness/dat-{edition}.zst"))
        self.db.execute(
            "INSERT INTO catalog_reading_rules VALUES (?,?,?,?,?,?,?)",
            (rules, f"witness-dat-{edition}", "no_intro_dat",
             f"no-intro-dat-v{revision}-{'strict' if strict else 'compatible'}",
             f"v{revision}", "reader", mode))
        self.db.execute("INSERT INTO catalog_editions VALUES (?,?,?,?,?,NULL,NULL)",
                        (edition, 10, source_file, rules, 40))
        self.db.execute("INSERT INTO catalog_set_groups VALUES (?,?, 'root')", (group, edition))

        header_fields = [("id", "0"), ("name", ""), ("description", ""), ("version", "1")]
        if strict:
            header_fields.append(("author", ""))
        registry = [(base, edition, "no_intro_dat_header"),
                    (base + 10, edition, "no_intro_dat_game"),
                    (base + 11, edition, "no_intro_dat_game_description"),
                    (base + 12, edition, "no_intro_dat_rom")]
        registry.extend((base + offset, edition, "no_intro_dat_header_text_child")
                        for offset in range(1, len(header_fields) + 1))
        self.db.executemany("INSERT INTO catalog_source_elements VALUES (?,?,?)", registry)
        self.db.execute(
            "INSERT INTO no_intro_dat_documents "
            "(edition_id,location_view,start_line,start_column,column_convention) "
            "VALUES (?,'transport_decoded_xml_text',1,1,'one_based_unicode_scalar')", (edition,))
        self.db.execute("INSERT INTO no_intro_dat_headers VALUES (?,?,0,1,1)", (base, group))
        for offset, (field_kind, value) in enumerate(header_fields, 1):
            self.db.execute(
                "INSERT INTO no_intro_dat_header_text_children VALUES (?,?,?,?,?,1,10)",
                (base + offset, base, field_kind, value, offset - 1))

        self.db.execute("INSERT INTO catalog_sets VALUES (?,?, 'game',1,2,8)", (base + 10, group))
        self.db.execute("INSERT INTO no_intro_dat_games VALUES (?,NULL)", (base + 10,))
        self.db.execute(
            "INSERT INTO no_intro_dat_game_field_positions VALUES (?, 'name',0,0,2,8,NULL)",
            (base + 10,))
        self.db.execute("INSERT INTO no_intro_dat_game_descriptions VALUES (?,?, '',0,2,1)",
                        (base + 11, base + 10))
        self.db.execute("INSERT INTO catalog_media_entries VALUES (?,NULL)", (base + 12,))
        self.db.execute(
            "INSERT INTO no_intro_dat_rom_claims "
            "(media_entry_id,set_id,name,size_text,source_order,source_line,source_column) "
            "VALUES (?,?, 'game.rom',?,1,2,20)",
            (base + 12, base + 10, "0" if strict else None))
        self.db.execute("INSERT INTO no_intro_dat_rom_field_positions VALUES (?, 'name',0,0,2,21,NULL)",
                        (base + 12,))
        if strict:
            fields = (("crc", "crc32", bytes([edition]) * 4),
                      ("md5", "md5", bytes([edition]) * 16),
                      ("sha1", "sha1", bytes([edition]) * 20))
            for index, (field, algorithm, digest) in enumerate(fields, 1):
                hash_id, reported_id = base + 100 + index, base + 200 + index
                self.db.execute("INSERT INTO hash_values VALUES (?,?,?)", (hash_id, algorithm, digest))
                self.db.execute(
                    "INSERT INTO catalog_entry_hashes "
                    "(reported_hash_id,media_entry_id,source_hash_field,field_occurrence,presence,hash_scope,hash_id) "
                    "VALUES (?,?,?,0,'value','unknown',?)",
                    (reported_id, base + 12, field, hash_id))
                self.db.execute("INSERT INTO no_intro_dat_rom_field_positions VALUES (?, ?,0,?,2,22,?)",
                                (base + 12, field, index + 1, reported_id))
            self.db.execute(
                "INSERT INTO no_intro_dat_rom_field_positions "
                "(media_entry_id,field_kind,field_occurrence,source_order,source_line,source_column) "
                "VALUES (?, 'size',0,1,2,22)", (base + 12,))
        count_fixtures.seal(self.db, "no_intro_dat", edition, {
            "header_count": 1,
            "header_text_child_count": len(header_fields),
            "game_count": 1,
            "game_description_count": 1,
            "rom_count": 1,
            "game_attribute_position_count": 1,
            "rom_attribute_position_count": 5 if strict else 1,
        })

    def cardinality(self, edition=None):
        query = "SELECT problem,owner_id,edition_id FROM candidate_no_intro_cardinality_problems"
        args = ()
        if edition is not None:
            query += " WHERE edition_id=?"
            args = (edition,)
        return self.db.execute(query, args).fetchall()

    def integrity(self, view, edition):
        return self.db.execute(
            f"SELECT problem,owner_id,edition_id FROM {view} WHERE edition_id=?", (edition,)).fetchall()

    def test_all_four_dat_modes_accept_their_minimal_complete_shape(self):
        for edition in (54, 55, 56, 57):
            with self.subTest(edition=edition):
                self.assertEqual(self.cardinality(edition), [])
                self.assertEqual(self.integrity("candidate_no_intro_integrity_problems", edition), [])
                self.publish(edition)
        # Required text counts are based on present child rows. Empty simple
        # text remains present; lexical restrictions belong to parser policy.
        self.assertEqual(self.cardinality(50), [])
        self.assertEqual(self.integrity("candidate_no_intro_integrity_problems", 50), [])
        # The same composed positive data includes the synthetic P/C fixture
        # and both observed export envelopes (nested and sibling-root header).
        for edition in (51, 52, 53):
            self.assertEqual(self.cardinality(edition), [])
            self.assertEqual(self.integrity("candidate_no_intro_integrity_problems", edition), [])

    def test_root_header_after_game_blocks_all_modes_then_repair_publishes(self):
        for edition in (54, 55, 56, 57):
            base = edition * 1000
            with self.subTest(edition=edition):
                self.db.execute(
                    "UPDATE no_intro_dat_headers SET source_order=2 WHERE header_id=?", (base,))
                self.db.execute(
                    "UPDATE catalog_sets SET source_order=0 WHERE set_id=?", (base + 10,))
                self.assertEqual(self.cardinality(edition), [
                    ("dat_header_after_game", base, edition)])
                with self.assertRaisesRegex(sqlite3.IntegrityError,
                                            "publication requires complete closure"):
                    self.publish(edition)

                self.db.execute(
                    "UPDATE catalog_sets SET source_order=3 WHERE set_id=?", (base + 10,))
                self.db.execute(
                    "UPDATE no_intro_dat_headers SET source_order=0 WHERE header_id=?", (base,))
                self.db.execute(
                    "UPDATE catalog_sets SET source_order=1 WHERE set_id=?", (base + 10,))
                self.assertEqual(self.integrity("candidate_integrity_problems", edition), [])
                self.publish(edition)

    def test_strict_header_and_game_child_order_block_then_repair(self):
        for edition in (54, 55):
            base = edition * 1000
            with self.subTest(edition=edition, sequence="header"):
                # The retained header child ordinals expose id after author.
                self.db.execute(
                    "UPDATE no_intro_dat_header_text_children SET source_order=5 "
                    "WHERE source_element_id=?", (base + 1,))
                self.assertIn(("dat_strict_header_child_order", base, edition),
                              self.cardinality(edition))
                with self.assertRaisesRegex(sqlite3.IntegrityError,
                                            "publication requires complete closure"):
                    self.publish(edition)
                self.db.execute(
                    "UPDATE no_intro_dat_header_text_children SET source_order=0 "
                    "WHERE source_element_id=?", (base + 1,))

            with self.subTest(edition=edition, sequence="game"):
                # The ROM child now precedes its required description.
                self.db.execute(
                    "UPDATE no_intro_dat_game_descriptions SET source_order=2 "
                    "WHERE source_element_id=?", (base + 11,))
                self.db.execute(
                    "UPDATE no_intro_dat_rom_claims SET source_order=0 "
                    "WHERE media_entry_id=?", (base + 12,))
                self.assertIn(("dat_strict_game_child_order", base + 10, edition),
                              self.cardinality(edition))
                with self.assertRaisesRegex(sqlite3.IntegrityError,
                                            "publication requires complete closure"):
                    self.publish(edition)
                self.db.execute(
                    "UPDATE no_intro_dat_rom_claims SET source_order=3 "
                    "WHERE media_entry_id=?", (base + 12,))
                self.db.execute(
                    "UPDATE no_intro_dat_game_descriptions SET source_order=0 "
                    "WHERE source_element_id=?", (base + 11,))
                self.db.execute(
                    "UPDATE no_intro_dat_rom_claims SET source_order=1 "
                    "WHERE media_entry_id=?", (base + 12,))
                self.assertEqual(self.integrity("candidate_integrity_problems", edition), [])
                self.publish(edition)

    def test_compatible_header_and_game_children_retain_order_freedom(self):
        for edition in (56, 57):
            base = edition * 1000
            with self.subTest(edition=edition):
                self.db.execute(
                    "UPDATE no_intro_dat_header_text_children SET source_order=source_order+10 "
                    "WHERE header_id=?", (base,))
                for field_kind, source_order in (
                    ("id", 2), ("name", 0), ("description", 3), ("version", 1),
                ):
                    self.db.execute(
                        "UPDATE no_intro_dat_header_text_children SET source_order=? "
                        "WHERE header_id=? AND field_kind=?",
                        (source_order, base, field_kind))
                self.db.execute(
                    "UPDATE no_intro_dat_game_descriptions SET source_order=2 "
                    "WHERE source_element_id=?", (base + 11,))
                self.db.execute(
                    "UPDATE no_intro_dat_rom_claims SET source_order=0 "
                    "WHERE media_entry_id=?", (base + 12,))
                self.db.execute(
                    "UPDATE no_intro_dat_game_descriptions SET source_order=1 "
                    "WHERE source_element_id=?", (base + 11,))
                self.assertEqual(self.cardinality(edition), [])
                self.assertEqual(self.integrity("candidate_integrity_problems", edition), [])
                self.publish(edition)

    def test_required_dat_header_and_game_text_children_are_missing_in_every_mode(self):
        for edition in (54, 55, 56, 57):
            base = edition * 1000
            header_text_id, game_text_id = base + 3, base + 11
            with self.subTest(edition=edition, owner="header"):
                self.db.execute("DELETE FROM no_intro_dat_header_text_children WHERE source_element_id=?",
                                (header_text_id,))
                self.assertIn(("flat_dat_header_missing_or_incomplete", edition, edition),
                              self.integrity("candidate_no_intro_integrity_problems", edition))
                self.db.execute("INSERT INTO no_intro_dat_header_text_children VALUES (?,?,'description','',2,1,10)",
                                (header_text_id, base))
                self.db.execute(
                    "UPDATE no_intro_dat_header_text_children SET field_kind='url' "
                    "WHERE source_element_id=?", (header_text_id,))
                self.assertIn(("flat_dat_header_missing_or_incomplete", edition, edition),
                              self.integrity("candidate_no_intro_integrity_problems", edition))
                self.db.execute(
                    "UPDATE no_intro_dat_header_text_children SET field_kind='description' "
                    "WHERE source_element_id=?", (header_text_id,))
            with self.subTest(edition=edition, owner="game"):
                self.db.execute("DELETE FROM no_intro_dat_game_descriptions WHERE source_element_id=?",
                                (game_text_id,))
                self.assertIn(("flat_dat_game_children_missing_or_mismatched", base + 10, edition),
                              self.integrity("candidate_no_intro_integrity_problems", edition))
                self.db.execute("INSERT INTO no_intro_dat_game_descriptions VALUES (?,?, '',0,2,1)",
                                (game_text_id, base + 10))

    def test_strict_modes_report_each_missing_rom_field_and_clear_on_repair(self):
        for edition in (54, 55):
            rom_id = edition * 1000 + 12
            for field in ("size", "crc", "md5", "sha1"):
                with self.subTest(edition=edition, field=field):
                    self.db.execute(
                        "DELETE FROM no_intro_dat_rom_field_positions "
                        "WHERE media_entry_id=? AND field_kind=?", (rom_id, field))
                    self.assertEqual(self.cardinality(edition), [
                        (f"dat_strict_rom_field_missing:{field}", rom_id, edition)])
                    if field == "size":
                        self.db.execute(
                            "INSERT INTO no_intro_dat_rom_field_positions "
                            "(media_entry_id,field_kind,field_occurrence,source_order,source_line,source_column) "
                            "VALUES (?, 'size',0,1,2,22)", (rom_id,))
                    else:
                        index = {"crc": 1, "md5": 2, "sha1": 3}[field]
                        base = edition * 1000
                        self.db.execute(
                            "INSERT INTO no_intro_dat_rom_field_positions "
                            "VALUES (?, ?,0,?,2,22,?)",
                            (rom_id, field, index + 1, base + 200 + index))
                    self.assertEqual(self.cardinality(edition), [])

    def test_compatible_modes_do_not_require_optional_rom_fields(self):
        for edition in (56, 57):
            self.assertEqual(self.cardinality(edition), [])
        # v3 compatible accepts header fields omitted by strict v3 and accepts
        # repeated compatible game_id and ROM children. Present empty values
        # still have an owner row.
        base = 56000
        for offset, kind, field, value, order in (
            (6, "no_intro_dat_header_text_child", "trademarks", "", 4),
            (7, "no_intro_dat_header_text_child", "piracy", "notice", 5),
            (8, "no_intro_dat_header_text_child", "comment", "", 6),
            (9, "no_intro_dat_identifier", None, "", 3),
        ):
            self.db.execute("INSERT INTO catalog_source_elements VALUES (?,?,?)", (base + offset, 56, kind))
            if field is not None:
                self.db.execute(
                    "INSERT INTO no_intro_dat_header_text_children VALUES (?,?,?,?,?,1,30)",
                    (base + offset, base, field, value, order))
            else:
                self.db.execute(
                    "INSERT INTO no_intro_dat_identifiers VALUES (?,?,?, ?,1,30)",
                    (base + offset, base + 10, value, order))
        self.add_rom(56, base + 13, "second.rom", 2)
        self.assertEqual(self.cardinality(56), [])
        self.assertEqual(self.integrity("candidate_no_intro_integrity_problems", 56), [])

    def add_rom(self, edition, media_id, name, child_order):
        game_id = edition * 1000 + 10
        self.db.execute("INSERT INTO catalog_source_elements VALUES (?,?, 'no_intro_dat_rom')",
                        (media_id, edition))
        self.db.execute("INSERT INTO catalog_media_entries VALUES (?,NULL)", (media_id,))
        self.db.execute(
            "INSERT INTO no_intro_dat_rom_claims "
            "(media_entry_id,set_id,name,source_order,source_line,source_column) "
            "VALUES (?,?,?, ?,2,40)", (media_id, game_id, name, child_order))
        self.db.execute("INSERT INTO no_intro_dat_rom_field_positions VALUES (?, 'name',0,0,2,41,NULL)",
                        (media_id,))

    def add_strict_rom_fields(self, media_id, hash_base):
        self.db.execute("UPDATE no_intro_dat_rom_claims SET size_text='0' WHERE media_entry_id=?", (media_id,))
        self.db.execute(
            "INSERT INTO no_intro_dat_rom_field_positions "
            "(media_entry_id,field_kind,field_occurrence,source_order,source_line,source_column) "
            "VALUES (?, 'size',0,1,2,42)", (media_id,))
        for index, (field, algorithm, length) in enumerate(
                (("crc", "crc32", 4), ("md5", "md5", 16), ("sha1", "sha1", 20)), 1):
            hash_id, reported_id = hash_base + index, hash_base + 10 + index
            digest = bytes([hash_base % 251]) * length
            self.db.execute("INSERT INTO hash_values VALUES (?,?,?)", (hash_id, algorithm, digest))
            self.db.execute(
                "INSERT INTO catalog_entry_hashes "
                "(reported_hash_id,media_entry_id,source_hash_field,field_occurrence,presence,hash_scope,hash_id) "
                "VALUES (?,?,?,0,'value','unknown',?)",
                (reported_id, media_id, field, hash_id))
            self.db.execute("INSERT INTO no_intro_dat_rom_field_positions VALUES (?, ?,0,?,2,43,?)",
                            (media_id, field, index + 1, reported_id))

    def test_strict_rom_child_extra_is_already_reported_by_integrity_view(self):
        self.add_rom(54, 54019, "extra.rom", 2)
        self.add_strict_rom_fields(54019, 54300)
        self.assertEqual(self.cardinality(54), [])
        self.assertEqual(self.integrity("candidate_no_intro_integrity_problems", 54), [
            ("flat_dat_game_children_missing_or_mismatched", 54010, 54)])

    def test_v3_strict_rejects_v4_header_children_but_v3_compatible_accepts_them(self):
        strict_header = 54000
        extra = 54006
        self.db.execute("INSERT INTO catalog_source_elements VALUES (?,54,'no_intro_dat_header_text_child')", (extra,))
        self.db.execute("INSERT INTO no_intro_dat_header_text_children VALUES (?,?,'trademarks','',5,1,30)",
                        (extra, strict_header))
        self.assertIn(("flat_dat_incompatible_field_presence", 54, 54),
                      self.integrity("candidate_no_intro_integrity_problems", 54))
        compatible_extra = 56006
        self.db.execute("INSERT INTO catalog_source_elements VALUES (?,56,'no_intro_dat_header_text_child')",
                        (compatible_extra,))
        self.db.execute("INSERT INTO no_intro_dat_header_text_children VALUES (?,?,'trademarks','',4,1,30)",
                        (compatible_extra, 56000))
        self.assertEqual(self.integrity("candidate_no_intro_integrity_problems", 56), [])

    def test_synthetic_pc_optional_text_and_export_sibling_envelope_controls(self):
        # Synthetic P/C deliberately permits no header text, version, or game
        # description; explicit empty and absent remain different row states.
        self.db.execute("DELETE FROM no_intro_pc_header_names WHERE source_element_id=201")
        self.db.execute("DELETE FROM catalog_source_elements WHERE source_element_id=201")
        self.db.execute("DELETE FROM no_intro_pc_header_versions WHERE source_element_id=202")
        self.db.execute("DELETE FROM catalog_source_elements WHERE source_element_id=202")
        self.db.execute("DELETE FROM no_intro_pc_header_descriptions WHERE header_id=200")
        self.db.execute("DELETE FROM no_intro_pc_game_descriptions WHERE source_element_id=204")
        self.db.execute("DELETE FROM catalog_source_elements WHERE source_element_id=204")
        self.assertEqual(self.cardinality(51), [])
        self.assertEqual(self.integrity("candidate_no_intro_integrity_problems", 51), [])

        self.db.execute(
            "UPDATE no_intro_export_documents SET envelope_mode='sibling_header_datafile' "
            "WHERE edition_id=52")
        self.assertIn(("export_header_mode", 52, 52),
                      self.integrity("candidate_no_intro_integrity_problems", 52))

    def test_required_game_text_removal_blocks_publication_and_repairs(self):
        self.assertEqual(self.integrity("candidate_integrity_problems", 50), [])
        self.db.execute("DELETE FROM no_intro_dat_game_descriptions WHERE set_id=102")
        self.assertIn(("flat_dat_game_children_missing_or_mismatched", 102, 50),
                      self.integrity("candidate_integrity_problems", 50))
        with self.assertRaisesRegex(sqlite3.IntegrityError, "publication requires complete closure"):
            self.publish(50)
        self.db.execute("INSERT INTO no_intro_dat_game_descriptions VALUES (103,102,'Description',0,2,1)")
        self.assertEqual(self.integrity("candidate_integrity_problems", 50), [])
        self.publish(50)

    def test_new_strict_rom_cardinality_problem_blocks_publication_then_repairs(self):
        edition, rom_id = 54, 54012
        self.db.execute(
            "DELETE FROM no_intro_dat_rom_field_positions WHERE media_entry_id=? AND field_kind='size'",
            (rom_id,))
        self.assertEqual(self.cardinality(edition), [
            ("dat_strict_rom_field_missing:size", rom_id, edition)])
        with self.assertRaisesRegex(sqlite3.IntegrityError, "publication requires complete closure"):
            self.publish(edition)
        self.db.execute(
            "INSERT INTO no_intro_dat_rom_field_positions "
            "(media_entry_id,field_kind,field_occurrence,source_order,source_line,source_column) "
            "VALUES (?, 'size',0,1,2,22)", (rom_id,))
        self.assertEqual(self.cardinality(edition), [])
        self.publish(edition)

    def publish(self, edition):
        self.db.execute(
            "INSERT INTO published_catalog_editions "
            "(edition_id,catalog_id,source_file_id,reading_rules_id,coverage_id,published_at) "
            "SELECT edition_id,catalog_id,source_file_id,reading_rules_id,coverage_id,'cardinality-test' "
            "FROM catalog_editions WHERE edition_id=?", (edition,))

    def test_cardinality_owner_and_edition_come_from_actual_parent_chain(self):
        self.db.execute(
            "DELETE FROM no_intro_dat_rom_field_positions WHERE media_entry_id=54012 AND field_kind='size'")
        self.db.execute(
            "DELETE FROM no_intro_dat_rom_field_positions WHERE media_entry_id=55012 AND field_kind='size'")
        self.assertEqual(self.cardinality(), [
            ("dat_strict_rom_field_missing:size", 54012, 54),
            ("dat_strict_rom_field_missing:size", 55012, 55),
        ])


if __name__ == "__main__":
    unittest.main()
