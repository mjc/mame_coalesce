use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, connection::SimpleConnection,
    sql_query, sql_types::Text,
};
use mame_coalesce::{
    NoIntroDatMode, RestorePolicy,
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    catalog_files,
    catalog_no_intro_dat::{
        NoIntroDatGameChild, NoIntroDatPage, NoIntroDatPageLimit, NoIntroDatQueryError,
        games_for_snapshot,
    },
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey, SnapshotKey},
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const HEADER: &str = "<header><id>1</id><name>List</name><description/><version>1</version><author/><clrmamepro forcenodump='required'/><romcenter plugin='p'/></header>";
const GAME: &str = "<game name='same' id='0001' cloneof='parent' cloneofid='0002'><category/><description>Game</description><game_id>0003</game_id><rom name='a.bin' size='4' crc='AABBCCDD' md5='00112233445566778899aabbccddeeff' sha1='00112233445566778899aabbccddeeff00112233'/><release name='' region='JP'/></game>";

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
        let report = self.import_report(
            key,
            xml,
            CatalogDocumentFormat::NoIntroDat(NoIntroDatMode::V4Compatible),
        )?;
        assert_eq!(report.status, CatalogImportStatus::Succeeded);
        Ok(report.snapshot_key.ok_or("missing published snapshot")?)
    }

    fn import_report(
        &self,
        key: &str,
        xml: &str,
        format: CatalogDocumentFormat,
    ) -> TestResult<app::CatalogImportReport> {
        let input = Utf8PathBuf::try_from(self.directory.path().join(format!("{key}.xml")))?;
        std::fs::write(&input, xml)?;
        Ok(app::import_catalog(
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
        )?)
    }

    fn page(&self, snapshot: &SnapshotKey) -> TestResult<NoIntroDatPage> {
        Ok(games_for_snapshot(
            &self.database,
            snapshot,
            None,
            NoIntroDatPageLimit::new(1)?,
        )?)
    }

    fn corrupt(&self, table: &str, statement: &str) -> TestResult {
        let mut connection = SqliteConnection::establish(self.path.as_str())?;
        let guards =
            sql_query("SELECT name,sql FROM sqlite_schema WHERE type='trigger' AND tbl_name=?")
                .bind::<Text, _>(table)
                .load::<Guard>(&mut connection)?;
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
}

#[derive(QueryableByName)]
struct Guard {
    #[diesel(sql_type = Text)]
    name: String,
    #[diesel(sql_type = Text)]
    sql: String,
}

fn document(games: &str) -> String {
    format!("<datafile>{HEADER}{games}</datafile>")
}

fn rejects_corruption(table: &str, statement: &str, files_also: bool) -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import("corruption", &document(GAME))?;
    let before = catalog.page(&snapshot)?;
    let ids = before.games[0]
        .children
        .iter()
        .filter_map(|child| match child {
            NoIntroDatGameChild::Rom(rom) => Some(rom.occurrence_id),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        catalog_files::occurrences_for_ids(&catalog.database, &ids)?.len(),
        1
    );
    catalog.corrupt(table, statement)?;
    assert!(
        catalog.page(&snapshot).is_err(),
        "native page accepted {statement}"
    );
    if files_also {
        assert!(
            catalog_files::occurrences_for_ids(&catalog.database, &ids).is_err(),
            "occurrence payload accepted {statement}"
        );
    }
    Ok(())
}

