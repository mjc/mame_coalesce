use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};
use mame_coalesce::{
    RestorePolicy,
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    create_backup,
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
    restore_backup,
};
use std::{error::Error, path::Path};

const SHA1: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const MD5_A: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const MD5_B: &str = "cccccccccccccccccccccccccccccccc";

#[derive(QueryableByName)]
struct Occurrence {
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
    #[diesel(sql_type = Nullable<Binary>)]
    content_uuid: Option<Vec<u8>>,
}

#[derive(QueryableByName)]
struct BinaryValue {
    #[diesel(sql_type = Binary)]
    value: Vec<u8>,
}

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

#[derive(QueryableByName)]
struct TextValue {
    #[diesel(sql_type = Text)]
    value: String,
}

#[derive(QueryableByName)]
struct Diagnostic {
    #[diesel(sql_type = Text)]
    code: String,
    #[diesel(sql_type = Text)]
    message: String,
}

#[derive(QueryableByName)]
struct NativeSize {
    #[diesel(sql_type = Nullable<BigInt>)]
    size: Option<i64>,
}

#[derive(QueryableByName)]
struct IntegerValue {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

#[derive(QueryableByName)]
struct ConflictHashRow {
    #[diesel(sql_type = Text)]
    role: String,
    #[diesel(sql_type = BigInt)]
    evidence_occurrence_id: i64,
    #[diesel(sql_type = Text)]
    algorithm: String,
    #[diesel(sql_type = Binary)]
    digest: Vec<u8>,
    #[diesel(sql_type = Text)]
    scope: String,
    #[diesel(sql_type = Text)]
    provenance: String,
}

#[derive(QueryableByName)]
struct ConflictSizeRow {
    #[diesel(sql_type = Text)]
    role: String,
    #[diesel(sql_type = BigInt)]
    evidence_occurrence_id: i64,
    #[diesel(sql_type = Text)]
    size_field: String,
    #[diesel(sql_type = BigInt)]
    size: i64,
}

#[derive(QueryableByName)]
struct NativeSample {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
}

fn utf8_path(path: impl AsRef<Path>) -> Result<Utf8PathBuf, Box<dyn Error>> {
    Utf8PathBuf::from_path_buf(path.as_ref().to_owned())
        .map_err(|path| format!("non-UTF-8 path: {}", path.display()).into())
}

fn open_database(path: &Path) -> Result<(Database, SqliteConnection), Box<dyn Error>> {
    let database_path = utf8_path(path)?;
    let database = Database::open(&database_path)?;
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    sql_query("PRAGMA foreign_keys = ON").execute(&mut connection)?;
    Ok((database, connection))
}

fn digest_id(
    connection: &mut SqliteConnection,
    algorithm: &str,
    digest: &[u8],
) -> Result<i64, diesel::result::Error> {
    sql_query("SELECT digest_id AS value FROM digest_values WHERE algorithm = ? AND digest = ?")
        .bind::<Text, _>(algorithm)
        .bind::<Binary, _>(digest)
        .get_result::<IntegerValue>(connection)
        .map(|row| row.value)
}

fn record_unexpected_write(
    operation: &str,
    result: &Result<usize, diesel::result::Error>,
    unexpected_writes: &mut Vec<String>,
) {
    if let Ok(rows) = result {
        unexpected_writes.push(format!("{operation} affected {rows} rows"));
    }
}

fn request(
    path: &Path,
    format: CatalogDocumentFormat,
    key: &str,
) -> Result<CatalogImportRequest, Box<dyn Error>> {
    Ok(CatalogImportRequest {
        document_path: utf8_path(path)?,
        format,
        source_key: PublishingSourceKey::new(format!("source-{key}")),
        source_display_name: format!("Source {key}"),
        catalog_key: CatalogKey::new(format!("catalog-{key}")),
        catalog_display_name: format!("Catalog {key}"),
        scope: CatalogScope::Unknown,
    })
}

fn import(
    database: &Database,
    connection: &mut SqliteConnection,
    path: &Path,
    format: CatalogDocumentFormat,
    key: &str,
) -> Result<(), Box<dyn Error>> {
    let report = app::import_catalog(database, &request(path, format, key)?)?;
    if report.status != CatalogImportStatus::Succeeded {
        let diagnostics = sql_query(
            "SELECT code, message FROM import_diagnostics \
             WHERE run_key = ? ORDER BY rowid",
        )
        .bind::<Text, _>(report.run_key.to_string())
        .load::<Diagnostic>(connection)?;
        let details = diagnostics
            .iter()
            .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
            .collect::<Vec<_>>();
        return Err(format!("catalog import failed: {details:?}").into());
    }
    Ok(())
}

fn occurrence(
    connection: &mut SqliteConnection,
    table: &str,
    name: &str,
) -> Result<Occurrence, diesel::result::Error> {
    let query = match table {
        "logiqx_rom_claims" | "mame_rom_claims" | "mame_disk_claims" => {
            format!(
                "SELECT occurrence.occurrence_id, occurrence.content_uuid \
                 FROM asset_occurrences AS occurrence \
                 JOIN {table} AS claim USING (occurrence_id) WHERE claim.name = ?"
            )
        }
        _ => unreachable!("only native claim tables are queried"),
    };
    sql_query(query)
        .bind::<Text, _>(name)
        .get_result(connection)
}

fn registry_uuid(connection: &mut SqliteConnection) -> Result<Vec<u8>, diesel::result::Error> {
    sql_query("SELECT registry_uuid AS value FROM file_id_registries WHERE registry_id = 1")
        .get_result::<BinaryValue>(connection)
        .map(|row| row.value)
}

fn assert_disputed_sha1_is_not_reused(
    connection: &mut SqliteConnection,
    initial: &Occurrence,
    conflicting: &Occurrence,
    later: &Occurrence,
    initial_uuid: &[u8],
) -> Result<(), Box<dyn Error>> {
    assert!(
        later.content_uuid.is_none(),
        "SHA-1 alone must not choose an identity after that alias was disputed"
    );
    let disputed_alias = sql_query(
        "SELECT COUNT(*) AS count FROM occurrence_content_conflicts \
         WHERE occurrence_id = ? AND candidate_content_uuid = ? \
           AND reason = 'disputed_alias'",
    )
    .bind::<BigInt, _>(later.occurrence_id)
    .bind::<Binary, _>(initial_uuid)
    .get_result::<Count>(connection)?;
    assert_eq!(
        disputed_alias.count, 1,
        "the later claim records the disputed candidate identity"
    );

    let source_sha1_assertions = sql_query(
        "SELECT COUNT(*) AS count FROM occurrence_digest_assertions AS assertion \
         JOIN digest_values AS digest USING (digest_id) \
         WHERE assertion.occurrence_id IN (?, ?, ?) \
           AND assertion.provenance = 'source_declared' \
           AND digest.algorithm = 'sha1' AND digest.digest = ?",
    )
    .bind::<BigInt, _>(initial.occurrence_id)
    .bind::<BigInt, _>(conflicting.occurrence_id)
    .bind::<BigInt, _>(later.occurrence_id)
    .bind::<Binary, _>(hex::decode(SHA1)?.as_slice())
    .get_result::<Count>(connection)?;
    assert_eq!(
        source_sha1_assertions.count, 3,
        "source assertions remain attached to all three native claims"
    );
    let conflicting_md5_assertion = sql_query(
        "SELECT COUNT(*) AS count FROM occurrence_digest_assertions AS assertion \
         JOIN digest_values AS digest USING (digest_id) \
         WHERE assertion.occurrence_id = ? AND assertion.provenance = 'source_declared' \
           AND digest.algorithm = 'md5' AND digest.digest = ?",
    )
    .bind::<BigInt, _>(conflicting.occurrence_id)
    .bind::<Binary, _>(hex::decode(MD5_B)?.as_slice())
    .get_result::<Count>(connection)?;
    assert_eq!(
        conflicting_md5_assertion.count, 1,
        "the conflicting native claim keeps its own MD5 assertion"
    );
    Ok(())
}

fn assert_conflict_hash_evidence(
    connection: &mut SqliteConnection,
    initial: &Occurrence,
    conflicting: &Occurrence,
    initial_uuid: &[u8],
) -> Result<(), Box<dyn Error>> {
    let conflict_hashes = sql_query(
        "SELECT evidence.role, evidence.evidence_occurrence_id, digest.algorithm, \
                digest.digest, evidence.scope, evidence.provenance \
         FROM occurrence_content_conflict_hashes AS evidence \
         JOIN digest_values AS digest USING (digest_id) \
         WHERE evidence.occurrence_id = ? AND evidence.candidate_content_uuid = ? \
         ORDER BY evidence.role, digest.algorithm",
    )
    .bind::<BigInt, _>(conflicting.occurrence_id)
    .bind::<Binary, _>(initial_uuid)
    .load::<ConflictHashRow>(connection)?;
    let mut hash_owners = conflict_hashes
        .iter()
        .map(|row| {
            assert_eq!(row.scope, "whole_asset");
            assert_eq!(row.provenance, "source_declared");
            let expected_owner = if row.role == "incoming" {
                conflicting.occurrence_id
            } else {
                initial.occurrence_id
            };
            assert_eq!(row.evidence_occurrence_id, expected_owner);
            (
                row.role.clone(),
                row.algorithm.clone(),
                hex::encode(&row.digest),
            )
        })
        .collect::<Vec<_>>();
    hash_owners.sort();
    assert_eq!(
        hash_owners,
        [
            ("candidate".to_owned(), "md5".to_owned(), MD5_A.to_owned()),
            ("candidate".to_owned(), "sha1".to_owned(), SHA1.to_owned()),
            ("incoming".to_owned(), "md5".to_owned(), MD5_B.to_owned()),
            ("incoming".to_owned(), "sha1".to_owned(), SHA1.to_owned()),
        ],
        "conflict hash evidence stays attached to the native assertion owner"
    );
    Ok(())
}

fn assert_conflict_size_evidence(
    connection: &mut SqliteConnection,
    initial: &Occurrence,
    conflicting: &Occurrence,
    initial_uuid: &[u8],
) -> Result<(), Box<dyn Error>> {
    let conflict_sizes = sql_query(
        "SELECT evidence.role, evidence.evidence_occurrence_id, evidence.size_field, sizes.size \
         FROM occurrence_content_conflict_sizes AS evidence \
         JOIN catalog_file_size_assertions AS sizes \
           ON sizes.occurrence_id = evidence.evidence_occurrence_id \
          AND sizes.size_field = evidence.size_field \
         WHERE evidence.occurrence_id = ? AND evidence.candidate_content_uuid = ? \
         ORDER BY evidence.role",
    )
    .bind::<BigInt, _>(conflicting.occurrence_id)
    .bind::<Binary, _>(initial_uuid)
    .load::<ConflictSizeRow>(connection)?;
    let size_owners = conflict_sizes
        .iter()
        .map(|row| {
            let expected_owner = if row.role == "incoming" {
                conflicting.occurrence_id
            } else {
                initial.occurrence_id
            };
            assert_eq!(row.evidence_occurrence_id, expected_owner);
            (row.role.as_str(), row.size_field.as_str(), row.size)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        size_owners,
        [
            ("candidate", "logiqx_rom_size", 16),
            ("incoming", "mame_rom_size", 17),
        ],
        "conflict size evidence retains each catalog's native size"
    );
    Ok(())
}

fn assert_conflict_rows_reject_invalid_owners(
    connection: &mut SqliteConnection,
    conflicting: &Occurrence,
    initial_uuid: &[u8],
) -> Result<(), Box<dyn Error>> {
    let candidate_md5_id = digest_id(connection, "md5", &hex::decode(MD5_A)?)?;
    assert!(
        sql_query(
            "INSERT INTO occurrence_content_conflict_hashes \
             (occurrence_id, candidate_content_uuid, evidence_occurrence_id, digest_id, \
              scope, provenance, role) \
             VALUES (?, ?, ?, ?, 'whole_asset', 'source_declared', 'incoming')",
        )
        .bind::<BigInt, _>(conflicting.occurrence_id)
        .bind::<Binary, _>(initial_uuid)
        .bind::<BigInt, _>(conflicting.occurrence_id)
        .bind::<BigInt, _>(candidate_md5_id)
        .execute(connection)
        .is_err(),
        "a conflict hash cannot be copied from a different occurrence"
    );
    let conflicting_md5_id = digest_id(connection, "md5", &hex::decode(MD5_B)?)?;
    let unknown_candidate = [0x5a; 16];
    assert!(
        sql_query(
            "INSERT INTO occurrence_content_conflict_hashes \
             (occurrence_id, candidate_content_uuid, evidence_occurrence_id, digest_id, \
              scope, provenance, role) \
             VALUES (?, ?, ?, ?, 'whole_asset', 'source_declared', 'incoming')",
        )
        .bind::<BigInt, _>(conflicting.occurrence_id)
        .bind::<Binary, _>(unknown_candidate.as_slice())
        .bind::<BigInt, _>(conflicting.occurrence_id)
        .bind::<BigInt, _>(conflicting_md5_id)
        .execute(connection)
        .is_err(),
        "conflict hashes require an existing occurrence/candidate conflict"
    );
    assert!(
        sql_query(
            "INSERT INTO occurrence_content_conflict_sizes \
             (occurrence_id, candidate_content_uuid, evidence_occurrence_id, size_field, role) \
             VALUES (?, ?, ?, 'mame_rom_size', 'candidate')",
        )
        .bind::<BigInt, _>(conflicting.occurrence_id)
        .bind::<Binary, _>(initial_uuid)
        .bind::<BigInt, _>(conflicting.occurrence_id)
        .execute(connection)
        .is_err(),
        "a conflict size cannot claim an unlinked occurrence as the candidate owner"
    );
    assert!(
        sql_query(
            "INSERT INTO occurrence_content_conflict_sizes \
             (occurrence_id, candidate_content_uuid, evidence_occurrence_id, size_field, role) \
             VALUES (?, ?, ?, 'mame_rom_size', 'incoming')",
        )
        .bind::<BigInt, _>(conflicting.occurrence_id)
        .bind::<Binary, _>(unknown_candidate.as_slice())
        .bind::<BigInt, _>(conflicting.occurrence_id)
        .execute(connection)
        .is_err(),
        "conflict sizes require an existing occurrence/candidate conflict"
    );
    Ok(())
}

fn assert_digest_projection_and_uuid_constraints(
    connection: &mut SqliteConnection,
    initial: &Occurrence,
    initial_uuid: &[u8],
) -> Result<(), Box<dyn Error>> {
    let view_kind = sql_query(
        "SELECT type AS value FROM sqlite_master WHERE name = 'catalog_content_digest_assertions'",
    )
    .get_result::<TextValue>(connection)?;
    assert_eq!(view_kind.value, "view");
    let sha1_id = digest_id(connection, "sha1", &hex::decode(SHA1)?)?;
    assert!(
        sql_query(
            "INSERT INTO catalog_content_digest_assertions \
             (content_uuid, occurrence_id, digest_id, scope, provenance) \
             VALUES (?, ?, ?, 'whole_asset', 'source_declared')",
        )
        .bind::<Binary, _>(initial_uuid)
        .bind::<BigInt, _>(initial.occurrence_id)
        .bind::<BigInt, _>(sha1_id)
        .execute(connection)
        .is_err(),
        "source assertion copies cannot be inserted into the read-only projection"
    );
    let expected_size_column = sql_query(
        "SELECT COUNT(*) AS count FROM pragma_table_info('catalog_contents') \
         WHERE name = 'expected_size'",
    )
    .get_result::<Count>(connection)?;
    assert_eq!(expected_size_column.count, 0);
    let invalid_content_uuid = [0x5a; 15];
    assert!(
        sql_query("INSERT INTO catalog_contents (content_uuid) VALUES (?)")
            .bind::<Binary, _>(invalid_content_uuid.as_slice())
            .execute(connection)
            .is_err(),
        "content UUIDs must be exactly 16 bytes"
    );
    Ok(())
}

fn assert_native_claim_sizes_and_scope(
    connection: &mut SqliteConnection,
) -> Result<(), Box<dyn Error>> {
    let logiqx_sizes = sql_query(
        "SELECT size AS size FROM logiqx_rom_claims \
         WHERE name IN ('initial.bin', 'later.bin') ORDER BY name",
    )
    .load::<NativeSize>(connection)?;
    assert_eq!(logiqx_sizes.len(), 2);
    assert_eq!(logiqx_sizes[0].size, Some(16));
    assert_eq!(logiqx_sizes[1].size, None);
    let mame_size = sql_query("SELECT size FROM mame_rom_claims WHERE name = 'conflicting.bin'")
        .get_result::<NativeSize>(connection)?;
    assert_eq!(mame_size.size, Some(17));

    assert!(
        occurrence(connection, "mame_disk_claims", "media.chd")?
            .content_uuid
            .is_none(),
        "a scoped disk claim does not receive whole-file identity"
    );
    let sample = sql_query("SELECT record_id AS set_id FROM mame_samples JOIN asset_occurrences USING(occurrence_id) WHERE name = 'sample.wav'")
        .get_result::<NativeSample>(connection)?;
    let sample_occurrences = sql_query(
        "SELECT COUNT(*) AS count FROM asset_occurrences \
         WHERE record_id = ? AND claim_kind = 'mame_sample'",
    )
    .bind::<BigInt, _>(sample.set_id)
    .get_result::<Count>(connection)?;
    assert_eq!(
        sample_occurrences.count, 1,
        "the native MAME sample has its own media occurrence without a whole-file identity"
    );
    Ok(())
}

fn assert_registry_generation_immutable(
    connection: &mut SqliteConnection,
    initial_uuid: &[u8],
) -> Result<(), Box<dyn Error>> {
    let registry = registry_uuid(connection)?;
    assert_eq!(registry.len(), 16);
    assert_eq!(initial_uuid.len(), 16);
    assert!(
        sql_query(
            "UPDATE file_id_registries SET registry_uuid = zeroblob(16) WHERE registry_id = 1"
        )
        .execute(connection)
        .is_err(),
        "an initialized registry generation cannot be changed"
    );
    assert_eq!(registry_uuid(connection)?, registry);
    Ok(())
}

#[test]
fn disputed_sha1_does_not_gain_a_late_sparse_alias() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let (database, mut connection) = open_database(&directory.path().join("disputes.sqlite"))?;

    let initial_path = directory.path().join("initial.dat");
    std::fs::write(
        &initial_path,
        format!(
            "<datafile><header><name>Initial</name></header><game name=\"initial\"><rom name=\"initial.bin\" size=\"16\" md5=\"{MD5_A}\" sha1=\"{SHA1}\"/></game></datafile>"
        ),
    )?;
    import(
        &database,
        &mut connection,
        &initial_path,
        CatalogDocumentFormat::Logiqx,
        "initial",
    )?;
    let initial = occurrence(&mut connection, "logiqx_rom_claims", "initial.bin")?;
    let initial_uuid = initial
        .content_uuid
        .as_deref()
        .ok_or("the initial complete identity should be linked")?;

    let conflicting_path = directory.path().join("conflicting.xml");
    std::fs::write(
        &conflicting_path,
        format!(
            "<mame build=\"synthetic\" debug=\"no\" mameconfig=\"10\"><machine name=\"conflicting\" sourcefile=\"machine.cpp\" isbios=\"yes\"><description>Conflicting machine</description><year>1991</year><manufacturer>MAME maker</manufacturer><device_ref tag=\":sound\" name=\"sound\"/><rom name=\"conflicting.bin\" size=\"17\" md5=\"{MD5_B}\" sha1=\"{SHA1}\"/><disk name=\"media.chd\" sha1=\"{}\"/><sample name=\"sample.wav\"/></machine><machine name=\"sound\"><description>Sound device</description></machine></mame>",
            "d".repeat(40)
        ),
    )?;
    import(
        &database,
        &mut connection,
        &conflicting_path,
        CatalogDocumentFormat::MameListXml,
        "conflicting",
    )?;
    let conflicting = occurrence(&mut connection, "mame_rom_claims", "conflicting.bin")?;
    assert!(
        conflicting.content_uuid.is_none(),
        "a claim that contradicts both the MD5 and size must stay unlinked"
    );

    let later_path = directory.path().join("later.dat");
    std::fs::write(
        &later_path,
        format!(
            "<datafile><header><name>Later</name></header><game name=\"later\"><rom name=\"later.bin\" sha1=\"{SHA1}\"/></game></datafile>"
        ),
    )?;
    import(
        &database,
        &mut connection,
        &later_path,
        CatalogDocumentFormat::Logiqx,
        "later",
    )?;
    let later = occurrence(&mut connection, "logiqx_rom_claims", "later.bin")?;
    assert_disputed_sha1_is_not_reused(
        &mut connection,
        &initial,
        &conflicting,
        &later,
        initial_uuid,
    )?;
    assert_conflict_hash_evidence(&mut connection, &initial, &conflicting, initial_uuid)?;
    assert_conflict_size_evidence(&mut connection, &initial, &conflicting, initial_uuid)?;

    assert_conflict_rows_reject_invalid_owners(&mut connection, &conflicting, initial_uuid)?;

    assert_digest_projection_and_uuid_constraints(&mut connection, &initial, initial_uuid)?;
    assert_native_claim_sizes_and_scope(&mut connection)?;
    assert_registry_generation_immutable(&mut connection, initial_uuid)?;
    Ok(())
}

fn record_registry_and_digest_write_attempts(
    connection: &mut SqliteConnection,
    sha1_id: i64,
    conflicting_md5_id: i64,
    unexpected_writes: &mut Vec<String>,
) {
    record_unexpected_write(
        "registry generation INSERT OR REPLACE",
        &sql_query(
            "INSERT OR REPLACE INTO file_id_registries (registry_id, registry_uuid) \
             VALUES (1, ?)",
        )
        .bind::<Binary, _>(&[0x4d; 16][..])
        .execute(connection),
        unexpected_writes,
    );
    record_unexpected_write(
        "known digest ID INSERT OR REPLACE",
        &sql_query(
            "INSERT OR REPLACE INTO digest_values (digest_id, algorithm, digest) \
             VALUES (?, 'sha256', ?)",
        )
        .bind::<BigInt, _>(sha1_id)
        .bind::<Binary, _>(&[0x6c; 32][..])
        .execute(connection),
        unexpected_writes,
    );
    record_unexpected_write(
        "digest bytes update",
        &sql_query("UPDATE digest_values SET digest = ? WHERE digest_id = ?")
            .bind::<Binary, _>(&[0x5a; 16][..])
            .bind::<BigInt, _>(conflicting_md5_id)
            .execute(connection),
        unexpected_writes,
    );
    record_unexpected_write(
        "digest algorithm update",
        &sql_query("UPDATE digest_values SET algorithm = 'sha256', digest = ? WHERE digest_id = ?")
            .bind::<Binary, _>(&[0x6b; 32][..])
            .bind::<BigInt, _>(sha1_id)
            .execute(connection),
        unexpected_writes,
    );
}

fn record_published_assertion_write_attempts(
    connection: &mut SqliteConnection,
    occurrence_id: i64,
    candidate_md5_id: i64,
    conflicting_md5_id: i64,
    sha1_id: i64,
    unexpected_writes: &mut Vec<String>,
) {
    record_unexpected_write(
        "published source assertion insert",
        &sql_query(
            "INSERT INTO occurrence_digest_assertions \
             (occurrence_id, digest_id, scope, provenance) \
             VALUES (?, ?, 'whole_asset', 'source_declared')",
        )
        .bind::<BigInt, _>(occurrence_id)
        .bind::<BigInt, _>(conflicting_md5_id)
        .execute(connection),
        unexpected_writes,
    );
    record_unexpected_write(
        "published source assertion scope update",
        &sql_query(
            "UPDATE occurrence_digest_assertions SET scope = 'whole_file' \
             WHERE occurrence_id = ? AND digest_id = ? AND scope = 'whole_asset' \
               AND provenance = 'source_declared'",
        )
        .bind::<BigInt, _>(occurrence_id)
        .bind::<BigInt, _>(sha1_id)
        .execute(connection),
        unexpected_writes,
    );
    record_unexpected_write(
        "published source assertion provenance update",
        &sql_query(
            "UPDATE occurrence_digest_assertions SET provenance = 'computed' \
             WHERE occurrence_id = ? AND digest_id = ? AND provenance = 'source_declared'",
        )
        .bind::<BigInt, _>(occurrence_id)
        .bind::<BigInt, _>(candidate_md5_id)
        .execute(connection),
        unexpected_writes,
    );
    record_unexpected_write(
        "published source assertion digest update",
        &sql_query(
            "UPDATE occurrence_digest_assertions SET digest_id = ? \
             WHERE occurrence_id = ? AND digest_id = ?",
        )
        .bind::<BigInt, _>(conflicting_md5_id)
        .bind::<BigInt, _>(occurrence_id)
        .bind::<BigInt, _>(sha1_id)
        .execute(connection),
        unexpected_writes,
    );
    record_unexpected_write(
        "published source assertion delete",
        &sql_query(
            "DELETE FROM occurrence_digest_assertions \
             WHERE occurrence_id = ? AND digest_id = ?",
        )
        .bind::<BigInt, _>(occurrence_id)
        .bind::<BigInt, _>(candidate_md5_id)
        .execute(connection),
        unexpected_writes,
    );
}

fn record_conflict_write_attempts(
    connection: &mut SqliteConnection,
    conflicting: &Occurrence,
    initial_uuid: &[u8],
    unexpected_writes: &mut Vec<String>,
) {
    record_unexpected_write(
        "published conflict reason update",
        &sql_query(
            "UPDATE occurrence_content_conflicts SET reason = 'ambiguous_alias' \
             WHERE occurrence_id = ? AND candidate_content_uuid = ?",
        )
        .bind::<BigInt, _>(conflicting.occurrence_id)
        .bind::<Binary, _>(initial_uuid)
        .execute(connection),
        unexpected_writes,
    );
    record_unexpected_write(
        "published conflict delete",
        &sql_query(
            "DELETE FROM occurrence_content_conflicts \
             WHERE occurrence_id = ? AND candidate_content_uuid = ?",
        )
        .bind::<BigInt, _>(conflicting.occurrence_id)
        .bind::<Binary, _>(initial_uuid)
        .execute(connection),
        unexpected_writes,
    );
}

#[test]
fn published_digest_assertions_and_disputes_are_immutable() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let (database, mut connection) = open_database(&directory.path().join("immutable.sqlite"))?;

