use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query, sql_types::BigInt,
};
use diesel::{
    connection::SimpleConnection,
    sql_types::{Binary, Nullable, Text},
};
use mame_coalesce::database::Database;
use mame_coalesce::{
    DocumentStore, RestorePolicy,
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    create_backup,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey, RelationshipEndpoint},
    no_intro_db_xml::NoIntroDatabaseMode,
    restore_backup,
};
use std::io::Write;

#[path = "support/import_warning_fixture.rs"]
mod import_warning_fixture;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

#[test]
fn a_fresh_database_has_distinct_native_export_source_and_release_owners() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = camino::Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let _database = Database::open(&path)?;
    let mut connection = SqliteConnection::establish(path.as_str())?;
    let owners = sql_query(
        "SELECT COUNT(*) AS value FROM sqlite_schema WHERE type='table' AND name IN (\
         'no_intro_exports','no_intro_archive_descriptions','no_intro_dump_sources',\
         'no_intro_dump_files','no_intro_releases','no_intro_release_files')",
    )
    .get_result::<Count>(&mut connection)?;
    assert_eq!(
        owners.value, 6,
        "export sources/releases need actual native owners"
    );
    Ok(())
}

fn request(path: camino::Utf8PathBuf, mode: NoIntroDatabaseMode) -> CatalogImportRequest {
    CatalogImportRequest {
        document_path: path,
        format: CatalogDocumentFormat::NoIntroDatabase(mode),
        source_key: PublishingSourceKey::new("native-export-source"),
        source_display_name: "Native export source".into(),
        catalog_key: CatalogKey::new("native-export"),
        catalog_display_name: "Native export".into(),
        scope: CatalogScope::Complete,
    }
}

#[derive(QueryableByName)]
struct WarningSpan {
    #[diesel(sql_type = Binary)]
    source_excerpt: Vec<u8>,
    #[diesel(sql_type = Text)]
    excerpt_view: String,
    #[diesel(sql_type = BigInt)]
    excerpt_start_byte: i64,
    #[diesel(sql_type = BigInt)]
    problem_start_byte: i64,
    #[diesel(sql_type = BigInt)]
    problem_end_byte: i64,
    #[diesel(sql_type = BigInt)]
    source_problem_start_byte: i64,
    #[diesel(sql_type = BigInt)]
    source_problem_end_byte: i64,
    #[diesel(sql_type = Nullable<BigInt>)]
    original_problem_start_byte: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    original_problem_end_byte: Option<i64>,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
}