#[test]
fn fractional_native_positions_and_counts_are_not_truncated() -> TestResult {
    for (table, column) in [
        ("no_intro_dat_documents", "source_column"),
        ("no_intro_dat_headers", "source_order"),
        ("no_intro_dat_clrmamepro_options", "source_column"),
        ("no_intro_dat_romcenter_options", "source_order"),
        ("no_intro_dat_games", "source_order"),
        ("no_intro_dat_categories", "category_order"),
        ("no_intro_dat_identifiers", "identifier_order"),
        ("no_intro_dat_releases", "name_source_column"),
        ("no_intro_dat_rom_claims", "source_order"),
        ("no_intro_dat_header_field_positions", "source_order"),
        ("no_intro_dat_clrmamepro_field_positions", "source_column"),
        ("no_intro_dat_romcenter_field_positions", "source_column"),
        ("no_intro_dat_game_field_positions", "source_order"),
        ("no_intro_dat_rom_field_positions", "source_order"),
        ("no_intro_dat_parse_counts", "game_count"),
        ("catalog_sets", "list_order"),
        ("asset_occurrences", "occurrence_order"),
    ] {
        rejects_corruption(
            table,
            &format!("UPDATE {table} SET {column}={column}+0.5"),
            false,
        )?;
    }
    Ok(())
}

#[test]
fn every_sealed_count_requires_nonnegative_integer_storage() -> TestResult {
    for column in [
        "game_count",
        "rom_count",
        "category_count",
        "identifier_count",
        "release_count",
        "clrmamepro_option_count",
        "romcenter_option_count",
        "header_field_count",
        "clrmamepro_field_count",
        "romcenter_field_count",
        "game_field_count",
        "rom_field_count",
    ] {
        for value in ["0.5", "-1"] {
            rejects_corruption(
                "no_intro_dat_parse_counts",
                &format!("UPDATE no_intro_dat_parse_counts SET {column}={value}"),
                false,
            )?;
        }
    }
    Ok(())
}

#[test]
fn blob_metadata_cannot_masquerade_as_declared_text() -> TestResult {
    for (table, column) in [
        ("no_intro_dat_headers", "name"),
        ("no_intro_dat_documents", "schema_location"),
        ("catalog_sets", "set_name"),
        ("no_intro_dat_games", "id_text"),
        ("no_intro_dat_categories", "category"),
        ("no_intro_dat_identifiers", "identifier"),
        ("no_intro_dat_releases", "name"),
        ("no_intro_dat_rom_claims", "name"),
        ("no_intro_dat_clrmamepro_options", "forcenodump_text"),
        ("no_intro_dat_romcenter_options", "plugin_text"),
        ("no_intro_dat_set_links", "target_literal"),
    ] {
        rejects_corruption(
            table,
            &format!("UPDATE {table} SET {column}=X'6162'"),
            table == "no_intro_dat_rom_claims",
        )?;
    }
    Ok(())
}

#[test]
fn source_and_catalog_display_names_require_text_storage() -> TestResult {
    for table in ["publishing_sources", "catalogs"] {
        rejects_corruption(
            table,
            &format!("UPDATE {table} SET display_name=X'6162'"),
            false,
        )?;
    }
    rejects_corruption(
        "catalog_set_groups",
        "UPDATE catalog_set_groups SET kind=X'726F6F74'",
        false,
    )
}

#[test]
fn required_document_header_counts_and_root_are_not_omitted() -> TestResult {
    for table in [
        "no_intro_dat_documents",
        "no_intro_dat_headers",
        "no_intro_dat_parse_counts",
        "catalog_set_groups",
        "no_intro_dat_games",
        "no_intro_dat_rom_claims",
    ] {
        rejects_corruption(table, &format!("DELETE FROM {table}"), false)?;
    }
    Ok(())
}

#[test]
fn positions_must_match_present_values_in_both_directions() -> TestResult {
    for (table, sql) in [
        (
            "no_intro_dat_header_field_positions",
            "DELETE FROM no_intro_dat_header_field_positions WHERE field_kind=1",
        ),
        (
            "no_intro_dat_headers",
            "UPDATE no_intro_dat_headers SET name=NULL",
        ),
        (
            "no_intro_dat_game_field_positions",
            "DELETE FROM no_intro_dat_game_field_positions WHERE field_kind=1",
        ),
        (
            "no_intro_dat_games",
            "UPDATE no_intro_dat_games SET id_text=NULL",
        ),
        (
            "no_intro_dat_rom_field_positions",
            "DELETE FROM no_intro_dat_rom_field_positions WHERE field_kind=1",
        ),
        (
            "no_intro_dat_rom_claims",
            "UPDATE no_intro_dat_rom_claims SET size_text=NULL",
        ),
    ] {
        rejects_corruption(table, sql, table.contains("rom_"))?;
    }
    Ok(())
}

