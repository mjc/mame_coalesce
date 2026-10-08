#!/usr/bin/env python3
"""Composed in-memory checks for Logiqx/CMP child cardinality.

The fixture is constructed candidate SQL, not importer or source-corpus proof.
Run: python3 -Werror::ResourceWarning docs/schema-candidate/logiqx_cmp_cardinality_check.py
"""

from pathlib import Path
import re
import sqlite3
import sys
import unittest

sys.dont_write_bytecode = True
import assemble
import count_fixtures
import logiqx_cmp_presence_check

HERE = Path(__file__).resolve().parent


def composed_fixture():
    """Compose the actual assembler schema with the native witness rows."""
    source = (HERE / "logiqx_cmp_field_witnesses.sql").read_text()
    marker = ".read docs/schema-candidate/logiqx_cmp.sql\n\n"
    if source.count(marker) != 1:
        raise AssertionError("native fixture boundary changed")
    native = source.split(marker, 1)[1].split("CREATE TEMP TABLE field_assert", 1)[0]
    for table in ("catalog_reading_rules", "catalog_editions"):
        native, count = re.subn(r"INSERT INTO " + table + r" VALUES[^;]+;", "", native, count=1)
        if count != 1:
            raise AssertionError("common fixture boundary changed: " + table)
    relationships = []
    kinds = ("logiqx_cloneof", "logiqx_romof", "logiqx_sampleof", "logiqx_rom_merge",
             "logiqx_disk_merge", "logiqx_device_ref", "clrmamepro_cloneof",
             "clrmamepro_sampleof", "clrmamepro_rom_merge")
    for identity, kind in enumerate(kinds, 1):
        edition = 1 if kind.startswith("logiqx_") else 2
        relationships.extend((
            f"INSERT INTO catalog_relationships(relationship_id,assertion_key,origin,edition_id) VALUES({identity},'cardinality-{identity}','source',{edition});",
            f"INSERT INTO reported_catalog_relationships VALUES({identity},'{kind}');",
        ))
    native, count = re.subn(r"INSERT INTO reported_catalog_relationships VALUES[^;]+;",
                            "\n".join(relationships), native, count=1)
    if count != 1:
        raise AssertionError("relationship fixture boundary changed")
    common = """
INSERT INTO catalog_publishers VALUES(1,'cardinality','Cardinality witness',NULL);
INSERT INTO catalogs VALUES(1,1,'cardinality','Cardinality witness');
INSERT INTO catalog_source_files VALUES(1,zeroblob(32),NULL,10000,'cardinality-source','zstd');
INSERT INTO catalog_coverage VALUES(1,'complete');
INSERT INTO catalog_reading_rules VALUES
 (1,'cardinality-logiqx','logiqx','compat','1.5','candidate','logiqx-declared-text-compat-v2'),
 (2,'cardinality-cmp','clrmamepro','compat','observed','candidate','clrmamepro-declared-text-compat-v1');
INSERT INTO catalog_editions VALUES(1,1,1,1,1,NULL,NULL),(2,1,1,2,1,NULL,NULL);
"""
    return common + native


