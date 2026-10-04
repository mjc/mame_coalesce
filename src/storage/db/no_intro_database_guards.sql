-- Guard native owners even for clients that disable SQLite FK checking.
-- Parse-count insertion seals one unpublished snapshot after all parser rows.

-- Run immutability must seek native links directly, including orphan evidence
-- left by FK-off clients whose diagnostic or import-run rows are missing.
CREATE INDEX no_intro_export_diagnostics_by_run
    ON no_intro_export_diagnostics(run_key);
CREATE INDEX no_intro_export_header_diagnostics_by_run
    ON no_intro_export_header_diagnostics(run_key);
CREATE INDEX no_intro_header_field_diagnostics_by_run
    ON no_intro_header_field_diagnostics(run_key);
CREATE INDEX no_intro_game_diagnostics_by_run
    ON no_intro_game_diagnostics(run_key);
CREATE INDEX no_intro_archive_diagnostics_by_run
    ON no_intro_archive_diagnostics(run_key);
CREATE INDEX no_intro_dump_source_diagnostics_by_run
    ON no_intro_dump_source_diagnostics(run_key);
CREATE INDEX no_intro_dump_details_diagnostics_by_run
    ON no_intro_dump_details_diagnostics(run_key);
CREATE INDEX no_intro_dump_serials_diagnostics_by_run
    ON no_intro_dump_serials_diagnostics(run_key);
CREATE INDEX no_intro_dump_file_diagnostics_by_run
    ON no_intro_dump_file_diagnostics(run_key);
CREATE INDEX no_intro_release_diagnostics_by_run
    ON no_intro_release_diagnostics(run_key);
CREATE INDEX no_intro_release_details_diagnostics_by_run
    ON no_intro_release_details_diagnostics(run_key);
CREATE INDEX no_intro_release_serials_diagnostics_by_run
    ON no_intro_release_serials_diagnostics(run_key);
CREATE INDEX no_intro_release_file_diagnostics_by_run
    ON no_intro_release_file_diagnostics(run_key);

CREATE TRIGGER import_diagnostics_validate_storage BEFORE INSERT ON import_diagnostics
WHEN typeof(NEW.diagnostic_key)<>'text' OR typeof(NEW.run_key)<>'text'
 OR typeof(NEW.document_key)<>'text' OR typeof(NEW.severity)<>'text'
 OR typeof(NEW.code)<>'text' OR typeof(NEW.message)<>'text'
 OR (NEW.record_kind IS NOT NULL AND typeof(NEW.record_kind)<>'text')
 OR (NEW.record_name IS NOT NULL AND typeof(NEW.record_name)<>'text')
 OR (NEW.field_name IS NOT NULL AND typeof(NEW.field_name)<>'text')
 OR (NEW.offending_text IS NOT NULL AND typeof(NEW.offending_text)<>'text')
 OR (NEW.source_line IS NOT NULL AND typeof(NEW.source_line)<>'integer')
 OR (NEW.source_column IS NOT NULL AND typeof(NEW.source_column)<>'integer')
BEGIN SELECT RAISE(ABORT,'diagnostic fields have invalid SQLite storage classes'); END;

-- This view centralizes the run/document/catalog/interpretation contract. It
-- contains no owner discriminator or persisted copy of any native key.
CREATE VIEW no_intro_valid_recovery_diagnostics AS
SELECT d.diagnostic_key, d.run_key, r.snapshot_key, d.source_line, d.source_column
FROM import_diagnostics d
JOIN import_runs r ON r.run_key=d.run_key AND r.document_key=d.document_key
JOIN catalog_snapshots s ON s.snapshot_key=r.snapshot_key
    AND s.catalog_key=r.catalog_key AND s.document_key=r.document_key
    AND s.interpretation_key=r.interpretation_key
JOIN parser_interpretations p ON p.interpretation_key=s.interpretation_key
JOIN no_intro_exports e ON e.snapshot_key=s.snapshot_key
WHERE typeof(d.diagnostic_key)='text' AND typeof(d.run_key)='text'
  AND typeof(d.document_key)='text' AND typeof(d.code)='text'
  AND typeof(d.message)='text' AND d.severity='warning'
  AND d.code='xml_nul_recovered' AND r.status='succeeded'
  AND p.format IN ('no-intro-database-xml-compatible',
                   'no-intro-database-xml-nul-compatible')
  AND d.coordinate_view='transport_decoded_xml_text'
  AND d.column_convention='unicode_scalar_1based'
  AND typeof(d.source_line)='integer' AND d.source_line>0
  AND typeof(d.source_column)='integer' AND d.source_column>0;

CREATE VIEW no_intro_diagnostic_link_keys AS
SELECT diagnostic_key, run_key FROM no_intro_export_diagnostics
UNION ALL SELECT diagnostic_key, run_key FROM no_intro_export_header_diagnostics
UNION ALL SELECT diagnostic_key, run_key FROM no_intro_header_field_diagnostics
UNION ALL SELECT diagnostic_key, run_key FROM no_intro_game_diagnostics
UNION ALL SELECT diagnostic_key, run_key FROM no_intro_archive_diagnostics
UNION ALL SELECT diagnostic_key, run_key FROM no_intro_dump_source_diagnostics
UNION ALL SELECT diagnostic_key, run_key FROM no_intro_dump_details_diagnostics
UNION ALL SELECT diagnostic_key, run_key FROM no_intro_dump_serials_diagnostics
UNION ALL SELECT diagnostic_key, run_key FROM no_intro_dump_file_diagnostics
UNION ALL SELECT diagnostic_key, run_key FROM no_intro_release_diagnostics
UNION ALL SELECT diagnostic_key, run_key FROM no_intro_release_details_diagnostics
UNION ALL SELECT diagnostic_key, run_key FROM no_intro_release_serials_diagnostics
UNION ALL SELECT diagnostic_key, run_key FROM no_intro_release_file_diagnostics;

CREATE TRIGGER no_intro_owned_diagnostics_immutable_update BEFORE UPDATE ON import_diagnostics
WHEN EXISTS(SELECT 1 FROM no_intro_diagnostic_link_keys WHERE diagnostic_key=OLD.diagnostic_key)
 OR EXISTS(SELECT 1 FROM no_intro_diagnostic_link_keys WHERE diagnostic_key=NEW.diagnostic_key)
BEGIN SELECT RAISE(ABORT,'diagnostic evidence with a native owner is immutable'); END;
CREATE TRIGGER no_intro_owned_diagnostics_immutable_delete BEFORE DELETE ON import_diagnostics
WHEN EXISTS(SELECT 1 FROM no_intro_diagnostic_link_keys WHERE diagnostic_key=OLD.diagnostic_key)
BEGIN SELECT RAISE(ABORT,'diagnostic evidence with a native owner is immutable'); END;
CREATE TRIGGER no_intro_owned_diagnostics_reject_replace BEFORE INSERT ON import_diagnostics
WHEN EXISTS(SELECT 1 FROM no_intro_diagnostic_link_keys WHERE diagnostic_key=NEW.diagnostic_key)
 OR EXISTS(
    SELECT 1 FROM import_diagnostics old
    JOIN no_intro_diagnostic_link_keys linked
      ON linked.diagnostic_key=old.diagnostic_key AND linked.run_key=old.run_key
    WHERE old.run_key=NEW.run_key AND old.diagnostic_order=NEW.diagnostic_order
 )
BEGIN SELECT RAISE(ABORT,'diagnostic evidence with a native owner is immutable'); END;

CREATE TRIGGER no_intro_owned_diagnostic_runs_immutable_update BEFORE UPDATE ON import_runs
WHEN EXISTS(SELECT 1 FROM no_intro_diagnostic_link_keys WHERE run_key=OLD.run_key)
 OR EXISTS(SELECT 1 FROM no_intro_diagnostic_link_keys WHERE run_key=NEW.run_key)
BEGIN SELECT RAISE(ABORT,'import runs with native diagnostic evidence are immutable'); END;
CREATE TRIGGER no_intro_owned_diagnostic_runs_immutable_delete BEFORE DELETE ON import_runs
WHEN EXISTS(SELECT 1 FROM no_intro_diagnostic_link_keys WHERE run_key=OLD.run_key)
BEGIN SELECT RAISE(ABORT,'import runs with native diagnostic evidence are immutable'); END;
CREATE TRIGGER no_intro_owned_diagnostic_runs_reject_replace BEFORE INSERT ON import_runs
WHEN EXISTS(SELECT 1 FROM no_intro_diagnostic_link_keys WHERE run_key=NEW.run_key)
BEGIN SELECT RAISE(ABORT,'import runs with native diagnostic evidence are immutable'); END;

CREATE TRIGGER no_intro_export_diagnostics_require_matching_owner BEFORE INSERT ON no_intro_export_diagnostics
WHEN NOT EXISTS (
    SELECT 1 FROM no_intro_valid_recovery_diagnostics d
    JOIN no_intro_exports owner ON owner.snapshot_key=d.snapshot_key
    WHERE d.diagnostic_key=NEW.diagnostic_key AND d.run_key=NEW.run_key
      AND d.snapshot_key=NEW.snapshot_key
      AND (d.source_line>1 OR (d.source_line=1 AND d.source_column>=1))
      AND (d.source_line<owner.document_end_line
           OR (d.source_line=owner.document_end_line AND d.source_column<owner.document_end_column))
)
BEGIN SELECT RAISE(ABORT,'diagnostic does not belong to this export document extent'); END;

CREATE TRIGGER no_intro_export_header_diagnostics_require_matching_owner BEFORE INSERT ON no_intro_export_header_diagnostics
WHEN NOT EXISTS (
    SELECT 1 FROM no_intro_valid_recovery_diagnostics d
    JOIN no_intro_export_headers owner ON owner.snapshot_key=d.snapshot_key
    WHERE d.diagnostic_key=NEW.diagnostic_key AND d.run_key=NEW.run_key
      AND d.snapshot_key=NEW.snapshot_key
      AND (d.source_line>owner.source_line OR (d.source_line=owner.source_line AND d.source_column>=owner.source_column))
      AND (d.source_line<owner.source_end_line OR (d.source_line=owner.source_end_line AND d.source_column<owner.source_end_column))
)
BEGIN SELECT RAISE(ABORT,'diagnostic does not belong to this export header extent'); END;

CREATE TRIGGER no_intro_header_field_diagnostics_require_matching_owner BEFORE INSERT ON no_intro_header_field_diagnostics
WHEN NOT EXISTS (
    SELECT 1 FROM no_intro_valid_recovery_diagnostics d
    JOIN no_intro_header_fields owner ON owner.snapshot_key=d.snapshot_key
      AND owner.source_order=NEW.source_order
    JOIN no_intro_export_headers header ON header.snapshot_key=owner.snapshot_key
    WHERE d.diagnostic_key=NEW.diagnostic_key AND d.run_key=NEW.run_key
      AND d.snapshot_key=NEW.snapshot_key
      AND (d.source_line>owner.source_line OR (d.source_line=owner.source_line AND d.source_column>=owner.source_column))
      AND (d.source_line<owner.source_end_line OR (d.source_line=owner.source_end_line AND d.source_column<owner.source_end_column))
)
BEGIN SELECT RAISE(ABORT,'diagnostic does not belong to this export header field extent'); END;

CREATE TRIGGER no_intro_game_diagnostics_require_matching_owner BEFORE INSERT ON no_intro_game_diagnostics
WHEN NOT EXISTS (
    SELECT 1 FROM no_intro_valid_recovery_diagnostics d
    JOIN no_intro_database_games owner ON owner.set_id=NEW.set_id
    JOIN catalog_sets game ON game.set_id=owner.set_id
    JOIN catalog_set_groups parent ON parent.set_group_id=game.set_group_id
      AND parent.snapshot_key=d.snapshot_key AND parent.kind='root'
    WHERE d.diagnostic_key=NEW.diagnostic_key AND d.run_key=NEW.run_key
      AND d.snapshot_key=NEW.snapshot_key
      AND game.source_element_kind='no_intro_database_game'
      AND (d.source_line>game.source_line OR (d.source_line=game.source_line AND d.source_column>=game.source_column))
      AND (d.source_line<owner.source_end_line OR (d.source_line=owner.source_end_line AND d.source_column<owner.source_end_column))
)
BEGIN SELECT RAISE(ABORT,'diagnostic does not belong to this No-Intro game extent'); END;

CREATE TRIGGER no_intro_archive_diagnostics_require_matching_owner BEFORE INSERT ON no_intro_archive_diagnostics
WHEN NOT EXISTS (
    SELECT 1 FROM no_intro_valid_recovery_diagnostics d
    JOIN no_intro_archive_descriptions owner ON owner.archive_id=NEW.archive_id
    JOIN no_intro_database_games game ON game.set_id=owner.set_id
    JOIN catalog_sets native ON native.set_id=game.set_id
    JOIN catalog_set_groups parent ON parent.set_group_id=native.set_group_id
      AND parent.snapshot_key=d.snapshot_key AND parent.kind='root'
    WHERE d.diagnostic_key=NEW.diagnostic_key AND d.run_key=NEW.run_key
      AND d.snapshot_key=NEW.snapshot_key
      AND native.source_element_kind='no_intro_database_game'
      AND (d.source_line>owner.source_line OR (d.source_line=owner.source_line AND d.source_column>=owner.source_column))
      AND (d.source_line<owner.source_end_line OR (d.source_line=owner.source_end_line AND d.source_column<owner.source_end_column))
)
BEGIN SELECT RAISE(ABORT,'diagnostic does not belong to this archive description extent'); END;

CREATE TRIGGER no_intro_dump_source_diagnostics_require_matching_owner BEFORE INSERT ON no_intro_dump_source_diagnostics
WHEN NOT EXISTS (
    SELECT 1 FROM no_intro_valid_recovery_diagnostics d
    JOIN no_intro_dump_sources owner ON owner.dump_source_id=NEW.dump_source_id
    JOIN no_intro_database_games game ON game.set_id=owner.set_id
    JOIN catalog_sets native ON native.set_id=game.set_id
    JOIN catalog_set_groups parent ON parent.set_group_id=native.set_group_id
      AND parent.snapshot_key=d.snapshot_key AND parent.kind='root'
    WHERE d.diagnostic_key=NEW.diagnostic_key AND d.run_key=NEW.run_key
      AND d.snapshot_key=NEW.snapshot_key
      AND native.source_element_kind='no_intro_database_game'
      AND (d.source_line>owner.source_line OR (d.source_line=owner.source_line AND d.source_column>=owner.source_column))
      AND (d.source_line<owner.source_end_line OR (d.source_line=owner.source_end_line AND d.source_column<owner.source_end_column))
)
BEGIN SELECT RAISE(ABORT,'diagnostic does not belong to this No-Intro dump source extent'); END;

