use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Nullable, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    database::Database,
    domain::{
        CatalogKey, CatalogScope, CatalogSnapshotDiff, PublishingSourceKey, SnapshotKey,
        SnapshotRecordStatus,
    },
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

struct PcCatalog {
    directory: tempfile::TempDir,
    database: Database,
    database_path: Utf8PathBuf,
}

impl PcCatalog {
    fn new() -> TestResult<Self> {
        let directory = tempfile::tempdir()?;
        let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
            .map_err(|_| "non-UTF-8 database path")?;
        let database = Database::open(&database_path)?;
        Ok(Self {
            directory,
            database,
            database_path,
        })
    }

    fn import(&self, name: &str, source: &str) -> TestResult<SnapshotKey> {
        self.import_bytes(name, source.as_bytes())
    }

    fn import_bytes(&self, name: &str, source: &[u8]) -> TestResult<SnapshotKey> {
        let document_path =
            Utf8PathBuf::from_path_buf(self.directory.path().join(format!("{name}.xml")))
                .map_err(|_| "non-UTF-8 document path")?;
        std::fs::write(&document_path, source)?;
        let report = app::import_catalog(
            &self.database,
            &CatalogImportRequest {
                document_path,
                format: CatalogDocumentFormat::NoIntroPcXml,
                source_key: PublishingSourceKey::new("synthetic-pc-history"),
                source_display_name: "Synthetic P/C history".into(),
                catalog_key: CatalogKey::new("synthetic-pc-history"),
                catalog_display_name: "Synthetic P/C history".into(),
                scope: CatalogScope::Complete,
            },
        )?;
        assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
        Ok(report
            .snapshot_key
            .ok_or("successful import has no snapshot")?)
    }

    fn connection(&self) -> TestResult<SqliteConnection> {
        Ok(SqliteConnection::establish(self.database_path.as_str())?)
    }

    fn diff(&self, before: &str, after: &str) -> TestResult<CatalogSnapshotDiff> {
        let previous = self.import("previous", before)?;
        let current = self.import("current", after)?;
        Ok(app::diff_catalog_snapshots(
            &self.database,
            &previous,
            &current,
        )?)
    }
}

#[derive(QueryableByName)]
struct HeaderChild {
    #[diesel(sql_type = Text)]
    kind: String,
    #[diesel(sql_type = Text)]
    value: String,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
}

#[derive(QueryableByName)]
struct NativeSize {
    #[diesel(sql_type = Nullable<Text>)]
    size_text: Option<String>,
    #[diesel(sql_type = Nullable<BigInt>)]
    size: Option<i64>,
    #[diesel(sql_type = BigInt)]
    content_uuid_is_null: i64,
}

fn game_record(
    diff: &CatalogSnapshotDiff,
) -> TestResult<&mame_coalesce::domain::SnapshotRecordDiff> {
    assert_eq!(diff.records.len(), 1);
    diff.records
        .first()
        .ok_or_else(|| "P/C game history missing".into())
}