#[test]
fn recovered_warning_spans_retain_raw_encoded_bytes_and_reimport_has_a_new_run() -> TestResult {
    let prefix = "<datafile>\r\n<game name='é😀'><source><details comment1='a";
    let xml = format!("{prefix}\0b'/></source></game></datafile>");
    for (utf16, gzip) in [(false, false), (true, false), (false, true), (true, true)] {
        let directory = tempfile::tempdir()?;
        let path = camino::Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
        let document = camino::Utf8PathBuf::try_from(directory.path().join("export.xml"))?;
        let bytes = encoded_document(&xml, utf16, gzip)?;
        std::fs::write(&document, &bytes)?;
        let database = Database::open(&path)?;
        let request = request(document, NoIntroDatabaseMode::NullRecoveryCompatible);
        let first = app::import_catalog(&database, &request)?;
        assert_eq!(first.status, CatalogImportStatus::Succeeded);
        assert_eq!(first.diagnostic_count, 1);
        let second = app::import_catalog(&database, &request)?;
        assert_eq!(second.snapshot_key, first.snapshot_key);
        assert_ne!(second.run_key, first.run_key);
        assert_eq!(second.diagnostic_count, 1);
        let mut connection = SqliteConnection::establish(path.as_str())?;
        let spans=sql_query("SELECT source_excerpt,excerpt_view,excerpt_start_byte,problem_start_byte,problem_end_byte,source_problem_start_byte,source_problem_end_byte,original_problem_start_byte,original_problem_end_byte,source_line,source_column FROM import_diagnostics WHERE severity='warning' ORDER BY run_key").load::<WarningSpan>(&mut connection)?;
        assert_eq!(spans.len(), 2);
        let offset = i64::try_from(if utf16 {
            2 + prefix.encode_utf16().count() * 2
        } else {
            prefix.len()
        })?;
        for span in spans {
            assert_eq!(
                span.source_excerpt,
                if utf16 { vec![0, 0] } else { vec![0] }
            );
            assert_eq!(
                span.excerpt_view,
                if gzip {
                    "transport_decoded_xml_bytes"
                } else {
                    "retained_original_bytes"
                }
            );
            assert_eq!(span.excerpt_start_byte, offset);
            assert_eq!(
                (span.problem_start_byte, span.problem_end_byte),
                (0, if utf16 { 2 } else { 1 })
            );
            assert_eq!(
                span.original_problem_start_byte,
                if gzip { None } else { Some(offset) }
            );
            let width = if utf16 { 2 } else { 1 };
            assert_eq!(
                (span.source_problem_start_byte, span.source_problem_end_byte),
                (offset, offset + width)
            );
            assert_eq!(
                span.original_problem_end_byte,
                if gzip { None } else { Some(offset + width) }
            );
            assert_eq!(span.source_line, 2);
            assert_eq!(
                span.source_column,
                i64::try_from(
                    prefix
                        .rsplit_once('\n')
                        .ok_or("line break")?
                        .1
                        .chars()
                        .count()
                )? + 1
            );
        }
        assert_eq!(
            sql_query("SELECT COUNT(*) AS value FROM no_intro_dump_sources")
                .get_result::<Count>(&mut connection)?
                .value,
            1
        );
        assert_eq!(
            sql_query("SELECT COUNT(*) AS value FROM no_intro_dump_details_diagnostics")
                .get_result::<Count>(&mut connection)?
                .value,
            2
        );
        let snapshot = first.snapshot_key.ok_or("snapshot")?;
        assert_eq!(
            DocumentStore::open(path.as_str())?.load_snapshot(&snapshot)?,
            bytes
        );
    }
    Ok(())
}

fn encoded_document(xml: &str, utf16: bool, gzip: bool) -> Result<Vec<u8>, std::io::Error> {
    let raw = if utf16 {
        let mut encoded = vec![0xff, 0xfe];
        encoded.extend(xml.encode_utf16().flat_map(u16::to_le_bytes));
        encoded
    } else {
        xml.as_bytes().to_vec()
    };
    if !gzip {
        return Ok(raw);
    }
    let mut writer = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    writer.write_all(&raw)?;
    writer.finish()
}

fn recovered_owner_database()
-> Result<(tempfile::TempDir, Database, SqliteConnection), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = camino::Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let document = camino::Utf8PathBuf::try_from(directory.path().join("export.xml"))?;
    std::fs::write(
        &document,
        "<datafile><game name='g'><source><details comment1='a\0b'/></source></game></datafile>",
    )?;
    let database = Database::open(&path)?;
    let report = app::import_catalog(
        &database,
        &request(document, NoIntroDatabaseMode::NullRecoveryCompatible),
    )?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    let mut connection = SqliteConnection::establish(path.as_str())?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF")?;
    Ok((directory, database, connection))
}

#[test]
fn an_unowned_diagnostic_cannot_replace_owned_evidence_through_update() -> TestResult {
    let (_directory, _database, mut connection) = recovered_owner_database()?;
    connection.batch_execute("INSERT INTO import_diagnostics(diagnostic_key,run_key,diagnostic_order,document_key,code,message) SELECT 'unowned-diagnostic',r.run_key,(SELECT COUNT(*) FROM import_diagnostics d WHERE d.run_key=r.run_key),r.document_key,'parse_failed','unowned' FROM import_runs r LIMIT 1")?;
    let replacement = connection.batch_execute("UPDATE OR REPLACE import_diagnostics SET diagnostic_key=(SELECT diagnostic_key FROM no_intro_dump_details_diagnostics LIMIT 1) WHERE diagnostic_key='unowned-diagnostic'");
    assert!(
        replacement.is_err(),
        "unowned evidence cannot steal a native owner's diagnostic key"
    );
    Ok(())
}