CREATE TRIGGER no_intro_dump_details_diagnostics_require_matching_owner BEFORE INSERT ON no_intro_dump_details_diagnostics
WHEN NOT EXISTS (
    SELECT 1 FROM no_intro_valid_recovery_diagnostics d
    JOIN no_intro_dump_details owner ON owner.dump_source_id=NEW.dump_source_id
    JOIN no_intro_dump_sources parent_owner USING(dump_source_id)
    JOIN no_intro_database_games game ON game.set_id=parent_owner.set_id
    JOIN catalog_sets native ON native.set_id=game.set_id
    JOIN catalog_set_groups parent ON parent.set_group_id=native.set_group_id
      AND parent.snapshot_key=d.snapshot_key AND parent.kind='root'
    WHERE d.diagnostic_key=NEW.diagnostic_key AND d.run_key=NEW.run_key
      AND d.snapshot_key=NEW.snapshot_key
      AND native.source_element_kind='no_intro_database_game'
      AND (d.source_line>owner.source_line OR (d.source_line=owner.source_line AND d.source_column>=owner.source_column))
      AND (d.source_line<owner.source_end_line OR (d.source_line=owner.source_end_line AND d.source_column<owner.source_end_column))
)
BEGIN SELECT RAISE(ABORT,'diagnostic does not belong to this No-Intro dump details extent'); END;

CREATE TRIGGER no_intro_dump_serials_diagnostics_require_matching_owner BEFORE INSERT ON no_intro_dump_serials_diagnostics
WHEN NOT EXISTS (
    SELECT 1 FROM no_intro_valid_recovery_diagnostics d
    JOIN no_intro_dump_serials owner ON owner.dump_source_id=NEW.dump_source_id
    JOIN no_intro_dump_sources parent_owner USING(dump_source_id)
    JOIN no_intro_database_games game ON game.set_id=parent_owner.set_id
    JOIN catalog_sets native ON native.set_id=game.set_id
    JOIN catalog_set_groups parent ON parent.set_group_id=native.set_group_id
      AND parent.snapshot_key=d.snapshot_key AND parent.kind='root'
    WHERE d.diagnostic_key=NEW.diagnostic_key AND d.run_key=NEW.run_key
      AND d.snapshot_key=NEW.snapshot_key
      AND native.source_element_kind='no_intro_database_game'
      AND (d.source_line>owner.source_line OR (d.source_line=owner.source_line AND d.source_column>=owner.source_column))
      AND (d.source_line<owner.source_end_line OR (d.source_line=owner.source_end_line AND d.source_column<owner.source_end_column))
)
BEGIN SELECT RAISE(ABORT,'diagnostic does not belong to this No-Intro dump serials extent'); END;

CREATE TRIGGER no_intro_dump_file_diagnostics_require_matching_owner BEFORE INSERT ON no_intro_dump_file_diagnostics
WHEN NOT EXISTS (
    SELECT 1 FROM no_intro_valid_recovery_diagnostics d
    JOIN no_intro_dump_files owner ON owner.occurrence_id=NEW.occurrence_id
    JOIN no_intro_dump_sources parent_owner
      ON parent_owner.dump_source_id=owner.dump_source_id AND parent_owner.set_id=owner.set_id
    JOIN no_intro_database_games game ON game.set_id=parent_owner.set_id
    JOIN catalog_sets native ON native.set_id=game.set_id
    JOIN catalog_set_groups parent ON parent.set_group_id=native.set_group_id
      AND parent.snapshot_key=d.snapshot_key AND parent.kind='root'
    WHERE d.diagnostic_key=NEW.diagnostic_key AND d.run_key=NEW.run_key
      AND d.snapshot_key=NEW.snapshot_key
      AND native.source_element_kind='no_intro_database_game'
      AND owner.claim_kind='no_intro_database_source_file'
      AND (d.source_line>owner.source_line OR (d.source_line=owner.source_line AND d.source_column>=owner.source_column))
      AND (d.source_line<owner.source_end_line OR (d.source_line=owner.source_end_line AND d.source_column<owner.source_end_column))
)
BEGIN SELECT RAISE(ABORT,'diagnostic does not belong to this No-Intro dump file extent'); END;

CREATE TRIGGER no_intro_release_diagnostics_require_matching_owner BEFORE INSERT ON no_intro_release_diagnostics
WHEN NOT EXISTS (
    SELECT 1 FROM no_intro_valid_recovery_diagnostics d
    JOIN no_intro_releases owner ON owner.release_id=NEW.release_id
    JOIN no_intro_database_games game ON game.set_id=owner.set_id
    JOIN catalog_sets native ON native.set_id=game.set_id
    JOIN catalog_set_groups parent ON parent.set_group_id=native.set_group_id
      AND parent.snapshot_key=d.snapshot_key AND parent.kind='root'
    WHERE d.diagnostic_key=NEW.diagnostic_key AND d.run_key=NEW.run_key
      AND d.snapshot_key=NEW.snapshot_key
      AND native.source_element_kind='no_intro_database_game'
      AND (d.source_line>owner.source_line OR (d.source_line=owner.source_line AND d.source_column>=owner.source_column))
      AND (d.source_line<owner.source_end_line OR (d.source_line=owner.source_end_line AND d.source_column<owner.source_end_column))
)
BEGIN SELECT RAISE(ABORT,'diagnostic does not belong to this No-Intro release extent'); END;

CREATE TRIGGER no_intro_release_details_diagnostics_require_matching_owner BEFORE INSERT ON no_intro_release_details_diagnostics
WHEN NOT EXISTS (
    SELECT 1 FROM no_intro_valid_recovery_diagnostics d
    JOIN no_intro_release_details owner ON owner.release_id=NEW.release_id
    JOIN no_intro_releases parent_owner USING(release_id)
    JOIN no_intro_database_games game ON game.set_id=parent_owner.set_id
    JOIN catalog_sets native ON native.set_id=game.set_id
    JOIN catalog_set_groups parent ON parent.set_group_id=native.set_group_id
      AND parent.snapshot_key=d.snapshot_key AND parent.kind='root'
    WHERE d.diagnostic_key=NEW.diagnostic_key AND d.run_key=NEW.run_key
      AND d.snapshot_key=NEW.snapshot_key
      AND native.source_element_kind='no_intro_database_game'
      AND (d.source_line>owner.source_line OR (d.source_line=owner.source_line AND d.source_column>=owner.source_column))
      AND (d.source_line<owner.source_end_line OR (d.source_line=owner.source_end_line AND d.source_column<owner.source_end_column))
)
BEGIN SELECT RAISE(ABORT,'diagnostic does not belong to this No-Intro release details extent'); END;

CREATE TRIGGER no_intro_release_serials_diagnostics_require_matching_owner BEFORE INSERT ON no_intro_release_serials_diagnostics
WHEN NOT EXISTS (
    SELECT 1 FROM no_intro_valid_recovery_diagnostics d
    JOIN no_intro_release_serials owner ON owner.release_id=NEW.release_id
    JOIN no_intro_releases parent_owner USING(release_id)
    JOIN no_intro_database_games game ON game.set_id=parent_owner.set_id
    JOIN catalog_sets native ON native.set_id=game.set_id
    JOIN catalog_set_groups parent ON parent.set_group_id=native.set_group_id
      AND parent.snapshot_key=d.snapshot_key AND parent.kind='root'
    WHERE d.diagnostic_key=NEW.diagnostic_key AND d.run_key=NEW.run_key
      AND d.snapshot_key=NEW.snapshot_key
      AND native.source_element_kind='no_intro_database_game'
      AND (d.source_line>owner.source_line OR (d.source_line=owner.source_line AND d.source_column>=owner.source_column))
      AND (d.source_line<owner.source_end_line OR (d.source_line=owner.source_end_line AND d.source_column<owner.source_end_column))
)
BEGIN SELECT RAISE(ABORT,'diagnostic does not belong to this No-Intro release serials extent'); END;

CREATE TRIGGER no_intro_release_file_diagnostics_require_matching_owner BEFORE INSERT ON no_intro_release_file_diagnostics
WHEN NOT EXISTS (
    SELECT 1 FROM no_intro_valid_recovery_diagnostics d
    JOIN no_intro_release_files owner ON owner.occurrence_id=NEW.occurrence_id
    JOIN no_intro_releases parent_owner
      ON parent_owner.release_id=owner.release_id AND parent_owner.set_id=owner.set_id
    JOIN no_intro_database_games game ON game.set_id=parent_owner.set_id
    JOIN catalog_sets native ON native.set_id=game.set_id
    JOIN catalog_set_groups parent ON parent.set_group_id=native.set_group_id
      AND parent.snapshot_key=d.snapshot_key AND parent.kind='root'
    WHERE d.diagnostic_key=NEW.diagnostic_key AND d.run_key=NEW.run_key
      AND d.snapshot_key=NEW.snapshot_key
      AND native.source_element_kind='no_intro_database_game'
      AND owner.claim_kind='no_intro_database_release_file'
      AND (d.source_line>owner.source_line OR (d.source_line=owner.source_line AND d.source_column>=owner.source_column))
      AND (d.source_line<owner.source_end_line OR (d.source_line=owner.source_end_line AND d.source_column<owner.source_end_column))
)
BEGIN SELECT RAISE(ABORT,'diagnostic does not belong to this No-Intro release file extent'); END;

CREATE TRIGGER no_intro_export_diagnostics_immutable_update BEFORE UPDATE ON no_intro_export_diagnostics
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;
CREATE TRIGGER no_intro_export_diagnostics_immutable_delete BEFORE DELETE ON no_intro_export_diagnostics
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;
CREATE TRIGGER no_intro_export_diagnostics_reject_replace BEFORE INSERT ON no_intro_export_diagnostics
WHEN EXISTS(SELECT 1 FROM no_intro_export_diagnostics WHERE diagnostic_key=NEW.diagnostic_key AND snapshot_key=NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;

CREATE TRIGGER no_intro_export_header_diagnostics_immutable_update BEFORE UPDATE ON no_intro_export_header_diagnostics
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;
CREATE TRIGGER no_intro_export_header_diagnostics_immutable_delete BEFORE DELETE ON no_intro_export_header_diagnostics
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;
CREATE TRIGGER no_intro_export_header_diagnostics_reject_replace BEFORE INSERT ON no_intro_export_header_diagnostics
WHEN EXISTS(SELECT 1 FROM no_intro_export_header_diagnostics WHERE diagnostic_key=NEW.diagnostic_key AND snapshot_key=NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;

CREATE TRIGGER no_intro_header_field_diagnostics_immutable_update BEFORE UPDATE ON no_intro_header_field_diagnostics
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;
CREATE TRIGGER no_intro_header_field_diagnostics_immutable_delete BEFORE DELETE ON no_intro_header_field_diagnostics
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;
CREATE TRIGGER no_intro_header_field_diagnostics_reject_replace BEFORE INSERT ON no_intro_header_field_diagnostics
WHEN EXISTS(SELECT 1 FROM no_intro_header_field_diagnostics WHERE diagnostic_key=NEW.diagnostic_key AND snapshot_key=NEW.snapshot_key AND source_order=NEW.source_order)
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;

CREATE TRIGGER no_intro_game_diagnostics_immutable_update BEFORE UPDATE ON no_intro_game_diagnostics
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;
CREATE TRIGGER no_intro_game_diagnostics_immutable_delete BEFORE DELETE ON no_intro_game_diagnostics
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;
CREATE TRIGGER no_intro_game_diagnostics_reject_replace BEFORE INSERT ON no_intro_game_diagnostics
WHEN EXISTS(SELECT 1 FROM no_intro_game_diagnostics WHERE diagnostic_key=NEW.diagnostic_key AND set_id=NEW.set_id)
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;

CREATE TRIGGER no_intro_archive_diagnostics_immutable_update BEFORE UPDATE ON no_intro_archive_diagnostics
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;
CREATE TRIGGER no_intro_archive_diagnostics_immutable_delete BEFORE DELETE ON no_intro_archive_diagnostics
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;
CREATE TRIGGER no_intro_archive_diagnostics_reject_replace BEFORE INSERT ON no_intro_archive_diagnostics
WHEN EXISTS(SELECT 1 FROM no_intro_archive_diagnostics WHERE diagnostic_key=NEW.diagnostic_key AND archive_id=NEW.archive_id)
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;

CREATE TRIGGER no_intro_dump_source_diagnostics_immutable_update BEFORE UPDATE ON no_intro_dump_source_diagnostics
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;
CREATE TRIGGER no_intro_dump_source_diagnostics_immutable_delete BEFORE DELETE ON no_intro_dump_source_diagnostics
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;
CREATE TRIGGER no_intro_dump_source_diagnostics_reject_replace BEFORE INSERT ON no_intro_dump_source_diagnostics
WHEN EXISTS(SELECT 1 FROM no_intro_dump_source_diagnostics WHERE diagnostic_key=NEW.diagnostic_key AND dump_source_id=NEW.dump_source_id)
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;

CREATE TRIGGER no_intro_dump_details_diagnostics_immutable_update BEFORE UPDATE ON no_intro_dump_details_diagnostics
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;
CREATE TRIGGER no_intro_dump_details_diagnostics_immutable_delete BEFORE DELETE ON no_intro_dump_details_diagnostics
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;
CREATE TRIGGER no_intro_dump_details_diagnostics_reject_replace BEFORE INSERT ON no_intro_dump_details_diagnostics
WHEN EXISTS(SELECT 1 FROM no_intro_dump_details_diagnostics WHERE diagnostic_key=NEW.diagnostic_key AND dump_source_id=NEW.dump_source_id)
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;

CREATE TRIGGER no_intro_dump_serials_diagnostics_immutable_update BEFORE UPDATE ON no_intro_dump_serials_diagnostics
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;
CREATE TRIGGER no_intro_dump_serials_diagnostics_immutable_delete BEFORE DELETE ON no_intro_dump_serials_diagnostics
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;
CREATE TRIGGER no_intro_dump_serials_diagnostics_reject_replace BEFORE INSERT ON no_intro_dump_serials_diagnostics
WHEN EXISTS(SELECT 1 FROM no_intro_dump_serials_diagnostics WHERE diagnostic_key=NEW.diagnostic_key AND dump_source_id=NEW.dump_source_id)
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;

CREATE TRIGGER no_intro_dump_file_diagnostics_immutable_update BEFORE UPDATE ON no_intro_dump_file_diagnostics
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;
CREATE TRIGGER no_intro_dump_file_diagnostics_immutable_delete BEFORE DELETE ON no_intro_dump_file_diagnostics
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;
CREATE TRIGGER no_intro_dump_file_diagnostics_reject_replace BEFORE INSERT ON no_intro_dump_file_diagnostics
WHEN EXISTS(SELECT 1 FROM no_intro_dump_file_diagnostics WHERE diagnostic_key=NEW.diagnostic_key AND occurrence_id=NEW.occurrence_id)
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;

CREATE TRIGGER no_intro_release_diagnostics_immutable_update BEFORE UPDATE ON no_intro_release_diagnostics
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;
CREATE TRIGGER no_intro_release_diagnostics_immutable_delete BEFORE DELETE ON no_intro_release_diagnostics
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;
CREATE TRIGGER no_intro_release_diagnostics_reject_replace BEFORE INSERT ON no_intro_release_diagnostics
WHEN EXISTS(SELECT 1 FROM no_intro_release_diagnostics WHERE diagnostic_key=NEW.diagnostic_key AND release_id=NEW.release_id)
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;

CREATE TRIGGER no_intro_release_details_diagnostics_immutable_update BEFORE UPDATE ON no_intro_release_details_diagnostics
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;
CREATE TRIGGER no_intro_release_details_diagnostics_immutable_delete BEFORE DELETE ON no_intro_release_details_diagnostics
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;
CREATE TRIGGER no_intro_release_details_diagnostics_reject_replace BEFORE INSERT ON no_intro_release_details_diagnostics
WHEN EXISTS(SELECT 1 FROM no_intro_release_details_diagnostics WHERE diagnostic_key=NEW.diagnostic_key AND release_id=NEW.release_id)
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;

CREATE TRIGGER no_intro_release_serials_diagnostics_immutable_update BEFORE UPDATE ON no_intro_release_serials_diagnostics
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;
CREATE TRIGGER no_intro_release_serials_diagnostics_immutable_delete BEFORE DELETE ON no_intro_release_serials_diagnostics
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;
CREATE TRIGGER no_intro_release_serials_diagnostics_reject_replace BEFORE INSERT ON no_intro_release_serials_diagnostics
WHEN EXISTS(SELECT 1 FROM no_intro_release_serials_diagnostics WHERE diagnostic_key=NEW.diagnostic_key AND release_id=NEW.release_id)
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;

CREATE TRIGGER no_intro_release_file_diagnostics_immutable_update BEFORE UPDATE ON no_intro_release_file_diagnostics
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;
CREATE TRIGGER no_intro_release_file_diagnostics_immutable_delete BEFORE DELETE ON no_intro_release_file_diagnostics
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;
CREATE TRIGGER no_intro_release_file_diagnostics_reject_replace BEFORE INSERT ON no_intro_release_file_diagnostics
WHEN EXISTS(SELECT 1 FROM no_intro_release_file_diagnostics WHERE diagnostic_key=NEW.diagnostic_key AND occurrence_id=NEW.occurrence_id)
BEGIN SELECT RAISE(ABORT,'native diagnostic links are immutable'); END;
CREATE TRIGGER no_intro_exports_require_database_format BEFORE INSERT ON no_intro_exports
WHEN NOT EXISTS (SELECT 1 FROM catalog_snapshots s JOIN parser_interpretations p USING(interpretation_key) WHERE s.snapshot_key=NEW.snapshot_key AND p.format IN ('no-intro-database-xml-compatible','no-intro-database-xml-nul-compatible') AND NOT EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=NEW.snapshot_key))
BEGIN SELECT RAISE(ABORT,'No-Intro export requires an unpublished database-export snapshot'); END;

CREATE TRIGGER no_intro_database_games_require_native_identity BEFORE INSERT ON no_intro_database_games
WHEN NOT EXISTS (SELECT 1 FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) JOIN no_intro_exports e USING(snapshot_key) WHERE s.set_id=NEW.set_id AND s.source_element_kind='no_intro_database_game' AND c.kind='root' AND NOT EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=(SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=NEW.set_id)) AND NOT EXISTS (SELECT 1 FROM no_intro_database_parse_counts WHERE snapshot_key=(SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=NEW.set_id)))
BEGIN SELECT RAISE(ABORT,'No-Intro database game requires a real unpublished catalog set'); END;

