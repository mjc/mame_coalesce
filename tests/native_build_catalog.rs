use camino::Utf8PathBuf;
use diesel::{
    QueryableByName, SqliteConnection,
    prelude::*,
    sql_query,
    sql_types::{BigInt, Nullable, Text},
};
use mame_coalesce::{
    app::{
        self, AuditRefresh, AuditRequest, BuildPlanRequest, CatalogDocumentFormat,
        CatalogImportRequest, DatImportRequest, SourceScanRequest,
    },
    database::Database,
    domain::{
        BuildMode, CatalogKey, CatalogScope, MatchingPolicy, MissingContentPolicy,
        PublishingSourceKey, SetSelection,
    },
};

const COMPLETE_ROM: &str = r#"<rom name="game.rom" size="3" crc="352441c2" md5="900150983cd24fb0d6963f7d28e17f72" sha1="a9993e364706816aba3e25717850c26c9cd0d89d"/>"#;

struct Fixture {
    directory: tempfile::TempDir,
    database: Database,
    database_path: Utf8PathBuf,
    dat_path: Utf8PathBuf,
    source_path: Utf8PathBuf,
}

impl Fixture {
    fn new(contents: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let root = Utf8PathBuf::from_path_buf(directory.path().to_path_buf())
            .map_err(|path| format!("non-UTF-8 fixture path: {}", path.display()))?;
        let database_path = root.join("catalog.sqlite");
        let dat_path = root.join("input.dat");
        let source_path = root.join("roms");
        std::fs::create_dir(&source_path)?;
        std::fs::write(&dat_path, contents)?;
        let database = Database::open(&database_path)?;
        Ok(Self {
            directory,
            database,
            database_path,
            dat_path,
            source_path,
        })
    }

    fn replace_dat(&self, contents: &str) -> Result<(), Box<dyn std::error::Error>> {
        std::fs::write(&self.dat_path, contents)?;
        Ok(())
    }

    fn import_dat(&self) -> mame_coalesce::Result<app::DatImportReport> {
        self.import_at(&self.dat_path)
    }

    fn import_at(&self, dat_path: &Utf8PathBuf) -> mame_coalesce::Result<app::DatImportReport> {
        app::import_dat(
            &self.database,
            &DatImportRequest {
                dat_path: dat_path.clone(),
            },
        )
    }

    fn connection(&self) -> Result<SqliteConnection, diesel::ConnectionError> {
        SqliteConnection::establish(self.database_path.as_str())
    }
}

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

#[derive(QueryableByName)]
struct TextRow {
    #[diesel(sql_type = Text)]
    value: String,
}

#[derive(QueryableByName)]
struct SnapshotFactsRow {
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Nullable<BigInt>)]
    size: Option<i64>,
    #[diesel(sql_type = BigInt)]
    md5_count: i64,
    #[diesel(sql_type = BigInt)]
    sha1_count: i64,
}

const fn plan_request(dat_path: Utf8PathBuf, source_path: Utf8PathBuf) -> BuildPlanRequest {
    BuildPlanRequest {
        dat_path,
        source_path,
        mode: BuildMode::PerGame,
        matching_policy: MatchingPolicy::Sha1Compatibility,
        missing_policy: MissingContentPolicy::AllowPartial,
        set_selection: SetSelection::All,
    }
}

fn import_catalog(
    database: &Database,
    path: &Utf8PathBuf,
    catalog_key: &str,
    header_name: &str,
) -> mame_coalesce::Result<app::CatalogImportReport> {
    app::import_catalog(
        database,
        &CatalogImportRequest {
            document_path: path.clone(),
            format: CatalogDocumentFormat::Logiqx,
            source_key: PublishingSourceKey::new(format!("publisher-{catalog_key}")),
            source_display_name: format!("Publisher {catalog_key}"),
            catalog_key: CatalogKey::new(catalog_key),
            catalog_display_name: header_name.to_owned(),
            scope: CatalogScope::Complete,
        },
    )
}

fn write_dat(
    directory: &tempfile::TempDir,
    name: &str,
    contents: &str,
) -> Result<Utf8PathBuf, Box<dyn std::error::Error>> {
    let path = Utf8PathBuf::from_path_buf(directory.path().join(name))
        .map_err(|path| format!("non-UTF-8 DAT path: {}", path.display()))?;
    std::fs::write(&path, contents)?;
    Ok(path)
}

