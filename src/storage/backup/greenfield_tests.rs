//! Constructed-native-model backup regressions, not parser-capture evidence.

use camino::{Utf8Path, Utf8PathBuf};
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};
use tempfile::tempdir;

use crate::{
    domain::{DocumentDigest, PublishingSource, PublishingSourceKey},
    storage::{
        catalog_editions::{self, EditionSelection, EditionTarget},
        catalog_ids::{CatalogId, CoverageId, EditionId, ReadingRulesId},
        documents::{AcquisitionMetadata, DocumentStore, RetainedDocument},
        test_catalog,
    },
};

use super::{RestorePolicy, check_integrity, create_backup, restore_backup};

type TestResult<T = ()> = crate::Result<T>;

const SOURCE_KEY: &str = "greenfield-backup";
const KEPT_FILE_UUID: [u8; 16] = *b"kept-file-uuid01";
const ALIAS_FILE_UUID: [u8; 16] = *b"alias-file-uuid1";

#[derive(Debug, PartialEq, Eq, QueryableByName)]
struct InsertedId {
    #[diesel(sql_type = BigInt)]
    id: i64,
}

#[derive(Debug, PartialEq, Eq, QueryableByName)]
struct EditionState {
    #[diesel(sql_type = BigInt)]
    edition_id: i64,
    #[diesel(sql_type = BigInt)]
    source_file_id: i64,
    #[diesel(sql_type = Nullable<BigInt>)]
    previous_edition_id: Option<i64>,
    #[diesel(sql_type = Text)]
    published_at: String,
}

#[derive(Debug, PartialEq, Eq, QueryableByName)]
struct ImportState {
    #[diesel(sql_type = Text)]
    import_key: String,
    #[diesel(sql_type = Text)]
    status: String,
    #[diesel(sql_type = Text)]
    started_at: String,
    #[diesel(sql_type = Nullable<Text>)]
    finished_at: Option<String>,
}

#[derive(Debug, PartialEq, Eq, QueryableByName)]
struct SetState {
    #[diesel(sql_type = BigInt)]
    edition_id: i64,
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = Text)]
    set_name: String,
}

#[derive(Debug, PartialEq, Eq, QueryableByName)]
struct SourceFileState {
    #[diesel(sql_type = BigInt)]
    source_file_id: i64,
    #[diesel(sql_type = Binary)]
    sha256: Vec<u8>,
    #[diesel(sql_type = Nullable<Binary>)]
    sha1: Option<Vec<u8>>,
    #[diesel(sql_type = BigInt)]
    byte_length: i64,
    #[diesel(sql_type = Text)]
    object_key: String,
    #[diesel(sql_type = Text)]
    codec: String,
}

#[derive(Debug, PartialEq, Eq, QueryableByName)]
struct FetchState {
    #[diesel(sql_type = BigInt)]
    fetch_attempt_id: i64,
    #[diesel(sql_type = BigInt)]
    publisher_id: i64,
    #[diesel(sql_type = Text)]
    attempt_key: String,
    #[diesel(sql_type = Text)]
    uri: String,
    #[diesel(sql_type = Text)]
    method: String,
    #[diesel(sql_type = Text)]
    requested_at: String,
    #[diesel(sql_type = Nullable<Text>)]
    responded_at: Option<String>,
    #[diesel(sql_type = Text)]
    outcome: String,
    #[diesel(sql_type = Text)]
    verification_status: String,
    #[diesel(sql_type = Nullable<Text>)]
    declared_filename: Option<String>,
}

#[derive(Debug, PartialEq, Eq, QueryableByName)]
struct ReceiptState {
    #[diesel(sql_type = BigInt)]
    file_receipt_id: i64,
    #[diesel(sql_type = Text)]
    receipt_key: String,
    #[diesel(sql_type = BigInt)]
    fetch_attempt_id: i64,
    #[diesel(sql_type = BigInt)]
    source_file_id: i64,
}

#[derive(Debug, PartialEq, Eq, QueryableByName)]
struct RegistryState {
    #[diesel(sql_type = Binary)]
    registry_uuid: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq, QueryableByName)]
struct IssuedUuidState {
    #[diesel(sql_type = Binary)]
    file_uuid: Vec<u8>,
    #[diesel(sql_type = BigInt)]
    registry_id: i64,
}