CREATE TRIGGER no_intro_database_games_require_ordered_extent BEFORE INSERT ON no_intro_database_games
WHEN NOT EXISTS (
    SELECT 1 FROM catalog_sets source
    WHERE source.set_id=NEW.set_id
      AND typeof(source.source_line)='integer' AND typeof(source.source_column)='integer'
      AND typeof(NEW.source_end_line)='integer' AND typeof(NEW.source_end_column)='integer'
      AND (NEW.source_end_line>source.source_line
           OR (NEW.source_end_line=source.source_line AND NEW.source_end_column>source.source_column))
)
BEGIN SELECT RAISE(ABORT,'No-Intro game end must follow its inherited source start'); END;

CREATE TRIGGER no_intro_header_fields_requires_open_native_owner BEFORE INSERT ON no_intro_header_fields
WHEN NOT EXISTS (SELECT 1 FROM no_intro_export_headers h JOIN no_intro_exports e USING(snapshot_key) WHERE h.snapshot_key=NEW.snapshot_key AND e.header_present=1)
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=NEW.snapshot_key)
 OR EXISTS (SELECT 1 FROM no_intro_database_parse_counts WHERE snapshot_key=NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT,'no_intro_header_fields requires its open unpublished No-Intro native owner'); END;

CREATE TRIGGER no_intro_export_headers_require_declared_presence BEFORE INSERT ON no_intro_export_headers
WHEN NOT EXISTS (SELECT 1 FROM no_intro_exports e WHERE e.snapshot_key=NEW.snapshot_key AND e.header_present=1)
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=NEW.snapshot_key)
 OR EXISTS (SELECT 1 FROM no_intro_database_parse_counts WHERE snapshot_key=NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT,'No-Intro header element must match the open export declaration'); END;

CREATE TRIGGER no_intro_archive_descriptions_requires_open_native_owner BEFORE INSERT ON no_intro_archive_descriptions
WHEN NOT EXISTS (SELECT 1 FROM no_intro_database_games WHERE set_id=NEW.set_id) OR NOT EXISTS (SELECT 1 FROM no_intro_exports WHERE snapshot_key=(SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=NEW.set_id))
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=(SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=NEW.set_id))
 OR EXISTS (SELECT 1 FROM no_intro_database_parse_counts WHERE snapshot_key=(SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=NEW.set_id))
BEGIN SELECT RAISE(ABORT,'no_intro_archive_descriptions requires its open unpublished No-Intro native owner'); END;

CREATE TRIGGER no_intro_archive_clone_markers_require_exclusive_owner BEFORE INSERT ON no_intro_archive_clone_markers
WHEN NOT EXISTS (SELECT 1 FROM no_intro_archive_descriptions a JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) JOIN no_intro_exports e USING(snapshot_key) WHERE a.archive_id=NEW.archive_id)
 OR EXISTS (SELECT 1 FROM no_intro_archive_clone_links WHERE archive_id=NEW.archive_id)
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_archive_descriptions a JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE a.archive_id=NEW.archive_id))
 OR EXISTS (SELECT 1 FROM no_intro_database_parse_counts WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_archive_descriptions a JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE a.archive_id=NEW.archive_id))
BEGIN SELECT RAISE(ABORT,'archive clone marker requires an open archive without a clone link'); END;

CREATE TRIGGER no_intro_archive_clone_links_require_exclusive_owner BEFORE INSERT ON no_intro_archive_clone_links
WHEN NOT EXISTS (SELECT 1 FROM no_intro_archive_descriptions a JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) JOIN no_intro_exports e USING(snapshot_key) WHERE a.archive_id=NEW.archive_id)
 OR EXISTS (SELECT 1 FROM no_intro_archive_clone_markers WHERE archive_id=NEW.archive_id)
 OR EXISTS (SELECT 1 FROM reported_catalog_relationship_owner_ids WHERE relationship_id=NEW.relationship_id)
 OR NOT EXISTS (
    SELECT 1 FROM no_intro_archive_descriptions AS archive
    JOIN no_intro_database_games AS native USING(set_id)
    JOIN catalog_sets AS sets ON sets.set_id=native.set_id
    JOIN catalog_set_groups AS groups USING(set_group_id)
    JOIN no_intro_exports AS export USING(snapshot_key)
    JOIN catalog_snapshots AS snapshot USING(snapshot_key)
    JOIN parser_interpretations AS interpretation USING(interpretation_key)
    JOIN no_intro_archive_field_positions AS position
      ON position.archive_id=archive.archive_id AND position.field_kind=30
    JOIN catalog_relationships AS identity ON identity.relationship_id=NEW.relationship_id
    JOIN reported_catalog_relationships AS reported USING(relationship_id)
    WHERE archive.archive_id=NEW.archive_id
      AND sets.source_element_kind='no_intro_database_game' AND groups.kind='root'
      AND interpretation.format IN ('no-intro-database-xml-compatible',
                                     'no-intro-database-xml-nul-compatible')
      AND identity.origin='source' AND identity.snapshot_key=groups.snapshot_key
      AND reported.source_reference_kind='no_intro_database_archive_clone'
      AND NOT EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=groups.snapshot_key)
      AND NOT EXISTS (SELECT 1 FROM no_intro_database_parse_counts WHERE snapshot_key=groups.snapshot_key)
 )
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_archive_descriptions a JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE a.archive_id=NEW.archive_id))
 OR EXISTS (SELECT 1 FROM no_intro_database_parse_counts WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_archive_descriptions a JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE a.archive_id=NEW.archive_id))
BEGIN SELECT RAISE(ABORT,'archive clone link requires its unused reported identity and matching open archive field'); END;

CREATE TRIGGER no_intro_archive_merge_links_require_open_owner BEFORE INSERT ON no_intro_archive_merge_links
WHEN NOT EXISTS (SELECT 1 FROM no_intro_archive_descriptions a JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) JOIN no_intro_exports e USING(snapshot_key) WHERE a.archive_id=NEW.archive_id)
 OR EXISTS (SELECT 1 FROM reported_catalog_relationship_owner_ids WHERE relationship_id=NEW.relationship_id)
 OR NOT EXISTS (
    SELECT 1 FROM no_intro_archive_descriptions AS archive
    JOIN no_intro_database_games AS native USING(set_id)
    JOIN catalog_sets AS sets ON sets.set_id=native.set_id
    JOIN catalog_set_groups AS groups USING(set_group_id)
    JOIN no_intro_exports AS export USING(snapshot_key)
    JOIN catalog_snapshots AS snapshot USING(snapshot_key)
    JOIN parser_interpretations AS interpretation USING(interpretation_key)
    JOIN no_intro_archive_field_positions AS position
      ON position.archive_id=archive.archive_id AND position.field_kind=31
    JOIN catalog_relationships AS identity ON identity.relationship_id=NEW.relationship_id
    JOIN reported_catalog_relationships AS reported USING(relationship_id)
    WHERE archive.archive_id=NEW.archive_id
      AND sets.source_element_kind='no_intro_database_game' AND groups.kind='root'
      AND interpretation.format IN ('no-intro-database-xml-compatible',
                                     'no-intro-database-xml-nul-compatible')
      AND identity.origin='source' AND identity.snapshot_key=groups.snapshot_key
      AND reported.source_reference_kind='no_intro_database_archive_mergeof'
      AND NOT EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=groups.snapshot_key)
      AND NOT EXISTS (SELECT 1 FROM no_intro_database_parse_counts WHERE snapshot_key=groups.snapshot_key)
 )
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_archive_descriptions a JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE a.archive_id=NEW.archive_id))
 OR EXISTS (SELECT 1 FROM no_intro_database_parse_counts WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_archive_descriptions a JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE a.archive_id=NEW.archive_id))
BEGIN SELECT RAISE(ABORT,'archive merge link requires its unused reported identity and matching open archive field'); END;

CREATE TRIGGER no_intro_dump_sources_requires_open_native_owner BEFORE INSERT ON no_intro_dump_sources
WHEN NOT EXISTS (SELECT 1 FROM no_intro_database_games WHERE set_id=NEW.set_id) OR NOT EXISTS (SELECT 1 FROM no_intro_exports WHERE snapshot_key=(SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=NEW.set_id))
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=(SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=NEW.set_id))
 OR EXISTS (SELECT 1 FROM no_intro_database_parse_counts WHERE snapshot_key=(SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=NEW.set_id))
BEGIN SELECT RAISE(ABORT,'no_intro_dump_sources requires its open unpublished No-Intro native owner'); END;

CREATE TRIGGER no_intro_dump_details_requires_open_native_owner BEFORE INSERT ON no_intro_dump_details
WHEN NOT EXISTS (SELECT 1 FROM no_intro_dump_sources WHERE dump_source_id=NEW.dump_source_id) OR NOT EXISTS (SELECT 1 FROM no_intro_exports WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_dump_sources p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.dump_source_id=NEW.dump_source_id))
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_dump_sources p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.dump_source_id=NEW.dump_source_id))
 OR EXISTS (SELECT 1 FROM no_intro_database_parse_counts WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_dump_sources p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.dump_source_id=NEW.dump_source_id))
BEGIN SELECT RAISE(ABORT,'no_intro_dump_details requires its open unpublished No-Intro native owner'); END;

CREATE TRIGGER no_intro_dump_serials_requires_open_native_owner BEFORE INSERT ON no_intro_dump_serials
WHEN NOT EXISTS (SELECT 1 FROM no_intro_dump_sources WHERE dump_source_id=NEW.dump_source_id) OR NOT EXISTS (SELECT 1 FROM no_intro_exports WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_dump_sources p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.dump_source_id=NEW.dump_source_id))
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_dump_sources p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.dump_source_id=NEW.dump_source_id))
 OR EXISTS (SELECT 1 FROM no_intro_database_parse_counts WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_dump_sources p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.dump_source_id=NEW.dump_source_id))
BEGIN SELECT RAISE(ABORT,'no_intro_dump_serials requires its open unpublished No-Intro native owner'); END;