#[test]
fn header_values_repeats_empty_presence_and_cross_family_order_are_historical() -> TestResult {
    let catalog = PcCatalog::new()?;
    let game = "<game name='same'><rom name='same.bin' size='4'/></game>";
    let base = format!(
        "<datafile><header><name>First</name><description>One</description><version>v1</version><name>Second</name><description>Two</description></header>{game}</datafile>"
    );
    for changed in [
        format!(
            "<datafile><header><name>Changed</name><description>One</description><version>v1</version><name>Second</name><description>Two</description></header>{game}</datafile>"
        ),
        format!(
            "<datafile><header><name>First</name><description>Changed</description><version>v1</version><name>Second</name><description>Two</description></header>{game}</datafile>"
        ),
        format!(
            "<datafile><header><name>First</name><description>One</description><version>v2</version><name>Second</name><description>Two</description></header>{game}</datafile>"
        ),
        format!(
            "<datafile><header><name>First</name><description>One</description><version>v1</version><name/><description>Two</description></header>{game}</datafile>"
        ),
        format!(
            "<datafile><header><name>First</name><description>One</description><version>v1</version><name>Second</name><description/></header>{game}</datafile>"
        ),
        format!(
            "<datafile><header><name>First</name><description>One</description><version/></header>{game}</datafile>"
        ),
        format!(
            "<datafile><header><name>First</name><description>One</description><name>Second</name><description>Two</description></header>{game}</datafile>"
        ),
        format!("<datafile>{game}</datafile>"),
    ] {
        let diff = catalog.diff(&base, &changed)?;
        assert!(diff.document_metadata_changed);
        let record = game_record(&diff)?;
        assert_eq!(record.status, SnapshotRecordStatus::Unchanged);
        assert!(!record.metadata_changed);
        assert!(record.requirement_changes.is_empty());
    }

    let before = catalog.import("header-before", &base)?;
    let mut connection = catalog.connection()?;
    let rows = sql_query(
        "SELECT 'name' AS kind,name_text AS value,source_order FROM no_intro_pc_header_names WHERE snapshot_key=? \
         UNION ALL SELECT 'description',description_text,source_order FROM no_intro_pc_header_descriptions WHERE snapshot_key=? \
         UNION ALL SELECT 'version',version_text,version_order FROM no_intro_pc_headers WHERE snapshot_key=? ORDER BY source_order",
    )
    .bind::<Text, _>(before.as_str())
    .bind::<Text, _>(before.as_str())
    .bind::<Text, _>(before.as_str())
    .load::<HeaderChild>(&mut connection)?;
    assert_eq!(
        rows.iter()
            .map(|row| (row.kind.as_str(), row.value.as_str(), row.source_order))
            .collect::<Vec<_>>(),
        [
            ("name", "First", 0),
            ("description", "One", 1),
            ("version", "v1", 2),
            ("name", "Second", 3),
            ("description", "Two", 4)
        ]
    );
    let reordered = format!(
        "<datafile><header><description>One</description><name>First</name><version>v1</version><name>Second</name><description>Two</description></header>{game}</datafile>"
    );
    assert!(catalog.diff(&base, &reordered)?.document_metadata_changed);
    Ok(())
}

#[test]
fn vendor_insertions_and_reindent_do_not_fake_edits_but_header_game_move_does() -> TestResult {
    let catalog = PcCatalog::new()?;
    let before = "<datafile><header><name>Pack</name><version>v1</version></header><game name='same'><description>Stable</description><rom name='a.bin' size='4'/><rom name='b.bin' size='8'/></game></datafile>";
    let edited_formatting = "<datafile>\n  <header>\n    <name>Pack</name><vendor>ignored</vendor>\n    <version>v1</version>\n  </header>\n  <vendor>ignored</vendor>\n  <game name='same'>\n    <description>Stable</description><vendor>ignored</vendor>\n    <rom name='a.bin' size='4'/>\n    <rom name='b.bin' size='8'/>\n  </game>\n</datafile>";
    let diff = catalog.diff(before, edited_formatting)?;
    assert!(!diff.document_metadata_changed);
    let record = game_record(&diff)?;
    assert_eq!(record.status, SnapshotRecordStatus::Unchanged);
    assert!(!record.metadata_changed);
    assert!(record.requirement_changes.is_empty());

    let moved = "<datafile><game name='same'><description>Stable</description><rom name='a.bin' size='4'/><rom name='b.bin' size='8'/></game><header><name>Pack</name><version>v1</version></header></datafile>";
    let moved_diff = catalog.diff(before, moved)?;
    assert!(moved_diff.document_metadata_changed);
    let record = game_record(&moved_diff)?;
    assert_eq!(record.status, SnapshotRecordStatus::Unchanged);
    assert!(!record.metadata_changed);
    Ok(())
}

#[test]
fn synthetic_game_attribute_order_is_native_history_metadata() -> TestResult {
    let catalog = PcCatalog::new()?;
    let diff = catalog.diff(
        "<datafile><game name='same' id='01' region='USA'/></datafile>",
        "<datafile><game region='USA' id='01' name='same'/></datafile>",
    )?;
    let record = game_record(&diff)?;
    assert!(!diff.document_metadata_changed);
    assert!(
        record.metadata_changed,
        "recognized game attribute order was lost"
    );
    assert!(record.requirement_changes.is_empty());
    Ok(())
}