#[test]
fn import_dat_publishes_native_catalog_and_retains_source_outside_sqlite()
-> Result<(), Box<dyn std::error::Error>> {
    let document = include_bytes!("../fixtures/catalog/logiqx/catalog-a-v1.dat");
    let fixture = Fixture::new(std::str::from_utf8(document)?)?;
    let _report = fixture.import_dat()?;

    let mut connection = fixture.connection()?;
    let native_publications = sql_query("SELECT COUNT(*) AS count FROM snapshot_publications")
        .get_result::<CountRow>(&mut connection)?;
    assert_eq!(native_publications.count, 1);

    for table in ["data_files", "games", "roms"] {
        let schema_entries = sql_query(
            "SELECT COUNT(*) AS count FROM sqlite_schema \
             WHERE type = 'table' AND name = ?",
        )
        .bind::<Text, _>(table)
        .get_result::<CountRow>(&mut connection)?;
        assert_eq!(schema_entries.count, 0, "legacy table {table} still exists");
    }

    let native_games = sql_query("SELECT COUNT(*) AS count FROM logiqx_games")
        .get_result::<CountRow>(&mut connection)?;
    assert_eq!(native_games.count, 1);

    let document_key = sql_query("SELECT document_key AS value FROM catalog_snapshots")
        .get_result::<TextRow>(&mut connection)?
        .value
        .parse::<mame_coalesce::domain::DocumentKey>()?;
    let retained =
        mame_coalesce::DocumentStore::open(fixture.database_path.as_str())?.load(&document_key)?;
    assert_eq!(retained, document);
    Ok(())
}

#[test]
fn native_import_facts_drive_plan_and_audit_and_keep_optional_evidence_and_wide_sizes()
-> Result<(), Box<dyn std::error::Error>> {
    let document = r#"<datafile><header><name>Evidence catalog</name></header>
      <game name="large"><rom name="large.rom" size="4294967297" sha1="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"/></game>
      <game name="unknown"><rom name="unknown.rom"/></game>
    </datafile>"#;
    let fixture = Fixture::new(document)?;
    fixture.import_dat()?;
    app::scan_source(
        &fixture.database,
        &SourceScanRequest {
            source_path: fixture.source_path.clone(),
            jobs: 1,
        },
    )?;

    let mut connection = fixture.connection()?;
    let native_facts = sql_query(
        "SELECT claim.name AS name, claim.size AS size, \
         (SELECT COUNT(*) FROM occurrence_digest_assertions AS assertion \
          JOIN digest_values AS digest USING (digest_id) \
          WHERE assertion.occurrence_id = occurrence.occurrence_id \
            AND digest.algorithm = 'md5') AS md5_count, \
         (SELECT COUNT(*) FROM occurrence_digest_assertions AS assertion \
          JOIN digest_values AS digest USING (digest_id) \
          WHERE assertion.occurrence_id = occurrence.occurrence_id \
            AND digest.algorithm = 'sha1') AS sha1_count \
         FROM snapshot_publications AS publication \
         JOIN catalog_set_groups AS groups USING (snapshot_key) \
         JOIN catalog_sets AS sets USING (set_group_id) \
         JOIN asset_occurrences AS occurrence ON occurrence.record_id = sets.set_id \
         JOIN logiqx_rom_claims AS claim USING (occurrence_id) \
         WHERE sets.set_name IN ('large', 'unknown') ORDER BY claim.name",
    )
    .load::<SnapshotFactsRow>(&mut connection)?;
    let large = native_facts
        .iter()
        .find(|facts| facts.name == "large.rom")
        .ok_or("native facts omitted the large ROM")?;
    assert_eq!(large.size, Some(4_294_967_297));
    assert_eq!(large.md5_count, 0);
    assert_eq!(large.sha1_count, 1);
    let unknown_facts = native_facts
        .iter()
        .find(|facts| facts.name == "unknown.rom")
        .ok_or("native facts omitted the hashless ROM")?;
    assert_eq!(unknown_facts.size, None);
    assert_eq!(unknown_facts.md5_count, 0);
    assert_eq!(unknown_facts.sha1_count, 0);

    let plan = app::plan_build(
        &fixture.database,
        &plan_request(fixture.dat_path.clone(), fixture.source_path.clone()),
    )?;
    let unknown = plan
        .report
        .missing_roms
        .iter()
        .find(|missing| missing.rom_name == "unknown.rom")
        .ok_or("native plan omitted the hashless ROM requirement")?;
    assert_eq!(unknown.sha1, None);

    let audit = app::audit(
        &fixture.database,
        &AuditRequest {
            dat_path: fixture.dat_path.clone(),
            source_path: fixture.source_path.clone(),
            refresh: AuditRefresh::Cached,
            matching_policy: MatchingPolicy::Sha1Compatibility,
            jobs: 1,
            set_selection: SetSelection::All,
        },
    )?;
    let audited_unknown = audit
        .report()
        .missing_roms
        .iter()
        .find(|missing| missing.rom_name == "unknown.rom")
        .ok_or("native audit omitted the hashless ROM requirement")?;
    assert_eq!(audited_unknown.sha1, None);
    Ok(())
}