#[derive(Debug, PartialEq, Eq, QueryableByName)]
struct RedirectState {
    #[diesel(sql_type = Binary)]
    old_file_uuid: Vec<u8>,
    #[diesel(sql_type = Binary)]
    kept_file_uuid: Vec<u8>,
    #[diesel(sql_type = BigInt)]
    decision_id: i64,
}

#[derive(Debug, PartialEq, Eq, QueryableByName)]
struct ReviewState {
    #[diesel(sql_type = BigInt)]
    relationship_id: i64,
    #[diesel(sql_type = Text)]
    relation_type: String,
    #[diesel(sql_type = BigInt)]
    from_set_id: i64,
    #[diesel(sql_type = BigInt)]
    to_set_id: i64,
    #[diesel(sql_type = BigInt)]
    review_id: i64,
    #[diesel(sql_type = Text)]
    decision: String,
    #[diesel(sql_type = Text)]
    note: String,
}

#[derive(Debug, PartialEq, Eq)]
struct NativeState {
    editions: Vec<EditionState>,
    imports: Vec<ImportState>,
    sets: Vec<SetState>,
    source_files: Vec<SourceFileState>,
    fetch_attempts: Vec<FetchState>,
    receipts: Vec<ReceiptState>,
    registry: RegistryState,
    issued_uuids: Vec<IssuedUuidState>,
    redirects: Vec<RedirectState>,
    reviewed_relationships: Vec<ReviewState>,
}

struct Fixture {
    retained: Vec<RetainedDocument>,
    payloads: Vec<Vec<u8>>,
}

fn utf8_path(path: std::path::PathBuf) -> TestResult<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(path)
        .map_err(|path| crate::Error::InvalidPath(path.display().to_string()))
}

fn connect(path: &Utf8Path) -> TestResult<SqliteConnection> {
    let mut connection = SqliteConnection::establish(path.as_str())
        .map_err(|error| crate::Error::DatabaseSchema(error.to_string()))?;
    connection.batch_execute("PRAGMA foreign_keys=ON")?;
    Ok(connection)
}

fn payload(label: &str) -> Vec<u8> {
    format!(
        "<datafile><header><name>{label}</name></header><game name=\"fixture\"><description>native backup bytes</description></game></datafile>"
    )
    .into_bytes()
}

fn retained_source(store: &DocumentStore, bytes: &[u8]) -> TestResult<RetainedDocument> {
    store.retain(
        &AcquisitionMetadata {
            source_key: PublishingSourceKey::new(SOURCE_KEY),
            source_uri: Some(format!("https://example.invalid/{SOURCE_KEY}/catalog.dat")),
            method: Some("https".to_owned()),
            transport_headers: Vec::new(),
            expected_sha256: Some(DocumentDigest::from_bytes(bytes)),
        },
        bytes,
    )
}

