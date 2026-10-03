use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    catalog_files::{LogiqxDumpStatus, LogiqxFilePayload, occurrences_for_ids},
    catalog_logiqx::{
        LogiqxDocument, LogiqxForceMerging, LogiqxForceNoDump, LogiqxForcePacking, LogiqxGame,
        LogiqxOptionValue, LogiqxPageLimit, LogiqxRomMode, LogiqxSampleMode, LogiqxYesNo,
        logiqx_for_snapshot,
    },
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
    logiqx::{GameAttribute, LogiqxMode, RomAttribute},
};

#[path = "support/logiqx_dtd15.rs"]
mod dtd_fixture;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

struct Fixture {
    directory: tempfile::TempDir,
    database: Database,
    connection: SqliteConnection,
}

impl Fixture {
    fn new() -> TestResult<Self> {
        let directory = tempfile::tempdir()?;
        let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
        Ok(Self {
            database: Database::open(&path)?,
            connection: SqliteConnection::establish(path.as_str())?,
            directory,
        })
    }

    fn import(
        &self,
        name: &str,
        xml: &str,
        mode: LogiqxMode,
    ) -> TestResult<app::CatalogImportReport> {
        let document_path = Utf8PathBuf::try_from(self.directory.path().join(name))?;
        std::fs::write(&document_path, xml)?;
        Ok(app::import_catalog(
            &self.database,
            &CatalogImportRequest {
                document_path,
                format: CatalogDocumentFormat::Logiqx(mode),
                source_key: PublishingSourceKey::new("strict-logiqx-publisher"),
                source_display_name: "Strict Logiqx publisher".into(),
                catalog_key: CatalogKey::new("strict-logiqx-catalog"),
                catalog_display_name: "Strict Logiqx catalog".into(),
                scope: CatalogScope::Complete,
            },
        )?)
    }
}

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

fn count(connection: &mut SqliteConnection, table: &str) -> TestResult<i64> {
    Ok(sql_query(format!("SELECT count(*) AS count FROM {table}"))
        .get_result::<Count>(connection)?
        .count)
}

#[derive(QueryableByName)]
struct Rule {
    #[diesel(sql_type = Text)]
    rule: String,
}

#[test]
fn strict_and_compatible_editions_have_distinct_immutable_interpretations() -> TestResult {
    let mut fixture = Fixture::new()?;
    let xml = "<datafile><header><name/><description/><version> v </version><author/></header><game name='same'><description/><rom name='same.bin' size='1' sha1='1111111111111111111111111111111111111111'/></game></datafile>";
    let compatible = fixture.import("same.dat", xml, LogiqxMode::ObservedCompatible)?;
    let strict = fixture.import("same.dat", xml, LogiqxMode::StrictDtd15)?;
    assert_eq!(compatible.status, CatalogImportStatus::Succeeded);
    assert_eq!(strict.status, CatalogImportStatus::Succeeded);
    assert_ne!(compatible.snapshot_key, strict.snapshot_key);
    let compatible_key = compatible
        .snapshot_key
        .ok_or("compatible snapshot missing")?;
    let strict_key = strict.snapshot_key.ok_or("strict snapshot missing")?;
    let limit = LogiqxPageLimit::new(1)?;
    let compatible_page = logiqx_for_snapshot(&fixture.database, &compatible_key, None, limit)?;
    let strict_page = logiqx_for_snapshot(&fixture.database, &strict_key, None, limit)?;
    assert_eq!(compatible_page.document, strict_page.document);
    assert_eq!(
        compatible_page.snapshot.document_key,
        strict_page.snapshot.document_key
    );
    assert_ne!(
        compatible_page.snapshot.interpretation_key,
        strict_page.snapshot.interpretation_key
    );
    assert_eq!(strict_page.snapshot.format, "logiqx");
    assert_eq!(
        strict_page.snapshot.declared_version.as_deref(),
        Some(" v ")
    );
    assert_eq!(
        count(&mut fixture.connection, "catalog_contents")?,
        1,
        "both interpretations share eligible whole-file identity"
    );
    let rules = sql_query("SELECT rules_version AS rule FROM parser_interpretations WHERE format='logiqx' ORDER BY rules_version")
        .load::<Rule>(&mut fixture.connection)?;
    assert_eq!(rules.len(), 2);
    assert!(
        rules
            .iter()
            .any(|row| row.rule == "logiqx-declared-text-compat-v2")
    );
    assert!(rules.iter().any(|row| row.rule == "logiqx-dtd-1.5-v1"));
    let before = count(&mut fixture.connection, "catalog_sets")?;
    let reimported = fixture.import("same.dat", xml, LogiqxMode::StrictDtd15)?;
    assert_eq!(reimported.snapshot_key.as_ref(), Some(&strict_key));
    assert_eq!(count(&mut fixture.connection, "catalog_sets")?, before);
    Ok(())
}