class CardinalityChecks(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.db = sqlite3.connect(":memory:")
        cls.addClassCleanup(cls.db.close)
        cls.db.execute("PRAGMA foreign_keys=ON")
        cls.db.executescript(assemble.assemble())
        cls.db.executescript(composed_fixture())
        logiqx_cmp_presence_check.seal_transplanted_fixture(cls.db)
        view = cls.db.execute(
            "SELECT 1 FROM sqlite_schema WHERE type='view' AND name=?",
            ("candidate_logiqx_cmp_cardinality_problems",),
        ).fetchone()
        if view is None:
            raise AssertionError("assemble() did not install the Logiqx/CMP cardinality view")
        cls.db.commit()

    def setUp(self):
        self.db.execute("SAVEPOINT cardinality_case")

    def tearDown(self):
        self.db.execute("ROLLBACK TO cardinality_case")
        self.db.execute("RELEASE cardinality_case")

    def reports(self):
        return self.db.execute(
            "SELECT problem,owner_id,edition_id "
            "FROM candidate_logiqx_cmp_cardinality_problems ORDER BY problem,owner_id,edition_id"
        ).fetchall()

    def edition_problems(self, edition_id):
        return self.db.execute(
            "SELECT problem,owner_id,edition_id FROM candidate_integrity_problems "
            "WHERE edition_id=? ORDER BY problem,owner_id",
            (edition_id,),
        ).fetchall()

    def adjust_expected_events(self, family, edition_id, events):
        """Apply literal fixture-event deltas without reading audited rows."""
        assignments = ",".join(
            f"{assemble.identifier(counter)}={assemble.identifier(counter)}+?"
            for counter in events
        )
        self.db.execute(
            f"UPDATE {assemble.identifier(family + '_source_count_seals')} "
            f"SET {assignments} WHERE edition_id=?",
            (*events.values(), edition_id),
        )

    def assert_only_publication_problem(self, edition_id, problem, owner_id,
                                        source_file_id=None, reading_rules_id=3,
                                        allow_problem_prefixes=(),
                                        expected_other_problems=()):
        found = self.edition_problems(edition_id)
        if isinstance(owner_id, (set, frozenset)):
            expected_rows = [row for row in found if row[0] == problem and row[2] == edition_id]
            self.assertEqual(len(expected_rows), 1, found)
            self.assertIn(expected_rows[0][1], owner_id)
        else:
            self.assertIn((problem, owner_id, edition_id), found)
        unexpected = [
            row for row in found
            if row[0] != problem
            and row not in expected_other_problems
            and not any(row[0].startswith(prefix) for prefix in allow_problem_prefixes)
        ]
        self.assertEqual(unexpected, [], found)
        self.assertEqual(
            sorted(row for row in found if row in expected_other_problems),
            sorted(expected_other_problems), found,
        )
        if source_file_id is None:
            source_file_id = edition_id
        with self.assertRaisesRegex(sqlite3.IntegrityError, "complete closure"):
            self.db.execute(
                "INSERT INTO published_catalog_editions VALUES(?,?,?,?,1,'expected-rejection')",
                (edition_id, 1, source_file_id, reading_rules_id),
            )

    def disable_sibling_order_guards(self, *tables):
        """Stage legacy-corrupt rows in memory; publication guards stay active."""
        for table in tables:
            rows = self.db.execute(
                "SELECT name FROM sqlite_schema WHERE type='trigger' AND tbl_name=? "
                "AND (name GLOB 'candidate_siblings_*' OR name GLOB 'candidate_position_order_*')",
                (table,),
            ).fetchall()
            for (name,) in rows:
                self.db.execute('DROP TRIGGER "' + name.replace('"', '""') + '"')

    def assert_repaired_publication(self, edition_id, source_file_id=None,
                                    reading_rules_id=3):
        self.assertEqual(self.edition_problems(edition_id), [])
        if source_file_id is None:
            source_file_id = edition_id
        self.db.execute(
            "INSERT INTO published_catalog_editions VALUES(?,?,?,?,1,'repaired')",
            (edition_id, 1, source_file_id, reading_rules_id),
        )

    def add_empty_cmp_edition(self, edition_id=3, source_file_id=2, group_id=None):
        if group_id is None:
            group_id = edition_id
        self.db.execute(
            "INSERT INTO catalog_source_files VALUES(?,?,NULL,?,'cardinality-empty-' || ?,'zstd')",
            (source_file_id, bytes([edition_id]) * 32, 10000 + edition_id, edition_id),
        )
        self.db.execute("INSERT INTO catalog_editions VALUES(?,?,?,2,1,NULL,NULL)", (edition_id, 1, source_file_id))
        self.db.execute("INSERT INTO catalog_set_groups VALUES(?,?, 'root')", (group_id, edition_id))
        self.db.execute(
            "INSERT INTO clrmamepro_documents(edition_id,set_group_id,header_present,comment_count) "
            "VALUES(?,?,0,0)",
            (edition_id, group_id),
        )
        self.db.execute(
            "UPDATE clrmamepro_documents SET location_view='decoded_dat_text', "
            "start_line=1,start_column=1,end_line=1,end_column=1, "
            "column_convention='one_based_unicode_scalar' WHERE edition_id=?",
            (edition_id,),
        )
        count_fixtures.seal(self.db, "clrmamepro", edition_id, {})

    def add_cmp_set(self, source_element_id, source_order, positioned=False, edition_id=3, group_id=None, same_line=False):
        if group_id is None:
            group_id = edition_id
        self.db.execute(
            "INSERT INTO catalog_source_elements VALUES(?,?,'clrmamepro_set')",
            (source_element_id, edition_id),
        )
        line = 1 if same_line else source_order + 1
        column = source_order * 20 + 1 if same_line else 1
        self.db.execute(
            "INSERT INTO catalog_sets VALUES(?,?, '',?,?,?)",
            (source_element_id, group_id, source_order, line, column),
        )
        self.db.execute(
            "INSERT INTO clrmamepro_sets(set_id,source_block) VALUES(?,'game')",
            (source_element_id,),
        )
        if positioned:
            self.db.execute(
                "INSERT INTO clrmamepro_set_field_positions "
                "(set_id,field_kind,source_order,keyword,keyword_line,keyword_column, "
                "value_line,value_column,value_is_quoted) "
                "VALUES(?,0,?,'name',?,?,?, ?,1)",
                (source_element_id, source_order, line, column + 2, line, column + 7),
            )
        events = {"set_count": 1}
        if positioned:
            events["set_field_position_count"] = 1
        self.adjust_expected_events("clrmamepro", edition_id, events)

    def test_existing_complete_fixture_has_no_cardinality_problems(self):
        self.assertEqual(self.reports(), [])

    def test_cmp_root_requires_one_or_more_game_or_set_forms_and_repairs(self):
        self.add_empty_cmp_edition()
        expected = [("cmp_requires_at_least_one_set", 3, 3)]
        self.assertEqual(self.reports(), expected)

        # One empty-name set is a present child; a second is also permitted.
        self.add_cmp_set(301, 0)
        self.assertEqual(self.reports(), [])
        self.add_cmp_set(302, 1)
        self.assertEqual(self.reports(), [])

    def test_edition_attribution_comes_from_the_document_root_group(self):
        self.add_empty_cmp_edition()
        self.assertEqual(self.reports(), [("cmp_requires_at_least_one_set", 3, 3)])
        self.assertEqual(
            self.db.execute(
                "SELECT edition_id FROM catalog_set_groups WHERE set_group_id=3"
            ).fetchone(),
            (3,),
        )

    def test_logiqx_pcdata_requirements_follow_strict_and_compat_modes(self):
        self.db.execute(
            "INSERT INTO catalog_reading_rules VALUES "
            "(3,'cardinality-logiqx-strict','logiqx','strict','1.5','candidate','logiqx-dtd-1.5-v1')"
        )
        self.db.execute("INSERT INTO catalog_source_files VALUES(3,zeroblob(32),NULL,10002,'cardinality-strict','zstd')")
        self.db.execute("INSERT INTO catalog_editions VALUES(3,1,3,3,1,NULL,NULL)")
        self.db.execute("INSERT INTO catalog_set_groups VALUES(4,3,'root')")
        self.db.execute(
            "INSERT INTO logiqx_documents(edition_id,set_group_id,debug,debug_was_present) "
            "VALUES(3,4,'no',0)"
        )
        self.assertIn(
            ("strict_logiqx_game_count", 3, 3),
            self.db.execute(
                "SELECT problem,owner_id,edition_id FROM candidate_logiqx_cmp_integrity_problems"
            ).fetchall(),
        )

        # A compatible document may have no games; its absent description is
        # also optional. The strict edition below exercises the required case.
        self.db.execute("INSERT INTO catalog_source_files VALUES(4,zeroblob(32),NULL,10003,'cardinality-compatible-empty','zstd')")
        self.db.execute("INSERT INTO catalog_editions VALUES(4,1,4,1,1,NULL,NULL)")
        self.db.execute("INSERT INTO catalog_set_groups VALUES(5,4,'root')")
        self.db.execute(
            "INSERT INTO logiqx_documents(edition_id,set_group_id,debug,debug_was_present) "
            "VALUES(4,5,'no',0)"
        )
        self.db.execute("INSERT INTO catalog_source_elements VALUES(408,4,'logiqx_game')")
        self.db.execute("INSERT INTO catalog_sets VALUES(408,5,'',0,1,1)")
        self.db.execute("INSERT INTO logiqx_games VALUES(408,NULL,'no',0,NULL,NULL)")
        self.assertNotIn("strict_logiqx_game_count", {
            row[0] for row in self.db.execute(
                "SELECT problem FROM candidate_logiqx_cmp_integrity_problems WHERE edition_id=4"
            )
        })
        self.assertNotIn(("missing_strict_game_description", 408, 4), set(
            self.db.execute(
                "SELECT problem,owner_id,edition_id FROM candidate_logiqx_cmp_integrity_problems"
            )
        ))

        self.db.execute("INSERT INTO catalog_source_elements VALUES(401,3,'logiqx_game')")
        self.db.execute("INSERT INTO catalog_sets VALUES(401,4,'',1,1,1)")
        self.db.execute("INSERT INTO logiqx_games VALUES(401,NULL,'no',0,NULL,NULL)")
        problems = set(self.db.execute(
            "SELECT problem,owner_id,edition_id FROM candidate_logiqx_cmp_integrity_problems"
        ))
        self.assertIn(("missing_strict_game_description", 401, 3), problems)

        self.db.execute("INSERT INTO catalog_source_elements VALUES(402,3,'logiqx_game_text')")
        self.db.execute(
            "INSERT INTO logiqx_game_text_elements "
            "VALUES(402,401,0,'',0,1,1)"
        )
        problems = set(self.db.execute(
            "SELECT problem,owner_id,edition_id FROM candidate_logiqx_cmp_integrity_problems"
        ))
        self.assertNotIn(("missing_strict_game_description", 401, 3), problems)
        # Empty PCDATA is present. The same child is absent by row deletion.
        self.assertEqual(self.db.execute(
            "SELECT text_value FROM logiqx_game_text_elements WHERE source_element_id=402"
        ).fetchone(), ("",))

        self.db.execute("INSERT INTO catalog_source_elements VALUES(403,3,'logiqx_header')")
        self.db.execute("INSERT INTO logiqx_headers VALUES(403,4,0,1,1)")
        problems = set(self.db.execute(
            "SELECT problem,owner_id,edition_id FROM candidate_logiqx_cmp_integrity_problems"
        ))
        self.assertIn(("missing_strict_header_text", 403, 3), problems)
        for source_id, field_kind in zip((404, 405, 406, 407), (0, 1, 3, 5)):
            self.db.execute(
                "INSERT INTO catalog_source_elements VALUES(?,3,'logiqx_header_text')",
                (source_id,),
            )
            self.db.execute(
                "INSERT INTO logiqx_header_text_elements VALUES(?,?,?,?,?,?,?)",
                (source_id, 403, field_kind, "", field_kind, 1, field_kind + 1),
            )
        problems = set(self.db.execute(
            "SELECT problem,owner_id,edition_id FROM candidate_logiqx_cmp_integrity_problems"
        ))
        self.assertNotIn(("missing_strict_header_text", 403, 3), problems)

    def test_singleton_extra_children_are_rejected_by_typed_keys(self):
        # Repeated Logiqx header text fields and repeated CMP scalar fields are
        # schema-level max-one rules; repeatable comments/samples remain rows.
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute(
                "INSERT INTO logiqx_header_text_elements "
                "(source_element_id,header_id,field_kind,text_value,source_order,source_line,source_column) "
                "SELECT 999,header_id,field_kind,text_value,99,1,1 "
                "FROM logiqx_header_text_elements LIMIT 1"
            )

    def test_present_pcdata_cannot_be_malformed_as_a_null_value(self):
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute(
                "UPDATE logiqx_game_text_elements SET text_value=NULL "
                "WHERE source_element_id=140"
            )

    def test_publication_gate_uses_cardinality_report_when_parent_integrates_it(self):
        audits = [row[0] for row in self.db.execute(
            "SELECT sql FROM sqlite_schema WHERE type='view' AND name GLOB 'candidate_integrity_chunk_*'"
        )]
        self.assertTrue(any("candidate_logiqx_cmp_cardinality_problems" in sql for sql in audits))
        self.add_empty_cmp_edition()
        with self.assertRaisesRegex(sqlite3.IntegrityError, "complete closure"):
            self.db.execute("INSERT INTO published_catalog_editions VALUES(3,1,2,2,1,'cardinality')")
        self.add_cmp_set(301, 0, positioned=True)
        self.db.execute("INSERT INTO published_catalog_editions VALUES(3,1,2,2,1,'cardinality')")

    def add_strict_logiqx_edition(self, edition_id, defect, rules_id=3):
        source_file_id = edition_id
        group_id = edition_id + 1
        header_id = 600 + edition_id
        game_id = 700 + edition_id
        self.db.execute(
            "INSERT INTO catalog_source_files VALUES(?,?,NULL,?,'strict-sequence-' || ?,'zstd')",
            (source_file_id, bytes([edition_id]) * 32, 10000 + edition_id, edition_id),
        )
        self.db.execute(
            "INSERT INTO catalog_editions VALUES(?,?,?,?,1,NULL,NULL)",
            (edition_id, 1, source_file_id, rules_id),
        )
        self.db.execute("INSERT INTO catalog_set_groups VALUES(?,?, 'root')", (group_id, edition_id))
        self.db.execute(
            "INSERT INTO logiqx_documents "
            "(edition_id,set_group_id,debug,debug_was_present,location_view,start_line,start_column, "
            "end_line,end_column,column_convention) "
            "VALUES(?,?,'no',0,'transport_decoded_xml_text',1,1,10,1,'one_based_unicode_scalar')",
            (edition_id, group_id),
        )
        header_order = 2 if defect in ("root", "compatible") else 0
        self.db.execute("INSERT INTO catalog_source_elements VALUES(?,?,'logiqx_header')", (header_id, edition_id))
        self.db.execute(
            "INSERT INTO logiqx_headers VALUES(?,?,?,?,?)",
            (header_id, group_id, header_order, 1, 1),
        )
        self.db.execute("INSERT INTO catalog_source_elements VALUES(?,?,'logiqx_game')", (game_id, edition_id))
        self.db.execute(
            "INSERT INTO catalog_sets VALUES(?,?, '',1,2,1)",
            (game_id, group_id),
        )
        self.db.execute("INSERT INTO logiqx_games VALUES(?,NULL,'no',0,NULL,NULL)", (game_id,))
        self.db.execute(
            "INSERT INTO logiqx_game_attribute_positions "
            "(set_id,field_kind,source_order,source_line,source_column) VALUES(?,0,0,1,1)",
            (game_id,),
        )

        if defect == "header":
            header_ordering = ((5, 0), (0, 1), (1, 2), (3, 3))
        elif defect == "compatible":
            header_ordering = ((5, 0), (0, 1), (1, 2), (3, 3))
        else:
            header_ordering = ((0, 0), (1, 1), (3, 2), (5, 3))
        for offset, (field_kind, source_order) in enumerate(header_ordering):
            source_id = 1000 + edition_id * 10 + offset
            self.db.execute(
                "INSERT INTO catalog_source_elements VALUES(?,?,'logiqx_header_text')",
                (source_id, edition_id),
            )
            self.db.execute(
                "INSERT INTO logiqx_header_text_elements VALUES(?,?,?,?,?,?,?)",
                (source_id, header_id, field_kind, "", source_order, 2, offset + 1),
            )

        game_ordering = ((1, 0), (0, 1)) if defect in ("game", "compatible") else ((0, 0),)
        for offset, (field_kind, source_order) in enumerate(game_ordering):
            source_id = 2000 + edition_id * 10 + offset
            self.db.execute(
                "INSERT INTO catalog_source_elements VALUES(?,?,'logiqx_game_text')",
                (source_id, edition_id),
            )
            self.db.execute(
                "INSERT INTO logiqx_game_text_elements VALUES(?,?,?,?,?,?,?)",
                (source_id, game_id, field_kind, "", source_order, 3, offset + 1),
            )
        count_fixtures.seal(self.db, "logiqx", edition_id, {
            "header_count": 1,
            "game_count": 1,
            "header_text_count": 4,
            "game_text_count": 2 if defect in ("game", "compatible") else 1,
            "game_attribute_position_count": 1,
        })
        return edition_id, header_id, game_id, header_ordering, game_ordering

    def test_strict_logiqx_sequence_defects_reject_publication_and_repair(self):
        self.db.execute(
            "INSERT INTO catalog_reading_rules VALUES "
            "(3,'cardinality-logiqx-strict','logiqx','strict','1.5','candidate','logiqx-dtd-1.5-v1')"
        )
        defects = (
            (3, "root", "strict_logiqx_root_child_order"),
            (4, "header", "strict_logiqx_header_child_order"),
            (5, "game", "strict_logiqx_game_child_order"),
        )
        for edition_id, defect, expected_problem in defects:
            with self.subTest(defect=defect):
                edition, header_id, game_id, header_rows, game_rows = self.add_strict_logiqx_edition(edition_id, defect)
                actual = self.db.execute(
                    "SELECT problem,edition_id FROM candidate_integrity_problems WHERE edition_id=?",
                    (edition,),
                ).fetchall()
                self.assertIn((expected_problem, edition), actual)
                with self.assertRaisesRegex(sqlite3.IntegrityError, "complete closure"):
                    self.db.execute(
                        "INSERT INTO published_catalog_editions VALUES(?,?,?,3,1,'strict-invalid')",
                        (edition, 1, edition),
                    )

                if defect == "root":
                    self.db.execute(
                        "UPDATE logiqx_headers SET source_order=0 WHERE header_id=?",
                        (header_id,),
                    )
                elif defect == "header":
                    self.db.execute(
                        "UPDATE logiqx_header_text_elements SET source_order=source_order+10 WHERE header_id=?",
                        (header_id,),
                    )
                    repaired = {0: 0, 1: 1, 3: 2, 5: 3}
                    for field_kind, source_order in repaired.items():
                        self.db.execute(
                            "UPDATE logiqx_header_text_elements SET source_order=? "
                            "WHERE header_id=? AND field_kind=?",
                            (source_order, header_id, field_kind),
                        )
                else:
                    self.db.execute(
                        "UPDATE logiqx_game_text_elements SET source_order=source_order+10 WHERE set_id=?",
                        (game_id,),
                    )
                    for field_kind, source_order in ((0, 0), (1, 1)):
                        self.db.execute(
                            "UPDATE logiqx_game_text_elements SET source_order=? "
                            "WHERE set_id=? AND field_kind=?",
                            (source_order, game_id, field_kind),
                        )
                self.assertNotIn(expected_problem, {
                    row[0] for row in self.db.execute(
                        "SELECT problem FROM candidate_integrity_problems WHERE edition_id=?",
                        (edition,),
                    )
                })
                remaining = self.db.execute(
                    "SELECT problem,owner_id FROM candidate_integrity_problems WHERE edition_id=?",
                    (edition,),
                ).fetchall()
                self.assertEqual(remaining, [], (defect, remaining))
                self.db.execute(
                    "INSERT INTO published_catalog_editions VALUES(?,?,?,3,1,'strict-repaired')",
                    (edition, 1, edition),
                )

    def test_missing_strict_children_keep_native_edition_after_registry_erasure(self):
        self.db.execute(
            "INSERT INTO catalog_reading_rules VALUES "
            "(3,'cardinality-logiqx-strict','logiqx','strict','1.5','candidate','logiqx-dtd-1.5-v1')"
        )
        edition, _, game_id, _, _ = self.add_strict_logiqx_edition(7, "clean")
        description_id = 2000 + edition * 10
        self.db.execute(
            "DELETE FROM logiqx_game_text_elements WHERE source_element_id=?",
            (description_id,),
        )
        self.db.execute(
            "DELETE FROM catalog_source_elements WHERE source_element_id=?",
            (description_id,),
        )
        self.assert_only_publication_problem(
            edition, "missing_strict_game_description", game_id,
            expected_other_problems=(("source_count:logiqx:game_text_count", edition, edition),),
        )
        self.db.execute(
            "INSERT INTO catalog_source_elements VALUES(?,?,'logiqx_game_text')",
            (description_id, edition),
        )
        self.db.execute(
            "INSERT INTO logiqx_game_text_elements VALUES(?,?,0,'',0,3,1)",
            (description_id, game_id),
        )
        self.assert_repaired_publication(edition)

        edition, _, game_id, _, _ = self.add_strict_logiqx_edition(8, "clean")
        media_id = 8008
        self.db.execute(
            "INSERT INTO catalog_source_elements VALUES(?,?,'logiqx_rom')",
            (media_id, edition),
        )
        self.db.execute("INSERT INTO catalog_media_entries VALUES(?,NULL)", (media_id,))
        self.db.execute(
            "INSERT INTO logiqx_roms(media_entry_id,set_id,name,size_text,status,status_specified,date,source_order,source_line,source_column) "
            "VALUES(?,?, 'rom',NULL,'good',0,NULL,1,4,1)",
            (media_id, game_id),
        )
        self.db.execute(
            "INSERT INTO logiqx_rom_attribute_positions "
            "(media_entry_id,field_kind,source_order,source_line,source_column) "
            "VALUES(?,0,0,4,10)",
            (media_id,),
        )
        self.adjust_expected_events("logiqx", edition, {
            "rom_count": 1,
            "rom_attribute_position_count": 2,
        })
        self.assert_only_publication_problem(
            edition, "missing_strict_rom_size", media_id,
            expected_other_problems=(("source_count:logiqx:rom_attribute_position_count", edition, edition),),
        )
        self.db.execute("UPDATE logiqx_roms SET size_text='0' WHERE media_entry_id=?", (media_id,))
        self.db.execute(
            "INSERT INTO logiqx_rom_attribute_positions "
            "(media_entry_id,field_kind,source_order,source_line,source_column) "
            "VALUES(?,1,1,4,15)",
            (media_id,),
        )
        self.assert_repaired_publication(edition)

    def test_position_problems_use_exact_native_owner_scope_for_publication(self):
        self.db.execute(
            "INSERT INTO catalog_reading_rules VALUES "
            "(3,'cardinality-logiqx-strict','logiqx','strict','1.5','candidate','logiqx-dtd-1.5-v1')"
        )
        edition, _, game_id, _, _ = self.add_strict_logiqx_edition(31, "clean")
        self.db.execute("UPDATE logiqx_games SET sourcefile='' WHERE set_id=?", (game_id,))
        self.assert_only_publication_problem(
            edition, "missing_position", game_id,
            allow_problem_prefixes=("field_presence:logiqx_game_attribute_positions:1",),
        )
        self.db.execute(
            "INSERT INTO logiqx_game_attribute_positions "
            "(set_id,field_kind,source_order,source_line,source_column) VALUES(?,1,1,1,2)",
            (game_id,),
        )
        self.adjust_expected_events("logiqx", edition, {"game_attribute_position_count": 1})
        self.assert_repaired_publication(edition)

        edition, _, game_id, _, _ = self.add_strict_logiqx_edition(32, "clean")
        self.db.execute(
            "INSERT INTO logiqx_game_attribute_positions "
            "(set_id,field_kind,source_order,source_line,source_column) VALUES(?,1,1,1,2)",
            (game_id,),
        )
        self.adjust_expected_events("logiqx", edition, {"game_attribute_position_count": 1})
        self.assert_only_publication_problem(
            edition, "unexpected_position", game_id,
            allow_problem_prefixes=("field_presence:logiqx_game_attribute_positions:1",),
        )
        self.db.execute(
            "DELETE FROM logiqx_game_attribute_positions WHERE set_id=? AND field_kind=1",
            (game_id,),
        )
        self.adjust_expected_events("logiqx", edition, {"game_attribute_position_count": -1})
        self.assert_repaired_publication(edition)

    def test_missing_cmp_rom_detail_facet_blocks_and_repairs_publication(self):
        edition = 11
        source_file_id = 12
        self.add_empty_cmp_edition(edition, source_file_id)
        set_id, media_id = 11000, 11001
        self.add_cmp_set(set_id, 0, positioned=True, edition_id=edition)
        self.db.execute(
            "INSERT INTO catalog_source_elements VALUES(?,?,'clrmamepro_rom')",
            (media_id, edition),
        )
        self.db.execute("INSERT INTO catalog_media_entries VALUES(?,NULL)", (media_id,))
        self.db.execute(
            "INSERT INTO clrmamepro_roms "
            "(media_entry_id,set_id,name,size_text,source_order,source_line,source_column) "
            "VALUES(?,?, 'rom',NULL,1,2,1)",
            (media_id, set_id),
        )
        self.db.execute(
            "INSERT INTO clrmamepro_rom_field_positions "
            "(media_entry_id,field_kind,source_order,keyword,keyword_line,keyword_column,"
            "value_line,value_column,value_is_quoted) VALUES(?,0,0,'name',2,3,2,8,1)",
            (media_id,),
        )
        self.adjust_expected_events("clrmamepro", edition, {
            "rom_count": 1,
            "rom_field_position_count": 1,
        })
        self.db.execute("DELETE FROM clrmamepro_rom_details WHERE media_entry_id=?", (media_id,))
        self.assert_only_publication_problem(
            edition, "missing_cmp_rom_details", media_id,
            source_file_id=source_file_id, reading_rules_id=2,
        )
        self.db.execute(
            "INSERT INTO clrmamepro_rom_details(media_entry_id) VALUES(?)", (media_id,)
        )
        self.assert_repaired_publication(
            edition, source_file_id=source_file_id, reading_rules_id=2
        )

    def test_all_logiqx_mixed_order_collisions_are_edition_scoped(self):
        # Root/header/game collision scopes use typed parents, not rowids
        # guessed from catalog_source_elements.
        edition, header_id, game_id, _, _ = self.add_strict_logiqx_edition(
            12, "clean", rules_id=1
        )
        self.disable_sibling_order_guards("catalog_sets")
        self.db.execute("UPDATE catalog_sets SET source_order=0 WHERE set_id=?", (game_id,))
        self.assert_only_publication_problem(
            edition, "logiqx_root_mixed_order_collision",
            {header_id, game_id}, reading_rules_id=1,
            allow_problem_prefixes=("mixed_order:",),
        )
        self.db.execute("UPDATE catalog_sets SET source_order=1 WHERE set_id=?", (game_id,))
        self.assert_repaired_publication(edition, reading_rules_id=1)

        edition, header_id, _, _, _ = self.add_strict_logiqx_edition(13, "clean", rules_id=1)
        option_id = 13010
        self.disable_sibling_order_guards("logiqx_clrmamepro_options")
        self.db.execute(
            "INSERT INTO catalog_source_elements VALUES(?,?,'logiqx_clrmamepro_options')",
            (option_id, edition),
        )
        self.db.execute(
            "INSERT INTO logiqx_clrmamepro_options "
            "(source_element_id,header_id,source_order,source_line,source_column,"
            "forcemerging_was_present,forcenodump_was_present,forcepacking_was_present) "
            "VALUES(?,?,0,2,10,0,0,0)",
            (option_id, header_id),
        )
        self.adjust_expected_events("logiqx", edition, {"clrmamepro_options_count": 1})
        header_text_ids = {2000 + edition * 10 + offset for offset in range(4)}
        self.assert_only_publication_problem(
            edition, "logiqx_header_mixed_order_collision",
            header_text_ids | {option_id}, reading_rules_id=1,
            allow_problem_prefixes=("mixed_order:",),
        )
        self.db.execute(
            "DELETE FROM logiqx_clrmamepro_options WHERE source_element_id=?", (option_id,)
        )
        self.db.execute("DELETE FROM catalog_source_elements WHERE source_element_id=?", (option_id,))
        self.adjust_expected_events("logiqx", edition, {"clrmamepro_options_count": -1})
        self.assert_repaired_publication(edition, reading_rules_id=1)

        edition, _, game_id, _, _ = self.add_strict_logiqx_edition(14, "clean", rules_id=1)
        comment_id = 14010
        self.disable_sibling_order_guards("logiqx_game_comments")
        self.db.execute(
            "INSERT INTO catalog_source_elements VALUES(?,?,'logiqx_game_comment')",
            (comment_id, edition),
        )
        self.db.execute(
            "INSERT INTO logiqx_game_comments VALUES(?,?, 'comment',0,3,10)",
            (comment_id, game_id),
        )
        self.adjust_expected_events("logiqx", edition, {"game_comment_count": 1})
        description_id = 2000 + edition * 10
        self.assert_only_publication_problem(
            edition, "logiqx_game_mixed_order_collision",
            {description_id, comment_id}, reading_rules_id=1,
            allow_problem_prefixes=("mixed_order:",),
        )
        self.db.execute("DELETE FROM logiqx_game_comments WHERE source_element_id=?", (comment_id,))
        self.db.execute("DELETE FROM catalog_source_elements WHERE source_element_id=?", (comment_id,))
        self.adjust_expected_events("logiqx", edition, {"game_comment_count": -1})
        self.assert_repaired_publication(edition, reading_rules_id=1)

    def test_cmp_document_and_set_item_collisions_are_edition_scoped(self):
        edition, source_file_id = 15, 16
        self.add_empty_cmp_edition(edition, source_file_id)
        self.disable_sibling_order_guards("clrmamepro_headers", "catalog_sets")
        header_id, set_id = 15000, 15001
        self.db.execute(
            "INSERT INTO catalog_source_elements VALUES(?,?,'clrmamepro_header')",
            (header_id, edition),
        )
        self.db.execute(
            "INSERT INTO clrmamepro_headers "
            "(header_id,edition_id,source_block,source_order,source_line,source_column) "
            "VALUES(?,?,'clrmamepro',0,1,1)",
            (header_id, edition),
        )
        self.db.execute("UPDATE clrmamepro_documents SET header_present=1 WHERE edition_id=?", (edition,))
        self.add_cmp_set(set_id, 0, positioned=True, edition_id=edition)
        self.db.execute("UPDATE catalog_sets SET source_column=2 WHERE set_id=?", (set_id,))
        self.assert_only_publication_problem(
            edition, "cmp_document_mixed_order_collision", {header_id, set_id},
            source_file_id=source_file_id, reading_rules_id=2,
            allow_problem_prefixes=("mixed_order:",),
        )
        self.db.execute("DELETE FROM clrmamepro_header_options WHERE header_id=?", (header_id,))
        self.db.execute("DELETE FROM clrmamepro_headers WHERE header_id=?", (header_id,))
        self.db.execute("DELETE FROM catalog_source_elements WHERE source_element_id=?", (header_id,))
        self.db.execute("UPDATE clrmamepro_documents SET header_present=0 WHERE edition_id=?", (edition,))
        self.assert_repaired_publication(
            edition, source_file_id=source_file_id, reading_rules_id=2
        )

        edition, source_file_id = 17, 18
        self.add_empty_cmp_edition(edition, source_file_id)
        set_id, media_id = 17000, 17001
        self.add_cmp_set(set_id, 0, positioned=True, edition_id=edition)
        self.disable_sibling_order_guards("clrmamepro_roms")
        self.db.execute(
            "INSERT INTO catalog_source_elements VALUES(?,?,'clrmamepro_rom')", (media_id, edition)
        )
        self.db.execute("INSERT INTO catalog_media_entries VALUES(?,NULL)", (media_id,))
        self.db.execute(
            "INSERT INTO clrmamepro_roms "
            "(media_entry_id,set_id,name,size_text,source_order,source_line,source_column) "
            "VALUES(?,?, 'rom',NULL,0,2,1)",
            (media_id, set_id),
        )
        self.db.execute(
            "INSERT INTO clrmamepro_rom_field_positions "
            "(media_entry_id,field_kind,source_order,keyword,keyword_line,keyword_column,"
            "value_line,value_column,value_is_quoted) VALUES(?,0,0,'name',2,3,2,8,1)",
            (media_id,),
        )
        self.adjust_expected_events("clrmamepro", edition, {
            "rom_count": 1,
            "rom_field_position_count": 1,
        })
        self.assert_only_publication_problem(
            edition, "cmp_set_item_mixed_order_collision", {0, media_id, set_id},
            source_file_id=source_file_id, reading_rules_id=2,
            allow_problem_prefixes=("mixed_order:", "position_order:"),
        )
        self.db.execute("UPDATE clrmamepro_roms SET source_order=1 WHERE media_entry_id=?", (media_id,))
        self.assert_repaired_publication(
            edition, source_file_id=source_file_id, reading_rules_id=2
        )

    def test_cmp_lexical_order_branches_block_and_repair_publication(self):
        edition, source_file_id = 18, 19
        self.add_empty_cmp_edition(edition, source_file_id)
        set_id, media_id = 18000, 18001
        self.add_cmp_set(set_id, 0, positioned=True, edition_id=edition)
        self.disable_sibling_order_guards("clrmamepro_roms")
        self.db.execute(
            "UPDATE clrmamepro_set_field_positions SET keyword_line=1,keyword_column=20,"
            "value_line=1,value_column=25 WHERE set_id=? AND field_kind=0",
            (set_id,),
        )
        self.db.execute(
            "INSERT INTO catalog_source_elements VALUES(?,?,'clrmamepro_rom')", (media_id, edition)
        )
        self.db.execute("INSERT INTO catalog_media_entries VALUES(?,NULL)", (media_id,))
        self.db.execute(
            "INSERT INTO clrmamepro_roms "
            "(media_entry_id,set_id,name,size_text,source_order,source_line,source_column) "
            "VALUES(?,?, 'rom',NULL,1,1,10)",
            (media_id, set_id),
        )
        self.db.execute(
            "INSERT INTO clrmamepro_rom_field_positions "
            "(media_entry_id,field_kind,source_order,keyword,keyword_line,keyword_column,"
            "value_line,value_column,value_is_quoted) VALUES(?,0,0,'name',1,11,1,15,1)",
            (media_id,),
        )
        self.adjust_expected_events("clrmamepro", edition, {
            "rom_count": 1,
            "rom_field_position_count": 1,
        })
        self.assert_only_publication_problem(
            edition, "cmp_set_item_order_inversion", set_id,
            source_file_id=source_file_id, reading_rules_id=2,
        )
        self.db.execute(
            "UPDATE clrmamepro_set_field_positions SET keyword_column=5,value_column=7 "
            "WHERE set_id=? AND field_kind=0",
            (set_id,),
        )
        self.assert_repaired_publication(
            edition, source_file_id=source_file_id, reading_rules_id=2
        )

        edition, source_file_id = 19, 20
        self.add_empty_cmp_edition(edition, source_file_id)
        self.add_cmp_set(19001, 1, positioned=True, edition_id=edition)
        header_id = 19000
        self.db.execute(
            "INSERT INTO catalog_source_elements VALUES(?,?,'clrmamepro_header')",
            (header_id, edition),
        )
        self.db.execute(
            "INSERT INTO clrmamepro_headers "
            "(header_id,edition_id,source_block,source_order,source_line,source_column,name,description) "
            "VALUES(?,?,'clrmamepro',0,1,1,'n','d')",
            (header_id, edition),
        )
        self.db.execute("UPDATE clrmamepro_documents SET header_present=1,end_line=4,end_column=50 WHERE edition_id=?", (edition,))
        self.db.executemany(
            "INSERT INTO clrmamepro_header_field_positions "
            "(header_id,field_kind,source_order,keyword,keyword_line,keyword_column,"
            "value_line,value_column,value_is_quoted) VALUES(?,?,?,?,2,?,2,?,1)",
            ((header_id, 0, 0, "name", 20, 25), (header_id, 1, 1, "description", 10, 15)),
        )
        self.adjust_expected_events("clrmamepro", edition, {"header_field_position_count": 2})
        self.assert_only_publication_problem(
            edition, "cmp_nested_field_order_inversion", header_id,
            source_file_id=source_file_id, reading_rules_id=2,
        )
        self.db.execute(
            "UPDATE clrmamepro_header_field_positions SET keyword_column=10,value_column=15 "
            "WHERE header_id=? AND field_kind=0",
            (header_id,),
        )
        self.db.execute(
            "UPDATE clrmamepro_header_field_positions SET keyword_column=20,value_column=25 "
            "WHERE header_id=? AND field_kind=1",
            (header_id,),
        )
        self.assert_repaired_publication(
            edition, source_file_id=source_file_id, reading_rules_id=2
        )

        edition, source_file_id = 40, 41
        self.add_empty_cmp_edition(edition, source_file_id)
        header_id, set_id = 40000, 40001
        self.db.execute(
            "INSERT INTO catalog_source_elements VALUES(?,?,'clrmamepro_header')",
            (header_id, edition),
        )
        self.db.execute(
            "INSERT INTO clrmamepro_headers "
            "(header_id,edition_id,source_block,source_order,source_line,source_column) "
            "VALUES(?,?,'clrmamepro',0,2,1)",
            (header_id, edition),
        )
        self.db.execute("UPDATE clrmamepro_documents SET header_present=1,end_line=4,end_column=50 WHERE edition_id=?", (edition,))
        self.add_cmp_set(set_id, 1, positioned=True, edition_id=edition)
        self.db.execute("UPDATE catalog_sets SET source_line=1,source_column=1 WHERE set_id=?", (set_id,))
        self.assert_only_publication_problem(
            edition, "cmp_document_form_order_inversion", set_id,
            source_file_id=source_file_id, reading_rules_id=2,
        )
        self.db.execute(
            "UPDATE catalog_sets SET source_line=3,source_column=1 WHERE set_id=?", (set_id,)
        )
        self.assert_repaired_publication(
            edition, source_file_id=source_file_id, reading_rules_id=2
        )

    def test_cmp_hash_state_uses_native_rom_edition_scope(self):
        edition, source_file_id = 21, 22
        self.add_empty_cmp_edition(edition, source_file_id)
        set_id, media_id, hash_id = 21000, 21001, 21002
        self.add_cmp_set(set_id, 0, positioned=True, edition_id=edition)
        self.db.execute(
            "INSERT INTO catalog_source_elements VALUES(?,?,'clrmamepro_rom')", (media_id, edition)
        )
        self.db.execute("INSERT INTO catalog_media_entries VALUES(?,NULL)", (media_id,))
        self.db.execute(
            "INSERT INTO clrmamepro_roms "
            "(media_entry_id,set_id,name,size_text,source_order,source_line,source_column) "
            "VALUES(?,?, 'rom',NULL,1,2,1)",
            (media_id, set_id),
        )
        self.db.execute(
            "INSERT INTO catalog_entry_hashes VALUES(?,?, 'crc',0,'invalid','unknown',NULL,'bad-crc')",
            (hash_id, media_id),
        )
        self.db.execute(
            "INSERT INTO clrmamepro_rom_field_positions "
            "(media_entry_id,field_kind,source_order,keyword,keyword_line,keyword_column,"
            "value_line,value_column,value_is_quoted) "
            "VALUES(?,0,0,'name',2,3,2,8,1)",
            (media_id,),
        )
        self.db.execute(
            "INSERT INTO invalid_catalog_entry_hashes VALUES(?,'constructed_invalid_hex')",
            (hash_id,),
        )
        self.db.execute(
            "INSERT INTO clrmamepro_rom_field_positions "
            "(media_entry_id,field_kind,source_order,keyword,keyword_line,keyword_column,"
            "value_line,value_column,value_is_quoted,reported_hash_id) "
            "VALUES(?,2,1,'crc',2,10,2,15,1,?)",
            (media_id, hash_id),
        )
        self.adjust_expected_events("clrmamepro", edition, {
            "rom_count": 1,
            "rom_field_position_count": 2,
        })
        self.assert_only_publication_problem(
            edition, "cmp_invalid_hash_state", hash_id,
            source_file_id=source_file_id, reading_rules_id=2,
        )
        self.db.execute(
            "UPDATE catalog_entry_hashes SET presence='value',hash_id=1,reported_text=NULL "
            "WHERE reported_hash_id=?",
            (hash_id,),
        )
        self.db.execute("DELETE FROM invalid_catalog_entry_hashes WHERE reported_hash_id=?", (hash_id,))
        self.assert_repaired_publication(
            edition, source_file_id=source_file_id, reading_rules_id=2
        )

    def test_unknown_orphan_position_stays_unscoped_and_does_not_poison_known_publication(self):
        db = sqlite3.connect(":memory:")
        try:
            db.execute("PRAGMA foreign_keys=ON")
            db.executescript(assemble.assemble())
            db.executescript(composed_fixture())
            logiqx_cmp_presence_check.seal_transplanted_fixture(db)
            db.commit()
            db.execute("PRAGMA foreign_keys=OFF")
            trigger_rows = db.execute(
                "SELECT name FROM sqlite_schema WHERE type='trigger' "
                "AND tbl_name='clrmamepro_rom_field_positions' "
                "AND (name GLOB 'candidate_native_*' OR name GLOB 'candidate_fk_*' "
                "OR name GLOB 'candidate_position_order_*' OR name GLOB 'candidate_siblings_*')"
            ).fetchall()
            for (name,) in trigger_rows:
                db.execute('DROP TRIGGER "' + name.replace('"', '""') + '"')
            db.execute(
                "INSERT INTO clrmamepro_rom_field_positions "
                "(media_entry_id,field_kind,source_order,keyword,keyword_line,keyword_column,"
                "value_line,value_column,value_is_quoted) "
                "VALUES(999999,0,0,'name',1,1,1,2,1)"
            )
            self.assertEqual(
                db.execute(
                    "SELECT problem,owner_id,edition_id FROM candidate_integrity_problems "
                    "WHERE problem='unexpected_position' AND owner_id=999999"
                ).fetchall(),
                [("unexpected_position", 999999, None)],
            )
            db.execute(
                "INSERT INTO published_catalog_editions "
                "VALUES(2,1,1,2,1,'unknown-orphan-scope-control')"
            )
        finally:
            db.close()

    def test_compat_logiqx_allows_strict_only_sequence_variations(self):
        self.db.execute(
            "INSERT INTO catalog_reading_rules VALUES "
            "(6,'cardinality-logiqx-compat-order','logiqx','compat','1.5','candidate',"
            "'logiqx-declared-text-compat-v2')"
        )
        edition, _, _, _, _ = self.add_strict_logiqx_edition(
            6, "compatible", rules_id=6
        )
        issues = self.db.execute(
            "SELECT problem FROM candidate_integrity_problems WHERE edition_id=?",
            (edition,),
        ).fetchall()
        forbidden = {
            "strict_logiqx_root_child_order",
            "strict_logiqx_header_child_order",
            "strict_logiqx_game_child_order",
        }
        self.assertTrue(forbidden.isdisjoint({row[0] for row in issues}), issues)
        self.assertEqual(issues, [])
        self.db.execute(
            "INSERT INTO published_catalog_editions VALUES(6,1,6,6,1,'compat-out-of-order')"
        )

    def test_cmp_publication_audit_vm_steps_scale_with_populated_set_forms(self):
        measurements = {}
        for edition_id, same_line in ((3, False), (4, True)):
            case_db = sqlite3.connect(":memory:")
            case_db.execute("PRAGMA foreign_keys=ON")
            case_db.executescript(assemble.assemble())
            case_db.executescript(composed_fixture())
            logiqx_cmp_presence_check.seal_transplanted_fixture(case_db)
            case_db.commit()
            original_db = self.db
            self.db = case_db
            try:
                self.add_empty_cmp_edition(edition_id, edition_id + 1)
                next_id = 3000 + edition_id * 1000
                previous = 0
                by_count = {}
                for count in (100, 200, 400):
                    for source_order in range(previous, count):
                        self.add_cmp_set(
                            next_id, source_order, positioned=True,
                            edition_id=edition_id, same_line=same_line,
                        )
                        next_id += 1
                    previous = count
                    steps = [0]

                    def count_vm_step():
                        steps[0] += 1
                        return 0

                    self.db.set_progress_handler(count_vm_step, 1)
                    try:
                        rejected = self.db.execute(
                            "SELECT EXISTS(SELECT 1 FROM candidate_integrity_problems "
                            "WHERE edition_id=? LIMIT 1)",
                            (edition_id,),
                        ).fetchone()[0]
                    finally:
                        self.db.set_progress_handler(None, 0)
                    self.assertEqual(rejected, 0)
                    by_count[count] = steps[0]
            finally:
                self.db = original_db
                case_db.close()
            measurements["same-line" if same_line else "varying-line"] = by_count
            self.assertLess(by_count[200], by_count[100] * 3)
            self.assertLess(by_count[400], by_count[200] * 3)
        print("edition-filtered publication audit VM steps:", measurements)


if __name__ == "__main__":
    unittest.main(verbosity=2)