fn fixture(path: &Utf8Path) -> TestResult<Fixture> {
    let publisher = PublishingSource::new(SOURCE_KEY, "Greenfield backup fixture");
    let payloads = vec![payload("first"), payload("second")];
    let retained = {
        let store = DocumentStore::open(path.as_str())?;
        store.register_source(&publisher)?;
        payloads
            .iter()
            .map(|bytes| retained_source(&store, bytes))
            .collect::<TestResult<Vec<_>>>()?
    };

    let mut connection = connect(path)?;
    let catalog = sql_query(
        "INSERT INTO catalogs (publisher_id,catalog_key,display_name) \
         SELECT publisher_id,'greenfield-backup-catalog','Greenfield backup catalog' \
         FROM catalog_publishers WHERE publisher_key=? RETURNING catalog_id AS id",
    )
    .bind::<Text, _>(SOURCE_KEY)
    .get_result::<InsertedId>(&mut connection)?;
    let rules = sql_query(
        "INSERT INTO catalog_reading_rules \
         (rules_key,format_family,dialect,specification_version,parser_version,rules_version) \
         VALUES ('greenfield-backup-rules','mame','fixture','0.289','fixture','1') \
         RETURNING reading_rules_id AS id",
    )
    .get_result::<InsertedId>(&mut connection)?;
    let coverage = sql_query(
        "INSERT INTO catalog_coverage (kind) VALUES ('complete') \
         RETURNING coverage_id AS id",
    )
    .get_result::<InsertedId>(&mut connection)?;
    let catalog_id = CatalogId::try_from(catalog.id)?;
    let reading_rules_id = ReadingRulesId::try_from(rules.id)?;
    let coverage_id = CoverageId::try_from(coverage.id)?;

    for uuid in [KEPT_FILE_UUID, ALIAS_FILE_UUID] {
        sql_query("INSERT INTO shared_catalog_files (file_uuid,registry_id) VALUES (?,1)")
            .bind::<Binary, _>(uuid.as_slice())
            .execute(&mut connection)?;
    }

    let mut edition_ids = Vec::with_capacity(2);
    for (index, source) in retained.iter().enumerate() {
        let target = EditionTarget {
            catalog_id,
            source_file_id: source.source_file_id,
            reading_rules_id,
            coverage_id,
            file_receipt_id: Some(source.file_receipt_id),
        };
        let started = catalog_editions::begin(
            &mut connection,
            target,
            &format!("greenfield-backup-import-{}", index + 1),
            &format!("2026-10-08T00:00:0{}Z", index + 1),
        )?;
        let EditionSelection::New(edition_id) = started.edition else {
            return Err(crate::Error::DatabaseSchema(
                "fixture unexpectedly reused an edition".to_owned(),
            ));
        };
        if index == 0 {
            test_catalog::publish_minimal_mame_with_rom(
                &mut connection,
                edition_id,
                "2026-10-08T00:00:03Z",
            )?;
        } else {
            test_catalog::publish_minimal_mame(
                &mut connection,
                edition_id,
                "2026-10-08T00:00:04Z",
            )?;
        }
        sql_query("UPDATE catalog_imports SET status='succeeded',finished_at=? WHERE import_id=?")
            .bind::<Text, _>(format!("2026-10-08T00:00:0{}Z", index + 5))
            .bind::<BigInt, _>(started.import_id.as_i64())
            .execute(&mut connection)?;
        edition_ids.push(edition_id);
    }

    insert_reviewed_relationship(&mut connection, edition_ids[0], edition_ids[1])?;
    insert_reviewed_uuid_redirect(&mut connection, edition_ids[0])?;
    Ok(Fixture { retained, payloads })
}

fn insert_reviewed_relationship(
    connection: &mut SqliteConnection,
    older: EditionId,
    newer: EditionId,
) -> TestResult {
    let from_set_id = older.as_i64() * 10 + 1;
    let to_set_id = newer.as_i64() * 10 + 1;
    connection.batch_execute(
        "INSERT INTO catalog_relationships \
             (relationship_id,assertion_key,origin,edition_id) \
         VALUES (1,'greenfield-backup-review','user',NULL);
         INSERT INTO catalog_relationship_targets (target_id,target_kind) VALUES \
             (1,'catalog_set'),(2,'catalog_set');",
    )?;
    sql_query("INSERT INTO catalog_set_targets (target_id,set_id) VALUES (1,?),(2,?)")
        .bind::<BigInt, _>(from_set_id)
        .bind::<BigInt, _>(to_set_id)
        .execute(connection)?;
    connection.batch_execute(
        "INSERT INTO manual_catalog_relationships \
             (relationship_id,relation_type,from_target_id,to_target_id) \
         VALUES (1,'revision_of',1,2);
         INSERT INTO catalog_relationship_rationales \
             (relationship_id,reason) VALUES (1,'Published fixture editions are successive revisions');
         INSERT INTO catalog_relationship_evidence_publications \
             (relationship_id,evidence_kind) VALUES (1,'rationale');
         INSERT INTO catalog_relationship_reviews \
             (review_key,relationship_id,decision,note,reviewed_at) \
         VALUES ('greenfield-backup-review-accepted',1,'accepted', \
                 'Reviewed native edition relationship','2026-10-08T00:00:08Z');
         INSERT INTO catalog_relationship_review_publications (review_id,published_at) \
         VALUES (1,'2026-10-08T00:00:09Z');",
    )?;
    Ok(())
}

