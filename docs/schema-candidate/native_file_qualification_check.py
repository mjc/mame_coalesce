#!/usr/bin/env python3
"""Composed in-memory witnesses for native file byte qualification."""

from contextlib import closing
from pathlib import Path
import sqlite3
import sys
import time
import unittest

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))
import assemble
import count_fixtures
import mame_presence_check
import logiqx_cmp_presence_check
import no_intro_presence_check
import software_presence_check


def composed_db(add_cleanup=None):
    db = sqlite3.connect(":memory:")
    if add_cleanup is not None:
        add_cleanup(db.close)
    try:
        db.execute("PRAGMA foreign_keys=ON")
        db.executescript(assemble.assemble())
    except BaseException:
        db.close()
        raise
    return db


def add_hash(db, *, media_id, reported_id, hash_id, field, algorithm, scope,
             bytes_value, position_sql, position_values):
    db.execute("INSERT INTO hash_values VALUES(?,?,?)",
               (hash_id, algorithm, bytes_value))
    db.execute(
        "INSERT INTO catalog_entry_hashes "
        "(reported_hash_id,media_entry_id,source_hash_field,field_occurrence,"
        "presence,hash_scope,hash_id) VALUES(?,?,?,0,'value',?,?)",
        (reported_id, media_id, field, scope, hash_id),
    )
    db.execute(position_sql, position_values)


