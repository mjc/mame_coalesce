#![allow(clippy::expect_used)]

use diesel::{
    QueryableByName, RunQueryDsl, sql_query,
    sql_types::{BigInt, Binary, Bool, Text},
};

use super::*;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
}

#[derive(QueryableByName)]
struct TempTableRow {
    #[diesel(sql_type = Bool)]
    present: bool,
}

#[derive(QueryableByName)]
struct ExplainRow {
    #[diesel(sql_type = Text)]
    detail: String,
}

#[derive(QueryableByName)]
struct DigestIdRow {
    #[diesel(sql_type = BigInt)]
    digest_id: i64,
}

fn pool() -> crate::Result<Pool> {
    super::super::db::create_db_pool(":memory:")
}

fn import_document(
    pool: &Pool,
    directory: &tempfile::TempDir,
    name: &str,
    format: crate::app::CatalogDocumentFormat,
    contents: &str,
) -> crate::Result<()> {
    import_document_with_identity(
        pool,
        directory,
        name,
        format,
        contents,
        &format!("source-{name}"),
        &format!("catalog-{name}"),
    )
}

fn import_document_with_identity(
    pool: &Pool,
    directory: &tempfile::TempDir,
    name: &str,
    format: crate::app::CatalogDocumentFormat,
    contents: &str,
    source_key: &str,
    catalog_key: &str,
) -> crate::Result<()> {
    use crate::{
        app::{CatalogImportRequest, CatalogImportStatus},
        domain::{CatalogKey, CatalogScope, PublishingSourceKey},
    };

    let path = directory.path().join(name);
    std::fs::write(&path, contents)?;
    let path = camino::Utf8PathBuf::from_path_buf(path).map_err(|path| {
        crate::Error::InvalidPath(format!("test path is not UTF-8: {}", path.display()))
    })?;
    let request = CatalogImportRequest {
        document_path: path,
        format,
        source_key: PublishingSourceKey::new(source_key),
        source_display_name: format!("Source {source_key}"),
        catalog_key: CatalogKey::new(catalog_key),
        catalog_display_name: format!("Catalog {catalog_key}"),
        scope: CatalogScope::Unknown,
    };
    let report = crate::storage::catalog_import::import(pool, &request)?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    Ok(())
}

fn occurrence_ids(pool: &Pool) -> crate::Result<Vec<OccurrenceId>> {
    let mut connection = pool.get()?;
    Ok(
        sql_query("SELECT occurrence_id FROM asset_occurrences ORDER BY occurrence_id")
            .load::<IdRow>(&mut connection)?
            .into_iter()
            .map(|row| OccurrenceId::from_database(row.occurrence_id))
            .collect(),
    )
}

fn request_temp_table_exists(pool: &Pool) -> crate::Result<bool> {
    let mut connection = pool.get()?;
    Ok(sql_query(
        "SELECT EXISTS (SELECT 1 FROM sqlite_temp_schema \
         WHERE type = 'table' AND name = 'catalog_files_requested_occurrences') AS present",
    )
    .get_result::<TempTableRow>(&mut connection)?
    .present)
}

fn logiqx_document() -> &'static str {
    r#"<datafile>
  <header><name>root</name></header>
  <game name="same">
    <rom name="repeated.bin" size="4" crc="352441c2" md5="900150983cd24fb0d6963f7d28e17f72" sha1="a9993e364706816aba3e25717850c26c9cd0d89d"/>
  </game>
  <game name="same">
    <rom name="repeated.bin" size="4" crc="352441c2" md5="900150983cd24fb0d6963f7d28e17f72" sha1="a9993e364706816aba3e25717850c26c9cd0d89d"/>
  </game>
  <game name="sample-owner"><sample name="click"/></game>
</datafile>"#
}