#[test]
fn synthetic_attribute_vendor_gaps_and_coordinates_are_history_neutral() -> TestResult {
    let catalog = PcCatalog::new()?;
    let diff = catalog.diff(
        "<datafile><game name='same' id='01'><rom name='same.bin' size='1' crc='12345678'/></game></datafile>",
        "<datafile>\r\n <game xmlns:v='urn:vendor' name='same' v:note='é'\r\n\t id='01'>\r\n <rom name='same.bin' v:note='𝄞'\r\n size='1' crc='12345678'/>\r\n </game></datafile>",
    )?;
    let record = game_record(&diff)?;
    assert!(!diff.document_metadata_changed);
    assert!(!record.metadata_changed);
    assert!(record.requirement_changes.is_empty());
    Ok(())
}

#[derive(QueryableByName)]
struct AttributeRelationshipLocation {
    #[diesel(sql_type = Text)]
    field: String,
    #[diesel(sql_type = BigInt)]
    line: i64,
    #[diesel(sql_type = BigInt)]
    column: i64,
}

#[test]
fn synthetic_parent_relationships_locate_the_declaring_attribute() -> TestResult {
    let catalog = PcCatalog::new()?;
    catalog.import(
        "attributes",
        "<datafile>\n  <game name='same'\n    clone='02'\n    mergeof='03'/>\n</datafile>",
    )?;
    let rows = sql_query(
        "SELECT source_field AS field,source_line AS line,source_column AS column FROM no_intro_pc_source_relationships ORDER BY source_field",
    )
    .load::<AttributeRelationshipLocation>(&mut catalog.connection()?)?;
    let actual = rows
        .into_iter()
        .map(|row| (row.field, row.line, row.column))
        .collect::<Vec<_>>();
    assert_eq!(actual, [("clone".into(), 3, 5), ("mergeof".into(), 4, 5)]);
    Ok(())
}

#[test]
fn synthetic_rom_attribute_order_changes_only_other_evidence() -> TestResult {
    let catalog = PcCatalog::new()?;
    let diff = catalog.diff(
        "<datafile><game name='same'><rom name='same.bin' size='01' crc='12345678'/></game></datafile>",
        "<datafile><game name='same'><rom crc='12345678' size='01' name='same.bin'/></game></datafile>",
    )?;
    let record = game_record(&diff)?;
    assert!(!diff.document_metadata_changed);
    assert!(!record.metadata_changed);
    assert_eq!(
        record.requirement_changes.len(),
        1,
        "recognized ROM attribute order was lost"
    );
    let requirement = record
        .requirement_changes
        .first()
        .ok_or("missing ROM attribute history")?;
    assert!(!requirement.size_changed);
    assert!(!requirement.hash_changed);
    assert!(requirement.other_evidence_changed);
    Ok(())
}

#[test]
fn moving_game_description_between_roms_changes_game_metadata_only() -> TestResult {
    let catalog = PcCatalog::new()?;
    let before = "<datafile><game name='same'><description>Old</description><rom name='a.bin' size='4'/><rom name='b.bin' size='8'/></game></datafile>";
    let after = "<datafile><game name='same'><rom name='a.bin' size='4'/><description>Old</description><rom name='b.bin' size='8'/></game></datafile>";
    let diff = catalog.diff(before, after)?;
    assert!(!diff.document_metadata_changed);
    let record = game_record(&diff)?;
    assert_eq!(record.status, SnapshotRecordStatus::Changed);
    assert!(record.metadata_changed);
    assert!(record.requirement_changes.is_empty());
    let mut connection = catalog.connection()?;
    let orders = sql_query(
        "SELECT game.description_order AS value FROM no_intro_pc_games AS game JOIN catalog_sets AS sets USING(set_id) JOIN catalog_set_groups AS groups USING(set_group_id) WHERE groups.snapshot_key=?",
    )
    .bind::<Text, _>(diff.previous.as_str())
    .get_result::<OrderValue>(&mut connection)?;
    assert_eq!(orders.value, 0);
    let current_order = sql_query(
        "SELECT game.description_order AS value FROM no_intro_pc_games AS game JOIN catalog_sets AS sets USING(set_id) JOIN catalog_set_groups AS groups USING(set_group_id) WHERE groups.snapshot_key=?",
    )
    .bind::<Text, _>(diff.current.as_str())
    .get_result::<OrderValue>(&mut connection)?;
    assert_eq!(current_order.value, 1);
    Ok(())
}

