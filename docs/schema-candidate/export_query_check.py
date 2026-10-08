#!/usr/bin/env python3
"""Executable DOC-24 query/access-path witness for the design-only schema.

GAME_ANCHOR and REQUESTED_MEDIA are evidence projections, not complete
validators: GAME_ANCHOR does not check a game-name position or every
intermediate source-element kind, and REQUESTED_MEDIA does not join
catalog_media_entries or validate every intermediate native kind/edition edge.
Consumers must reject incomplete or inconsistent projected ancestry; these
queries alone do not prove a complete cursor or requested-file validation.
"""

import sqlite3
import time
import unittest

import assemble


EDITION = 10300
GROUP = 10310
GAME = 13010
DUMP = 13030
RELEASE = 13040
DUMP_FILE = 13033
RELEASE_FILE = 13043
UNRELATED = 512


# These are candidate consumer queries over the composed DDL. They deliberately
# start from the requested group, game, owner, or media ID; none is an audit.
GAME_PAGE = """
SELECT game.set_id, entry.source_order, entry.set_name,
       entry.source_order - CASE WHEN document.envelope_mode='single_datafile'
                                      AND EXISTS (
                                          SELECT 1 FROM no_intro_export_header_placements AS placement
                                          WHERE placement.datafile_edition_id=document.edition_id)
                                 THEN 1 ELSE 0 END AS list_order
FROM catalog_sets AS entry
JOIN no_intro_export_games AS game ON game.set_id=entry.set_id
JOIN catalog_source_elements AS element ON element.source_element_id=game.set_id
JOIN catalog_set_groups AS parent ON parent.set_group_id=entry.set_group_id
JOIN no_intro_export_documents AS document ON document.root_set_group_id=parent.set_group_id
JOIN published_catalog_editions AS publication ON publication.edition_id=document.edition_id
WHERE entry.set_group_id=? AND element.edition_id=document.edition_id
  AND element.element_kind='no_intro_export_game'
  AND (entry.source_order,entry.set_id)>(?,?)
ORDER BY entry.source_order,entry.set_id LIMIT min(max(?,0),500)+1
"""

GAME_ANCHOR = """
SELECT entry.set_group_id, entry.source_order, entry.set_name,
       game.set_id, element.edition_id, element.element_kind,
       document.edition_id, document.root_set_group_id,
       game.extent_view, game.extent_start, game.extent_end,
       document.extent_view, document.extent_start, document.extent_end,
       (SELECT publication.edition_id
        FROM published_catalog_editions AS publication
        WHERE publication.edition_id=document.edition_id) AS publication_edition,
       CASE WHEN game.extent_start IS NULL THEN 1
            WHEN document.extent_start IS NOT NULL
             AND game.extent_view=document.extent_view
             AND game.extent_start>=document.extent_start
             AND game.extent_end<=document.extent_end THEN 1 ELSE 0 END AS extent_contained
FROM catalog_sets AS entry
LEFT JOIN no_intro_export_games AS game ON game.set_id=entry.set_id
LEFT JOIN catalog_source_elements AS element ON element.source_element_id=game.set_id
LEFT JOIN catalog_set_groups AS parent ON parent.set_group_id=entry.set_group_id
LEFT JOIN no_intro_export_documents AS document ON document.root_set_group_id=parent.set_group_id
WHERE entry.set_id=?
"""

DIRECT_CHILDREN = {
    "archive": """
        SELECT row.archive_id,row.set_id,row.source_order,element.edition_id,element.element_kind
        FROM no_intro_archive_descriptions AS row
        LEFT JOIN catalog_source_elements AS element ON element.source_element_id=row.archive_id
        WHERE row.set_id=? ORDER BY row.source_order,row.archive_id
    """,
    "dump_source": """
        SELECT row.dump_source_id,row.set_id,row.source_order,element.edition_id,element.element_kind
        FROM no_intro_dump_sources AS row
        LEFT JOIN catalog_source_elements AS element ON element.source_element_id=row.dump_source_id
        WHERE row.set_id=? ORDER BY row.source_order,row.dump_source_id
    """,
    "release": """
        SELECT row.release_id,row.set_id,row.source_order,element.edition_id,element.element_kind
        FROM no_intro_releases AS row
        LEFT JOIN catalog_source_elements AS element ON element.source_element_id=row.release_id
        WHERE row.set_id=? ORDER BY row.source_order,row.release_id
    """,
}