    let initial_path = directory.path().join("initial.dat");
    std::fs::write(
        &initial_path,
        format!(
            "<datafile><header><name>Initial</name></header><game name=\"initial\"><rom name=\"initial.bin\" size=\"16\" md5=\"{MD5_A}\" sha1=\"{SHA1}\"/></game></datafile>"
        ),
    )?;
    import(
        &database,
        &mut connection,
        &initial_path,
        CatalogDocumentFormat::Logiqx,
        "immutable-initial",
    )?;
    let initial = occurrence(&mut connection, "logiqx_rom_claims", "initial.bin")?;
    let initial_uuid = initial
        .content_uuid
        .as_deref()
        .ok_or("the imported identity should be linked")?;

    let conflicting_path = directory.path().join("conflicting.xml");
    std::fs::write(
        &conflicting_path,
        format!(
            "<mame build=\"synthetic\" debug=\"no\" mameconfig=\"10\"><machine name=\"conflicting\"><description>Conflicting</description><device_ref tag=\":sound\" name=\"sound\"/><rom name=\"conflicting.bin\" size=\"17\" md5=\"{MD5_B}\" sha1=\"{SHA1}\"/></machine><machine name=\"sound\"><description>Sound device</description></machine></mame>"
        ),
    )?;
    import(
        &database,
        &mut connection,
        &conflicting_path,
        CatalogDocumentFormat::MameListXml,
        "immutable-conflicting",
    )?;
    let conflicting = occurrence(&mut connection, "mame_rom_claims", "conflicting.bin")?;
    let conflict_count = sql_query(
        "SELECT COUNT(*) AS count FROM occurrence_content_conflicts \
         WHERE occurrence_id = ? AND candidate_content_uuid = ?",
    )
    .bind::<BigInt, _>(conflicting.occurrence_id)
    .bind::<Binary, _>(initial_uuid)
    .get_result::<Count>(&mut connection)?;
    assert_eq!(conflict_count.count, 1);