#[test]
fn strict_game_sql_callback_precedes_a_late_grammar_error() -> TestResult {
    let mut fixture = Fixture::new()?;
    sql_query("CREATE TRIGGER strict_before_late_header BEFORE INSERT ON logiqx_games BEGIN SELECT RAISE(ABORT, 'strict-game-written-before-late-header'); END")
        .execute(&mut fixture.connection)?;
    let result = fixture.import(
        "late-header.dat",
        "<datafile><game name='first'><description/></game><header><name/><description/><version/><author/></header></datafile>",
        LogiqxMode::StrictDtd15,
    );
    let error = result
        .err()
        .ok_or("expected SQL failure before the malformed header")?;
    assert!(
        error
            .to_string()
            .contains("strict-game-written-before-late-header"),
        "{error}"
    );
    assert_eq!(count(&mut fixture.connection, "snapshot_publications")?, 0);
    assert_eq!(count(&mut fixture.connection, "catalog_sets")?, 0);
    Ok(())
}

#[test]
fn strict_late_grammar_failure_rolls_back_already_written_games_and_uuid_evidence() -> TestResult {
    let mut fixture = Fixture::new()?;
    let baseline = fixture.import(
        "valid.dat",
        "<datafile><game name='old'><description/><rom name='old.bin' size='1' sha1='1111111111111111111111111111111111111111'/></game></datafile>",
        LogiqxMode::StrictDtd15,
    )?;
    assert_eq!(baseline.status, CatalogImportStatus::Succeeded);
    let tables = [
        "catalog_snapshots",
        "catalog_set_groups",
        "catalog_sets",
        "asset_occurrences",
        "catalog_contents",
        "digest_values",
        "occurrence_digest_assertions",
        "occurrence_content_conflicts",
        "logiqx_games",
        "logiqx_rom_claims",
        "logiqx_game_attribute_positions",
        "logiqx_rom_attribute_positions",
        "snapshot_publications",
    ];
    let before = tables
        .iter()
        .map(|table| count(&mut fixture.connection, table))
        .collect::<TestResult<Vec<_>>>()?;
    let failed = fixture.import(
        "invalid.dat",
        "<datafile><game name='new'><description/><rom name='new.bin' size='2' sha1='2222222222222222222222222222222222222222'/></game><header><name/><description/><version/><author/></header></datafile>",
        LogiqxMode::StrictDtd15,
    )?;
    assert_eq!(failed.status, CatalogImportStatus::Failed);
    assert!(failed.snapshot_key.is_none());
    assert_eq!(failed.diagnostic_count, 1);
    for (table, previous) in tables.iter().zip(before) {
        assert_eq!(count(&mut fixture.connection, table)?, previous, "{table}");
    }
    assert_eq!(count(&mut fixture.connection, "documents")?, 2);
    assert_eq!(count(&mut fixture.connection, "import_runs")?, 2);
    let history = app::catalog_snapshot_history(
        &fixture.database,
        &CatalogKey::new("strict-logiqx-catalog"),
    )?;
    assert_eq!(history.len(), 1);
    assert_eq!(Some(&history[0].snapshot), baseline.snapshot_key.as_ref());
    Ok(())
}