OWNER_CHILDREN = {
    "dump_details": "SELECT details_element_id,dump_source_id,source_order FROM no_intro_dump_details WHERE dump_source_id=? ORDER BY source_order,details_element_id",
    "dump_serials": "SELECT serials_element_id,dump_source_id,source_order FROM no_intro_dump_serials WHERE dump_source_id=? ORDER BY source_order,serials_element_id",
    "dump_files": "SELECT media_entry_id,dump_source_id,source_order FROM no_intro_dump_files WHERE dump_source_id=? ORDER BY source_order,media_entry_id",
    "release_details": "SELECT details_element_id,release_id,source_order FROM no_intro_release_details WHERE release_id=? ORDER BY source_order,details_element_id",
    "release_serials": "SELECT serials_element_id,release_id,source_order FROM no_intro_release_serials WHERE release_id=? ORDER BY source_order,serials_element_id",
    "release_files": "SELECT media_entry_id,release_id,source_order FROM no_intro_release_files WHERE release_id=? ORDER BY source_order,media_entry_id",
}

POSITION_QUERIES = {
    "archive_positions": "SELECT archive_id,field_kind,source_order,relationship_id FROM no_intro_archive_field_positions WHERE archive_id=? ORDER BY source_order",
    "dump_details_positions": "SELECT details_element_id,field_kind,source_order FROM no_intro_dump_details_field_positions WHERE details_element_id=? ORDER BY source_order",
    "dump_serials_positions": "SELECT serials_element_id,field_kind,source_order FROM no_intro_dump_serials_field_positions WHERE serials_element_id=? ORDER BY source_order",
    "release_details_positions": "SELECT details_element_id,field_kind,source_order,reported_nfo_hash_id FROM no_intro_release_details_field_positions WHERE details_element_id=? ORDER BY source_order",
    "release_serials_positions": "SELECT serials_element_id,field_kind,source_order FROM no_intro_release_serials_field_positions WHERE serials_element_id=? ORDER BY source_order",
    "dump_file_positions": "SELECT media_entry_id,field_kind,source_order,reported_hash_id FROM no_intro_dump_file_field_positions WHERE media_entry_id=? ORDER BY source_order",
    "release_file_positions": "SELECT media_entry_id,field_kind,source_order,reported_hash_id FROM no_intro_release_file_field_positions WHERE media_entry_id=? ORDER BY source_order",
}

DECLARATIONS = {
    "dump_hashes": """
        SELECT position.media_entry_id,position.field_kind,position.source_order,
               position.reported_hash_id,hash.source_hash_field,hash.presence,hash.hash_id
        FROM no_intro_dump_file_field_positions AS position
        LEFT JOIN catalog_entry_hashes AS hash ON hash.reported_hash_id=position.reported_hash_id
        WHERE position.media_entry_id=? ORDER BY position.source_order
    """,
    "release_hashes": """
        SELECT position.media_entry_id,position.field_kind,position.source_order,
               position.reported_hash_id,hash.source_hash_field,hash.presence,hash.hash_id
        FROM no_intro_release_file_field_positions AS position
        LEFT JOIN catalog_entry_hashes AS hash ON hash.reported_hash_id=position.reported_hash_id
        WHERE position.media_entry_id=? ORDER BY position.source_order
    """,
    "nfo_hashes": """
        SELECT position.details_element_id,position.field_kind,position.source_order,
               position.reported_nfo_hash_id,hash.source_hash_field,hash.presence,hash.hash_id
        FROM no_intro_release_details_field_positions AS position
        LEFT JOIN no_intro_release_nfo_hashes AS hash
          ON hash.reported_nfo_hash_id=position.reported_nfo_hash_id
        WHERE position.details_element_id=? ORDER BY position.source_order
    """,
}

REQUESTED_MEDIA = """
WITH requested(media_entry_id) AS (VALUES (?)),
routes AS (
    SELECT requested.media_entry_id,'dump' AS route,file.dump_source_id AS parent_id,
           parent.set_id,entry.set_group_id,group_row.edition_id,document.edition_id AS document_edition,
           element.edition_id AS file_edition,element.element_kind
    FROM requested
    LEFT JOIN no_intro_dump_files AS file ON file.media_entry_id=requested.media_entry_id
    LEFT JOIN no_intro_dump_sources AS parent ON parent.dump_source_id=file.dump_source_id
    LEFT JOIN no_intro_export_games AS game ON game.set_id=parent.set_id
    LEFT JOIN catalog_sets AS entry ON entry.set_id=game.set_id
    LEFT JOIN catalog_set_groups AS group_row ON group_row.set_group_id=entry.set_group_id
    LEFT JOIN no_intro_export_documents AS document ON document.root_set_group_id=group_row.set_group_id
    LEFT JOIN catalog_source_elements AS element ON element.source_element_id=file.media_entry_id
    WHERE file.media_entry_id IS NOT NULL OR NOT EXISTS (
        SELECT 1 FROM no_intro_release_files WHERE media_entry_id=requested.media_entry_id)
    UNION ALL
    SELECT requested.media_entry_id,'release',file.release_id,
           parent.set_id,entry.set_group_id,group_row.edition_id,document.edition_id,
           element.edition_id,element.element_kind
    FROM requested
    LEFT JOIN no_intro_release_files AS file ON file.media_entry_id=requested.media_entry_id
    LEFT JOIN no_intro_releases AS parent ON parent.release_id=file.release_id
    LEFT JOIN no_intro_export_games AS game ON game.set_id=parent.set_id
    LEFT JOIN catalog_sets AS entry ON entry.set_id=game.set_id
    LEFT JOIN catalog_set_groups AS group_row ON group_row.set_group_id=entry.set_group_id
    LEFT JOIN no_intro_export_documents AS document ON document.root_set_group_id=group_row.set_group_id
    LEFT JOIN catalog_source_elements AS element ON element.source_element_id=file.media_entry_id
    WHERE file.media_entry_id IS NOT NULL OR NOT EXISTS (
        SELECT 1 FROM no_intro_dump_files WHERE media_entry_id=requested.media_entry_id)
)
SELECT media_entry_id,route,parent_id,set_id,set_group_id,edition_id,document_edition,
       file_edition,element_kind FROM routes ORDER BY route
"""

