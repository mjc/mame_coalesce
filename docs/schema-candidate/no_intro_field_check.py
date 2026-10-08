"""Design-only No-Intro ledger/constructed SQLite witnesses; stdlib, no production import.

Run from any cwd: python3 /path/to/docs/schema-candidate/no_intro_field_check.py
Assembles shared + ALL family fragments with assemble.py --emit, then runs each
No-Intro fixture in a fresh connection with FKs ON. No files/database are written.
Expected identity/route lists below are the DOC-14@37510 / DOC-9@37326 dictionary,
not an inventory inferred from candidate DDL or the TSV's reported row count.
"""
import csv
import pathlib
import re
import sqlite3
import subprocess
import sys

DIRECTORY = pathlib.Path(__file__).resolve().parent
CONTRACT = (
    ("no_intro_dat", "no_intro_dat_header_text_children", "datafile/header", "id name description version date author homepage url trademarks piracy subset comment", "-"),
    ("no_intro_dat", "no_intro_dat_clrmamepro_options", "datafile/header/clrmamepro", "forcenodump header", "no_intro_dat_clrmamepro_field_positions"),
    ("no_intro_dat", "no_intro_dat_romcenter_options", "datafile/header/romcenter", "plugin", "no_intro_dat_romcenter_field_positions"),
    ("no_intro_dat", "no_intro_dat_games", "datafile/game", "name id cloneof cloneofid", "no_intro_dat_game_field_positions"),
    ("no_intro_dat", "no_intro_dat_game_descriptions", "datafile/game/description", "text", "-"),
    ("no_intro_dat", "no_intro_dat_categories", "datafile/game/category", "text", "-"),
    ("no_intro_dat", "no_intro_dat_identifiers", "datafile/game/game_id", "text", "-"),
    ("no_intro_dat", "no_intro_dat_releases", "datafile/game/release", "name region", "no_intro_dat_release_field_positions"),
    ("no_intro_dat", "no_intro_dat_rom_claims", "datafile/game/rom", "name size crc md5 sha1 sha256 status serial header date mia", "no_intro_dat_rom_field_positions"),
    ("no_intro_pc_fixture", "no_intro_pc_header_names", "datafile/header/name", "text", "-"),
    ("no_intro_pc_fixture", "no_intro_pc_header_descriptions", "datafile/header/description", "text", "-"),
    ("no_intro_pc_fixture", "no_intro_pc_header_versions", "datafile/header/version", "text", "-"),
    ("no_intro_pc_fixture", "no_intro_pc_game_descriptions", "datafile/game/description", "text", "-"),
    ("no_intro_pc_fixture", "no_intro_pc_games", "datafile/game", "name id namealt region languages version bios clone mergeof", "no_intro_pc_game_attribute_positions"),
    ("no_intro_pc_fixture", "no_intro_pc_file_claims", "datafile/game/rom", "name size crc md5 sha1", "no_intro_pc_rom_attribute_positions"),
    ("no_intro_database", "no_intro_export_header_fields", "header", "author piracy trademarks url version", "-"),
    ("no_intro_database", "no_intro_export_games", "datafile/game", "name", "no_intro_export_game_field_positions"),
    ("no_intro_database", "no_intro_archive_descriptions", "datafile/game/archive", "additional adult aftermarket alt bios categories clone complete dat datter_note description devstatus gameid1 gameid2 langchecked languages licensed listed mergename mergeof name name_alt number physical region regparent showlang special1 special2 sticky_note version1 version2", "no_intro_archive_field_positions"),
    ("no_intro_database", "no_intro_dump_details", "datafile/game/source/details", "comment1 comment2 d_date d_date_info dumper id link1 link2 link3 media_title nodump origin originalformat project r_date r_date_info region rominfo section tool", "no_intro_dump_details_field_positions"),
    ("no_intro_database", "no_intro_dump_serials", "datafile/game/source/serials", "box_barcode box_serial chip_serial digital_serial1 digital_serial2 lockout_serial media_serial1 media_serial2 media_serial3 mediastamp pcb_serial romchip_serial1 romchip_serial2 savechip_serial", "no_intro_dump_serials_field_positions"),
    ("no_intro_database", "no_intro_dump_files", "datafile/game/source/file", "bad crc32 date extension filter forcename forcescenename format header id item md5 mia note origin_sha256 origin_size serial sha1 sha256 size unique update_type version", "no_intro_dump_file_field_positions"),
    ("no_intro_database", "no_intro_release_details", "datafile/game/release/details", "archivename category comment date dirname group id nfo_crc32 nfo_size nfocrc nfoname nfosize origin originalformat region rominfo tool", "no_intro_release_details_field_positions"),
    ("no_intro_database", "no_intro_release_serials", "datafile/game/release/serials", "box_barcode box_serial media_serial1 mediastamp pcb_serial romchip_serial1", "no_intro_release_serials_field_positions"),
    ("no_intro_database", "no_intro_release_files", "datafile/game/release/file", "bad crc32 extension forcename forcescenename format header id item md5 note serial sha1 sha256 size update_type version", "no_intro_release_file_field_positions"),
)
XSI_NAMESPACE = "{http://www.w3.org/2001/XMLSchema-instance}"
XSI_COMPLEX = (
    ("no_intro_dat_documents", "datafile", "document"),
    ("no_intro_dat_headers", "datafile/header", "header"),
    ("no_intro_dat_clrmamepro_options", "datafile/header/clrmamepro", "clrmamepro"),
    ("no_intro_dat_romcenter_options", "datafile/header/romcenter", "romcenter"),
    ("no_intro_dat_games", "datafile/game", "game"),
    ("no_intro_dat_releases", "datafile/game/release", "release"),
    ("no_intro_dat_rom_claims", "datafile/game/rom", "rom"),
)
XSI_SIMPLE = (
    ("no_intro_dat_game_descriptions", "datafile/game/description", "game_description"),
    ("no_intro_dat_categories", "datafile/game/category", "category"),
    ("no_intro_dat_identifiers", "datafile/game/game_id", "identifier"),
)
SCALAR_COLUMNS = {
    "no_intro_dat_header_text_children": "value_text",
    "no_intro_dat_game_descriptions": "description_text",
    "no_intro_dat_categories": "category",
    "no_intro_dat_identifiers": "identifier",
    "no_intro_pc_header_names": "name_text",
    "no_intro_pc_header_descriptions": "description_text",
    "no_intro_pc_header_versions": "version_text",
    "no_intro_pc_game_descriptions": "description_text",
    "no_intro_export_header_fields": "text",
}
HASH_FIELDS = {"crc", "crc32", "md5", "sha1", "sha256", "origin_sha256"}


