use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    catalog_files::{self, OccurrenceId},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey, SnapshotKey},
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(QueryableByName)]
struct Column {
    #[diesel(sql_type = Text)]
    name: String,
}

#[test]
fn areas_have_kind_specific_storage_without_nullable_disk_payloads() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let _database = Database::open(&path)?;
    let mut connection = SqliteConnection::establish(path.as_str())?;
    let columns =
        |table: &str, connection: &mut diesel::SqliteConnection| -> TestResult<Vec<String>> {
            Ok(
                sql_query("SELECT name FROM pragma_table_xinfo(?) ORDER BY cid")
                    .bind::<Text, _>(table)
                    .load::<Column>(connection)?
                    .into_iter()
                    .map(|column| column.name)
                    .collect(),
            )
        };
    assert_eq!(
        columns("software_areas", &mut connection)?,
        ["area_id", "part_id", "record_id", "area_kind", "area_order"]
    );
    assert_eq!(
        columns("software_data_areas", &mut connection)?,
        [
            "area_id",
            "area_name",
            "source_order",
            "declared_size_text",
            "declared_size",
            "width",
            "width_specified",
            "endianness",
            "endianness_specified",
            "source_line",
            "source_column"
        ]
    );
    assert_eq!(
        columns("software_disk_areas", &mut connection)?,
        [
            "area_id",
            "area_name",
            "source_order",
            "source_line",
            "source_column"
        ]
    );
    Ok(())
}

#[derive(QueryableByName)]
struct Id {
    #[diesel(sql_type = BigInt)]
    id: i64,
}

struct ImportedAreas {
    _directory: tempfile::TempDir,
    database: Database,
    connection: SqliteConnection,
    snapshot: SnapshotKey,
    ids: Vec<OccurrenceId>,
}

fn imported_areas() -> TestResult<ImportedAreas> {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&path)?;
    let document_path = Utf8PathBuf::try_from(directory.path().join("software.xml"))?;
    std::fs::write(
        &document_path,
        r#"<softwarelist name="list"><software name="game">
        <description>Game</description><year>2000</year><publisher>Example</publisher>
        <part name="cart" interface="cart"><dataarea name="rom" size="8">
        <rom name="file.bin" size="8" offset="0"/></dataarea>
        <diskarea name="disks"><disk name="image"/></diskarea></part>
        </software></softwarelist>"#,
    )?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::MameSoftwareListXml,
            source_key: PublishingSourceKey::new("area-storage"),
            source_display_name: "Area storage".into(),
            catalog_key: CatalogKey::new("area-storage"),
            catalog_display_name: "Area storage".into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    let snapshot = report.snapshot_key.ok_or("snapshot missing")?;
    let mut connection = SqliteConnection::establish(path.as_str())?;
    let ids = sql_query("SELECT occurrence_id AS id FROM asset_occurrences ORDER BY occurrence_id")
        .load::<Id>(&mut connection)?
        .into_iter()
        .map(|row| OccurrenceId::from_database(row.id))
        .collect();
    Ok(ImportedAreas {
        _directory: directory,
        database,
        connection,
        snapshot,
        ids,
    })
}

fn add_opposite_detail(imported: &mut ImportedAreas, kind: &str) -> TestResult {
    imported
        .connection
        .batch_execute("PRAGMA foreign_keys = OFF; PRAGMA recursive_triggers = OFF")?;
    let area = sql_query("SELECT area_id AS id FROM software_areas WHERE area_kind = ?")
        .bind::<Text, _>(kind)
        .get_result::<Id>(&mut imported.connection)?
        .id;
    match kind {
        "disk" => {
            imported
                .connection
                .batch_execute("DROP TRIGGER software_data_areas_native_owner_insert")?;
            sql_query("INSERT INTO software_data_areas(area_id,area_name,source_order,declared_size_text,width,width_specified,endianness,endianness_specified,source_line,source_column) VALUES (?,'opposite',99,'8',8,0,'little',0,1,1)")
                .bind::<BigInt, _>(area).execute(&mut imported.connection)?;
        }
        "data" => {
            imported
                .connection
                .batch_execute("DROP TRIGGER software_disk_areas_native_owner_insert")?;
            sql_query("INSERT INTO software_disk_areas(area_id,area_name,source_order,source_line,source_column) VALUES (?,'opposite',99,1,1)")
                .bind::<BigInt, _>(area).execute(&mut imported.connection)?;
        }
        _ => return Err("unexpected fixture area kind".into()),
    }
    Ok(())
}

#[test]
fn file_queries_reject_a_second_opposite_area_detail() -> TestResult {
    for kind in ["disk", "data"] {
        let mut imported = imported_areas()?;
        assert_eq!(
            catalog_files::occurrences_for_ids(&imported.database, &imported.ids)?.len(),
            2
        );
        add_opposite_detail(&mut imported, kind)?;
        assert!(
            matches!(
                catalog_files::occurrences_for_ids(&imported.database, &imported.ids),
                Err(catalog_files::CatalogFilesError::MismatchedSoftwareFileOwner(_))
            ),
            "{kind} area with both subtypes was silently accepted"
        );
    }
    Ok(())
}

#[test]
fn history_rejects_a_second_opposite_area_detail() -> TestResult {
    for kind in ["disk", "data"] {
        let mut imported = imported_areas()?;
        app::diff_catalog_snapshots(&imported.database, &imported.snapshot, &imported.snapshot)?;
        add_opposite_detail(&mut imported, kind)?;
        let result =
            app::diff_catalog_snapshots(&imported.database, &imported.snapshot, &imported.snapshot);
        assert!(
            matches!(&result,
                Err(mame_coalesce::Error::InvalidPath(message)) if message.contains("exactly one matching native detail")),
            "{kind} area with both subtypes must report its invalid native detail, got {result:?}"
        );
    }
    Ok(())
}
