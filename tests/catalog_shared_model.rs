use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    database::Database,
    domain::{CatalogKey, CatalogScope, CatalogSetId, PublishingSourceKey, RelationshipEndpoint},
};

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

#[derive(QueryableByName)]
struct Column {
    #[diesel(sql_type = Text)]
    name: String,
}

#[derive(QueryableByName)]
struct TextValue {
    #[diesel(sql_type = Text)]
    value: String,
}

#[test]
fn catalog_storage_has_no_json_columns_or_catch_all_vendor_fields()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 path")?;
    let _database = Database::open(&path)?;
    let mut connection = SqliteConnection::establish(path.as_str())?;
    let json_columns = sql_query(
        "SELECT count(*) AS count FROM sqlite_schema AS schema \
         JOIN pragma_table_info(schema.name) AS columns \
         WHERE schema.type = 'table' AND lower(columns.name) LIKE '%json%'",
    )
    .get_result::<Count>(&mut connection)?;
    assert_eq!(
        json_columns.count, 0,
        "catalog facts must use typed native columns"
    );
    let catch_all =
        sql_query("SELECT count(*) AS count FROM sqlite_schema WHERE name = 'snapshot_extensions'")
            .get_result::<Count>(&mut connection)?;
    assert_eq!(
        catch_all.count, 0,
        "vendor-only fields belong in external source documents"
    );
    Ok(())
}

#[test]
fn shared_identity_storage_does_not_encode_scope_or_set_metadata_as_json()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 path")?;
    let _database = Database::open(&path)?;
    let mut connection = SqliteConnection::establish(path.as_str())?;
    for table in [
        "catalog_snapshots",
        "parser_interpretations",
        "catalog_sets",
    ] {
        let columns = sql_query(format!("SELECT name FROM pragma_table_info('{table}')"))
            .load::<Column>(&mut connection)?;
        assert!(!columns.is_empty(), "missing {table}");
        assert!(
            columns.iter().all(|column| !column.name.contains("json")),
            "{table} still stores a JSON projection"
        );
    }
    Ok(())
}

#[test]
fn repeated_logiqx_set_names_retain_separate_ordered_native_owners()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 path")?;
    let document = Utf8PathBuf::from_path_buf(directory.path().join("repeated.dat"))
        .map_err(|_| "non-UTF-8 path")?;
    std::fs::write(
        &document,
        "<datafile><game name='same' cloneof='parent-a'><description>First</description><rom name='first.bin' size='4'/></game><game name='same' cloneof='parent-b'><description>Second</description><rom name='second.bin' size='8'/></game></datafile>",
    )?;
    let database = Database::open(&path)?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path: document,
            format: CatalogDocumentFormat::Logiqx,
            source_key: PublishingSourceKey::new("repeated"),
            source_display_name: "Repeated".into(),
            catalog_key: CatalogKey::new("repeated"),
            catalog_display_name: "Repeated".into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    let mut connection = SqliteConnection::establish(path.as_str())?;
    let count = sql_query("SELECT count(*) AS count FROM catalog_sets WHERE set_name = 'same'")
        .get_result::<Count>(&mut connection)?;
    assert_eq!(count.count, 2);
    let owners = sql_query("SELECT count(DISTINCT record_id) AS count FROM asset_occurrences")
        .get_result::<Count>(&mut connection)?;
    assert_eq!(owners.count, 2);
    let descriptions = sql_query("SELECT count(DISTINCT description) AS count FROM logiqx_games")
        .get_result::<Count>(&mut connection)?;
    assert_eq!(descriptions.count, 2);
    let relationship_owners = sql_query(
        "SELECT count(DISTINCT subject_set_id) AS count FROM relationship_assertions WHERE origin = 'source_assertion' AND relation_type = 'source_parent_clone'",
    )
    .get_result::<Count>(&mut connection)?;
    assert_eq!(relationship_owners.count, 2);
    let explanations = app::explain_relationships(&database)?;
    let mut decoded_owners = std::collections::BTreeSet::new();
    for explanation in explanations {
        let RelationshipEndpoint::CatalogRecord(subject) = explanation.claim.subject else {
            return Err("source parent relationship has no catalog subject".into());
        };
        decoded_owners.insert(subject.owner_set_id.ok_or("source subject has no owner")?);
        let mut logical_selector = subject.clone();
        logical_selector.owner_set_id = None;
        assert!(subject.matches_selector(&logical_selector));
        assert!(subject.matches_selector(&subject));
    }
    assert_eq!(decoded_owners.len(), 2);
    Ok(())
}

