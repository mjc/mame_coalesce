use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Text},
};
use mame_coalesce::{
    RestorePolicy,
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    catalog_logiqx::{
        LogiqxForceMerging, LogiqxForceNoDump, LogiqxForcePacking, LogiqxMediaKind,
        LogiqxOptionValue, LogiqxPageLimit, LogiqxQueryError, LogiqxRomMode, LogiqxSampleMode,
        LogiqxYesNo, logiqx_for_snapshot,
    },
    database::Database,
    domain::{
        CatalogKey, CatalogScope, DocumentKey, ParserInterpretationKey, PublishingSourceKey,
        SnapshotKey,
    },
    logiqx::{
        BiosSetAttribute, ClrMameProAttribute, DocumentAttribute, GameAttribute, ReleaseAttribute,
        RomCenterAttribute,
    },
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const FULL_DOCUMENT: &str = r#"<datafile xmlns:v="urn:test" build="" debug="no">
  <header>
    <name>Fixture</name><description></description><category>Arcade</category>
    <version>1.5</version><date>2008-10-28</date><author>Author</author>
    <email>a@example.test</email><homepage>https://example.test</homepage>
    <url>https://example.test/dat</url><comment>header comment</comment>
    <clrmamepro header="" forcemerging="full" forcenodump="ignore" forcepacking="unzip"/>
    <romcenter plugin="" rommode="unmerged" biosmode="merged" samplemode="unmerged"
      lockrommode="yes" lockbiosmode="yes" locksamplemode="yes"/>
  </header>
  <file_name>declared.dat</file_name><sha1>0123456789012345678901234567890123456789</sha1>
  <game xmlns:v="urn:test" rebuildto="rebuilt" board="board" sampleof="sample-parent"
        romof="rom-parent" cloneof="" isbios="yes" sourcefile="source.zip" name="repeat">
    <comment></comment><comment>same</comment>
    <description></description><year>1990</year><manufacturer>Maker</manufacturer>
    <release name="World" region="US" language="en" date="1990-01" default="yes"/>
    <release name="World" region="EU"/>
    <biosset name="base" description="Base" default="yes"/>
    <biosset name="base" description="Alternate"/>
    <rom name="rom.bin" size="0004" crc="12345678" md5="11111111111111111111111111111111"
         sha1="2222222222222222222222222222222222222222" merge="parent.bin" status="verified"
         date="1990" serial="compat-serial"/>
    <disk name="disk.chd" sha1="3333333333333333333333333333333333333333"
          md5="44444444444444444444444444444444" merge="parent.chd" status="baddump"/>
    <sample name="intro"/><archive name="container.zip"/>
    <device_ref name="sound"/><device_ref name="sound"/>
  </game>
  <game name="repeat"><description>Second</description></game>
</datafile>"#;

struct Catalog {
    directory: tempfile::TempDir,
    path: Utf8PathBuf,
    database: Database,
}

impl Catalog {
    fn new() -> TestResult<Self> {
        let directory = tempfile::tempdir()?;
        let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
        let database = Database::open(&path)?;
        Ok(Self {
            directory,
            path,
            database,
        })
    }

    fn import(
        &self,
        key: &str,
        format: CatalogDocumentFormat,
        xml: &str,
    ) -> TestResult<SnapshotKey> {
        let document_path =
            Utf8PathBuf::try_from(self.directory.path().join(format!("{key}.xml")))?;
        std::fs::write(&document_path, xml)?;
        let report = app::import_catalog(
            &self.database,
            &CatalogImportRequest {
                document_path,
                format,
                source_key: PublishingSourceKey::new(format!("logiqx-query-{key}")),
                source_display_name: format!("Logiqx query {key}"),
                catalog_key: CatalogKey::new(format!("logiqx-query-{key}")),
                catalog_display_name: format!("Logiqx query {key}"),
                scope: CatalogScope::Complete,
            },
        )?;
        assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
        Ok(report
            .snapshot_key
            .ok_or("successful import omitted its snapshot")?)
    }

    fn connection(&self) -> TestResult<SqliteConnection> {
        Ok(SqliteConnection::establish(self.path.as_str())?)
    }
}

#[test]
fn public_pages_return_complete_native_document_and_game_fields() -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import(
        "full",
        CatalogDocumentFormat::Logiqx(mame_coalesce::logiqx::LogiqxMode::ObservedCompatible),
        FULL_DOCUMENT,
    )?;

    let first = logiqx_for_snapshot(&catalog.database, &snapshot, None, LogiqxPageLimit::new(1)?)?;
    assert_eq!(first.snapshot.snapshot_key, snapshot);
    assert_eq!(first.snapshot.format, "logiqx");
    assert_eq!(first.snapshot.declared_version.as_deref(), Some("1.5"));
    assert_full_document(&first)?;
    let game = first.games.first().ok_or("first game missing")?;
    assert_game_attributes(game);
    assert_game_children(game);
    assert_next_game_page(&catalog, &snapshot, &first)
}