FLATTENED_FILES = """
WITH files AS (
    SELECT parent.source_order AS parent_order,file.source_order AS local_order,
           file.media_entry_id,'dump' AS route,parent.dump_source_id AS parent_id
    FROM no_intro_dump_sources AS parent
    JOIN no_intro_dump_files AS file ON file.dump_source_id=parent.dump_source_id
    WHERE parent.set_id=?
    UNION ALL
    SELECT parent.source_order,file.source_order,file.media_entry_id,
           'release',parent.release_id
    FROM no_intro_releases AS parent
    JOIN no_intro_release_files AS file ON file.release_id=parent.release_id
    WHERE parent.set_id=?
)
SELECT media_entry_id,route,parent_id,
       row_number() OVER (ORDER BY parent_order,local_order,media_entry_id)-1 AS occurrence_order
FROM files ORDER BY parent_order,local_order,media_entry_id
"""


def load_fixture(db):
    fixture = (assemble.ROOT / "no_intro_field_witnesses.sql").read_text()
    db.executescript(fixture.split("\n-- expect-error:", 1)[0])
    db.execute("RELEASE no_intro_fields")


def drop_triggers_for_test_copy(db):
    # This is a dedicated in-memory corruption copy. DDL is committed before
    # any corruption savepoint; the guarded positive fixture uses another DB.
    tables = (
        "catalog_sets", "catalog_source_elements", "no_intro_export_documents",
        "no_intro_export_games", "no_intro_export_header_placements",
        "no_intro_dump_sources", "no_intro_dump_files", "no_intro_releases",
        "no_intro_release_files",
    )
    placeholders = ",".join("?" for _ in tables)
    names = [row[0] for row in db.execute(
        f"SELECT name FROM sqlite_schema WHERE type='trigger' AND tbl_name IN ({placeholders})",
        tables).fetchall()]
    db.execute("BEGIN")
    try:
        for name in names:
            db.execute(f"DROP TRIGGER {assemble.identifier(name)}")
        db.commit()
    except BaseException:
        db.rollback()
        raise
    return len(names)