def identifier(name):
    if not re.fullmatch(r"[a-z_][a-z_0-9]*", name):
        raise AssertionError(("not a SQL identifier", name))
    return '"' + name + '"'


def expected_routes():
    result = {}
    for family, table, path, fields, position in CONTRACT:
        for field in fields.split():
            scalar = position == "-"
            wire = (path + ("/" + field if field != "text" else "") + "/#text"
                    if scalar else path + "/@" + field)
            value_table, columns = table, field
            if scalar:
                columns = SCALAR_COLUMNS[table]
            elif field in HASH_FIELDS:
                value_table, columns = "declared_catalog_hash_text", "declared_text"
            elif field in {"nfo_crc32", "nfocrc"}:
                value_table, columns = "no_intro_release_nfo_hashes", "presence,hash_id,reported_text"
            elif field == "name" and table in {
                    "no_intro_dat_games", "no_intro_pc_games", "no_intro_export_games"}:
                value_table, columns = "catalog_sets", "set_name"
            elif table == "no_intro_dat_games":
                if field == "id":
                    columns = "publisher_id_text"
                else:
                    value_table, columns = "no_intro_dat_set_links", "target_literal"
            elif table == "no_intro_pc_games":
                columns = {"id": "archive_id", "namealt": "name_alt", "bios": "bios_text"}.get(field, field)
                if field == "languages":
                    value_table, columns = "no_intro_pc_languages", "language"
                if field == "clone":
                    value_table, columns = "no_intro_pc_declared_clone_text", "declared_text"
                if field == "mergeof":
                    value_table, columns = "no_intro_pc_merge_links", "target_archive_id"
            elif table == "no_intro_archive_descriptions" and field in {"clone", "mergeof"}:
                value_table, columns = (("no_intro_archive_declared_clone_text", "declared_text")
                                        if field == "clone" else ("no_intro_archive_merge_links", "declared_mergeof"))
            elif table == "no_intro_dat_rom_claims" and field in {"status", "serial", "header", "date", "mia"}:
                columns = field + "_text"
            elif field == "size":
                columns = "size_text"
            result[(family, table, field, wire)] = (value_table, columns, position)
    header_fields = CONTRACT[0][3].split()
    simple = list(XSI_SIMPLE) + [
        ("no_intro_dat_header_text_children", "datafile/header/" + field, "header_text_child")
        for field in header_fields
    ]
    for table, path, kind in [*XSI_COMPLEX, *simple]:
        simple_owner = (table, path, kind) in simple
        value_table = "no_intro_dat_" + kind + "_xsi_attributes"
        for field in ["schemaLocation", "noNamespaceSchemaLocation", *(["type"] if simple_owner else []), "nil"]:
            columns = "value_text,source_qname" + (",resolved_type_kind" if field == "type" else "")
            result[("no_intro_dat", table, field, path + "/@" + XSI_NAMESPACE + field)] = (
                value_table, columns, value_table)
    return result