fn assert_full_document(first: &mame_coalesce::catalog_logiqx::LogiqxPage) -> TestResult {
    assert_eq!(first.document.build.as_deref(), Some(""));
    assert_eq!(first.document.debug, LogiqxYesNo::No);
    assert!(first.document.debug_was_present);
    assert_eq!(first.document.file_name.as_deref(), Some("declared.dat"));
    assert_eq!(
        first.document.sha1,
        Some(hex::decode("0123456789012345678901234567890123456789")?)
    );

    let header = first.document.header.as_ref().ok_or("header missing")?;
    assert_eq!(header.name.as_deref(), Some("Fixture"));
    assert_eq!(header.description.as_deref(), Some(""));
    assert_eq!(header.category.as_deref(), Some("Arcade"));
    assert_eq!(header.version.as_deref(), Some("1.5"));
    assert_eq!(header.date.as_deref(), Some("2008-10-28"));
    assert_eq!(header.author.as_deref(), Some("Author"));
    assert_eq!(header.email.as_deref(), Some("a@example.test"));
    assert_eq!(header.homepage.as_deref(), Some("https://example.test"));
    assert_eq!(header.url.as_deref(), Some("https://example.test/dat"));
    assert_eq!(header.comment.as_deref(), Some("header comment"));
    assert_eq!(header.text_positions.len(), 10);

    assert_eq!(
        first
            .document
            .attribute_positions
            .iter()
            .map(|p| p.field)
            .collect::<Vec<_>>(),
        [DocumentAttribute::Build, DocumentAttribute::Debug]
    );
    let clrmamepro = first
        .document
        .clrmamepro
        .as_ref()
        .ok_or("ClrMamePro missing")?;
    assert_eq!(clrmamepro.header.as_deref(), Some(""));
    assert_eq!(
        clrmamepro.forcemerging,
        LogiqxOptionValue::Explicit(LogiqxForceMerging::Full)
    );
    assert!(clrmamepro.forcemerging.was_present());
    assert_eq!(
        clrmamepro.forcenodump,
        LogiqxOptionValue::Explicit(LogiqxForceNoDump::Ignore)
    );
    assert_eq!(
        clrmamepro.forcepacking,
        LogiqxOptionValue::Explicit(LogiqxForcePacking::Unzip)
    );
    assert_eq!(clrmamepro.attribute_positions.len(), 4);
    assert_eq!(
        clrmamepro.attribute_positions[0].field,
        ClrMameProAttribute::Header
    );

    let romcenter = first
        .document
        .romcenter
        .as_ref()
        .ok_or("RomCenter missing")?;
    assert_eq!(romcenter.plugin.as_deref(), Some(""));
    assert_eq!(
        romcenter.rommode,
        LogiqxOptionValue::Explicit(LogiqxRomMode::Unmerged)
    );
    assert_eq!(
        romcenter.biosmode,
        LogiqxOptionValue::Explicit(LogiqxRomMode::Merged)
    );
    assert_eq!(
        romcenter.samplemode,
        LogiqxOptionValue::Explicit(LogiqxSampleMode::Unmerged)
    );
    assert_eq!(
        romcenter.lockrommode,
        LogiqxOptionValue::Explicit(LogiqxYesNo::Yes)
    );
    assert_eq!(romcenter.attribute_positions.len(), 7);
    assert_eq!(
        romcenter.attribute_positions[0].field,
        RomCenterAttribute::Plugin
    );

    Ok(())
}