#[derive(QueryableByName)]
struct OrderValue {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

#[test]
fn raw_rom_size_spelling_changes_history_while_numeric_size_stays_equal() -> TestResult {
    let catalog = PcCatalog::new()?;
    let diff = catalog.diff(
        "<datafile><game name='same'><rom name='same.bin' size='0004'/></game></datafile>",
        "<datafile><game name='same'><rom name='same.bin' size='4'/></game></datafile>",
    )?;
    let record = game_record(&diff)?;
    assert_eq!(record.status, SnapshotRecordStatus::Changed);
    assert_eq!(record.requirement_changes.len(), 1);
    let requirement = &record.requirement_changes[0];
    assert!(!requirement.size_changed);
    assert!(!requirement.hash_changed);
    assert!(requirement.other_evidence_changed);

    let mut connection = catalog.connection()?;
    let sizes = [diff.previous.as_str(), diff.current.as_str()]
        .map(|snapshot| {
            sql_query(
                "SELECT claim.size_text,claim.size,occurrence.content_uuid IS NULL AS content_uuid_is_null FROM no_intro_pc_file_claims AS claim JOIN asset_occurrences AS occurrence USING(occurrence_id) JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id JOIN catalog_set_groups AS groups USING(set_group_id) WHERE groups.snapshot_key=?",
            )
            .bind::<Text, _>(snapshot)
            .get_result::<NativeSize>(&mut connection)
        });
    let [previous, current] = sizes;
    let previous = previous?;
    let current = current?;
    assert_eq!(previous.size_text.as_deref(), Some("0004"));
    assert_eq!(current.size_text.as_deref(), Some("4"));
    assert_eq!(previous.size, Some(4));
    assert_eq!(current.size, Some(4));
    Ok(())
}

#[test]
fn native_size_spelling_edit_remains_visible_alongside_a_hash_edit() -> TestResult {
    let catalog = PcCatalog::new()?;
    let diff = catalog.diff(
        "<datafile><game name='same'><rom name='same.bin' size='0004' sha1='0000000000000000000000000000000000000000'/></game></datafile>",
        "<datafile><game name='same'><rom name='same.bin' size='4' sha1='1111111111111111111111111111111111111111'/></game></datafile>",
    )?;
    let record = game_record(&diff)?;
    assert_eq!(record.requirement_changes.len(), 1);
    let change = &record.requirement_changes[0];
    assert!(!change.size_changed);
    assert!(change.hash_changed);
    assert!(change.other_evidence_changed);
    Ok(())
}

#[test]
fn unsigned_u64_maximum_is_retained_without_signed_size_or_uuid() -> TestResult {
    let catalog = PcCatalog::new()?;
    let mut connection = catalog.connection()?;
    for (index, raw_size) in ["18446744073709551615", "+00018446744073709551615"]
        .iter()
        .enumerate()
    {
        let source = format!(
            "<datafile><game name='game-{index}'><rom name='large.bin' size='{raw_size}' sha1='0123456789abcdef0123456789abcdef01234567'/></game></datafile>"
        );
        let snapshot = catalog.import(&format!("large-{index}"), &source)?;
        let stored = sql_query(
            "SELECT claim.size_text,claim.size,occurrence.content_uuid IS NULL AS content_uuid_is_null FROM no_intro_pc_file_claims AS claim JOIN asset_occurrences AS occurrence USING(occurrence_id) JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id JOIN catalog_set_groups AS groups USING(set_group_id) WHERE groups.snapshot_key=?",
        )
        .bind::<Text, _>(snapshot.as_str())
        .get_result::<NativeSize>(&mut connection)?;
        assert_eq!(stored.size_text.as_deref(), Some(*raw_size));
        assert_eq!(stored.size, None);
        assert_eq!(stored.content_uuid_is_null, 1);
    }
    Ok(())
}

#[test]
fn native_version_is_public_history_and_relationship_provenance_not_snapshot_column() -> TestResult
{
    let catalog = PcCatalog::new()?;
    let snapshot = catalog.import(
        "versioned",
        "<datafile><header><name>Pack</name><version>native-v7</version></header><game name='clone' clone='0007'/></datafile>",
    )?;
    let history =
        app::catalog_snapshot_history(&catalog.database, &CatalogKey::new("synthetic-pc-history"))?;
    let entry = history
        .iter()
        .find(|entry| entry.snapshot == snapshot)
        .ok_or("snapshot missing from public history")?;
    assert_eq!(entry.declared_version.as_deref(), Some("native-v7"));

    let explanation = app::explain_relationships(&catalog.database)?
        .into_iter()
        .find(|item| {
            item.source
                .as_ref()
                .is_some_and(|source| source.declared_version.as_deref() == Some("native-v7"))
        })
        .ok_or("clone relationship with native version provenance missing")?;
    assert_eq!(
        explanation
            .source
            .as_ref()
            .ok_or("missing source provenance")?
            .declared_version
            .as_deref(),
        Some("native-v7")
    );

    let mut connection = catalog.connection()?;
    let raw_version =
        sql_query("SELECT CASE WHEN EXISTS (SELECT 1 FROM pragma_table_info('catalog_snapshots') WHERE name='declared_version') THEN 'copied' ELSE NULL END AS value")
            .get_result::<NullableText>(&mut connection)?;
    assert_eq!(raw_version.value, None);
    Ok(())
}

#[derive(QueryableByName)]
struct NullableText {
    #[diesel(sql_type = Nullable<Text>)]
    value: Option<String>,
}

#[derive(QueryableByName)]
struct NativeAttributePosition {
    #[diesel(sql_type = Text)]
    owner: String,
    #[diesel(sql_type = BigInt)]
    field_kind: i64,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
}

#[derive(QueryableByName)]
struct OccurrenceIdentity {
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
}

#[test]
fn every_synthetic_attribute_retains_qname_position_in_utf8_and_utf16() -> TestResult {
    use mame_coalesce::catalog_files::{self, NoIntroPcRomAttribute, OccurrenceId};
    let catalog = PcCatalog::new()?;
    let body = "<datafile>\r\n\t<game xmlns:v='urn:vendor' v:note='é𝄞' name='same'\r\n    id='0001' namealt='' region='' languages=''\r\n    version='' bios='no' clone='P' mergeof='0009'>\r\n\t<rom v:note='é𝄞' name='same.bin'\r\n    size='0001' crc='12345678'\r\n    md5='11111111111111111111111111111111' sha1='2222222222222222222222222222222222222222'/>\r\n</game></datafile>";
    let utf8 = format!("<?xml version='1.0' encoding='UTF-8'?>\r\n{body}").into_bytes();
    let utf16_text = format!("<?xml version='1.0' encoding='UTF-16'?>\r\n{body}");
    let mut utf16 = vec![0xff, 0xfe];
    utf16.extend(utf16_text.encode_utf16().flat_map(u16::to_le_bytes));
    let mut requested = Vec::new();
    for (name, bytes) in [("utf8-positions", utf8), ("utf16-positions", utf16)] {
        let snapshot = catalog.import_bytes(name, &bytes)?;
        let mut conn = catalog.connection()?;
        let rows = sql_query(
            "SELECT 'game' AS owner, positions.field_kind, positions.source_order, positions.source_line, positions.source_column \
             FROM catalog_set_groups AS groups JOIN catalog_sets AS sets USING(set_group_id) \
             JOIN no_intro_pc_game_attribute_positions AS positions USING(set_id) WHERE groups.snapshot_key=? \
             UNION ALL SELECT 'rom', positions.field_kind, positions.source_order, positions.source_line, positions.source_column \
             FROM catalog_set_groups AS groups JOIN catalog_sets AS sets USING(set_group_id) \
             JOIN asset_occurrences AS occurrence ON occurrence.record_id=sets.set_id \
             JOIN no_intro_pc_rom_attribute_positions AS positions USING(occurrence_id) WHERE groups.snapshot_key=? \
             ORDER BY 1,3",
        )
        .bind::<Text, _>(snapshot.as_str())
        .bind::<Text, _>(snapshot.as_str())
        .load::<NativeAttributePosition>(&mut conn)?;
        assert_eq!(
            rows.iter()
                .map(|row| (
                    row.owner.as_str(),
                    row.field_kind,
                    row.source_order,
                    row.source_line,
                    row.source_column
                ))
                .collect::<Vec<_>>(),
            [
                ("game", 0, 2, 3, 41),
                ("game", 1, 3, 4, 5),
                ("game", 2, 4, 4, 15),
                ("game", 3, 5, 4, 26),
                ("game", 4, 6, 4, 36),
                ("game", 5, 7, 5, 5),
                ("game", 6, 8, 5, 16),
                ("game", 7, 9, 5, 26),
                ("game", 8, 10, 5, 36),
                ("rom", 0, 1, 6, 19),
                ("rom", 1, 2, 7, 5),
                ("rom", 2, 3, 7, 17),
                ("rom", 3, 4, 8, 5),
                ("rom", 4, 5, 8, 44),
            ]
        );
        let rule = sql_query("SELECT rules_version AS value FROM parser_interpretations JOIN catalog_snapshots USING(interpretation_key) WHERE snapshot_key=?")
            .bind::<Text,_>(snapshot.as_str()).get_result::<HeaderValue>(&mut conn)?;
        assert_eq!(rule.value, "no-intro-pc-synthetic-provenance-v2");
        let ids = sql_query("SELECT occurrence_id FROM asset_occurrences AS occurrence JOIN catalog_sets AS sets ON occurrence.record_id=sets.set_id JOIN catalog_set_groups AS groups USING(set_group_id) WHERE groups.snapshot_key=?")
            .bind::<Text,_>(snapshot.as_str()).load::<OccurrenceIdentity>(&mut conn)?;
        assert_eq!(ids.len(), 1);
        requested.push(OccurrenceId::from_database(
            ids.first()
                .ok_or("native P/C occurrence missing")?
                .occurrence_id,
        ));
    }
    catalog.import(
        "unrequested",
        "<datafile><game name='outside'><rom size='2' name='outside.bin'/></game></datafile>",
    )?;
    // Multiple requested snapshots must keep their positions with their own ROM,
    // without fetching attribute rows from the unrequested snapshot.
    let files = catalog_files::occurrences_for_ids(&catalog.database, &requested)?;
    assert_eq!(files.len(), 2);
    assert_eq!(
        files
            .iter()
            .map(|file| file.occurrence_id)
            .collect::<Vec<_>>(),
        requested
    );
    for file in files {
        let payload = file.no_intro_pc_rom.ok_or("missing native P/C ROM")?;
        assert_eq!(
            payload
                .attribute_positions
                .iter()
                .map(|position| (
                    position.field,
                    position.source_order,
                    position.location.line,
                    position.location.column
                ))
                .collect::<Vec<_>>(),
            [
                (NoIntroPcRomAttribute::Name, 1, 6, 19),
                (NoIntroPcRomAttribute::Size, 2, 7, 5),
                (NoIntroPcRomAttribute::Crc, 3, 7, 17),
                (NoIntroPcRomAttribute::Md5, 4, 8, 5),
                (NoIntroPcRomAttribute::Sha1, 5, 8, 44),
            ]
        );
    }
    Ok(())
}

#[test]
fn backup_restore_retains_native_header_and_source_after_input_removal() -> TestResult {
    let catalog = PcCatalog::new()?;
    let source = "<datafile><header><name>  Pack &amp; Co  </name><version>native-v9</version><description/><name>Second</name></header><game name='same'><description>Game</description><rom name='a.bin' size='0004'/></game></datafile>";
    let snapshot = catalog.import("backup-source", source)?;
    let backup_path = Utf8PathBuf::from_path_buf(catalog.directory.path().join("backup.sqlite"))
        .map_err(|_| "non-UTF-8 backup path")?;
    let restored_path =
        Utf8PathBuf::from_path_buf(catalog.directory.path().join("restored.sqlite"))
            .map_err(|_| "non-UTF-8 restored path")?;
    let input_path = catalog.directory.path().join("backup-source.xml");
    drop(catalog.database);
    std::fs::remove_file(input_path)?;
    assert!(matches!(
        mame_coalesce::create_backup(&catalog.database_path, &backup_path)?,
        mame_coalesce::BackupOutcome::Published
            | mame_coalesce::BackupOutcome::PublishedDurabilityUnconfirmed { .. }
    ));
    assert!(matches!(
        mame_coalesce::restore_backup(
            &backup_path,
            &restored_path,
            mame_coalesce::RestorePolicy::CreateNew
        )?,
        mame_coalesce::RestoreOutcome::Published
            | mame_coalesce::RestoreOutcome::PublishedDurabilityUnconfirmed { .. }
    ));
    assert!(mame_coalesce::check_integrity(&restored_path)?.is_clean());
    let restored = Database::open(&restored_path)?;
    assert_eq!(
        app::load_snapshot_source(&restored, &snapshot)?,
        source.as_bytes()
    );
    let mut restored_connection = SqliteConnection::establish(restored_path.as_str())?;
    let values = sql_query(
        "SELECT name_text AS value FROM no_intro_pc_header_names WHERE snapshot_key=? ORDER BY source_order",
    )
    .bind::<Text, _>(snapshot.as_str())
    .load::<HeaderValue>(&mut restored_connection)?;
    assert_eq!(
        values
            .iter()
            .map(|row| row.value.as_str())
            .collect::<Vec<_>>(),
        ["  Pack & Co  ", "Second"]
    );
    let version =
        sql_query("SELECT version_text AS value FROM no_intro_pc_headers WHERE snapshot_key=?")
            .bind::<Text, _>(snapshot.as_str())
            .get_result::<HeaderValue>(&mut restored_connection)?;
    assert_eq!(version.value, "native-v9");
    let descriptions = sql_query(
        "SELECT description_text AS value FROM no_intro_pc_header_descriptions WHERE snapshot_key=?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<HeaderValue>(&mut restored_connection)?;
    assert_eq!(descriptions.value, "");
    let size = sql_query(
        "SELECT size_text AS value FROM no_intro_pc_file_claims AS claim JOIN asset_occurrences AS occurrence USING(occurrence_id) JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id JOIN catalog_set_groups AS groups USING(set_group_id) WHERE groups.snapshot_key=?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<HeaderValue>(&mut restored_connection)?;
    assert_eq!(size.value, "0004");
    let ids = sql_query("SELECT occurrence_id FROM asset_occurrences AS occurrence JOIN catalog_sets AS sets ON occurrence.record_id=sets.set_id JOIN catalog_set_groups AS groups USING(set_group_id) WHERE groups.snapshot_key=?")
        .bind::<Text,_>(snapshot.as_str()).load::<OccurrenceIdentity>(&mut restored_connection)?;
    let ids = ids
        .into_iter()
        .map(|row| mame_coalesce::catalog_files::OccurrenceId::from_database(row.occurrence_id))
        .collect::<Vec<_>>();
    let files = mame_coalesce::catalog_files::occurrences_for_ids(&restored, &ids)?;
    let payload = files
        .first()
        .and_then(|file| file.no_intro_pc_rom.as_ref())
        .ok_or("restored native P/C ROM missing")?;
    assert_eq!(
        payload
            .attribute_positions
            .iter()
            .map(|position| position.field.as_str())
            .collect::<Vec<_>>(),
        ["name", "size"]
    );
    Ok(())
}

#[derive(QueryableByName)]
struct HeaderValue {
    #[diesel(sql_type = Text)]
    value: String,
}
