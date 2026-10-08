use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Binary, Text},
};
use mame_coalesce::{
    AcquisitionMetadata, DocumentStore, PublishingSource, RestorePolicy, check_integrity,
    create_backup,
    domain::{DocumentDigest, PublishingSourceKey},
    restore_backup,
};
use tempfile::tempdir;

const RETAINED_DAT: &[u8] = b"<datafile><header><name>Retention fixture</name></header></datafile>";
type TestResult<T = ()> = mame_coalesce::Result<T>;

#[derive(QueryableByName, PartialEq, Eq, Debug)]
struct SourceFileMetadata {
    #[diesel(sql_type = Binary)]
    sha256: Vec<u8>,
    #[diesel(sql_type = diesel::sql_types::Nullable<Binary>)]
    sha1: Option<Vec<u8>>,
    #[diesel(sql_type = BigInt)]
    byte_length: i64,
    #[diesel(sql_type = Text)]
    object_key: String,
    #[diesel(sql_type = Text)]
    codec: String,
}

#[derive(QueryableByName, PartialEq, Eq, Debug)]
struct RegistryIdentity {
    #[diesel(sql_type = Binary)]
    registry_uuid: Vec<u8>,
}

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

fn open_connection(path: &Utf8PathBuf) -> TestResult<SqliteConnection> {
    SqliteConnection::establish(path.as_str())
        .map_err(|error| mame_coalesce::Error::DatabaseSchema(error.to_string()))
}

#[test]
fn backup_restore_preserves_greenfield_source_file_and_external_object() -> TestResult {
    let directory = tempdir()?;
    let source_path = Utf8PathBuf::from_path_buf(directory.path().join("source.sqlite"))
        .map_err(|path| mame_coalesce::Error::InvalidPath(path.display().to_string()))?;
    let backup_path = Utf8PathBuf::from_path_buf(directory.path().join("backup.sqlite"))
        .map_err(|path| mame_coalesce::Error::InvalidPath(path.display().to_string()))?;
    let restored_path = Utf8PathBuf::from_path_buf(directory.path().join("restored.sqlite"))
        .map_err(|path| mame_coalesce::Error::InvalidPath(path.display().to_string()))?;

    let store = DocumentStore::open(source_path.as_str())?;
    let publisher = PublishingSource::new("backup-source", "Backup source");
    store.register_source(&publisher)?;
    let retained = store.retain(
        &AcquisitionMetadata {
            source_key: PublishingSourceKey::new("backup-source"),
            source_uri: Some("https://example.invalid/backup-source/catalog.dat".to_owned()),
            method: Some("https".to_owned()),
            transport_headers: Vec::new(),
            expected_sha256: Some(DocumentDigest::from_bytes(RETAINED_DAT)),
        },
        RETAINED_DAT,
    )?;
    assert_eq!(
        store.load_source_file(retained.source_file_id)?,
        RETAINED_DAT
    );

    let (source_metadata, source_registry, source_attempts, source_receipts) = {
        let mut connection = open_connection(&source_path)?;
        let metadata = sql_query(
            "SELECT sha256, sha1, byte_length, object_key, codec \
             FROM catalog_source_files WHERE source_file_id = ?",
        )
        .bind::<BigInt, _>(retained.source_file_id.as_i64())
        .get_result::<SourceFileMetadata>(&mut connection)?;
        let registry =
            sql_query("SELECT registry_uuid FROM file_id_registries WHERE registry_id = 1")
                .get_result::<RegistryIdentity>(&mut connection)?;
        let attempts = sql_query("SELECT COUNT(*) AS count FROM catalog_fetch_attempts")
            .get_result::<Count>(&mut connection)?
            .count;
        let receipts = sql_query("SELECT COUNT(*) AS count FROM catalog_file_receipts")
            .get_result::<Count>(&mut connection)?
            .count;
        (metadata, registry, attempts, receipts)
    };
    assert_eq!(source_metadata.byte_length, RETAINED_DAT.len() as i64);
    assert_eq!(source_metadata.codec, "zstd");
    assert_eq!(source_attempts, 1);
    assert_eq!(source_receipts, 1);
    drop(store);
    assert!(check_integrity(&source_path)?.is_clean());

    create_backup(&source_path, &backup_path)?;
    restore_backup(&backup_path, &restored_path, RestorePolicy::CreateNew)?;

    let restored_store = DocumentStore::open(restored_path.as_str())?;
    assert_eq!(
        restored_store.load_source_file(retained.source_file_id)?,
        RETAINED_DAT
    );
    drop(restored_store);
    let mut connection = open_connection(&restored_path)?;
    let restored_metadata = sql_query(
        "SELECT sha256, sha1, byte_length, object_key, codec \
         FROM catalog_source_files WHERE source_file_id = ?",
    )
    .bind::<BigInt, _>(retained.source_file_id.as_i64())
    .get_result::<SourceFileMetadata>(&mut connection)?;
    let restored_registry =
        sql_query("SELECT registry_uuid FROM file_id_registries WHERE registry_id = 1")
            .get_result::<RegistryIdentity>(&mut connection)?;
    let restored_attempts = sql_query("SELECT COUNT(*) AS count FROM catalog_fetch_attempts")
        .get_result::<Count>(&mut connection)?
        .count;
    let restored_receipts = sql_query("SELECT COUNT(*) AS count FROM catalog_file_receipts")
        .get_result::<Count>(&mut connection)?
        .count;

    assert_eq!(restored_metadata, source_metadata);
    assert_eq!(restored_registry, source_registry);
    assert_eq!(restored_attempts, source_attempts);
    assert_eq!(restored_receipts, source_receipts);
    assert!(check_integrity(&restored_path)?.is_clean());
    Ok(())
}