fn assert_game_attributes(game: &mame_coalesce::catalog_logiqx::LogiqxGame) {
    assert_eq!(game.name, "repeat");
    assert_eq!(game.sourcefile.as_deref(), Some("source.zip"));
    assert_eq!(game.is_bios, LogiqxYesNo::Yes);
    assert!(game.is_bios_was_present);
    assert_eq!(
        game.cloneof
            .as_ref()
            .map(|reference| reference.target_name.as_str()),
        Some("")
    );
    assert_eq!(
        game.romof
            .as_ref()
            .map(|reference| reference.target_name.as_str()),
        Some("rom-parent")
    );
    assert_eq!(
        game.sampleof
            .as_ref()
            .map(|reference| reference.target_name.as_str()),
        Some("sample-parent")
    );
    assert_eq!(game.board.as_deref(), Some("board"));
    assert_eq!(game.rebuildto.as_deref(), Some("rebuilt"));
    assert_eq!(
        game.description.as_ref().map(|field| field.value.as_str()),
        Some("")
    );
    assert_eq!(
        game.year.as_ref().map(|field| field.value.as_str()),
        Some("1990")
    );
    assert_eq!(
        game.manufacturer.as_ref().map(|field| field.value.as_str()),
        Some("Maker")
    );
    assert_eq!(game.attribute_positions.len(), 8);
    assert_eq!(game.attribute_positions[0].field, GameAttribute::RebuildTo);
    assert_ne!(
        (
            game.attribute_positions[0].location.line,
            game.attribute_positions[0].location.column
        ),
        (game.location.line, game.location.column),
    );
}

fn assert_game_children(game: &mame_coalesce::catalog_logiqx::LogiqxGame) {
    assert_eq!(
        game.comments
            .iter()
            .map(|comment| comment.text.as_str())
            .collect::<Vec<_>>(),
        ["", "same"]
    );
    assert_eq!(game.releases.len(), 2);
    assert_eq!(game.releases[0].name, "World");
    assert_eq!(game.releases[0].is_default, LogiqxYesNo::Yes);
    assert!(game.releases[0].default_was_present);
    assert_eq!(game.releases[1].is_default, LogiqxYesNo::No);
    assert!(!game.releases[1].default_was_present);
    assert_eq!(game.releases[0].attribute_positions.len(), 5);
    assert_eq!(
        game.releases[0].attribute_positions[0].field,
        ReleaseAttribute::Name
    );
    assert_eq!(game.bios_sets.len(), 2);
    assert_eq!(game.bios_sets[0].is_default, LogiqxYesNo::Yes);
    assert!(!game.bios_sets[1].default_was_present);
    assert_eq!(game.bios_sets[0].attribute_positions.len(), 3);
    assert_eq!(
        game.bios_sets[0].attribute_positions[0].field,
        BiosSetAttribute::Name
    );
    assert_eq!(game.archives.len(), 1);
    assert_eq!(game.archives[0].name, "container.zip");
    assert_eq!(
        game.device_references
            .iter()
            .map(|device| device.name.as_str())
            .collect::<Vec<_>>(),
        ["sound", "sound"]
    );

    assert_eq!(game.media.len(), 3);
    assert_eq!(
        game.media
            .iter()
            .map(|media| media.kind)
            .collect::<Vec<_>>(),
        [
            LogiqxMediaKind::Rom,
            LogiqxMediaKind::Disk,
            LogiqxMediaKind::Sample,
        ]
    );
    assert_ne!(game.media[0].occurrence_id, game.media[1].occurrence_id);
    assert_eq!(game.media[0].source_order, 9);
    assert_eq!(game.media[1].source_order, 10);
    assert_eq!(game.media[2].source_order, 11);
}