#[test]
fn cmp_samples_have_distinct_bulk_owners_in_native_media_order() -> TestResult {
    let pool = pool()?;
    let directory = tempfile::tempdir()?;
    import_document(
        &pool,
        &directory,
        "samples.dat",
        crate::app::CatalogDocumentFormat::ClrMamePro,
        r#"GAME (
  NAME repeated
  SAMPLE "click.wav"
  ROM ( NAME file.bin SIZE 4 SHA1 a9993e364706816aba3e25717850c26c9cd0d89d )
  sample click.wav
  SaMpLe ""
)
set ( name repeated sample click.wav )"#,
    )?;
    let ids = occurrence_ids(&pool)?;
    assert_eq!(ids.len(), 5, "every scalar sample needs a media owner");
    let entries = occurrences_for_ids_in_pool(&pool, &ids)?;
    assert_eq!(entries.len(), 5);
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.provenance.occurrence_kind)
            .collect::<Vec<_>>(),
        [
            OccurrenceKind::ClrMameProSample,
            OccurrenceKind::ClrMameProRom,
            OccurrenceKind::ClrMameProSample,
            OccurrenceKind::ClrMameProSample,
            OccurrenceKind::ClrMameProSample
        ]
    );
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.provenance.asset_name.as_deref())
            .collect::<Vec<_>>(),
        [
            Some("click.wav"),
            Some("file.bin"),
            Some("click.wav"),
            Some(""),
            Some("click.wav")
        ]
    );
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.provenance.occurrence_order)
            .collect::<Vec<_>>(),
        [0, 1, 2, 3, 0]
    );
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.provenance.native_occurrence_location)
            .collect::<Vec<_>>(),
        [(3, 10), (4, 3), (5, 10), (6, 10), (8, 28)]
            .map(|(line, column)| Some(SourceLocation { line, column }))
    );
    for entry in &entries {
        assert_eq!(entry.provenance.set_name, "repeated");
        assert_eq!(
            entry.provenance.source_element_kind,
            SourceElementKind::ClrMameProSet
        );
        assert!(entry.provenance.native_occurrence_location.is_some());
        if entry.provenance.occurrence_kind == OccurrenceKind::ClrMameProSample {
            assert!(entry.content_id.is_none());
            assert!(entry.digests.is_empty());
        }
    }
    assert_ne!(entries[0].provenance.set_id, entries[4].provenance.set_id);
    let content = entries[1]
        .content_id
        .expect("ROM has eligible whole-file evidence");
    let page =
        occurrences_for_content_in_pool(&pool, content, ContentOccurrenceLimit::new(10)?, None)?;
    assert_eq!(
        page.occurrences.len(),
        1,
        "filename-only samples never join UUID membership"
    );
    assert_eq!(page.occurrences[0].occurrence_id, ids[1]);
    Ok(())
}

fn add_computed_digest_children(pool: &Pool, ids: &[OccurrenceId]) -> TestResult {
    let mut connection = pool.get()?;
    sql_query("INSERT OR IGNORE INTO digest_values (algorithm, digest) VALUES ('sha256', ?)")
        .bind::<Binary, _>([0x5a; 32].as_slice())
        .execute(&mut connection)?;
    sql_query(
        "INSERT INTO occurrence_digest_assertions \
             (occurrence_id, digest_id, scope, provenance) \
             SELECT ?, digest_id, 'part:main', 'computed' \
             FROM digest_values WHERE algorithm = 'sha256' AND digest = ?",
    )
    .bind::<BigInt, _>(ids[0].database_value())
    .bind::<Binary, _>([0x5a; 32].as_slice())
    .execute(&mut connection)?;
    sql_query(
        "INSERT INTO occurrence_digest_assertions \
             (occurrence_id, digest_id, scope, provenance) \
             SELECT ?, digest_id, 'computed:whole', 'computed' \
             FROM digest_values WHERE algorithm = 'sha256' AND digest = ?",
    )
    .bind::<BigInt, _>(ids[0].database_value())
    .bind::<Binary, _>([0x5a; 32].as_slice())
    .execute(&mut connection)?;
    sql_query("INSERT OR IGNORE INTO digest_values (algorithm, digest) VALUES ('crc32', ?)")
        .bind::<Binary, _>([0x01, 0x02, 0x03, 0x04].as_slice())
        .execute(&mut connection)?;
    sql_query(
        "INSERT INTO occurrence_digest_assertions \
             (occurrence_id, digest_id, scope, provenance) \
             SELECT ?, digest_id, 'sample:click', 'computed' \
             FROM digest_values WHERE algorithm = 'crc32' AND digest = ?",
    )
    .bind::<BigInt, _>(ids[2].database_value())
    .bind::<Binary, _>([0x01, 0x02, 0x03, 0x04].as_slice())
    .execute(&mut connection)?;
    Ok(())
}

