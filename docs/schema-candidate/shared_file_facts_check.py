#!/usr/bin/env python3
"""Thin review/maintenance witnesses, not native qualification or parser proof."""

import sqlite3
import unittest

import assemble
import shared_file_facts


A, B, C, D = (bytes([number]) * 16 for number in range(1, 5))


def connection():
    db = sqlite3.connect(':memory:')
    db.execute('PRAGMA foreign_keys=ON')
    shared = (assemble.ROOT / 'shared.sql').read_text().replace(
        '/* SOURCE_ELEMENT_KINDS */', "'mame_rom'")
    db.executescript(shared + (assemble.ROOT / 'relationships.sql').read_text())
    # These are explicitly thin source-size/qualification adapters. Exact
    # native owners, lexical grammars and eligibility have independent suites.
    db.executescript('''
        CREATE TABLE test_native_sizes(media_entry_id INTEGER PRIMARY KEY,
                                       byte_length INTEGER);
        CREATE VIEW candidate_native_file_sizes AS
        SELECT media_entry_id,'size' AS source_size_field,byte_length,
               CASE WHEN byte_length IS NULL THEN 'omitted' ELSE 'value' END AS size_state
        FROM test_native_sizes;
        CREATE VIEW candidate_native_file_qualification AS
        SELECT media_entry_id FROM test_native_sizes;
        CREATE VIEW candidate_native_file_byte_coverage AS
        SELECT media_entry_id FROM test_native_sizes;
        CREATE VIEW candidate_qualified_file_hashes AS
        SELECT reported_hash_id,media_entry_id,hash_id,hash_scope,source_hash_field
        FROM catalog_entry_hashes WHERE presence='value'
          AND hash_scope IN ('whole_file','whole_asset');
        CREATE TABLE test_completed_batches(batch_id INTEGER PRIMARY KEY,file_uuid BLOB);
    ''')
    db.executescript((assemble.ROOT / 'relationships_guards.sql').read_text())
    db.executescript(shared_file_facts.sql())
    db.executescript('CREATE TRIGGER test_batch_finalize AFTER INSERT ON test_completed_batches '
                     'BEGIN ' + shared_file_facts.rebuild('SELECT NEW.file_uuid AS canonical_file_uuid') + ' END;')
    guards, _ = assemble.foreign_key_guards(db)
    db.executescript('\n'.join(guards + assemble.collision_guards(db)
                               + assemble.immutable_dictionary_sql()))
    db.execute("INSERT INTO catalog_publishers VALUES(1,'publisher','Publisher',NULL)")
    db.execute("INSERT INTO catalogs VALUES(1,1,'catalog','Catalog')")
    db.execute("INSERT INTO catalog_source_files VALUES(1,?,NULL,128,'object','zstd')", (b's' * 32,))
    db.execute("INSERT INTO catalog_reading_rules VALUES(1,'rules','mame','observed','0.289','fixture','1')")
    db.execute("INSERT INTO catalog_file_byte_contracts VALUES(1,'mame_0289_machine_rom')")
    db.execute("INSERT INTO catalog_coverage VALUES(1,'unknown')")
    db.execute('INSERT INTO catalog_editions VALUES(1,1,1,1,1,NULL,NULL)')
    db.execute('INSERT INTO file_id_registries VALUES(1,?)', (b'r' * 16,))
    db.executemany('INSERT INTO shared_catalog_files VALUES(?,1)', ((file,) for file in (A, B, C, D)))
    db.execute("INSERT INTO hash_values VALUES(1,'sha1',?)", (b'h' * 20,))
    db.execute("INSERT INTO hash_values VALUES(2,'sha1',?)", (b'i' * 20,))
    return db


def source(db, media, file, size=10, hash_id=1, scope='whole_asset', edition=1):
    db.execute("INSERT INTO catalog_source_elements VALUES(?,?,'mame_rom')", (media, edition))
    db.execute('INSERT INTO catalog_media_entries VALUES(?,?)', (media, file))
    db.execute('INSERT INTO test_native_sizes VALUES(?,?)', (media, size))
    db.execute("INSERT INTO catalog_entry_hashes VALUES(?,?,'sha1',0,'value',?,?,NULL)",
               (media, media, scope, hash_id))


def finish_batch(db, file):
    db.execute('INSERT INTO test_completed_batches(file_uuid) VALUES(?)', (file,))