CREATE TRIGGER no_intro_dump_files_requires_open_native_owner BEFORE INSERT ON no_intro_dump_files
WHEN NOT EXISTS (SELECT 1 FROM no_intro_dump_sources p JOIN no_intro_database_games g ON g.set_id=p.set_id JOIN asset_occurrences o ON o.record_id=p.set_id WHERE p.dump_source_id=NEW.dump_source_id AND p.set_id=NEW.set_id AND o.occurrence_id=NEW.occurrence_id AND o.claim_kind='no_intro_database_source_file') OR NOT EXISTS (SELECT 1 FROM no_intro_exports WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_dump_sources p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.dump_source_id=NEW.dump_source_id))
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_dump_sources p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.dump_source_id=NEW.dump_source_id))
 OR EXISTS (SELECT 1 FROM no_intro_database_parse_counts WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_dump_sources p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.dump_source_id=NEW.dump_source_id))
BEGIN SELECT RAISE(ABORT,'no_intro_dump_files requires its open unpublished No-Intro native owner'); END;

CREATE TRIGGER no_intro_releases_requires_open_native_owner BEFORE INSERT ON no_intro_releases
WHEN NOT EXISTS (SELECT 1 FROM no_intro_database_games WHERE set_id=NEW.set_id) OR NOT EXISTS (SELECT 1 FROM no_intro_exports WHERE snapshot_key=(SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=NEW.set_id))
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=(SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=NEW.set_id))
 OR EXISTS (SELECT 1 FROM no_intro_database_parse_counts WHERE snapshot_key=(SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=NEW.set_id))
BEGIN SELECT RAISE(ABORT,'no_intro_releases requires its open unpublished No-Intro native owner'); END;

CREATE TRIGGER no_intro_release_details_requires_open_native_owner BEFORE INSERT ON no_intro_release_details
WHEN NOT EXISTS (SELECT 1 FROM no_intro_releases WHERE release_id=NEW.release_id) OR NOT EXISTS (SELECT 1 FROM no_intro_exports WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_releases p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.release_id=NEW.release_id))
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_releases p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.release_id=NEW.release_id))
 OR EXISTS (SELECT 1 FROM no_intro_database_parse_counts WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_releases p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.release_id=NEW.release_id))
BEGIN SELECT RAISE(ABORT,'no_intro_release_details requires its open unpublished No-Intro native owner'); END;

CREATE TRIGGER no_intro_release_serials_requires_open_native_owner BEFORE INSERT ON no_intro_release_serials
WHEN NOT EXISTS (SELECT 1 FROM no_intro_releases WHERE release_id=NEW.release_id) OR NOT EXISTS (SELECT 1 FROM no_intro_exports WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_releases p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.release_id=NEW.release_id))
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_releases p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.release_id=NEW.release_id))
 OR EXISTS (SELECT 1 FROM no_intro_database_parse_counts WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_releases p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.release_id=NEW.release_id))
BEGIN SELECT RAISE(ABORT,'no_intro_release_serials requires its open unpublished No-Intro native owner'); END;

CREATE TRIGGER no_intro_release_files_requires_open_native_owner BEFORE INSERT ON no_intro_release_files
WHEN NOT EXISTS (SELECT 1 FROM no_intro_releases p JOIN no_intro_database_games g ON g.set_id=p.set_id JOIN asset_occurrences o ON o.record_id=p.set_id WHERE p.release_id=NEW.release_id AND p.set_id=NEW.set_id AND o.occurrence_id=NEW.occurrence_id AND o.claim_kind='no_intro_database_release_file') OR NOT EXISTS (SELECT 1 FROM no_intro_exports WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_releases p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.release_id=NEW.release_id))
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_releases p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.release_id=NEW.release_id))
 OR EXISTS (SELECT 1 FROM no_intro_database_parse_counts WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_releases p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.release_id=NEW.release_id))
BEGIN SELECT RAISE(ABORT,'no_intro_release_files requires its open unpublished No-Intro native owner'); END;

CREATE TRIGGER no_intro_archive_field_positions_requires_open_native_owner BEFORE INSERT ON no_intro_archive_field_positions
WHEN NOT EXISTS (SELECT 1 FROM no_intro_archive_descriptions WHERE archive_id=NEW.archive_id) OR NOT EXISTS (SELECT 1 FROM no_intro_exports e JOIN catalog_set_groups c USING(snapshot_key) JOIN catalog_sets s USING(set_group_id) JOIN no_intro_archive_descriptions a USING(set_id) WHERE a.archive_id=NEW.archive_id AND e.snapshot_key=c.snapshot_key)
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_archive_descriptions a JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE a.archive_id=NEW.archive_id))
 OR EXISTS (SELECT 1 FROM no_intro_database_parse_counts WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_archive_descriptions a JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE a.archive_id=NEW.archive_id))
BEGIN SELECT RAISE(ABORT,'no_intro_archive_field_positions requires its open unpublished No-Intro native owner'); END;

CREATE TRIGGER no_intro_dump_details_field_positions_requires_open_native_owner BEFORE INSERT ON no_intro_dump_details_field_positions
WHEN NOT EXISTS (SELECT 1 FROM no_intro_dump_details WHERE dump_source_id=NEW.dump_source_id) OR NOT EXISTS (SELECT 1 FROM no_intro_exports WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_dump_sources p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.dump_source_id=NEW.dump_source_id))
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_dump_sources p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.dump_source_id=NEW.dump_source_id))
 OR EXISTS (SELECT 1 FROM no_intro_database_parse_counts WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_dump_sources p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.dump_source_id=NEW.dump_source_id))
BEGIN SELECT RAISE(ABORT,'no_intro_dump_details_field_positions requires its open unpublished No-Intro native owner'); END;

CREATE TRIGGER no_intro_dump_serials_field_positions_requires_open_native_owner BEFORE INSERT ON no_intro_dump_serials_field_positions
WHEN NOT EXISTS (SELECT 1 FROM no_intro_dump_serials WHERE dump_source_id=NEW.dump_source_id) OR NOT EXISTS (SELECT 1 FROM no_intro_exports WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_dump_sources p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.dump_source_id=NEW.dump_source_id))
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_dump_sources p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.dump_source_id=NEW.dump_source_id))
 OR EXISTS (SELECT 1 FROM no_intro_database_parse_counts WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_dump_sources p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.dump_source_id=NEW.dump_source_id))
BEGIN SELECT RAISE(ABORT,'no_intro_dump_serials_field_positions requires its open unpublished No-Intro native owner'); END;

CREATE TRIGGER no_intro_dump_file_field_positions_requires_open_native_owner BEFORE INSERT ON no_intro_dump_file_field_positions
WHEN NOT EXISTS (SELECT 1 FROM no_intro_dump_files WHERE occurrence_id=NEW.occurrence_id) OR NOT EXISTS (SELECT 1 FROM no_intro_exports WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_dump_files f JOIN catalog_sets s ON s.set_id=f.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=NEW.occurrence_id))
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_dump_files f JOIN catalog_sets s ON s.set_id=f.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=NEW.occurrence_id))
 OR EXISTS (SELECT 1 FROM no_intro_database_parse_counts WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_dump_files f JOIN catalog_sets s ON s.set_id=f.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=NEW.occurrence_id))
BEGIN SELECT RAISE(ABORT,'no_intro_dump_file_field_positions requires its open unpublished No-Intro native owner'); END;

CREATE TRIGGER no_intro_release_details_field_positions_requires_open_native_owner BEFORE INSERT ON no_intro_release_details_field_positions
WHEN NOT EXISTS (SELECT 1 FROM no_intro_release_details WHERE release_id=NEW.release_id) OR NOT EXISTS (SELECT 1 FROM no_intro_exports WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_releases p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.release_id=NEW.release_id))
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_releases p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.release_id=NEW.release_id))
 OR EXISTS (SELECT 1 FROM no_intro_database_parse_counts WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_releases p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.release_id=NEW.release_id))
BEGIN SELECT RAISE(ABORT,'no_intro_release_details_field_positions requires its open unpublished No-Intro native owner'); END;

CREATE TRIGGER no_intro_release_serials_field_positions_requires_open_native_owner BEFORE INSERT ON no_intro_release_serials_field_positions
WHEN NOT EXISTS (SELECT 1 FROM no_intro_release_serials WHERE release_id=NEW.release_id) OR NOT EXISTS (SELECT 1 FROM no_intro_exports WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_releases p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.release_id=NEW.release_id))
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_releases p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.release_id=NEW.release_id))
 OR EXISTS (SELECT 1 FROM no_intro_database_parse_counts WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_releases p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.release_id=NEW.release_id))
BEGIN SELECT RAISE(ABORT,'no_intro_release_serials_field_positions requires its open unpublished No-Intro native owner'); END;

CREATE TRIGGER no_intro_release_file_field_positions_requires_open_native_owner BEFORE INSERT ON no_intro_release_file_field_positions
WHEN NOT EXISTS (SELECT 1 FROM no_intro_release_files WHERE occurrence_id=NEW.occurrence_id) OR NOT EXISTS (SELECT 1 FROM no_intro_exports WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_release_files f JOIN catalog_sets s ON s.set_id=f.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=NEW.occurrence_id))
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_release_files f JOIN catalog_sets s ON s.set_id=f.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=NEW.occurrence_id))
 OR EXISTS (SELECT 1 FROM no_intro_database_parse_counts WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_release_files f JOIN catalog_sets s ON s.set_id=f.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=NEW.occurrence_id))
BEGIN SELECT RAISE(ABORT,'no_intro_release_file_field_positions requires its open unpublished No-Intro native owner'); END;

CREATE TRIGGER no_intro_dump_file_digests_requires_open_native_owner BEFORE INSERT ON no_intro_dump_file_digests
WHEN NOT EXISTS (SELECT 1 FROM no_intro_dump_files WHERE occurrence_id=NEW.occurrence_id) OR NOT EXISTS (SELECT 1 FROM no_intro_exports WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_dump_files f JOIN catalog_sets s ON s.set_id=f.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=NEW.occurrence_id))
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_dump_files f JOIN catalog_sets s ON s.set_id=f.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=NEW.occurrence_id))
 OR EXISTS (SELECT 1 FROM no_intro_database_parse_counts WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_dump_files f JOIN catalog_sets s ON s.set_id=f.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=NEW.occurrence_id))
BEGIN SELECT RAISE(ABORT,'no_intro_dump_file_digests requires its open unpublished No-Intro native owner'); END;

CREATE TRIGGER no_intro_release_nfo_hashes_requires_open_native_owner BEFORE INSERT ON no_intro_release_nfo_hashes
WHEN NOT EXISTS (SELECT 1 FROM no_intro_release_details WHERE release_id=NEW.release_id) OR NOT EXISTS (SELECT 1 FROM no_intro_exports WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_releases p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.release_id=NEW.release_id))
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_releases p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.release_id=NEW.release_id))
 OR EXISTS (SELECT 1 FROM no_intro_database_parse_counts WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_releases p JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE p.release_id=NEW.release_id))
BEGIN SELECT RAISE(ABORT,'no_intro_release_nfo_hashes requires its open unpublished No-Intro native owner'); END;

CREATE TRIGGER no_intro_release_file_digests_requires_open_native_owner BEFORE INSERT ON no_intro_release_file_digests
WHEN NOT EXISTS (SELECT 1 FROM no_intro_release_files WHERE occurrence_id=NEW.occurrence_id) OR NOT EXISTS (SELECT 1 FROM no_intro_exports WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_release_files f JOIN catalog_sets s ON s.set_id=f.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=NEW.occurrence_id))
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_release_files f JOIN catalog_sets s ON s.set_id=f.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=NEW.occurrence_id))
 OR EXISTS (SELECT 1 FROM no_intro_database_parse_counts WHERE snapshot_key=(SELECT c.snapshot_key FROM no_intro_release_files f JOIN catalog_sets s ON s.set_id=f.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=NEW.occurrence_id))
BEGIN SELECT RAISE(ABORT,'no_intro_release_file_digests requires its open unpublished No-Intro native owner'); END;

-- This seals counts for rows currently staged in SQLite. It does not claim
-- that the streaming reader reached EOF or proved the complete input corpus.
CREATE TRIGGER no_intro_database_parse_counts_seal BEFORE INSERT ON no_intro_database_parse_counts
WHEN NOT EXISTS (SELECT 1 FROM no_intro_exports WHERE snapshot_key=NEW.snapshot_key)
 OR EXISTS (SELECT 1 FROM snapshot_publications WHERE snapshot_key=NEW.snapshot_key)
 OR EXISTS (SELECT 1 FROM no_intro_database_parse_counts WHERE snapshot_key=NEW.snapshot_key)
 OR NEW.game_count <> (SELECT COUNT(*) FROM no_intro_database_games g JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE c.snapshot_key=NEW.snapshot_key)
 OR NEW.game_count <> (SELECT COUNT(*) FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE c.snapshot_key=NEW.snapshot_key AND s.source_element_kind='no_intro_database_game')
 OR NEW.archive_count <> (SELECT COUNT(*) FROM no_intro_archive_descriptions a JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE c.snapshot_key=NEW.snapshot_key)
 OR NEW.dump_source_count <> (SELECT COUNT(*) FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE c.snapshot_key=NEW.snapshot_key)
 OR NEW.dump_details_count <> (SELECT COUNT(*) FROM no_intro_dump_details d JOIN no_intro_dump_sources p USING(dump_source_id) JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE c.snapshot_key=NEW.snapshot_key)
 OR NEW.dump_serials_count <> (SELECT COUNT(*) FROM no_intro_dump_serials d JOIN no_intro_dump_sources p USING(dump_source_id) JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE c.snapshot_key=NEW.snapshot_key)
 OR NEW.dump_file_count <> (SELECT COUNT(*) FROM no_intro_dump_files f JOIN catalog_sets s ON s.set_id=f.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE c.snapshot_key=NEW.snapshot_key)
 OR NEW.release_count <> (SELECT COUNT(*) FROM no_intro_releases r JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE c.snapshot_key=NEW.snapshot_key)
 OR NEW.release_details_count <> (SELECT COUNT(*) FROM no_intro_release_details d JOIN no_intro_releases p USING(release_id) JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE c.snapshot_key=NEW.snapshot_key)
 OR NEW.release_serials_count <> (SELECT COUNT(*) FROM no_intro_release_serials d JOIN no_intro_releases p USING(release_id) JOIN catalog_sets s ON s.set_id=p.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE c.snapshot_key=NEW.snapshot_key)
 OR NEW.release_file_count <> (SELECT COUNT(*) FROM no_intro_release_files f JOIN catalog_sets s ON s.set_id=f.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE c.snapshot_key=NEW.snapshot_key)
 OR NEW.header_field_count <> (SELECT COUNT(*) FROM no_intro_header_fields WHERE snapshot_key=NEW.snapshot_key)
 OR NEW.archive_field_count <> (SELECT COUNT(*) FROM no_intro_archive_field_positions p JOIN no_intro_archive_descriptions a USING(archive_id) JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE c.snapshot_key=NEW.snapshot_key)
 OR NEW.dump_details_field_count <> (SELECT COUNT(*) FROM no_intro_dump_details_field_positions p JOIN no_intro_dump_sources d USING(dump_source_id) JOIN catalog_sets s ON s.set_id=d.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE c.snapshot_key=NEW.snapshot_key)
 OR NEW.dump_serials_field_count <> (SELECT COUNT(*) FROM no_intro_dump_serials_field_positions p JOIN no_intro_dump_sources d USING(dump_source_id) JOIN catalog_sets s ON s.set_id=d.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE c.snapshot_key=NEW.snapshot_key)
 OR NEW.dump_file_field_count <> (SELECT COUNT(*) FROM no_intro_dump_file_field_positions p JOIN no_intro_dump_files f USING(occurrence_id) JOIN catalog_sets s ON s.set_id=f.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE c.snapshot_key=NEW.snapshot_key)
 OR NEW.release_details_field_count <> (SELECT COUNT(*) FROM no_intro_release_details_field_positions p JOIN no_intro_releases r USING(release_id) JOIN catalog_sets s ON s.set_id=r.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE c.snapshot_key=NEW.snapshot_key)
 OR NEW.release_serials_field_count <> (SELECT COUNT(*) FROM no_intro_release_serials_field_positions p JOIN no_intro_releases r USING(release_id) JOIN catalog_sets s ON s.set_id=r.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE c.snapshot_key=NEW.snapshot_key)
 OR NEW.release_file_field_count <> (SELECT COUNT(*) FROM no_intro_release_file_field_positions p JOIN no_intro_release_files f USING(occurrence_id) JOIN catalog_sets s ON s.set_id=f.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE c.snapshot_key=NEW.snapshot_key)
 OR NEW.dump_file_count + NEW.release_file_count <> (SELECT COUNT(*) FROM asset_occurrences o JOIN catalog_sets s ON s.set_id=o.record_id JOIN catalog_set_groups c USING(set_group_id) WHERE c.snapshot_key=NEW.snapshot_key AND o.claim_kind IN ('no_intro_database_source_file','no_intro_database_release_file'))