#[test]
fn removed_directive_owners_leave_detectable_orphan_positions() -> TestResult {
    for table in [
        "no_intro_dat_clrmamepro_options",
        "no_intro_dat_romcenter_options",
    ] {
        rejects_corruption(table, &format!("DELETE FROM {table}"), false)?;
    }
    Ok(())
}

#[test]
fn duplicate_raw_mixed_child_orders_are_rejected() -> TestResult {
    rejects_corruption(
        "no_intro_dat_identifiers",
        "UPDATE no_intro_dat_identifiers SET source_order=0",
        false,
    )?;
    rejects_corruption(
        "no_intro_dat_romcenter_options",
        "UPDATE no_intro_dat_romcenter_options SET source_order=5",
        false,
    )
}

#[test]
fn family_orders_and_terminal_game_count_are_checked() -> TestResult {
    for (table, column) in [
        ("no_intro_dat_categories", "category_order"),
        ("no_intro_dat_identifiers", "identifier_order"),
        ("no_intro_dat_releases", "release_order"),
        ("asset_occurrences", "occurrence_order"),
    ] {
        rejects_corruption(table, &format!("UPDATE {table} SET {column}=2"), false)?;
    }
    rejects_corruption(
        "no_intro_dat_parse_counts",
        "UPDATE no_intro_dat_parse_counts SET game_count=2",
        false,
    )
}

#[test]
fn parent_links_require_actual_reported_registry_provenance() -> TestResult {
    for (table, sql) in [
        (
            "reported_catalog_relationships",
            "UPDATE reported_catalog_relationships SET source_reference_kind='no_intro_dat_cloneofid' WHERE source_reference_kind='no_intro_dat_cloneof'",
        ),
        (
            "catalog_relationships",
            "UPDATE catalog_relationships SET origin='reviewed',snapshot_key=NULL",
        ),
        (
            "no_intro_dat_set_links",
            "UPDATE no_intro_dat_set_links SET relationship_id=999999 WHERE link_kind='cloneof'",
        ),
        (
            "no_intro_dat_set_links",
            "DELETE FROM no_intro_dat_set_links WHERE link_kind='cloneof'",
        ),
    ] {
        rejects_corruption(table, sql, false)?;
    }
    Ok(())
}

#[test]
fn parent_discriminators_require_text_storage() -> TestResult {
    for (table, column) in [
        ("no_intro_dat_set_links", "link_kind"),
        ("reported_catalog_relationships", "source_reference_kind"),
        ("catalog_relationships", "origin"),
        ("catalog_relationships", "snapshot_key"),
    ] {
        rejects_corruption(
            table,
            &format!("UPDATE {table} SET {column}=CAST({column} AS BLOB)"),
            false,
        )?;
    }
    Ok(())
}

