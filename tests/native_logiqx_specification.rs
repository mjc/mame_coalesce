use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Bool, Nullable, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey, SnapshotKey},
};

#[derive(QueryableByName)]
struct DocumentRow {
    #[diesel(sql_type = Text)]
    debug: String,
    #[diesel(sql_type = Bool)]
    debug_was_present: bool,
    #[diesel(sql_type = Nullable<Text>)]
    header_description: Option<String>,
}

#[derive(QueryableByName)]
struct SetIdRow {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
}

#[derive(QueryableByName)]
struct GameRow {
    #[diesel(sql_type = Text)]
    is_bios: String,
    #[diesel(sql_type = Bool)]
    is_bios_was_present: bool,
}

#[derive(QueryableByName)]
// SQL decoding mirrors independent source-presence columns, not behavioral flags.
#[allow(clippy::struct_excessive_bools)]
struct ClrOptionsRow {
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
    #[diesel(sql_type = Nullable<Text>)]
    header: Option<String>,
    #[diesel(sql_type = Bool)]
    header_was_present: bool,
    #[diesel(sql_type = Text)]
    forcemerging: String,
    #[diesel(sql_type = Bool)]
    forcemerging_was_present: bool,
    #[diesel(sql_type = Text)]
    forcenodump: String,
    #[diesel(sql_type = Bool)]
    forcenodump_was_present: bool,
    #[diesel(sql_type = Text)]
    forcepacking: String,
    #[diesel(sql_type = Bool)]
    forcepacking_was_present: bool,
}

#[derive(QueryableByName)]
// SQL decoding mirrors independent source-presence columns, not behavioral flags.
#[allow(clippy::struct_excessive_bools)]
struct RomCenterOptionsRow {
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
    #[diesel(sql_type = Nullable<Text>)]
    plugin: Option<String>,
    #[diesel(sql_type = Bool)]
    plugin_was_present: bool,
    #[diesel(sql_type = Text)]
    rommode: String,
    #[diesel(sql_type = Bool)]
    rommode_was_present: bool,
    #[diesel(sql_type = Text)]
    biosmode: String,
    #[diesel(sql_type = Bool)]
    biosmode_was_present: bool,
    #[diesel(sql_type = Text)]
    samplemode: String,
    #[diesel(sql_type = Bool)]
    samplemode_was_present: bool,
    #[diesel(sql_type = Text)]
    lockrommode: String,
    #[diesel(sql_type = Bool)]
    lockrommode_was_present: bool,
    #[diesel(sql_type = Text)]
    lockbiosmode: String,
    #[diesel(sql_type = Bool)]
    lockbiosmode_was_present: bool,
    #[diesel(sql_type = Text)]
    locksamplemode: String,
    #[diesel(sql_type = Bool)]
    locksamplemode_was_present: bool,
}

#[derive(QueryableByName)]
struct CommentRow {
    #[diesel(sql_type = BigInt)]
    comment_order: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = Text)]
    comment_text: String,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
}

#[derive(QueryableByName)]
struct ReleaseRow {
    #[diesel(sql_type = BigInt)]
    release_order: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    region: String,
    #[diesel(sql_type = Nullable<Text>)]
    language: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    date: Option<String>,
    #[diesel(sql_type = Text)]
    default: String,
    #[diesel(sql_type = Bool)]
    default_was_present: bool,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
}

#[derive(QueryableByName)]
struct BiosRow {
    #[diesel(sql_type = BigInt)]
    bios_order: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    description: String,
    #[diesel(sql_type = Text)]
    is_default: String,
    #[diesel(sql_type = Bool)]
    default_was_present: bool,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
}

#[derive(QueryableByName)]
struct ArchiveRow {
    #[diesel(sql_type = BigInt)]
    archive_order: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = Text)]
    archive_name: String,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
}

#[derive(QueryableByName)]
struct AssetRow {
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = Text)]
    claim_kind: String,
}

