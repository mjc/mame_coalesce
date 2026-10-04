use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, connection::SimpleConnection,
    sql_query, sql_types::Text,
};
use mame_coalesce::{
    RestorePolicy,
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    catalog_clrmamepro::{
        ClrMameProPage, ClrMameProPageLimit, ClrMameProQueryError, sets_for_snapshot,
    },
    catalog_files::{ClrMameProFilePayload, occurrences_for_ids},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey, SnapshotKey},
    logiqx::LogiqxMode,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn checked<T>(values: &[T], index: usize) -> TestResult<&T> {
    values.get(index).ok_or_else(|| {
        format!(
            "missing fixture item at index {index} ({} items)",
            values.len()
        )
        .into()
    })
}

const HEADER: &str = "clrmamepro ( name Catalog version v1 forcenodump required )";
const SET: &str = "game ( name set-a description Description year 1984 sample intro.wav rom ( name a.bin size 8 ) )";
const CMP: &str = "; first comment\nclrmamepro ( name Catalog version v1 forcenodump required )\n; second comment\n game ( name set-a description Description year 1984 cloneof parent sample intro.wav rom ( name a.bin size 8 ) )";

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
        contents: &str,
        format: CatalogDocumentFormat,
    ) -> TestResult<app::CatalogImportReport> {
        let input = Utf8PathBuf::try_from(self.directory.path().join(format!("{key}.dat")))?;
        std::fs::write(&input, contents)?;
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

    fn import_cmp(&self, key: &str, contents: &str) -> TestResult<SnapshotKey> {
        let report = self.import(key, contents, CatalogDocumentFormat::ClrMamePro)?;
        assert_eq!(report.status, CatalogImportStatus::Succeeded);
        Ok(report
            .snapshot_key
            .ok_or("missing published CMP snapshot")?)
    }

    fn hide_sources(&self, keys: &[&str]) -> TestResult {
        for key in keys {
            std::fs::rename(
                self.directory.path().join(format!("{key}.dat")),
                self.directory.path().join(format!("{key}.unavailable")),
            )?;
        }
        let objects = Utf8PathBuf::from(format!("{}.documents", self.path));
        if objects.exists() {
            std::fs::rename(objects, self.directory.path().join("objects-unavailable"))?;
        }
        Ok(())
    }

    fn page(
        &self,
        snapshot: &SnapshotKey,
        cursor: Option<&mame_coalesce::catalog_clrmamepro::ClrMameProCursor>,
        limit: usize,
    ) -> Result<ClrMameProPage, ClrMameProQueryError> {
        sets_for_snapshot(
            &self.database,
            snapshot,
            cursor,
            ClrMameProPageLimit::new(limit)?,
        )
    }

    fn corrupt(&self, table: &str, statement: &str) -> TestResult {
        let mut connection = SqliteConnection::establish(self.path.as_str())?;
        let guards = sql_query(
            "SELECT name,sql FROM sqlite_schema WHERE type='trigger' AND tbl_name=? ORDER BY name",
        )
        .bind::<Text, _>(table)
        .load::<Guard>(&mut connection)?;
        if guards.is_empty() {
            return Err(format!("no triggers found for corruption target {table}").into());
        }
        connection.batch_execute("PRAGMA ignore_check_constraints=ON; PRAGMA foreign_keys=OFF;")?;
        for guard in &guards {
            connection.batch_execute(&format!("DROP TRIGGER {}", guard.name))?;
        }
        let mutation = connection.batch_execute(statement);
        for guard in &guards {
            connection.batch_execute(&guard.sql)?;
        }
        connection.batch_execute("PRAGMA ignore_check_constraints=OFF; PRAGMA foreign_keys=ON;")?;
        mutation?;
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

fn quoted_snapshot(snapshot: &SnapshotKey) -> String {
    format!("'{}'", snapshot.as_str().replace('\'', "''"))
}

fn rejects_corruption(table: &str, mutation: &str, fixture: &str) -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import_cmp("corrupt", fixture)?;
    let control = catalog.import_cmp("control", &format!("{HEADER} {SET}"))?;
    catalog.hide_sources(&["corrupt", "control"])?;
    catalog.corrupt(
        table,
        &mutation.replace("__SNAPSHOT_KEY__", &quoted_snapshot(&snapshot)),
    )?;
    assert!(
        catalog.page(&snapshot, None, 20).is_err(),
        "CMP reader accepted corruption: {mutation}"
    );
    assert_eq!(catalog.page(&control, None, 20)?.sets.len(), 1);
    Ok(())
}

#[test]
fn page_limit_checks_bounds() {
    assert!(ClrMameProPageLimit::new(0).is_err());
    assert!(ClrMameProPageLimit::new(1).is_ok());
    assert!(ClrMameProPageLimit::new(500).is_ok());
    assert!(ClrMameProPageLimit::new(501).is_err());
    assert!(ClrMameProPageLimit::new(usize::MAX).is_err());
}

#[test]
fn public_import_pages_sets_after_source_and_objects_are_unavailable() -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import_cmp("source-free", CMP)?;
    catalog.hide_sources(&["source-free"])?;
    let page = catalog.page(&snapshot, None, 10)?;
    assert_eq!(page.snapshot.snapshot_key, snapshot);
    assert_eq!(page.sets.len(), 1);
    assert!(page.next_cursor.is_none());
    Ok(())
}

