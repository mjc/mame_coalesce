"""Design-only No-Intro value/position closure tests. No production imports/writes.

Run in the active project environment:
  python3 -Werror::ResourceWarning docs/schema-candidate/no_intro_presence_check.py

Uses main's actual assemble()/field_presence_sql() and one composed in-memory
database per class. Savepoints isolate every corruption; existing SQL supplies
constructed typed owners, not a source/parser/corpus completeness oracle.
"""
import contextlib
import csv
import pathlib
import re
import sqlite3
import sys
import unittest

sys.dont_write_bytecode = True
import assemble
import count_fixtures

ROOT = pathlib.Path(__file__).resolve().parent

# Independent DOC-14@37510 / DOC-9@38191 inventory, not extracted from the
# manifest/coverage TSV or from its claimed count.
NATIVE = (
    ("no_intro_dat_clrmamepro_options", "source_element_id", "no_intro_dat_clrmamepro_field_positions", "source_element_id", "forcenodump header"),
    ("no_intro_dat_romcenter_options", "source_element_id", "no_intro_dat_romcenter_field_positions", "source_element_id", "plugin"),
    ("no_intro_dat_games", "set_id", "no_intro_dat_game_field_positions", "set_id", "name id cloneof cloneofid"),
    ("no_intro_dat_releases", "source_element_id", "no_intro_dat_release_field_positions", "source_element_id", "name region"),
    ("no_intro_dat_rom_claims", "media_entry_id", "no_intro_dat_rom_field_positions", "media_entry_id", "name size crc md5 sha1 sha256 status serial header date mia"),
    ("no_intro_pc_games", "set_id", "no_intro_pc_game_attribute_positions", "source_element_id", "name id namealt region languages version bios clone mergeof"),
    ("no_intro_pc_file_claims", "media_entry_id", "no_intro_pc_rom_attribute_positions", "source_element_id", "name size crc md5 sha1"),
    ("no_intro_export_games", "set_id", "no_intro_export_game_field_positions", "set_id", "name"),
    ("no_intro_archive_descriptions", "archive_id", "no_intro_archive_field_positions", "archive_id", "additional adult aftermarket alt bios categories clone complete dat datter_note description devstatus gameid1 gameid2 langchecked languages licensed listed mergename mergeof name name_alt number physical region regparent showlang special1 special2 sticky_note version1 version2"),
    ("no_intro_dump_details", "details_element_id", "no_intro_dump_details_field_positions", "details_element_id", "comment1 comment2 d_date d_date_info dumper id link1 link2 link3 media_title nodump origin originalformat project r_date r_date_info region rominfo section tool"),
    ("no_intro_dump_serials", "serials_element_id", "no_intro_dump_serials_field_positions", "serials_element_id", "box_barcode box_serial chip_serial digital_serial1 digital_serial2 lockout_serial media_serial1 media_serial2 media_serial3 mediastamp pcb_serial romchip_serial1 romchip_serial2 savechip_serial"),
    ("no_intro_dump_files", "media_entry_id", "no_intro_dump_file_field_positions", "media_entry_id", "bad crc32 date extension filter forcename forcescenename format header id item md5 mia note origin_sha256 origin_size serial sha1 sha256 size unique update_type version"),
    ("no_intro_release_details", "details_element_id", "no_intro_release_details_field_positions", "details_element_id", "archivename category comment date dirname group id nfo_crc32 nfo_size nfocrc nfoname nfosize origin originalformat region rominfo tool"),
    ("no_intro_release_serials", "serials_element_id", "no_intro_release_serials_field_positions", "serials_element_id", "box_barcode box_serial media_serial1 mediastamp pcb_serial romchip_serial1"),
    ("no_intro_release_files", "media_entry_id", "no_intro_release_file_field_positions", "media_entry_id", "bad crc32 extension forcename forcescenename format header id item md5 note serial sha1 sha256 size update_type version"),
)
XSI = (
    ("no_intro_dat_documents", "edition_id", "document", False),
    ("no_intro_dat_headers", "header_id", "header", False),
    ("no_intro_dat_header_text_children", "source_element_id", "header_text_child", True),
    ("no_intro_dat_clrmamepro_options", "source_element_id", "clrmamepro", False),
    ("no_intro_dat_romcenter_options", "source_element_id", "romcenter", False),
    ("no_intro_dat_games", "set_id", "game", False),
    ("no_intro_dat_game_descriptions", "source_element_id", "game_description", True),
    ("no_intro_dat_categories", "source_element_id", "category", True),
    ("no_intro_dat_identifiers", "source_element_id", "identifier", True),
    ("no_intro_dat_releases", "source_element_id", "release", False),
    ("no_intro_dat_rom_claims", "media_entry_id", "rom", False),
)
POPULATED = {
    "no_intro_dat_clrmamepro_options": 11020,
    "no_intro_dat_romcenter_options": 11021,
    "no_intro_dat_games": 11030,
    "no_intro_dat_releases": 11034,
    "no_intro_dat_rom_claims": 11035,
    "no_intro_pc_games": 12010,
    "no_intro_pc_file_claims": 12012,
    "no_intro_export_games": 13010,
    "no_intro_archive_descriptions": 13020,
    "no_intro_dump_details": 13031,
    "no_intro_dump_serials": 13032,
    "no_intro_dump_files": 13033,
    "no_intro_release_details": 13041,
    "no_intro_release_serials": 13042,
    "no_intro_release_files": 13043,
    "no_intro_dat_documents": 10100,
    "no_intro_dat_headers": 11000,
    "no_intro_dat_header_text_children": 11001,
    "no_intro_dat_game_descriptions": 11031,
    "no_intro_dat_categories": 11032,
    "no_intro_dat_identifiers": 11033,
}
EDITIONS = {table: 10200 if table.startswith("no_intro_pc_") else
            10100 if table.startswith("no_intro_dat_") else 10300
            for table in POPULATED}