#[test]
fn explicit_native_catalog_key_selects_a_normal_build_plan()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let fixture = Fixture::new("")?;
    let path = write_dat(
        &directory,
        "explicit.dat",
        &format!(
            "<datafile><header><name>Explicit key catalog</name></header><game name='explicit'>{COMPLETE_ROM}</game></datafile>"
        ),
    )?;
    let imported = import_catalog(
        &fixture.database,
        &path,
        "explicit-native-key",
        "Explicit key catalog",
    )?;
    assert_eq!(imported.status, app::CatalogImportStatus::Succeeded);

    let plan = app::plan_build(
        &fixture.database,
        &plan_request(
            Utf8PathBuf::from("explicit-native-key"),
            fixture.source_path.clone(),
        ),
    )?;
    assert_eq!(plan.report.missing_roms.len(), 1);
    assert_eq!(plan.report.missing_roms[0].game_name, "explicit");
    Ok(())
}

#[test]
fn identical_header_names_remain_path_scoped_and_header_alias_is_unique()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new(
        "<datafile><header><name>Shared Header</name></header><game name='first'><rom name='first.rom' size='1' sha1='1111111111111111111111111111111111111111'/></game></datafile>",
    )?;
    let second = Utf8PathBuf::from_path_buf(fixture.directory.path().join("second.dat"))
        .map_err(|path| format!("non-UTF-8 DAT path: {}", path.display()))?;
    let unique = Utf8PathBuf::from_path_buf(fixture.directory.path().join("unique.dat"))
        .map_err(|path| format!("non-UTF-8 DAT path: {}", path.display()))?;
    std::fs::write(
        &second,
        "<datafile><header><name>Shared Header</name></header><game name='second'><rom name='second.rom' size='1' sha1='2222222222222222222222222222222222222222'/></game></datafile>",
    )?;
    std::fs::write(
        &unique,
        "<datafile><header><name>Unique Header</name></header><game name='unique'><rom name='unique.rom' size='1' sha1='3333333333333333333333333333333333333333'/></game></datafile>",
    )?;
    let first_report = fixture.import_dat()?;
    let second_report = fixture.import_at(&second)?;
    fixture.import_at(&unique)?;
    assert_ne!(first_report.catalog_key, second_report.catalog_key);

    for (path, game_name, rom_name) in [
        (&fixture.dat_path, "first", "first.rom"),
        (&second, "second", "second.rom"),
    ] {
        let plan = app::plan_build(
            &fixture.database,
            &plan_request(path.clone(), fixture.source_path.clone()),
        )?;
        assert_eq!(plan.report.missing_roms.len(), 1);
        assert_eq!(plan.report.missing_roms[0].game_name, game_name);
        assert_eq!(plan.report.missing_roms[0].rom_name, rom_name);
    }

    let by_header = app::plan_build(
        &fixture.database,
        &plan_request(
            Utf8PathBuf::from("Shared Header"),
            fixture.source_path.clone(),
        ),
    );
    assert!(
        by_header.is_err(),
        "ambiguous header selected an arbitrary catalog"
    );

    let unique_alias = app::plan_build(
        &fixture.database,
        &plan_request(
            Utf8PathBuf::from("Unique Header"),
            fixture.source_path.clone(),
        ),
    )?;
    assert_eq!(unique_alias.report.missing_roms.len(), 1);
    assert_eq!(unique_alias.report.missing_roms[0].game_name, "unique");
    Ok(())
}