#[test]
fn an_unowned_run_cannot_replace_an_owned_run_through_update() -> TestResult {
    let (_directory, _database, mut connection) = recovered_owner_database()?;
    connection.batch_execute("INSERT INTO import_runs(run_key,catalog_key,document_key,interpretation_key,status) SELECT 'unowned-run',catalog_key,document_key,interpretation_key,'failed' FROM import_runs LIMIT 1")?;
    let replacement = connection.batch_execute("UPDATE OR REPLACE import_runs SET run_key=(SELECT run_key FROM no_intro_dump_details_diagnostics LIMIT 1) WHERE run_key='unowned-run'");
    assert!(
        replacement.is_err(),
        "unowned run cannot steal a native diagnostic's import key"
    );
    Ok(())
}

#[test]
fn diagnostic_owner_links_reject_wrong_imports_and_evidence_rewrites() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = camino::Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&path)?;
    let document = camino::Utf8PathBuf::try_from(directory.path().join("export.xml"))?;
    let xml = "<datafile><game name='same'><source><details comment1='a\0b'/></source><release><details comment='c\0d'/></release></game></datafile>";
    std::fs::write(&document, xml)?;
    let request = request(
        document.clone(),
        NoIntroDatabaseMode::NullRecoveryCompatible,
    );
    let first = app::import_catalog(&database, &request)?;
    std::fs::write(&document, format!("{xml} "))?;
    let second = app::import_catalog(&database, &request)?;
    assert_ne!(first.snapshot_key, second.snapshot_key);
    assert_eq!(first.diagnostic_count, 2);
    assert_eq!(second.diagnostic_count, 2);
    let mut connection = SqliteConnection::establish(path.as_str())?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF")?;

    for (table, owner) in [
        ("no_intro_dump_details_diagnostics", "dump_source_id"),
        ("no_intro_release_details_diagnostics", "release_id"),
    ] {
        // A fresh unlinked copy avoids a duplicate-PK failure masking the owner guard.
        sql_query(format!("INSERT INTO import_diagnostics(diagnostic_key,run_key,diagnostic_order,document_key,severity,code,message,source_line,source_column,coordinate_view,column_convention) SELECT ?,d.run_key,(SELECT COUNT(*) FROM import_diagnostics existing WHERE existing.run_key=d.run_key),d.document_key,d.severity,d.code,d.message,d.source_line,d.source_column,d.coordinate_view,d.column_convention FROM import_diagnostics d JOIN {table} l USING(diagnostic_key) WHERE d.run_key=?"))
            .bind::<Text, _>(table)
            .bind::<Text, _>(first.run_key.to_string())
            .execute(&mut connection)?;
        let wrong_import = sql_query(format!("INSERT INTO {table}(diagnostic_key,run_key,snapshot_key,{owner}) SELECT ?,?,snapshot_key,{owner} FROM {table} WHERE run_key=?"))
            .bind::<Text, _>(table)
            .bind::<Text, _>(first.run_key.to_string())
            .bind::<Text, _>(second.run_key.to_string())
            .execute(&mut connection);
        assert!(
            wrong_import.is_err(),
            "equal coordinates do not establish source ownership"
        );
        // The same evidence can be linked to its actual owner.
        sql_query(format!("INSERT INTO {table}(diagnostic_key,run_key,snapshot_key,{owner}) SELECT ?,run_key,snapshot_key,{owner} FROM {table} WHERE run_key=?"))
            .bind::<Text, _>(table)
            .bind::<Text, _>(first.run_key.to_string())
            .execute(&mut connection)?;
        for operation in [
            format!("UPDATE {table} SET {owner}={owner}"),
            format!("DELETE FROM {table}"),
            format!("INSERT OR REPLACE INTO {table} SELECT * FROM {table}"),
        ] {
            assert!(connection.batch_execute(&operation).is_err());
        }
    }
    for operation in [
        "UPDATE import_diagnostics SET source_column=source_column+1",
        "DELETE FROM import_diagnostics",
        "INSERT OR REPLACE INTO import_diagnostics SELECT * FROM import_diagnostics",
        "UPDATE import_runs SET status='failed'",
        "DELETE FROM import_runs",
        "INSERT OR REPLACE INTO import_runs SELECT * FROM import_runs",
    ] {
        assert!(connection.batch_execute(operation).is_err(), "{operation}");
    }
    Ok(())
}