HASH_FIELDS = {"crc", "crc32", "md5", "sha1", "sha256", "origin_sha256"}
RELATION_FIELDS = {"cloneof", "cloneofid", "clone", "mergeof"}


def quoted(name):
    assert re.fullmatch(r"[a-z_][a-z_0-9]*", name), name
    return '"' + name + '"'


def expected_routes():
    routes = {(position, code): (owner, key, parent)
              for owner, key, position, parent, fields in NATIVE
              for code in fields.split()}
    for owner, key, kind, simple in XSI:
        table = "no_intro_dat_" + kind + "_xsi_attributes"
        for code in ("schemaLocation", "noNamespaceSchemaLocation", *(("type",) if simple else ()), "nil"):
            routes[table, code] = (owner, key, key)
    return routes


def populate_fixture(db, *, file_byte_contracts=()):
    """Populate the constructed witnesses, optionally issuing contracts first."""
    fixture = (ROOT / "no_intro_field_witnesses.sql").read_text()
    fixture = fixture.split("\n-- expect-error:", 1)[0]
    marker = "INSERT INTO catalog_editions("
    if fixture.count(marker) != 4:
        raise AssertionError("No-Intro edition fixture boundary changed")
    contracts = "".join(
        "INSERT INTO catalog_file_byte_contracts VALUES "
        f"({int(rules_id)},{assemble.literal(contract_kind)});\n"
        for rules_id, contract_kind in file_byte_contracts
    )
    if contracts:
        fixture = fixture.replace(marker, contracts + marker, 1)
    db.executescript(fixture)
    # Independent sparse literals for the constructed native rows. Export
    # family 10300 intentionally has no source-count seal.
    count_fixtures.seal(db, "no_intro_dat", 10100, {
        "header_count": 1, "header_text_child_count": 12,
        "clrmamepro_element_count": 1, "romcenter_element_count": 1,
        "game_count": 1, "game_description_count": 1, "category_count": 1,
        "identifier_count": 1, "release_count": 1, "rom_count": 1,
        "game_attribute_position_count": 4, "release_attribute_position_count": 2,
        "clrmamepro_attribute_position_count": 2,
        "romcenter_attribute_position_count": 1, "rom_attribute_position_count": 11,
        "document_xsi_attribute_count": 3, "header_xsi_attribute_count": 3,
        "header_text_child_xsi_attribute_count": 48,
        "clrmamepro_xsi_attribute_count": 3, "romcenter_xsi_attribute_count": 3,
        "game_xsi_attribute_count": 3, "game_description_xsi_attribute_count": 4,
        "category_xsi_attribute_count": 4, "identifier_xsi_attribute_count": 4,
        "release_xsi_attribute_count": 3, "rom_xsi_attribute_count": 3,
    })
    count_fixtures.seal(db, "no_intro_dat", 10400, {
        "header_count": 1, "header_text_child_count": 4,
        "clrmamepro_element_count": 1,
    })
    count_fixtures.seal(db, "no_intro_pc_fixture", 10200, {
        "header_count": 1, "header_name_child_count": 1,
        "header_description_child_count": 1, "header_version_child_count": 1,
        "game_count": 2, "language_token_count": 3, "game_description_count": 1,
        "rom_count": 3, "game_attribute_position_count": 12,
        "rom_attribute_position_count": 7,
    })
    db.execute("RELEASE no_intro_fields")


