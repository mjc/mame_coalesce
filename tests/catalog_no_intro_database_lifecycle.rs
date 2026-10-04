use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, connection::SimpleConnection,
    sql_query, sql_types::Text,
};
use mame_coalesce::{
    RestorePolicy,
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    catalog_no_intro_database::{NoIntroDatabasePageLimit, games_for_snapshot},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey, SnapshotKey},
    no_intro_db_xml::NoIntroDatabaseMode,
};
use std::fmt::Write as _;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

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

    fn import(&self, key: &str, xml: &str) -> TestResult<SnapshotKey> {
        self.import_format(
            key,
            xml,
            CatalogDocumentFormat::NoIntroDatabase(NoIntroDatabaseMode::ObservedCompatible),
        )
    }

    fn import_format(
        &self,
        key: &str,
        xml: &str,
        format: CatalogDocumentFormat,
    ) -> TestResult<SnapshotKey> {
        let input = Utf8PathBuf::try_from(self.directory.path().join(format!("{key}.xml")))?;
        std::fs::write(&input, xml)?;
        let report = app::import_catalog(
            &self.database,
            &CatalogImportRequest {
                document_path: input,
                format,
                source_key: PublishingSourceKey::new(key),
                source_display_name: key.into(),
                catalog_key: CatalogKey::new(key),
                catalog_display_name: key.into(),
                scope: CatalogScope::Complete,
            },
        )?;
        assert_eq!(report.status, CatalogImportStatus::Succeeded);
        Ok(report.snapshot_key.ok_or("missing published snapshot")?)
    }

    fn hide_sources(&self, key: &str) -> TestResult {
        std::fs::rename(
            self.directory.path().join(format!("{key}.xml")),
            self.directory.path().join(format!("{key}.unavailable")),
        )?;
        std::fs::rename(
            format!("{}.documents", self.path),
            self.directory.path().join("objects-unavailable"),
        )?;
        Ok(())
    }
}

const THREE_GAMES: &str =
    "<datafile><game name='repeat'/><game name='repeat'/><game name='last'/></datafile>";

const OWNER_DOCUMENT: &str = "<datafile><header><author>a</author></header><game name='owner'><archive name='a'/><source><details dumper='d'/><serials box_serial='s'/><file size='1'/></source><release><details group='g' nfo_crc32='12345678'/><serials box_serial='s'/><file size='1'/></release></game></datafile>";

#[derive(QueryableByName)]
struct Guard {
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    sql: String,
}

fn corrupt_temporary_owner(catalog: &Catalog, table: &str, statement: &str) -> TestResult {
    let mut connection = SqliteConnection::establish(catalog.path.as_str())?;
    let guards =
        sql_query("SELECT name,sql FROM sqlite_schema WHERE type='trigger' AND tbl_name=?")
            .bind::<Text, _>(table)
            .load::<Guard>(&mut connection)?;
    assert!(!guards.is_empty(), "{table} needs its native guards");
    connection.batch_execute("PRAGMA ignore_check_constraints=ON; PRAGMA foreign_keys=OFF;")?;
    for guard in &guards {
        connection.batch_execute(&format!("DROP TRIGGER {}", guard.name))?;
    }
    let result = connection.batch_execute(statement);
    for guard in &guards {
        connection.batch_execute(&guard.sql)?;
    }
    connection.batch_execute("PRAGMA ignore_check_constraints=OFF; PRAGMA foreign_keys=ON;")?;
    result?;
    Ok(())
}