#[test]
fn paired_backup_restores_native_fields_recovery_and_unresolved_archive_relationships() -> TestResult
{
    let directory = tempfile::tempdir()?;
    let path = camino::Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let document = camino::Utf8PathBuf::try_from(directory.path().join("export.xml"))?;
    let xml=b"<datafile><game name='g'><archive number='0001' clone='0002' mergeof='missing'/><source><details comment1='a\0b'/><file id='same' sha1='0123456789abcdef0123456789abcdef01234567'/></source><release><details nfo_crc32='01234567'/><file id='same' crc32='bad'/></release></game></datafile>";
    std::fs::write(&document, xml)?;
    let database = Database::open(&path)?;
    let report = app::import_catalog(
        &database,
        &request(document, NoIntroDatabaseMode::NullRecoveryCompatible),
    )?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    let snapshot = report.snapshot_key.ok_or("snapshot")?;
    let explanations = app::explain_relationships(&database)?;
    assert_eq!(explanations.len(), 2);
    for explanation in &explanations {
        assert!(matches!(
            &explanation.claim.subject,
            RelationshipEndpoint::NoIntroArchive { .. }
        ));
        assert!(matches!(
            &explanation.claim.target,
            RelationshipEndpoint::NoIntroArchiveReference { .. }
        ));
    }
    let backup = camino::Utf8PathBuf::try_from(directory.path().join("backup.sqlite"))?;
    let restored = camino::Utf8PathBuf::try_from(directory.path().join("restored.sqlite"))?;
    create_backup(&path, &backup)?;
    restore_backup(&backup, &restored, RestorePolicy::CreateNew)?;
    let restored_database = Database::open(&restored)?;
    assert_eq!(
        app::explain_relationships(&restored_database)?,
        explanations
    );
    assert_eq!(
        DocumentStore::open(restored.as_str())?.load_snapshot(&snapshot)?,
        xml
    );
    let mut connection = SqliteConnection::establish(restored.as_str())?;
    for (table, count) in [
        ("no_intro_dump_files", 1),
        ("no_intro_release_files", 1),
        ("no_intro_release_nfo_hashes", 1),
        ("import_diagnostics", 1),
        ("no_intro_dump_details_diagnostics", 1),
    ] {
        assert_eq!(
            sql_query(format!("SELECT COUNT(*) AS value FROM {table}"))
                .get_result::<Count>(&mut connection)?
                .value,
            count
        );
    }
    Ok(())
}

#[test]
fn late_failure_after_recovery_keeps_only_error_and_no_rolled_back_owner_links() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = camino::Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let document = camino::Utf8PathBuf::try_from(directory.path().join("export.xml"))?;
    std::fs::write(&document,b"<datafile><game name='g'><source><details comment1='a\0b'/></source></game></datafile><bad/>")?;
    let database = Database::open(&path)?;
    let report = app::import_catalog(
        &database,
        &request(document, NoIntroDatabaseMode::NullRecoveryCompatible),
    )?;
    assert_eq!(report.status, CatalogImportStatus::Failed);
    let mut connection = SqliteConnection::establish(path.as_str())?;
    for table in [
        "no_intro_dump_details_diagnostics",
        "no_intro_release_details_diagnostics",
        "no_intro_dump_details",
        "snapshot_publications",
    ] {
        assert_eq!(
            sql_query(format!("SELECT COUNT(*) AS value FROM {table}"))
                .get_result::<Count>(&mut connection)?
                .value,
            0
        );
    }
    assert_eq!(
        sql_query("SELECT COUNT(*) AS value FROM import_diagnostics WHERE severity='error'")
            .get_result::<Count>(&mut connection)?
            .value,
        1
    );
    assert_eq!(
        sql_query("SELECT COUNT(*) AS value FROM import_diagnostics WHERE severity='warning'")
            .get_result::<Count>(&mut connection)?
            .value,
        0
    );
    Ok(())
}