fn assert_next_game_page(
    catalog: &Catalog,
    snapshot: &SnapshotKey,
    first: &mame_coalesce::catalog_logiqx::LogiqxPage,
) -> TestResult {
    let cursor = first
        .next_cursor
        .as_ref()
        .ok_or("second page cursor missing")?;
    let second = logiqx_for_snapshot(
        &catalog.database,
        snapshot,
        Some(cursor),
        LogiqxPageLimit::new(1)?,
    )?;
    assert_eq!(second.games.len(), 1);
    assert_eq!(second.games[0].name, "repeat");
    assert_ne!(first.games[0].id, second.games[0].id);
    assert_eq!(second.games[0].is_bios, LogiqxYesNo::No);
    assert!(!second.games[0].is_bios_was_present);
    assert!(second.next_cursor.is_none());
    Ok(())
}

#[test]
fn snapshot_cursor_limits_and_format_are_validated() -> TestResult {
    assert!(LogiqxPageLimit::new(0).is_err());
    assert!(LogiqxPageLimit::new(501).is_err());
    assert!(LogiqxPageLimit::new(500).is_ok());

    let catalog = Catalog::new()?;
    let snapshot = catalog.import(
        "source",
        CatalogDocumentFormat::Logiqx(mame_coalesce::logiqx::LogiqxMode::ObservedCompatible),
        FULL_DOCUMENT,
    )?;
    let other_snapshot = catalog.import(
        "other",
        CatalogDocumentFormat::Logiqx(mame_coalesce::logiqx::LogiqxMode::ObservedCompatible),
        "<datafile><game name='other'><description>Other</description></game></datafile>",
    )?;
    let page = logiqx_for_snapshot(&catalog.database, &snapshot, None, LogiqxPageLimit::new(1)?)?;
    let cursor = page.next_cursor.as_ref().ok_or("cursor missing")?;
    assert!(matches!(
        logiqx_for_snapshot(
            &catalog.database,
            &other_snapshot,
            Some(cursor),
            LogiqxPageLimit::new(1)?
        ),
        Err(LogiqxQueryError::CursorSnapshotMismatch)
    ));

    let mame = catalog.import(
        "mame",
        CatalogDocumentFormat::MameListXml,
        "<mame mameconfig='10'><machine name='machine'><description>Machine</description></machine></mame>",
    )?;
    assert!(matches!(
        logiqx_for_snapshot(&catalog.database, &mame, None, LogiqxPageLimit::new(1)?),
        Err(LogiqxQueryError::NotPublishedLogiqx(_))
    ));
    let missing = SnapshotKey::new(
        &CatalogKey::new("missing-catalog"),
        &DocumentKey::from_bytes(b"missing-document"),
        &ParserInterpretationKey::logiqx_v1(&CatalogScope::Complete),
    );
    assert!(matches!(
        logiqx_for_snapshot(&catalog.database, &missing, None, LogiqxPageLimit::new(1)?),
        Err(LogiqxQueryError::NotPublishedLogiqx(_))
    ));
    Ok(())
}

#[test]
fn absent_header_empty_header_and_zero_game_documents_remain_distinct() -> TestResult {
    let catalog = Catalog::new()?;
    let absent = catalog.import(
        "absent-header",
        CatalogDocumentFormat::Logiqx(mame_coalesce::logiqx::LogiqxMode::ObservedCompatible),
        "<datafile/>",
    )?;
    let empty = catalog.import(
        "empty-header",
        CatalogDocumentFormat::Logiqx(mame_coalesce::logiqx::LogiqxMode::ObservedCompatible),
        "<datafile><header><name></name><clrmamepro/><romcenter/></header></datafile>",
    )?;

    let absent_page =
        logiqx_for_snapshot(&catalog.database, &absent, None, LogiqxPageLimit::new(1)?)?;
    assert!(absent_page.document.header.is_none());
    assert!(absent_page.document.file_name.is_none());
    assert!(absent_page.document.sha1.is_none());
    assert!(absent_page.games.is_empty());
    assert!(absent_page.next_cursor.is_none());

    let empty_page =
        logiqx_for_snapshot(&catalog.database, &empty, None, LogiqxPageLimit::new(1)?)?;
    assert_eq!(
        empty_page
            .document
            .header
            .as_ref()
            .and_then(|header| header.name.as_deref()),
        Some("")
    );
    assert!(empty_page.games.is_empty());
    assert!(empty_page.next_cursor.is_none());
    let cmp = empty_page
        .document
        .clrmamepro
        .as_ref()
        .ok_or("empty options missing")?;
    assert_eq!(
        cmp.forcemerging,
        LogiqxOptionValue::Defaulted(LogiqxForceMerging::Split)
    );
    assert_eq!(
        cmp.forcenodump,
        LogiqxOptionValue::Defaulted(LogiqxForceNoDump::Obsolete)
    );
    assert_eq!(
        cmp.forcepacking,
        LogiqxOptionValue::Defaulted(LogiqxForcePacking::Zip)
    );
    assert!(!cmp.forcemerging.was_present());
    assert_eq!(cmp.forcemerging.effective(), &LogiqxForceMerging::Split);
    assert!(cmp.attribute_positions.is_empty());
    let center = empty_page
        .document
        .romcenter
        .as_ref()
        .ok_or("empty options missing")?;
    assert_eq!(
        center.rommode,
        LogiqxOptionValue::Defaulted(LogiqxRomMode::Split)
    );
    assert_eq!(
        center.biosmode,
        LogiqxOptionValue::Defaulted(LogiqxRomMode::Split)
    );
    assert_eq!(
        center.samplemode,
        LogiqxOptionValue::Defaulted(LogiqxSampleMode::Merged)
    );
    for lock in [
        center.lockrommode,
        center.lockbiosmode,
        center.locksamplemode,
    ] {
        assert_eq!(lock, LogiqxOptionValue::Defaulted(LogiqxYesNo::No));
    }
    assert!(center.attribute_positions.is_empty());
    Ok(())
}