#[test]
fn native_reader_rejects_fractional_provenance_instead_of_truncating_it() -> TestResult {
    for (table, column) in [
        ("no_intro_exports", "source_column"),
        ("no_intro_export_headers", "source_column"),
        ("no_intro_header_fields", "source_order"),
        ("no_intro_database_parse_counts", "game_count"),
        ("no_intro_database_parse_counts", "header_field_count"),
        ("catalog_sets", "list_order"),
        ("no_intro_database_games", "name_source_order"),
        ("no_intro_archive_descriptions", "source_order"),
        ("no_intro_dump_sources", "source_order"),
        ("no_intro_dump_details", "opening_end_column"),
        ("no_intro_dump_serials", "source_column"),
        ("no_intro_dump_files", "source_order"),
        ("no_intro_releases", "source_column"),
        ("no_intro_release_details", "opening_end_column"),
        ("no_intro_release_serials", "source_order"),
        ("no_intro_release_files", "source_column"),
        ("no_intro_archive_field_positions", "source_column"),
        ("no_intro_dump_details_field_positions", "source_order"),
        ("no_intro_dump_serials_field_positions", "field_kind"),
        ("no_intro_dump_file_field_positions", "source_line"),
        ("no_intro_release_details_field_positions", "source_order"),
        ("no_intro_release_serials_field_positions", "source_column"),
        ("no_intro_release_file_field_positions", "field_kind"),
    ] {
        let catalog = Catalog::new()?;
        let snapshot = catalog.import("fractional", OWNER_DOCUMENT)?;
        assert_eq!(
            games_for_snapshot(
                &catalog.database,
                &snapshot,
                None,
                NoIntroDatabasePageLimit::new(1)?
            )?
            .games
            .len(),
            1
        );
        corrupt_temporary_owner(
            &catalog,
            table,
            &format!("UPDATE {table} SET {column}={column}+0.5"),
        )?;
        assert!(
            games_for_snapshot(
                &catalog.database,
                &snapshot,
                None,
                NoIntroDatabasePageLimit::new(1)?
            )
            .is_err(),
            "{table}.{column} must not be silently coerced"
        );
    }
    Ok(())
}

#[test]
fn native_reader_rejects_missing_attribute_witnesses_and_nfo_values() -> TestResult {
    for table in [
        "no_intro_exports",
        "no_intro_export_headers",
        "no_intro_database_games",
        "no_intro_database_parse_counts",
        "no_intro_archive_field_positions",
        "no_intro_dump_details_field_positions",
        "no_intro_dump_serials_field_positions",
        "no_intro_dump_file_field_positions",
        "no_intro_dump_files",
        "no_intro_release_details_field_positions",
        "no_intro_release_serials_field_positions",
        "no_intro_release_file_field_positions",
        "no_intro_release_files",
        "no_intro_release_nfo_hashes",
    ] {
        let catalog = Catalog::new()?;
        let snapshot = catalog.import("missing-witness", OWNER_DOCUMENT)?;
        assert_eq!(
            games_for_snapshot(
                &catalog.database,
                &snapshot,
                None,
                NoIntroDatabasePageLimit::new(1)?
            )?
            .games
            .len(),
            1
        );
        corrupt_temporary_owner(&catalog, table, &format!("DELETE FROM {table}"))?;
        assert!(
            games_for_snapshot(
                &catalog.database,
                &snapshot,
                None,
                NoIntroDatabasePageLimit::new(1)?
            )
            .is_err(),
            "{table} missing facts must not produce a partial successful page"
        );
    }
    Ok(())
}

#[test]
fn missing_optional_owners_do_not_hide_surviving_attribute_positions() -> TestResult {
    for table in [
        "no_intro_dump_details",
        "no_intro_dump_serials",
        "no_intro_release_details",
        "no_intro_release_serials",
    ] {
        let catalog = Catalog::new()?;
        let snapshot = catalog.import("missing-optional-owner", OWNER_DOCUMENT)?;
        games_for_snapshot(
            &catalog.database,
            &snapshot,
            None,
            NoIntroDatabasePageLimit::new(1)?,
        )?;
        corrupt_temporary_owner(&catalog, table, &format!("DELETE FROM {table}"))?;
        assert!(
            games_for_snapshot(
                &catalog.database,
                &snapshot,
                None,
                NoIntroDatabasePageLimit::new(1)?
            )
            .is_err(),
            "{table} must not hide surviving attribute positions"
        );
    }
    Ok(())
}