BEGIN SELECT RAISE(ABORT,'No-Intro database parse counts do not seal the current imported rows'); END;

CREATE TRIGGER no_intro_exports_reject_replace_collision BEFORE INSERT ON no_intro_exports
WHEN EXISTS (SELECT 1 FROM no_intro_exports old WHERE old.snapshot_key=NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT,'No-Intro database rows cannot be replaced'); END;
CREATE TRIGGER no_intro_exports_immutable_update BEFORE UPDATE ON no_intro_exports
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;
CREATE TRIGGER no_intro_exports_immutable_delete BEFORE DELETE ON no_intro_exports
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;

CREATE TRIGGER no_intro_database_games_reject_replace_collision BEFORE INSERT ON no_intro_database_games
WHEN EXISTS (SELECT 1 FROM no_intro_database_games old WHERE old.set_id=NEW.set_id)
BEGIN SELECT RAISE(ABORT,'No-Intro database rows cannot be replaced'); END;
CREATE TRIGGER no_intro_database_games_immutable_update BEFORE UPDATE ON no_intro_database_games
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;
CREATE TRIGGER no_intro_database_games_immutable_delete BEFORE DELETE ON no_intro_database_games
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;

CREATE TRIGGER no_intro_header_fields_reject_replace_collision BEFORE INSERT ON no_intro_header_fields
WHEN EXISTS (SELECT 1 FROM no_intro_header_fields old WHERE old.snapshot_key=NEW.snapshot_key AND old.source_order=NEW.source_order)
BEGIN SELECT RAISE(ABORT,'No-Intro database rows cannot be replaced'); END;
CREATE TRIGGER no_intro_header_fields_immutable_update BEFORE UPDATE ON no_intro_header_fields
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;
CREATE TRIGGER no_intro_header_fields_immutable_delete BEFORE DELETE ON no_intro_header_fields
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;

CREATE TRIGGER no_intro_export_headers_reject_replace_collision BEFORE INSERT ON no_intro_export_headers
WHEN EXISTS (SELECT 1 FROM no_intro_export_headers WHERE snapshot_key=NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT,'No-Intro database rows cannot be replaced'); END;
CREATE TRIGGER no_intro_export_headers_immutable_update BEFORE UPDATE ON no_intro_export_headers
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;
CREATE TRIGGER no_intro_export_headers_immutable_delete BEFORE DELETE ON no_intro_export_headers
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;

CREATE TRIGGER no_intro_archive_descriptions_reject_replace_collision BEFORE INSERT ON no_intro_archive_descriptions
WHEN EXISTS (SELECT 1 FROM no_intro_archive_descriptions old WHERE old.archive_id=NEW.archive_id OR (old.set_id=NEW.set_id AND old.source_order=NEW.source_order))
BEGIN SELECT RAISE(ABORT,'No-Intro database rows cannot be replaced'); END;
CREATE TRIGGER no_intro_archive_descriptions_immutable_update BEFORE UPDATE ON no_intro_archive_descriptions
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;
CREATE TRIGGER no_intro_archive_descriptions_immutable_delete BEFORE DELETE ON no_intro_archive_descriptions
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;

CREATE TRIGGER no_intro_archive_clone_markers_reject_replace BEFORE INSERT ON no_intro_archive_clone_markers
WHEN EXISTS (SELECT 1 FROM no_intro_archive_clone_markers WHERE archive_id=NEW.archive_id)
BEGIN SELECT RAISE(ABORT,'No-Intro database rows cannot be replaced'); END;
CREATE TRIGGER no_intro_archive_clone_markers_immutable_update BEFORE UPDATE ON no_intro_archive_clone_markers
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;
CREATE TRIGGER no_intro_archive_clone_markers_immutable_delete BEFORE DELETE ON no_intro_archive_clone_markers
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;

CREATE TRIGGER no_intro_archive_clone_links_reject_replace BEFORE INSERT ON no_intro_archive_clone_links
WHEN EXISTS (SELECT 1 FROM no_intro_archive_clone_links WHERE archive_id=NEW.archive_id)
BEGIN SELECT RAISE(ABORT,'No-Intro database rows cannot be replaced'); END;
CREATE TRIGGER no_intro_archive_clone_links_immutable_update BEFORE UPDATE ON no_intro_archive_clone_links
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;
CREATE TRIGGER no_intro_archive_clone_links_immutable_delete BEFORE DELETE ON no_intro_archive_clone_links
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;

CREATE TRIGGER no_intro_archive_merge_links_reject_replace BEFORE INSERT ON no_intro_archive_merge_links
WHEN EXISTS (SELECT 1 FROM no_intro_archive_merge_links WHERE archive_id=NEW.archive_id)
BEGIN SELECT RAISE(ABORT,'No-Intro database rows cannot be replaced'); END;
CREATE TRIGGER no_intro_archive_merge_links_immutable_update BEFORE UPDATE ON no_intro_archive_merge_links
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;
CREATE TRIGGER no_intro_archive_merge_links_immutable_delete BEFORE DELETE ON no_intro_archive_merge_links
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;

CREATE TRIGGER no_intro_dump_sources_reject_replace_collision BEFORE INSERT ON no_intro_dump_sources
WHEN EXISTS (SELECT 1 FROM no_intro_dump_sources old WHERE old.dump_source_id=NEW.dump_source_id)
BEGIN SELECT RAISE(ABORT,'No-Intro database rows cannot be replaced'); END;
CREATE TRIGGER no_intro_dump_sources_immutable_update BEFORE UPDATE ON no_intro_dump_sources
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;
CREATE TRIGGER no_intro_dump_sources_immutable_delete BEFORE DELETE ON no_intro_dump_sources
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;

CREATE TRIGGER no_intro_dump_details_reject_replace_collision BEFORE INSERT ON no_intro_dump_details
WHEN EXISTS (SELECT 1 FROM no_intro_dump_details old WHERE old.dump_source_id=NEW.dump_source_id)
BEGIN SELECT RAISE(ABORT,'No-Intro database rows cannot be replaced'); END;
CREATE TRIGGER no_intro_dump_details_immutable_update BEFORE UPDATE ON no_intro_dump_details
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;
CREATE TRIGGER no_intro_dump_details_immutable_delete BEFORE DELETE ON no_intro_dump_details
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;

CREATE TRIGGER no_intro_dump_serials_reject_replace_collision BEFORE INSERT ON no_intro_dump_serials
WHEN EXISTS (SELECT 1 FROM no_intro_dump_serials old WHERE old.dump_source_id=NEW.dump_source_id)
BEGIN SELECT RAISE(ABORT,'No-Intro database rows cannot be replaced'); END;
CREATE TRIGGER no_intro_dump_serials_immutable_update BEFORE UPDATE ON no_intro_dump_serials
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;
CREATE TRIGGER no_intro_dump_serials_immutable_delete BEFORE DELETE ON no_intro_dump_serials
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;

CREATE TRIGGER no_intro_dump_files_reject_replace_collision BEFORE INSERT ON no_intro_dump_files
WHEN EXISTS (SELECT 1 FROM no_intro_dump_files old WHERE old.occurrence_id=NEW.occurrence_id)
BEGIN SELECT RAISE(ABORT,'No-Intro database rows cannot be replaced'); END;
CREATE TRIGGER no_intro_dump_files_immutable_update BEFORE UPDATE ON no_intro_dump_files
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;
CREATE TRIGGER no_intro_dump_files_immutable_delete BEFORE DELETE ON no_intro_dump_files
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;

CREATE TRIGGER no_intro_releases_reject_replace_collision BEFORE INSERT ON no_intro_releases
WHEN EXISTS (SELECT 1 FROM no_intro_releases old WHERE old.release_id=NEW.release_id)
BEGIN SELECT RAISE(ABORT,'No-Intro database rows cannot be replaced'); END;
CREATE TRIGGER no_intro_releases_immutable_update BEFORE UPDATE ON no_intro_releases
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;
CREATE TRIGGER no_intro_releases_immutable_delete BEFORE DELETE ON no_intro_releases
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;

CREATE TRIGGER no_intro_release_details_reject_replace_collision BEFORE INSERT ON no_intro_release_details
WHEN EXISTS (SELECT 1 FROM no_intro_release_details old WHERE old.release_id=NEW.release_id)
BEGIN SELECT RAISE(ABORT,'No-Intro database rows cannot be replaced'); END;
CREATE TRIGGER no_intro_release_details_immutable_update BEFORE UPDATE ON no_intro_release_details
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;
CREATE TRIGGER no_intro_release_details_immutable_delete BEFORE DELETE ON no_intro_release_details
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;

CREATE TRIGGER no_intro_release_serials_reject_replace_collision BEFORE INSERT ON no_intro_release_serials
WHEN EXISTS (SELECT 1 FROM no_intro_release_serials old WHERE old.release_id=NEW.release_id)
BEGIN SELECT RAISE(ABORT,'No-Intro database rows cannot be replaced'); END;
CREATE TRIGGER no_intro_release_serials_immutable_update BEFORE UPDATE ON no_intro_release_serials
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;
CREATE TRIGGER no_intro_release_serials_immutable_delete BEFORE DELETE ON no_intro_release_serials
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;

CREATE TRIGGER no_intro_release_files_reject_replace_collision BEFORE INSERT ON no_intro_release_files
WHEN EXISTS (SELECT 1 FROM no_intro_release_files old WHERE old.occurrence_id=NEW.occurrence_id)
BEGIN SELECT RAISE(ABORT,'No-Intro database rows cannot be replaced'); END;
CREATE TRIGGER no_intro_release_files_immutable_update BEFORE UPDATE ON no_intro_release_files
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;
CREATE TRIGGER no_intro_release_files_immutable_delete BEFORE DELETE ON no_intro_release_files
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;

CREATE TRIGGER no_intro_archive_field_positions_reject_replace_collision BEFORE INSERT ON no_intro_archive_field_positions
WHEN EXISTS (SELECT 1 FROM no_intro_archive_field_positions old WHERE old.archive_id=NEW.archive_id AND old.field_kind=NEW.field_kind)
BEGIN SELECT RAISE(ABORT,'No-Intro database rows cannot be replaced'); END;
CREATE TRIGGER no_intro_archive_field_positions_immutable_update BEFORE UPDATE ON no_intro_archive_field_positions
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;
CREATE TRIGGER no_intro_archive_field_positions_immutable_delete BEFORE DELETE ON no_intro_archive_field_positions
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;

CREATE TRIGGER no_intro_dump_details_field_positions_reject_replace_collision BEFORE INSERT ON no_intro_dump_details_field_positions
WHEN EXISTS (SELECT 1 FROM no_intro_dump_details_field_positions old WHERE old.dump_source_id=NEW.dump_source_id AND old.field_kind=NEW.field_kind)
BEGIN SELECT RAISE(ABORT,'No-Intro database rows cannot be replaced'); END;
CREATE TRIGGER no_intro_dump_details_field_positions_immutable_update BEFORE UPDATE ON no_intro_dump_details_field_positions
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;
CREATE TRIGGER no_intro_dump_details_field_positions_immutable_delete BEFORE DELETE ON no_intro_dump_details_field_positions
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;

CREATE TRIGGER no_intro_dump_serials_field_positions_reject_replace_collision BEFORE INSERT ON no_intro_dump_serials_field_positions
WHEN EXISTS (SELECT 1 FROM no_intro_dump_serials_field_positions old WHERE old.dump_source_id=NEW.dump_source_id AND old.field_kind=NEW.field_kind)
BEGIN SELECT RAISE(ABORT,'No-Intro database rows cannot be replaced'); END;
CREATE TRIGGER no_intro_dump_serials_field_positions_immutable_update BEFORE UPDATE ON no_intro_dump_serials_field_positions
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;
CREATE TRIGGER no_intro_dump_serials_field_positions_immutable_delete BEFORE DELETE ON no_intro_dump_serials_field_positions
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;