class NoIntroPresence(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.routes = assemble.field_presence_routes(("no_intro",))
        cls.db = sqlite3.connect(":memory:")
        cls.addClassCleanup(cls.db.close)
        cls.db.execute("PRAGMA foreign_keys=ON")
        # Actual composed DDL includes generated audit + publication wiring.
        # Main calls its generator once; do not assemble once per test/route.
        cls.db.executescript(assemble.assemble())
        # Only the constructed positive setup, before expected-failure controls.
        populate_fixture(cls.db)
        cls.by_code = {(r["position_table"], r["field_code"]): r for r in cls.routes}

    def setUp(self):
        self.db.execute("SAVEPOINT no_intro_presence_test")

    def tearDown(self):
        self.db.execute("ROLLBACK TO no_intro_presence_test")
        self.db.execute("RELEASE no_intro_presence_test")

    @contextlib.contextmanager
    def isolated(self):
        self.db.execute("SAVEPOINT no_intro_presence_mutation")
        try:
            yield
        finally:
            self.db.execute("ROLLBACK TO no_intro_presence_mutation")
            self.db.execute("RELEASE no_intro_presence_mutation")

    def problems(self, position=None, code=None, owner=None):
        query = "SELECT problem,owner_id,edition_id FROM candidate_field_presence_problems"
        clauses, arguments = [], []
        if position is not None:
            clauses.append("problem=?")
            arguments.append("field_presence:" + position + ":" + code)
        if owner is not None:
            clauses.append("owner_id=?")
            arguments.append(owner)
        if clauses:
            query += " WHERE " + " AND ".join(clauses)
        return self.db.execute(query, arguments).fetchall()

    def expect_problem(self, table, code, owner, edition):
        self.assertEqual(self.problems(table, code, owner),
                         [("field_presence:" + table + ":" + code, owner, edition)])

    def publish(self, edition):
        self.db.execute(
            "INSERT INTO published_catalog_editions "
            "(edition_id,catalog_id,source_file_id,reading_rules_id,coverage_id,published_at) "
            "SELECT edition_id,catalog_id,source_file_id,reading_rules_id,coverage_id,'presence-witness' "
            "FROM catalog_editions WHERE edition_id=?", (edition,))

    def test_exact_physical_inventory_and_actual_fk_routes(self):
        expected = expected_routes()
        self.assertEqual(len(self.routes), len(expected))
        self.assertEqual({(r["position_table"], r["field_code"]):
                          (r["owner_table"], r["owner_key"], r["position_owner"])
                          for r in self.routes}, expected)
        with (ROOT / "no_intro-field-coverage.tsv").open(newline="") as source:
            coverage = list(csv.DictReader(source, delimiter="\t"))
        self.assertEqual({(r["position_table"], r["field_code"]) for r in coverage
                          if r["position_table"] != "-"}, set(expected))
        for position in {key[0] for key in expected}:
            with self.subTest(table=position):
                sql = self.db.execute("SELECT sql FROM sqlite_schema WHERE name=?", (position,)).fetchone()[0]
                closed = re.search(r"field_kind\s+TEXT\s+NOT\s+NULL\s+CHECK\s*\(\s*field_kind\s+(?:IN\s*\(([^)]*)\)|=\s*('([^']*)'))", sql, re.I)
                self.assertIsNotNone(closed)
                codes = set(re.findall(r"'([^']*)'", closed.group(1) or closed.group(2)))
                self.assertEqual(codes, {code for table, code in expected if table == position})
                for (table, code), (owner, key, parent) in expected.items():
                    if table == position:
                        fks = {(r[2], r[3], r[4]) for r in self.db.execute("PRAGMA foreign_key_list(" + quoted(table) + ")")}
                        self.assertIn((owner, parent, key), fks)
                        columns = {r[1] for r in self.db.execute("PRAGMA table_info(" + quoted(table) + ")")}
                        if table.endswith("_xsi_attributes"):
                            pk = [r[1] for r in sorted(self.db.execute("PRAGMA table_info(" + quoted(table) + ")"), key=lambda r: r[5]) if r[5]]
                            self.assertEqual(pk, [parent, "field_kind"])
                            self.assertNotIn("field_occurrence", columns)
                        else:
                            self.assertIn("field_occurrence", columns)

    def test_composed_baseline_predicates_boolean_and_gate_clean(self):
        self.assertEqual(self.problems(), [])
        for route in self.routes:
            with self.subTest(route=(route["position_table"], route["field_code"])):
                predicate = route["present_sql"]
                self.assertFalse(any(c in predicate for c in ";\n\r"))
                actual = self.db.execute("SELECT (" + predicate + ") FROM " +
                    quoted(route["owner_table"]) + " AS owner WHERE owner." +
                    quoted(route["owner_key"]) + "=?",
                    (POPULATED[route["owner_table"]],)).fetchone()
                self.assertEqual(actual, (1,))
        # Establish a clean publication control before asserting failures.
        for edition in (10100, 10200, 10300, 10400):
            problems = self.db.execute("SELECT problem,owner_id FROM candidate_integrity_problems WHERE edition_id=?", (edition,)).fetchall()
            self.assertEqual(problems, [], edition)
            self.publish(edition)

    def test_every_ordinary_code_missing_position_is_red(self):
        for table, key, position, parent, fields in NATIVE:
            for code in fields.split():
                with self.subTest(position=position, code=code), self.isolated():
                    owner = POPULATED[table]
                    result = self.db.execute("DELETE FROM " + quoted(position) +
                        " WHERE " + quoted(parent) + "=? AND field_kind=? AND field_occurrence=0", (owner, code))
                    self.assertEqual(result.rowcount, 1)
                    self.expect_problem(position, code, owner, EDITIONS[table])

    def test_nullable_scalar_erasure_with_position_is_red(self):
        checked = 0
        for table, key, position, parent, fields in NATIVE:
            for code in fields.split():
                if code in HASH_FIELDS | RELATION_FIELDS | {"nfo_crc32", "nfocrc"}:
                    continue
                if code == "name" and table in {"no_intro_dat_games", "no_intro_pc_games", "no_intro_export_games"}:
                    continue
                if table == "no_intro_pc_games" and code == "languages":
                    continue
                column = {"id": "publisher_id_text"}.get(code, code) if table == "no_intro_dat_games" else code
                if table == "no_intro_pc_games":
                    column = {"id": "archive_id", "namealt": "name_alt", "bios": "bios_text"}.get(code, code)
                if code == "size":
                    column = "size_text"
                if table == "no_intro_dat_rom_claims" and code in {"status", "serial", "header", "date", "mia"}:
                    column += "_text"
                info = {r[1]: r for r in self.db.execute("PRAGMA table_info(" + quoted(table) + ")")}
                if info[column][3]:  # Required values cannot be SQL NULL.
                    continue
                with self.subTest(table=table, code=code), self.isolated():
                    owner = POPULATED[table]
                    self.db.execute("UPDATE " + quoted(table) + " SET " + quoted(column) +
                                    "=NULL WHERE " + quoted(key) + "=?", (owner,))
                    self.expect_problem(position, code, owner, EDITIONS[table])
                    checked += 1
        self.assertGreater(checked, 100)

    def test_absent_empty_explicit_default_and_publication(self):
        position = "no_intro_dat_clrmamepro_field_positions"
        self.assertEqual(self.problems(position, "forcenodump", 14020), [])
        self.db.execute("UPDATE no_intro_dat_clrmamepro_options SET forcenodump='' WHERE source_element_id=11020")
        self.assertEqual(self.problems(position, "forcenodump", 11020), [])
        self.db.execute("UPDATE no_intro_dat_clrmamepro_options SET forcenodump='obsolete' WHERE source_element_id=14020")
        self.expect_problem(position, "forcenodump", 14020, 10400)
        with self.assertRaisesRegex(sqlite3.IntegrityError, "publication requires complete closure"):
            self.publish(10400)
        self.db.execute("INSERT INTO no_intro_dat_clrmamepro_field_positions VALUES(14020,'forcenodump',0,0,1,1)")
        self.assertEqual(self.problems(position, "forcenodump", 14020), [])
        self.db.execute(
            "UPDATE no_intro_dat_source_count_seals "
            "SET clrmamepro_attribute_position_count=1 WHERE edition_id=10400")
        self.publish(10400)

    def test_optional_pc_size_and_invented_position(self):
        position = "no_intro_pc_rom_attribute_positions"
        self.assertEqual(self.problems(position, "size", 12013), [])
        self.db.execute("INSERT INTO no_intro_pc_rom_attribute_positions VALUES(12013,'size',0,1,10,1,NULL)")
        self.expect_problem(position, "size", 12013, 10200)
        with self.assertRaisesRegex(sqlite3.IntegrityError, "publication requires complete closure"):
            self.publish(10200)
        self.db.execute("UPDATE no_intro_pc_file_claims SET size_text='+0000' WHERE media_entry_id=12013")
        self.assertEqual(self.problems(position, "size", 12013), [])
        self.db.execute(
            "UPDATE no_intro_pc_fixture_source_count_seals "
            "SET rom_attribute_position_count=8 WHERE edition_id=10200")
        self.publish(10200)

    def test_equal_count_owner_substitution_is_red(self):
        position = "no_intro_pc_game_attribute_positions"
        before = self.db.execute("SELECT count(*) FROM " + position).fetchone()
        self.db.execute("UPDATE no_intro_pc_game_attribute_positions SET source_element_id=12020,source_order=3 WHERE source_element_id=12010 AND field_kind='namealt'")
        self.assertEqual(self.db.execute("SELECT count(*) FROM " + position).fetchone(), before)
        self.expect_problem(position, "namealt", 12010, 10200)
        self.expect_problem(position, "namealt", 12020, 10200)
        with self.assertRaisesRegex(sqlite3.IntegrityError, "publication requires complete closure"):
            self.publish(10200)

    def test_hash_scope_owner_and_empty_invalid_presence(self):
        # Empty CRC and invalid MD5 are still present source declarations.
        for code, state in (("crc", "empty"), ("md5", "invalid")):
            self.assertEqual(self.db.execute("SELECT presence FROM catalog_entry_hashes WHERE media_entry_id=11035 AND source_hash_field=?", (code,)).fetchone(), (state,))
            self.assertEqual(self.problems("no_intro_dat_rom_field_positions", code, 11035), [])
        for owner, code, position, scope, edition in (
                (12012, "sha1", "no_intro_pc_rom_attribute_positions", "unknown", 10200),
                (13033, "origin_sha256", "no_intro_dump_file_field_positions", "unknown", 10300),
                (13043, "sha256", "no_intro_release_file_field_positions", "source_origin", 10300)):
            with self.subTest(owner=owner, code=code), self.isolated():
                self.db.execute("UPDATE catalog_entry_hashes SET hash_scope=? WHERE media_entry_id=? AND source_hash_field=?", (scope, owner, code))
                self.expect_problem(position, code, owner, edition)
        # A declaration with another alias or occurrence cannot satisfy CRC.
        for assignment in ("source_hash_field='crc32'", "field_occurrence=1"):
            with self.subTest(assignment=assignment), self.isolated():
                self.db.execute("UPDATE catalog_entry_hashes SET " + assignment +
                                " WHERE media_entry_id=11035 AND source_hash_field='crc'")
                self.expect_problem("no_intro_dat_rom_field_positions", "crc", 11035, 10100)

    def test_erasing_optional_value_and_position_is_not_source_proof(self):
        self.db.execute("UPDATE no_intro_pc_games SET name_alt=NULL WHERE set_id=12010")
        self.db.execute("DELETE FROM no_intro_pc_game_attribute_positions WHERE source_element_id=12010 AND field_kind='namealt'")
        self.assertEqual(self.problems("no_intro_pc_game_attribute_positions", "namealt", 12010), [])
        # This is a coherent absent state. Only independent source inventory can
        # distinguish it from loss of an optional accepted source declaration.
        self.assertEqual(
            self.db.execute(
                "SELECT problem,owner_id,edition_id FROM candidate_integrity_problems "
                "WHERE edition_id=10200 AND problem="
                "'source_count:no_intro_pc_fixture:game_attribute_position_count'"
            ).fetchall(),
            [("source_count:no_intro_pc_fixture:game_attribute_position_count", 10200, 10200)],
        )
        with self.assertRaisesRegex(sqlite3.IntegrityError, "publication requires complete closure"):
            self.publish(10200)

    def test_nfo_aliases_remain_on_actual_details_owner(self):
        self.assertEqual(self.db.execute("SELECT source_hash_field,presence FROM no_intro_release_nfo_hashes WHERE release_details_element_id=13041 ORDER BY source_hash_field").fetchall(), [("nfo_crc32", "value"), ("nfocrc", "invalid")])
        self.assertEqual(self.db.execute("SELECT count(*) FROM catalog_entry_hashes WHERE source_hash_field IN ('nfo_crc32','nfocrc')").fetchone(), (0,))
        self.db.execute("INSERT INTO catalog_source_elements VALUES(13900,10300,'no_intro_export_release')")
        self.db.execute("INSERT INTO no_intro_releases VALUES(13900,13010,4,10,1,10,1000)")
        self.db.execute("INSERT INTO catalog_source_elements VALUES(13901,10300,'no_intro_export_release_details')")
        self.db.execute("INSERT INTO no_intro_release_details(details_element_id,release_id,source_order,source_line,source_column,source_end_line,source_end_column,opening_end_line,opening_end_column) VALUES(13901,13900,0,10,1,10,1000,10,2)")
        self.db.execute("UPDATE no_intro_release_nfo_hashes SET release_details_element_id=13901 WHERE source_hash_field='nfocrc' AND release_details_element_id=13041")
        self.expect_problem("no_intro_release_details_field_positions", "nfocrc", 13041, 10300)
        self.expect_problem("no_intro_release_details_field_positions", "nfocrc", 13901, 10300)

    def test_p_markers_and_one_literal_relationship_branch(self):
        for table, owner, key in (("no_intro_archive_field_positions", 13021, "archive_id"),
                                  ("no_intro_pc_game_attribute_positions", 12020, "source_element_id")):
            with self.subTest(table=table), self.isolated():
                self.assertEqual(self.db.execute("SELECT relationship_id FROM " + table + " WHERE " + key + "=? AND field_kind='clone'", (owner,)).fetchone(), (None,))
                self.db.execute("DELETE FROM " + table + " WHERE " + key + "=? AND field_kind='clone'", (owner,))
                self.expect_problem(table, "clone", owner, 10200 if owner == 12020 else 10300)
        self.assertEqual(self.db.execute("SELECT declared_text FROM no_intro_archive_declared_clone_text WHERE archive_id=13020").fetchall(), [("",)])
        self.assertEqual(self.db.execute("SELECT declared_text FROM no_intro_archive_declared_clone_text WHERE archive_id=13021").fetchall(), [("P",)])
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("UPDATE no_intro_archive_clone_links SET declared_target_number='P' WHERE archive_id=13020")

    def test_language_position_is_presence_marker_not_independent_capture(self):
        position = "no_intro_pc_game_attribute_positions"
        # Nonempty token facts require a position.
        self.db.execute("DELETE FROM no_intro_pc_game_attribute_positions WHERE source_element_id=12010 AND field_kind='languages'")
        self.expect_problem(position, "languages", 12010, 10200)
        # For zero tokens, the position is the ONLY accepted presence fact.
        # Its whole erasure is indistinguishable from source omission.
        self.db.execute("DELETE FROM no_intro_pc_game_attribute_positions WHERE source_element_id=12020 AND field_kind='languages'")
        self.assertEqual(self.problems(position, "languages", 12020), [])
        self.db.execute("INSERT INTO no_intro_pc_game_attribute_positions VALUES(12020,'languages',0,1,10,1,NULL)")
        self.assertEqual(self.problems(position, "languages", 12020), [])

    def test_xsi_same_row_structural_closure_is_not_capture_proof(self):
        for table, key, kind, simple in XSI:
            position = "no_intro_dat_" + kind + "_xsi_attributes"
            for code in ("schemaLocation", "noNamespaceSchemaLocation", *(("type",) if simple else ()), "nil"):
                with self.subTest(table=position, code=code), self.isolated():
                    owner = POPULATED[table]
                    self.assertEqual(self.db.execute("DELETE FROM " + quoted(position) + " WHERE " + quoted(key) + "=? AND field_kind=?", (owner, code)).rowcount, 1)
                    self.assertEqual(self.problems(position, code, owner), [])
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("UPDATE no_intro_dat_document_xsi_attributes SET value_text=NULL WHERE edition_id=10100 AND field_kind='nil'")

    def test_native_xsi_ordinal_collision_rejects_in_composed_schema(self):
        # RED until main's position_order_sql includes attribute_ordinal.
        # Every mutation is against a real native owner with ordinary ordinal 0.
        for table, key, owner in (
                ("no_intro_dat_game_xsi_attributes", "set_id", 11030),
                ("no_intro_dat_release_xsi_attributes", "source_element_id", 11034),
                ("no_intro_dat_clrmamepro_xsi_attributes", "source_element_id", 11020),
                ("no_intro_dat_romcenter_xsi_attributes", "source_element_id", 11021),
                ("no_intro_dat_rom_xsi_attributes", "media_entry_id", 11035)):
            with self.subTest(table=table), self.isolated():
                with self.assertRaises(sqlite3.IntegrityError):
                    self.db.execute("UPDATE " + table + " SET attribute_ordinal=0 WHERE " + key + "=? AND field_kind='nil'", (owner,))

    def test_detached_registry_requires_native_owner_not_fake_position(self):
        self.db.execute("INSERT INTO catalog_source_elements VALUES(19000,10100,'no_intro_dat_game')")
        # No native row exists from which to reconstruct ordinary field presence.
        self.assertEqual(self.problems(owner=19000), [])
        self.assertTrue(self.db.execute("SELECT 1 FROM candidate_integrity_problems WHERE owner_id=19000 AND edition_id=10100").fetchone())
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO no_intro_dat_game_field_positions VALUES(19000,'name',0,0,10,1,NULL)")
        with self.assertRaisesRegex(sqlite3.IntegrityError, "publication requires complete closure"):
            self.publish(10100)

    def test_published_value_and_position_immutability(self):
        self.publish(10200)
        for statement in (
                "UPDATE no_intro_pc_games SET name_alt=NULL WHERE set_id=12010",
                "DELETE FROM no_intro_pc_game_attribute_positions WHERE source_element_id=12010 AND field_kind='namealt'",
                "INSERT INTO no_intro_pc_rom_attribute_positions VALUES(12013,'size',0,1,10,1,NULL)"):
            with self.subTest(statement=statement):
                with self.assertRaisesRegex(sqlite3.IntegrityError, "published"):
                    self.db.execute(statement)


if __name__ == "__main__":
    unittest.main(verbosity=2)