#[test]
fn bulk_keeps_repeated_and_unlinked_owners_and_digests_as_children() -> TestResult {
    let pool = pool()?;
    let directory = tempfile::tempdir()?;
    import_document(
        &pool,
        &directory,
        "root.dat",
        crate::app::CatalogDocumentFormat::Logiqx,
        logiqx_document(),
    )?;
    let ids = occurrence_ids(&pool)?;
    assert_eq!(ids.len(), 3);
    add_computed_digest_children(&pool, &ids)?;

    assert!(!request_temp_table_exists(&pool)?);
    let occurrences = occurrences_for_ids_in_pool(&pool, &[ids[2], ids[0], ids[1], ids[0]])?;
    assert!(!request_temp_table_exists(&pool)?);
    assert_eq!(occurrences.len(), 3);
    assert_eq!(
        occurrences
            .iter()
            .map(|occurrence| occurrence.provenance.set_name.as_str())
            .collect::<Vec<_>>(),
        ["same", "same", "sample-owner"]
    );
    assert!(occurrences[0].content_id.is_some());
    assert!(occurrences[1].content_id.is_some());
    assert_eq!(occurrences[2].content_id, None);
    assert_eq!(occurrences[0].provenance.set_group_kind, SetGroupKind::Root);
    assert_eq!(
        occurrences[0].provenance.asset_name.as_deref(),
        Some("repeated.bin")
    );
    assert_ne!(
        occurrences[0].provenance.set_location.line,
        occurrences[0]
            .provenance
            .native_occurrence_location
            .ok_or("missing root occurrence location")?
            .line
    );
    assert_eq!(
        occurrences[2].provenance.asset_name.as_deref(),
        Some("click")
    );
    assert!(!occurrences[0].digests.is_empty());
    assert!(
        occurrences[0]
            .digests
            .iter()
            .any(|digest| digest.scope == "part:main")
    );
    assert!(
        occurrences[0]
            .digests
            .iter()
            .any(|digest| digest.algorithm == DigestAlgorithm::Sha256)
    );
    assert!(occurrences[0].digests.iter().any(|digest| {
        digest.scope == "computed:whole" && digest.provenance == DigestProvenance::Computed
    }));
    assert!(
        occurrences[2]
            .digests
            .iter()
            .any(|digest| digest.scope == "sample:click")
    );
    assert!(occurrences[0].digests.windows(2).all(|pair| {
        (&pair[0].scope, pair[0].algorithm, &pair[0].value)
            <= (&pair[1].scope, pair[1].algorithm, &pair[1].value)
    }));
    Ok(())
}

#[test]
fn content_pages_are_keyset_ordered_and_cursor_is_uuid_bound() -> TestResult {
    let pool = pool()?;
    let directory = tempfile::tempdir()?;
    import_document(
        &pool,
        &directory,
        "root.dat",
        crate::app::CatalogDocumentFormat::Logiqx,
        logiqx_document(),
    )?;
    let ids = occurrence_ids(&pool)?;
    let owner_rows = occurrences_for_ids_in_pool(&pool, &ids)?;
    let content_id = owner_rows[0].content_id.ok_or_else(|| {
        crate::Error::DatabaseSchema("fixture occurrence was not linked to content".to_owned())
    })?;

    let limit = ContentOccurrenceLimit::new(1)?;
    assert!(!request_temp_table_exists(&pool)?);
    let first = occurrences_for_content_in_pool(&pool, content_id, limit, None)?;
    assert!(!request_temp_table_exists(&pool)?);
    assert_eq!(first.occurrences.len(), 1);
    let cursor = first
        .next_cursor
        .as_ref()
        .ok_or_else(|| crate::Error::DatabaseSchema("expected another page".to_owned()))?;
    let second = occurrences_for_content_in_pool(&pool, content_id, limit, Some(cursor))?;
    assert!(!request_temp_table_exists(&pool)?);
    assert_eq!(second.occurrences.len(), 1);
    assert!(second.next_cursor.is_none());
    assert_ne!(
        first.occurrences[0].occurrence_id,
        second.occurrences[0].occurrence_id
    );

    let other_id = CatalogContentId::from_bytes([0xA5; 16]);
    assert!(matches!(
        occurrences_for_content_in_pool(&pool, other_id, limit, Some(cursor)),
        Err(CatalogFilesError::CursorContentMismatch)
    ));
    let stale_registry_cursor = ContentOccurrenceCursor {
        content: content_id,
        registry: CatalogRegistryId::from_bytes([0xA5; 16]),
        after_occurrence: cursor.after_occurrence,
    };
    assert!(matches!(
        occurrences_for_content_in_pool(&pool, content_id, limit, Some(&stale_registry_cursor)),
        Err(CatalogFilesError::CursorRegistryMismatch)
    ));
    Ok(())
}