#[test]
fn import_persists_native_logiqx_fields_defaults_order_locations_and_guards()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let database = Database::open(&database_path)?;
    let document_path = directory.path().join("native-logiqx.xml");
    std::fs::write(
        &document_path,
        r#"<datafile debug="yes">
  <header>
    <name>Native</name><description></description><version>1</version><author>Author</author>
    <clrmamepro header="" forcemerging="full"/>
    <romcenter plugin="" rommode="unmerged" lockrommode="yes"/>
  </header>
  <game name="repeat" isbios="yes">
    <comment></comment><comment>same</comment>
    <description></description><year>1990</year><manufacturer>Maker</manufacturer>
    <release name="Repeat" region="US"/>
    <release name="Repeat" region="EU" default="yes"/>
    <biosset name="base" description="Base"/>
    <biosset name="base" description="Alternate" default="yes"/>
    <rom name="rom.bin" size="12"/>
    <disk name="disk.chd"/>
    <archive name="container.zip"/>
    <sample name="intro"/>
  </game>
</datafile>"#,
    )?;
    let request = CatalogImportRequest {
        document_path: Utf8PathBuf::from_path_buf(document_path)
            .map_err(|_| "non-UTF-8 document path")?,
        format: CatalogDocumentFormat::Logiqx(
            mame_coalesce::logiqx::LogiqxMode::ObservedCompatible,
        ),
        source_key: PublishingSourceKey::new("native-logiqx-specification"),
        source_display_name: "Native Logiqx specification".to_owned(),
        catalog_key: CatalogKey::new("native-logiqx-specification"),
        catalog_display_name: "Native Logiqx specification".to_owned(),
        scope: CatalogScope::Complete,
    };

    let report = app::import_catalog(&database, &request)?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    let snapshot = report.snapshot_key.ok_or("snapshot missing")?;
    let mut conn = SqliteConnection::establish(database_path.as_str())?;
    let document = sql_query(
        "SELECT debug, debug_was_present, header_description \
         FROM logiqx_document_facts WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<DocumentRow>(&mut conn)?;
    assert_eq!(document.debug, "yes");
    assert!(document.debug_was_present);
    assert_eq!(document.header_description.as_deref(), Some(""));

    assert_header_options(&mut conn, &snapshot)?;

    let set = sql_query(
        "SELECT catalog_sets.set_id AS set_id FROM catalog_sets \
         JOIN catalog_set_groups USING (set_group_id) \
         WHERE snapshot_key = ? AND set_name = 'repeat'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<SetIdRow>(&mut conn)?;
    let game = sql_query("SELECT is_bios, is_bios_was_present FROM logiqx_games WHERE set_id = ?")
        .bind::<BigInt, _>(set.set_id)
        .get_result::<GameRow>(&mut conn)?;
    assert_eq!(game.is_bios, "yes");
    assert!(game.is_bios_was_present);

    assert_comments_and_releases(&mut conn, set.set_id)?;

    assert_bios_archives_and_media(&mut conn, set.set_id)?;

    assert!(
        sql_query("UPDATE logiqx_releases SET name = 'changed' WHERE set_id = ?")
            .bind::<BigInt, _>(set.set_id)
            .execute(&mut conn)
            .is_err()
    );
    assert!(
        sql_query("DELETE FROM logiqx_game_comments WHERE set_id = ?")
            .bind::<BigInt, _>(set.set_id)
            .execute(&mut conn)
            .is_err()
    );
    Ok(())
}

#[test]
fn headerless_native_document_keeps_option_families_absent()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("headerless.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let database = Database::open(&database_path)?;
    let document_path = directory.path().join("headerless.xml");
    std::fs::write(
        &document_path,
        "<datafile><game name='headerless'><description>Minimal</description></game></datafile>",
    )?;
    let request = CatalogImportRequest {
        document_path: Utf8PathBuf::from_path_buf(document_path)
            .map_err(|_| "non-UTF-8 document path")?,
        format: CatalogDocumentFormat::Logiqx(
            mame_coalesce::logiqx::LogiqxMode::ObservedCompatible,
        ),
        source_key: PublishingSourceKey::new("headerless-native-logiqx"),
        source_display_name: "Headerless Logiqx".to_owned(),
        catalog_key: CatalogKey::new("headerless-native-logiqx"),
        catalog_display_name: "Headerless Logiqx".to_owned(),
        scope: CatalogScope::Complete,
    };
    let report = app::import_catalog(&database, &request)?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    let snapshot = report.snapshot_key.ok_or("snapshot missing")?;
    let mut conn = SqliteConnection::establish(database_path.as_str())?;
    let document = sql_query(
        "SELECT debug, debug_was_present, header_description \
         FROM logiqx_document_facts WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<DocumentRow>(&mut conn)?;
    assert_eq!(document.debug, "no");
    assert!(!document.debug_was_present);
    assert_eq!(document.header_description, None);
    let set = sql_query(
        "SELECT catalog_sets.set_id AS set_id FROM catalog_sets \
         JOIN catalog_set_groups USING (set_group_id) \
         WHERE snapshot_key = ? AND set_name = 'headerless'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<SetIdRow>(&mut conn)?;
    let game = sql_query("SELECT is_bios, is_bios_was_present FROM logiqx_games WHERE set_id = ?")
        .bind::<BigInt, _>(set.set_id)
        .get_result::<GameRow>(&mut conn)?;
    assert_eq!(game.is_bios, "no");
    assert!(!game.is_bios_was_present);
    let count = sql_query(
        "SELECT COUNT(*) AS value FROM logiqx_clrmamepro_options \
         WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<CountRow>(&mut conn)?;
    assert_eq!(count.value, 0);
    let count =
        sql_query("SELECT COUNT(*) AS value FROM logiqx_romcenter_options WHERE snapshot_key = ?")
            .bind::<Text, _>(snapshot.as_str())
            .get_result::<CountRow>(&mut conn)?;
    assert_eq!(count.value, 0);
    Ok(())
}

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

fn assert_header_options(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
) -> Result<(), Box<dyn std::error::Error>> {
    let clrmamepro = sql_query(
        "SELECT source_order, source_line, source_column, header, header_was_present, forcemerging, \
                forcemerging_was_present, forcenodump, forcenodump_was_present, \
                forcepacking, forcepacking_was_present \
         FROM logiqx_clrmamepro_options WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<ClrOptionsRow>(conn)?;
    assert_eq!(clrmamepro.source_order, 4);
    assert!(clrmamepro.source_line > 0 && clrmamepro.source_column > 0);
    assert_eq!(clrmamepro.header.as_deref(), Some(""));
    assert!(clrmamepro.header_was_present);
    assert_eq!(clrmamepro.forcemerging, "full");
    assert!(clrmamepro.forcemerging_was_present);
    assert_eq!(clrmamepro.forcenodump, "obsolete");
    assert!(!clrmamepro.forcenodump_was_present);
    assert_eq!(clrmamepro.forcepacking, "zip");
    assert!(!clrmamepro.forcepacking_was_present);

    let romcenter = sql_query(
        "SELECT source_order, source_line, source_column, plugin, plugin_was_present, rommode, rommode_was_present, \
                biosmode, biosmode_was_present, samplemode, samplemode_was_present, \
                lockrommode, lockrommode_was_present, lockbiosmode, lockbiosmode_was_present, \
                locksamplemode, locksamplemode_was_present \
         FROM logiqx_romcenter_options WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<RomCenterOptionsRow>(conn)?;
    assert_eq!(romcenter.source_order, 5);
    assert!(romcenter.source_line > 0 && romcenter.source_column > 0);
    assert_eq!(romcenter.plugin.as_deref(), Some(""));
    assert!(romcenter.plugin_was_present);
    assert_eq!(romcenter.rommode, "unmerged");
    assert!(romcenter.rommode_was_present);
    assert_eq!(romcenter.biosmode, "split");
    assert!(!romcenter.biosmode_was_present);
    assert_eq!(romcenter.samplemode, "merged");
    assert!(!romcenter.samplemode_was_present);
    assert_eq!(romcenter.lockrommode, "yes");
    assert!(romcenter.lockrommode_was_present);
    assert_eq!(romcenter.lockbiosmode, "no");
    assert!(!romcenter.lockbiosmode_was_present);
    assert_eq!(romcenter.locksamplemode, "no");
    assert!(!romcenter.locksamplemode_was_present);
    Ok(())
}

fn assert_comments_and_releases(
    conn: &mut SqliteConnection,
    set_id: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    let comments = sql_query(
        "SELECT comment_order, source_order, comment_text, source_line, source_column \
         FROM logiqx_game_comments WHERE set_id = ? ORDER BY comment_order",
    )
    .bind::<BigInt, _>(set_id)
    .load::<CommentRow>(conn)?;
    assert_eq!(comments.len(), 2);
    assert_eq!(
        (comments[0].comment_order, comments[0].source_order),
        (0, 0)
    );
    assert_eq!(comments[0].comment_text, "");
    assert!(comments[0].source_line > 0 && comments[0].source_column > 0);
    assert_eq!(
        (comments[1].comment_order, comments[1].source_order),
        (1, 1)
    );
    assert_eq!(comments[1].comment_text, "same");

    let releases = sql_query(
        "SELECT release_order, source_order, name, region, language, date, \
                \"default\" AS \"default\", default_was_present, source_line, source_column \
         FROM logiqx_releases WHERE set_id = ? ORDER BY release_order",
    )
    .bind::<BigInt, _>(set_id)
    .load::<ReleaseRow>(conn)?;
    assert_eq!(releases.len(), 2);
    assert_eq!(
        (releases[0].release_order, releases[0].source_order),
        (0, 5)
    );
    assert_eq!(releases[0].name, "Repeat");
    assert_eq!(releases[0].region, "US");
    assert_eq!(releases[0].language, None);
    assert_eq!(releases[0].date, None);
    assert_eq!(releases[0].default, "no");
    assert!(!releases[0].default_was_present);
    assert!(releases[0].source_line > 0 && releases[0].source_column > 0);
    assert_eq!(
        (releases[1].release_order, releases[1].source_order),
        (1, 6)
    );
    assert_eq!(releases[1].name, "Repeat");
    assert_eq!(releases[1].region, "EU");
    assert_eq!(releases[1].default, "yes");
    assert!(releases[1].default_was_present);
    assert!(releases[1].source_line > 0 && releases[1].source_column > 0);
    Ok(())
}

fn assert_bios_archives_and_media(
    conn: &mut SqliteConnection,
    set_id: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    let bios_sets = sql_query(
        "SELECT bios_order, source_order, name, description, is_default, default_was_present, \
                source_line, source_column \
         FROM logiqx_bios_sets WHERE set_id = ? ORDER BY bios_order",
    )
    .bind::<BigInt, _>(set_id)
    .load::<BiosRow>(conn)?;
    assert_eq!(bios_sets.len(), 2);
    assert_eq!((bios_sets[0].bios_order, bios_sets[0].source_order), (0, 7));
    assert_eq!(bios_sets[0].name, "base");
    assert_eq!(bios_sets[0].description, "Base");
    assert_eq!(bios_sets[0].is_default, "no");
    assert!(!bios_sets[0].default_was_present);
    assert!(bios_sets[0].source_line > 0 && bios_sets[0].source_column > 0);
    assert_eq!((bios_sets[1].bios_order, bios_sets[1].source_order), (1, 8));
    assert_eq!(bios_sets[1].name, "base");
    assert_eq!(bios_sets[1].is_default, "yes");
    assert!(bios_sets[1].default_was_present);
    assert!(bios_sets[1].source_line > 0 && bios_sets[1].source_column > 0);

    let archives = sql_query(
        "SELECT archive_order, source_order, archive_name, source_line, source_column \
         FROM logiqx_archive_references \
         WHERE set_id = ? ORDER BY archive_order",
    )
    .bind::<BigInt, _>(set_id)
    .load::<ArchiveRow>(conn)?;
    assert_eq!(archives.len(), 1);
    assert_eq!(
        (archives[0].archive_order, archives[0].source_order),
        (0, 11)
    );
    assert_eq!(archives[0].archive_name, "container.zip");
    assert!(archives[0].source_line > 0 && archives[0].source_column > 0);

    let assets = sql_query(
        "SELECT COALESCE(rom.source_order, disk.source_order, sample.source_order) AS source_order, \
                occurrence.claim_kind \
         FROM asset_occurrences AS occurrence \
         LEFT JOIN logiqx_rom_claims AS rom USING (occurrence_id) \
         LEFT JOIN logiqx_disk_claims AS disk USING (occurrence_id) \
         LEFT JOIN logiqx_sample_claims AS sample USING (occurrence_id) \
         WHERE record_id = ? ORDER BY source_order",
    )
    .bind::<BigInt, _>(set_id)
    .load::<AssetRow>(conn)?;
    assert_eq!(assets.len(), 3);
    assert_eq!(assets[0].source_order, 9);
    assert_eq!(assets[1].source_order, 10);
    assert_eq!(assets[2].source_order, 12);
    assert!(assets.iter().any(|asset| asset.claim_kind == "logiqx_rom"));
    assert!(assets.iter().any(|asset| asset.claim_kind == "logiqx_disk"));
    assert!(
        assets
            .iter()
            .any(|asset| asset.claim_kind == "logiqx_sample")
    );
    Ok(())
}