#[test]
fn continuation_pages_keep_duplicate_names_distinct_and_reject_another_snapshot() -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import_cmp(
        "pages",
        &format!("{HEADER} game ( name same ) game ( name same ) game ( name same )"),
    )?;
    let other = catalog.import_cmp("other", &format!("{HEADER} game ( name other )"))?;
    catalog.hide_sources(&["pages", "other"])?;
    let first = catalog.page(&snapshot, None, 1)?;
    let cursor = first.next_cursor.as_ref().ok_or("missing first cursor")?;
    let second = catalog.page(&snapshot, Some(cursor), 1)?;
    let third = catalog.page(&snapshot, second.next_cursor.as_ref(), 1)?;
    assert_eq!(checked(&first.sets, 0)?.list_order, 0);
    assert_eq!(checked(&second.sets, 0)?.list_order, 1);
    assert_eq!(checked(&third.sets, 0)?.list_order, 2);
    assert_ne!(checked(&first.sets, 0)?.id, checked(&second.sets, 0)?.id);
    assert_ne!(checked(&second.sets, 0)?.id, checked(&third.sets, 0)?.id);
    assert!(third.next_cursor.is_none());
    assert!(matches!(
        catalog.page(&other, Some(cursor), 1),
        Err(ClrMameProQueryError::CursorSnapshotMismatch)
    ));
    Ok(())
}

#[test]
fn continuation_requires_its_actual_native_set_anchor() -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import_cmp("anchor", &format!("{HEADER} {SET} {SET}"))?;
    let control = catalog.import_cmp("anchor-control", &format!("{HEADER} {SET}"))?;
    catalog.hide_sources(&["anchor", "anchor-control"])?;
    let first = catalog.page(&snapshot, None, 1)?;
    let cursor = first.next_cursor.as_ref().ok_or("missing cursor")?;
    catalog.corrupt(
        "catalog_sets",
        &format!(
            "DELETE FROM catalog_sets WHERE set_id={}",
            checked(&first.sets, 0)?.id.as_i64()
        ),
    )?;
    assert!(catalog.page(&snapshot, Some(cursor), 1).is_err());
    assert_eq!(catalog.page(&control, None, 10)?.sets.len(), 1);
    Ok(())
}

#[test]
fn equivalent_snapshot_in_a_fresh_registry_rejects_a_cursor() -> TestResult {
    let first_catalog = Catalog::new()?;
    let source = format!("{HEADER} {SET} {SET}");
    let snapshot = first_catalog.import_cmp("same", &source)?;
    let first = first_catalog.page(&snapshot, None, 1)?;
    let cursor = first.next_cursor.as_ref().ok_or("missing cursor")?;
    first_catalog.hide_sources(&["same"])?;

    let second_catalog = Catalog::new()?;
    let other_snapshot = second_catalog.import_cmp("same", &source)?;
    second_catalog.hide_sources(&["same"])?;
    assert_eq!(snapshot, other_snapshot);
    assert!(matches!(
        second_catalog.page(&other_snapshot, Some(cursor), 1),
        Err(ClrMameProQueryError::CursorRegistryMismatch)
    ));
    Ok(())
}

