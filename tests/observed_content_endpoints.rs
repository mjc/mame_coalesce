use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Binary, Text},
};
use mame_coalesce::{
    app::{self, DatImportRequest},
    database::Database,
    domain::{
        CatalogContentId, ContentDigestAlgorithm, ContentIdentity, ObservedContentIdentity,
        RelationshipClaim, RelationshipEndpoint, RelationshipEvidence, RelationshipOrigin,
        RelationshipType,
    },
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const MALFORMED_DIGESTS: [&str; 5] = [
    "algorithm='sha1',digest='abcdefghijklmnopqrst'",
    "algorithm='sha1',digest=x'01'",
    "algorithm='sha256',digest=zeroblob(33)",
    "algorithm='unknown',digest=zeroblob(20)",
    "algorithm=CAST('sha1' AS BLOB),digest=zeroblob(20)",
];

struct CorruptedObservation {
    _directory: tempfile::TempDir,
    connection: SqliteConnection,
    relationship_id: i64,
    draft_id: i64,
    review_id: i64,
}

impl CorruptedObservation {
    fn new(mutation: &str) -> TestResult<Self> {
        let directory = tempfile::tempdir()?;
        let path = Utf8PathBuf::try_from(directory.path().join("corruption.sqlite"))?;
        let database = Database::open(&path)?;
        let observed = RelationshipEndpoint::ObservedContent(ObservedContentIdentity::new(
            ContentDigestAlgorithm::Sha1,
            "11".repeat(20),
        )?);
        let key = app::record_relationship(&database, &relationship(observed.clone(), observed))?;
        let mut connection = SqliteConnection::establish(path.as_str())?;
        let relationship_id = sql_query(
            "SELECT relationship_id AS count FROM catalog_relationships WHERE assertion_key=?",
        )
        .bind::<Text, _>(key.as_str())
        .get_result::<CountRow>(&mut connection)?
        .count;
        let draft_id = sql_query("INSERT INTO catalog_relationships(assertion_key,origin) VALUES('pending-observed','user') RETURNING relationship_id AS count").get_result::<CountRow>(&mut connection)?.count;
        sql_query("INSERT INTO manual_catalog_relationships(relationship_id,relation_type,from_target_id,to_target_id) SELECT ?,'catalog_continuity',target_id,target_id FROM observed_digest_targets").bind::<BigInt,_>(draft_id).execute(&mut connection)?;
        sql_query("INSERT INTO catalog_relationship_rationales(relationship_id,reason) VALUES (?,'valid unsealed draft')").bind::<BigInt,_>(draft_id).execute(&mut connection)?;
        let review_id = sql_query("INSERT INTO catalog_relationship_reviews(review_key,relationship_id,decision,note) VALUES('pending-observed-review',?,'accepted','valid unsealed review') RETURNING review_id AS count").bind::<BigInt,_>(relationship_id).get_result::<CountRow>(&mut connection)?.count;
        connection.batch_execute("SAVEPOINT healthy_publication")?;
        sql_query("INSERT INTO catalog_relationship_evidence_publications(relationship_id,evidence_kind) VALUES (?,'rationale')").bind::<BigInt,_>(draft_id).execute(&mut connection)?;
        sql_query("INSERT INTO catalog_relationship_review_publications(review_id) VALUES (?)")
            .bind::<BigInt, _>(review_id)
            .execute(&mut connection)?;
        connection.batch_execute("ROLLBACK TO healthy_publication; RELEASE healthy_publication; PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF; PRAGMA ignore_check_constraints=ON; DROP TRIGGER digest_identity_immutable_update")?;
        assert_eq!(
            sql_query(format!("UPDATE digest_values SET {mutation}")).execute(&mut connection)?,
            1
        );
        connection.batch_execute("PRAGMA ignore_check_constraints=OFF")?;
        let count = sql_query("SELECT COUNT(*) AS count FROM digest_values")
            .get_result::<CountRow>(&mut connection)?
            .count;
        assert_eq!(count, 1, "one actual dictionary row must be corrupted");
        Ok(Self {
            _directory: directory,
            connection,
            relationship_id,
            draft_id,
            review_id,
        })
    }
}

#[test]
fn observed_subtype_insertion_rejects_malformed_dictionary_rows() -> TestResult {
    for mutation in MALFORMED_DIGESTS {
        let directory = tempfile::tempdir()?;
        let path = Utf8PathBuf::try_from(directory.path().join("insert.sqlite"))?;
        let _database = Database::open(&path)?;
        let mut connection = SqliteConnection::establish(path.as_str())?;
        connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF; PRAGMA ignore_check_constraints=ON; INSERT INTO digest_values(algorithm,digest) VALUES('sha1',zeroblob(20)); DROP TRIGGER digest_identity_immutable_update")?;
        connection.batch_execute(&format!("UPDATE digest_values SET {mutation}"))?;
        connection.batch_execute("PRAGMA ignore_check_constraints=OFF; INSERT INTO catalog_relationship_targets(kind) VALUES('observed_content')")?;
        assert!(connection.batch_execute("INSERT INTO observed_digest_targets(target_id,digest_id) SELECT target.target_id,digest.digest_id FROM catalog_relationship_targets target CROSS JOIN digest_values digest").is_err(), "malformed subtype accepted: {mutation}");
    }
    Ok(())
}

#[test]
fn observed_readiness_rejects_corrupted_digest_algorithm_types_and_lengths() -> TestResult {
    for mutation in MALFORMED_DIGESTS {
        let mut fixture = CorruptedObservation::new(mutation)?;
        let complete = sql_query(
            "SELECT is_complete AS count FROM catalog_relationship_closure WHERE relationship_id=?",
        )
        .bind::<BigInt, _>(fixture.relationship_id)
        .get_result::<CountRow>(&mut fixture.connection)?
        .count;
        assert_eq!(
            complete, 0,
            "corrupted endpoint remains complete: {mutation}"
        );
    }
    Ok(())
}

#[test]
fn observed_evidence_publication_rejects_corrupted_digest_owners() -> TestResult {
    for mutation in MALFORMED_DIGESTS {
        let mut fixture = CorruptedObservation::new(mutation)?;
        let result = sql_query("INSERT INTO catalog_relationship_evidence_publications(relationship_id,evidence_kind) VALUES (?,'rationale')").bind::<BigInt,_>(fixture.draft_id).execute(&mut fixture.connection);
        assert!(
            result.is_err(),
            "corrupted endpoint evidence published: {mutation}"
        );
    }
    Ok(())
}

#[test]
fn observed_review_publication_rejects_corrupted_digest_owners() -> TestResult {
    for mutation in MALFORMED_DIGESTS {
        let mut fixture = CorruptedObservation::new(mutation)?;
        let result =
            sql_query("INSERT INTO catalog_relationship_review_publications(review_id) VALUES (?)")
                .bind::<BigInt, _>(fixture.review_id)
                .execute(&mut fixture.connection);
        assert!(
            result.is_err(),
            "corrupted endpoint review published: {mutation}"
        );
    }
    Ok(())
}

#[derive(QueryableByName)]
struct ContentUuidRow {
    #[diesel(sql_type = Text)]
    content_uuid: String,
}

#[test]
fn observed_digests_round_trip_all_algorithms_without_creating_catalog_content() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::try_from(directory.path().join("observed.sqlite"))?;
    let database = Database::open(&path)?;
    for algorithm in [
        ContentDigestAlgorithm::Crc32,
        ContentDigestAlgorithm::Md5,
        ContentDigestAlgorithm::Sha1,
        ContentDigestAlgorithm::Sha256,
    ] {
        let text = "AB".repeat(algorithm.byte_length());
        let observed =
            RelationshipEndpoint::ObservedContent(ObservedContentIdentity::new(algorithm, &text)?);
        let key =
            app::record_relationship(&database, &relationship(observed.clone(), observed.clone()))?;
        let explanation = app::explain_relationships(&database)?
            .into_iter()
            .find(|explanation| explanation.assertion_key == key)
            .ok_or("missing observed explanation")?;
        assert_eq!(explanation.claim.subject, observed);
        assert_eq!(explanation.claim.target, observed);
        let serialized = serde_json::to_string(&observed)?;
        assert_eq!(
            serde_json::from_str::<RelationshipEndpoint>(&serialized)?,
            observed
        );
    }
    let mut connection = SqliteConnection::establish(path.as_str())?;
    for (table, expected) in [
        ("catalog_contents", 0),
        ("observed_digest_targets", 4),
        ("digest_values", 4),
        ("catalog_relationship_targets", 4),
    ] {
        let count = sql_query(format!("SELECT COUNT(*) AS count FROM {table}"))
            .get_result::<CountRow>(&mut connection)?
            .count;
        assert_eq!(count, expected, "{table}");
    }
    drop(connection);
    drop(database);
    let backup = path.with_file_name("paired-backup.sqlite");
    mame_coalesce::create_backup(&path, &backup)?;
    let restored = path.with_file_name("restored.sqlite");
    mame_coalesce::restore_backup(&backup, &restored, mame_coalesce::RestorePolicy::CreateNew)?;
    let restored_database = Database::open(&restored)?;
    let explanations = app::explain_relationships(&restored_database)?;
    assert_eq!(explanations.len(), 4);
    assert!(explanations.iter().all(|explanation| matches!(
        explanation.claim.subject,
        RelationshipEndpoint::ObservedContent(_)
    ) && matches!(
        explanation.claim.target,
        RelationshipEndpoint::ObservedContent(_)
    )));
    Ok(())
}