fn insert_reviewed_uuid_redirect(
    connection: &mut SqliteConnection,
    edition: EditionId,
) -> TestResult {
    let incoming_media_id = edition.as_i64() * 10 + 3;
    sql_query(
        "INSERT INTO file_match_conflicts \
             (conflict_id,incoming_media_entry_id,candidate_file_uuid,reason) \
         VALUES (1,?,?, 'disputed_alias'),(2,?,?, 'disputed_alias')",
    )
    .bind::<BigInt, _>(incoming_media_id)
    .bind::<Binary, _>(KEPT_FILE_UUID.as_slice())
    .bind::<BigInt, _>(incoming_media_id)
    .bind::<Binary, _>(ALIAS_FILE_UUID.as_slice())
    .execute(connection)?;
    sql_query(
        "INSERT INTO file_match_decisions (decision_id,decision,kept_file_uuid,rationale) \
         VALUES (1,'merge',?,'Reviewed fixture alias resolution')",
    )
    .bind::<Binary, _>(KEPT_FILE_UUID.as_slice())
    .execute(connection)?;
    connection.batch_execute(
        "INSERT INTO file_match_decision_conflicts (decision_id,conflict_id,outcome) \
         VALUES (1,1,'merged'),(1,2,'merged');",
    )?;
    sql_query(
        "INSERT INTO file_match_uuid_redirects \
             (old_file_uuid,kept_file_uuid,decision_id) VALUES (?,?,1)",
    )
    .bind::<Binary, _>(ALIAS_FILE_UUID.as_slice())
    .bind::<Binary, _>(KEPT_FILE_UUID.as_slice())
    .execute(connection)?;
    connection.batch_execute(
        "INSERT INTO file_match_decision_publications (decision_id,published_at) \
         VALUES (1,'2026-10-08T00:00:10Z');",
    )?;
    Ok(())
}

fn native_state(connection: &mut SqliteConnection) -> TestResult<NativeState> {
    Ok(NativeState {
        editions: sql_query(
            "SELECT edition.edition_id,edition.source_file_id,edition.previous_edition_id, \
                    publication.published_at \
             FROM catalog_editions AS edition \
             JOIN published_catalog_editions AS publication USING (edition_id) \
             ORDER BY edition.edition_id",
        )
        .load(connection)?,
        imports: sql_query(
            "SELECT import_key,status,started_at,finished_at FROM catalog_imports \
             ORDER BY import_id",
        )
        .load(connection)?,
        sets: sql_query(
            "SELECT groups.edition_id,sets.set_id,sets.set_name \
             FROM catalog_sets AS sets JOIN catalog_set_groups AS groups USING (set_group_id) \
             ORDER BY groups.edition_id,sets.set_id",
        )
        .load(connection)?,
        source_files: sql_query(
            "SELECT source_file_id,sha256,sha1,byte_length,object_key,codec \
             FROM catalog_source_files ORDER BY source_file_id",
        )
        .load(connection)?,
        fetch_attempts: sql_query(
            "SELECT fetch_attempt_id,publisher_id,attempt_key,uri,method,requested_at, \
                    responded_at,outcome,verification_status,declared_filename \
             FROM catalog_fetch_attempts ORDER BY fetch_attempt_id",
        )
        .load(connection)?,
        receipts: sql_query(
            "SELECT file_receipt_id,receipt_key,fetch_attempt_id,source_file_id \
             FROM catalog_file_receipts ORDER BY file_receipt_id",
        )
        .load(connection)?,
        registry: sql_query("SELECT registry_uuid FROM file_id_registries WHERE registry_id=1")
            .get_result(connection)?,
        issued_uuids: sql_query(
            "SELECT file_uuid,registry_id FROM shared_catalog_files ORDER BY file_uuid",
        )
        .load(connection)?,
        redirects: sql_query(
            "SELECT redirect.old_file_uuid,redirect.kept_file_uuid,redirect.decision_id \
             FROM file_match_uuid_redirects AS redirect \
             JOIN file_match_decision_publications AS publication USING (decision_id) \
             ORDER BY redirect.old_file_uuid",
        )
        .load(connection)?,
        reviewed_relationships: sql_query(
            "SELECT relationship.relationship_id,manual.relation_type, \
                    from_set.set_id AS from_set_id,to_set.set_id AS to_set_id, \
                    review.review_id,review.decision,review.note \
             FROM active_catalog_relationship_reviews AS active \
             JOIN catalog_relationship_reviews AS review USING (review_id) \
             JOIN catalog_relationships AS relationship USING (relationship_id) \
             JOIN manual_catalog_relationships AS manual USING (relationship_id) \
             JOIN catalog_set_targets AS from_set ON from_set.target_id=manual.from_target_id \
             JOIN catalog_set_targets AS to_set ON to_set.target_id=manual.to_target_id \
             ORDER BY relationship.relationship_id",
        )
        .load(connection)?,
    })
}