#[test]
fn strict_enum_normalization_persists_native_values_without_losing_qname_positions() -> TestResult {
    let mut fixture = Fixture::new()?;
    let xml = "<datafile debug='  yes  '><game name=' boundary ' isbios='&#32;yes&#32;'><description> boundary </description><rom name='a' size='invalid' sha1='1111111111111111111111111111111111111111' status=' good '/></game></datafile>";
    let imported = fixture.import("normalized.dat", xml, LogiqxMode::StrictDtd15)?;
    assert_eq!(imported.status, CatalogImportStatus::Succeeded);
    let snapshot = imported.snapshot_key.ok_or("strict snapshot missing")?;
    let page = logiqx_for_snapshot(&fixture.database, &snapshot, None, LogiqxPageLimit::new(1)?)?;
    assert_eq!(page.document.debug, LogiqxYesNo::Yes);
    assert!(page.document.debug_was_present);
    assert_eq!(page.document.attribute_positions.len(), 1);
    assert_eq!(
        page.document
            .attribute_positions
            .first()
            .ok_or("debug position missing")?
            .location
            .column,
        11
    );
    let game = page.games.first().ok_or("strict game missing")?;
    assert_eq!(game.name, " boundary ");
    assert_eq!(game.is_bios, LogiqxYesNo::Yes);
    assert!(game.is_bios_was_present);
    assert_eq!(game.attribute_positions.len(), 2);
    let bios_position = game
        .attribute_positions
        .iter()
        .find(|position| position.field == GameAttribute::IsBios)
        .ok_or("isbios position missing")?;
    assert_eq!(
        bios_position.location.column,
        i64::try_from(xml.find("isbios=").ok_or("source QName missing")? + 1)?
    );
    assert_eq!(
        game.description
            .as_ref()
            .ok_or("description missing")?
            .value,
        " boundary "
    );
    assert_eq!(game.media.len(), 1);
    let media = game.media.first().ok_or("ROM reference missing")?;
    let occurrences = occurrences_for_ids(&fixture.database, &[media.occurrence_id])?;
    let occurrence = occurrences.first().ok_or("native ROM payload missing")?;
    let Some(LogiqxFilePayload::Rom(rom)) = &occurrence.logiqx_file else {
        return Err("strict ROM payload has wrong native kind".into());
    };
    assert_eq!(rom.size_text.as_deref(), Some("invalid"));
    assert_eq!(rom.status, LogiqxDumpStatus::Good);
    assert!(rom.status_was_present);
    assert!(occurrence.content_id.is_none());
    let status_position = rom
        .attribute_positions
        .iter()
        .find(|position| position.field == RomAttribute::Status)
        .ok_or("status position missing")?;
    assert_eq!(
        status_position.location.column,
        i64::try_from(xml.find("status=").ok_or("source status missing")? + 1)?
    );
    assert_eq!(
        count(&mut fixture.connection, "catalog_contents")?,
        0,
        "invalid declared size cannot issue UUID"
    );
    Ok(())
}

#[test]
fn every_dtd_attribute_and_text_field_is_queryable_from_native_storage() -> TestResult {
    let mut fixture = Fixture::new()?;
    let imported = fixture.import(
        "all-fields.dat",
        dtd_fixture::ALL_DECLARED_FIELDS,
        LogiqxMode::StrictDtd15,
    )?;
    assert_eq!(imported.status, CatalogImportStatus::Succeeded);
    let snapshot = imported.snapshot_key.ok_or("strict snapshot missing")?;
    let page = logiqx_for_snapshot(&fixture.database, &snapshot, None, LogiqxPageLimit::new(1)?)?;
    assert_eq!(page.games.len(), 1);
    assert!(page.next_cursor.is_none());
    let game = page.games.first().ok_or("native game missing")?;
    let positions = assert_all_document_fields(&page.document)?
        + assert_all_game_fields(game)?
        + assert_all_media_fields(&fixture.database, game)?;
    assert_eq!(
        positions, 44,
        "every DTD attribute has a native position owner"
    );
    assert_eq!(count(&mut fixture.connection, "catalog_contents")?, 0);
    Ok(())
}

fn assert_all_document_fields(document: &LogiqxDocument) -> TestResult<usize> {
    assert_eq!(document.build.as_deref(), Some(""));
    assert_eq!(document.debug, LogiqxYesNo::Yes);
    assert!(document.debug_was_present);
    let header = document.header.as_ref().ok_or("native header missing")?;
    for field in [
        &header.name,
        &header.description,
        &header.category,
        &header.version,
        &header.date,
        &header.author,
        &header.email,
        &header.homepage,
        &header.url,
        &header.comment,
    ] {
        assert_eq!(field.as_deref(), Some(""), "present empty header text");
    }
    assert_eq!(header.text_positions.len(), 10);
    let cmp = document
        .clrmamepro
        .as_ref()
        .ok_or("native CMP options missing")?;
    assert_eq!(cmp.header.as_deref(), Some(""));
    assert_eq!(
        cmp.forcemerging,
        LogiqxOptionValue::Explicit(LogiqxForceMerging::None)
    );
    assert_eq!(
        cmp.forcenodump,
        LogiqxOptionValue::Explicit(LogiqxForceNoDump::Ignore)
    );
    assert_eq!(
        cmp.forcepacking,
        LogiqxOptionValue::Explicit(LogiqxForcePacking::Unzip)
    );
    let rc = document
        .romcenter
        .as_ref()
        .ok_or("native RomCenter options missing")?;
    assert_eq!(rc.plugin.as_deref(), Some(""));
    assert_eq!(
        rc.rommode,
        LogiqxOptionValue::Explicit(LogiqxRomMode::Merged)
    );
    assert_eq!(
        rc.biosmode,
        LogiqxOptionValue::Explicit(LogiqxRomMode::Unmerged)
    );
    assert_eq!(
        rc.samplemode,
        LogiqxOptionValue::Explicit(LogiqxSampleMode::Unmerged)
    );
    for field in [rc.lockrommode, rc.lockbiosmode, rc.locksamplemode] {
        assert_eq!(field, LogiqxOptionValue::Explicit(LogiqxYesNo::Yes));
    }
    assert_eq!(document.attribute_positions.len(), 2);
    assert_eq!(cmp.attribute_positions.len(), 4);
    assert_eq!(rc.attribute_positions.len(), 7);
    Ok(13)
}

