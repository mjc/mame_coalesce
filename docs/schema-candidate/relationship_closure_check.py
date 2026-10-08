#!/usr/bin/env python3
"""Small constructed relationship publication and corruption witnesses."""

import sqlite3
import unittest
from pathlib import Path

import assemble
import relationship_closure


ROOT = Path(__file__).resolve().parent


def connection():
    db = sqlite3.connect(":memory:")
    db.execute("PRAGMA foreign_keys=ON")
    shared = (ROOT / "shared.sql").read_text().replace(
        "/* SOURCE_ELEMENT_KINDS */", "'mame_machine','mame_rom','no_intro_dat_game','no_intro_dat_rom'")
    db.executescript(shared + ''.join((ROOT / f'{family}.sql').read_text()
                                     for family in assemble.FAMILIES)
                     + (ROOT / "relationships.sql").read_text())
    db.executescript((ROOT / "relationships_guards.sql").read_text())
    # Bounded owner projection for these unit cases; see the actual native
    # MAME/No-Intro integration witness below.
    db.executescript("CREATE TABLE relationship_test_native_owners(source_element_id INTEGER,element_kind TEXT);")
    db.executescript("CREATE VIEW candidate_native_owners AS SELECT * FROM relationship_test_native_owners;")
    db.executemany("INSERT INTO relationship_test_native_owners VALUES(?,?)",
                   ((1, 'mame_machine'), (2, 'mame_machine')))
    manifest = assemble.owners()
    db.executescript(relationship_closure.sql(manifest))
    db.executescript("\n".join(relationship_closure.guards(manifest)))
    db.executescript("CREATE VIEW candidate_relationship_closure_audit AS "
                     + " UNION ALL ".join(relationship_closure.audit_queries()))
    db.execute("INSERT INTO catalog_publishers VALUES(1,'publisher','Publisher',NULL)")
    db.execute("INSERT INTO catalogs VALUES(1,1,'catalog','Catalog')")
    db.execute("INSERT INTO catalog_source_files VALUES(1,?,NULL,128,'object','zstd')", (b's' * 32,))
    db.execute("INSERT INTO catalog_reading_rules VALUES(1,'rules','mame','observed','pinned','candidate','v3')")
    db.execute("INSERT INTO catalog_coverage VALUES(1,'complete')")
    db.execute("INSERT INTO catalog_editions VALUES(1,1,1,1,1,NULL,NULL)")
    db.execute("INSERT INTO catalog_set_groups VALUES(1,1,'root')")
    db.execute("INSERT INTO catalog_source_elements VALUES(1,1,'mame_machine')")
    db.execute("INSERT INTO catalog_source_elements VALUES(2,1,'mame_machine')")
    db.execute("INSERT INTO catalog_sets VALUES(1,1,'set',0,1,1)")
    db.execute("INSERT INTO catalog_sets VALUES(2,1,'set-2',1,1,1)")
    db.execute("INSERT INTO published_catalog_editions VALUES(1,1,1,1,1,'published')")
    db.execute("INSERT INTO catalog_relationship_rules VALUES(1,'rule','1','test')")
    db.execute("INSERT INTO catalog_relationship_targets VALUES(1,'catalog_set')")
    db.execute("INSERT INTO catalog_set_targets(target_id,set_id) VALUES(1,1)")
    db.execute("INSERT INTO catalog_relationship_targets VALUES(2,'catalog_set')")
    return db