def add_unrelated_owners(db, count):
    for offset in range(count):
        edition = 20000 + offset
        group = 30000 + offset
        game = 40000 + offset
        source_file = 50000 + offset
        dump_source = 60000 + offset
        media_entry = 70000 + offset
        release = 80000 + offset
        release_media = 90000 + offset
        db.execute("INSERT INTO catalog_source_files(source_file_id,sha256,sha1,byte_length,object_key,codec) VALUES(?,?,NULL,0,?,'zstd')",
                   (source_file, offset.to_bytes(4, "big") + bytes(28), f"plan/{source_file}"))
        db.execute("INSERT INTO catalog_editions(edition_id,catalog_id,source_file_id,reading_rules_id,coverage_id) VALUES(?,?,?,?,10000)",
                   (edition, 10003, source_file, 10300))
        db.execute("INSERT INTO catalog_set_groups VALUES(?,?,'root')", (group, edition))
        db.execute("INSERT INTO no_intro_export_documents(edition_id,root_set_group_id,envelope_mode,header_present) VALUES(?,?,'single_datafile',0)",
                   (edition, group))
        db.execute("INSERT INTO no_intro_export_datafiles(edition_id,location_view,start_line,start_column,column_convention) VALUES(?,'transport_decoded_xml_text',1,1,'one_based_unicode_scalar')",
                   (edition,))
        db.execute("INSERT INTO catalog_source_elements VALUES(?,?, 'no_intro_export_game')", (game, edition))
        db.execute("INSERT INTO catalog_sets VALUES(?,?,?,0,1,1)", (game, group, f"unrelated-{offset}"))
        db.execute("INSERT INTO no_intro_export_games(set_id) VALUES(?)", (game,))
        db.execute("INSERT INTO catalog_source_elements VALUES(?,?, 'no_intro_export_dump_source')",
                   (dump_source, edition))
        db.execute("INSERT INTO no_intro_dump_sources(dump_source_id,set_id,source_order,source_line,source_column,source_end_line,source_end_column) VALUES(?,?,0,1,1,1,1)",
                   (dump_source, game))
        db.execute("INSERT INTO catalog_source_elements VALUES(?,?, 'no_intro_export_source_file')",
                   (media_entry, edition))
        db.execute("INSERT INTO catalog_media_entries(media_entry_id,file_uuid) VALUES(?,NULL)",
                   (media_entry,))
        db.execute("INSERT INTO no_intro_dump_files(media_entry_id,dump_source_id,source_order,source_line,source_column,source_end_line,source_end_column) VALUES(?,?,0,1,1,1,1)",
                   (media_entry, dump_source))
        db.execute("INSERT INTO catalog_source_elements VALUES(?,?, 'no_intro_export_release')",
                   (release, edition))
        db.execute("INSERT INTO no_intro_releases(release_id,set_id,source_order,source_line,source_column,source_end_line,source_end_column) VALUES(?,?,1,1,1,1,1)",
                   (release, game))
        db.execute("INSERT INTO catalog_source_elements VALUES(?,?, 'no_intro_export_release_file')",
                   (release_media, edition))
        db.execute("INSERT INTO catalog_media_entries(media_entry_id,file_uuid) VALUES(?,NULL)",
                   (release_media,))
        db.execute("INSERT INTO no_intro_release_files(media_entry_id,release_id,source_order,source_line,source_column,source_end_line,source_end_column) VALUES(?,?,0,1,1,1,1)",
                   (release_media, release))
    db.commit()


def vm_steps(db, sql, args):
    steps = 0

    def tick():
        nonlocal steps
        steps += 1
        return 0

    db.set_progress_handler(tick, 1)
    try:
        db.execute(sql, args).fetchall()
    finally:
        db.set_progress_handler(None, 0)
    return steps


def plan_details(db, sql, args):
    return [row[3] for row in db.execute("EXPLAIN QUERY PLAN " + sql, args)]