#[test]
fn catalog_set_ids_reject_nonpositive_deserialized_values() {
    assert!(CatalogSetId::try_from(0).is_err());
    assert!(CatalogSetId::try_from(-1).is_err());
    assert!(serde_json::from_str::<CatalogSetId>("0").is_err());
    assert!(serde_json::from_str::<CatalogSetId>("-1").is_err());
    assert_eq!(
        serde_json::from_str::<CatalogSetId>("1")
            .map(CatalogSetId::as_i64)
            .ok(),
        Some(1)
    );
}

#[test]
fn synthetic_pc_native_fields_preserve_empty_presence_language_order_and_archive_tokens()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 path")?;
    let document = Utf8PathBuf::from_path_buf(directory.path().join("pc.xml"))
        .map_err(|_| "non-UTF-8 path")?;
    std::fs::write(
        &document,
        "<datafile><game name='absent'/><game name='empty' namealt='' languages='' clone='P' bios='arbitrary text'/><game name='linked' id='0001' clone='0002' mergeof='0003' languages=' Ja, En, Ja '/></datafile>",
    )?;
    let database = Database::open(&path)?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path: document,
            format: CatalogDocumentFormat::NoIntroPcXml,
            source_key: PublishingSourceKey::new("pc"),
            source_display_name: "P/C".into(),
            catalog_key: CatalogKey::new("pc"),
            catalog_display_name: "P/C".into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    let mut connection = SqliteConnection::establish(path.as_str())?;
    for (query, expected) in [
        (
            "SELECT count(*) AS count FROM no_intro_pc_games WHERE languages_present=0",
            1,
        ),
        (
            "SELECT count(*) AS count FROM no_intro_pc_games WHERE name_alt='' AND bios_text='arbitrary text' AND languages_present=1",
            1,
        ),
        ("SELECT count(*) AS count FROM no_intro_pc_clone_markers", 1),
        (
            "SELECT count(*) AS count FROM no_intro_pc_clone_links WHERE target_archive_id='0002'",
            1,
        ),
        (
            "SELECT count(*) AS count FROM no_intro_pc_merge_links WHERE target_archive_id='0003'",
            1,
        ),
        (
            "SELECT count(*) AS count FROM no_intro_pc_games WHERE archive_id='0001'",
            1,
        ),
        ("SELECT count(*) AS count FROM relationship_assertions", 0),
    ] {
        assert_eq!(
            sql_query(query).get_result::<Count>(&mut connection)?.count,
            expected
        );
    }
    let languages = sql_query("SELECT group_concat(language,',') AS value FROM (SELECT language FROM no_intro_pc_languages ORDER BY language_order)")
        .get_result::<TextValue>(&mut connection)?;
    assert_eq!(languages.value, "Ja,En,Ja");
    Ok(())
}

#[test]
fn native_set_details_and_links_enforce_format_ownership_and_immutability()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 path")?;
    let document = Utf8PathBuf::from_path_buf(directory.path().join("links.dat"))
        .map_err(|_| "non-UTF-8 path")?;
    std::fs::write(
        &document,
        "<datafile><game name='child' cloneof='parent' romof='bios'><device_ref name='device'/></game></datafile>",
    )?;
    let database = Database::open(&path)?;
    app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path: document,
            format: CatalogDocumentFormat::Logiqx,
            source_key: PublishingSourceKey::new("links"),
            source_display_name: "Links".into(),
            catalog_key: CatalogKey::new("links"),
            catalog_display_name: "Links".into(),
            scope: CatalogScope::Complete,
        },
    )?;
    let mut connection = SqliteConnection::establish(path.as_str())?;
    for statement in [
        "UPDATE logiqx_games SET description='changed'",
        "UPDATE logiqx_set_links SET target_name='changed'",
        "DELETE FROM logiqx_device_references",
        "INSERT INTO mame_machine_links(set_id,link_kind,target_name,source_line,source_column) SELECT set_id,'cloneof','wrong',1,1 FROM catalog_sets",
        "INSERT INTO no_intro_pc_games(set_id) SELECT set_id FROM catalog_sets",
        "INSERT INTO logiqx_device_references(set_id,reference_order,target_name,source_line,source_column) SELECT set_id,1,'late',1,1 FROM catalog_sets",
        "INSERT INTO catalog_sets(set_group_id,source_element_kind,list_order,set_name,source_line,source_column) SELECT set_group_id,'logiqx_game',1,'late',1,1 FROM catalog_set_groups",
    ] {
        assert!(
            sql_query(statement).execute(&mut connection).is_err(),
            "accepted {statement}"
        );
    }
    Ok(())
}