class NativeFileQualificationChecks(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.dat_db = composed_db(cls.addClassCleanup)
        no_intro_presence_check.populate_fixture(
            cls.dat_db,
            file_byte_contracts=((10100, "no_intro_dat_unfiltered_file"),
                                 (10400, "no_intro_dat_unfiltered_file"),
                                 (10200, "no_intro_pc_fixture_asset")),
        )
        cls.dat_db.execute("UPDATE no_intro_pc_file_claims SET size_text='0' WHERE media_entry_id=12012")
        cls.dat_db.execute("INSERT INTO catalog_source_elements VALUES(15000,10400,'no_intro_dat_game')")
        cls.dat_db.execute("INSERT INTO catalog_source_elements VALUES(15001,10400,'no_intro_dat_rom')")
        cls.dat_db.execute(
            "INSERT INTO catalog_sets(set_id,set_group_id,set_name,source_order,source_line,source_column) "
            "VALUES(15000,10410,'flat-dat-positive',1,11,1)"
        )
        cls.dat_db.execute("INSERT INTO no_intro_dat_games VALUES(15000,NULL)")
        cls.dat_db.execute("INSERT INTO catalog_media_entries(media_entry_id) VALUES(15001)")
        cls.dat_db.execute(
            "INSERT INTO no_intro_dat_rom_claims "
            "(media_entry_id,set_id,name,size_text,source_order,source_line,source_column) "
            "VALUES(15001,15000,'flat.dat.rom','0',0,11,1)"
        )
        cls.dat_db.execute("INSERT INTO hash_values VALUES(804,'sha1',?)", (bytes.fromhex("84" * 20),))
        cls.dat_db.execute(
            "INSERT INTO catalog_entry_hashes VALUES(904,15001,'sha1',0,'value','whole_file',804,NULL)"
        )
        cls.dat_db.execute(
            "INSERT INTO no_intro_dat_rom_field_positions "
            "(media_entry_id,field_kind,field_occurrence,source_order,source_line,source_column,reported_hash_id) "
            "VALUES(15001,'name',0,0,11,10,NULL)"
        )
        cls.dat_db.execute(
            "INSERT INTO no_intro_dat_rom_field_positions "
            "(media_entry_id,field_kind,field_occurrence,source_order,source_line,source_column,reported_hash_id) "
            "VALUES(15001,'sha1',0,1,11,30,904)"
        )

        cls.mame_db = composed_db(cls.addClassCleanup)
        mame_presence_check.seed(
            cls.mame_db,
            file_byte_contracts=((1, "mame_0289_machine_rom"),),
        )
        cls.mame_db.execute("INSERT INTO catalog_source_elements VALUES(237,1,'mame_rom')")
        cls.mame_db.execute("INSERT INTO catalog_media_entries(media_entry_id) VALUES(237)")
        cls.mame_db.execute(
            "INSERT INTO mame_roms(media_entry_id,machine_id,name,size_text,status,status_specified,"
            "optional,optional_specified,source_order,source_line,source_column) "
            "VALUES(237,100,'qualified.rom','0','good',0,0,0,100,47,3)"
        )
        cls.mame_db.execute(
            "INSERT INTO mame_rom_claims_attribute_positions "
            "(media_entry_id,field_kind,field_occurrence,source_order,source_line,source_column) "
            "VALUES(237,'name',0,0,47,10)"
        )
        add_hash(
            cls.mame_db, media_id=237, reported_id=9237, hash_id=9237,
            field="sha1", algorithm="sha1", scope="whole_file",
            bytes_value=bytes.fromhex("23" * 20),
            position_sql=("INSERT INTO mame_rom_claims_attribute_positions "
                          "(media_entry_id,field_kind,field_occurrence,reported_hash_id,source_order,source_line,source_column) "
                          "VALUES(237,'sha1',0,9237,1,47,30)"),
            position_values=(),
        )
        cls.mame_db.execute("INSERT INTO catalog_source_elements VALUES(238,1,'mame_rom')")
        cls.mame_db.execute("INSERT INTO catalog_media_entries(media_entry_id) VALUES(238)")
        cls.mame_db.execute(
            "INSERT INTO mame_roms(media_entry_id,machine_id,name,size_text,status,status_specified,"
            "optional,optional_specified,source_order,source_line,source_column) "
            "VALUES(238,100,'unknown-scope.rom','0','good',0,0,0,101,48,3)"
        )
        add_hash(
            cls.mame_db, media_id=238, reported_id=9239, hash_id=9239,
            field="sha1", algorithm="sha1", scope="unknown",
            bytes_value=bytes.fromhex("38" * 20),
            position_sql=("INSERT INTO mame_rom_claims_attribute_positions "
                          "(media_entry_id,field_kind,field_occurrence,reported_hash_id,source_order,source_line,source_column) "
                          "VALUES(238,'sha1',0,9239,0,48,30)"),
            position_values=(),
        )

        cls.software_db = composed_db(cls.addClassCleanup)
        software_fixture = (ROOT / "software_field_witnesses.sql").read_text()
        edition_marker = "INSERT INTO catalog_editions\n"
        if software_fixture.count(edition_marker) != 1:
            raise AssertionError("software edition fixture boundary changed")
        software_fixture = software_fixture.replace(
            edition_marker,
            "INSERT INTO catalog_file_byte_contracts VALUES "
            "(1,'mame_0289_software_file');\n" + edition_marker,
            1,
        )
        cls.software_db.executescript(software_fixture)
        count_fixtures.seal(cls.software_db, "software", 1,
                            software_presence_check.SOFTWARE_FIELD_EVENTS)
        cls.software_db.execute("INSERT INTO catalog_source_elements VALUES(53,1,'software_rom_entry')")
        cls.software_db.execute("INSERT INTO catalog_media_entries(media_entry_id) VALUES(53)")
        cls.software_db.execute(
            "INSERT INTO software_rom_load_entries "
            "(media_entry_id,data_area_id,name,size_text,status,status_specified,source_order,source_line,source_column) "
            "VALUES(53,300,'whole-file.bin','4','good',0,3,40,5)"
        )
        cls.software_db.execute("INSERT INTO software_required_files VALUES(62,53)")
        cls.software_db.execute("INSERT INTO software_file_load_steps VALUES(53,62)")
        add_hash(
            cls.software_db, media_id=53, reported_id=9053, hash_id=9053,
            field="sha1", algorithm="sha1", scope="whole_file",
            bytes_value=bytes.fromhex("53" * 20),
            position_sql=("INSERT INTO software_rom_attribute_positions "
                          "(media_entry_id,field_kind,field_occurrence,reported_hash_id,source_order,source_line,source_column) "
                          "VALUES(53,3,0,9053,0,8,30)"),
            position_values=(),
        )

        cls.cmp_db = composed_db(cls.addClassCleanup)
        cls.cmp_db.executescript(logiqx_cmp_presence_check.transplanted_fixture(
            file_byte_contracts=((1, "logiqx_complete_declared_file"),
                                 (2, "clrmamepro_declared_asset")),
        ))
        logiqx_cmp_presence_check.seal_transplanted_fixture(cls.cmp_db)
        cls.cmp_db.execute("INSERT INTO catalog_source_elements VALUES(5000,1,'logiqx_rom')")
        cls.cmp_db.execute("INSERT INTO catalog_media_entries(media_entry_id,file_uuid) VALUES(5000,NULL)")
        cls.cmp_db.execute(
            "INSERT INTO logiqx_roms(media_entry_id,set_id,name,size_text,status,status_specified,source_order,source_line,source_column) "
            "VALUES(5000,104,'logiqx.bin','0','good',0,20,9,1)"
        )
        add_hash(
            cls.cmp_db, media_id=5000, reported_id=9112, hash_id=9112,
            field="sha1", algorithm="sha1", scope="whole_file",
            bytes_value=bytes.fromhex("12" * 20),
            position_sql=("INSERT INTO logiqx_rom_attribute_positions "
                          "(media_entry_id,field_kind,field_occurrence,source_order,source_line,source_column,reported_hash_id) "
                          "VALUES(5000,3,0,0,9,20,9112)"),
            position_values=(),
        )
        cls.cmp_db.execute("INSERT INTO catalog_source_elements VALUES(6000,2,'clrmamepro_rom')")
        cls.cmp_db.execute("INSERT INTO catalog_media_entries(media_entry_id,file_uuid) VALUES(6000,NULL)")
        cls.cmp_db.execute(
            "INSERT INTO clrmamepro_roms(media_entry_id,set_id,name,size_text,source_order,source_line,source_column) "
            "VALUES(6000,202,'cmp.bin','0',20,9,1)"
        )
        cls.cmp_db.execute(
            "INSERT INTO clrmamepro_rom_field_positions "
            "(media_entry_id,field_kind,source_order,keyword,keyword_line,keyword_column,value_line,value_column,value_is_quoted) "
            "VALUES(6000,0,0,'name',9,3,9,8,1)"
        )
        add_hash(
            cls.cmp_db, media_id=6000, reported_id=9206, hash_id=9206,
            field="sha1", algorithm="sha1", scope="whole_asset",
            bytes_value=bytes.fromhex("26" * 20),
            position_sql=("INSERT INTO clrmamepro_rom_field_positions "
                          "(media_entry_id,field_kind,source_order,keyword,keyword_line,keyword_column,"
                          "value_line,value_column,value_is_quoted,reported_hash_id) "
                          "VALUES(6000,5,1,'sha1',9,12,9,18,1,9206)"),
            position_values=(),
        )

    @classmethod
    def tearDownClass(cls):
        for db in (cls.mame_db, cls.software_db, cls.cmp_db):
            db.close()
        no_intro_presence_check.NoIntroPresence.doClassCleanups()

    def test_all_six_roles_require_real_owner_and_contract(self):
        actual = {
            (db, media_id)
            for db, media_id in (
            (self.mame_db, 237), (self.software_db, 53),
            (self.cmp_db, 5000), (self.cmp_db, 6000),
                (self.dat_db, 15001), (self.dat_db, 12012),
            )
            if db.execute(
                "SELECT 1 FROM candidate_native_file_qualification WHERE media_entry_id=?",
                (media_id,),
            ).fetchone()
        }
        self.assertEqual(len(actual), 6)
        hashes = self.software_db.execute(
            "SELECT hash_scope,source_hash_field FROM candidate_qualified_file_hashes "
            "WHERE media_entry_id=53"
        ).fetchall()
        self.assertEqual(hashes, [("whole_file", "sha1")])
        pc_hashes = self.dat_db.execute(
            "SELECT hash_scope FROM candidate_qualified_file_hashes WHERE media_entry_id=12012"
        ).fetchall()
        self.assertEqual({scope for (scope,) in pc_hashes}, {"whole_asset"})
        self.assertEqual(self.mame_db.execute(
            "SELECT count(*) FROM candidate_qualified_file_hashes WHERE media_entry_id=237"
        ).fetchone()[0], 1)

    def test_bad_present_hash_blocks_autoassignment_but_not_hash_review(self):
        db = self.mame_db
        db.execute(
            "INSERT INTO catalog_entry_hashes VALUES(9238,237,'crc',0,'invalid','whole_file',NULL,'bad')"
        )
        db.execute(
            "INSERT INTO mame_rom_claims_attribute_positions "
            "(media_entry_id,field_kind,field_occurrence,reported_hash_id,source_order,source_line,source_column) "
            "VALUES(237,'crc',0,9238,2,47,42)"
        )
        self.assertEqual(db.execute(
            "SELECT size_state,byte_length FROM candidate_native_file_byte_coverage "
            "WHERE media_entry_id=237"
        ).fetchall(), [("value", 0)])
        self.assertEqual(db.execute(
            "SELECT hash_id,source_hash_field FROM candidate_qualified_file_hashes "
            "WHERE media_entry_id=237"
        ).fetchall(), [(9237, "sha1")])
        self.assertIsNone(db.execute(
            "SELECT 1 FROM candidate_native_file_qualification WHERE media_entry_id=237"
        ).fetchone())

        # Exercise draft review with the actual native hash/position
        # projections above. The malformed CRC is absent from review evidence;
        # the valid SHA1 remains rejectable.
        registry_id = db.execute(
            "SELECT coalesce(max(registry_id),0)+1 FROM file_id_registries"
        ).fetchone()[0]
        file_uuid = bytes.fromhex("99" * 16)
        db.execute("INSERT INTO file_id_registries VALUES(?,?)",
                   (registry_id, bytes.fromhex("98" * 16)))
        db.execute("INSERT INTO shared_catalog_files VALUES(?,?)", (file_uuid, registry_id))
        db.execute(
            "INSERT INTO file_match_conflicts VALUES(1,237,?,'contradictory_assertions')",
            (file_uuid,),
        )
        db.execute("INSERT INTO file_match_decisions VALUES(1,'keep_separate',NULL,'reviewed','time')")
        db.execute("INSERT INTO file_match_decision_conflicts VALUES(1,1,'keep_separate')")
        db.execute("INSERT INTO file_match_conflict_hashes VALUES(1,9237,'incoming')")
        db.execute("INSERT INTO file_match_hash_decisions VALUES(1,1,9237,'incoming','reject')")
        self.assertEqual(db.execute(
            "SELECT disposition FROM file_match_hash_decisions "
            "WHERE decision_id=1 AND reported_hash_id=9237"
        ).fetchall(), [("reject",)])

        db.execute("UPDATE mame_roms SET size_text='not-a-size' WHERE media_entry_id=237")
        self.assertEqual(db.execute(
            "SELECT size_state FROM candidate_native_file_byte_coverage WHERE media_entry_id=237"
        ).fetchall(), [("unusable",)])
        self.assertEqual(db.execute(
            "SELECT reported_hash_id FROM candidate_qualified_file_hashes WHERE media_entry_id=237"
        ).fetchall(), [(9237,)])
        self.assertIsNone(db.execute(
            "SELECT 1 FROM candidate_native_file_qualification WHERE media_entry_id=237"
        ).fetchone())
        self.assertEqual(db.execute(
            "SELECT count(*) FROM published_catalog_editions WHERE edition_id=1"
        ).fetchone()[0], 0)
        with self.assertRaises(sqlite3.IntegrityError):
            db.execute("INSERT INTO file_match_decision_publications VALUES(1,'time')")

    def test_mame_point_qualification_and_uuid_evidence_stay_bounded(self):
        db = composed_db()
        try:
            mame_presence_check.seed(
                db,
                file_byte_contracts=((1, "mame_0289_machine_rom"),),
            )
            db.execute("INSERT INTO catalog_source_elements VALUES(237,1,'mame_rom')")
            db.execute("INSERT INTO catalog_media_entries(media_entry_id) VALUES(237)")
            db.execute(
                "INSERT INTO mame_roms(media_entry_id,machine_id,name,size_text,status,status_specified,"
                "optional,optional_specified,source_order,source_line,source_column) "
                "VALUES(237,100,'qualified.rom','0','good',0,0,0,100,47,3)"
            )
            db.execute(
                "INSERT INTO mame_rom_claims_attribute_positions "
                "(media_entry_id,field_kind,field_occurrence,source_order,source_line,source_column) "
                "VALUES(237,'name',0,0,47,10)"
            )
            add_hash(
                db, media_id=237, reported_id=9237, hash_id=9237,
                field="sha1", algorithm="sha1", scope="whole_file",
                bytes_value=bytes.fromhex("23" * 20),
                position_sql=("INSERT INTO mame_rom_claims_attribute_positions "
                              "(media_entry_id,field_kind,field_occurrence,reported_hash_id,source_order,source_line,source_column) "
                              "VALUES(237,'sha1',0,9237,1,47,30)"),
                position_values=(),
            )

            registry_id = db.execute(
                "SELECT coalesce(max(registry_id),0)+1 FROM file_id_registries"
            ).fetchone()[0]
            file_uuid = bytes.fromhex("a7" * 16)
            db.execute("INSERT INTO file_id_registries VALUES(?,?)",
                       (registry_id, bytes.fromhex("a8" * 16)))
            db.execute("INSERT INTO shared_catalog_files VALUES(?,?)", (file_uuid, registry_id))
            db.execute("UPDATE catalog_media_entries SET file_uuid=? WHERE media_entry_id=237", (file_uuid,))
            probes = (
                ('hash/media', 'SELECT reported_hash_id FROM candidate_qualified_file_hashes WHERE media_entry_id=?', (237,), [(9237,)]),
                ('hash/reported', 'SELECT reported_hash_id FROM candidate_qualified_file_hashes WHERE reported_hash_id=?', (9237,), [(9237,)]),
                ('strict/media', 'SELECT media_entry_id FROM candidate_native_file_qualification WHERE media_entry_id=?', (237,), [(237,)]),
                ('accepted-hash/UUID', 'SELECT reported_hash_id FROM accepted_catalog_file_hash_evidence WHERE file_uuid=?', (file_uuid,), [(9237,)]),
                ('accepted-size/UUID', 'SELECT media_entry_id,byte_length FROM accepted_catalog_file_size_evidence WHERE file_uuid=?', (file_uuid,), [(237, 0)]),
            )

            def measured(probe, instruction_limit=100000):
                name, query, parameters, expected = probe
                steps = 0
                deadline = time.monotonic() + 2.0

                def tick():
                    nonlocal steps
                    steps += 1
                    return int(steps > instruction_limit or time.monotonic() > deadline)

                db.set_progress_handler(tick, 1)
                try:
                    self.assertEqual(db.execute(query, parameters).fetchall(), expected, name)
                except sqlite3.OperationalError as error:
                    raise AssertionError(f'{name}: exceeded {instruction_limit} VM instructions at {steps}') from error
                finally:
                    db.set_progress_handler(None, 0)
                return steps

            for probe in probes:
                measured(probe)  # Prepare each actual public filter first.
            before = [measured(probe) for probe in probes]

            # Populate real MAME ROM, hash, and source-position rows so the
            # target lookup runs with substantial unrelated native data.
            native_rows = []
            hash_rows = []
            position_rows = []
            for index in range(1024):
                media_id = 30000 + index
                reported_id = 50000 + index
                hash_id = 60000 + index
                native_rows.append((media_id, 100, "unrelated.rom", "0", "good", 0,
                                    0, 0, 1000 + index, 60 + index, 3))
                hash_rows.append((hash_id, "sha1", index.to_bytes(4, "big") + bytes(16)))
                position_rows.extend((
                    (media_id, "name", 0, None, 0, 60 + index, 10),
                    (media_id, "size", 0, None, 1, 60 + index, 24),
                    (media_id, "sha1", 0, reported_id, 2, 60 + index, 40),
                ))
            db.executemany(
                "INSERT INTO catalog_source_elements VALUES(?,1,'mame_rom')",
                ((30000 + index,) for index in range(1024)),
            )
            db.executemany(
                "INSERT INTO catalog_media_entries(media_entry_id) VALUES(?)",
                ((30000 + index,) for index in range(1024)),
            )
            db.executemany(
                "INSERT INTO mame_roms(media_entry_id,machine_id,name,size_text,status,status_specified,"
                "optional,optional_specified,source_order,source_line,source_column) "
                "VALUES(?,?,?,?,?,?,?,?,?,?,?)",
                native_rows,
            )
            db.executemany("INSERT INTO hash_values VALUES(?,?,?)", hash_rows)
            db.executemany(
                "INSERT INTO catalog_entry_hashes "
                "(reported_hash_id,media_entry_id,source_hash_field,field_occurrence,presence,"
                "hash_scope,hash_id) VALUES(?,?, 'sha1',0,'value','whole_file',?)",
                ((50000 + index, 30000 + index, 60000 + index)
                 for index in range(1024)),
            )
            db.executemany(
                "INSERT INTO mame_rom_claims_attribute_positions "
                "(media_entry_id,field_kind,field_occurrence,reported_hash_id,source_order,source_line,source_column) "
                "VALUES(?,?,?,?,?,?,?)",
                position_rows,
            )

            after = [measured(probe) for probe in probes]
            for probe, initial, final in zip(probes, before, after):
                print(f'Native VM {probe[0]} after 1024 unrelated declarations: {initial}->{final}')
            for probe, initial, final in zip(probes, before, after):
                if final > initial + 32:
                    print('Regressed plan scans:', [row[3] for row in db.execute(
                        'EXPLAIN QUERY PLAN ' + probe[1], probe[2])
                        if 'SCAN' in row[3] or 'MATERIALIZE' in row[3]])
                self.assertLessEqual(final, initial + 32, (probe[0], initial, final))
        finally:
            db.close()

    def test_a_cmp_nodump_keeps_qualified_hash_and_strict_eligibility(self):
        db = self.cmp_db
        original_status = db.execute(
            "SELECT status_text FROM clrmamepro_rom_details WHERE media_entry_id=6000"
        ).fetchone()[0]
        db.execute("UPDATE clrmamepro_rom_details SET status_text='nodump' WHERE media_entry_id=6000")
        db.execute(
            "INSERT INTO clrmamepro_rom_field_positions "
            "(media_entry_id,field_kind,source_order,keyword,keyword_line,keyword_column,"
            "value_line,value_column,value_is_quoted) "
            "VALUES(6000,10,2,'nodump',9,40,NULL,NULL,0)"
        )
        try:
            self.assertEqual(db.execute(
                "SELECT status_text FROM clrmamepro_rom_details WHERE media_entry_id=6000"
            ).fetchall(), [("nodump",)])
            self.assertEqual(db.execute(
                "SELECT count(*) FROM clrmamepro_rom_field_positions "
                "WHERE media_entry_id=6000 AND field_kind=10"
            ).fetchone()[0], 1)
            self.assertEqual(db.execute(
                "SELECT hash_id,source_hash_field FROM candidate_qualified_file_hashes "
                "WHERE media_entry_id=6000"
            ).fetchall(), [(9206, "sha1")])
            self.assertEqual(db.execute(
                "SELECT media_entry_id FROM candidate_native_file_qualification "
                "WHERE media_entry_id=6000"
            ).fetchall(), [(6000,)])
        finally:
            db.execute("DELETE FROM clrmamepro_rom_field_positions WHERE media_entry_id=6000 AND field_kind=10")
            db.execute("UPDATE clrmamepro_rom_details SET status_text=? WHERE media_entry_id=6000",
                       (original_status,))

    def test_unknown_chd_disk_dump_export_and_control_rows_are_excluded(self):
        self.assertIsNone(self.cmp_db.execute(
            "SELECT 1 FROM candidate_native_file_roles WHERE media_entry_id IN (111,203)"
        ).fetchone())
        self.assertIsNone(self.dat_db.execute(
            "SELECT 1 FROM candidate_native_file_roles WHERE media_entry_id IN (13033,13043)"
        ).fetchone())
        self.assertIsNone(self.software_db.execute(
            "SELECT 1 FROM candidate_native_file_roles WHERE media_entry_id IN (403)"
        ).fetchone())
        self.assertIsNone(self.mame_db.execute(
            "SELECT 1 FROM candidate_native_file_qualification WHERE media_entry_id=238"
        ).fetchone())

    def test_dat_global_header_filter_blocks_even_empty_option(self):
        self.assertIsNone(self.dat_db.execute(
            "SELECT 1 FROM candidate_native_file_roles WHERE media_entry_id=11035"
        ).fetchone())
        self.dat_db.execute(
            "UPDATE no_intro_dat_clrmamepro_options SET header='' WHERE source_element_id=11020"
        )
        self.assertIsNone(self.dat_db.execute(
            "SELECT 1 FROM candidate_native_file_roles WHERE media_entry_id=11035"
        ).fetchone())

    def test_dat_rom_header_filter_even_empty_and_mame_load_overrides_block(self):
        self.dat_db.execute("UPDATE no_intro_dat_rom_claims SET header_text='' WHERE media_entry_id=15001")
        self.assertIsNone(self.dat_db.execute(
            "SELECT 1 FROM candidate_native_file_roles WHERE media_entry_id=15001"
        ).fetchone())
        self.mame_db.execute(
            "INSERT INTO mame_rom_compatibility(media_entry_id,loadflag) VALUES(237,'')"
        )
        self.assertIsNone(self.mame_db.execute(
            "SELECT 1 FROM candidate_native_file_roles WHERE media_entry_id=237"
        ).fetchone())

    def test_cmp_conflicting_crc_fields_block_uuid_qualification(self):
        for reported_id, field, field_kind, source_order, byte in (
            (9207, "crc", 2, 2, b"1234"),
            (9208, "crc32", 3, 3, b"5678"),
        ):
            add_hash(
                self.cmp_db, media_id=6000, reported_id=reported_id,
                hash_id=reported_id, field=field, algorithm="crc32",
                scope="whole_asset", bytes_value=byte,
                position_sql=("INSERT INTO clrmamepro_rom_field_positions "
                              "(media_entry_id,field_kind,source_order,keyword,keyword_line,keyword_column,"
                              "value_line,value_column,value_is_quoted,reported_hash_id) "
                              "VALUES(6000,?,?,?,9,24,9,28,1,?)"),
                position_values=(field_kind, source_order, field, reported_id),
            )
        self.assertIsNone(self.cmp_db.execute(
            "SELECT 1 FROM candidate_native_file_qualification WHERE media_entry_id=6000"
        ).fetchone())
        self.assertEqual(self.cmp_db.execute(
            "SELECT source_hash_field,hash_id FROM candidate_qualified_file_hashes "
            "WHERE media_entry_id=6000 ORDER BY source_hash_field"
        ).fetchall(), [("crc", 9207), ("crc32", 9208), ("sha1", 9206)])


if __name__ == "__main__":
    unittest.main()