def native_connection():
    """Actual MAME/No-Intro typed-owner routes; not parser/corpus evidence."""
    db = sqlite3.connect(":memory:")
    db.execute("PRAGMA foreign_keys=ON")
    manifest = assemble.owners()
    native = tuple(owner for owner in manifest if owner.kind.startswith(("mame_", "no_intro_")))
    kinds = ",".join(assemble.literal(owner.kind) for owner in manifest)
    source = (ROOT / "shared.sql").read_text().replace("/* SOURCE_ELEMENT_KINDS */", kinds)
    source += (ROOT / "mame.sql").read_text() + (ROOT / "no_intro.sql").read_text()
    source += (ROOT / "relationships.sql").read_text()
    db.executescript(source + (ROOT / "relationships_guards.sql").read_text())
    owner_view, _, native_guards = assemble.ownership_sql(db, native)
    db.executescript(owner_view + "\n".join(native_guards))
    db.executescript(relationship_closure.sql(native))
    db.executescript("\n".join(relationship_closure.guards(native)))
    db.executescript("CREATE VIEW candidate_relationship_closure_audit AS "
                     + " UNION ALL ".join(relationship_closure.audit_queries()))

    for publisher_id in (1, 2):
        db.execute("INSERT INTO catalog_publishers VALUES(?,?,?,NULL)",
                   (publisher_id, f"publisher-{publisher_id}", f"Publisher {publisher_id}"))
        db.execute("INSERT INTO catalogs VALUES(?,?,?,?)",
                   (publisher_id, publisher_id, f"catalog-{publisher_id}", f"Catalog {publisher_id}"))
        db.execute("INSERT INTO catalog_source_files VALUES(?,?,NULL,128,?,'zstd')",
                   (publisher_id, bytes([publisher_id]) * 32, f"object-{publisher_id}"))
        db.execute("INSERT INTO catalog_coverage VALUES(?,'complete')", (publisher_id,))
    db.execute("INSERT INTO catalog_reading_rules VALUES(1,'mame-rules','mame','observed','pinned','candidate','v3')")
    db.execute("INSERT INTO catalog_reading_rules VALUES(2,'dat-rules','no_intro_dat','no-intro-dat-v4-compatible','pinned','candidate','v3')")
    db.execute("INSERT INTO catalog_editions VALUES(1,1,1,1,1,NULL,NULL)")
    db.execute("INSERT INTO catalog_editions VALUES(2,2,2,2,2,NULL,NULL)")
    db.execute("INSERT INTO catalog_set_groups VALUES(1,1,'root')")
    db.execute("INSERT INTO catalog_set_groups VALUES(2,2,'root')")
    db.execute("INSERT INTO catalog_source_elements VALUES(1,1,'mame_machine')")
    db.execute("INSERT INTO catalog_source_elements VALUES(2,2,'no_intro_dat_game')")
    db.execute("INSERT INTO catalog_sets VALUES(1,1,'mame-set',0,1,1)")
    db.execute("INSERT INTO catalog_sets VALUES(2,2,'dat-set',0,1,1)")
    db.execute("INSERT INTO mame_machines VALUES(1,NULL,0,1,0,1,0,1,1,1)")
    db.execute("INSERT INTO no_intro_dat_games VALUES(2,NULL)")
    for edition_id in (1, 2):
        db.execute("INSERT INTO published_catalog_editions VALUES(?,?,?,?,?,'published')",
                   (edition_id, edition_id, edition_id, edition_id, edition_id))
    db.execute("INSERT INTO catalog_relationship_rules VALUES(1,'rule','1','test')")
    db.execute("INSERT INTO catalog_relationship_targets VALUES(1,'catalog_set')")
    db.execute("INSERT INTO catalog_set_targets(target_id,set_id) VALUES(1,1)")
    db.execute("INSERT INTO catalog_relationship_targets VALUES(2,'catalog_set')")
    db.execute("INSERT INTO catalog_set_targets(target_id,set_id) VALUES(2,2)")
    identity(db)
    derived(db, from_target=1, to_target=2)
    rationale(db)
    db.execute("INSERT INTO catalog_relationship_evidence_publications VALUES(1,'rationale')")
    identity(db, origin="user", relationship_id=2)
    db.execute("INSERT INTO manual_catalog_relationships VALUES(2,'revision_of',1,2)")
    rationale(db, relationship_id=2)
    db.execute("INSERT INTO catalog_relationship_evidence_publications VALUES(2,'rationale')")
    return db


def identity(db, origin="derived", relationship_id=1):
    db.execute("INSERT INTO catalog_relationships(relationship_id,assertion_key,origin) VALUES(?,?,?)",
               (relationship_id, f"rel-{relationship_id}", origin))


def derived(db, relationship_id=1, from_target=1, to_target=2):
    db.execute("INSERT INTO inferred_catalog_relationships VALUES(?,?,?,?,?)",
               (relationship_id, "revision_of", from_target, to_target, 1))