#[test]
fn unsupported_uuid_links_are_rejected_by_both_public_readers() -> TestResult {
    for hash in [
        "",
        " crc='12345678'",
        " md5='00112233445566778899aabbccddeeff'",
    ] {
        let catalog = Catalog::new()?;
        let snapshot = catalog.import(
            "uuid-eligibility",
            &document(&format!(
                "<game name='identity'><description/><rom name='hashed' size='4' \
                 sha1='00112233445566778899aabbccddeeff00112233'/></game>\
                 <game name='unsupported'><description/><rom name='unhashed' size='4'{hash}/></game>"
            )),
        )?;
        let before = games_for_snapshot(
            &catalog.database,
            &snapshot,
            None,
            NoIntroDatPageLimit::new(2)?,
        )?;
        let ids = before
            .games
            .iter()
            .flat_map(|game| &game.children)
            .filter_map(|child| match child {
                NoIntroDatGameChild::Rom(rom) => Some(rom.occurrence_id),
                _ => None,
            })
            .collect::<Vec<_>>();
        let occurrences = catalog_files::occurrences_for_ids(&catalog.database, &ids)?;
        assert_eq!(occurrences.len(), 2);
        assert!(occurrences[0].content_id.is_some());
        assert!(occurrences[1].content_id.is_none());
        catalog.corrupt(
            "asset_occurrences",
            "UPDATE asset_occurrences SET content_uuid=(\
               SELECT content_uuid FROM asset_occurrences WHERE content_uuid IS NOT NULL LIMIT 1\
             ) WHERE content_uuid IS NULL",
        )?;
        assert!(
            games_for_snapshot(
                &catalog.database,
                &snapshot,
                None,
                NoIntroDatPageLimit::new(2)?,
            )
            .is_err(),
            "native page accepted a UUID without SHA-1/SHA-256 evidence"
        );
        assert!(
            catalog_files::occurrences_for_ids(&catalog.database, &ids).is_err(),
            "occurrence reader accepted a UUID without SHA-1/SHA-256 evidence"
        );
    }
    Ok(())
}

#[test]
fn cursor_anchor_must_follow_the_required_header() -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import("anchor-header", &document(&GAME.repeat(2)))?;
    let first = catalog.page(&snapshot)?;
    let cursor = first.next_cursor.as_ref().ok_or("missing cursor")?;
    assert!(
        games_for_snapshot(
            &catalog.database,
            &snapshot,
            Some(cursor),
            NoIntroDatPageLimit::new(1)?,
        )
        .is_ok()
    );
    catalog.corrupt(
        "no_intro_dat_games",
        &format!(
            "UPDATE no_intro_dat_games SET source_order=0 WHERE set_id={}",
            first.games[0].id.as_i64(),
        ),
    )?;
    assert!(
        games_for_snapshot(
            &catalog.database,
            &snapshot,
            Some(cursor),
            NoIntroDatPageLimit::new(1)?,
        )
        .is_err(),
        "continuation accepted an anchor colliding with the header"
    );
    Ok(())
}

#[test]
fn all_digest_rows_are_checked_not_only_known_per_kind_joins() -> TestResult {
    for sql in [
        "UPDATE no_intro_dat_rom_digest_fields SET digest_id=999999 WHERE field_kind=2",
        "UPDATE no_intro_dat_rom_digest_fields SET field_kind=99 WHERE field_kind=2",
        "UPDATE no_intro_dat_rom_digest_fields SET field_kind=2.5 WHERE field_kind=2",
        "UPDATE no_intro_dat_rom_digest_fields SET invalid_text='bad' WHERE field_kind=2",
        "UPDATE no_intro_dat_rom_digest_fields SET digest_id=NULL,invalid_text=NULL WHERE field_kind=2",
    ] {
        rejects_corruption("no_intro_dat_rom_digest_fields", sql, true)?;
    }
    Ok(())
}

#[test]
fn digest_dictionary_algorithm_storage_and_length_are_checked() -> TestResult {
    for sql in [
        "UPDATE digest_values SET digest='aabbccdd' WHERE algorithm='crc32'",
        "UPDATE digest_values SET digest=X'AA' WHERE algorithm='crc32'",
        "UPDATE digest_values SET algorithm='unknown' WHERE algorithm='crc32'",
    ] {
        rejects_corruption("digest_values", sql, true)?;
    }
    Ok(())
}

#[test]
fn native_payload_scope_and_assertion_provenance_cannot_be_relabelled() -> TestResult {
    rejects_corruption(
        "no_intro_dat_rom_claims",
        "UPDATE no_intro_dat_rom_claims SET evidence_scope='unknown'",
        true,
    )?;
    rejects_corruption(
        "occurrence_digest_assertions",
        "UPDATE occurrence_digest_assertions SET provenance='computed'",
        true,
    )
}