#[test]
fn cursor_cannot_be_reused_after_registry_rebuild() -> TestResult {
    let first_pool = pool()?;
    let first_directory = tempfile::tempdir()?;
    import_document(
        &first_pool,
        &first_directory,
        "root.dat",
        crate::app::CatalogDocumentFormat::Logiqx,
        logiqx_document(),
    )?;
    let first_ids = occurrence_ids(&first_pool)?;
    let content_id = occurrences_for_ids_in_pool(&first_pool, &first_ids)?[0]
        .content_id
        .ok_or("fixture occurrence was not linked")?;
    let first_page = occurrences_for_content_in_pool(
        &first_pool,
        content_id,
        ContentOccurrenceLimit::new(1)?,
        None,
    )?;
    let cursor = first_page
        .next_cursor
        .as_ref()
        .ok_or("expected cursor from first registry")?;

    let rebuilt_pool = pool()?;
    let rebuilt_directory = tempfile::tempdir()?;
    import_document(
        &rebuilt_pool,
        &rebuilt_directory,
        "root.dat",
        crate::app::CatalogDocumentFormat::Logiqx,
        logiqx_document(),
    )?;
    let rebuilt_ids = occurrence_ids(&rebuilt_pool)?;
    assert_ne!(
        occurrences_for_ids_in_pool(&rebuilt_pool, &rebuilt_ids)?[0].content_id,
        Some(content_id)
    );
    // Even an identical issued UUID in a different registry cannot validate a
    // cursor whose native occurrence IDs belong to the earlier generation.
    sql_query("INSERT INTO catalog_contents(content_uuid) VALUES (?)")
        .bind::<Binary, _>(content_id.as_bytes().as_slice())
        .execute(&mut rebuilt_pool.get()?)?;
    assert!(matches!(
        occurrences_for_content_in_pool(
            &rebuilt_pool,
            content_id,
            ContentOccurrenceLimit::new(1)?,
            Some(cursor)
        ),
        Err(CatalogFilesError::CursorRegistryMismatch)
    ));
    Ok(())
}

#[test]
fn empty_missing_and_invalid_page_requests_are_bounded() -> TestResult {
    let pool = pool()?;
    assert!(occurrences_for_ids_in_pool(&pool, &[])?.is_empty());
    assert!(
        occurrences_for_ids_in_pool(&pool, &[OccurrenceId::from_database(i64::MAX)])?.is_empty()
    );
    let oversized = (1..=i64::try_from(MAX_BULK_OCCURRENCES + 1)?)
        .map(OccurrenceId::from_database)
        .collect::<Vec<_>>();
    assert!(matches!(
        occurrences_for_ids_in_pool(&pool, &oversized),
        Err(CatalogFilesError::BulkRequestTooLarge { .. })
    ));
    let more_than_sqlite_variable_limit = (1..=1_200)
        .map(|id| OccurrenceId::from_database(i64::from(id)))
        .collect::<Vec<_>>();
    assert!(occurrences_for_ids_in_pool(&pool, &more_than_sqlite_variable_limit)?.is_empty());
    assert!(matches!(
        ContentOccurrenceLimit::new(0),
        Err(CatalogFilesError::InvalidPageLimit { .. })
    ));
    assert!(matches!(
        ContentOccurrenceLimit::new(MAX_PAGE_SIZE + 1),
        Err(CatalogFilesError::InvalidPageLimit { .. })
    ));
    assert!(
        occurrences_for_content_in_pool(
            &pool,
            CatalogContentId::from_bytes([0xA5; 16]),
            ContentOccurrenceLimit::new(5)?,
            None
        )?
        .occurrences
        .is_empty()
    );
    Ok(())
}