def complete_second_target(db):
    db.execute("INSERT INTO catalog_set_targets(target_id,set_id) VALUES(2,2)")


def rationale(db, relationship_id=1):
    db.execute("INSERT INTO catalog_relationship_rationales VALUES(?,?)",
               (relationship_id, "reviewed rationale"))


def review(db, relationship_id=1, decision="accepted"):
    db.execute("INSERT INTO catalog_relationship_reviews"
               "(review_key,relationship_id,decision,note) VALUES(?,?,?,?)",
               (f"review-{relationship_id}", relationship_id, decision, "reviewed"))
    return db.execute("SELECT last_insert_rowid()").fetchone()[0]


class RelationshipClosure(unittest.TestCase):
    def setUp(self):
        self.db = connection()
        self.addCleanup(self.db.close)

    def test_evidence_seal_requires_exact_origin_payload_and_complete_endpoint(self):
        identity(self.db)
        rationale(self.db)
        with self.assertRaisesRegex(sqlite3.IntegrityError, "relationship evidence"):
            self.db.execute("INSERT INTO catalog_relationship_evidence_publications VALUES(1,'rationale')")

        derived(self.db)
        complete_second_target(self.db)
        self.db.execute("INSERT INTO catalog_relationship_evidence_publications VALUES(1,'rationale')")
        self.assertEqual(self.db.execute(
            "SELECT * FROM candidate_relationship_closure_audit").fetchall(), [])

    def test_manual_endpoint_must_be_typed_and_published_before_evidence_seal(self):
        identity(self.db, "user")
        self.db.execute("INSERT INTO manual_catalog_relationships VALUES(1,'revision_of',1,2)")
        rationale(self.db)
        with self.assertRaisesRegex(sqlite3.IntegrityError, "relationship evidence"):
            self.db.execute("INSERT INTO catalog_relationship_evidence_publications VALUES(1,'rationale')")

    def test_accepted_review_rechecks_missing_origin_payload(self):
        identity(self.db, "user")
        rationale(self.db)
        # Model a legacy/corrupt evidence seal that passed the old guard.
        self.db.execute("DROP TRIGGER relationship_closure_evidence_guard")
        self.db.execute("INSERT INTO catalog_relationship_evidence_publications VALUES(1,'rationale')")
        review_id = review(self.db)
        with self.assertRaisesRegex(sqlite3.IntegrityError, "relationship review"):
            self.db.execute("INSERT INTO catalog_relationship_review_publications VALUES(?,'published')", (review_id,))

    def test_accepted_review_rechecks_missing_typed_endpoint(self):
        identity(self.db, "user")
        self.db.execute("INSERT INTO manual_catalog_relationships VALUES(1,'revision_of',1,2)")
        rationale(self.db)
        # Construct a pre-existing seal that the former evidence guard allowed.
        self.db.execute("DROP TRIGGER relationship_closure_evidence_guard")
        self.db.execute("INSERT INTO catalog_relationship_evidence_publications VALUES(1,'rationale')")
        review_id = review(self.db)
        with self.assertRaisesRegex(sqlite3.IntegrityError, "accepted relationship review"):
            self.db.execute("INSERT INTO catalog_relationship_review_publications VALUES(?,'published')", (review_id,))

    def test_sealed_meaning_and_endpoint_facts_cannot_be_rewritten_or_extended(self):
        identity(self.db)
        derived(self.db)
        complete_second_target(self.db)
        rationale(self.db)
        self.db.execute("INSERT INTO catalog_relationship_evidence_publications VALUES(1,'rationale')")
        with self.assertRaisesRegex(sqlite3.IntegrityError, "immutable"):
            self.db.execute("UPDATE inferred_catalog_relationships SET to_target_id=2 WHERE relationship_id=1")
        with self.assertRaisesRegex(sqlite3.IntegrityError, "immutable"):
            self.db.execute("UPDATE catalog_relationship_rationales SET reason='changed' WHERE relationship_id=1")
        with self.assertRaisesRegex(sqlite3.IntegrityError, "sealed"):
            self.db.execute("INSERT INTO external_catalog_targets(target_id,namespace,declared_key) VALUES(1,'x','y')")
        with self.assertRaisesRegex(sqlite3.IntegrityError, "sealed"):
            self.db.execute("INSERT INTO catalog_relationship_evidence VALUES(1,0,1)")

    def test_rule_definition_cannot_change_or_disappear_after_derived_seal(self):
        identity(self.db)
        derived(self.db)
        complete_second_target(self.db)
        rationale(self.db)
        self.db.execute("INSERT INTO catalog_relationship_evidence_publications VALUES(1,'rationale')")
        with self.assertRaisesRegex(sqlite3.IntegrityError, "immutable"):
            self.db.execute("UPDATE catalog_relationship_rules SET description='rewritten' WHERE rule_id=1")
        with self.assertRaisesRegex(sqlite3.IntegrityError, "immutable"):
            self.db.execute("DELETE FROM catalog_relationship_rules WHERE rule_id=1")

    def test_unresolved_endpoint_freezes_edition_context_only_after_seal(self):
        self.db.execute("INSERT INTO catalog_reading_rules VALUES(2,'rules-2','mame','observed','pinned','candidate','v3')")
        self.db.execute("INSERT INTO catalog_reading_rules VALUES(3,'rules-3','mame','observed','pinned','candidate','v3')")
        self.db.execute("INSERT INTO catalog_reading_rules VALUES(4,'rules-4','mame','observed','pinned','candidate','v3')")
        self.db.execute("INSERT INTO catalog_editions VALUES(2,1,1,2,1,NULL,NULL)")
        self.db.execute("INSERT INTO catalog_relationship_targets VALUES(3,'unresolved_catalog')")
        self.db.execute("INSERT INTO unresolved_catalog_targets"
                        "(target_id,edition_id,record_kind,declared_name) VALUES(3,2,'catalog_set','draft')")
        self.db.execute("INSERT INTO catalog_editions VALUES(3,1,1,4,1,NULL,NULL)")
        self.db.execute("INSERT INTO catalog_relationship_targets VALUES(4,'unresolved_catalog')")
        self.db.execute("INSERT INTO unresolved_catalog_targets"
                        "(target_id,edition_id,record_kind,declared_name) VALUES(4,3,'catalog_set','manual-draft')")
        identity(self.db)
        derived(self.db, to_target=3)
        rationale(self.db)
        identity(self.db, origin="user", relationship_id=2)
        self.db.execute("INSERT INTO manual_catalog_relationships VALUES(2,'revision_of',1,4)")
        rationale(self.db, relationship_id=2)

        # Draft references do not freeze the edition's defining context.
        self.db.execute("UPDATE catalog_editions SET reading_rules_id=3 WHERE edition_id=2")
        self.assertEqual(self.db.execute(
            "SELECT reading_rules_id FROM catalog_editions WHERE edition_id=2").fetchone(), (3,))
        self.db.execute("INSERT INTO catalog_relationship_evidence_publications VALUES(1,'rationale')")
        with self.assertRaisesRegex(sqlite3.IntegrityError, "immutable"):
            self.db.execute("UPDATE catalog_editions SET reading_rules_id=2 WHERE edition_id=2")
        self.db.execute("INSERT INTO catalog_relationship_evidence_publications VALUES(2,'rationale')")
        with self.assertRaisesRegex(sqlite3.IntegrityError, "immutable"):
            self.db.execute("UPDATE catalog_editions SET reading_rules_id=2 WHERE edition_id=3")

        # Ignore the ordinary FK restriction so DELETE must be rejected by the
        # sealed-relationship guard itself.
        self.db.commit()
        self.db.execute("PRAGMA foreign_keys=OFF")
        with self.assertRaisesRegex(sqlite3.IntegrityError, "immutable"):
            self.db.execute("DELETE FROM catalog_editions WHERE edition_id=2")
        with self.assertRaisesRegex(sqlite3.IntegrityError, "immutable"):
            self.db.execute("DELETE FROM catalog_editions WHERE edition_id=3")

    def test_draft_child_cannot_be_moved_onto_a_sealed_relationship(self):
        identity(self.db)
        derived(self.db)
        complete_second_target(self.db)
        rationale(self.db)
        self.db.execute("INSERT INTO catalog_relationship_evidence_publications VALUES(1,'rationale')")
        identity(self.db, "user", 2)
        rationale(self.db, 2)
        with self.assertRaisesRegex(sqlite3.IntegrityError, "immutable"):
            self.db.execute("UPDATE catalog_relationship_rationales SET relationship_id=1 WHERE relationship_id=2")

    def test_comparison_payload_and_fields_freeze_at_evidence_seal(self):
        identity(self.db)
        derived(self.db)
        complete_second_target(self.db)
        self.db.execute("INSERT INTO catalog_relationship_comparisons VALUES(1,'supports')")
        self.db.execute("INSERT INTO catalog_relationship_comparison_fields VALUES(1,'support',0,'set_name')")
        self.db.execute("INSERT INTO catalog_relationship_evidence_publications VALUES(1,'catalog_comparison')")
        with self.assertRaisesRegex(sqlite3.IntegrityError, "immutable"):
            self.db.execute("UPDATE catalog_relationship_comparisons SET status='inconclusive' WHERE relationship_id=1")
        with self.assertRaisesRegex(sqlite3.IntegrityError, "immutable"):
            self.db.execute("UPDATE catalog_relationship_comparison_fields SET field_kind='media_name' WHERE relationship_id=1")
        with self.assertRaisesRegex(sqlite3.IntegrityError, "immutable"):
            self.db.execute("INSERT INTO catalog_relationship_comparison_fields VALUES(1,'support',1,'media_name')")

    def test_native_owner_kind_routes_are_format_specific_and_unique(self):
        self.db.execute("INSERT INTO catalog_source_elements VALUES(3,1,'mame_rom')")
        self.db.execute("INSERT INTO catalog_media_entries VALUES(3,NULL)")
        self.db.execute("INSERT INTO relationship_test_native_owners VALUES(3,'mame_rom')")
        self.db.execute("INSERT INTO catalog_relationship_targets VALUES(3,'catalog_media_entry')")
        self.db.execute("INSERT INTO catalog_media_entry_targets(target_id,media_entry_id) VALUES(3,3)")
        identity(self.db)
        derived(self.db, from_target=1, to_target=3)
        rationale(self.db)
        self.db.execute("INSERT INTO catalog_relationship_evidence_publications VALUES(1,'rationale')")

        self.db.execute("INSERT INTO catalog_source_elements VALUES(4,1,'mame_rom')")
        self.db.execute("INSERT INTO catalog_media_entries VALUES(4,NULL)")
        self.db.execute("INSERT INTO relationship_test_native_owners VALUES(4,'mame_machine')")
        self.db.execute("INSERT INTO catalog_relationship_targets VALUES(4,'catalog_media_entry')")
        self.db.execute("INSERT INTO catalog_media_entry_targets(target_id,media_entry_id) VALUES(4,4)")
        identity(self.db, relationship_id=2)
        derived(self.db, relationship_id=2, from_target=1, to_target=4)
        rationale(self.db, relationship_id=2)
        with self.assertRaisesRegex(sqlite3.IntegrityError, "relationship evidence"):
            self.db.execute("INSERT INTO catalog_relationship_evidence_publications VALUES(2,'rationale')")

        self.db.execute("INSERT INTO relationship_test_native_owners VALUES(1,'no_intro_dat_game')")
        identity(self.db, relationship_id=3)
        derived(self.db, relationship_id=3, from_target=1, to_target=2)
        rationale(self.db, relationship_id=3)
        with self.assertRaisesRegex(sqlite3.IntegrityError, "relationship evidence"):
            self.db.execute("INSERT INTO catalog_relationship_evidence_publications VALUES(3,'rationale')")

    def test_actual_mame_and_no_intro_owner_guards_feed_endpoint_readiness(self):
        db = native_connection()
        self.addCleanup(db.close)
        self.assertEqual(db.execute("SELECT * FROM candidate_relationship_closure_audit").fetchall(), [])
        with self.assertRaisesRegex(sqlite3.IntegrityError, "candidate native kind or ancestry"):
            db.execute("UPDATE mame_machines SET set_id=2 WHERE set_id=1")
        with self.assertRaisesRegex(sqlite3.IntegrityError, "candidate native kind or ancestry"):
            db.execute("UPDATE no_intro_dat_games SET set_id=1 WHERE set_id=2")
        # Bypass the native update guard to model damaged published native
        # ownership; the separately generated relationship audit must catch it.
        db.execute("DROP TRIGGER candidate_native_mame_machines_update")
        db.execute("UPDATE mame_machines SET set_id=2 WHERE set_id=1")
        owners = {row[1] for row in db.execute(
            "SELECT problem,owner_id,edition_id FROM candidate_relationship_closure_audit")}
        self.assertEqual(owners, {1, 2})

    def test_other_typed_endpoints_join_their_backing_fact_rows(self):
        db = self.db
        db.execute("INSERT INTO file_id_registries VALUES(1,?)", (b'r' * 16,))
        db.execute("INSERT INTO shared_catalog_files VALUES(?,1)", (b'f' * 16,))
        db.execute("INSERT INTO hash_values VALUES(1,'md5',?)", (b'h' * 16,))
        for target_id, kind in ((3, 'shared_file'), (4, 'declared_hash'),
                                (5, 'unresolved_catalog'), (6, 'external_record')):
            db.execute("INSERT INTO catalog_relationship_targets VALUES(?,?)", (target_id, kind))
        db.execute("INSERT INTO shared_file_targets(target_id,file_uuid) VALUES(3,?)", (b'f' * 16,))
        db.execute("INSERT INTO declared_hash_targets(target_id,hash_id) VALUES(4,1)")
        db.execute("INSERT INTO unresolved_catalog_targets"
                   "(target_id,edition_id,record_kind,declared_name) VALUES(5,1,'catalog_set','missing')")
        db.execute("INSERT INTO external_catalog_targets(target_id,namespace,declared_key) "
                   "VALUES(6,'test','external')")
        for relationship_id, target_id in zip(range(10, 14), range(3, 7)):
            identity(db, relationship_id=relationship_id)
            derived(db, relationship_id=relationship_id, from_target=1, to_target=target_id)
            rationale(db, relationship_id)
            db.execute("INSERT INTO catalog_relationship_evidence_publications VALUES(?,'rationale')",
                       (relationship_id,))
        self.assertEqual(db.execute("SELECT * FROM candidate_relationship_closure_audit").fetchall(), [])

        # These rows normally have restrictive foreign keys. Disable enforcement
        # only to model independently corrupted backing facts after publication.
        db.commit()
        db.execute("PRAGMA foreign_keys=OFF")
        db.execute("DELETE FROM shared_catalog_files WHERE file_uuid=?", (b'f' * 16,))
        db.execute("DROP TRIGGER candidate_hash_value_immutable_delete")
        db.execute("DELETE FROM hash_values WHERE hash_id=1")
        db.execute("DROP TRIGGER relationship_closure_unresolved_catalog_targets_update")
        db.execute("UPDATE unresolved_catalog_targets SET edition_id=99 WHERE target_id=5")
        owners = {row[1] for row in db.execute(
            "SELECT problem,owner_id,edition_id FROM candidate_relationship_closure_audit")}
        self.assertEqual(owners, {10, 11, 12})

    def test_independent_audit_finds_endpoint_corruption_after_guard_bypass(self):
        identity(self.db)
        derived(self.db)
        complete_second_target(self.db)
        rationale(self.db)
        self.db.execute("INSERT INTO catalog_relationship_evidence_publications VALUES(1,'rationale')")
        self.db.execute("DROP TRIGGER relationship_closure_catalog_set_targets_delete")
        self.db.execute("DELETE FROM catalog_set_targets WHERE target_id=1")
        problems = self.db.execute("SELECT problem FROM candidate_relationship_closure_audit").fetchall()
        self.assertTrue(any("endpoint" in problem[0] for problem in problems), problems)


if __name__ == "__main__":
    unittest.main()