def decision(db, identity, incoming, candidates, *, merge=False, kept=None,
             reject_hashes=(), reject_sizes=(), incoming_witness=False, publish=True):
    db.execute('INSERT INTO file_match_decisions VALUES(?,?,?,?,?)',
               (identity, 'merge' if merge else 'keep_separate', kept, 'checked rationale', 'time'))
    for offset, candidate in enumerate(candidates):
        conflict = identity * 100 + offset
        db.execute("INSERT INTO file_match_conflicts VALUES(?,?,?,'contradictory_assertions')",
                   (conflict, incoming, candidate))
        db.execute('INSERT INTO file_match_decision_conflicts VALUES(?,?,?)',
                   (identity, conflict, 'merged' if merge else 'keep_separate'))
        # Witnesses belong to the complete current component, including aliases.
        media = [row[0] for row in db.execute(
            'SELECT media.media_entry_id FROM candidate_file_component_members AS member '
            'JOIN catalog_media_entries AS media ON media.file_uuid=member.file_uuid '
            'WHERE member.canonical_file_uuid=?', (candidate,))]
        witnesses = [(value, 'candidate') for value in media]
        if incoming_witness:
            witnesses.append((incoming, 'incoming'))
        for value, role in witnesses:
            db.execute('INSERT INTO file_match_conflict_hashes VALUES(?,?,?)', (conflict, value, role))
            db.execute('INSERT INTO file_match_conflict_sizes VALUES(?,?,?,?)',
                       (conflict, value, 'size', role))
            db.execute('INSERT INTO file_match_hash_decisions VALUES(?,?,?,?,?)',
                       (identity, conflict, value, role, 'reject' if value in reject_hashes else 'accept'))
            db.execute('INSERT INTO file_match_size_decisions VALUES(?,?,?,?,?,?)',
                       (identity, conflict, value, 'size', role,
                        'reject' if value in reject_sizes else 'accept'))
        if merge and candidate != kept:
            db.execute('INSERT INTO file_match_uuid_redirects VALUES(?,?,?)', (candidate, kept, identity))
    if publish:
        db.execute('INSERT INTO file_match_decision_publications VALUES(?,?)', (identity, 'time'))