CREATE TRIGGER no_intro_dump_file_field_positions_reject_replace_collision BEFORE INSERT ON no_intro_dump_file_field_positions
WHEN EXISTS (SELECT 1 FROM no_intro_dump_file_field_positions old WHERE old.occurrence_id=NEW.occurrence_id AND old.field_kind=NEW.field_kind)
BEGIN SELECT RAISE(ABORT,'No-Intro database rows cannot be replaced'); END;
CREATE TRIGGER no_intro_dump_file_field_positions_immutable_update BEFORE UPDATE ON no_intro_dump_file_field_positions
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;
CREATE TRIGGER no_intro_dump_file_field_positions_immutable_delete BEFORE DELETE ON no_intro_dump_file_field_positions
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;

CREATE TRIGGER no_intro_release_details_field_positions_reject_replace_collision BEFORE INSERT ON no_intro_release_details_field_positions
WHEN EXISTS (SELECT 1 FROM no_intro_release_details_field_positions old WHERE old.release_id=NEW.release_id AND old.field_kind=NEW.field_kind)
BEGIN SELECT RAISE(ABORT,'No-Intro database rows cannot be replaced'); END;
CREATE TRIGGER no_intro_release_details_field_positions_immutable_update BEFORE UPDATE ON no_intro_release_details_field_positions
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;
CREATE TRIGGER no_intro_release_details_field_positions_immutable_delete BEFORE DELETE ON no_intro_release_details_field_positions
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;

CREATE TRIGGER no_intro_release_serials_field_positions_reject_replace_collision BEFORE INSERT ON no_intro_release_serials_field_positions
WHEN EXISTS (SELECT 1 FROM no_intro_release_serials_field_positions old WHERE old.release_id=NEW.release_id AND old.field_kind=NEW.field_kind)
BEGIN SELECT RAISE(ABORT,'No-Intro database rows cannot be replaced'); END;
CREATE TRIGGER no_intro_release_serials_field_positions_immutable_update BEFORE UPDATE ON no_intro_release_serials_field_positions
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;
CREATE TRIGGER no_intro_release_serials_field_positions_immutable_delete BEFORE DELETE ON no_intro_release_serials_field_positions
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;

CREATE TRIGGER no_intro_release_file_field_positions_reject_replace_collision BEFORE INSERT ON no_intro_release_file_field_positions
WHEN EXISTS (SELECT 1 FROM no_intro_release_file_field_positions old WHERE old.occurrence_id=NEW.occurrence_id AND old.field_kind=NEW.field_kind)
BEGIN SELECT RAISE(ABORT,'No-Intro database rows cannot be replaced'); END;
CREATE TRIGGER no_intro_release_file_field_positions_immutable_update BEFORE UPDATE ON no_intro_release_file_field_positions
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;
CREATE TRIGGER no_intro_release_file_field_positions_immutable_delete BEFORE DELETE ON no_intro_release_file_field_positions
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;

CREATE TRIGGER no_intro_dump_file_digests_reject_replace_collision BEFORE INSERT ON no_intro_dump_file_digests
WHEN EXISTS (SELECT 1 FROM no_intro_dump_file_digests old WHERE old.occurrence_id=NEW.occurrence_id AND old.field_kind=NEW.field_kind)
BEGIN SELECT RAISE(ABORT,'No-Intro database rows cannot be replaced'); END;
CREATE TRIGGER no_intro_dump_file_digests_immutable_update BEFORE UPDATE ON no_intro_dump_file_digests
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;
CREATE TRIGGER no_intro_dump_file_digests_immutable_delete BEFORE DELETE ON no_intro_dump_file_digests
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;

CREATE TRIGGER no_intro_release_nfo_hashes_reject_replace_collision BEFORE INSERT ON no_intro_release_nfo_hashes
WHEN EXISTS (SELECT 1 FROM no_intro_release_nfo_hashes old WHERE old.release_id=NEW.release_id AND old.source_hash_field=NEW.source_hash_field)
BEGIN SELECT RAISE(ABORT,'No-Intro database rows cannot be replaced'); END;
CREATE TRIGGER no_intro_release_nfo_hashes_immutable_update BEFORE UPDATE ON no_intro_release_nfo_hashes
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;
CREATE TRIGGER no_intro_release_nfo_hashes_immutable_delete BEFORE DELETE ON no_intro_release_nfo_hashes
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;

CREATE TRIGGER no_intro_release_file_digests_reject_replace_collision BEFORE INSERT ON no_intro_release_file_digests
WHEN EXISTS (SELECT 1 FROM no_intro_release_file_digests old WHERE old.occurrence_id=NEW.occurrence_id AND old.field_kind=NEW.field_kind)
BEGIN SELECT RAISE(ABORT,'No-Intro database rows cannot be replaced'); END;
CREATE TRIGGER no_intro_release_file_digests_immutable_update BEFORE UPDATE ON no_intro_release_file_digests
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;
CREATE TRIGGER no_intro_release_file_digests_immutable_delete BEFORE DELETE ON no_intro_release_file_digests
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;

CREATE TRIGGER no_intro_database_parse_counts_reject_replace_collision BEFORE INSERT ON no_intro_database_parse_counts
WHEN EXISTS (SELECT 1 FROM no_intro_database_parse_counts old WHERE old.snapshot_key=NEW.snapshot_key)
BEGIN SELECT RAISE(ABORT,'No-Intro database rows cannot be replaced'); END;
CREATE TRIGGER no_intro_database_parse_counts_immutable_update BEFORE UPDATE ON no_intro_database_parse_counts
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;
CREATE TRIGGER no_intro_database_parse_counts_immutable_delete BEFORE DELETE ON no_intro_database_parse_counts
BEGIN SELECT RAISE(ABORT,'No-Intro database rows are immutable'); END;