#[test]
fn reimporting_changed_bytes_at_the_same_path_keeps_both_published_snapshots()
-> Result<(), Box<dyn std::error::Error>> {
    let first = "<datafile><header><name>Old</name></header><game name='old'><rom name='old.rom' size='3' sha1='aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'/></game></datafile>";
    let second = "<datafile><header><name>New</name></header><game name='new'><rom name='new.rom' size='5' sha1='bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'/></game></datafile>";
    let fixture = Fixture::new(first)?;
    let previous_report = fixture.import_dat()?;
    let previous = previous_report.snapshot_key.to_string();
    let mut connection = fixture.connection()?;

    fixture.replace_dat(second)?;
    let latest_report = fixture.import_dat()?;
    let current = latest_report.snapshot_key.to_string();
    assert_ne!(previous, current);

    let second_path = Utf8PathBuf::from_path_buf(fixture.directory.path().join("other.dat"))
        .map_err(|path| format!("non-UTF-8 DAT path: {}", path.display()))?;
    std::fs::write(
        &second_path,
        "<datafile><header><name>Old</name></header><game name='other'><rom name='other.rom' size='7' sha1='cccccccccccccccccccccccccccccccccccccccc'/></game></datafile>",
    )?;
    fixture.import_at(&second_path)?;

    for (snapshot, expected_name) in [(&previous, "old.rom"), (&current, "new.rom")] {
        let facts = sql_query(
            "SELECT claim.name AS value FROM catalog_set_groups AS groups \
             JOIN catalog_sets AS sets USING (set_group_id) \
             JOIN asset_occurrences AS occurrence ON occurrence.record_id = sets.set_id \
             JOIN logiqx_rom_claims AS claim USING (occurrence_id) \
             WHERE groups.snapshot_key = ?",
        )
        .bind::<Text, _>(snapshot)
        .get_result::<TextRow>(&mut connection)?;
        assert_eq!(facts.value, expected_name);
    }

    for (selector, expected_rom) in [
        ("Old", "other.rom"),
        ("New", "new.rom"),
        (latest_report.catalog_key.as_str(), "new.rom"),
    ] {
        let plan = app::plan_build(
            &fixture.database,
            &plan_request(Utf8PathBuf::from(selector), fixture.source_path.clone()),
        )?;
        assert_eq!(plan.report.missing_roms.len(), 1);
        assert_eq!(plan.report.missing_roms[0].rom_name, expected_rom);
    }
    Ok(())
}

#[test]
fn one_scanned_rom_observation_is_counted_once_across_native_catalogs()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let fixture = Fixture::new("")?;
    for (catalog_key, game_name) in [("scan-first", "first"), ("scan-second", "second")] {
        let path = write_dat(
            &directory,
            &format!("{catalog_key}.dat"),
            &format!(
                "<datafile><header><name>{catalog_key}</name></header><game name='{game_name}'>{COMPLETE_ROM}</game></datafile>"
            ),
        )?;
        import_catalog(&fixture.database, &path, catalog_key, catalog_key)?;
    }
    std::fs::write(fixture.source_path.join("game.rom"), b"abc")?;

    let scan = app::scan_source(
        &fixture.database,
        &SourceScanRequest {
            source_path: fixture.source_path.clone(),
            jobs: 1,
        },
    )?;
    assert_eq!(scan.observation_count, 1);
    assert_eq!(scan.catalog_candidate_count, 1);

    let mut connection = fixture.connection()?;
    let observed_rows = sql_query("SELECT COUNT(*) AS count FROM rom_files")
        .get_result::<CountRow>(&mut connection)?;
    assert_eq!(observed_rows.count, 1);
    Ok(())
}

#[test]
fn failed_partial_rescan_preserves_the_last_complete_native_build_inventory()
-> Result<(), Box<dyn std::error::Error>> {
    let document = format!(
        "<datafile><header><name>Scan inventory</name></header><game name='game'>{COMPLETE_ROM}</game></datafile>"
    );
    let fixture = Fixture::new(&document)?;
    fixture.import_dat()?;
    std::fs::write(fixture.source_path.join("game.rom"), b"abc")?;
    app::scan_source(
        &fixture.database,
        &SourceScanRequest {
            source_path: fixture.source_path.clone(),
            jobs: 1,
        },
    )?;

    std::fs::remove_file(fixture.source_path.join("game.rom"))?;
    std::fs::write(
        fixture.source_path.join("broken.zip"),
        b"PK\x03\x04not a zip",
    )?;
    assert!(
        app::scan_source(
            &fixture.database,
            &SourceScanRequest {
                source_path: fixture.source_path.clone(),
                jobs: 1,
            },
        )
        .is_err()
    );

    let cached = app::plan_build(
        &fixture.database,
        &plan_request(fixture.dat_path.clone(), fixture.source_path.clone()),
    )?;
    assert_eq!(cached.report.matched_roms, 1);
    assert!(cached.report.missing_roms.is_empty());
    Ok(())
}