#[test]
fn observed_identity_rejects_invalid_digest_and_remains_explicitly_whole_file() -> TestResult {
    let identity = ObservedContentIdentity::new(ContentDigestAlgorithm::Sha1, "AB".repeat(20))?;
    assert_eq!(
        ObservedContentIdentity::SCOPE,
        mame_coalesce::domain::EvidenceScope::WholeAsset
    );
    assert_eq!(identity.algorithm(), ContentDigestAlgorithm::Sha1);
    assert_eq!(identity.digest(), "ab".repeat(20));
    for digest in ["", "AB", "not-a-sha1", &"GG".repeat(20)] {
        assert!(ObservedContentIdentity::new(ContentDigestAlgorithm::Sha1, digest).is_err());
        let serialized = serde_json::json!({"algorithm":"sha1", "digest":digest});
        assert!(serde_json::from_value::<ObservedContentIdentity>(serialized).is_err());
    }
    Ok(())
}

#[test]
fn observed_targets_are_immutable_and_require_real_digest_and_matching_kind_without_fk_enforcement()
-> TestResult {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::try_from(directory.path().join("guard.sqlite"))?;
    let database = Database::open(&path)?;
    let observed = RelationshipEndpoint::ObservedContent(ObservedContentIdentity::new(
        ContentDigestAlgorithm::Sha256,
        "AB".repeat(32),
    )?);
    app::record_relationship(&database, &relationship(observed.clone(), observed))?;
    let mut connection = SqliteConnection::establish(path.as_str())?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF")?;
    for statement in [
        "UPDATE observed_digest_targets SET digest_id=999999",
        "DELETE FROM observed_digest_targets",
        "INSERT OR REPLACE INTO observed_digest_targets SELECT * FROM observed_digest_targets",
        "INSERT OR REPLACE INTO observed_digest_targets(target_id,digest_id) SELECT target_id+1000,digest_id FROM observed_digest_targets",
        "INSERT INTO declared_digest_targets SELECT target_id,'declared_digest',digest_id FROM observed_digest_targets",
    ] {
        let before = sql_query("SELECT COUNT(*) AS count FROM observed_digest_targets")
            .get_result::<CountRow>(&mut connection)?
            .count;
        assert_eq!(before, 1);
        assert!(
            connection.batch_execute(statement).is_err(),
            "mutation bypass: {statement}"
        );
    }
    for (kind, digest) in [("external_record", 999_999), ("observed_content", 999_999)] {
        connection.batch_execute("SAVEPOINT invalid_observed")?;
        sql_query("INSERT INTO catalog_relationship_targets(kind) VALUES (?)")
            .bind::<Text, _>(kind)
            .execute(&mut connection)?;
        let result = sql_query("INSERT INTO observed_digest_targets(target_id,digest_id) SELECT max(target_id),? FROM catalog_relationship_targets").bind::<BigInt,_>(digest).execute(&mut connection);
        assert!(result.is_err(), "{kind} must not accept a missing digest");
        connection.batch_execute("ROLLBACK TO invalid_observed; RELEASE invalid_observed")?;
    }
    connection.batch_execute("SAVEPOINT wrong_kind; INSERT INTO digest_values(algorithm,digest) VALUES('sha1',zeroblob(20)); INSERT INTO catalog_relationship_targets(kind) VALUES('external_record')")?;
    assert!(connection.batch_execute("INSERT INTO observed_digest_targets(target_id,digest_id) SELECT max(target_id),(SELECT max(digest_id) FROM digest_values) FROM catalog_relationship_targets").is_err(), "a valid unowned digest must not hide a wrong target kind");
    connection.batch_execute("ROLLBACK TO wrong_kind; RELEASE wrong_kind")?;
    assert_eq!(app::explain_relationships(&database)?.len(), 1);
    Ok(())
}

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