fn assert_all_game_fields(game: &LogiqxGame) -> TestResult<usize> {
    assert_eq!(game.name, "");
    for field in [&game.sourcefile, &game.board, &game.rebuildto] {
        assert_eq!(field.as_deref(), Some(""));
    }
    assert_eq!(game.is_bios, LogiqxYesNo::Yes);
    assert!(game.is_bios_was_present);
    for parent in [&game.cloneof, &game.romof, &game.sampleof] {
        assert_eq!(
            parent.as_ref().ok_or("native parent missing")?.target_name,
            ""
        );
    }
    for text in [&game.description, &game.year, &game.manufacturer] {
        assert_eq!(text.as_ref().ok_or("native game text missing")?.value, "");
    }
    assert_eq!(game.comments.len(), 1);
    assert_eq!(
        game.comments.first().ok_or("native comment missing")?.text,
        ""
    );
    assert_eq!(game.releases.len(), 1);
    let release = game.releases.first().ok_or("native release missing")?;
    assert_eq!(release.name, "");
    assert_eq!(release.region, "");
    assert_eq!(release.language.as_deref(), Some(""));
    assert_eq!(release.date.as_deref(), Some(""));
    assert_eq!(release.is_default, LogiqxYesNo::Yes);
    assert!(release.default_was_present);
    assert_eq!(game.bios_sets.len(), 1);
    let bios = game.bios_sets.first().ok_or("native BIOS set missing")?;
    assert_eq!(bios.name, "");
    assert_eq!(bios.description, "");
    assert_eq!(bios.is_default, LogiqxYesNo::Yes);
    assert!(bios.default_was_present);
    assert_eq!(game.archives.len(), 1);
    let archive = game.archives.first().ok_or("native archive missing")?;
    assert_eq!(archive.name, "");
    assert!(game.device_references.is_empty());
    assert_eq!(game.attribute_positions.len(), 8);
    assert_eq!(release.attribute_positions.len(), 5);
    assert_eq!(bios.attribute_positions.len(), 3);
    assert_eq!(archive.attribute_positions.len(), 1);
    Ok(17)
}

fn assert_all_media_fields(database: &Database, game: &LogiqxGame) -> TestResult<usize> {
    assert_eq!(game.media.len(), 3);
    let ids = game
        .media
        .iter()
        .map(|media| media.occurrence_id)
        .collect::<Vec<_>>();
    let occurrences = occurrences_for_ids(database, &ids)?;
    assert_eq!(occurrences.len(), 3);
    let mut positions = 0;
    for (index, id) in ids.iter().enumerate() {
        let occurrence = occurrences
            .iter()
            .find(|item| item.occurrence_id == *id)
            .ok_or("native media occurrence missing")?;
        assert_eq!(occurrence.provenance.asset_name.as_deref(), Some(""));
        assert!(occurrence.content_id.is_none());
        let payload = occurrence
            .logiqx_file
            .as_ref()
            .ok_or("native media payload missing")?;
        positions += match payload {
            LogiqxFilePayload::Rom(rom) => {
                assert_eq!(index, 0);
                for field in [
                    &rom.size_text,
                    &rom.crc_text,
                    &rom.sha1_text,
                    &rom.md5_text,
                    &rom.merge_name,
                    &rom.date,
                ] {
                    assert_eq!(field.as_deref(), Some(""));
                }
                assert_eq!(rom.status, LogiqxDumpStatus::CompatibilityVerified);
                assert!(rom.status_was_present);
                assert_eq!(rom.attribute_positions.len(), 8);
                8
            }
            LogiqxFilePayload::Disk(disk) => {
                assert_eq!(index, 1);
                for field in [&disk.sha1_text, &disk.md5_text, &disk.merge_name] {
                    assert_eq!(field.as_deref(), Some(""));
                }
                assert_eq!(disk.status, LogiqxDumpStatus::BadDump);
                assert!(disk.status_was_present);
                assert_eq!(disk.attribute_positions.len(), 5);
                5
            }
            LogiqxFilePayload::Sample(sample) => {
                assert_eq!(index, 2);
                assert_eq!(sample.attribute_positions.len(), 1);
                1
            }
        };
    }
    Ok(positions)
}