#[test]
fn failed_dat_import_retains_diagnostics_without_replacing_the_active_snapshot()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new(
        "<datafile><header><name>Import failure</name></header><game name='active'><rom name='active.rom' size='3' sha1='aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'/></game></datafile>",
    )?;
    let active = fixture.import_dat()?;
    fixture.replace_dat("<datafile><game name='broken'>")?;
    let failed_run = match fixture.import_dat() {
        Err(mame_coalesce::Error::CatalogImportFailed(run_key)) => run_key.to_string(),
        Err(error) => return Err(error.into()),
        Ok(_) => return Err("malformed DAT import unexpectedly succeeded".into()),
    };

    let mut connection = fixture.connection()?;
    let failed_run_status = sql_query(
        "SELECT COUNT(*) AS count FROM import_runs \
         WHERE run_key = ? AND status = 'failed' AND snapshot_key IS NULL",
    )
    .bind::<Text, _>(&failed_run)
    .get_result::<CountRow>(&mut connection)?;
    assert_eq!(failed_run_status.count, 1);
    let diagnostics =
        sql_query("SELECT COUNT(*) AS count FROM import_diagnostics WHERE run_key = ?")
            .bind::<Text, _>(&failed_run)
            .get_result::<CountRow>(&mut connection)?;
    assert!(diagnostics.count > 0);

    let active_publication =
        sql_query("SELECT snapshot_key AS value FROM snapshot_publications WHERE catalog_key = ?")
            .bind::<Text, _>(active.catalog_key.to_string())
            .get_result::<TextRow>(&mut connection)?;
    assert_eq!(active_publication.value, active.snapshot_key.to_string());
    let published_count = sql_query("SELECT COUNT(*) AS count FROM snapshot_publications")
        .get_result::<CountRow>(&mut connection)?;
    assert_eq!(published_count.count, 1);
    Ok(())
}

#[test]
fn flat_build_rejects_repeated_set_names_with_distinct_native_owners()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new(
        "<datafile><header><name>Repeated owners</name></header><game name='same'><rom name='first.rom' size='1' sha1='1111111111111111111111111111111111111111'/></game><game name='same'><rom name='second.rom' size='2' sha1='2222222222222222222222222222222222222222'/></game></datafile>",
    )?;
    fixture.import_dat()?;
    let mut connection = fixture.connection()?;
    let owners = sql_query("SELECT COUNT(*) AS count FROM catalog_sets WHERE set_name = 'same'")
        .get_result::<CountRow>(&mut connection)?;
    assert_eq!(owners.count, 2);

    let selected = app::plan_build(
        &fixture.database,
        &plan_request(fixture.dat_path.clone(), fixture.source_path.clone()),
    );
    assert!(
        selected.is_err(),
        "flat build silently merged repeated native set names"
    );
    Ok(())
}

#[test]
fn flat_build_rejects_software_list_catalogs_instead_of_flattening_numeric_owners()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let fixture = Fixture::new("")?;
    let path = write_dat(
        &directory,
        "software-list.xml",
        r#"<softwarelist name="games"><software name="item"><description>Item</description><year>2000</year><publisher>Example</publisher><part name="cart" interface="cart"><dataarea name="rom" size="1"><rom name="program.bin" size="1" sha1="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"/></dataarea></part></software></softwarelist>"#,
    )?;
    let imported = app::import_catalog(
        &fixture.database,
        &CatalogImportRequest {
            document_path: path,
            format: CatalogDocumentFormat::MameSoftwareListXml,
            source_key: PublishingSourceKey::new("software-list-source"),
            source_display_name: "Software list source".to_owned(),
            catalog_key: CatalogKey::new("software-list-catalog"),
            catalog_display_name: "Games software list".to_owned(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(imported.status, app::CatalogImportStatus::Succeeded);
    let mut connection = fixture.connection()?;
    let software_items = sql_query("SELECT COUNT(*) AS count FROM software_items")
        .get_result::<CountRow>(&mut connection)?;
    assert_eq!(software_items.count, 1);

    let selected = app::plan_build(
        &fixture.database,
        &plan_request(
            Utf8PathBuf::from("software-list-catalog"),
            fixture.source_path.clone(),
        ),
    );
    assert!(
        selected.is_err(),
        "flat build silently flattened software-list records"
    );
    Ok(())
}