-- Relational views compare closed typed positions with present stored
-- attributes. Digest spellings and positions are each retained exactly once.
CREATE VIEW no_intro_database_expected_attribute_positions AS
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=o.set_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 0 AS field_kind FROM no_intro_archive_descriptions o WHERE o.additional IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=o.set_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 1 AS field_kind FROM no_intro_archive_descriptions o WHERE o.adult IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=o.set_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 2 AS field_kind FROM no_intro_archive_descriptions o WHERE o.aftermarket IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=o.set_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 3 AS field_kind FROM no_intro_archive_descriptions o WHERE o.alt IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=o.set_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 4 AS field_kind FROM no_intro_archive_descriptions o WHERE o.bios IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=o.set_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 5 AS field_kind FROM no_intro_archive_descriptions o WHERE o.categories IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=o.set_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 6 AS field_kind FROM no_intro_archive_descriptions o WHERE o.complete IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=o.set_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 7 AS field_kind FROM no_intro_archive_descriptions o WHERE o.dat IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=o.set_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 8 AS field_kind FROM no_intro_archive_descriptions o WHERE o.datter_note IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=o.set_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 9 AS field_kind FROM no_intro_archive_descriptions o WHERE o.description IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=o.set_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 10 AS field_kind FROM no_intro_archive_descriptions o WHERE o.devstatus IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=o.set_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 11 AS field_kind FROM no_intro_archive_descriptions o WHERE o.gameid1 IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=o.set_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 12 AS field_kind FROM no_intro_archive_descriptions o WHERE o.gameid2 IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=o.set_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 13 AS field_kind FROM no_intro_archive_descriptions o WHERE o.langchecked IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=o.set_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 14 AS field_kind FROM no_intro_archive_descriptions o WHERE o.languages IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=o.set_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 15 AS field_kind FROM no_intro_archive_descriptions o WHERE o.licensed IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=o.set_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 16 AS field_kind FROM no_intro_archive_descriptions o WHERE o.listed IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=o.set_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 17 AS field_kind FROM no_intro_archive_descriptions o WHERE o.mergename IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=o.set_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 18 AS field_kind FROM no_intro_archive_descriptions o WHERE o.name IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=o.set_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 19 AS field_kind FROM no_intro_archive_descriptions o WHERE o.name_alt IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=o.set_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 20 AS field_kind FROM no_intro_archive_descriptions o WHERE o.number IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=o.set_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 21 AS field_kind FROM no_intro_archive_descriptions o WHERE o.physical IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=o.set_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 22 AS field_kind FROM no_intro_archive_descriptions o WHERE o.region IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=o.set_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 23 AS field_kind FROM no_intro_archive_descriptions o WHERE o.regparent IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=o.set_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 24 AS field_kind FROM no_intro_archive_descriptions o WHERE o.showlang IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=o.set_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 25 AS field_kind FROM no_intro_archive_descriptions o WHERE o.special1 IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=o.set_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 26 AS field_kind FROM no_intro_archive_descriptions o WHERE o.special2 IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=o.set_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 27 AS field_kind FROM no_intro_archive_descriptions o WHERE o.sticky_note IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=o.set_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 28 AS field_kind FROM no_intro_archive_descriptions o WHERE o.version1 IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE s.set_id=o.set_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 29 AS field_kind FROM no_intro_archive_descriptions o WHERE o.version2 IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_archive_descriptions a JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE a.archive_id=o.archive_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 30 AS field_kind FROM no_intro_archive_clone_markers o
UNION ALL
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) JOIN no_intro_archive_descriptions a USING(set_id) WHERE a.archive_id=o.archive_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 30 AS field_kind FROM no_intro_archive_clone_links o
UNION ALL
SELECT (SELECT c.snapshot_key FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) JOIN no_intro_archive_descriptions a USING(set_id) WHERE a.archive_id=o.archive_id) AS snapshot_key, 'archive' AS owner_kind, o.archive_id AS owner_id, 0 AS owner_sub_id, 31 AS field_kind FROM no_intro_archive_merge_links o
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_details' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 0 AS field_kind FROM no_intro_dump_details o WHERE o.comment1 IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_details' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 1 AS field_kind FROM no_intro_dump_details o WHERE o.comment2 IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_details' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 2 AS field_kind FROM no_intro_dump_details o WHERE o.d_date IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_details' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 3 AS field_kind FROM no_intro_dump_details o WHERE o.d_date_info IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_details' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 4 AS field_kind FROM no_intro_dump_details o WHERE o.dumper IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_details' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 5 AS field_kind FROM no_intro_dump_details o WHERE o.id IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_details' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 6 AS field_kind FROM no_intro_dump_details o WHERE o.link1 IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_details' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 7 AS field_kind FROM no_intro_dump_details o WHERE o.link2 IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_details' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 8 AS field_kind FROM no_intro_dump_details o WHERE o.link3 IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_details' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 9 AS field_kind FROM no_intro_dump_details o WHERE o.media_title IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_details' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 10 AS field_kind FROM no_intro_dump_details o WHERE o.nodump IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_details' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 11 AS field_kind FROM no_intro_dump_details o WHERE o.origin IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_details' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 12 AS field_kind FROM no_intro_dump_details o WHERE o.originalformat IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_details' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 13 AS field_kind FROM no_intro_dump_details o WHERE o.project IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_details' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 14 AS field_kind FROM no_intro_dump_details o WHERE o.r_date IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_details' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 15 AS field_kind FROM no_intro_dump_details o WHERE o.r_date_info IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_details' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 16 AS field_kind FROM no_intro_dump_details o WHERE o.region IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_details' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 17 AS field_kind FROM no_intro_dump_details o WHERE o.rominfo IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_details' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 18 AS field_kind FROM no_intro_dump_details o WHERE o.section IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_details' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 19 AS field_kind FROM no_intro_dump_details o WHERE o.tool IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_serials' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 0 AS field_kind FROM no_intro_dump_serials o WHERE o.box_barcode IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_serials' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 1 AS field_kind FROM no_intro_dump_serials o WHERE o.box_serial IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_serials' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 2 AS field_kind FROM no_intro_dump_serials o WHERE o.chip_serial IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_serials' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 3 AS field_kind FROM no_intro_dump_serials o WHERE o.digital_serial1 IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_serials' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 4 AS field_kind FROM no_intro_dump_serials o WHERE o.digital_serial2 IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_serials' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 5 AS field_kind FROM no_intro_dump_serials o WHERE o.lockout_serial IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_serials' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 6 AS field_kind FROM no_intro_dump_serials o WHERE o.media_serial1 IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_serials' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 7 AS field_kind FROM no_intro_dump_serials o WHERE o.media_serial2 IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_serials' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 8 AS field_kind FROM no_intro_dump_serials o WHERE o.media_serial3 IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_serials' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 9 AS field_kind FROM no_intro_dump_serials o WHERE o.mediastamp IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_serials' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 10 AS field_kind FROM no_intro_dump_serials o WHERE o.pcb_serial IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_serials' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 11 AS field_kind FROM no_intro_dump_serials o WHERE o.romchip_serial1 IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_serials' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 12 AS field_kind FROM no_intro_dump_serials o WHERE o.romchip_serial2 IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE d.dump_source_id=o.dump_source_id) AS snapshot_key, 'dump_serials' AS owner_kind, o.dump_source_id AS owner_id, 0 AS owner_sub_id, 13 AS field_kind FROM no_intro_dump_serials o WHERE o.savechip_serial IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_files f JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=o.occurrence_id) AS snapshot_key, 'dump_file' AS owner_kind, o.occurrence_id AS owner_id, 0 AS owner_sub_id, 0 AS field_kind FROM no_intro_dump_files o WHERE o.bad IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_files f JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=o.occurrence_id) AS snapshot_key, 'dump_file' AS owner_kind, o.occurrence_id AS owner_id, 0 AS owner_sub_id, 2 AS field_kind FROM no_intro_dump_files o WHERE o.date IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_files f JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=o.occurrence_id) AS snapshot_key, 'dump_file' AS owner_kind, o.occurrence_id AS owner_id, 0 AS owner_sub_id, 3 AS field_kind FROM no_intro_dump_files o WHERE o.extension IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_files f JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=o.occurrence_id) AS snapshot_key, 'dump_file' AS owner_kind, o.occurrence_id AS owner_id, 0 AS owner_sub_id, 4 AS field_kind FROM no_intro_dump_files o WHERE o.filter IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_files f JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=o.occurrence_id) AS snapshot_key, 'dump_file' AS owner_kind, o.occurrence_id AS owner_id, 0 AS owner_sub_id, 5 AS field_kind FROM no_intro_dump_files o WHERE o.forcename IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_files f JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=o.occurrence_id) AS snapshot_key, 'dump_file' AS owner_kind, o.occurrence_id AS owner_id, 0 AS owner_sub_id, 6 AS field_kind FROM no_intro_dump_files o WHERE o.forcescenename IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_files f JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=o.occurrence_id) AS snapshot_key, 'dump_file' AS owner_kind, o.occurrence_id AS owner_id, 0 AS owner_sub_id, 7 AS field_kind FROM no_intro_dump_files o WHERE o.format IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_files f JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=o.occurrence_id) AS snapshot_key, 'dump_file' AS owner_kind, o.occurrence_id AS owner_id, 0 AS owner_sub_id, 8 AS field_kind FROM no_intro_dump_files o WHERE o.header IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_files f JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=o.occurrence_id) AS snapshot_key, 'dump_file' AS owner_kind, o.occurrence_id AS owner_id, 0 AS owner_sub_id, 9 AS field_kind FROM no_intro_dump_files o WHERE o.id IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_files f JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=o.occurrence_id) AS snapshot_key, 'dump_file' AS owner_kind, o.occurrence_id AS owner_id, 0 AS owner_sub_id, 10 AS field_kind FROM no_intro_dump_files o WHERE o.item IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_files f JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=o.occurrence_id) AS snapshot_key, 'dump_file' AS owner_kind, o.occurrence_id AS owner_id, 0 AS owner_sub_id, 12 AS field_kind FROM no_intro_dump_files o WHERE o.mia IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_files f JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=o.occurrence_id) AS snapshot_key, 'dump_file' AS owner_kind, o.occurrence_id AS owner_id, 0 AS owner_sub_id, 13 AS field_kind FROM no_intro_dump_files o WHERE o.note IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_files f JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=o.occurrence_id) AS snapshot_key, 'dump_file' AS owner_kind, o.occurrence_id AS owner_id, 0 AS owner_sub_id, 15 AS field_kind FROM no_intro_dump_files o WHERE o.origin_size IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_files f JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=o.occurrence_id) AS snapshot_key, 'dump_file' AS owner_kind, o.occurrence_id AS owner_id, 0 AS owner_sub_id, 16 AS field_kind FROM no_intro_dump_files o WHERE o.serial IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_files f JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=o.occurrence_id) AS snapshot_key, 'dump_file' AS owner_kind, o.occurrence_id AS owner_id, 0 AS owner_sub_id, 19 AS field_kind FROM no_intro_dump_files o WHERE o.source_size IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_files f JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=o.occurrence_id) AS snapshot_key, 'dump_file' AS owner_kind, o.occurrence_id AS owner_id, 0 AS owner_sub_id, 20 AS field_kind FROM no_intro_dump_files o WHERE o."unique" IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_files f JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=o.occurrence_id) AS snapshot_key, 'dump_file' AS owner_kind, o.occurrence_id AS owner_id, 0 AS owner_sub_id, 21 AS field_kind FROM no_intro_dump_files o WHERE o.update_type IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_files f JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=o.occurrence_id) AS snapshot_key, 'dump_file' AS owner_kind, o.occurrence_id AS owner_id, 0 AS owner_sub_id, 22 AS field_kind FROM no_intro_dump_files o WHERE o.version IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_releases r JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE r.release_id=o.release_id) AS snapshot_key, 'release_details' AS owner_kind, o.release_id AS owner_id, 0 AS owner_sub_id, 0 AS field_kind FROM no_intro_release_details o WHERE o.archivename IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_releases r JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE r.release_id=o.release_id) AS snapshot_key, 'release_details' AS owner_kind, o.release_id AS owner_id, 0 AS owner_sub_id, 1 AS field_kind FROM no_intro_release_details o WHERE o.category IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_releases r JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE r.release_id=o.release_id) AS snapshot_key, 'release_details' AS owner_kind, o.release_id AS owner_id, 0 AS owner_sub_id, 2 AS field_kind FROM no_intro_release_details o WHERE o.comment IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_releases r JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE r.release_id=o.release_id) AS snapshot_key, 'release_details' AS owner_kind, o.release_id AS owner_id, 0 AS owner_sub_id, 3 AS field_kind FROM no_intro_release_details o WHERE o.date IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_releases r JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE r.release_id=o.release_id) AS snapshot_key, 'release_details' AS owner_kind, o.release_id AS owner_id, 0 AS owner_sub_id, 4 AS field_kind FROM no_intro_release_details o WHERE o.dirname IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_releases r JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE r.release_id=o.release_id) AS snapshot_key, 'release_details' AS owner_kind, o.release_id AS owner_id, 0 AS owner_sub_id, 5 AS field_kind FROM no_intro_release_details o WHERE o."group" IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_releases r JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE r.release_id=o.release_id) AS snapshot_key, 'release_details' AS owner_kind, o.release_id AS owner_id, 0 AS owner_sub_id, 6 AS field_kind FROM no_intro_release_details o WHERE o.id IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_releases r JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE r.release_id=o.release_id) AS snapshot_key, 'release_details' AS owner_kind, o.release_id AS owner_id, 0 AS owner_sub_id, 8 AS field_kind FROM no_intro_release_details o WHERE o.nfo_size IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_releases r JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE r.release_id=o.release_id) AS snapshot_key, 'release_details' AS owner_kind, o.release_id AS owner_id, 0 AS owner_sub_id, 10 AS field_kind FROM no_intro_release_details o WHERE o.nfoname IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_releases r JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE r.release_id=o.release_id) AS snapshot_key, 'release_details' AS owner_kind, o.release_id AS owner_id, 0 AS owner_sub_id, 11 AS field_kind FROM no_intro_release_details o WHERE o.nfosize IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_releases r JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE r.release_id=o.release_id) AS snapshot_key, 'release_details' AS owner_kind, o.release_id AS owner_id, 0 AS owner_sub_id, 12 AS field_kind FROM no_intro_release_details o WHERE o.origin IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_releases r JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE r.release_id=o.release_id) AS snapshot_key, 'release_details' AS owner_kind, o.release_id AS owner_id, 0 AS owner_sub_id, 13 AS field_kind FROM no_intro_release_details o WHERE o.originalformat IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_releases r JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE r.release_id=o.release_id) AS snapshot_key, 'release_details' AS owner_kind, o.release_id AS owner_id, 0 AS owner_sub_id, 14 AS field_kind FROM no_intro_release_details o WHERE o.region IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_releases r JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE r.release_id=o.release_id) AS snapshot_key, 'release_details' AS owner_kind, o.release_id AS owner_id, 0 AS owner_sub_id, 15 AS field_kind FROM no_intro_release_details o WHERE o.rominfo IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_releases r JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE r.release_id=o.release_id) AS snapshot_key, 'release_details' AS owner_kind, o.release_id AS owner_id, 0 AS owner_sub_id, 16 AS field_kind FROM no_intro_release_details o WHERE o.tool IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_releases r JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE r.release_id=o.release_id) AS snapshot_key, 'release_serials' AS owner_kind, o.release_id AS owner_id, 0 AS owner_sub_id, 0 AS field_kind FROM no_intro_release_serials o WHERE o.box_barcode IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_releases r JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE r.release_id=o.release_id) AS snapshot_key, 'release_serials' AS owner_kind, o.release_id AS owner_id, 0 AS owner_sub_id, 1 AS field_kind FROM no_intro_release_serials o WHERE o.box_serial IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_releases r JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE r.release_id=o.release_id) AS snapshot_key, 'release_serials' AS owner_kind, o.release_id AS owner_id, 0 AS owner_sub_id, 2 AS field_kind FROM no_intro_release_serials o WHERE o.media_serial1 IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_releases r JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE r.release_id=o.release_id) AS snapshot_key, 'release_serials' AS owner_kind, o.release_id AS owner_id, 0 AS owner_sub_id, 3 AS field_kind FROM no_intro_release_serials o WHERE o.mediastamp IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_releases r JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE r.release_id=o.release_id) AS snapshot_key, 'release_serials' AS owner_kind, o.release_id AS owner_id, 0 AS owner_sub_id, 4 AS field_kind FROM no_intro_release_serials o WHERE o.pcb_serial IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_releases r JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE r.release_id=o.release_id) AS snapshot_key, 'release_serials' AS owner_kind, o.release_id AS owner_id, 0 AS owner_sub_id, 5 AS field_kind FROM no_intro_release_serials o WHERE o.romchip_serial1 IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_release_files f JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=o.occurrence_id) AS snapshot_key, 'release_file' AS owner_kind, o.occurrence_id AS owner_id, 0 AS owner_sub_id, 0 AS field_kind FROM no_intro_release_files o WHERE o.bad IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_release_files f JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=o.occurrence_id) AS snapshot_key, 'release_file' AS owner_kind, o.occurrence_id AS owner_id, 0 AS owner_sub_id, 2 AS field_kind FROM no_intro_release_files o WHERE o.extension IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_release_files f JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=o.occurrence_id) AS snapshot_key, 'release_file' AS owner_kind, o.occurrence_id AS owner_id, 0 AS owner_sub_id, 3 AS field_kind FROM no_intro_release_files o WHERE o.forcename IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_release_files f JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=o.occurrence_id) AS snapshot_key, 'release_file' AS owner_kind, o.occurrence_id AS owner_id, 0 AS owner_sub_id, 4 AS field_kind FROM no_intro_release_files o WHERE o.forcescenename IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_release_files f JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=o.occurrence_id) AS snapshot_key, 'release_file' AS owner_kind, o.occurrence_id AS owner_id, 0 AS owner_sub_id, 5 AS field_kind FROM no_intro_release_files o WHERE o.format IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_release_files f JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=o.occurrence_id) AS snapshot_key, 'release_file' AS owner_kind, o.occurrence_id AS owner_id, 0 AS owner_sub_id, 6 AS field_kind FROM no_intro_release_files o WHERE o.header IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_release_files f JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=o.occurrence_id) AS snapshot_key, 'release_file' AS owner_kind, o.occurrence_id AS owner_id, 0 AS owner_sub_id, 7 AS field_kind FROM no_intro_release_files o WHERE o.id IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_release_files f JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=o.occurrence_id) AS snapshot_key, 'release_file' AS owner_kind, o.occurrence_id AS owner_id, 0 AS owner_sub_id, 8 AS field_kind FROM no_intro_release_files o WHERE o.item IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_release_files f JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=o.occurrence_id) AS snapshot_key, 'release_file' AS owner_kind, o.occurrence_id AS owner_id, 0 AS owner_sub_id, 10 AS field_kind FROM no_intro_release_files o WHERE o.note IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_release_files f JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=o.occurrence_id) AS snapshot_key, 'release_file' AS owner_kind, o.occurrence_id AS owner_id, 0 AS owner_sub_id, 11 AS field_kind FROM no_intro_release_files o WHERE o.serial IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_release_files f JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=o.occurrence_id) AS snapshot_key, 'release_file' AS owner_kind, o.occurrence_id AS owner_id, 0 AS owner_sub_id, 14 AS field_kind FROM no_intro_release_files o WHERE o.source_size IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_release_files f JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=o.occurrence_id) AS snapshot_key, 'release_file' AS owner_kind, o.occurrence_id AS owner_id, 0 AS owner_sub_id, 15 AS field_kind FROM no_intro_release_files o WHERE o.update_type IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_release_files f JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=o.occurrence_id) AS snapshot_key, 'release_file' AS owner_kind, o.occurrence_id AS owner_id, 0 AS owner_sub_id, 16 AS field_kind FROM no_intro_release_files o WHERE o.version IS NOT NULL
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_files f JOIN catalog_sets s ON s.set_id=f.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=d.occurrence_id) AS snapshot_key, 'dump_file' AS owner_kind, d.occurrence_id AS owner_id, 0 AS owner_sub_id, 1 AS field_kind FROM no_intro_dump_file_digests d WHERE d.field_kind=0
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_files f JOIN catalog_sets s ON s.set_id=f.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=d.occurrence_id) AS snapshot_key, 'dump_file' AS owner_kind, d.occurrence_id AS owner_id, 0 AS owner_sub_id, 11 AS field_kind FROM no_intro_dump_file_digests d WHERE d.field_kind=1
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_files f JOIN catalog_sets s ON s.set_id=f.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=d.occurrence_id) AS snapshot_key, 'dump_file' AS owner_kind, d.occurrence_id AS owner_id, 0 AS owner_sub_id, 17 AS field_kind FROM no_intro_dump_file_digests d WHERE d.field_kind=2
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_files f JOIN catalog_sets s ON s.set_id=f.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=d.occurrence_id) AS snapshot_key, 'dump_file' AS owner_kind, d.occurrence_id AS owner_id, 0 AS owner_sub_id, 18 AS field_kind FROM no_intro_dump_file_digests d WHERE d.field_kind=3
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_dump_files f JOIN catalog_sets s ON s.set_id=f.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=d.occurrence_id) AS snapshot_key, 'dump_file' AS owner_kind, d.occurrence_id AS owner_id, 0 AS owner_sub_id, 14 AS field_kind FROM no_intro_dump_file_digests d WHERE d.field_kind=4
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_releases r JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE r.release_id=o.release_id) AS snapshot_key, 'release_details' AS owner_kind, o.release_id AS owner_id, 0 AS owner_sub_id, 7 AS field_kind FROM no_intro_release_nfo_hashes o WHERE o.source_hash_field='nfo_crc32'
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_releases r JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE r.release_id=o.release_id) AS snapshot_key, 'release_details' AS owner_kind, o.release_id AS owner_id, 0 AS owner_sub_id, 9 AS field_kind FROM no_intro_release_nfo_hashes o WHERE o.source_hash_field='nfocrc'
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_release_files f JOIN catalog_sets s ON s.set_id=f.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=d.occurrence_id) AS snapshot_key, 'release_file' AS owner_kind, d.occurrence_id AS owner_id, 0 AS owner_sub_id, 1 AS field_kind FROM no_intro_release_file_digests d WHERE d.field_kind=0
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_release_files f JOIN catalog_sets s ON s.set_id=f.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=d.occurrence_id) AS snapshot_key, 'release_file' AS owner_kind, d.occurrence_id AS owner_id, 0 AS owner_sub_id, 9 AS field_kind FROM no_intro_release_file_digests d WHERE d.field_kind=1
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_release_files f JOIN catalog_sets s ON s.set_id=f.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=d.occurrence_id) AS snapshot_key, 'release_file' AS owner_kind, d.occurrence_id AS owner_id, 0 AS owner_sub_id, 12 AS field_kind FROM no_intro_release_file_digests d WHERE d.field_kind=2
UNION ALL
SELECT (SELECT c.snapshot_key FROM no_intro_release_files f JOIN catalog_sets s ON s.set_id=f.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE f.occurrence_id=d.occurrence_id) AS snapshot_key, 'release_file' AS owner_kind, d.occurrence_id AS owner_id, 0 AS owner_sub_id, 13 AS field_kind FROM no_intro_release_file_digests d WHERE d.field_kind=3;
CREATE VIEW no_intro_database_actual_attribute_positions AS
SELECT c.snapshot_key,'archive' AS owner_kind,p.archive_id AS owner_id,0 AS owner_sub_id,p.field_kind FROM no_intro_archive_field_positions p JOIN no_intro_archive_descriptions a USING(archive_id) JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id)
UNION ALL
SELECT c.snapshot_key,'dump_details',p.dump_source_id,0,p.field_kind FROM no_intro_dump_details_field_positions p JOIN no_intro_dump_sources d USING(dump_source_id) JOIN catalog_sets s ON s.set_id=d.set_id JOIN catalog_set_groups c USING(set_group_id)
UNION ALL
SELECT c.snapshot_key,'dump_serials',p.dump_source_id,0,p.field_kind FROM no_intro_dump_serials_field_positions p JOIN no_intro_dump_sources d USING(dump_source_id) JOIN catalog_sets s ON s.set_id=d.set_id JOIN catalog_set_groups c USING(set_group_id)
UNION ALL
SELECT c.snapshot_key,'dump_file',p.occurrence_id,0,p.field_kind FROM no_intro_dump_file_field_positions p JOIN no_intro_dump_files f USING(occurrence_id) JOIN catalog_sets s ON s.set_id=f.set_id JOIN catalog_set_groups c USING(set_group_id)
UNION ALL
SELECT c.snapshot_key,'release_details',p.release_id,0,p.field_kind FROM no_intro_release_details_field_positions p JOIN no_intro_releases r USING(release_id) JOIN catalog_sets s ON s.set_id=r.set_id JOIN catalog_set_groups c USING(set_group_id)
UNION ALL
SELECT c.snapshot_key,'release_serials',p.release_id,0,p.field_kind FROM no_intro_release_serials_field_positions p JOIN no_intro_releases r USING(release_id) JOIN catalog_sets s ON s.set_id=r.set_id JOIN catalog_set_groups c USING(set_group_id)
UNION ALL
SELECT c.snapshot_key,'release_file',p.occurrence_id,0,p.field_kind FROM no_intro_release_file_field_positions p JOIN no_intro_release_files f USING(occurrence_id) JOIN catalog_sets s ON s.set_id=f.set_id JOIN catalog_set_groups c USING(set_group_id);