#[test]
fn malformed_digest_lengths_are_rejected_by_bulk_and_page_queries() -> TestResult {
    let pool = pool()?;
    let directory = tempfile::tempdir()?;
    import_document(
        &pool,
        &directory,
        "root.dat",
        crate::app::CatalogDocumentFormat::Logiqx,
        logiqx_document(),
    )?;
    let ids = occurrence_ids(&pool)?;
    let linked = occurrences_for_ids_in_pool(&pool, &ids)?
        .into_iter()
        .find(|occurrence| occurrence.content_id.is_some())
        .ok_or("fixture has no linked owner")?;
    let content_id = linked.content_id.ok_or("fixture has no content ID")?;
    {
        let mut connection = pool.get()?;
        sql_query("PRAGMA ignore_check_constraints = ON").execute(&mut connection)?;
        let digest_id = sql_query(
            "INSERT INTO digest_values (algorithm, digest) VALUES ('sha1', ?) RETURNING digest_id",
        )
        .bind::<Binary, _>([0x11; 19].as_slice())
        .get_result::<DigestIdRow>(&mut connection)?
        .digest_id;
        sql_query(
            "INSERT INTO occurrence_digest_assertions (occurrence_id, digest_id, scope, provenance) \
             VALUES (?, ?, 'corrupt-test', 'computed')",
        )
        .bind::<BigInt, _>(linked.occurrence_id.database_value())
        .bind::<BigInt, _>(digest_id)
        .execute(&mut connection)?;
        sql_query("PRAGMA ignore_check_constraints = OFF").execute(&mut connection)?;
    }

    let expected_error = |result: Result<Vec<CatalogFileOccurrence>, CatalogFilesError>| {
        matches!(
            result,
            Err(CatalogFilesError::DigestLengthMismatch {
                algorithm: DigestAlgorithm::Sha1,
                actual: 19,
                expected: 20,
                ..
            })
        )
    };
    assert!(expected_error(occurrences_for_ids_in_pool(
        &pool,
        &[linked.occurrence_id]
    )));
    assert!(!request_temp_table_exists(&pool)?);
    let page =
        occurrences_for_content_in_pool(&pool, content_id, ContentOccurrenceLimit::new(5)?, None);
    assert!(matches!(
        page,
        Err(CatalogFilesError::DigestLengthMismatch {
            algorithm: DigestAlgorithm::Sha1,
            actual: 19,
            expected: 20,
            ..
        })
    ));
    assert!(!request_temp_table_exists(&pool)?);
    Ok(())
}