#[test]
fn source_free_queries_match_after_paired_backup_restore() -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import(
        "backup",
        CatalogDocumentFormat::Logiqx(mame_coalesce::logiqx::LogiqxMode::ObservedCompatible),
        FULL_DOCUMENT,
    )?;
    let before = logiqx_for_snapshot(
        &catalog.database,
        &snapshot,
        None,
        LogiqxPageLimit::new(10)?,
    )?;
    let original = catalog.directory.path().join("backup.xml");
    std::fs::remove_file(original)?;

    let backup = Utf8PathBuf::try_from(catalog.directory.path().join("catalog.backup"))?;
    let restored = Utf8PathBuf::try_from(catalog.directory.path().join("restored.sqlite"))?;
    mame_coalesce::create_backup(&catalog.path, &backup)?;
    mame_coalesce::restore_backup(&backup, &restored, RestorePolicy::CreateNew)?;
    let restored_database = Database::open(&restored)?;
    let originals = Utf8PathBuf::from(format!("{restored}.documents"));
    let unavailable = catalog
        .directory
        .path()
        .join("restored-originals-unavailable");
    std::fs::rename(originals, unavailable)?;
    assert!(
        app::load_snapshot_source(&restored_database, &snapshot).is_err(),
        "the restored retained original must actually be unavailable"
    );
    let after = logiqx_for_snapshot(
        &restored_database,
        &snapshot,
        None,
        LogiqxPageLimit::new(10)?,
    )?;
    assert_eq!(after, before);
    Ok(())
}

#[derive(QueryableByName)]
struct OwnerId {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
}

#[derive(QueryableByName)]
struct SavedTrigger {
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    sql: String,
}

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

fn mutate_with_restored_guard(
    connection: &mut SqliteConnection,
    table: &str,
    column: &str,
    delta: f64,
) -> TestResult {
    let guards = sql_query(
        "SELECT name, sql FROM sqlite_schema WHERE type = 'trigger' AND tbl_name = ? \
         AND sql LIKE '%BEFORE UPDATE%'",
    )
    .bind::<Text, _>(table)
    .load::<SavedTrigger>(connection)?;
    assert!(
        !guards.is_empty(),
        "{table} must have an immutability guard"
    );
    for guard in &guards {
        connection.batch_execute(&format!("DROP TRIGGER {}", guard.name))?;
    }
    let update = format!("UPDATE {table} SET {column} = {column} + {delta}");
    let mutation = connection.batch_execute(&update);
    for guard in &guards {
        connection.batch_execute(&guard.sql)?;
    }
    mutation?;
    for guard in guards {
        let restored = sql_query("SELECT name, sql FROM sqlite_schema WHERE name = ?")
            .bind::<Text, _>(&guard.name)
            .get_result::<SavedTrigger>(connection)?;
        assert_eq!(restored.sql, guard.sql);
    }
    assert!(
        connection.batch_execute(&update).is_err(),
        "restored guard must reject updates"
    );
    Ok(())
}