fn native_state_at(path: &Utf8Path) -> TestResult<NativeState> {
    native_state(&mut connect(path)?)
}

#[test]
fn constructed_native_model_backup_preserves_history_identity_review_and_sources() -> TestResult {
    let directory = tempdir()?;
    let source = utf8_path(directory.path().join("source.sqlite"))?;
    let backup = utf8_path(directory.path().join("backup.sqlite"))?;
    let restored = utf8_path(directory.path().join("restored.sqlite"))?;
    let fixture = fixture(&source)?;
    let before = native_state_at(&source)?;

    assert_eq!(before.editions.len(), 2);
    assert_eq!(before.editions[0].previous_edition_id, None);
    assert_eq!(
        before.editions[1].previous_edition_id,
        Some(before.editions[0].edition_id)
    );
    assert!(
        before
            .imports
            .iter()
            .all(|import| { import.status == "succeeded" && import.finished_at.is_some() })
    );
    // The second edition also preserves an explicitly empty machine.
    assert_eq!(before.sets.len(), 3);
    assert_eq!(before.fetch_attempts.len(), 2);
    assert_eq!(before.receipts.len(), 2);
    assert_eq!(before.reviewed_relationships.len(), 1);
    assert_eq!(
        before.reviewed_relationships[0].relation_type,
        "revision_of"
    );
    assert_eq!(before.reviewed_relationships[0].decision, "accepted");
    assert_eq!(before.registry.registry_uuid.len(), 16);
    assert_eq!(before.issued_uuids.len(), 2);
    assert!(
        before
            .issued_uuids
            .iter()
            .all(|issued| issued.file_uuid.len() == 16)
    );
    assert_eq!(before.redirects.len(), 1);
    assert_eq!(before.redirects[0].old_file_uuid, ALIAS_FILE_UUID);
    assert_eq!(before.redirects[0].kept_file_uuid, KEPT_FILE_UUID);

    for (retained, bytes) in fixture.retained.iter().zip(&fixture.payloads) {
        let store = DocumentStore::open(source.as_str())?;
        assert_eq!(store.load_source_file(retained.source_file_id)?, *bytes);
    }
    assert!(check_integrity(&source)?.is_clean());
    create_backup(&source, &backup)?;
    restore_backup(&backup, &restored, RestorePolicy::CreateNew)?;

    for (retained, bytes) in fixture.retained.iter().zip(&fixture.payloads) {
        let store = DocumentStore::open(restored.as_str())?;
        assert_eq!(store.load_source_file(retained.source_file_id)?, *bytes);
    }
    let after = native_state_at(&restored)?;
    assert_eq!(after, before);
    assert_eq!(after.source_files.len(), 2);
    assert!(check_integrity(&restored)?.is_clean());
    Ok(())
}

#[test]
fn corrupt_constructed_native_model_does_not_replace_destination() -> TestResult {
    let directory = tempdir()?;
    let source = utf8_path(directory.path().join("source.sqlite"))?;
    let backup = utf8_path(directory.path().join("backup.sqlite"))?;
    let destination = utf8_path(directory.path().join("destination.sqlite"))?;
    fixture(&source)?;
    create_backup(&source, &backup)?;
    {
        let mut connection = connect(&backup)?;
        // A valid user identity without its published rationale is rejected by
        // the canonical relationship integrity view; no guard is disabled.
        sql_query(
            "INSERT INTO catalog_relationships \
             (relationship_id,assertion_key,origin,edition_id) \
             VALUES (999,'incomplete-native-backup-relationship','user',NULL)",
        )
        .execute(&mut connection)?;
    }

    let destination_payload = payload("destination-sentinel");
    let sentinel = {
        let store = DocumentStore::open(destination.as_str())?;
        let publisher = PublishingSource::new(SOURCE_KEY, "Destination sentinel");
        store.register_source(&publisher)?;
        retained_source(&store, &destination_payload)?
    };
    assert!(restore_backup(&backup, &destination, RestorePolicy::ReplaceExisting).is_err());
    {
        let store = DocumentStore::open(destination.as_str())?;
        assert_eq!(
            store.load_source_file(sentinel.source_file_id)?,
            destination_payload
        );
    }
    assert!(check_integrity(&destination)?.is_clean());
    Ok(())
}