def closed_codes(connection, table):
    sql = connection.execute("SELECT sql FROM sqlite_schema WHERE name=?", (table,)).fetchone()[0]
    match = re.search(r"field_kind\s+TEXT\s+NOT\s+NULL\s+CHECK\s*\(\s*field_kind\s+(?:IN\s*\(([^)]*)\)|=\s*('(?:[^']|'')*'))", sql, re.I)
    if not match:
        raise AssertionError(("missing concrete closed TEXT field_kind CHECK", table))
    return set(re.findall(r"'([^']*)'", match.group(1) or match.group(2)))


def validate_ledger(connection):
    with (DIRECTORY / "no_intro-field-coverage.tsv").open() as file:
        reader = csv.DictReader(file, delimiter="\t")
        assert reader.fieldnames == "family owner_table field_code wire_name value_owner value_column position_table presence default_rule evidence".split()
        rows = list(reader)
    expected = expected_routes()
    found, positions = {}, {}
    for row in rows:
        key = tuple(row[name] for name in ("family", "owner_table", "field_code", "wire_name"))
        assert key not in found, ("duplicate source path", key)
        route = tuple(row[name] for name in ("value_owner", "value_column", "position_table"))
        found[key] = route
        assert expected.get(key) == route, ("wrong field identity/canonical route", key, route, expected.get(key))
        for name in ("owner_table", "value_owner", "position_table"):
            table = row[name]
            if table == "-":
                continue
            assert connection.execute("SELECT 1 FROM sqlite_schema WHERE name=? AND type IN ('table','view')", (table,)).fetchone(), (key, name, table)
        columns = {col[1] for col in connection.execute("PRAGMA table_xinfo(" + identifier(row["value_owner"]) + ")")}
        assert set(row["value_column"].split(",")) <= columns, ("nonexistent value column", key)
        assert all(row[name] for name in ("presence", "default_rule", "evidence")), ("missing field policy", key)
        if row["position_table"] != "-":
            positions.setdefault(row["position_table"], set()).add(row["field_code"])
            assert row["field_code"] in closed_codes(connection, row["position_table"]), ("noncanonical position code", key)
        else:
            assert row["wire_name"].endswith("/#text"), ("unpositioned non-text field", key)
            if row["field_code"] != "text":
                assert row["field_code"] in closed_codes(connection, row["owner_table"])
    assert found == expected, ("missing/extra accepted source paths", expected.keys() - found.keys(), found.keys() - expected.keys())
    for table, codes in positions.items():
        assert codes == closed_codes(connection, table), ("missing position code route", table, codes ^ closed_codes(connection, table))
    return rows, positions