#[test]
fn fractional_native_provenance_returns_typed_errors_without_truncation() -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import(
        "fractional",
        CatalogDocumentFormat::Logiqx(mame_coalesce::logiqx::LogiqxMode::ObservedCompatible),
        FULL_DOCUMENT,
    )?;
    let mut connection = catalog.connection()?;
    let owner = sql_query(
        "SELECT sets.set_id FROM catalog_sets AS sets \
         JOIN catalog_set_groups AS groups USING(set_group_id) \
         WHERE groups.snapshot_key = ? AND sets.list_order = 0",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<OwnerId>(&mut connection)?;
    // Fractional family keys are a malformed-ancestry witness. Coordinates and
    // source ordinals need no CHECK bypass; the native range constraints permit REALs.
    connection.batch_execute("PRAGMA foreign_keys = OFF;")?;
    assert_fractional_families(&catalog, &snapshot, &mut connection, owner.set_id)?;
    logiqx_for_snapshot(&catalog.database, &snapshot, None, LogiqxPageLimit::new(1)?)?;
    Ok(())
}

fn assert_fractional_families(
    catalog: &Catalog,
    snapshot: &SnapshotKey,
    connection: &mut SqliteConnection,
    owner: i64,
) -> TestResult {
    for (table, columns, expected_owner) in fractional_native_families(owner) {
        assert_fractional_columns(
            catalog,
            snapshot,
            connection,
            table,
            columns,
            expected_owner,
        )?;
    }
    Ok(())
}

fn assert_fractional_columns(
    catalog: &Catalog,
    snapshot: &SnapshotKey,
    connection: &mut SqliteConnection,
    table: &str,
    columns: &[&str],
    expected_owner: i64,
) -> TestResult {
    let checks_bypassed =
        sql_query("SELECT ignore_check_constraints AS value FROM pragma_ignore_check_constraints")
            .get_result::<Count>(connection)?
            .value;
    for column in columns {
        mutate_with_restored_guard(connection, table, column, 0.5)?;
        let stored = sql_query(format!(
            "SELECT count(*) AS value FROM {table} WHERE typeof({column}) = 'real'"
        ))
        .get_result::<Count>(connection)?;
        assert!(
            stored.value > 0,
            "{table}.{column} must actually store REAL values"
        );
        connection
            .batch_execute("PRAGMA foreign_keys = ON; PRAGMA ignore_check_constraints = OFF;")?;
        let result =
            logiqx_for_snapshot(&catalog.database, snapshot, None, LogiqxPageLimit::new(1)?);
        connection.batch_execute(&format!(
            "PRAGMA foreign_keys = OFF; PRAGMA ignore_check_constraints = {checks_bypassed};"
        ))?;
        mutate_with_restored_guard(connection, table, column, -0.5)?;
        assert!(
            matches!(result, Err(LogiqxQueryError::InvalidPositions(id)) if id == expected_owner),
            "{table}.{column} must not be silently truncated"
        );
    }
    Ok(())
}

