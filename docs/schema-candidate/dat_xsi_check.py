"""Constructed No-Intro XSI checks against the actual composed candidate DDL."""

import contextlib
import pathlib
import sqlite3
import sys
import unittest

sys.dont_write_bytecode = True
import assemble
import count_fixtures


ROOT = pathlib.Path(__file__).resolve().parent
# Independent native fixture identities, not extracted from the generator.
OWNERS = (
    ("document", "edition_id", 10100),
    ("header", "header_id", 11000),
    ("header_text_child", "source_element_id", 11001),
    ("clrmamepro", "source_element_id", 11020),
    ("romcenter", "source_element_id", 11021),
    ("game", "set_id", 11030),
    ("game_description", "source_element_id", 11031),
    ("category", "source_element_id", 11032),
    ("identifier", "source_element_id", 11033),
    ("release", "source_element_id", 11034),
    ("rom", "media_entry_id", 11035),
)


class DatXsiChecks(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.db = sqlite3.connect(":memory:")
        cls.addClassCleanup(cls.db.close)
        cls.db.execute("PRAGMA foreign_keys=ON")
        cls.ddl = assemble.assemble()
        cls.db.executescript(cls.ddl)
        fixture = (ROOT / "no_intro_field_witnesses.sql").read_text()
        cls.db.executescript(fixture.split("\n-- expect-error:", 1)[0])
        cls.db.execute("RELEASE no_intro_fields")
        for index, dialect in enumerate(("v3-strict", "v3-compatible", "v4-strict", "v4-compatible"), 1):
            cls.db.execute("""INSERT INTO catalog_reading_rules VALUES
                (?,?,'no_intro_dat',?,'constructed','constructed','constructed')""",
                           (90100 + index, "xsi/" + dialect, "no-intro-dat-" + dialect))
        cls.db.commit()
        cls.guarded_db = cls.db
        cls.corruptible_db = sqlite3.connect(":memory:")
        cls.addClassCleanup(cls.corruptible_db.close)
        cls.corruptible_db.execute("PRAGMA foreign_keys=ON")
        cls.db.backup(cls.corruptible_db)
        # Drop local guards once on a separate committed test database. DDL
        # inside the outer savepoint would make every later ROLLBACK TO reset
        # SQLite's schema/prepared-statement cache, not just undo fixture DML.
        for suffix, _, _ in OWNERS:
            for operation in ('insert', 'update'):
                cls.corruptible_db.execute(f"DROP TRIGGER candidate_dat_xsi_{suffix}_{operation}")
        cls.corruptible_db.commit()

    def setUp(self):
        self.db = (self.guarded_db if self._testMethodName ==
                   'test_local_declaration_guards_cover_insert_update_and_rollback'
                   else self.corruptible_db)
        self.db.execute("SAVEPOINT xsi_test")

    def tearDown(self):
        self.db.execute("ROLLBACK TO xsi_test")
        self.db.execute("RELEASE xsi_test")

    @contextlib.contextmanager
    def isolated(self):
        self.db.execute("SAVEPOINT xsi_case")
        try:
            yield
        finally:
            self.db.execute("ROLLBACK TO xsi_case")
            self.db.execute("RELEASE xsi_case")

    def problems(self, edition=10100):
        return self.db.execute("SELECT problem,owner_id,edition_id FROM candidate_dat_xsi_problems "
                               "WHERE edition_id=? ORDER BY problem,owner_id", (edition,)).fetchall()

    def mode(self, dialect):
        index = ("v3-strict", "v3-compatible", "v4-strict", "v4-compatible").index(dialect) + 1
        self.db.execute("UPDATE catalog_editions SET reading_rules_id=? WHERE edition_id=10100",
                        (90100 + index,))

    def strict(self):
        for suffix, _, _ in OWNERS:
            self.db.execute(f"DELETE FROM no_intro_dat_{suffix}_xsi_attributes WHERE field_kind='nil'")
        self.mode("v4-strict")

    def attr(self, suffix, key, owner, field, value, **extra):
        fields = {"value_text": value, **extra}
        self.db.execute(f"UPDATE no_intro_dat_{suffix}_xsi_attributes SET " +
                        ",".join(name + "=?" for name in fields) + f" WHERE {key}=? AND field_kind=?",
                        (*fields.values(), owner, field))

    def selected_type(self, suffix, owner, kind, value):
        self.attr(suffix, "source_element_id", owner, "type", "schema:" + kind, resolved_type_kind=kind)
        table, column = {
            "header_text_child": ("no_intro_dat_header_text_children", "value_text"),
            "game_description": ("no_intro_dat_game_descriptions", "description_text"),
            "category": ("no_intro_dat_categories", "category"),
            "identifier": ("no_intro_dat_identifiers", "identifier"),
        }[suffix]
        self.db.execute(f"UPDATE {table} SET {column}=? WHERE source_element_id=?", (value, owner))

    def publish(self, edition):
        self.db.execute("""INSERT INTO published_catalog_editions
            SELECT edition_id,catalog_id,source_file_id,reading_rules_id,coverage_id,'xsi-witness'
            FROM catalog_editions WHERE edition_id=?""", (edition,))

    def strict_sparse_document(self):
        self.db.execute("UPDATE catalog_editions SET reading_rules_id=90103 WHERE edition_id=10400")
        self.db.execute("UPDATE no_intro_dat_header_text_children SET value_text='42' WHERE source_element_id=14001")
        # author precedes directives in the strict header sequence.
        self.db.execute("UPDATE no_intro_dat_clrmamepro_options SET source_order=5 WHERE source_element_id=14020")
        self.db.execute("INSERT INTO catalog_source_elements VALUES (14005,10400,'no_intro_dat_header_text_child')")
        self.db.execute("INSERT INTO no_intro_dat_header_text_children VALUES (14005,14000,'author','author',4,10,1)")

    def test_baseline_and_all_owner_routes(self):
        self.assertEqual(self.problems(), [])
        self.assertEqual(self.db.execute("SELECT count(*) FROM candidate_dat_xsi_attributes WHERE edition_id=10100").fetchone(), (81,))
        for suffix, key, owner in OWNERS:
            with self.subTest(owner=suffix), self.isolated():
                self.attr(suffix, key, owner, "nil", "true")
                self.assertIn(("dat_xsi_nil", owner, 10100), self.problems())

    def test_local_declaration_guards_cover_insert_update_and_rollback(self):
        for suffix, key, owner in OWNERS:
            for field, value, extra in (
                ('nil', 'true', {}), ('nil', 'false', {'source_qname': 'unprefixed'}),
                ('schemaLocation', 'odd', {}),
                ('noNamespaceSchemaLocation', 'schema_nointro_datfile_v3.xsd', {}),
            ):
                with self.subTest(owner=suffix, field=field), self.isolated():
                    with self.assertRaisesRegex(sqlite3.IntegrityError, 'invalid DAT XSI declaration'):
                        self.attr(suffix, key, owner, field, value, **extra)
                    self.assertEqual(self.problems(), [])
            with self.subTest(insert_owner=suffix), self.isolated():
                self.db.execute(f"DELETE FROM no_intro_dat_{suffix}_xsi_attributes WHERE {key}=? AND field_kind='nil'", (owner,))
                columns = f"{key},field_kind,value_text,source_qname,attribute_ordinal,source_line,source_column"
                with self.assertRaisesRegex(sqlite3.IntegrityError, 'invalid DAT XSI declaration'):
                    self.db.execute(f"INSERT INTO no_intro_dat_{suffix}_xsi_attributes ({columns}) VALUES (?,'nil','1','i:nil',50,1,1)", (owner,))
        for kind in ('string', 'int'):
            with self.subTest(type=kind), self.isolated():
                if kind == 'string':
                    args = ('header_text_child', 'source_element_id', 11001)
                else:
                    args = ('header_text_child', 'source_element_id', 11002)
                with self.assertRaisesRegex(sqlite3.IntegrityError, 'invalid DAT XSI declaration'):
                    self.attr(*args, 'type', 'xs:' + kind, resolved_type_kind=kind)
        with self.isolated():
            self.db.execute("INSERT INTO catalog_reading_rules VALUES (90200,'xsi/unknown','no_intro_dat','unrecognized','test','test','test')")
            self.db.execute("UPDATE catalog_editions SET reading_rules_id=90200 WHERE edition_id=10100")
            with self.assertRaisesRegex(sqlite3.IntegrityError, 'invalid DAT XSI declaration'):
                self.attr('document', 'edition_id', 10100, 'nil', 'false')
            self.assertIn(('dat_xsi_reading_rules', 10100, 10100), self.problems())

    def test_nil_modes_and_exact_lexemes(self):
        for dialect in ("v3-strict", "v3-compatible", "v4-strict", "v4-compatible"):
            for value in ("false", "0", " \tfalse\n\r", "true", "1", "", "  ", "False", "0\x00junk"):
                with self.subTest(dialect=dialect, value=value), self.isolated():
                    self.mode(dialect)
                    self.attr("document", "edition_id", 10100, "nil", value)
                    bad = ("dat_xsi_nil", 10100, 10100) in self.problems()
                    self.assertEqual(bad, dialect.endswith("strict") or value not in ("false", "0", " \tfalse\n\r"))
                    self.assertEqual(self.db.execute("SELECT value_text FROM no_intro_dat_document_xsi_attributes WHERE edition_id=10100 AND field_kind='nil'").fetchone(), (value,))

    def test_source_qname_and_exact_attribute_local_name(self):
        for suffix, key, owner in OWNERS:
            for qname, valid in (("instance:nil", True), ("π:nil", True), ("nil", False),
                                 ("x:Nil", False), ("x:other", False), ("a:b:nil", False),
                                 (" instance:nil", False), ("x\x00:nil", False)):
                with self.subTest(owner=suffix, qname=qname), self.isolated():
                    self.attr(suffix, key, owner, "nil", "false", source_qname=qname)
                    self.assertEqual(("dat_xsi_source_qname", owner, 10100) in self.problems(), not valid)

    def test_type_qname_and_derivation_even_in_compatible_mode(self):
        for value, kind, valid in (("xs:int", "int", True), (" int ", "int", True),
                                   ("\tx:byte\r", "byte", True), ("x:string", "string", False),
                                   ("x:short", "int", False), ("x: int", "int", False),
                                   ("x:y:int", "int", False), ("x:int\x00", "int", False)):
            with self.subTest(value=value, kind=kind), self.isolated():
                self.attr("header_text_child", "source_element_id", 11001, "type", value, resolved_type_kind=kind)
                self.assertEqual(("dat_xsi_type", 11001, 10100) in self.problems(), not valid)
        for suffix, owner in (("header_text_child", 11002), ("game_description", 11031),
                              ("category", 11032), ("identifier", 11033)):
            with self.subTest(owner=suffix), self.isolated():
                # Numeric built-ins cannot narrow a string owner. Non-header
                # relations additionally reject them with their local CHECK.
                if suffix == "header_text_child":
                    self.selected_type(suffix, owner, "int", "42")
                    self.assertIn(("dat_xsi_type", owner, 10100), self.problems())
                else:
                    with self.assertRaises(sqlite3.IntegrityError):
                        self.selected_type(suffix, owner, "int", "42")

    def test_strict_default_and_narrowed_integer_values(self):
        self.strict()
        for kind, text, valid in (("int", "-2147483648", True), ("int", "2147483648", False),
                                  ("short", "32767", True), ("short", "32768", False),
                                  ("byte", "-128", True), ("byte", "128", False),
                                  ("byte", " +00042\t", True), ("byte", "4 2", False),
                                  ("byte", "-0", True), ("byte", "４２", False)):
            with self.subTest(kind=kind, text=text), self.isolated():
                self.selected_type("header_text_child", 11001, kind, text)
                self.assertEqual(("dat_xsi_simple_value", 11001, 10100) in self.problems(), not valid)
                self.mode("v4-compatible")
                self.assertNotIn(("dat_xsi_simple_value", 11001, 10100), self.problems())
        self.db.execute("DELETE FROM no_intro_dat_header_text_child_xsi_attributes WHERE source_element_id=11001 AND field_kind='type'")
        self.db.execute("UPDATE no_intro_dat_header_text_children SET value_text='' WHERE source_element_id=11001")
        self.assertIn(("dat_xsi_simple_value", 11001, 10100), self.problems())

    def test_selected_string_types_and_compatible_retention(self):
        self.strict()
        for kind, value, valid in (
            ("string", " a\t b ", True), ("normalizedString", "a\tb", True),
            ("token", " a\t b ", True), ("language", " en-US ", True),
            ("language", "en_Us", False), ("Name", "prefix:name", True),
            ("NCName", "π_名", True), ("NCName", "a:b", False),
            ("NMTOKEN", "1:a", True), ("NMTOKEN", "a b", False),
            ("ENTITY", "valid_name", False), ("ENTITY", "", False),
        ):
            with self.subTest(kind=kind, value=value), self.isolated():
                self.selected_type("category", 11032, kind, value)
                self.assertEqual(("dat_xsi_simple_value", 11032, 10100) in self.problems(), not valid)
                self.mode("v3-compatible")
                self.assertEqual(self.problems(), [])

    def test_schema_hint_token_pairs_and_revision_locations(self):
        for field, value, problem in (
            ("schemaLocation", "", None), ("schemaLocation", " \t\r\n", None),
            ("schemaLocation", "urn:any anything", None),
            ("schemaLocation", "schema_nointro_datfile_v3.xsd arbitrary", None),
            ("schemaLocation", "urn:any schema_nointro_datfile_v3.xsd", "dat_xsi_schema_revision"),
            ("schemaLocation", "urn:any", "dat_xsi_schema_pairs"),
            ("schemaLocation", "a b c", "dat_xsi_schema_pairs"),
            ("schemaLocation", "a\tb\r c\nd", None),
            ("noNamespaceSchemaLocation", "", None),
            ("noNamespaceSchemaLocation", "spaces are not URI validated", None),
            ("noNamespaceSchemaLocation", "C:\\dir\\schema_nointro_datfile_v3.xsd?x#y", "dat_xsi_schema_revision"),
            ("noNamespaceSchemaLocation", "https://x/schema_nointro_datfile_v3.xsd#x?y", "dat_xsi_schema_revision"),
            ("noNamespaceSchemaLocation", "x_schema_nointro_datfile_v3.xsd", None),
            ("noNamespaceSchemaLocation", "SCHEMA_NOINTRO_DATFILE_V3.XSD", None),
            ("noNamespaceSchemaLocation", "schema_nointro_datfile_v4.xsd", None),
        ):
            for suffix, key, owner in OWNERS:
                with self.subTest(field=field, value=value, owner=suffix), self.isolated():
                    self.attr(suffix, key, owner, field, value)
                    self.assertEqual(self.problems(), [] if problem is None else [(problem, owner, 10100)])

    def test_ids_forward_references_duplicates_and_edition_scope(self):
        self.strict()
        self.selected_type("game_description", 11031, "IDREF", " anchor ")
        self.assertEqual(self.problems(), [("dat_xsi_unresolved_idref", 11031, 10100)])
        self.selected_type("category", 11032, "ID", "\tanchor\r")
        self.assertEqual(self.problems(), [])
        self.selected_type("header_text_child", 11002, "ID", "anchor")
        self.assertEqual(self.problems(), [("dat_xsi_duplicate_id", 11002, 10100),
                                          ("dat_xsi_duplicate_id", 11032, 10100)])
        self.mode("v4-compatible")
        self.assertEqual(self.problems(), [])
        self.mode("v4-strict")
        self.selected_type("header_text_child", 11002, "string", "anchor")
        self.selected_type("category", 11032, "ID", "different")
        # A matching ID in another edition is not a target.
        self.db.execute("INSERT INTO no_intro_dat_header_text_child_xsi_attributes VALUES "
                        "(14002,'type','xs:ID','i:type','ID',0,10,1)")
        self.db.execute("UPDATE no_intro_dat_header_text_children SET value_text='anchor' WHERE source_element_id=14002")
        self.assertEqual(self.problems(), [("dat_xsi_unresolved_idref", 11031, 10100)])

    def test_publication_wires_semantic_audit_and_freezes_valid_hints(self):
        # The separate sparse fixture can form a valid strict publication; no
        # games or media are invented to exercise a header-only source.
        self.strict_sparse_document()
        self.db.execute("INSERT INTO no_intro_dat_document_xsi_attributes VALUES (10400,'schemaLocation','odd','i:schemaLocation',0,1,1)")
        count_fixtures.seal(self.db, "no_intro_dat", 10400, {
            "header_count": 1, "header_text_child_count": 5, "clrmamepro_element_count": 1,
            "document_xsi_attribute_count": 1,
        })
        self.assertEqual(self.problems(10400), [("dat_xsi_schema_pairs", 10400, 10400)])
        self.assertIn(("dat_xsi_schema_pairs", 10400, 10400), self.db.execute(
            "SELECT * FROM candidate_integrity_problems WHERE edition_id=10400").fetchall())
        with self.assertRaisesRegex(sqlite3.IntegrityError, "complete closure"):
            self.publish(10400)
        self.db.execute("UPDATE no_intro_dat_document_xsi_attributes SET value_text='namespace schema_nointro_datfile_v4.xsd' WHERE edition_id=10400")
        self.assertEqual(self.db.execute("SELECT * FROM candidate_integrity_problems WHERE edition_id=10400").fetchall(), [])
        self.publish(10400)
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("UPDATE no_intro_dat_document_xsi_attributes SET value_text='changed' WHERE edition_id=10400")

    def test_forward_idref_publication_repair_and_immutable_target(self):
        self.strict_sparse_document()
        self.db.executemany("INSERT INTO no_intro_dat_header_text_child_xsi_attributes VALUES (?, 'type', ?, 'i:type', ?, 0,10,1)",
                            ((14002, 'xs:IDREF', 'IDREF'), (14005, 'xs:ID', 'ID')))
        self.db.execute("UPDATE no_intro_dat_header_text_children SET value_text='target' WHERE source_element_id=14002")
        self.db.execute("UPDATE no_intro_dat_header_text_children SET value_text='different' WHERE source_element_id=14005")
        count_fixtures.seal(self.db, "no_intro_dat", 10400, {
            "header_count": 1, "header_text_child_count": 5, "clrmamepro_element_count": 1,
            "header_text_child_xsi_attribute_count": 2,
        })
        self.assertEqual(self.db.execute("SELECT * FROM candidate_integrity_problems WHERE edition_id=10400").fetchall(),
                         [("dat_xsi_unresolved_idref", 14002, 10400)])
        with self.assertRaisesRegex(sqlite3.IntegrityError, "complete closure"):
            self.publish(10400)
        self.db.execute("UPDATE no_intro_dat_header_text_children SET value_text='target' WHERE source_element_id=14005")
        self.publish(10400)
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("UPDATE no_intro_dat_header_text_children SET value_text='different' WHERE source_element_id=14005")

    def test_edition_identity_reads_ignore_unrelated_owners_and_global_work_is_bounded(self):
        self.strict()
        self.selected_type('game_description', 11031, 'IDREF', 'target')
        self.selected_type('category', 11032, 'ID', 'target')
        self.db.execute("UPDATE catalog_editions SET reading_rules_id=90103 WHERE edition_id=10400")
        self.db.execute("UPDATE no_intro_dat_header_text_children SET value_text='42' WHERE source_element_id=14001")
        self.db.execute("INSERT INTO catalog_source_elements VALUES (15030,10400,'no_intro_dat_game')")
        self.db.execute("INSERT INTO catalog_sets VALUES (15030,10410,'scale',1,10,1)")
        self.db.execute("INSERT INTO no_intro_dat_games VALUES (15030,NULL)")
        queries = (
            'SELECT * FROM candidate_dat_simple_values WHERE edition_id=10100',
            'SELECT * FROM candidate_dat_identity_problems WHERE edition_id=10100',
            'SELECT * FROM candidate_dat_xsi_problems WHERE edition_id=10100',
        )

        def steps(query):
            expected = self.db.execute(query).fetchall()  # prepare outside count
            count = 0

            def instruction():
                nonlocal count
                count += 1
                return 0

            self.db.set_progress_handler(instruction, 1)
            try:
                self.assertEqual(self.db.execute(query).fetchall(), expected)
            finally:
                self.db.set_progress_handler(None, 0)
            return count

        baseline = [steps(query) for query in queries]
        global_counts = []
        for first in (0, 512):
            for pair in range(first, first + 512):
                for index, kind in enumerate(('ID', 'IDREF')):
                    owner = 200000 + pair * 2 + index
                    self.db.execute("INSERT INTO catalog_source_elements VALUES (?,10400,'no_intro_dat_category')", (owner,))
                    self.db.execute("INSERT INTO no_intro_dat_categories VALUES (?,15030,?,?,10,1)",
                                    (owner, 'identity_' + str(pair), pair * 2 + index + 10))
                    self.db.execute("INSERT INTO no_intro_dat_category_xsi_attributes VALUES (?, 'type', ?, 'i:type', ?, 0,10,1)",
                                    (owner, 'xs:' + kind, kind))
                    self.db.execute("INSERT INTO no_intro_dat_category_xsi_attributes VALUES (?, 'schemaLocation', 'n schema_nointro_datfile_v4.xsd', 'i:schemaLocation', NULL,1,10,2)", (owner,))
            for query, before in zip(queries, baseline):
                self.assertLessEqual(steps(query), before + 100, query)
            global_counts.append(steps('SELECT * FROM candidate_dat_identity_problems'))
        self.assertLessEqual(global_counts[1], global_counts[0] * 2.5)
        plan = '\n'.join(row[3] for row in self.db.execute('EXPLAIN QUERY PLAN ' + queries[0]))
        self.assertIn('SEARCH element USING COVERING INDEX catalog_source_elements_edition_kind', plan)
        self.assertNotIn('SCAN native', plan)


if __name__ == "__main__":
    unittest.main(verbosity=2)