    let sha1_id = digest_id(&mut connection, "sha1", &hex::decode(SHA1)?)?;
    let candidate_md5_id = digest_id(&mut connection, "md5", &hex::decode(MD5_A)?)?;
    let conflicting_md5_id = digest_id(&mut connection, "md5", &hex::decode(MD5_B)?)?;
    let mut unexpected_writes = Vec::new();
    record_registry_and_digest_write_attempts(
        &mut connection,
        sha1_id,
        conflicting_md5_id,
        &mut unexpected_writes,
    );
    record_published_assertion_write_attempts(
        &mut connection,
        initial.occurrence_id,
        candidate_md5_id,
        conflicting_md5_id,
        sha1_id,
        &mut unexpected_writes,
    );
    record_conflict_write_attempts(
        &mut connection,
        &conflicting,
        initial_uuid,
        &mut unexpected_writes,
    );

    assert!(
        unexpected_writes.is_empty(),
        "published identity evidence must be immutable; unexpectedly accepted: {unexpected_writes:?}"
    );
    Ok(())
}

#[test]
fn matching_cross_format_claims_share_identity() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let (database, mut connection) = open_database(&directory.path().join("shared.sqlite"))?;
    let logiqx_path = directory.path().join("whole-file.dat");
    std::fs::write(
        &logiqx_path,
        format!(
            "<datafile><header><name>Logiqx</name></header><game name=\"logiqx\"><rom name=\"logiqx.bin\" size=\"16\" md5=\"{MD5_A}\" sha1=\"{SHA1}\"/></game></datafile>"
        ),
    )?;
    import(
        &database,
        &mut connection,
        &logiqx_path,
        CatalogDocumentFormat::Logiqx,
        "whole-file",
    )?;

    let mame_path = directory.path().join("whole-asset.xml");
    std::fs::write(
        &mame_path,
        format!(
            "<mame build=\"synthetic\" debug=\"no\" mameconfig=\"10\"><machine name=\"mame\"><description>MAME machine</description><device_ref tag=\":sound\" name=\"sound\"/><rom name=\"mame.bin\" size=\"16\" md5=\"{MD5_A}\" sha1=\"{SHA1}\"/></machine><machine name=\"sound\"><description>Sound device</description></machine></mame>"
        ),
    )?;
    import(
        &database,
        &mut connection,
        &mame_path,
        CatalogDocumentFormat::MameListXml,
        "whole-asset",
    )?;

    let logiqx = occurrence(&mut connection, "logiqx_rom_claims", "logiqx.bin")?;
    let mame = occurrence(&mut connection, "mame_rom_claims", "mame.bin")?;
    assert_eq!(logiqx.content_uuid, mame.content_uuid);
    assert!(logiqx.content_uuid.is_some());
    let bytes: [u8; 16] = logiqx
        .content_uuid
        .as_deref()
        .ok_or("shared file UUID missing")?
        .try_into()?;
    let page = mame_coalesce::catalog_files::occurrences_for_content(
        &database,
        mame_coalesce::domain::CatalogContentId::from_bytes(bytes),
        mame_coalesce::catalog_files::ContentOccurrenceLimit::new(10)?,
        None,
    )?;
    assert_eq!(page.occurrences.len(), 2);
    let mut names = page
        .occurrences
        .iter()
        .filter_map(|entry| entry.provenance.asset_name.as_deref())
        .collect::<Vec<_>>();
    names.sort_unstable();
    assert_eq!(names, ["logiqx.bin", "mame.bin"]);
    assert!(page.next_cursor.is_none());
    let whole_scopes = sql_query(
        "SELECT DISTINCT assertion.scope AS value \
         FROM occurrence_digest_assertions AS assertion \
         JOIN digest_values AS digest USING (digest_id) \
         WHERE assertion.occurrence_id IN (?, ?) AND assertion.provenance = 'source_declared' \
           AND digest.algorithm = 'sha1' ORDER BY assertion.scope",
    )
    .bind::<BigInt, _>(logiqx.occurrence_id)
    .bind::<BigInt, _>(mame.occurrence_id)
    .load::<TextValue>(&mut connection)?;
    assert_eq!(
        whole_scopes
            .iter()
            .map(|row| row.value.as_str())
            .collect::<Vec<_>>(),
        ["whole_asset"]
    );
    Ok(())
}