#[test]
fn pagination_keeps_duplicate_names_distinct_and_rejects_other_snapshots() -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import("pages", &document(&GAME.repeat(3)))?;
    let other = catalog.import("other", &document(GAME))?;
    let first = catalog.page(&snapshot)?;
    let cursor = first.next_cursor.as_ref().ok_or("missing cursor")?;
    let second = games_for_snapshot(
        &catalog.database,
        &snapshot,
        Some(cursor),
        NoIntroDatPageLimit::new(1)?,
    )?;
    let third = games_for_snapshot(
        &catalog.database,
        &snapshot,
        second.next_cursor.as_ref(),
        NoIntroDatPageLimit::new(1)?,
    )?;
    assert_eq!(first.games[0].list_order, 0);
    assert_eq!(second.games[0].list_order, 1);
    assert_eq!(third.games[0].list_order, 2);
    assert_ne!(first.games[0].id, second.games[0].id);
    assert!(third.next_cursor.is_none());
    assert!(matches!(
        games_for_snapshot(
            &catalog.database,
            &other,
            Some(cursor),
            NoIntroDatPageLimit::new(1)?
        ),
        Err(NoIntroDatQueryError::CursorSnapshotMismatch)
    ));
    Ok(())
}

#[test]
fn occurrence_order_restarts_per_game_not_per_snapshot() -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import("multiple-games", &document(&GAME.repeat(3)))?;
    let page = games_for_snapshot(
        &catalog.database,
        &snapshot,
        None,
        NoIntroDatPageLimit::new(500)?,
    )?;
    assert_eq!(page.games.len(), 3);
    let roms = page
        .games
        .iter()
        .map(|game| {
            game.children
                .iter()
                .find_map(|child| match child {
                    NoIntroDatGameChild::Rom(rom) => Some(rom),
                    _ => None,
                })
                .ok_or("game missing ROM")
        })
        .collect::<Result<Vec<_>, _>>()?;
    assert!(roms.iter().all(|rom| rom.occurrence_order == 0));
    assert_ne!(roms[0].occurrence_id, roms[1].occurrence_id);
    assert_ne!(roms[1].occurrence_id, roms[2].occurrence_id);
    Ok(())
}

#[test]
fn selected_games_cannot_lose_required_description_or_all_roms() -> TestResult {
    for erased in ["description", "roms"] {
        let catalog = Catalog::new()?;
        let snapshot = catalog.import("required-game-facts", &document(&GAME.repeat(2)))?;
        catalog.page(&snapshot)?;
        if erased == "description" {
            catalog.corrupt(
                "no_intro_dat_games",
                "UPDATE no_intro_dat_games SET description_text=NULL",
            )?;
            catalog.corrupt(
                "no_intro_dat_game_field_positions",
                "DELETE FROM no_intro_dat_game_field_positions WHERE field_kind=4",
            )?;
        } else {
            catalog.corrupt("asset_occurrences", "DELETE FROM asset_occurrences")?;
        }
        assert!(
            catalog.page(&snapshot).is_err(),
            "partial selected game lost required {erased}"
        );
    }
    Ok(())
}

#[test]
fn description_keeps_its_place_after_earlier_categories() -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import("mixed-order", &document(GAME))?;
    let page = catalog.page(&snapshot)?;
    let game = page.games.first().ok_or("missing game")?;
    assert_eq!(
        game.children
            .iter()
            .map(NoIntroDatGameChild::source_order)
            .collect::<Vec<_>>(),
        [0, 1, 2, 3, 4]
    );
    assert!(matches!(
        game.children.first(),
        Some(NoIntroDatGameChild::Category(_))
    ));
    Ok(())
}