#[test]
fn paired_backup_keeps_source_free_continuation_pages() -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import_cmp("backup", &format!("{HEADER} {SET} {SET}"))?;
    let first = catalog.page(&snapshot, None, 1)?;
    let cursor = first.next_cursor.as_ref().ok_or("missing cursor")?;
    let before = catalog.page(&snapshot, Some(cursor), 1)?;
    let backup = Utf8PathBuf::try_from(catalog.directory.path().join("backup.sqlite"))?;
    let restored = Utf8PathBuf::try_from(catalog.directory.path().join("restored.sqlite"))?;
    mame_coalesce::create_backup(&catalog.path, &backup)?;
    mame_coalesce::restore_backup(&backup, &restored, RestorePolicy::CreateNew)?;
    let database = Database::open(&restored)?;
    std::fs::rename(
        format!("{restored}.documents"),
        catalog
            .directory
            .path()
            .join("restored-objects-unavailable"),
    )?;
    catalog.hide_sources(&["backup"])?;
    let after = sets_for_snapshot(
        &database,
        &snapshot,
        Some(cursor),
        ClrMameProPageLimit::new(1)?,
    )?;
    assert_eq!(checked(&after.sets, 0)?.id, checked(&before.sets, 0)?.id);
    assert_eq!(
        checked(&after.sets, 0)?.list_order,
        checked(&before.sets, 0)?.list_order
    );
    assert_eq!(after.next_cursor, before.next_cursor);
    Ok(())
}

#[test]
fn pages_accept_header_before_between_and_after_sets() -> TestResult {
    let catalog = Catalog::new()?;
    let before = catalog.import_cmp("header-before", &format!("{HEADER} {SET}"))?;
    let between = catalog.import_cmp("header-between", &format!("{SET} {HEADER} {SET}"))?;
    let after = catalog.import_cmp("header-after", &format!("{SET} {HEADER}"))?;
    catalog.hide_sources(&["header-before", "header-between", "header-after"])?;
    let before_page = catalog.page(&before, None, 10)?;
    let between_page = catalog.page(&between, None, 10)?;
    let after_page = catalog.page(&after, None, 10)?;
    for page in [&before_page, &between_page, &after_page] {
        assert!(page.document.header_present);
        assert!(page.document.header.is_some());
        assert!(!page.sets.is_empty());
    }
    let before_order = before_page
        .document
        .header
        .as_ref()
        .ok_or("missing header")?
        .source_order;
    assert!(before_order < checked(&before_page.sets, 0)?.source_order);
    let between_order = between_page
        .document
        .header
        .as_ref()
        .ok_or("missing header")?
        .source_order;
    assert!(checked(&between_page.sets, 0)?.source_order < between_order);
    assert!(between_order < checked(&between_page.sets, 1)?.source_order);
    let after_order = after_page
        .document
        .header
        .as_ref()
        .ok_or("missing header")?
        .source_order;
    assert!(checked(&after_page.sets, 0)?.source_order < after_order);
    Ok(())
}

#[test]
fn document_comments_and_metadata_are_read_from_native_facts() -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import_cmp("comments", CMP)?;
    catalog.hide_sources(&["comments"])?;
    let page = catalog.page(&snapshot, None, 10)?;
    assert!(page.document.header_present);
    assert_eq!(page.document.comments.len(), 2);
    assert_eq!(page.snapshot.declared_version.as_deref(), Some("v1"));
    Ok(())
}