fn relationship(subject: RelationshipEndpoint, target: RelationshipEndpoint) -> RelationshipClaim {
    RelationshipClaim {
        relation_type: RelationshipType::CatalogContinuity,
        subject,
        target,
        origin: RelationshipOrigin::UserConclusion,
        evidence: RelationshipEvidence::Rationale {
            reason: "observed bytes compared with a catalog assertion".into(),
        },
    }
}

#[test]
fn observed_digest_stays_distinct_from_declared_digest_and_qualified_catalog_uuid() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let dat_path = Utf8PathBuf::from_path_buf(directory.path().join("expected.dat"))
        .map_err(|_| "non-UTF-8 DAT path")?;
    let sha1 = "1111111111111111111111111111111111111111";
    std::fs::write(
        dat_path.as_std_path(),
        format!(
            "<?xml version=\"1.0\"?><datafile><header><name>expected</name></header><game name=\"game\"><rom name=\"game.bin\" size=\"1\" sha1=\"{sha1}\"/></game></datafile>"
        ),
    )?;

    let database = Database::open(&database_path)?;
    app::import_dat(&database, &DatImportRequest { dat_path })?;

    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    let expected_uuid = sql_query(
        "SELECT lower(hex(content_uuid)) AS content_uuid FROM asset_occurrences WHERE content_uuid IS NOT NULL",
    )
    .get_result::<ContentUuidRow>(&mut connection)?
    .content_uuid
    .parse::<CatalogContentId>()?;
    let expected_aliases = sql_query(
        "SELECT count(*) AS count FROM catalog_content_digest_assertions AS alias \
         JOIN digest_values AS digest USING(digest_id) \
         WHERE alias.content_uuid=? AND digest.algorithm='sha1' \
           AND lower(hex(digest.digest))=? AND alias.scope IN ('whole_file','whole_asset') \
           AND alias.provenance='source_declared'",
    )
    .bind::<Binary, _>(expected_uuid.as_bytes().as_slice())
    .bind::<Text, _>(sha1)
    .get_result::<CountRow>(&mut connection)?;
    assert_eq!(
        expected_aliases.count, 1,
        "the UUID must be supported by this whole-file source alias"
    );

    let observed_digest = ContentIdentity::new(ContentDigestAlgorithm::Sha1, sha1)?;
    let observed = RelationshipEndpoint::ObservedContent(ObservedContentIdentity::new(
        ContentDigestAlgorithm::Sha1,
        sha1,
    )?);
    let declared = RelationshipEndpoint::ContentObject(observed_digest);
    let expected = RelationshipEndpoint::SharedCatalogFile(expected_uuid);
    let observed_key =
        app::record_relationship(&database, &relationship(observed.clone(), expected.clone()))?;
    let declared_key =
        app::record_relationship(&database, &relationship(declared.clone(), expected))?;

    let explanations = app::explain_relationships(&database)?;
    assert_eq!(
        explanations
            .iter()
            .find(|explanation| explanation.assertion_key == observed_key)
            .ok_or("observed-content relationship missing")?
            .claim
            .subject,
        observed,
        "observed bytes must round-trip as their own whole-file digest endpoint"
    );
    assert_eq!(
        explanations
            .iter()
            .find(|explanation| explanation.assertion_key == declared_key)
            .ok_or("declared-digest relationship missing")?
            .claim
            .subject,
        declared,
        "an unscoped declared digest must remain a different endpoint"
    );

    for (kind, expected_count) in [
        ("observed_content", 1),
        ("declared_digest", 1),
        ("shared_file", 1),
    ] {
        let count =
            sql_query("SELECT count(*) AS count FROM catalog_relationship_targets WHERE kind=?")
                .bind::<Text, _>(kind)
                .get_result::<CountRow>(&mut connection)?;
        assert_eq!(
            count.count, expected_count,
            "wrong endpoint identity count for {kind}"
        );
    }
    assert_eq!(
        sql_query("SELECT count(*) AS count FROM catalog_contents")
            .get_result::<CountRow>(&mut connection)?
            .count,
        1,
        "observed digest interning must not issue another catalog UUID"
    );
    Ok(())
}