#[test]
fn pages_validate_saved_game_counts_and_contiguous_orders() -> TestResult {
    for statement in [
        "DELETE FROM catalog_sets WHERE list_order=2",
        "DELETE FROM catalog_sets WHERE list_order=0",
        "UPDATE catalog_sets SET list_order=200 WHERE list_order=2",
        "UPDATE catalog_sets SET list_order=200 WHERE list_order=1",
    ] {
        let catalog = Catalog::new()?;
        let snapshot = catalog.import("game-completeness", THREE_GAMES)?;
        let first = games_for_snapshot(
            &catalog.database,
            &snapshot,
            None,
            NoIntroDatabasePageLimit::new(1)?,
        )?;
        corrupt_temporary_owner(&catalog, "catalog_sets", statement)?;
        assert!(
            games_for_snapshot(
                &catalog.database,
                &snapshot,
                None,
                NoIntroDatabasePageLimit::new(3)?
            )
            .is_err(),
            "complete page accepted {statement}"
        );
        assert!(
            games_for_snapshot(
                &catalog.database,
                &snapshot,
                first.next_cursor.as_ref(),
                NoIntroDatabasePageLimit::new(1)?
            )
            .is_err(),
            "continuation accepted {statement}"
        );
    }
    Ok(())
}

#[test]
fn headers_validate_saved_counts_and_dense_child_orders() -> TestResult {
    for statement in [
        "DELETE FROM no_intro_header_fields WHERE source_order=1",
        "DELETE FROM no_intro_header_fields WHERE source_order=2",
        "UPDATE no_intro_header_fields SET source_order=200 WHERE source_order=2",
    ] {
        let catalog = Catalog::new()?;
        let snapshot = catalog.import(
            "header-completeness",
            "<datafile><header><author>a</author><version>v</version><url>u</url></header></datafile>",
        )?;
        games_for_snapshot(
            &catalog.database,
            &snapshot,
            None,
            NoIntroDatabasePageLimit::new(1)?,
        )?;
        corrupt_temporary_owner(&catalog, "no_intro_header_fields", statement)?;
        assert!(
            games_for_snapshot(
                &catalog.database,
                &snapshot,
                None,
                NoIntroDatabasePageLimit::new(1)?
            )
            .is_err(),
            "header query accepted {statement}"
        );
    }
    Ok(())
}

#[test]
fn missing_empty_file_owners_do_not_hide_their_shared_occurrences() -> TestResult {
    for table in ["no_intro_dump_files", "no_intro_release_files"] {
        let catalog = Catalog::new()?;
        let snapshot = catalog.import(
            "empty-files",
            "<datafile><game name='empty'><source><file/></source><release><file/></release></game></datafile>",
        )?;
        games_for_snapshot(
            &catalog.database,
            &snapshot,
            None,
            NoIntroDatabasePageLimit::new(1)?,
        )?;
        corrupt_temporary_owner(&catalog, table, &format!("DELETE FROM {table}"))?;
        assert!(
            games_for_snapshot(
                &catalog.database,
                &snapshot,
                None,
                NoIntroDatabasePageLimit::new(1)?
            )
            .is_err(),
            "{table} must not hide its surviving shared occurrence"
        );
    }
    Ok(())
}

