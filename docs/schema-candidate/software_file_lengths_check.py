#!/usr/bin/env python3
"""Independent witnesses for the query-only software file-length candidate."""

from pathlib import Path
import sqlite3
import sys


ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))

import assemble
import count_fixtures
from software_count_mapping_check import EXPECTED_COUNTS


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def make_db():
    db = sqlite3.connect(":memory:")
    db.executescript(assemble.assemble())
    db.executescript((ROOT / "software_field_witnesses.sql").read_text())
    count_fixtures.seal(db, "software", 1, EXPECTED_COUNTS)
    problems = db.execute("SELECT * FROM candidate_integrity_problems").fetchall()
    require(not problems, f"constructed software baseline is not integrity-clean: {problems}")
    db.commit()
    return db


def add_rom(db, media_id, area_id, order, size, flag, *, required_id=None,
            first_load_id=None, step_id="same"):
    db.execute("INSERT INTO catalog_source_elements VALUES (?,1,'software_rom_entry')", (media_id,))
    db.execute("INSERT INTO catalog_media_entries(media_entry_id) VALUES (?)", (media_id,))
    db.execute(
        """INSERT INTO software_rom_load_entries
           (media_entry_id,data_area_id,name,size_text,offset_text,value,loadflag,
            status,status_specified,source_order,source_line,source_column)
           VALUES (?,?,NULL,?,NULL,NULL,?,'good',0,?,1,1)""",
        (media_id, area_id, size, flag, order),
    )
    if required_id is not None:
        db.execute("INSERT INTO software_required_files VALUES (?,?)",
                   (required_id, first_load_id if first_load_id is not None else media_id))
    if step_id != "omit":
        linked = required_id if step_id == "same" else step_id
        db.execute("INSERT INTO software_file_load_steps VALUES (?,?)", (media_id, linked))


def assert_length(db, media_id, expected):
    actual = db.execute(
        "SELECT byte_length FROM candidate_software_file_lengths WHERE media_entry_id=?",
        (media_id,),
    ).fetchone()
    require(actual == (expected,), f"{media_id}: expected {expected!r}, got {actual!r}")


def opcode_count(db, media_id):
    count = 0

    def progress():
        nonlocal count
        count += 1
        return 0

    db.set_progress_handler(progress, 1)
    try:
        assert_length(db, media_id, 9)
    finally:
        db.set_progress_handler(None, 0)
    return count