#[test]
fn software_occurrences_keep_their_list_group_provenance() -> TestResult {
    let pool = pool()?;
    let directory = tempfile::tempdir()?;
    import_document(
        &pool,
        &directory,
        "software.xml",
        crate::app::CatalogDocumentFormat::MameSoftwareListXml,
        r#"<softwarelists><softwarelist name="cart-list"><software name="game">
           <description>Game</description><year>2000</year><publisher>Maker</publisher>
           <part name="cart" interface="cart">
             <dataarea name="shared" size="8">
               <rom name="game.bin" size="4" crc="352441c2" sha1="a9993e364706816aba3e25717850c26c9cd0d89d"/>
               <rom loadflag="continue" size="4" offset="4"/>
             </dataarea>
             <diskarea name="shared"><disk name="game.chd" sha1="a9993e364706816aba3e25717850c26c9cd0d89d"/></diskarea>
           </part>
           </software></softwarelist></softwarelists>"#,
    )?;
    let ids = occurrence_ids(&pool)?;
    assert_eq!(ids.len(), 3);
    let occurrences = occurrences_for_ids_in_pool(&pool, &ids)?;
    assert_eq!(
        occurrences[0].provenance.source_element_kind,
        SourceElementKind::SoftwareItem
    );
    assert_eq!(
        occurrences[0].provenance.set_group_kind,
        SetGroupKind::SoftwareList {
            name: "cart-list".to_owned()
        }
    );
    assert_eq!(occurrences[0].provenance.set_name, "game");
    assert_eq!(
        occurrences[0].provenance.asset_name.as_deref(),
        Some("game.bin")
    );
    let data_owner = occurrences[0]
        .provenance
        .software_owner
        .as_ref()
        .ok_or("missing software owner")?;
    assert_ne!(data_owner.part_id, 0);
    assert_eq!(data_owner.part_name, "cart");
    assert_ne!(data_owner.area_id, 0);
    assert_eq!(data_owner.part_order, 0);
    assert_eq!(data_owner.area_order, 0);
    assert_eq!(data_owner.area_name, "shared");
    assert_eq!(data_owner.area_kind, SoftwareAreaKind::Data);
    assert_eq!(
        occurrences[1].provenance.occurrence_kind,
        OccurrenceKind::SoftwareRomOperation
    );
    assert_eq!(occurrences[1].provenance.asset_name, None);
    assert!(
        occurrences[1]
            .provenance
            .native_occurrence_location
            .is_some()
    );
    assert_eq!(
        occurrences[1]
            .provenance
            .software_owner
            .as_ref()
            .expect("software occurrence owner")
            .area_name,
        "shared"
    );
    assert_eq!(
        occurrences[2].provenance.asset_name.as_deref(),
        Some("game.chd")
    );
    assert_eq!(
        occurrences[2]
            .provenance
            .software_owner
            .as_ref()
            .expect("software occurrence owner")
            .area_kind,
        SoftwareAreaKind::Disk
    );
    let disk_owner = occurrences[2]
        .provenance
        .software_owner
        .as_ref()
        .ok_or("missing disk owner")?;
    assert_eq!(disk_owner.part_id, data_owner.part_id);
    assert_ne!(disk_owner.area_id, data_owner.area_id);
    assert_eq!(disk_owner.part_order, 0);
    assert_eq!(disk_owner.area_order, 1);
    Ok(())
}

#[test]
fn bulk_owner_and_digest_plans_search_requested_ids_before_catalog_rows() -> TestResult {
    let pool = pool()?;
    let mut connection = pool.get()?;
    create_request_table(&mut connection)?;
    insert_requested_ids(&mut connection, &BTreeSet::from([1]))?;
    let owners = requested_occurrence_select();
    for (query, expected_search, forbidden_scan) in [
        (owners, "search occurrence", "occurrence"),
        (digest_select(), "search assertion", "assertion"),
        (
            no_intro_dat_rom_select(),
            "search rom using primary key (occurrence_id=?)",
            "rom",
        ),
    ] {
        let details = sql_query(format!("EXPLAIN QUERY PLAN {query}"))
            .load::<ExplainRow>(&mut connection)?
            .into_iter()
            .map(|row| row.detail.to_ascii_lowercase())
            .collect::<Vec<_>>();
        assert!(
            !details
                .iter()
                .any(|detail| detail.starts_with(&format!("scan {forbidden_scan}"))),
            "catalog-wide scan for bounded request: {details:?}"
        );
        assert!(
            details
                .iter()
                .any(|detail| detail.starts_with("scan requested"))
        );
        assert!(
            details
                .iter()
                .any(|detail| detail.starts_with(expected_search)),
            "missing bounded owner lookup {expected_search}: {details:?}"
        );
    }
    drop_request_table(&mut connection)?;
    Ok(())
}