#[test]
fn fractional_presence_bits_and_composite_position_keys_reject_check_bypass() -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import(
        "check-bypass",
        CatalogDocumentFormat::Logiqx(mame_coalesce::logiqx::LogiqxMode::ObservedCompatible),
        FULL_DOCUMENT,
    )?;
    let page = logiqx_for_snapshot(&catalog.database, &snapshot, None, LogiqxPageLimit::new(1)?)?;
    let owner = page
        .games
        .first()
        .ok_or("fixture game missing")?
        .id
        .as_i64();
    let mut connection = catalog.connection()?;
    // These columns have integer/bool constraints. This separately witnesses a
    // malformed database, not values that a valid import or normal SQL can store.
    connection.batch_execute("PRAGMA foreign_keys = OFF; PRAGMA ignore_check_constraints = ON;")?;
    let families: &[(&str, &[&str], i64)] = &[
        ("logiqx_document_facts", &["debug_was_present"], 0),
        ("logiqx_games", &["is_bios_was_present"], owner),
        (
            "logiqx_clrmamepro_options",
            &[
                "header_was_present",
                "forcemerging_was_present",
                "forcenodump_was_present",
                "forcepacking_was_present",
            ],
            0,
        ),
        (
            "logiqx_romcenter_options",
            &[
                "plugin_was_present",
                "rommode_was_present",
                "biosmode_was_present",
                "samplemode_was_present",
                "lockrommode_was_present",
                "lockbiosmode_was_present",
                "locksamplemode_was_present",
            ],
            0,
        ),
        ("logiqx_releases", &["default_was_present"], owner),
        ("logiqx_bios_sets", &["default_was_present"], owner),
        (
            "logiqx_release_attribute_positions",
            &["release_order"],
            owner,
        ),
        ("logiqx_bios_attribute_positions", &["bios_order"], owner),
        (
            "logiqx_archive_attribute_positions",
            &["archive_order"],
            owner,
        ),
        (
            "logiqx_device_reference_attribute_positions",
            &["reference_order"],
            owner,
        ),
    ];
    for &(table, columns, expected_owner) in families {
        assert_fractional_columns(
            &catalog,
            &snapshot,
            &mut connection,
            table,
            columns,
            expected_owner,
        )?;
    }
    connection.batch_execute("PRAGMA ignore_check_constraints = OFF; PRAGMA foreign_keys = ON;")?;
    logiqx_for_snapshot(&catalog.database, &snapshot, None, LogiqxPageLimit::new(1)?)?;
    Ok(())
}

fn fractional_native_families(owner: i64) -> Vec<(&'static str, &'static [&'static str], i64)> {
    let families: &[(&str, &[&str], i64)] = &[
        (
            "logiqx_game_comments",
            &[
                "comment_order",
                "source_order",
                "source_line",
                "source_column",
            ],
            owner,
        ),
        (
            "logiqx_releases",
            &[
                "release_order",
                "source_order",
                "source_line",
                "source_column",
            ],
            owner,
        ),
        (
            "logiqx_bios_sets",
            &["bios_order", "source_order", "source_line", "source_column"],
            owner,
        ),
        (
            "logiqx_archive_references",
            &[
                "archive_order",
                "source_order",
                "source_line",
                "source_column",
            ],
            owner,
        ),
        ("logiqx_device_references", &["reference_order"], owner),
        (
            "logiqx_clrmamepro_options",
            &["source_order", "source_line", "source_column"],
            0,
        ),
        (
            "logiqx_romcenter_options",
            &["source_order", "source_line", "source_column"],
            0,
        ),
    ];
    families.to_vec()
}

#[test]
fn missing_native_attribute_position_returns_typed_error_after_exact_guard_restore() -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import(
        "corrupt",
        CatalogDocumentFormat::Logiqx(mame_coalesce::logiqx::LogiqxMode::ObservedCompatible),
        FULL_DOCUMENT,
    )?;
    let mut connection = catalog.connection()?;
    let owner = sql_query(
        "SELECT sets.set_id FROM catalog_sets AS sets \
         JOIN catalog_set_groups AS groups USING(set_group_id) \
         WHERE groups.snapshot_key = ? AND sets.list_order = 0",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<OwnerId>(&mut connection)?;

    connection.batch_execute("DROP TRIGGER logiqx_game_positions_immutable_delete;")?;
    let delete_result = sql_query(
        "DELETE FROM logiqx_game_attribute_positions WHERE set_id = ? AND field_kind = 0",
    )
    .bind::<BigInt, _>(owner.set_id)
    .execute(&mut connection);
    connection.batch_execute(
        "CREATE TRIGGER logiqx_game_positions_immutable_delete \
         BEFORE DELETE ON logiqx_game_attribute_positions \
         BEGIN SELECT RAISE(ABORT,'Logiqx attribute positions are immutable'); END;",
    )?;
    assert_eq!(delete_result?, 1);

    assert!(matches!(
        logiqx_for_snapshot(&catalog.database, &snapshot, None, LogiqxPageLimit::new(1)?),
        Err(LogiqxQueryError::InvalidPositions(id)) if id == owner.set_id
    ));
    Ok(())
}