#[test]
fn a_missing_last_native_rom_is_not_hidden_on_a_partial_game_page() -> TestResult {
    let catalog = Catalog::new()?;
    let first_game = GAME.replace("<release", "<rom name='last.bin'/><release");
    let snapshot = catalog.import(
        "missing-last-rom",
        &document(&format!("{first_game}{GAME}")),
    )?;
    let page = catalog.page(&snapshot)?;
    let game = page.games.first().ok_or("missing game")?;
    assert_eq!(
        game.children
            .iter()
            .filter(|child| matches!(child, NoIntroDatGameChild::Rom(_)))
            .count(),
        2
    );
    catalog.corrupt("no_intro_dat_rom_claims", &format!("DELETE FROM no_intro_dat_rom_claims WHERE occurrence_id=(SELECT MAX(occurrence_id) FROM asset_occurrences WHERE record_id={})", game.id.as_i64()))?;
    assert!(
        catalog.page(&snapshot).is_err(),
        "missing native ROM was silently omitted"
    );
    Ok(())
}

#[test]
fn strict_v3_and_v4_source_free_pages_preserve_native_children() -> TestResult {
    for mode in [NoIntroDatMode::V3Strict, NoIntroDatMode::V4Strict] {
        let catalog = Catalog::new()?;
        let xml = format!(
            "<datafile>{HEADER}<game name='g' id='0001' cloneof='parent' cloneofid='0002'><category/><description>Game</description><rom name='a.bin' size='0004' crc='AABBCCDD' md5='00112233445566778899aabbccddeeff' sha1='00112233445566778899aabbccddeeff00112233'/><release name='' region='JP'/></game></datafile>"
        );
        let report =
            catalog.import_report("strict-page", &xml, CatalogDocumentFormat::NoIntroDat(mode))?;
        assert_eq!(report.status, CatalogImportStatus::Succeeded);
        let snapshot = report.snapshot_key.ok_or("missing snapshot")?;
        std::fs::rename(
            catalog.directory.path().join("strict-page.xml"),
            catalog.directory.path().join("input-unavailable"),
        )?;
        std::fs::rename(
            format!("{}.documents", catalog.path),
            catalog.directory.path().join("objects-unavailable"),
        )?;
        let page = catalog.page(&snapshot)?;
        assert_eq!(page.document.mode, mode);
        let game = page.games.first().ok_or("missing strict game")?;
        assert_eq!(
            game.children
                .iter()
                .map(NoIntroDatGameChild::source_order)
                .collect::<Vec<_>>(),
            [0, 1, 2, 3]
        );
        let rom = game
            .children
            .iter()
            .find_map(|child| match child {
                NoIntroDatGameChild::Rom(rom) => Some(rom),
                _ => None,
            })
            .ok_or("missing strict ROM")?;
        assert_eq!(rom.occurrence_order, 0);
        assert_eq!(rom.payload.size_text.as_deref(), Some("0004"));
        assert_eq!(rom.payload.size, Some(4));
        assert_eq!(
            rom.payload.evidence_scope,
            catalog_files::NoIntroDatEvidenceScope::WholeFile
        );
    }
    Ok(())
}