#[test]
fn native_payload_page_plan_searches_payload_keys_without_scanning_union_sources() -> TestResult {
    let pool = pool()?;
    let directory = tempfile::tempdir()?;
    import_document(
        &pool,
        &directory,
        "root.dat",
        crate::app::CatalogDocumentFormat::Logiqx,
        logiqx_document(),
    )?;
    let ids = occurrence_ids(&pool)?;
    let mut connection = pool.get()?;
    create_request_table(&mut connection)?;
    insert_requested_ids(&mut connection, &BTreeSet::from([ids[0].database_value()]))?;
    let plan = sql_query(format!("EXPLAIN QUERY PLAN {}", native_payload_select()))
        .load::<ExplainRow>(&mut connection)?;
    drop_request_table(&mut connection)?;

    let details = plan
        .iter()
        .map(|row| row.detail.to_ascii_lowercase())
        .collect::<Vec<_>>();
    assert!(!details.iter().any(|detail| detail.contains("materialize")));
    for table in [
        "mame_rom",
        "mame_disk",
        "logiqx_rom",
        "logiqx_disk",
        "logiqx_sample",
        "cmp_rom",
        "cmp_sample",
        "no_intro_file",
        "mame_sample",
        "software_rom",
        "software_disk",
        "software_use",
        "software_declaration",
        "software_area",
        "software_part",
    ] {
        assert!(
            !details
                .iter()
                .any(|detail| detail.contains(&format!("scan {table}"))),
            "unexpected full scan of {table}: {details:?}"
        );
    }
    assert!(
        details
            .iter()
            .any(|detail| detail.contains("scan requested"))
    );
    assert!(
        details
            .iter()
            .any(|detail| detail.contains("search occurrence"))
    );
    assert!(
        details
            .iter()
            .any(|detail| detail.contains("search logiqx_rom "))
    );
    Ok(())
}

#[test]
fn root_and_software_lists_share_a_content_identity_without_flattening() -> TestResult {
    let pool = pool()?;
    let directory = tempfile::tempdir()?;
    import_document(
        &pool,
        &directory,
        "root.dat",
        crate::app::CatalogDocumentFormat::Logiqx,
        logiqx_document(),
    )?;
    import_document(
        &pool,
        &directory,
        "software.xml",
        crate::app::CatalogDocumentFormat::MameSoftwareListXml,
        r#"<softwarelists><softwarelist name="cart-list"><software name="game">
           <description>Game</description><year>2000</year><publisher>Maker</publisher>
           <part name="cart" interface="cart"><dataarea name="rom" size="4">
           <rom name="game.bin" size="4" crc="352441c2" sha1="a9993e364706816aba3e25717850c26c9cd0d89d"/></dataarea></part>
           </software></softwarelist></softwarelists>"#,
    )?;

    let ids = occurrence_ids(&pool)?;
    let occurrences = occurrences_for_ids_in_pool(&pool, &ids)?;
    let root = occurrences
        .iter()
        .find(|occurrence| occurrence.provenance.set_group_kind == SetGroupKind::Root)
        .ok_or_else(|| crate::Error::DatabaseSchema("root owner missing".to_owned()))?;
    let software = occurrences
        .iter()
        .find(|occurrence| {
            matches!(
                occurrence.provenance.set_group_kind,
                SetGroupKind::SoftwareList { .. }
            )
        })
        .ok_or_else(|| crate::Error::DatabaseSchema("software owner missing".to_owned()))?;
    assert_eq!(root.content_id, software.content_id);
    let content_id = root
        .content_id
        .ok_or_else(|| crate::Error::DatabaseSchema("shared UUID missing".to_owned()))?;
    let page =
        occurrences_for_content_in_pool(&pool, content_id, ContentOccurrenceLimit::new(10)?, None)?;
    assert!(
        page.occurrences
            .iter()
            .any(|occurrence| occurrence.provenance.set_group_kind == SetGroupKind::Root)
    );
    assert!(page.occurrences.iter().any(|occurrence| matches!(
        occurrence.provenance.set_group_kind,
        SetGroupKind::SoftwareList { .. }
    )));
    Ok(())
}

#[derive(QueryableByName)]
struct SnapshotFixture {
    #[diesel(sql_type = diesel::sql_types::Text)]
    catalog_key: String,
    #[diesel(sql_type = diesel::sql_types::Text)]
    document_key: String,
    #[diesel(sql_type = diesel::sql_types::Text)]
    interpretation_key: String,
    #[diesel(sql_type = BigInt)]
    coverage_id: i64,
}