#[test]
fn registry_generation_and_content_identity_survive_backup_restore() -> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let source_path = directory.path().join("source.sqlite");
    let backup_path = directory.path().join("source.backup.sqlite");
    let restored_path = directory.path().join("restored.sqlite");
    let fresh_path = directory.path().join("fresh.sqlite");
    let document_path = directory.path().join("identity.dat");
    std::fs::write(
        &document_path,
        format!(
            "<datafile><header><name>Identity</name></header><game name=\"identity\"><rom name=\"identity.bin\" size=\"16\" md5=\"{MD5_A}\" sha1=\"{SHA1}\"/></game></datafile>"
        ),
    )?;

    let (database, mut connection) = open_database(&source_path)?;
    import(
        &database,
        &mut connection,
        &document_path,
        CatalogDocumentFormat::Logiqx,
        "identity",
    )?;
    let original_occurrence = occurrence(&mut connection, "logiqx_rom_claims", "identity.bin")?;
    let original_uuid = original_occurrence
        .content_uuid
        .ok_or("source content identity missing")?;
    let original_registry = registry_uuid(&mut connection)?;
    drop(connection);
    drop(database);

    create_backup(&utf8_path(&source_path)?, &utf8_path(&backup_path)?)?;
    restore_backup(
        &utf8_path(&backup_path)?,
        &utf8_path(&restored_path)?,
        RestorePolicy::CreateNew,
    )?;

    let (restored_database, mut restored_connection) = open_database(&restored_path)?;
    assert_eq!(registry_uuid(&mut restored_connection)?, original_registry);
    let restored_occurrence = occurrence(
        &mut restored_connection,
        "logiqx_rom_claims",
        "identity.bin",
    )?;
    assert_eq!(
        restored_occurrence.content_uuid.as_deref(),
        Some(original_uuid.as_slice())
    );
    drop(restored_connection);
    drop(restored_database);

    let (fresh_database, mut fresh_connection) = open_database(&fresh_path)?;
    import(
        &fresh_database,
        &mut fresh_connection,
        &document_path,
        CatalogDocumentFormat::Logiqx,
        "identity",
    )?;
    let fresh_registry = registry_uuid(&mut fresh_connection)?;
    assert_ne!(fresh_registry, original_registry);
    let fresh_occurrence = occurrence(&mut fresh_connection, "logiqx_rom_claims", "identity.bin")?;
    assert_ne!(
        fresh_occurrence.content_uuid.as_deref(),
        Some(original_uuid.as_slice())
    );
    Ok(())
}