class SharedFactReviews(unittest.TestCase):
    def setUp(self):
        self.db = connection()
        self.addCleanup(self.db.close)
        source(self.db, 1, A)
        source(self.db, 2, A)
        source(self.db, 3, None)
        finish_batch(self.db, A)
        self.db.execute("INSERT INTO published_catalog_editions VALUES(1,1,1,1,1,'time')")

    def memberships(self, file=A):
        return (
            self.db.execute('SELECT hash_id FROM shared_file_hashes WHERE file_uuid=? ORDER BY hash_id', (file,)).fetchall(),
            self.db.execute('SELECT byte_length FROM shared_file_sizes WHERE file_uuid=? ORDER BY byte_length', (file,)).fetchall(),
        )

    def test_both_lanes_have_distinct_source_supported_membership(self):
        self.assertEqual(self.memberships(), ([(1,)], [(10,)]))
        self.assertEqual(self.db.execute('SELECT * FROM catalog_shared_fact_mismatches').fetchall(), [])

    def test_reject_one_witness_then_last_removes_only_its_lane(self):
        decision(self.db, 1, 3, [A], reject_hashes={1}, reject_sizes={1})
        self.assertEqual(self.memberships(), ([(1,)], [(10,)]))
        # Do not re-accept the exact witness rejected by the first review.
        decision(self.db, 2, 3, [A], reject_hashes={1, 2}, reject_sizes={1})
        self.assertEqual(self.memberships(), ([], [(10,)]))
        decision(self.db, 3, 3, [A], reject_hashes={1, 2}, reject_sizes={1, 2})
        self.assertEqual(self.memberships(), ([], []))
        self.assertEqual(self.db.execute('SELECT * FROM catalog_shared_fact_mismatches').fetchall(), [])

    def test_draft_rejection_has_no_effect_and_exact_rejection_cannot_be_restored(self):
        decision(self.db, 1, 3, [A], reject_hashes={1, 2}, reject_sizes={1, 2}, publish=False)
        self.assertEqual(self.memberships(), ([(1,)], [(10,)]))
        self.db.execute("INSERT INTO file_match_decision_publications VALUES(1,'time')")
        self.assertEqual(self.memberships(), ([], []))
        with self.assertRaises(sqlite3.IntegrityError):
            decision(self.db, 2, 3, [A])

    def test_accepting_incoming_does_not_link_it_or_promote_its_facts(self):
        self.db.execute('UPDATE test_native_sizes SET byte_length=11 WHERE media_entry_id=3')
        decision(self.db, 1, 3, [A], incoming_witness=True)
        self.assertEqual(self.memberships(), ([(1,)], [(10,)]))
        self.assertIsNone(self.db.execute('SELECT file_uuid FROM catalog_media_entries WHERE media_entry_id=3').fetchone()[0])

    def test_merge_cleans_old_aliases_and_preserves_issued_source_assignments(self):
        source(self.db, 4, B)
        finish_batch(self.db, B)
        decision(self.db, 1, 3, [A, B], merge=True, kept=A, incoming_witness=True)
        self.assertEqual(self.memberships(B), ([], []))
        self.assertEqual(self.memberships(), ([(1,)], [(10,)]))
        self.assertEqual(self.db.execute('SELECT file_uuid FROM catalog_media_entries WHERE media_entry_id=4').fetchone(), (B,))
        self.assertEqual(self.db.execute('SELECT canonical_file_uuid FROM canonical_shared_file_uuids WHERE file_uuid=?', (B,)).fetchone(), (A,))
        # A subsequent decision may cite a source still assigned the old alias.
        decision(self.db, 2, 3, [A], reject_hashes={4}, reject_sizes={4})
        self.assertEqual(self.memberships(), ([(1,)], [(10,)]))
        self.assertEqual(self.db.execute('SELECT * FROM catalog_shared_fact_mismatches').fetchall(), [])

    def test_incompatible_incoming_merge_rolls_back_redirect_visibility_and_both_lanes(self):
        source(self.db, 4, B)
        finish_batch(self.db, B)
        self.db.execute('UPDATE test_native_sizes SET byte_length=11 WHERE media_entry_id=3')
        decision(self.db, 1, 3, [A, B], merge=True, kept=A, incoming_witness=True, publish=False)
        before = (self.memberships(A), self.memberships(B))
        with self.assertRaisesRegex(sqlite3.IntegrityError, 'incoming size'):
            self.db.execute("INSERT INTO file_match_decision_publications VALUES(1,'time')")
        self.assertEqual((self.memberships(A), self.memberships(B)), before)
        self.assertEqual(self.db.execute('SELECT * FROM file_match_decision_publications').fetchall(), [])
        self.assertEqual(self.db.execute('SELECT canonical_file_uuid FROM canonical_shared_file_uuids WHERE file_uuid=?', (B,)).fetchone(), (B,))

    def test_wrong_size_selector_and_late_witnesses_are_rejected(self):
        decision(self.db, 1, 3, [A])
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO file_match_conflict_sizes VALUES(100,3,'file_length','incoming')")
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO file_match_conflict_hashes VALUES(100,3,'incoming')")

    def test_incompatible_incoming_hash_rolls_back_both_membership_lanes(self):
        source(self.db, 4, B)
        finish_batch(self.db, B)
        self.db.execute('UPDATE catalog_entry_hashes SET hash_id=2 WHERE reported_hash_id=3')
        decision(self.db, 1, 3, [A, B], merge=True, kept=A,
                 incoming_witness=True, publish=False)
        before = (self.memberships(A), self.memberships(B))
        with self.assertRaisesRegex(sqlite3.IntegrityError, 'incoming hash'):
            self.db.execute("INSERT INTO file_match_decision_publications VALUES(1,'time')")
        self.assertEqual((self.memberships(A), self.memberships(B)), before)
        self.assertEqual(self.db.execute('SELECT * FROM file_match_decision_publications').fetchall(), [])
        self.assertEqual(self.db.execute(
            'SELECT canonical_file_uuid FROM canonical_shared_file_uuids WHERE file_uuid=?',
            (B,)).fetchone(), (B,))

    def test_conflicting_component_facts_require_explicit_rejection_before_merge(self):
        source(self.db, 4, B, size=11, hash_id=2)
        finish_batch(self.db, B)
        self.db.execute('SAVEPOINT failed_review')
        decision(self.db, 1, 3, [A, B], merge=True, kept=A, publish=False)
        before = (self.memberships(A), self.memberships(B))
        with self.assertRaisesRegex(sqlite3.IntegrityError, 'contradictory accepted facts'):
            self.db.execute("INSERT INTO file_match_decision_publications VALUES(1,'time')")
        self.assertEqual((self.memberships(A), self.memberships(B)), before)
        self.db.execute('ROLLBACK TO failed_review')
        self.db.execute('RELEASE failed_review')
        # An independent decision may settle the same conflicts by rejecting
        # exactly the source witnesses, not deleting the source declarations.
        decision(self.db, 2, 3, [A, B], merge=True, kept=A,
                 reject_hashes={4}, reject_sizes={4})
        self.assertEqual(self.memberships(A), ([(1,)], [(10,)]))
        self.assertEqual(self.memberships(B), ([], []))
        self.assertEqual(self.db.execute(
            'SELECT hash_id FROM catalog_entry_hashes WHERE reported_hash_id=4').fetchone(), (2,))

    def test_unrelated_bad_draft_component_does_not_block_review(self):
        self.db.execute('INSERT INTO shared_file_sizes VALUES(?,999)', (D,))
        decision(self.db, 1, 3, [A])
        self.assertEqual(self.memberships(), ([(1,)], [(10,)]))
        self.assertEqual(self.memberships(D), ([], [(999,)]))
        self.assertEqual(self.db.execute('SELECT problem FROM catalog_shared_fact_mismatches').fetchall(), [('size_unsupported',)])

    def test_review_waits_for_all_component_and_incoming_source_owners(self):
        for edition in (2, 3):
            self.db.execute(
                "INSERT INTO catalog_reading_rules VALUES(?,?,'mame','observed','0.289','fixture','1')",
                (edition, f'draft-{edition}'))
            self.db.execute('INSERT INTO catalog_editions VALUES(?,1,1,?,1,NULL,NULL)',
                            (edition, edition))
        source(self.db, 4, A, size=None, edition=2)
        self.db.execute('DELETE FROM catalog_entry_hashes WHERE reported_hash_id=4')
        source(self.db, 5, None, edition=2)
        source(self.db, 6, D, edition=3)
        # Neither decision names explicit witnesses. Component owners with no
        # usable facts and unlinked incoming owners still must be frozen.
        for identity, incoming, candidate in ((1, 3, A), (2, 5, B)):
            self.db.execute("INSERT INTO file_match_decisions VALUES(?,'keep_separate',NULL,'checked','time')", (identity,))
            self.db.execute("INSERT INTO file_match_conflicts VALUES(?,?,?,'contradictory_assertions')", (identity, incoming, candidate))
            self.db.execute("INSERT INTO file_match_decision_conflicts VALUES(?,?,'keep_separate')", (identity, identity))
            before = self.memberships()
            with self.assertRaisesRegex(sqlite3.IntegrityError, 'published source owners'):
                self.db.execute("INSERT INTO file_match_decision_publications VALUES(?,'time')", (identity,))
            self.assertEqual(self.memberships(), before)
        self.assertEqual(self.db.execute('SELECT * FROM file_match_decision_publications').fetchall(), [])
        self.db.execute("INSERT INTO published_catalog_editions VALUES(2,1,1,2,1,'time')")
        # The same decisions now publish; edition 3's unrelated D draft does
        # not block A or B. No separate committed-state flag is needed.
        self.db.executemany("INSERT INTO file_match_decision_publications VALUES(?,'time')", ((1,), (2,)))
        self.assertEqual(self.memberships(), ([(1,)], [(10,)]))
        self.assertIsNone(self.db.execute('SELECT file_uuid FROM catalog_media_entries WHERE media_entry_id=5').fetchone()[0])

    def test_corrupt_redirect_edges_are_audited_and_cycles_cannot_be_extended(self):
        # Corruption injection is independent of the supported write guards.
        self.db.execute('INSERT INTO file_id_registries VALUES(2,?)', (b'q' * 16,))
        foreign = b'x' * 16
        self.db.execute('INSERT INTO shared_catalog_files VALUES(?,2)', (foreign,))
        disabled = [(name, sql) for name, sql in self.db.execute(
            "SELECT name,sql FROM sqlite_schema WHERE type='trigger'").fetchall()
            if name.startswith('file_match_') or name.startswith('shared_file_facts_')]
        for name, _ in disabled:
            self.db.execute('DROP TRIGGER "' + name + '"')
        self.db.execute("INSERT INTO file_match_decisions VALUES(90,'merge',?,'corrupted','time')", (C,))
        self.db.execute('INSERT INTO file_match_uuid_redirects VALUES(?,?,90)', (A, foreign))
        self.db.execute('INSERT INTO file_match_uuid_redirects VALUES(?,?,90)', (foreign, C))
        self.db.execute("INSERT INTO file_match_decision_publications VALUES(90,'time')")
        self.assertEqual(self.db.execute(
            "SELECT count(*) FROM candidate_shared_identity_problems "
            "WHERE problem LIKE 'issued_file_cross_registry_redirect:%'"
        ).fetchone(), (2,))
        # Replace only the deliberately corrupt edge, then restore real guards.
        self.db.execute('UPDATE file_match_uuid_redirects SET kept_file_uuid=? WHERE old_file_uuid=?', (A, foreign))
        self.db.executescript('\n'.join(sql + ';' for _, sql in disabled))
        source(self.db, 4, D)
        finish_batch(self.db, D)
        decision(self.db, 1, 3, [A, D], merge=True, kept=A, publish=False)
        with self.assertRaisesRegex(sqlite3.IntegrityError, 'incomplete or has an unreviewed redirect'):
            self.db.execute("INSERT INTO file_match_decision_publications VALUES(1,'time')")
        self.assertEqual(self.db.execute(
            'SELECT canonical_file_uuid FROM canonical_shared_file_uuids WHERE file_uuid=?', (D,)
        ).fetchone(), (D,))

    def test_redirect_chain_point_reads_do_not_visit_unrelated_source_witnesses(self):
        source(self.db, 4, B)
        source(self.db, 5, C)
        finish_batch(self.db, B)
        finish_batch(self.db, C)
        decision(self.db, 1, 3, [A, B], merge=True, kept=A)
        decision(self.db, 2, 3, [A, C], merge=True, kept=C)
        self.assertEqual(self.memberships(A), ([], []))
        self.assertEqual(self.memberships(B), ([], []))
        self.assertEqual(self.memberships(C), ([(1,)], [(10,)]))

        def point_work():
            count = 0

            def tick():
                nonlocal count
                count += 1
                return 0

            self.db.set_progress_handler(tick, 1)
            try:
                self.assertEqual(self.db.execute(
                    'SELECT canonical_file_uuid FROM canonical_shared_file_uuids WHERE file_uuid=?',
                    (B,),
                ).fetchall(), [(C,)])
                self.assertEqual(self.db.execute(
                    'SELECT hash_id FROM canonical_catalog_file_hash_evidence WHERE file_uuid=?',
                    (C,),
                ).fetchall(), [(1,)])
                self.assertEqual(self.db.execute(
                    'SELECT byte_length FROM canonical_catalog_file_size_evidence WHERE file_uuid=?',
                    (C,),
                ).fetchall(), [(10,)])
            finally:
                self.db.set_progress_handler(None, 0)
            return count

        point_work()
        before = point_work()

        def publish_work(identity, instruction_limit=100000):
            decision(self.db, identity, 3, [C], publish=False)
            count = 0

            def tick():
                nonlocal count
                count += 1
                return int(count > instruction_limit)

            self.db.set_progress_handler(tick, 1)
            try:
                self.db.execute("INSERT INTO file_match_decision_publications VALUES(?,'time')", (identity,))
            except sqlite3.OperationalError as error:
                if count > instruction_limit:
                    raise AssertionError(f'review exceeded {instruction_limit} VM instructions') from error
                raise
            finally:
                self.db.set_progress_handler(None, 0)
            return count

        publish_before = publish_work(3)
        for offset in range(1024):
            file = (10000 + offset).to_bytes(16, 'big')
            self.db.execute('INSERT INTO shared_catalog_files VALUES(?,1)', (file,))
            source(self.db, 10000 + offset, file)
        after = point_work()
        # An indexed cursor's terminal branch may change with B-tree layout;
        # allow a fixed four instructions, not work proportional to 1,024 rows.
        self.assertLessEqual(after, before + 4, (before, after))
        # These unrelated incomplete batches do not block or amplify a review.
        publish_after = publish_work(4, publish_before + 256)
        self.assertLessEqual(publish_after, publish_before + 256,
                             (publish_before, publish_after))
        print(f'Component VM work after 1024 unrelated witnesses: '
              f'point {before}->{after}; publication {publish_before}->{publish_after}')
        self.assertEqual(self.memberships(C), ([(1,)], [(10,)]))

    def test_contract_is_family_bound_immutable_and_cannot_upgrade_published_rules(self):
        for contract in ('mame_0289_software_file', 'no_intro_pc_fixture_asset'):
            with self.subTest(contract=contract), self.assertRaises(sqlite3.IntegrityError):
                self.db.execute('INSERT OR REPLACE INTO catalog_file_byte_contracts VALUES(1,?)', (contract,))
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("UPDATE catalog_file_byte_contracts SET contract_kind='mame_0289_machine_rom'")
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute('DELETE FROM catalog_file_byte_contracts')
        self.db.execute("INSERT INTO catalog_reading_rules VALUES(2,'unproven','mame','observed','0.289','fixture','1')")
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO catalog_file_byte_contracts VALUES(2,'logiqx_complete_declared_file')")
        self.db.execute('INSERT INTO catalog_editions VALUES(2,1,1,2,1,NULL,NULL)')
        self.db.execute("INSERT INTO published_catalog_editions VALUES(2,1,1,2,1,'time')")
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO catalog_file_byte_contracts VALUES(2,'mame_0289_machine_rom')")


if __name__ == '__main__':
    unittest.main()