#[test]
fn file_reference_claim_kind_agrees_with_its_shared_occurrence() -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import("file-kind", OWNER_DOCUMENT)?;
    games_for_snapshot(
        &catalog.database,
        &snapshot,
        None,
        NoIntroDatabasePageLimit::new(1)?,
    )?;
    corrupt_temporary_owner(
        &catalog,
        "asset_occurrences",
        "UPDATE asset_occurrences SET claim_kind='logiqx_rom'",
    )?;
    assert!(
        games_for_snapshot(
            &catalog.database,
            &snapshot,
            None,
            NoIntroDatabasePageLimit::new(1)?
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn digest_rows_reject_dangling_dictionary_and_hidden_field_codes() -> TestResult {
    let document = "<datafile><game name='hashes'><source><file crc32='12345678'/></source><release><file crc32='12345678'/></release></game></datafile>";
    for table in [
        "no_intro_dump_file_digests",
        "no_intro_release_file_digests",
    ] {
        for statement in [
            format!("UPDATE {table} SET digest_id=999999999"),
            format!(
                "INSERT INTO {table}(occurrence_id,field_kind,digest_id,invalid_literal) SELECT occurrence_id,99,digest_id,invalid_literal FROM {table}"
            ),
            format!(
                "INSERT INTO {table}(occurrence_id,field_kind,digest_id,invalid_literal) SELECT occurrence_id,0.5,digest_id,invalid_literal FROM {table}"
            ),
        ] {
            assert_corruption_rejected(document, table, &statement)?;
        }
    }
    Ok(())
}

#[test]
fn nfo_checksums_reject_coerced_digest_storage_for_both_aliases() -> TestResult {
    for alias in ["nfocrc", "nfo_crc32"] {
        let document = format!(
            "<datafile><game name='nfo'><release><details {alias}='12345678'/></release></game></datafile>"
        );
        assert_corruption_rejected(
            &document,
            "digest_values",
            &format!(
                "UPDATE digest_values SET digest='abcd' WHERE digest_id IN (SELECT hash_id FROM no_intro_release_nfo_hashes WHERE source_hash_field='{alias}')"
            ),
        )?;
        let document = format!(
            "<datafile><game name='nfo'><release><details {alias}='bad'/></release></game></datafile>"
        );
        assert_corruption_rejected(
            &document,
            "no_intro_release_nfo_hashes",
            &format!(
                "UPDATE no_intro_release_nfo_hashes SET invalid_literal=x'626164' WHERE source_hash_field='{alias}'"
            ),
        )?;
    }
    Ok(())
}

#[test]
fn descendants_reject_mixed_child_gaps_collisions_and_reordered_file_traversal() -> TestResult {
    for (table, statement) in [
        (
            "no_intro_releases",
            "UPDATE no_intro_releases SET source_order=200",
        ),
        (
            "no_intro_dump_details",
            "UPDATE no_intro_dump_details SET source_order=1",
        ),
        (
            "no_intro_release_serials",
            "UPDATE no_intro_release_serials SET source_order=200",
        ),
        (
            "asset_occurrences",
            "UPDATE asset_occurrences SET occurrence_order=200 WHERE claim_kind='no_intro_database_release_file'",
        ),
    ] {
        assert_corruption_rejected(OWNER_DOCUMENT, table, statement)?;
    }
    Ok(())
}

#[test]
fn file_owner_closure_starts_from_games_even_without_surviving_histories() -> TestResult {
    for (table, document) in [
        (
            "no_intro_dump_sources",
            "<datafile><game name='g'><source><file/></source></game></datafile>",
        ),
        (
            "no_intro_releases",
            "<datafile><game name='g'><release><file/></release></game></datafile>",
        ),
    ] {
        assert_corruption_rejected(document, table, &format!("DELETE FROM {table}"))?;
    }
    for (table, column, owners) in [
        (
            "no_intro_dump_files",
            "dump_source_id",
            "no_intro_dump_sources",
        ),
        ("no_intro_release_files", "release_id", "no_intro_releases"),
    ] {
        assert_corruption_rejected(
            "<datafile><game name='a'><source><file/></source><release><file/></release></game><game name='b'><source/><release/></game></datafile>",
            table,
            &format!("UPDATE {table} SET {column}=(SELECT MAX({column}) FROM {owners})"),
        )?;
    }
    Ok(())
}

#[test]
fn native_files_cannot_outlive_both_occurrences_and_history_owners() -> TestResult {
    for (table, document) in [
        (
            "no_intro_dump_sources",
            "<datafile><game name='g'><source><file/></source></game></datafile>",
        ),
        (
            "no_intro_releases",
            "<datafile><game name='g'><release><file/></release></game></datafile>",
        ),
    ] {
        let catalog = Catalog::new()?;
        let snapshot = catalog.import("native-file-anchor", document)?;
        games_for_snapshot(
            &catalog.database,
            &snapshot,
            None,
            NoIntroDatabasePageLimit::new(1)?,
        )?;
        corrupt_temporary_owner(
            &catalog,
            "asset_occurrences",
            "DELETE FROM asset_occurrences",
        )?;
        corrupt_temporary_owner(&catalog, table, &format!("DELETE FROM {table}"))?;
        assert!(
            games_for_snapshot(
                &catalog.database,
                &snapshot,
                None,
                NoIntroDatabasePageLimit::new(1)?
            )
            .is_err(),
            "{table} must not hide a native file's surviving game anchor"
        );
    }
    Ok(())
}

#[test]
fn archive_relationship_ids_require_their_reported_registry_provenance() -> TestResult {
    let document =
        "<datafile><game name='links'><archive clone='0012' mergeof='literal'/></game></datafile>";
    for (table, statement) in [
        (
            "no_intro_archive_clone_links",
            "UPDATE no_intro_archive_clone_links SET relationship_id=999999999",
        ),
        (
            "no_intro_archive_merge_links",
            "UPDATE no_intro_archive_merge_links SET relationship_id=999999999",
        ),
        (
            "catalog_relationships",
            "UPDATE catalog_relationships SET snapshot_key='wrong-snapshot'",
        ),
        (
            "catalog_relationships",
            "UPDATE catalog_relationships SET origin='reviewed'",
        ),
        (
            "reported_catalog_relationships",
            "UPDATE reported_catalog_relationships SET source_reference_kind='logiqx_cloneof'",
        ),
    ] {
        assert_corruption_rejected(document, table, statement)?;
    }
    Ok(())
}

fn assert_corruption_rejected(document: &str, table: &str, statement: &str) -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import("corrupt-native-owner", document)?;
    games_for_snapshot(
        &catalog.database,
        &snapshot,
        None,
        NoIntroDatabasePageLimit::new(1)?,
    )?;
    corrupt_temporary_owner(&catalog, table, statement)?;
    assert!(
        games_for_snapshot(
            &catalog.database,
            &snapshot,
            None,
            NoIntroDatabasePageLimit::new(1)?
        )
        .is_err(),
        "native query accepted {statement}"
    );
    Ok(())
}

#[test]
fn pagination_keeps_duplicate_game_owners_and_rejects_cross_snapshot_cursors() -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import("paged", THREE_GAMES)?;
    let first = games_for_snapshot(
        &catalog.database,
        &snapshot,
        None,
        NoIntroDatabasePageLimit::new(1)?,
    )?;
    assert_eq!(first.games.len(), 1);
    let cursor = first.next_cursor.as_ref().ok_or("missing continuation")?;
    let second = games_for_snapshot(
        &catalog.database,
        &snapshot,
        Some(cursor),
        NoIntroDatabasePageLimit::new(1)?,
    )?;
    assert_eq!(second.games.len(), 1);
    let last = games_for_snapshot(
        &catalog.database,
        &snapshot,
        second.next_cursor.as_ref(),
        NoIntroDatabasePageLimit::new(1)?,
    )?;
    assert_eq!(last.games.len(), 1);
    assert!(last.next_cursor.is_none());
    assert_ne!(first.games[0].id, second.games[0].id);
    assert_ne!(second.games[0].id, last.games[0].id);

    let other = catalog.import("other", THREE_GAMES)?;
    assert!(
        games_for_snapshot(
            &catalog.database,
            &other,
            Some(cursor),
            NoIntroDatabasePageLimit::new(1)?
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn rebuilding_an_equivalent_snapshot_does_not_reuse_a_cursor_registry() -> TestResult {
    let original = Catalog::new()?;
    let snapshot = original.import("same-key", THREE_GAMES)?;
    let page = games_for_snapshot(
        &original.database,
        &snapshot,
        None,
        NoIntroDatabasePageLimit::new(1)?,
    )?;
    let cursor = page.next_cursor.as_ref().ok_or("missing cursor")?;
    let rebuilt = Catalog::new()?;
    let rebuilt_snapshot = rebuilt.import("same-key", THREE_GAMES)?;
    assert_eq!(rebuilt_snapshot, snapshot);
    assert!(
        games_for_snapshot(
            &rebuilt.database,
            &rebuilt_snapshot,
            Some(cursor),
            NoIntroDatabasePageLimit::new(1)?
        )
        .is_err()
    );
    assert_eq!(
        games_for_snapshot(
            &rebuilt.database,
            &rebuilt_snapshot,
            None,
            NoIntroDatabasePageLimit::new(1)?
        )?
        .games
        .len(),
        1
    );
    Ok(())
}

#[test]
fn continuation_requires_its_actual_native_game_anchor() -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import("anchor", THREE_GAMES)?;
    let first = games_for_snapshot(
        &catalog.database,
        &snapshot,
        None,
        NoIntroDatabasePageLimit::new(1)?,
    )?;
    let cursor = first.next_cursor.as_ref().ok_or("missing continuation")?;
    corrupt_temporary_owner(
        &catalog,
        "no_intro_database_games",
        &format!(
            "DELETE FROM no_intro_database_games WHERE set_id={}",
            first.games[0].id.as_i64()
        ),
    )?;
    assert!(
        games_for_snapshot(
            &catalog.database,
            &snapshot,
            Some(cursor),
            NoIntroDatabasePageLimit::new(1)?
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn paired_backup_preserves_pages_and_cursors_without_reopening_source_objects() -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import("backup", THREE_GAMES)?;
    let first = games_for_snapshot(
        &catalog.database,
        &snapshot,
        None,
        NoIntroDatabasePageLimit::new(1)?,
    )?;
    let cursor = first.next_cursor.as_ref().ok_or("missing cursor")?;
    let before = games_for_snapshot(
        &catalog.database,
        &snapshot,
        Some(cursor),
        NoIntroDatabasePageLimit::new(1)?,
    )?;
    let backup = Utf8PathBuf::try_from(catalog.directory.path().join("backup.sqlite"))?;
    let restored = Utf8PathBuf::try_from(catalog.directory.path().join("restored.sqlite"))?;
    mame_coalesce::create_backup(&catalog.path, &backup)?;
    mame_coalesce::restore_backup(&backup, &restored, RestorePolicy::CreateNew)?;
    let restored_database = Database::open(&restored)?;
    catalog.hide_sources("backup")?;
    std::fs::rename(
        format!("{restored}.documents"),
        catalog
            .directory
            .path()
            .join("restored-objects-unavailable"),
    )?;
    assert!(app::load_snapshot_source(&restored_database, &snapshot).is_err());
    let after = games_for_snapshot(
        &restored_database,
        &snapshot,
        Some(cursor),
        NoIntroDatabasePageLimit::new(1)?,
    )?;
    assert_eq!(after, before);
    Ok(())
}

#[test]
fn a_published_non_export_snapshot_is_not_a_native_export_page() -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import_format(
        "logiqx",
        "<datafile><game name='flat'/></datafile>",
        CatalogDocumentFormat::Logiqx(mame_coalesce::logiqx::LogiqxMode::ObservedCompatible),
    )?;
    assert!(
        games_for_snapshot(
            &catalog.database,
            &snapshot,
            None,
            NoIntroDatabasePageLimit::new(1)?
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn one_game_page_keeps_every_history_owner_across_descendant_batches() -> TestResult {
    let catalog = Catalog::new()?;
    let mut xml = String::from("<datafile><game name='many-histories'>");
    for order in 0..1_100 {
        write!(
            xml,
            "<source><details id='same-publisher-id' dumper='dumper-{order}'/><file id='same-file-id' size='1'/></source>"
        )?;
    }
    xml.push_str("</game></datafile>");
    let snapshot = catalog.import("descendant-batches", &xml)?;
    catalog.hide_sources("descendant-batches")?;
    assert!(app::load_snapshot_source(&catalog.database, &snapshot).is_err());
    let page = games_for_snapshot(
        &catalog.database,
        &snapshot,
        None,
        NoIntroDatabasePageLimit::new(1)?,
    )?;
    assert_eq!(page.games.len(), 1);
    assert!(page.next_cursor.is_none());
    let children = &page.games[0].children;
    assert_eq!(children.len(), 1_100);
    for (order, child) in children.iter().enumerate() {
        assert_eq!(child.source_order(), i64::try_from(order)?);
        let mame_coalesce::catalog_no_intro_database::NoIntroDatabaseGameChild::DumpSource(source) =
            child
        else {
            return Err("dump history changed its owner kind".into());
        };
        assert_eq!(
            source
                .details
                .as_ref()
                .and_then(|details| details.dumper.as_ref())
                .map(mame_coalesce::no_intro_db_xml::DeclaredText::as_str),
            Some(format!("dumper-{order}").as_str())
        );
        assert_eq!(source.files.len(), 1);
        assert_eq!(source.files[0].occurrence_order, i64::try_from(order)?);
    }
    Ok(())
}

#[test]
fn recovery_text_and_equal_nfo_aliases_remain_distinct_source_free_facts() -> TestResult {
    use mame_coalesce::{
        catalog_files::NoIntroDatabaseDigestValue,
        catalog_no_intro_database::{NoIntroDatabaseGameChild, NoIntroDatabaseReleaseDetailsField},
    };

    let catalog = Catalog::new()?;
    let xml = "<datafile><game name='recovered'><release><details comment='a\0b' nfo_crc32='12345678' nfocrc='12345678'/></release></game></datafile>";
    let snapshot = catalog.import_format(
        "recovery",
        xml,
        CatalogDocumentFormat::NoIntroDatabase(NoIntroDatabaseMode::NullRecoveryCompatible),
    )?;
    assert_eq!(
        app::load_snapshot_source(&catalog.database, &snapshot)?,
        xml.as_bytes()
    );
    catalog.hide_sources("recovery")?;
    assert!(app::load_snapshot_source(&catalog.database, &snapshot).is_err());
    let page = games_for_snapshot(
        &catalog.database,
        &snapshot,
        None,
        NoIntroDatabasePageLimit::new(1)?,
    )?;
    assert_eq!(
        page.document.mode,
        NoIntroDatabaseMode::NullRecoveryCompatible
    );
    let release = page.games[0]
        .children
        .iter()
        .find_map(|child| match child {
            NoIntroDatabaseGameChild::Release(release) => Some(release),
            _ => None,
        })
        .ok_or("missing recovered release")?;
    assert!(
        release.files.is_empty(),
        "NFO checksums must not invent ROM occurrences"
    );
    let details = release.details.as_ref().ok_or("missing release details")?;
    assert_eq!(details.comment.as_deref(), Some("a\u{fffd}b"));
    let expected = Some(NoIntroDatabaseDigestValue::Valid(hex::decode("12345678")?));
    assert_eq!(details.nfo_crc32, expected);
    assert_eq!(details.nfocrc, expected);
    assert_eq!(
        details
            .attribute_positions
            .iter()
            .map(|position| position.field)
            .collect::<Vec<_>>(),
        [
            NoIntroDatabaseReleaseDetailsField::Comment,
            NoIntroDatabaseReleaseDetailsField::NfoCrc32,
            NoIntroDatabaseReleaseDetailsField::NfoCrc
        ]
    );
    Ok(())
}