#[test]
fn a_non_nul_forbidden_character_cannot_publish_in_recovery_mode() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = camino::Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let document = camino::Utf8PathBuf::try_from(directory.path().join("export.xml"))?;
    std::fs::write(
        &document,
        b"<datafile><game name='g'><source><details comment1='\x01'/></source></game></datafile>",
    )?;
    let database = Database::open(&path)?;
    let report = app::import_catalog(
        &database,
        &request(document, NoIntroDatabaseMode::NullRecoveryCompatible),
    )?;
    assert_eq!(report.status, CatalogImportStatus::Failed);
    assert!(report.snapshot_key.is_none());
    let mut connection = SqliteConnection::establish(path.as_str())?;
    assert_eq!(
        sql_query("SELECT COUNT(*) AS value FROM snapshot_publications")
            .get_result::<Count>(&mut connection)?
            .value,
        0
    );
    assert_eq!(
        sql_query("SELECT COUNT(*) AS value FROM import_diagnostics WHERE severity='error'")
            .get_result::<Count>(&mut connection)?
            .value,
        1
    );
    Ok(())
}

#[test]
fn recovered_nuls_cover_every_native_owner_and_keep_exact_source_evidence() -> TestResult {
    let xml = import_warning_fixture::native_owner_nul_document();
    let directory = tempfile::tempdir()?;
    let path = camino::Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let document = camino::Utf8PathBuf::try_from(directory.path().join("export.xml"))?;
    std::fs::write(&document, xml.as_bytes())?;

    let database = Database::open(&path)?;
    let report = app::import_catalog(
        &database,
        &request(document, NoIntroDatabaseMode::NullRecoveryCompatible),
    )?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    assert_eq!(report.diagnostic_count, 14);

    let mut connection = SqliteConnection::establish(path.as_str())?;
    let warnings = sql_query(
        "SELECT source_excerpt,excerpt_view,excerpt_start_byte,problem_start_byte,\
         problem_end_byte,source_problem_start_byte,source_problem_end_byte,\
         original_problem_start_byte,original_problem_end_byte,source_line,source_column \
         FROM import_diagnostics WHERE severity='warning' \
         ORDER BY source_problem_start_byte",
    )
    .load::<WarningSpan>(&mut connection)?;
    let nul_offsets = xml
        .match_indices('\0')
        .map(|(offset, _)| offset)
        .collect::<Vec<_>>();
    assert_eq!(nul_offsets.len(), 14);
    assert_eq!(warnings.len(), nul_offsets.len());

    for (warning, offset) in warnings.iter().zip(nul_offsets) {
        let offset = i64::try_from(offset)?;
        assert_eq!(warning.source_excerpt, [0]);
        assert_eq!(warning.excerpt_view, "retained_original_bytes");
        assert_eq!(warning.excerpt_start_byte, offset);
        assert_eq!(
            (warning.problem_start_byte, warning.problem_end_byte),
            (0, 1)
        );
        assert_eq!(
            (
                warning.source_problem_start_byte,
                warning.source_problem_end_byte
            ),
            (offset, offset + 1)
        );
        assert_eq!(warning.original_problem_start_byte, Some(offset));
        assert_eq!(warning.original_problem_end_byte, Some(offset + 1));
        assert_eq!(
            (warning.source_line, warning.source_column),
            independent_scalar_position(&xml, usize::try_from(offset)?)
        );
    }
    Ok(())
}

fn independent_scalar_position(xml: &str, byte_offset: usize) -> (i64, i64) {
    let mut line = 1_i64;
    let mut column = 1_i64;
    let mut previous_was_carriage_return = false;

    for (_, character) in xml
        .char_indices()
        .take_while(|(offset, _)| *offset < byte_offset)
    {
        match character {
            '\r' => {
                line += 1;
                column = 1;
                previous_was_carriage_return = true;
            }
            '\n' => {
                if !previous_was_carriage_return {
                    line += 1;
                }
                column = 1;
                previous_was_carriage_return = false;
            }
            _ => {
                column += 1;
                previous_was_carriage_return = false;
            }
        }
    }
    (line, column)
}