#[test]
fn continuation_requires_actual_native_anchor() -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import("anchor", &document(&GAME.repeat(2)))?;
    let first = catalog.page(&snapshot)?;
    let cursor = first.next_cursor.as_ref().ok_or("missing cursor")?;
    catalog.corrupt(
        "no_intro_dat_games",
        &format!(
            "DELETE FROM no_intro_dat_games WHERE set_id={}",
            first.games[0].id.as_i64()
        ),
    )?;
    assert!(
        games_for_snapshot(
            &catalog.database,
            &snapshot,
            Some(cursor),
            NoIntroDatPageLimit::new(1)?
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn fresh_registry_rejects_a_cursor_even_for_identical_import_bytes() -> TestResult {
    let catalog = Catalog::new()?;
    let xml = document(&GAME.repeat(2));
    let snapshot = catalog.import("registry", &xml)?;
    let page = catalog.page(&snapshot)?;
    let fresh = Catalog::new()?;
    let fresh_snapshot = fresh.import("registry", &xml)?;
    assert_eq!(snapshot, fresh_snapshot);
    assert!(matches!(
        games_for_snapshot(
            &fresh.database,
            &fresh_snapshot,
            page.next_cursor.as_ref(),
            NoIntroDatPageLimit::new(1)?
        ),
        Err(NoIntroDatQueryError::CursorRegistryMismatch)
    ));
    Ok(())
}

#[test]
fn paired_backup_preserves_cursor_and_native_page_without_sources() -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import("backup", &document(&GAME.repeat(2)))?;
    let first = catalog.page(&snapshot)?;
    let cursor = first.next_cursor.as_ref().ok_or("missing cursor")?;
    let before = games_for_snapshot(
        &catalog.database,
        &snapshot,
        Some(cursor),
        NoIntroDatPageLimit::new(1)?,
    )?;
    let backup = Utf8PathBuf::try_from(catalog.directory.path().join("backup.sqlite"))?;
    let restored = Utf8PathBuf::try_from(catalog.directory.path().join("restored.sqlite"))?;
    mame_coalesce::create_backup(&catalog.path, &backup)?;
    mame_coalesce::restore_backup(&backup, &restored, RestorePolicy::CreateNew)?;
    let database = Database::open(&restored)?;
    std::fs::rename(
        format!("{restored}.documents"),
        catalog.directory.path().join("objects-unavailable"),
    )?;
    std::fs::rename(
        catalog.directory.path().join("backup.xml"),
        catalog.directory.path().join("input-unavailable"),
    )?;
    assert!(app::load_snapshot_source(&database, &snapshot).is_err());
    let after = games_for_snapshot(
        &database,
        &snapshot,
        Some(cursor),
        NoIntroDatPageLimit::new(1)?,
    )?;
    assert_eq!(after, before);
    Ok(())
}

#[test]
fn wrong_published_format_is_not_a_flat_dat_page() -> TestResult {
    let catalog = Catalog::new()?;
    let report = catalog.import_report(
        "logiqx",
        "<datafile><game name='flat'/></datafile>",
        CatalogDocumentFormat::Logiqx(mame_coalesce::logiqx::LogiqxMode::ObservedCompatible),
    )?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    let snapshot = report.snapshot_key.ok_or("missing logiqx snapshot")?;
    assert!(matches!(
        games_for_snapshot(
            &catalog.database,
            &snapshot,
            None,
            NoIntroDatPageLimit::new(1)?
        ),
        Err(NoIntroDatQueryError::NotPublished(_))
    ));
    Ok(())
}

#[test]
fn unpublished_native_snapshot_cannot_be_queried() -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import("unpublished", &document(GAME))?;
    catalog.page(&snapshot)?;
    catalog.corrupt("snapshot_publications", "DELETE FROM snapshot_publications")?;
    assert!(matches!(
        games_for_snapshot(
            &catalog.database,
            &snapshot,
            None,
            NoIntroDatPageLimit::new(1)?
        ),
        Err(NoIntroDatQueryError::NotPublished(_))
    ));
    Ok(())
}

#[test]
fn another_group_cannot_hide_behind_the_flat_root_filter() -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import("groups", &document(GAME))?;
    catalog.page(&snapshot)?;
    catalog.corrupt("catalog_set_groups", &format!("INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) VALUES ('{}','software_list',0)", snapshot.as_str()))?;
    assert!(catalog.page(&snapshot).is_err());
    Ok(())
}

#[test]
fn late_invalid_input_never_publishes_a_partial_native_page() -> TestResult {
    let catalog = Catalog::new()?;
    let good = catalog.import("good", &document(GAME))?;
    let before = catalog.page(&good)?;
    let report = catalog.import_report(
        "bad",
        &format!("{}<broken", document(GAME)),
        CatalogDocumentFormat::NoIntroDat(NoIntroDatMode::V4Compatible),
    )?;
    assert_eq!(report.status, CatalogImportStatus::Failed);
    assert!(report.snapshot_key.is_none());
    assert_eq!(catalog.page(&good)?, before);
    Ok(())
}