CREATE TRIGGER no_intro_database_publication_guard BEFORE INSERT ON snapshot_publications
WHEN EXISTS (SELECT 1 FROM catalog_snapshots s JOIN parser_interpretations p USING(interpretation_key) WHERE s.snapshot_key=NEW.snapshot_key AND p.format IN ('no-intro-database-xml-compatible','no-intro-database-xml-nul-compatible'))
 AND (NOT EXISTS (SELECT 1 FROM no_intro_exports WHERE snapshot_key=NEW.snapshot_key)
 OR NOT EXISTS (SELECT 1 FROM no_intro_database_parse_counts WHERE snapshot_key=NEW.snapshot_key)
 OR EXISTS (SELECT 1 FROM no_intro_exports e WHERE e.snapshot_key=NEW.snapshot_key AND ((e.header_present=1 AND NOT EXISTS (SELECT 1 FROM no_intro_export_headers h WHERE h.snapshot_key=e.snapshot_key)) OR (e.header_present=0 AND EXISTS (SELECT 1 FROM no_intro_export_headers h WHERE h.snapshot_key=e.snapshot_key))))
 OR EXISTS (SELECT snapshot_key,owner_kind,owner_id,owner_sub_id,field_kind FROM no_intro_database_expected_attribute_positions WHERE snapshot_key=NEW.snapshot_key EXCEPT SELECT snapshot_key,owner_kind,owner_id,owner_sub_id,field_kind FROM no_intro_database_actual_attribute_positions WHERE snapshot_key=NEW.snapshot_key)
 OR EXISTS (SELECT snapshot_key,owner_kind,owner_id,owner_sub_id,field_kind FROM no_intro_database_actual_attribute_positions WHERE snapshot_key=NEW.snapshot_key EXCEPT SELECT snapshot_key,owner_kind,owner_id,owner_sub_id,field_kind FROM no_intro_database_expected_attribute_positions WHERE snapshot_key=NEW.snapshot_key)
 OR EXISTS (SELECT 1 FROM (SELECT set_id,source_order FROM no_intro_archive_descriptions UNION ALL SELECT set_id,source_order FROM no_intro_dump_sources UNION ALL SELECT set_id,source_order FROM no_intro_releases) ch JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE c.snapshot_key=NEW.snapshot_key GROUP BY ch.set_id,ch.source_order HAVING COUNT(*)>1)
 OR EXISTS (SELECT 1 FROM (SELECT set_id,source_order FROM no_intro_archive_descriptions UNION ALL SELECT set_id,source_order FROM no_intro_dump_sources UNION ALL SELECT set_id,source_order FROM no_intro_releases) ch JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE c.snapshot_key=NEW.snapshot_key GROUP BY ch.set_id HAVING MIN(ch.source_order)<>0 OR MAX(ch.source_order)<>COUNT(*)-1)
 OR EXISTS (SELECT 1 FROM (SELECT 'dump' parent_kind,dump_source_id parent_id,source_order FROM no_intro_dump_details UNION ALL SELECT 'dump',dump_source_id,source_order FROM no_intro_dump_serials UNION ALL SELECT 'dump',dump_source_id,source_order FROM no_intro_dump_files UNION ALL SELECT 'release',release_id,source_order FROM no_intro_release_details UNION ALL SELECT 'release',release_id,source_order FROM no_intro_release_serials UNION ALL SELECT 'release',release_id,source_order FROM no_intro_release_files) child JOIN (SELECT 'dump' parent_kind,d.dump_source_id parent_id,c.snapshot_key FROM no_intro_dump_sources d JOIN catalog_sets s ON s.set_id=d.set_id JOIN catalog_set_groups c USING(set_group_id) UNION ALL SELECT 'release',r.release_id,c.snapshot_key FROM no_intro_releases r JOIN catalog_sets s ON s.set_id=r.set_id JOIN catalog_set_groups c USING(set_group_id)) parent USING(parent_kind,parent_id) WHERE parent.snapshot_key=NEW.snapshot_key GROUP BY child.parent_kind,child.parent_id,child.source_order HAVING COUNT(*)>1)
 OR EXISTS (SELECT 1 FROM no_intro_dump_files d JOIN no_intro_release_files r USING(occurrence_id) JOIN catalog_sets s ON s.set_id=d.set_id JOIN catalog_set_groups c USING(set_group_id) WHERE c.snapshot_key=NEW.snapshot_key)
 OR EXISTS (SELECT 1 FROM no_intro_dump_file_digests d JOIN no_intro_dump_files f USING(occurrence_id) JOIN catalog_sets s ON s.set_id=f.set_id JOIN catalog_set_groups c USING(set_group_id) LEFT JOIN occurrence_digest_assertions a ON a.occurrence_id=d.occurrence_id AND a.digest_id=d.digest_id AND a.provenance='source_declared' AND a.scope=CASE d.field_kind WHEN 4 THEN 'source_origin' ELSE f.evidence_scope END WHERE c.snapshot_key=NEW.snapshot_key AND d.digest_id IS NOT NULL AND a.digest_id IS NULL)
 OR EXISTS (SELECT 1 FROM no_intro_release_file_digests d JOIN no_intro_release_files f USING(occurrence_id) JOIN catalog_sets s ON s.set_id=f.set_id JOIN catalog_set_groups c USING(set_group_id) LEFT JOIN occurrence_digest_assertions a ON a.occurrence_id=d.occurrence_id AND a.digest_id=d.digest_id AND a.provenance='source_declared' AND a.scope=f.evidence_scope WHERE c.snapshot_key=NEW.snapshot_key AND d.digest_id IS NOT NULL AND a.digest_id IS NULL)
 OR EXISTS (SELECT 1 FROM catalog_sets s JOIN catalog_set_groups c USING(set_group_id) WHERE c.snapshot_key=NEW.snapshot_key GROUP BY s.set_group_id HAVING MIN(s.list_order)<>0 OR MAX(s.list_order)<>COUNT(*)-1)
 OR EXISTS (SELECT 1 FROM asset_occurrences o JOIN catalog_sets s ON s.set_id=o.record_id JOIN catalog_set_groups c USING(set_group_id) WHERE c.snapshot_key=NEW.snapshot_key AND o.claim_kind IN ('no_intro_database_source_file','no_intro_database_release_file') GROUP BY o.record_id HAVING MIN(o.occurrence_order)<>0 OR MAX(o.occurrence_order)<>COUNT(*)-1)
 OR EXISTS (SELECT 1 FROM (SELECT dump_source_id owner_id,source_order FROM no_intro_dump_details UNION ALL SELECT dump_source_id,source_order FROM no_intro_dump_serials UNION ALL SELECT dump_source_id,source_order FROM no_intro_dump_files) x JOIN no_intro_dump_sources d ON d.dump_source_id=x.owner_id JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE c.snapshot_key=NEW.snapshot_key GROUP BY x.owner_id HAVING MIN(x.source_order)<>0 OR MAX(x.source_order)<>COUNT(*)-1)
 OR EXISTS (SELECT 1 FROM (SELECT release_id owner_id,source_order FROM no_intro_release_details UNION ALL SELECT release_id,source_order FROM no_intro_release_serials UNION ALL SELECT release_id,source_order FROM no_intro_release_files) x JOIN no_intro_releases r ON r.release_id=x.owner_id JOIN catalog_sets s USING(set_id) JOIN catalog_set_groups c USING(set_group_id) WHERE c.snapshot_key=NEW.snapshot_key GROUP BY x.owner_id HAVING MIN(x.source_order)<>0 OR MAX(x.source_order)<>COUNT(*)-1)
 )
BEGIN SELECT RAISE(ABORT,'No-Intro database snapshot fails native completeness or ownership checks'); END;

-- Reject publisher hash assertions that have no matching typed source field.
CREATE TRIGGER no_intro_database_reject_unowned_hash_assertions BEFORE INSERT ON snapshot_publications
WHEN EXISTS (SELECT 1 FROM catalog_snapshots s JOIN parser_interpretations p USING(interpretation_key)
            WHERE s.snapshot_key=NEW.snapshot_key
              AND p.format IN ('no-intro-database-xml-compatible','no-intro-database-xml-nul-compatible'))
 AND EXISTS (
    WITH expected(occurrence_id,digest_id,scope) AS (
        SELECT d.occurrence_id,d.digest_id,
               CASE d.field_kind WHEN 4 THEN 'source_origin' ELSE f.evidence_scope END
        FROM no_intro_dump_file_digests d JOIN no_intro_dump_files f USING(occurrence_id)
        JOIN catalog_sets s ON s.set_id=f.set_id JOIN catalog_set_groups c USING(set_group_id)
        WHERE c.snapshot_key=NEW.snapshot_key AND d.digest_id IS NOT NULL
        UNION
        SELECT d.occurrence_id,d.digest_id,f.evidence_scope
        FROM no_intro_release_file_digests d JOIN no_intro_release_files f USING(occurrence_id)
        JOIN catalog_sets s ON s.set_id=f.set_id JOIN catalog_set_groups c USING(set_group_id)
        WHERE c.snapshot_key=NEW.snapshot_key AND d.digest_id IS NOT NULL
    )
    SELECT a.occurrence_id,a.digest_id,a.scope
    FROM occurrence_digest_assertions a JOIN asset_occurrences o USING(occurrence_id)
    JOIN catalog_sets s ON s.set_id=o.record_id JOIN catalog_set_groups c USING(set_group_id)
    WHERE c.snapshot_key=NEW.snapshot_key AND a.provenance='source_declared'
      AND o.claim_kind IN ('no_intro_database_source_file','no_intro_database_release_file')
    EXCEPT SELECT occurrence_id,digest_id,scope FROM expected
 )
BEGIN SELECT RAISE(ABORT,'No-Intro snapshot has a source hash assertion without a typed source field'); END;

CREATE TRIGGER no_intro_database_reject_missing_field_orders BEFORE INSERT ON snapshot_publications
WHEN EXISTS (SELECT 1 FROM catalog_snapshots s JOIN parser_interpretations p USING(interpretation_key)
            WHERE s.snapshot_key=NEW.snapshot_key
              AND p.format IN ('no-intro-database-xml-compatible','no-intro-database-xml-nul-compatible'))
 -- Header children are dense. Attribute positions are lexical ordinals that
 -- also count namespace declarations; the native presence/uniqueness guards
 -- establish their completeness without requiring dense recognized ordinals.
 AND EXISTS (SELECT 1 FROM no_intro_header_fields WHERE snapshot_key=NEW.snapshot_key
             GROUP BY snapshot_key HAVING MIN(source_order)<>0 OR MAX(source_order)<>COUNT(*)-1)
BEGIN SELECT RAISE(ABORT,'No-Intro snapshot has missing or noncontiguous header fields'); END;

CREATE TRIGGER no_intro_dump_file_digests_require_algorithm BEFORE INSERT ON no_intro_dump_file_digests
WHEN NEW.digest_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM digest_values d WHERE d.digest_id=NEW.digest_id AND d.algorithm=CASE NEW.field_kind WHEN 0 THEN 'crc32' WHEN 1 THEN 'md5' WHEN 2 THEN 'sha1' WHEN 3 THEN 'sha256' WHEN 4 THEN 'sha256' END)
BEGIN SELECT RAISE(ABORT,'No-Intro dump-file digest algorithm does not match its field'); END;
CREATE TRIGGER no_intro_release_nfo_hashes_require_crc32 BEFORE INSERT ON no_intro_release_nfo_hashes
WHEN NEW.hash_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM digest_values d WHERE d.digest_id=NEW.hash_id AND d.algorithm='crc32')
BEGIN SELECT RAISE(ABORT,'No-Intro NFO CRC must be an interned CRC32 value'); END;
CREATE TRIGGER no_intro_release_file_digests_require_algorithm BEFORE INSERT ON no_intro_release_file_digests
WHEN NEW.digest_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM digest_values d WHERE d.digest_id=NEW.digest_id AND d.algorithm=CASE NEW.field_kind WHEN 0 THEN 'crc32' WHEN 1 THEN 'md5' WHEN 2 THEN 'sha1' WHEN 3 THEN 'sha256' END)
BEGIN SELECT RAISE(ABORT,'No-Intro release-file digest algorithm does not match its field'); END;