def check_populated_coverage(connection, rows, positions):
    for table, codes in positions.items():
        actual = {row[0] for row in connection.execute("SELECT DISTINCT field_kind FROM " + identifier(table))}
        assert actual == codes, ("unexercised position code", table, codes - actual)
    # All 12 distinct header-child XSI routes must be populated, not just their
    # shared relation's four codes. No count-based substitution is sufficient.
    for row in rows:
        if row["owner_table"] == "no_intro_dat_header_text_children" and row["position_table"] != "-":
            kind = row["wire_name"].split("/@")[0].rsplit("/", 1)[1]
            assert connection.execute(
                "SELECT 1 FROM no_intro_dat_header_text_children child JOIN "
                "no_intro_dat_header_text_child_xsi_attributes attr USING(source_element_id) "
                "WHERE child.field_kind=? AND attr.field_kind=?", (kind, row["field_code"])
            ).fetchone(), ("missing populated header XSI path", row["wire_name"])


def run_fixture(connection, path, rows, positions):
    statement, expected_error = "", None
    assertions = negatives = 0
    for line_number, line in enumerate(path.read_text().splitlines(), 1):
        if line.startswith("-- expect-error: "):
            assert not statement.strip(), ("directive inside statement", path.name, line_number)
            expected_error = line.split(": ", 1)[1]
            continue
        if line == "-- CHECK-FIELD-COVERAGE":
            check_populated_coverage(connection, rows, positions)
        if not statement and (not line.strip() or line.lstrip().startswith("--")):
            continue
        statement += line + "\n"
        if not sqlite3.complete_statement(statement):
            continue
        try:
            connection.execute(statement)
        except sqlite3.IntegrityError as error:
            if expected_error is None or expected_error not in str(error):
                raise AssertionError((path.name, line_number, str(error), statement)) from error
            negatives += 1
        else:
            assert expected_error is None, ("negative accepted", path.name, line_number, statement)
            if re.match(r"INSERT INTO no_intro_(?:field_)?witness_assertions|INSERT INTO no_intro_field_assertions", statement):
                assertions += 1
        expected_error, statement = None, ""
    assert not statement.strip() and expected_error is None, ("incomplete fixture", path.name)
    assert not connection.execute("PRAGMA foreign_key_check").fetchall()
    assert not connection.in_transaction, "fixture did not release its savepoint"
    print(f"{path.name}: {assertions} concrete SQL assertions, {negatives} expected rejections")


def main():
    ddl = subprocess.run([sys.executable, str(DIRECTORY / "assemble.py"), "--emit"],
                         check=True, capture_output=True, text=True).stdout
    for filename in ("no_intro_field_witnesses.sql", "no_intro_witnesses.sql"):
        connection = sqlite3.connect(":memory:")
        connection.execute("PRAGMA foreign_keys=ON")
        connection.executescript(ddl)
        rows, positions = validate_ledger(connection)
        run_fixture(connection, DIRECTORY / filename, rows, positions)
        connection.close()
    print(f"ledger: {len(rows)} exact source paths; {sum(map(len, positions.values()))} distinct closed position table/codes")
    print("Design-only constructed evidence; no source/corpus/default/parser-capture/production approval implied.")


if __name__ == "__main__":
    main()