#[test]
fn historical_publications_are_visible_and_unpublished_snapshots_are_not() -> TestResult {
    let pool = pool()?;
    let directory = tempfile::tempdir()?;
    import_document_with_identity(
        &pool,
        &directory,
        "history-one.dat",
        crate::app::CatalogDocumentFormat::Logiqx,
        logiqx_document(),
        "history-source",
        "history-catalog",
    )?;
    import_document_with_identity(
        &pool,
        &directory,
        "history-two.dat",
        crate::app::CatalogDocumentFormat::Logiqx,
        &logiqx_document().replace("352441c2", "e8b7be43"),
        "history-source",
        "history-catalog",
    )?;

    let published_ids = occurrence_ids(&pool)?;
    assert_eq!(published_ids.len(), 6);
    let published_content_id = occurrences_for_ids_in_pool(&pool, &published_ids)?[0]
        .content_id
        .ok_or("published fixture occurrence was not linked")?;
    let staged_id = {
        let mut connection = pool.get()?;
        connection.transaction::<_, diesel::result::Error, _>(|connection| {
            let snapshot = sql_query(
                "SELECT catalog_key, document_key, interpretation_key, coverage_id \
                 FROM catalog_snapshots ORDER BY rowid DESC LIMIT 1",
            )
            .get_result::<SnapshotFixture>(connection)?;
            sql_query(
                "INSERT INTO catalog_snapshots \
                 (snapshot_key, catalog_key, document_key, interpretation_key, coverage_id) \
                 VALUES ('staged-snapshot', ?, ?, ?, ?)",
            )
            .bind::<diesel::sql_types::Text, _>(&snapshot.catalog_key)
            .bind::<diesel::sql_types::Text, _>(&snapshot.document_key)
            .bind::<diesel::sql_types::Text, _>(&snapshot.interpretation_key)
            .bind::<BigInt, _>(snapshot.coverage_id)
            .execute(connection)?;
            sql_query(
                "INSERT INTO catalog_set_groups (snapshot_key, kind, list_order) \
                 VALUES ('staged-snapshot', 'root', 0)",
            )
            .execute(connection)?;
            let group_id = sql_query(
                "SELECT set_group_id AS occurrence_id FROM catalog_set_groups \
                 WHERE snapshot_key = 'staged-snapshot'",
            )
            .get_result::<IdRow>(connection)?
            .occurrence_id;
            sql_query(
                "INSERT INTO catalog_sets \
                 (set_group_id, source_element_kind, list_order, set_name, source_line, source_column) \
                 VALUES (?, 'logiqx_game', 0, 'staged', 1, 1)",
            )
            .bind::<BigInt, _>(group_id)
            .execute(connection)?;
            let set_id = sql_query(
                "SELECT set_id AS occurrence_id FROM catalog_sets WHERE set_group_id = ?",
            )
            .bind::<BigInt, _>(group_id)
            .get_result::<IdRow>(connection)?
            .occurrence_id;
            let staged_id = sql_query(
                "INSERT INTO asset_occurrences (record_id, occurrence_order, claim_kind, content_uuid) \
                 VALUES (?, 0, 'logiqx_rom', ?) RETURNING occurrence_id",
            )
            .bind::<BigInt, _>(set_id)
            .bind::<Binary, _>(published_content_id.as_bytes().as_slice())
            .get_result::<IdRow>(connection)
            .map(|row| row.occurrence_id)?;
            sql_query(
                "INSERT INTO logiqx_rom_claims \
                 (occurrence_id, name, evidence_scope, evidence_provenance, source_line, source_column) \
                 VALUES (?, 'staged-rom.bin', 'rom:staged-rom.bin', 'source_declared', 1, 1)",
            )
            .bind::<BigInt, _>(staged_id)
            .execute(connection)?;
            Ok(staged_id)
        })?
    };

    let mut ids = published_ids;
    ids.push(OccurrenceId::from_database(staged_id));
    let visible = occurrences_for_ids_in_pool(&pool, &ids)?;
    assert_eq!(visible.len(), 6);
    assert!(
        visible
            .iter()
            .all(|occurrence| { occurrence.provenance.snapshot_key != "staged-snapshot" })
    );
    assert!(
        visible
            .iter()
            .any(|occurrence| !occurrence.provenance.document_key.is_empty())
    );
    Ok(())
}