def run(db):
    columns = tuple(row[1] for row in db.execute(
        "PRAGMA table_info(candidate_software_file_lengths)"))
    require(columns == ("media_entry_id", "byte_length"), f"unexpected view columns: {columns}")
    require(db.execute(
        "SELECT type FROM sqlite_schema WHERE name='candidate_software_file_lengths'"
    ).fetchone() == ("view",), "length derivation is not query-only")
    native_columns = {row[1] for row in db.execute(
        "PRAGMA table_xinfo(software_rom_load_entries)")}
    require("byte_length" not in native_columns, "computed length was persisted on native owner")

    # One complete ordinary-load run: MAME octal base + hexadecimal continue
    # + zero ignore. Reload and its continuation are excluded; the next load
    # and fill each close their preceding run.
    add_rom(db, 9000, 300, 3, "010", "load16_byte", required_id=19000)
    add_rom(db, 9001, 300, 4, "0x2", "continue", step_id=19000)
    add_rom(db, 9002, 300, 5, "0", "ignore", step_id=19000)
    add_rom(db, 9003, 300, 6, "7", "reload", step_id=19000)
    add_rom(db, 9004, 300, 7, "8", "continue", step_id=19000)
    add_rom(db, 9005, 300, 8, "5", "load32_dword", required_id=19005)
    add_rom(db, 9006, 300, 9, "999", "fill", step_id=None)

    # Base and continue are positive; ignore may be zero. Invalid declarations
    # remain NULL rather than becoming a numeric prefix or a partial sum.
    add_rom(db, 9007, 300, 10, "0", "load16_byte", required_id=19007)
    add_rom(db, 9008, 300, 11, "", "fill", step_id=None)
    add_rom(db, 9009, 300, 12, "1", "load16_byte", required_id=19009)
    add_rom(db, 9010, 300, 13, "0", "continue", step_id=19009)
    add_rom(db, 9011, 300, 14, "1", "load16_byte", required_id=19011)
    add_rom(db, 9012, 300, 15, "+1", "continue", step_id=19011)
    add_rom(db, 9013, 300, 16, "9223372036854775808", "load16_byte", required_id=19013)
    add_rom(db, 9014, 300, 17, "2", "continue", step_id=19013)
    add_rom(db, 9015, 300, 18, "1", "load16_byte", required_id=19015)
    add_rom(db, 9016, 300, 19, "2", "continue", step_id="omit")

    # Wrong requirement owner, wrong-area use of a required_file_id, malformed
    # source-order adjacency, and a positive valid terminal root at area end.
    add_rom(db, 9017, 300, 20, "1", "load16_byte", required_id=19017)
    add_rom(db, 9018, 300, 21, "2", "continue", step_id=19015)
    add_rom(db, 9019, 300, 22, "3", "load16_byte", required_id=19019)
    add_rom(db, 9020, 301, 1, "1", "continue", step_id=19019)
    add_rom(db, 9021, 300, 23, "4", "fill", step_id=None)
    add_rom(db, 9022, 300, 24, "1", "load16_byte", required_id=19022)
    add_rom(db, 9023, 300, 26, "2", "continue", step_id=19022)
    add_rom(db, 9024, 300, 27, "3", "load16_byte", required_id=19024)
    add_rom(db, 9025, 300, 28, "", "fill", step_id=None)
    add_rom(db, 9027, 300, 30, "1", "load16_byte", required_id=19027)
    add_rom(db, 9026, 300, 29, "1", "load16_byte", required_id=19026,
            first_load_id=9020)
    add_rom(db, 9028, 300, 31, "9", "load16_byte", required_id=19028)

    # Explicitly test reload_plain and NODUMP separation from length math.
    add_rom(db, 9030, 300, 32, "2", "load16_byte", required_id=19030)
    add_rom(db, 9031, 300, 33, "99", "reload_plain", step_id=19030)
    add_rom(db, 9032, 300, 34, "3", "load16_byte", required_id=19032)
    db.execute("UPDATE software_rom_load_entries SET status='nodump',status_specified=1 WHERE media_entry_id=9032")
    add_rom(db, 9033, 300, 35, "", "fill", step_id=None)
    add_rom(db, 9034, 300, 36, "9", "load16_byte", required_id=19034)
    add_rom(db, 9035, 300, 37, "9223372036854775807", "load16_byte", required_id=19035)
    add_rom(db, 9036, 300, 38, "1", "ignore", step_id=19035)
    add_rom(db, 9037, 300, 39, "1", "load16_byte", required_id=19037)
    add_rom(db, 9038, 300, 40, "9223372036854775807", "load16_byte", required_id=19038)
    add_rom(db, 9039, 300, 41, "0", "ignore", step_id=19038)
    add_rom(db, 9041, 301, 2, "1", "load16_byte", required_id=19041, step_id="omit")
    add_rom(db, 9042, 301, 3, "1", "load16_byte", step_id="omit")
    add_rom(db, 9040, 300, 42, "9", "load16_byte", required_id=19040)

    assert_length(db, 9000, 10)
    assert_length(db, 400, None)        # retained malformed sign is unknown
    assert_length(db, 402, 8)           # the native number view parses octal
    assert_length(db, 9005, 5)
    assert_length(db, 9007, None)       # zero base
    assert_length(db, 9009, None)       # zero continue
    assert_length(db, 9011, None)       # signed continuation
    assert_length(db, 9013, None)       # i64 overflow
    assert_length(db, 9015, None)       # broken operation owner chain
    assert_length(db, 9017, None)       # continuation linked to another file
    assert_length(db, 9019, None)       # cross-area required_file_id
    assert_length(db, 9022, None)       # gapped source order
    assert_length(db, 9024, 3)          # fill closes run
    assert_length(db, 9026, None)       # first_load_entry_id points elsewhere
    assert_length(db, 9027, 1)          # a new root starts after prior boundary
    assert_length(db, 9028, 9)          # valid run ends at the next load
    assert_length(db, 9030, 2)          # reload_plain closes first run
    assert_length(db, 9032, 3)          # NODUMP is not length qualification
    assert_length(db, 9034, 9)          # run ends at the next load
    assert_length(db, 9035, None)       # individually valid values sum past i64
    assert_length(db, 9038, 9223372036854775807)  # maximum plus zero is valid
    assert_length(db, 9041, None)       # base root has no load-step row
    assert_length(db, 9042, None)       # base root has no required-file owner
    assert_length(db, 9040, 9)          # actual final root after the fill
    for operation_id in (9001, 9002, 9003, 9004, 9006, 9031, 9033, 9036, 9039):
        require(db.execute(
            "SELECT 1 FROM candidate_software_file_lengths WHERE media_entry_id=?",
            (operation_id,),
        ).fetchone() is None, f"non-load operation {operation_id} leaked into the view")

    plan = [row[3] for row in db.execute(
        "EXPLAIN QUERY PLAN SELECT byte_length FROM candidate_software_file_lengths WHERE media_entry_id=9028")]
    text = "\n".join(plan)
    require(any("SEARCH root USING INTEGER PRIMARY KEY" in row for row in plan),
            f"point lookup does not constrain outer root:\n{text}")
    require(any("sqlite_autoindex_software_rom_load_entries_1 (data_area_id=? AND source_order>?)" in row
                for row in plan), f"successor lookup is not indexed:\n{text}")
    require(any("candidate_software_file_steps_by_required_file (required_file_id=?)" in row
                for row in plan), f"required-file membership is not indexed:\n{text}")

    # Warm SQLite's statement/cache path before comparing VM instruction work.
    opcode_count(db, 9028)
    before = opcode_count(db, 9028)
    # Add 128 unrelated native base-load/fill pairs in a different data area.
    for offset in range(128):
        base = 20000 + offset * 2
        req = 40000 + offset
        order = 4 + offset * 2
        add_rom(db, base, 301, order, "3", "load16_byte", required_id=req)
        add_rom(db, base + 1, 301, order + 1, "", "fill", step_id=None)
    after = opcode_count(db, 9028)
    require(after <= before,
            f"unrelated native chains increased point-query VM work: {before} -> {after} opcodes")
    print(f"software file lengths passed (point query SQLite VM opcodes: {before} -> {after} after 128 unrelated chains)")


def main():
    db = make_db()
    try:
        run(db)
    finally:
        db.close()


if __name__ == "__main__":
    main()