#[test]
fn scalar_sample_and_rom_children_preserve_their_mixed_order() -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import_cmp(
        "mixed-order",
        &format!(
            "{HEADER} game ( name set sample first.wav rom ( name a.bin ) year 1984 sample second.wav rom ( name b.bin ) )"
        ),
    )?;
    catalog.hide_sources(&["mixed-order"])?;
    let page = catalog.page(&snapshot, None, 10)?;
    assert_eq!(page.sets.len(), 1);
    let children = &checked(&page.sets, 0)?.children;
    let media = children
        .iter()
        .filter_map(|child| match child {
            mame_coalesce::catalog_clrmamepro::ClrMameProSetChild::Rom(_) => Some("rom"),
            mame_coalesce::catalog_clrmamepro::ClrMameProSetChild::Sample(_) => Some("sample"),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(media, ["sample", "rom", "sample", "rom"]);
    let orders = children
        .iter()
        .map(mame_coalesce::catalog_clrmamepro::ClrMameProSetChild::source_order)
        .collect::<Vec<_>>();
    assert!(
        orders
            .windows(2)
            .all(|pair| matches!(pair, [first, second] if first < second))
    );
    Ok(())
}

#[test]
fn set_media_references_match_source_free_bulk_file_payloads() -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import_cmp(
        "bulk-payload",
        &format!("{HEADER} game ( name set sample intro.wav rom ( name native.bin size 8 ) )"),
    )?;
    catalog.hide_sources(&["bulk-payload"])?;
    let page = catalog.page(&snapshot, None, 10)?;
    let children = &checked(&page.sets, 0)?.children;
    let declared = children
        .iter()
        .filter_map(|child| match child {
            mame_coalesce::catalog_clrmamepro::ClrMameProSetChild::Rom(rom) => {
                Some((rom.occurrence_id, rom.payload.name.clone()))
            }
            mame_coalesce::catalog_clrmamepro::ClrMameProSetChild::Sample(sample) => {
                Some((sample.occurrence_id, sample.payload.name.clone()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let occurrence_ids = declared
        .iter()
        .map(|(occurrence_id, _)| *occurrence_id)
        .collect::<Vec<_>>();
    let files = occurrences_for_ids(&catalog.database, &occurrence_ids)?;
    assert_eq!(files.len(), 2);
    for file in files {
        let Some(payload) = file.clrmamepro_file else {
            return Err("bulk file result omitted CMP payload".into());
        };
        let name = match payload {
            ClrMameProFilePayload::Rom(rom) => rom.name,
            ClrMameProFilePayload::Sample(sample) => sample.name,
        };
        assert!(declared.contains(&(file.occurrence_id, name)));
    }
    Ok(())
}

#[test]
fn source_free_set_without_header_has_no_invented_document_metadata() -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import_cmp("headerless", "set ( name empty )")?;
    catalog.hide_sources(&["headerless"])?;
    let page = catalog.page(&snapshot, None, 10)?;
    assert!(!page.document.header_present);
    assert!(page.document.header.is_none());
    assert!(page.document.comments.is_empty());
    assert_eq!(page.document.comment_count, 0);
    assert_eq!(page.sets.len(), 1);
    let [mame_coalesce::catalog_clrmamepro::ClrMameProSetChild::Field { field, value }] =
        checked(&page.sets, 0)?.children.as_slice()
    else {
        return Err("name-only set must contain exactly its declared name, without media".into());
    };
    assert_eq!(
        *field,
        mame_coalesce::catalog_clrmamepro::ClrMameProSetField::Name
    );
    assert_eq!(value.value, "empty");
    Ok(())
}

#[test]
fn wrong_published_format_is_not_a_clrmamepro_page() -> TestResult {
    let catalog = Catalog::new()?;
    let report = catalog.import(
        "wrong-format",
        "<datafile><game name='not-cmp'/></datafile>",
        CatalogDocumentFormat::Logiqx(LogiqxMode::ObservedCompatible),
    )?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    let snapshot = report.snapshot_key.ok_or("missing Logiqx snapshot")?;
    catalog.hide_sources(&["wrong-format"])?;
    assert!(matches!(
        catalog.page(&snapshot, None, 10),
        Err(ClrMameProQueryError::NotPublished(_))
    ));
    Ok(())
}

#[test]
fn unpublished_native_snapshot_cannot_be_queried() -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import_cmp("unpublished", &format!("{HEADER} {SET}"))?;
    let control = catalog.import_cmp("published-control", &format!("{HEADER} {SET}"))?;
    catalog.hide_sources(&["unpublished", "published-control"])?;
    catalog.corrupt(
        "snapshot_publications",
        &format!(
            "DELETE FROM snapshot_publications WHERE snapshot_key={}",
            quoted_snapshot(&snapshot)
        ),
    )?;
    assert!(matches!(
        catalog.page(&snapshot, None, 10),
        Err(ClrMameProQueryError::NotPublished(_))
    ));
    assert_eq!(catalog.page(&control, None, 10)?.sets.len(), 1);
    Ok(())
}

#[test]
fn missing_document_and_header_owners_are_rejected() -> TestResult {
    for (table, mutation) in [
        (
            "cmp_documents",
            "DELETE FROM cmp_documents WHERE snapshot_key=__SNAPSHOT_KEY__",
        ),
        (
            "cmp_header_facts",
            "DELETE FROM cmp_header_facts WHERE snapshot_key=__SNAPSHOT_KEY__",
        ),
    ] {
        rejects_corruption(table, mutation, &format!("{HEADER} {SET}"))?;
    }
    Ok(())
}

#[test]
fn media_free_pages_require_a_binary_registry_generation() -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import_cmp("empty-registry-control", "set ( name empty )")?;
    catalog.hide_sources(&["empty-registry-control"])?;
    assert!(catalog.page(&snapshot, None, 1).is_ok());
    for value in [
        "'abcdefghijklmnop'",
        "1234567890123456",
        "1234567890123456.0",
        "zeroblob(0)",
        "zeroblob(15)",
        "zeroblob(17)",
    ] {
        catalog.corrupt(
            "file_id_registries",
            &format!("UPDATE file_id_registries SET registry_uuid={value} WHERE registry_id=1"),
        )?;
        assert!(
            matches!(
                catalog.page(&snapshot, None, 1),
                Err(ClrMameProQueryError::Registry(_))
            ),
            "invalid registry generation {value} must be rejected even without media"
        );
        catalog.corrupt(
            "file_id_registries",
            "UPDATE file_id_registries SET registry_uuid=zeroblob(16) WHERE registry_id=1",
        )?;
        assert!(catalog.page(&snapshot, None, 1).is_ok());
    }
    Ok(())
}

#[test]
fn missing_field_positions_comments_and_orphan_positions_are_rejected() -> TestResult {
    for (table, mutation) in [
        (
            "cmp_set_field_positions",
            "DELETE FROM cmp_set_field_positions WHERE record_id=(SELECT set_id FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) WHERE snapshot_key=__SNAPSHOT_KEY__) AND field_kind=2",
        ),
        (
            "cmp_comments",
            "DELETE FROM cmp_comments WHERE snapshot_key=__SNAPSHOT_KEY__ AND comment_order=0",
        ),
        (
            "cmp_set_facts",
            "DELETE FROM cmp_set_facts WHERE record_id=(SELECT set_id FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) WHERE snapshot_key=__SNAPSHOT_KEY__)",
        ),
    ] {
        rejects_corruption(table, mutation, CMP)?;
    }
    rejects_corruption(
        "cmp_documents",
        "UPDATE cmp_documents SET comment_count=comment_count+1 WHERE snapshot_key=__SNAPSHOT_KEY__",
        CMP,
    )?;
    Ok(())
}

#[test]
fn fractional_and_blob_native_orders_and_counts_are_rejected() -> TestResult {
    for (table, column, value) in [
        ("cmp_documents", "comment_count", "comment_count+0.5"),
        (
            "cmp_header_field_positions",
            "source_order",
            "source_order+0.5",
        ),
        (
            "cmp_set_field_positions",
            "source_order",
            "source_order+0.5",
        ),
        ("cmp_set_rom_positions", "source_order", "source_order+0.5"),
        ("cmp_comments", "comment_order", "comment_order+0.5"),
        ("cmp_documents", "comment_count", "X'01'"),
        ("cmp_header_facts", "source_order", "source_order+0.5"),
        ("cmp_header_facts", "source_column", "X'01'"),
        ("cmp_header_field_positions", "source_line", "X'01'"),
        ("cmp_set_field_positions", "source_column", "X'01'"),
        ("cmp_set_rom_positions", "source_order", "X'01'"),
        ("cmp_set_facts", "document_order", "document_order+0.5"),
        ("catalog_sets", "list_order", "list_order+0.5"),
        ("catalog_sets", "list_order", "X'01'"),
    ] {
        let catalog = Catalog::new()?;
        let snapshot = catalog.import_cmp("bad-storage", CMP)?;
        let control = catalog.import_cmp("storage-control", &format!("{HEADER} {SET}"))?;
        catalog.hide_sources(&["bad-storage", "storage-control"])?;
        let key = quoted_snapshot(&snapshot);
        let predicate = match table {
            "cmp_documents"
            | "cmp_comments"
            | "cmp_header_field_positions"
            | "cmp_header_facts" => {
                format!("snapshot_key={key}")
            }
            "cmp_set_field_positions" | "cmp_set_facts" => format!(
                "record_id IN (SELECT set_id FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) WHERE snapshot_key={key})"
            ),
            "cmp_set_rom_positions" => format!(
                "occurrence_id IN (SELECT occurrence_id FROM asset_occurrences JOIN catalog_sets ON set_id=record_id JOIN catalog_set_groups USING(set_group_id) WHERE snapshot_key={key})"
            ),
            "catalog_sets" => format!(
                "set_id IN (SELECT set_id FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) WHERE snapshot_key={key})"
            ),
            _ => return Err(format!("unknown corruption fixture table {table:?}").into()),
        };
        catalog.corrupt(
            table,
            &format!("UPDATE {table} SET {column}={value} WHERE {predicate}"),
        )?;
        assert!(
            catalog.page(&snapshot, None, 10).is_err(),
            "accepted {table}.{column}={value}"
        );
        assert_eq!(catalog.page(&control, None, 10)?.sets.len(), 1);
    }
    Ok(())
}

#[test]
fn wrong_set_kind_and_unexpected_group_are_rejected() -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import_cmp("wrong-kind", &format!("{HEADER} {SET}"))?;
    let control = catalog.import_cmp("wrong-kind-control", &format!("{HEADER} {SET}"))?;
    catalog.hide_sources(&["wrong-kind", "wrong-kind-control"])?;
    let key = quoted_snapshot(&snapshot);
    catalog.corrupt(
        "catalog_sets",
        &format!("UPDATE catalog_sets SET source_element_kind='logiqx_game' WHERE set_group_id IN (SELECT set_group_id FROM catalog_set_groups WHERE snapshot_key={key})"),
    )?;
    assert!(catalog.page(&snapshot, None, 10).is_err());
    assert_eq!(catalog.page(&control, None, 10)?.sets.len(), 1);

    let groups = Catalog::new()?;
    let snapshot = groups.import_cmp("extra-group", &format!("{HEADER} {SET}"))?;
    let control = groups.import_cmp("extra-group-control", &format!("{HEADER} {SET}"))?;
    groups.hide_sources(&["extra-group", "extra-group-control"])?;
    groups.corrupt(
        "catalog_set_groups",
        &format!("INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) VALUES ({},'software_list',0)", quoted_snapshot(&snapshot)),
    )?;
    assert!(groups.page(&snapshot, None, 10).is_err());
    assert_eq!(groups.page(&control, None, 10)?.sets.len(), 1);
    Ok(())
}

#[test]
fn missing_reported_parent_registry_entry_is_rejected() -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import_cmp(
        "parent",
        &format!("{HEADER} game ( name child cloneof parent )"),
    )?;
    let control = catalog.import_cmp("parent-control", &format!("{HEADER} {SET}"))?;
    catalog.hide_sources(&["parent", "parent-control"])?;
    let page = catalog.page(&snapshot, None, 10)?;
    let relationship_id = checked(&page.sets, 0)?
        .children
        .iter()
        .find_map(|child| match child {
            mame_coalesce::catalog_clrmamepro::ClrMameProSetChild::Parent(parent)
                if parent.kind
                    == mame_coalesce::catalog_clrmamepro::ClrMameProParentKind::CloneOf
                    && parent.target.value == "parent" =>
            {
                Some(parent.relationship_id.database_value())
            }
            _ => None,
        })
        .ok_or("missing parent reference")?;
    assert!(relationship_id > 0);
    let key = quoted_snapshot(&snapshot);
    catalog.corrupt(
        "reported_catalog_relationships",
        &format!("DELETE FROM reported_catalog_relationships WHERE relationship_id IN (SELECT link.relationship_id FROM clrmamepro_set_links AS link JOIN catalog_sets ON catalog_sets.set_id=link.set_id JOIN catalog_set_groups USING(set_group_id) WHERE snapshot_key={key})"),
    )?;
    assert!(catalog.page(&snapshot, None, 10).is_err());
    assert_eq!(catalog.page(&control, None, 10)?.sets.len(), 1);
    Ok(())
}

#[test]
fn late_invalid_input_does_not_publish_a_partial_snapshot_or_change_control_page() -> TestResult {
    let catalog = Catalog::new()?;
    let good = catalog.import_cmp("good", &format!("{HEADER} {SET}"))?;
    catalog.hide_sources(&["good"])?;
    let before = catalog.page(&good, None, 10)?;
    let report = catalog.import(
        "late-invalid",
        &format!("{HEADER} {SET} game ( name broken rom ( name bad.bin size nope )"),
        CatalogDocumentFormat::ClrMamePro,
    )?;
    assert_eq!(report.status, CatalogImportStatus::Failed);
    assert!(report.snapshot_key.is_none());
    let after = catalog.page(&good, None, 10)?;
    assert_eq!(checked(&after.sets, 0)?.id, checked(&before.sets, 0)?.id);
    assert_eq!(after.next_cursor, before.next_cursor);
    Ok(())
}