class ExportQueryWitness(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        started = time.perf_counter()
        cls.stage_seconds = {}
        stage = time.perf_counter()
        ddl = assemble.assemble()
        cls.stage_seconds["canonical DDL assembly"] = time.perf_counter() - stage

        cls.guarded = sqlite3.connect(":memory:")
        cls.addClassCleanup(cls.guarded.close)
        cls.guarded.execute("PRAGMA foreign_keys=ON")
        stage = time.perf_counter()
        cls.guarded.executescript(ddl)
        cls.stage_seconds["canonical schema execution"] = time.perf_counter() - stage
        stage = time.perf_counter()
        load_fixture(cls.guarded)
        cls.stage_seconds["single guarded fixture construction"] = time.perf_counter() - stage
        stage = time.perf_counter()
        cls.guarded.execute("INSERT INTO published_catalog_editions VALUES(10300,10003,10300,10300,10000,'candidate')")
        cls.guarded.commit()
        cls.stage_seconds["guarded fixture publication"] = time.perf_counter() - stage
        stage = time.perf_counter()
        cls.fixture_positive = cls.guarded.execute(
            "SELECT count(*) FROM no_intro_field_assertions WHERE passed=1").fetchone()[0]
        cls.stage_seconds["positive fixture read"] = time.perf_counter() - stage

        cls.db = sqlite3.connect(":memory:")
        cls.addClassCleanup(cls.db.close)
        cls.db.execute("PRAGMA foreign_keys=ON")
        stage = time.perf_counter()
        cls.guarded.backup(cls.db)
        cls.stage_seconds["populated-copy backup"] = time.perf_counter() - stage
        if cls.db.execute("PRAGMA foreign_keys").fetchone() != (1,):
            raise AssertionError("foreign-key enforcement was not retained on populated copy")
        stage = time.perf_counter()
        cls.db.execute("ANALYZE")
        cls.stage_seconds["initial ANALYZE"] = time.perf_counter() - stage
        stage = time.perf_counter()
        cls.before_steps = vm_steps(cls.db, GAME_PAGE, (GROUP, -1, -1, 10))
        cls.stage_seconds["pre-population VM measurement"] = time.perf_counter() - stage
        stage = time.perf_counter()
        add_unrelated_owners(cls.db, UNRELATED)
        cls.stage_seconds["512-owner population with FK and triggers on"] = time.perf_counter() - stage
        stage = time.perf_counter()
        cls.db.execute("ANALYZE")
        cls.stage_seconds["populated ANALYZE"] = time.perf_counter() - stage
        stage = time.perf_counter()
        cls.after_steps = vm_steps(cls.db, GAME_PAGE, (GROUP, -1, -1, 10))
        cls.stage_seconds["post-population VM measurement"] = time.perf_counter() - stage

        cls.corrupt_db = sqlite3.connect(":memory:")
        cls.addClassCleanup(cls.corrupt_db.close)
        cls.corrupt_db.execute("PRAGMA foreign_keys=OFF")
        stage = time.perf_counter()
        cls.db.backup(cls.corrupt_db)
        cls.stage_seconds["corruption-copy backup"] = time.perf_counter() - stage
        stage = time.perf_counter()
        cls.dropped_trigger_count = drop_triggers_for_test_copy(cls.corrupt_db)
        cls.stage_seconds["targeted corruption trigger DDL (one transaction/commit)"] = time.perf_counter() - stage
        cls.stage_seconds["class setup total"] = time.perf_counter() - started
        cls.plan_evidence = []
        cls.test_seconds = {}

    @classmethod
    def tearDownClass(cls):
        print(f"\nPLAN EVIDENCE (ANALYZE, {UNRELATED} unrelated native games/sources/files):")
        for label, lines in cls.plan_evidence:
            print(f"  {label}: {' | '.join(lines)}")
        print(f"  game-page VM steps: {cls.before_steps} before population, {cls.after_steps} after")
        print(f"  corruption-copy triggers dropped: {cls.dropped_trigger_count}")
        print("  setup stages: " + ", ".join(
            f"{label}={seconds:.3f}s" for label, seconds in cls.stage_seconds.items()))
        print("  test stages: " + ", ".join(
            f"{label}={seconds:.4f}s" for label, seconds in cls.test_seconds.items()))

    def setUp(self):
        self._test_started = time.perf_counter()
        self.db = (self.corrupt_db if self._testMethodName in {
            "test_anchor_evidence_exposes_missing_owner_and_reparented_game",
            "test_cursor_anchor_checks_document_contained_extent",
            "test_nested_header_is_removed_from_derived_game_rank",
            "test_game_page_caps_at_500_plus_one_and_continues_after_visible_row",
            "test_requested_media_projection_exposes_ancestry_evidence",
        } else self.__class__.db)
        self.db.execute("SAVEPOINT export_query_case")

    def tearDown(self):
        self.db.execute("ROLLBACK TO export_query_case")
        self.db.execute("RELEASE export_query_case")
        self.test_seconds[self._testMethodName] = time.perf_counter() - self._test_started

    def assert_owner_search(self, label, sql, args, expected_table):
        details = plan_details(self.db, sql, args)
        scans = [line for line in details if "SCAN " in line.upper()
                 and "SCAN (SUBQUERY" not in line.upper()]
        self.assertFalse(scans, f"{label} unexpectedly scans: {details}")
        indexes = {row[1] for row in self.db.execute(
            f"PRAGMA index_list({assemble.identifier(expected_table)})")}
        table_sql = self.db.execute(
            "SELECT sql FROM sqlite_schema WHERE type='table' AND name=?",
            (expected_table,)).fetchone()[0]
        columns = self.db.execute(
            f"PRAGMA table_info({assemble.identifier(expected_table)})").fetchall()
        integer_primary_key = any(row[2].upper() == "INTEGER" and row[5] for row in columns)
        without_rowid_primary_key = "WITHOUT ROWID" in table_sql.upper()
        expected_search = any(
            any(f"USING {kind} {index}" in line for kind in ("INDEX", "COVERING INDEX"))
            for line in details for index in indexes)
        expected_search |= integer_primary_key and any(
            "USING INTEGER PRIMARY KEY" in line for line in details)
        expected_search |= without_rowid_primary_key and any(
            "USING PRIMARY KEY" in line for line in details)
        self.assertTrue(expected_search,
                        f"{label} lacks an indexed SEARCH of {expected_table}: {details}")
        self.plan_evidence.append((label, [line for line in details
                                           if "SEARCH " in line.upper() or "SCAN " in line.upper()]))
        return details

    def test_guarded_wellformed_fixture_retains_positive_field_evidence(self):
        self.assertGreater(self.fixture_positive, 100)
        self.assertEqual(self.guarded.execute(
            "SELECT count(*) FROM no_intro_export_games WHERE set_id=?", (GAME,)).fetchone(), (1,))
        self.assertEqual(self.guarded.execute(
            "SELECT count(*) FROM no_intro_dump_files WHERE media_entry_id=?", (DUMP_FILE,)).fetchone(), (1,))
        self.assertEqual(self.guarded.execute(
            "SELECT count(*) FROM no_intro_release_files WHERE media_entry_id=?", (RELEASE_FILE,)).fetchone(), (1,))

    def test_game_keyset_and_anchor_evidence_facts(self):
        page = self.db.execute(GAME_PAGE, (GROUP, -1, -1, 10)).fetchall()
        self.assertEqual(page, [(GAME, 0, "", 0)])
        anchor = self.db.execute(GAME_ANCHOR, (GAME,)).fetchone()
        self.assertEqual(anchor[:8], (GROUP, 0, "", GAME, EDITION, "no_intro_export_game", EDITION, GROUP))
        self.assertIsNone(anchor[8])
        self.assertEqual((anchor[14], anchor[15]), (EDITION, 1))
        self.assertEqual(self.db.execute(GAME_PAGE, (GROUP, 0, GAME, 10)).fetchall(), [])
        plan = self.assert_owner_search("root game keyset", GAME_PAGE, (GROUP, -1, -1, 10), "catalog_sets")
        self.assertTrue(any("SEARCH game USING PRIMARY KEY" in line for line in plan), plan)
        self.assert_owner_search("cursor anchor evidence", GAME_ANCHOR, (GAME,), "catalog_sets")

    def test_cursor_anchor_checks_document_contained_extent(self):
        self.db.execute("UPDATE no_intro_export_documents SET extent_view='transport_decoded_xml_bytes',extent_start=100,extent_end=200 WHERE edition_id=?",
                        (EDITION,))
        self.db.execute("UPDATE no_intro_export_games SET extent_view='transport_decoded_xml_bytes',extent_start=110,extent_end=190 WHERE set_id=?",
                        (GAME,))
        anchor = self.db.execute(GAME_ANCHOR, (GAME,)).fetchone()
        self.assertEqual((anchor[14], anchor[15]), (EDITION, 1))
        self.db.execute("UPDATE no_intro_export_games SET extent_start=99 WHERE set_id=?", (GAME,))
        anchor = self.db.execute(GAME_ANCHOR, (GAME,)).fetchone()
        self.assertEqual(anchor[15], 0)

    def test_nested_header_is_removed_from_derived_game_rank(self):
        self.db.execute("UPDATE no_intro_export_documents SET envelope_mode='single_datafile' WHERE edition_id=?",
                        (EDITION,))
        self.db.execute("UPDATE catalog_sets SET source_order=1 WHERE set_id=?", (GAME,))
        self.db.execute("INSERT INTO catalog_source_elements VALUES(?,?, 'no_intro_export_nested_header')",
                        (13100, EDITION))
        self.db.execute("INSERT INTO no_intro_export_header_placements(source_element_id,header_id,datafile_edition_id,source_order) VALUES(?,?,?,0)",
                        (13100, 13000, EDITION))
        row = self.db.execute(GAME_PAGE, (GROUP, -1, -1, 10)).fetchone()
        self.assertEqual(row, (GAME, 1, "", 0))

    def test_game_page_caps_at_500_plus_one_and_continues_after_visible_row(self):
        for offset in range(501):
            set_id = 110000 + offset
            self.db.execute("INSERT INTO catalog_source_elements VALUES(?,?, 'no_intro_export_game')",
                            (set_id, EDITION))
            self.db.execute("INSERT INTO catalog_sets VALUES(?,?,?, ?,1,1)",
                            (set_id, GROUP, f"page-{offset}", offset + 1))
            self.db.execute("INSERT INTO no_intro_export_games(set_id) VALUES(?)", (set_id,))
        first = self.db.execute(GAME_PAGE, (GROUP, -1, -1, 900)).fetchall()
        self.assertEqual(len(first), 501)
        visible, lookahead = first[:500], first[500]
        self.assertEqual((len(visible), visible[0][0], visible[-1][1]), (500, GAME, 499))
        self.assertEqual(lookahead[1], 500)
        continuation = self.db.execute(GAME_PAGE,
                                       (GROUP, visible[-1][1], visible[-1][0], 900)).fetchall()
        self.assertEqual(continuation[0], lookahead)
        self.assertEqual([row[1] for row in continuation], [500, 501])

    def test_anchor_evidence_exposes_missing_owner_and_reparented_game(self):
        self.db.execute("SAVEPOINT anchor_mutation")
        self.db.execute("DELETE FROM no_intro_export_games WHERE set_id=?", (GAME,))
        anchor = self.db.execute(GAME_ANCHOR, (GAME,)).fetchone()
        self.assertIsNone(anchor[3])
        self.db.execute("ROLLBACK TO anchor_mutation")
        self.db.execute("RELEASE anchor_mutation")
        other_group = self.db.execute("SELECT set_group_id FROM catalog_set_groups WHERE edition_id=20000").fetchone()[0]
        self.db.execute("UPDATE catalog_sets SET set_group_id=?,source_order=1 WHERE set_id=?", (other_group, GAME))
        anchor = self.db.execute(GAME_ANCHOR, (GAME,)).fetchone()
        self.assertEqual(anchor[0], other_group)
        self.assertNotEqual(anchor[6], EDITION)

    def test_selected_game_direct_and_nested_owner_routes(self):
        expected = {"archive": (13020, 13010, 0, EDITION, "no_intro_export_archive"),
                    "dump_source": (DUMP, GAME, 2, EDITION, "no_intro_export_dump_source"),
                    "release": (RELEASE, GAME, 3, EDITION, "no_intro_export_release")}
        for branch, sql in DIRECT_CHILDREN.items():
            with self.subTest(branch=branch):
                rows = self.db.execute(sql, (GAME,)).fetchall()
                self.assertTrue(rows)
                self.assertEqual(rows[0], expected[branch])
                table = {"archive": "no_intro_archive_descriptions", "dump_source": "no_intro_dump_sources",
                         "release": "no_intro_releases"}[branch]
                self.assert_owner_search(branch, sql, (GAME,), table)
        nested = (("dump_details", DUMP), ("dump_serials", DUMP), ("dump_files", DUMP),
                  ("release_details", RELEASE), ("release_serials", RELEASE), ("release_files", RELEASE))
        for branch, parent in nested:
            with self.subTest(branch=branch):
                rows = self.db.execute(OWNER_CHILDREN[branch], (parent,)).fetchall()
                self.assertEqual(len(rows), 1)
                table = OWNER_CHILDREN[branch].split(" FROM ", 1)[1].split()[0]
                self.assert_owner_search(branch, OWNER_CHILDREN[branch], (parent,), table)

    def test_positions_hashes_and_nfo_follow_actual_owner_keys(self):
        owners = {"archive_positions": 13021, "dump_details_positions": 13031,
                  "dump_serials_positions": 13032, "release_details_positions": 13041,
                  "release_serials_positions": 13042, "dump_file_positions": DUMP_FILE,
                  "release_file_positions": RELEASE_FILE}
        for name, sql in POSITION_QUERIES.items():
            with self.subTest(position=name):
                rows = self.db.execute(sql, (owners[name],)).fetchall()
                self.assertTrue(rows)
                table = sql.split(" FROM ", 1)[1].split()[0]
                self.assert_owner_search(name, sql, (owners[name],), table)
        for name, media in (("dump_hashes", DUMP_FILE), ("release_hashes", RELEASE_FILE)):
            rows = self.db.execute(DECLARATIONS[name], (media,)).fetchall()
            self.assertTrue(any(row[4] is not None for row in rows), name)
            self.assert_owner_search(name, DECLARATIONS[name], (media,), "no_intro_dump_file_field_positions" if name == "dump_hashes" else "no_intro_release_file_field_positions")
        nfo = self.db.execute(DECLARATIONS["nfo_hashes"], (13041,)).fetchall()
        self.assertEqual({row[4] for row in nfo if row[4]}, {"nfo_crc32", "nfocrc"})
        self.assert_owner_search("release NFO hashes", DECLARATIONS["nfo_hashes"], (13041,), "no_intro_release_details_field_positions")

    def test_requested_media_projection_exposes_ancestry_evidence(self):
        for media, route, parent, game in ((DUMP_FILE, "dump", DUMP, GAME),
                                           (RELEASE_FILE, "release", RELEASE, GAME)):
            rows = self.db.execute(REQUESTED_MEDIA, (media,)).fetchall()
            selected = [row for row in rows if row[1] == route]
            self.assertEqual(len(selected), 1, rows)
            row = selected[0]
            self.assertEqual((row[2], row[3], row[4], row[5], row[6], row[7]),
                             (parent, game, GROUP, EDITION, EDITION, EDITION))
            self.assertEqual(row[8], "no_intro_export_source_file" if route == "dump"
                             else "no_intro_export_release_file")
        details = plan_details(self.db, REQUESTED_MEDIA, (DUMP_FILE,))
        self.plan_evidence.append(("requested dump-file ancestry", [line for line in details
                                    if "SEARCH " in line.upper() or "SCAN " in line.upper()]))
        self.assertFalse([line for line in details if "SCAN FILE " in line.upper()
                          or "SCAN PARENT " in line.upper()], details)
        self.assertTrue(any("SEARCH file USING INTEGER PRIMARY KEY" in line for line in details), details)
        self.assertTrue(any("SEARCH parent USING INTEGER PRIMARY KEY" in line for line in details), details)
        release_details = plan_details(self.db, REQUESTED_MEDIA, (RELEASE_FILE,))
        self.plan_evidence.append(("requested release-file ancestry", [line for line in release_details
                                        if "SEARCH " in line.upper() or "SCAN " in line.upper()]))
        self.assertFalse([line for line in release_details if "SCAN FILE " in line.upper()
                          or "SCAN PARENT " in line.upper()], release_details)
        self.assertTrue(any("SEARCH file USING INTEGER PRIMARY KEY" in line
                            for line in release_details), release_details)

        # A missing immediate history parent must remain visible as NULL ancestry.
        self.db.execute("DELETE FROM no_intro_dump_sources WHERE dump_source_id=?", (DUMP,))
        broken = self.db.execute(REQUESTED_MEDIA, (DUMP_FILE,)).fetchall()
        dump_row = next(row for row in broken if row[1] == "dump")
        self.assertEqual(dump_row[2], DUMP)
        self.assertIsNone(dump_row[3])
        self.assertIsNone(dump_row[5])

        other_game = 40000
        other_source = 100000
        self.db.execute("INSERT INTO catalog_source_elements VALUES(?,?, 'no_intro_export_dump_source')",
                        (other_source, 20000))
        self.db.execute("INSERT INTO no_intro_dump_sources(dump_source_id,set_id,source_order,source_line,source_column,source_end_line,source_end_column) VALUES(?,?,0,1,1,1,1)",
                        (other_source, other_game))
        self.db.execute("UPDATE no_intro_dump_files SET dump_source_id=? WHERE media_entry_id=?",
                        (other_source, DUMP_FILE))
        reparented = next(row for row in self.db.execute(REQUESTED_MEDIA, (DUMP_FILE,)).fetchall()
                          if row[1] == "dump")
        self.assertEqual((reparented[2], reparented[3], reparented[5], reparented[7]),
                         (other_source, other_game, 20000, EDITION))
        self.assertNotEqual(reparented[5], reparented[7])

        self.db.execute("DELETE FROM no_intro_releases WHERE release_id=?", (RELEASE,))
        orphaned_release = next(row for row in self.db.execute(REQUESTED_MEDIA, (RELEASE_FILE,)).fetchall()
                                if row[1] == "release")
        self.assertEqual(orphaned_release[2], RELEASE)
        self.assertIsNone(orphaned_release[3])
        self.assertIsNone(orphaned_release[5])
        unrelated_release = 80000
        self.db.execute("UPDATE no_intro_release_files SET release_id=? WHERE media_entry_id=?",
                        (unrelated_release, RELEASE_FILE))
        reparented_release = next(row for row in self.db.execute(REQUESTED_MEDIA, (RELEASE_FILE,)).fetchall()
                                  if row[1] == "release")
        self.assertEqual((reparented_release[2], reparented_release[3], reparented_release[5], reparented_release[7]),
                         (unrelated_release, 40000, 20000, EDITION))

    def test_flattened_files_follow_mixed_parent_then_local_order(self):
        rows = self.db.execute(FLATTENED_FILES, (GAME, GAME)).fetchall()
        self.assertEqual(rows, [(DUMP_FILE, "dump", DUMP, 0),
                                (RELEASE_FILE, "release", RELEASE, 1)])
        plan = plan_details(self.db, FLATTENED_FILES, (GAME, GAME))
        self.plan_evidence.append(("flattened per-game files", [line for line in plan
                                    if "SEARCH " in line.upper() or "SCAN " in line.upper()]))
        self.assertFalse([line for line in plan if "SCAN " in line.upper()
                          and "SCAN (SUBQUERY" not in line.upper()], plan)
        self.assertTrue(any("SEARCH" in line.upper() and "no_intro_dump_sources" in line for line in plan), plan)
        self.assertTrue(any("SEARCH" in line.upper() and "no_intro_releases" in line for line in plan), plan)

    def test_file_rows_do_not_copy_game_keys_or_flattened_ranks(self):
        for table in ("no_intro_dump_files", "no_intro_release_files"):
            columns = {row[1] for row in self.db.execute(f"PRAGMA table_info({table})")}
            self.assertNotIn("set_id", columns)
            self.assertNotIn("occurrence_order", columns)
            self.assertIn("source_order", columns)

    def test_populated_plans_and_vm_work_are_bounded_by_unrelated_owners(self):
        self.assertEqual(UNRELATED, 512)
        self.assertLessEqual(abs(self.after_steps - self.before_steps), 32,
                             (self.before_steps, self.after_steps))
        self.assertLessEqual(self.after_steps, self.before_steps + 32,
                             (self.before_steps, self.after_steps))
        self.assertGreater(self.before_steps, 0)
        plans = plan_details(self.db, GAME_PAGE, (GROUP, -1, -1, 10))
        self.assertFalse([line for line in plans if "SCAN " in line.upper()
                          and "SCAN (SUBQUERY" not in line.upper()], plans)
        self.assertTrue(any("SEARCH" in line.upper() and "catalog_sets" in line for line in plans), plans)


if __name__ == "__main__":
    unittest.main(verbosity=2)
