use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Text},
};
use mame_coalesce::{
    NoIntroDatMode, RestorePolicy,
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    check_integrity, create_backup,
    database::Database,
    domain::{
        CatalogKey, CatalogScope, ExternalRecordRef, PublishingSourceKey, RelationshipClaim,
        RelationshipEndpoint, RelationshipEvidence, RelationshipOrigin, RelationshipReview,
        RelationshipReviewDecision, RelationshipType, SnapshotKey,
    },
    no_intro_db_xml::NoIntroDatabaseMode,
    restore_backup,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const LOGIQX: &str = r#"<datafile><game name="child" cloneof="missing-clone" romof="missing-rom" sampleof=""><description>Child</description><rom name="first.bin" size="1" merge="missing.bin"/><rom name="empty.bin" size="1" merge=""/><disk name="logical-disk" merge="missing-disk"/></game><game name="orphan"><description>Orphan</description><rom name="orphan.bin" size="1" merge="missing.bin"/></game></datafile>"#;
const CMP: &str = r#"game ( name "child" cloneof "missing-clone" sampleof "" rom ( name "first.bin" size 1 merge "missing.bin" ) rom ( name "empty.bin" size 1 merge "" ) ) game ( name "orphan" rom ( name "orphan.bin" size 1 merge "missing.bin" ) )"#;

const SOFTWARE: &str = r#"<softwarelists><softwarelist name="first" description="First"><software name="child" cloneof="missing"><description>Child</description><year>2026</year><publisher>Publisher</publisher></software></softwarelist><softwarelist name="second" description="Second"><software name="child" cloneof=""><description>Child</description><year>2026</year><publisher>Publisher</publisher></software></softwarelist></softwarelists>"#;
const NO_INTRO_DAT: &str = r#"<datafile><header><id>1</id><name>Native parents</name><description>Native parents</description><version>1</version></header><game name="child" cloneof="missing" cloneofid="0007"><description>Child</description><rom name="child.bin"/></game><game name="empty" cloneof="" cloneofid=""><description>Empty</description><rom name="empty.bin"/></game><game name="id-only" cloneofid="7"><description>ID only</description><rom name="id.bin"/></game></datafile>"#;
const NO_INTRO_DATABASE: &str = r#"<datafile><header><version>native-v1</version></header><game name="same"><archive number="0001" clone="0007" mergeof="7"/><archive number="0001" clone="" mergeof=""/><archive number="0007" clone="P"/></game><game name="same"><archive number="7" clone="0007"/></game></datafile>"#;

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
struct FieldPosition {
    #[diesel(sql_type = Text)]
    assertion_key: String,
    #[diesel(sql_type = Text)]
    source_field: String,
    #[diesel(sql_type = BigInt)]
    source_order: i64,
    #[diesel(sql_type = BigInt)]
    is_quoted: i64,
    #[diesel(sql_type = BigInt)]
    source_line: i64,
    #[diesel(sql_type = BigInt)]
    source_column: i64,
}

#[derive(QueryableByName)]
struct QueryPlan {
    #[diesel(sql_type = Text)]
    detail: String,
}

struct Fixture {
    directory: tempfile::TempDir,
    database_path: Utf8PathBuf,
    database: Database,
    request: CatalogImportRequest,
    snapshot: SnapshotKey,
}

impl Fixture {
    fn new(format: CatalogDocumentFormat, source: &str) -> TestResult<Self> {
        let directory = tempfile::tempdir()?;
        let path = Utf8PathBuf::try_from(directory.path().join("source.dat"))?;
        let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
        std::fs::write(&path, source)?;
        let database = Database::open(&database_path)?;
        let request = CatalogImportRequest {
            document_path: path,
            format,
            source_key: PublishingSourceKey::new("native-reference-test"),
            source_display_name: "Native reference test".into(),
            catalog_key: CatalogKey::new("native-reference-test"),
            catalog_display_name: "Native reference test".into(),
            scope: CatalogScope::Complete,
        };
        let report = app::import_catalog(&database, &request)?;
        if report.status != CatalogImportStatus::Succeeded {
            #[derive(Debug, QueryableByName)]
            struct Diagnostic {
                #[diesel(sql_type = Text)]
                message: String,
            }
            let mut connection = SqliteConnection::establish(database_path.as_str())?;
            let messages = sql_query("SELECT message FROM import_diagnostics WHERE run_key=?")
                .bind::<Text, _>(report.run_key.to_string())
                .load::<Diagnostic>(&mut connection)?
                .into_iter()
                .map(|row| row.message)
                .collect::<Vec<_>>();
            return Err(format!("fixture import failed: {messages:?}").into());
        }
        let snapshot = report.snapshot_key.ok_or("snapshot missing")?;
        Ok(Self {
            directory,
            database_path,
            database,
            request,
            snapshot,
        })
    }

    fn connection(&self) -> TestResult<SqliteConnection> {
        Ok(SqliteConnection::establish(self.database_path.as_str())?)
    }
}

fn assert_native_relationships(
    format: CatalogDocumentFormat,
    source: &str,
    expected_merges: usize,
    expected_identities: i64,
) -> TestResult {
    let fixture = Fixture::new(format, source)?;
    let snapshot = &fixture.snapshot;
    let explanations = app::explain_relationships(&fixture.database)?;
    let merges = explanations
        .iter()
        .filter(|row| row.claim.relation_type == RelationshipType::SourceMerge)
        .collect::<Vec<_>>();
    assert_eq!(
        merges.len(),
        expected_merges,
        "every declared merge owns a source identity, even without a resolvable parent"
    );
    for merge in merges {
        assert!(
            matches!(&merge.claim.subject, RelationshipEndpoint::CatalogMediaEntry { snapshot: actual, .. } if actual == snapshot)
        );
        assert!(
            matches!(&merge.claim.target, RelationshipEndpoint::CatalogMergeReference { snapshot: actual, .. } if actual == snapshot)
        );
        assert_eq!(merge.source_field.as_deref(), Some("merge"));
        assert!(merge.source_location.is_some());
    }
    assert!(
        !explanations
            .iter()
            .any(|row| row.claim.relation_type == RelationshipType::ExactContentIdentity),
        "a source merge must not manufacture resolved identity evidence"
    );
    assert!(
        explanations.iter().any(|row| matches!(&row.claim.target,
        RelationshipEndpoint::CatalogMergeReference { merge_name, .. } if merge_name.is_empty())),
        "present empty merge literals are not absent declarations"
    );
    assert!(
        explanations.iter().any(|row| matches!(
            &row.claim.target,
            RelationshipEndpoint::CatalogMergeReference {
                parent_name: None,
                ..
            }
        )),
        "parentless source declarations still require query storage"
    );
    let mut connection = fixture.connection()?;
    let issued = sql_query("SELECT count(*) AS count FROM catalog_relationships WHERE snapshot_key=? AND origin='source'").bind::<Text,_>(snapshot.as_str()).get_result::<Count>(&mut connection)?;
    assert_eq!(issued.count, expected_identities);
    let copied = sql_query(
        "SELECT count(*) AS count FROM relationship_assertions WHERE source_snapshot_key=?",
    )
    .bind::<Text, _>(snapshot.as_str())
    .get_result::<Count>(&mut connection)?;
    assert_eq!(
        copied.count, 0,
        "native declarations must not have a second generic owner"
    );
    let reimport = app::import_catalog(&fixture.database, &fixture.request)?;
    assert_eq!(reimport.snapshot_key.as_ref(), Some(snapshot));
    std::fs::remove_file(&fixture.request.document_path)?;
    assert_eq!(app::explain_relationships(&fixture.database)?, explanations);
    Ok(())
}

#[test]
fn logiqx_parent_and_merge_declarations_have_once_issued_native_owners() -> TestResult {
    assert_native_relationships(
        CatalogDocumentFormat::Logiqx(mame_coalesce::logiqx::LogiqxMode::ObservedCompatible),
        LOGIQX,
        4,
        7,
    )
}

#[test]
fn cmp_parent_and_merge_declarations_have_once_issued_native_owners() -> TestResult {
    assert_native_relationships(CatalogDocumentFormat::ClrMamePro, CMP, 3, 5)
}

fn assert_native_parent_owners(
    format: CatalogDocumentFormat,
    source: &str,
    expected_identities: i64,
) -> TestResult {
    let fixture = Fixture::new(format, source)?;
    let mut connection = fixture.connection()?;
    let issued = sql_query("SELECT count(*) AS count FROM reported_catalog_relationships AS reported JOIN catalog_relationships AS registry USING(relationship_id) WHERE registry.snapshot_key=?")
        .bind::<Text, _>(fixture.snapshot.as_str())
        .get_result::<Count>(&mut connection)?;
    assert_eq!(
        issued.count, expected_identities,
        "each declared parent needs a once-issued native relationship identity"
    );
    let copied = sql_query(
        "SELECT count(*) AS count FROM relationship_assertions WHERE source_snapshot_key=?",
    )
    .bind::<Text, _>(fixture.snapshot.as_str())
    .get_result::<Count>(&mut connection)?;
    assert_eq!(
        copied.count, 0,
        "native declarations must not have generic copies"
    );
    let before = app::explain_relationships(&fixture.database)?;
    assert_eq!(before.len(), usize::try_from(expected_identities)?);
    let reimport = app::import_catalog(&fixture.database, &fixture.request)?;
    assert_eq!(reimport.snapshot_key.as_ref(), Some(&fixture.snapshot));
    std::fs::remove_file(&fixture.request.document_path)?;
    assert_eq!(app::explain_relationships(&fixture.database)?, before);
    Ok(())
}

#[test]
fn software_clones_use_the_shared_native_identity_registry() -> TestResult {
    assert_native_parent_owners(CatalogDocumentFormat::MameSoftwareListXml, SOFTWARE, 2)
}

#[test]
fn dat_name_and_id_parents_have_distinct_native_identities() -> TestResult {
    assert_native_parent_owners(
        CatalogDocumentFormat::NoIntroDat(NoIntroDatMode::V4Compatible),
        NO_INTRO_DAT,
        5,
    )
}

#[test]
fn database_archive_references_use_once_issued_native_identities() -> TestResult {
    assert_native_parent_owners(
        CatalogDocumentFormat::NoIntroDatabase(NoIntroDatabaseMode::ObservedCompatible),
        NO_INTRO_DATABASE,
        5,
    )
}

fn remove_publication(fixture: &Fixture, connection: &mut SqliteConnection) -> TestResult {
    // Unpublish only this disposable fixture; preserve all original native facts.
    connection.batch_execute("DROP TRIGGER snapshot_publications_are_immutable_delete")?;
    sql_query("DELETE FROM snapshot_publications WHERE snapshot_key=?")
        .bind::<Text, _>(fixture.snapshot.as_str())
        .execute(connection)?;
    Ok(())
}

fn publish(fixture: &Fixture, connection: &mut SqliteConnection) -> diesel::QueryResult<usize> {
    sql_query("INSERT INTO snapshot_publications(catalog_key,document_key,interpretation_key,snapshot_key) SELECT catalog_key,document_key,interpretation_key,snapshot_key FROM catalog_snapshots WHERE snapshot_key=?")
        .bind::<Text,_>(fixture.snapshot.as_str()).execute(connection)
}

#[allow(clippy::expect_used)]
fn assert_generic_copies_are_rejected(
    format: CatalogDocumentFormat,
    source: &str,
    pending: bool,
) -> TestResult {
    let fixture = Fixture::new(format, source)?;
    let mut connection = fixture.connection()?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;
    let declarations = app::explain_relationships(&fixture.database)?;
    let columns = sql_query("SELECT projection.name FROM pragma_table_info('relationship_assertion_explanations') AS projection JOIN pragma_table_xinfo('relationship_assertions') AS stored USING(name) WHERE projection.name<>'assertion_key' AND stored.hidden=0 ORDER BY projection.cid")
        .load::<Column>(&mut connection)?
        .into_iter().map(|column| column.name).collect::<Vec<_>>().join(",");
    if pending {
        remove_publication(&fixture, &mut connection)?;
    }
    let mut copy_queries = vec![format!(
        "INSERT INTO relationship_assertions(assertion_key,{columns}) SELECT ?,{columns} FROM relationship_assertion_explanations WHERE assertion_key=?"
    )];
    if matches!(format, CatalogDocumentFormat::NoIntroDatabase(_)) {
        // A forbidden native shape is not sufficient: a copied source field
        // must also be rejected when endpoints are otherwise-valid root sets.
        copy_queries.push("INSERT INTO relationship_assertions(assertion_key,relation_type,origin,source_snapshot_key,source_field,source_line,source_column,subject_kind,subject_set_id,source_subject_a,target_kind,source_target_a) SELECT ?,explanation.relation_type,explanation.origin,explanation.source_snapshot_key,explanation.source_field,explanation.source_line,explanation.source_column,'catalog_set',archive.set_id,sets.set_name,'catalog_set',explanation.source_target_a FROM relationship_assertion_explanations AS explanation JOIN no_intro_archive_descriptions AS archive ON archive.archive_id=explanation.source_subject_c JOIN catalog_sets AS sets ON sets.set_id=archive.set_id WHERE explanation.assertion_key=?".into());
    }
    for declaration in &declarations {
        for copy_query in &copy_queries {
            let copied = sql_query(copy_query)
                .bind::<Text, _>(format!("copy:{}", declaration.assertion_key.as_str()))
                .bind::<Text, _>(declaration.assertion_key.as_str())
                .execute(&mut connection);
            let error = copied.expect_err("a native declaration acquired a generic second owner");
            assert!(
                error
                    .to_string()
                    .contains("relationship assertions are immutable"),
                "wrong guard rejected native copy: {error}"
            );
        }
    }
    assert_eq!(
        sql_query("SELECT count(*) AS count FROM relationship_assertions")
            .get_result::<Count>(&mut connection)?
            .count,
        0
    );
    if pending {
        publish(&fixture, &mut connection)?;
    }
    assert_eq!(app::explain_relationships(&fixture.database)?, declarations);
    Ok(())
}

#[test]
fn published_software_declarations_cannot_acquire_generic_copies() -> TestResult {
    assert_generic_copies_are_rejected(CatalogDocumentFormat::MameSoftwareListXml, SOFTWARE, false)
}

#[test]
fn pending_software_declarations_cannot_acquire_generic_copies() -> TestResult {
    assert_generic_copies_are_rejected(CatalogDocumentFormat::MameSoftwareListXml, SOFTWARE, true)
}

#[test]
fn published_dat_declarations_cannot_acquire_generic_copies() -> TestResult {
    assert_generic_copies_are_rejected(
        CatalogDocumentFormat::NoIntroDat(NoIntroDatMode::V4Compatible),
        NO_INTRO_DAT,
        false,
    )
}

#[test]
fn pending_dat_declarations_cannot_acquire_generic_copies() -> TestResult {
    assert_generic_copies_are_rejected(
        CatalogDocumentFormat::NoIntroDat(NoIntroDatMode::V4Compatible),
        NO_INTRO_DAT,
        true,
    )
}

#[test]
fn published_database_archive_references_cannot_acquire_generic_copies() -> TestResult {
    for mode in [
        NoIntroDatabaseMode::ObservedCompatible,
        NoIntroDatabaseMode::NullRecoveryCompatible,
    ] {
        assert_generic_copies_are_rejected(
            CatalogDocumentFormat::NoIntroDatabase(mode),
            NO_INTRO_DATABASE,
            false,
        )?;
    }
    Ok(())
}

#[test]
fn pending_database_archive_references_cannot_acquire_generic_copies() -> TestResult {
    for mode in [
        NoIntroDatabaseMode::ObservedCompatible,
        NoIntroDatabaseMode::NullRecoveryCompatible,
    ] {
        assert_generic_copies_are_rejected(
            CatalogDocumentFormat::NoIntroDatabase(mode),
            NO_INTRO_DATABASE,
            true,
        )?;
    }
    Ok(())
}

fn register_test_identity(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    kind: &str,
) -> TestResult<i64> {
    let identity = sql_query("INSERT INTO catalog_relationships(assertion_key,origin,snapshot_key) VALUES (?,'source',?) RETURNING relationship_id AS count")
        .bind::<Text,_>(uuid::Uuid::new_v4().to_string())
        .bind::<Text,_>(snapshot.as_str())
        .get_result::<Count>(connection)?.count;
    sql_query("INSERT INTO reported_catalog_relationships(relationship_id,source_reference_kind) VALUES (?,?)")
        .bind::<BigInt,_>(identity).bind::<Text,_>(kind).execute(connection)?;
    Ok(identity)
}

fn insert_test_parent(
    connection: &mut SqliteConnection,
    table: &str,
    owner: i64,
    identity: i64,
) -> diesel::QueryResult<usize> {
    let query = match table {
        "software_clone_links" => {
            "INSERT INTO software_clone_links(set_id,relationship_id,target_name) VALUES (?,?,'new-parent')"
        }
        "no_intro_dat_set_links" => {
            "INSERT INTO no_intro_dat_set_links(set_id,relationship_id,target_literal,link_kind) VALUES (?,?,'new-parent','cloneof')"
        }
        _ => unreachable!("test owner table must be one of the two bounded native families"),
    };
    sql_query(query)
        .bind::<BigInt, _>(owner)
        .bind::<BigInt, _>(identity)
        .execute(connection)
}

#[allow(clippy::expect_used)]
fn assert_native_guard(result: diesel::QueryResult<usize>, message: &str) {
    let error = result.expect_err("native ownership guard accepted invalid state");
    assert!(
        error.to_string().contains(message),
        "wrong native guard rejected state: {error}"
    );
}

fn assert_new_native_owner_guards(
    format: CatalogDocumentFormat,
    source: &str,
    table: &str,
    kind: &str,
) -> TestResult {
    // The first child has no cloneof link; another actual owner already uses one.
    let fixture = Fixture::new(format, &source.replace(" cloneof=\"missing\"", ""))?;
    let original = app::explain_relationships(&fixture.database)?;
    let mut connection = fixture.connection()?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;
    let owner = sql_query(
        "SELECT set_id AS count FROM catalog_sets WHERE set_name='child' ORDER BY set_id LIMIT 1",
    )
    .get_result::<Count>(&mut connection)?
    .count;
    let used = sql_query("SELECT reported.relationship_id AS count FROM reported_catalog_relationships AS reported JOIN catalog_relationships AS registry USING(relationship_id) WHERE registry.snapshot_key=? AND reported.source_reference_kind=?")
        .bind::<Text,_>(fixture.snapshot.as_str()).bind::<Text,_>(kind).get_result::<Count>(&mut connection)?.count;
    remove_publication(&fixture, &mut connection)?;

    connection.batch_execute("SAVEPOINT native_owner_cases")?;
    let wrong_kind = register_test_identity(&mut connection, &fixture.snapshot, "mame_cloneof")?;
    assert_native_guard(
        insert_test_parent(&mut connection, table, owner, wrong_kind),
        "unused source identity",
    );
    let unused = register_test_identity(&mut connection, &fixture.snapshot, kind)?;
    assert_native_guard(
        insert_test_parent(&mut connection, table, 999_999, unused),
        "unused source identity",
    );
    assert_native_guard(
        insert_test_parent(&mut connection, table, owner, used),
        "unused source identity",
    );
    assert_eq!(
        insert_test_parent(&mut connection, table, owner, unused)?,
        1,
        "the same actual owner must accept an unused matching identity while pending"
    );
    connection.batch_execute("ROLLBACK TO native_owner_cases; RELEASE native_owner_cases")?;

    connection.batch_execute("SAVEPOINT native_orphan_case")?;
    register_test_identity(&mut connection, &fixture.snapshot, kind)?;
    assert_native_guard(
        publish(&fixture, &mut connection),
        "reported source relationships require complete native identity ownership",
    );
    connection.batch_execute("ROLLBACK TO native_orphan_case; RELEASE native_orphan_case")?;
    assert_eq!(
        publish(&fixture, &mut connection)?,
        1,
        "complete native ownership must publish after the orphan is rolled back"
    );

    // Bypass only identity insertion seals to independently exercise the native
    // late-child seal with a fresh, matching, otherwise-unused source identity.
    connection.batch_execute("SAVEPOINT native_late_case; DROP TRIGGER catalog_relationships_insert_guard; DROP TRIGGER reported_catalog_relationships_insert_guard")?;
    let late = register_test_identity(&mut connection, &fixture.snapshot, kind)?;
    assert_native_guard(
        insert_test_parent(&mut connection, table, owner, late),
        "unused source identity",
    );
    connection.batch_execute("ROLLBACK TO native_late_case; RELEASE native_late_case")?;
    assert_eq!(app::explain_relationships(&fixture.database)?, original);
    Ok(())
}

#[test]
fn software_owner_guards_reject_wrong_kinds_reuse_orphans_and_late_children() -> TestResult {
    assert_new_native_owner_guards(
        CatalogDocumentFormat::MameSoftwareListXml,
        SOFTWARE,
        "software_clone_links",
        "software_cloneof",
    )
}

#[test]
fn dat_owner_guards_reject_wrong_kinds_reuse_orphans_and_late_children() -> TestResult {
    assert_new_native_owner_guards(
        CatalogDocumentFormat::NoIntroDat(NoIntroDatMode::V4Compatible),
        NO_INTRO_DAT,
        "no_intro_dat_set_links",
        "no_intro_dat_cloneof",
    )
}

fn assert_review_backup_and_rollback(
    format: CatalogDocumentFormat,
    source: &str,
    broken: &str,
    relation_type: RelationshipType,
) -> TestResult {
    let fixture = Fixture::new(format, source)?;
    let original = app::explain_relationships(&fixture.database)?;
    let keys = original
        .iter()
        .filter(|row| row.claim.relation_type == relation_type)
        .take(2)
        .map(|row| row.assertion_key.clone())
        .collect::<Vec<_>>();
    assert_eq!(keys.len(), 2);
    let candidate = app::record_relationship(
        &fixture.database,
        &RelationshipClaim {
            relation_type: RelationshipType::CatalogCorrection,
            subject: RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new(
                "native-test",
                "candidate",
            )),
            target: RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new(
                "native-test",
                "declaration",
            )),
            origin: RelationshipOrigin::DerivedCandidate {
                rule: mame_coalesce::domain::RelationshipRule::new(
                    "native-review",
                    "v1",
                    "Native review witness",
                )?,
                supporting_assertions: keys.clone(),
            },
            evidence: RelationshipEvidence::Rationale {
                reason: "Check actual issued native keys, not synthetic names".into(),
            },
        },
    )?;
    for key in &keys {
        app::review_relationship(
            &fixture.database,
            key,
            &RelationshipReview {
                decision: RelationshipReviewDecision::Accepted,
                note: "Source declaration checked".into(),
                superseded_by: None,
            },
        )?;
    }
    assert_eq!(
        app::import_catalog(&fixture.database, &fixture.request)?
            .snapshot_key
            .as_ref(),
        Some(&fixture.snapshot)
    );
    let expected = app::explain_relationships(&fixture.database)?;
    for key in &keys {
        let reviewed = expected
            .iter()
            .find(|row| &row.assertion_key == key)
            .ok_or("native key lost")?;
        assert_eq!(reviewed.review_history.len(), 1);
    }
    assert!(expected.iter().any(|row| row.assertion_key == candidate && matches!(&row.claim.origin,
        RelationshipOrigin::DerivedCandidate { supporting_assertions, .. } if supporting_assertions == &keys)));
    std::fs::write(&fixture.request.document_path, broken)?;
    let failed = app::import_catalog(&fixture.database, &fixture.request)?;
    assert_eq!(failed.status, CatalogImportStatus::Failed);
    assert!(failed.snapshot_key.is_none());
    assert_eq!(app::explain_relationships(&fixture.database)?, expected);
    std::fs::remove_file(&fixture.request.document_path)?;
    let backup = Utf8PathBuf::try_from(fixture.directory.path().join("catalog.backup"))?;
    let restored_path = Utf8PathBuf::try_from(fixture.directory.path().join("restored.sqlite"))?;
    drop(fixture.database);
    create_backup(&fixture.database_path, &backup)?;
    restore_backup(&backup, &restored_path, RestorePolicy::CreateNew)?;
    let restored = Database::open(&restored_path)?;
    assert_eq!(app::explain_relationships(&restored)?, expected);
    drop(restored);
    assert!(check_integrity(&restored_path)?.is_clean());
    Ok(())
}

#[test]
fn logiqx_native_keys_survive_reviews_backup_and_late_eof_failure() -> TestResult {
    assert_review_backup_and_rollback(
        CatalogDocumentFormat::Logiqx(mame_coalesce::logiqx::LogiqxMode::ObservedCompatible),
        LOGIQX,
        "<datafile><game name='late' cloneof='missing'><description>Late</description><rom name='late.bin' merge='lost'/></game>",
        RelationshipType::SourceMerge,
    )
}

#[test]
fn cmp_native_keys_survive_reviews_backup_and_late_syntax_failure() -> TestResult {
    assert_review_backup_and_rollback(
        CatalogDocumentFormat::ClrMamePro,
        CMP,
        "game ( name late cloneof missing rom ( name late.bin merge lost ) ) game (",
        RelationshipType::SourceMerge,
    )
}

#[test]
fn software_native_keys_survive_reviews_backup_and_late_eof_failure() -> TestResult {
    assert_review_backup_and_rollback(
        CatalogDocumentFormat::MameSoftwareListXml,
        SOFTWARE,
        SOFTWARE.trim_end_matches("</softwarelists>"),
        RelationshipType::SourceParentClone,
    )
}

#[test]
fn dat_native_keys_survive_reviews_backup_and_late_eof_failure() -> TestResult {
    assert_review_backup_and_rollback(
        CatalogDocumentFormat::NoIntroDat(NoIntroDatMode::V4Compatible),
        NO_INTRO_DAT,
        NO_INTRO_DAT.trim_end_matches("</datafile>"),
        RelationshipType::SourceParentClone,
    )
}

#[test]
fn database_archive_keys_survive_reviews_backup_and_late_eof_failure() -> TestResult {
    assert_review_backup_and_rollback(
        CatalogDocumentFormat::NoIntroDatabase(NoIntroDatabaseMode::ObservedCompatible),
        NO_INTRO_DATABASE,
        NO_INTRO_DATABASE.trim_end_matches("</datafile>"),
        RelationshipType::SourceParentClone,
    )
}

#[test]
fn archive_targets_preserve_empty_leading_zero_and_repeated_numbers_without_resolution()
-> TestResult {
    let fixture = Fixture::new(
        CatalogDocumentFormat::NoIntroDatabase(NoIntroDatabaseMode::ObservedCompatible),
        NO_INTRO_DATABASE,
    )?;
    let mut parents = Vec::new();
    let mut merges = Vec::new();
    let mut owner_ids = std::collections::BTreeSet::new();
    for explanation in app::explain_relationships(&fixture.database)? {
        let RelationshipEndpoint::NoIntroArchive {
            snapshot,
            archive_id,
        } = explanation.claim.subject
        else {
            return Err("archive reference subject lost its native archive owner".into());
        };
        assert_eq!(snapshot, fixture.snapshot);
        owner_ids.insert(archive_id);
        let RelationshipEndpoint::NoIntroArchiveReference { snapshot, literal } =
            explanation.claim.target
        else {
            return Err("archive literal was resolved or coerced to a set name".into());
        };
        assert_eq!(snapshot, fixture.snapshot);
        let field = explanation.source_field.ok_or("archive field absent")?;
        assert_eq!(
            explanation.claim.evidence,
            RelationshipEvidence::ArchiveReference {
                declared_archive_reference: literal.clone(),
                source_field: field.clone(),
            }
        );
        assert!(explanation.source_location.is_some());
        assert_eq!(
            explanation
                .source
                .ok_or("native provenance absent")?
                .declared_version
                .as_deref(),
            Some("native-v1")
        );
        match field.as_str() {
            "archive_clone" => parents.push(literal),
            "archive_mergeof" => merges.push(literal),
            _ => return Err("unexpected archive reference kind".into()),
        }
    }
    parents.sort();
    merges.sort();
    assert_eq!(parents, ["", "0007", "0007"]);
    assert_eq!(merges, ["", "7"]);
    assert_eq!(
        owner_ids.len(),
        3,
        "repeated numbers/names must not fan out or collapse actual owners; P is not a target"
    );
    Ok(())
}

#[test]
fn archive_explanations_derive_exact_field_order_and_location_from_positions() -> TestResult {
    let fixture = Fixture::new(
        CatalogDocumentFormat::NoIntroDatabase(NoIntroDatabaseMode::ObservedCompatible),
        NO_INTRO_DATABASE,
    )?;
    let mut connection = fixture.connection()?;
    let positions = sql_query(
        "SELECT registry.assertion_key, 'archive_clone' AS source_field, position.source_order, \
                0 AS is_quoted, position.source_line, position.source_column \
         FROM catalog_relationships registry JOIN no_intro_archive_clone_links link USING(relationship_id) \
         JOIN no_intro_archive_field_positions position ON position.archive_id=link.archive_id AND position.field_kind=30 \
         UNION ALL \
         SELECT registry.assertion_key, 'archive_mergeof', position.source_order, 0, position.source_line, position.source_column \
         FROM catalog_relationships registry JOIN no_intro_archive_merge_links link USING(relationship_id) \
         JOIN no_intro_archive_field_positions position ON position.archive_id=link.archive_id AND position.field_kind=31"
    ).load::<FieldPosition>(&mut connection)?;
    let explanations = app::explain_relationships(&fixture.database)?;
    assert_eq!(positions.len(), 5);
    for position in positions {
        let explanation = explanations
            .iter()
            .find(|row| row.assertion_key.as_str() == position.assertion_key)
            .ok_or("native archive key missing")?;
        assert_eq!(
            explanation.source_field.as_deref(),
            Some(position.source_field.as_str())
        );
        let location = explanation
            .source_location
            .ok_or("native position location missing")?;
        assert_eq!(
            (location.line, location.column),
            (position.source_line, position.source_column)
        );
        assert_eq!(
            position.source_order,
            if position.source_field == "archive_clone" {
                1
            } else {
                2
            }
        );
    }
    Ok(())
}

#[test]
fn software_clone_endpoints_keep_list_names_separate_from_item_names() -> TestResult {
    let fixture = Fixture::new(CatalogDocumentFormat::MameSoftwareListXml, SOFTWARE)?;
    let mut literals = std::collections::BTreeMap::new();
    for explanation in app::explain_relationships(&fixture.database)? {
        let RelationshipEndpoint::CatalogRecord(subject) = explanation.claim.subject else {
            return Err("software subject is not a catalog record".into());
        };
        let RelationshipEndpoint::CatalogRecord(target) = explanation.claim.target else {
            return Err("software target is not a list-local reference".into());
        };
        let (list, item): (String, String) = serde_json::from_str(subject.key.as_str())?;
        let (target_list, parent): (String, String) = serde_json::from_str(target.key.as_str())?;
        assert_eq!(item, "child");
        assert_eq!(target_list, list);
        assert!(subject.owner_set_id.is_some());
        assert!(target.owner_set_id.is_none());
        assert_eq!(
            explanation.claim.evidence,
            RelationshipEvidence::SoftwareClone {
                list_name: list.clone(),
                target_item_name: parent.clone()
            }
        );
        literals.insert(list, parent);
    }
    assert_eq!(
        literals,
        std::collections::BTreeMap::from([
            ("first".into(), "missing".into()),
            ("second".into(), String::new())
        ])
    );
    Ok(())
}

#[test]
fn dat_native_parent_endpoints_preserve_empty_and_leading_zero_ids() -> TestResult {
    let fixture = Fixture::new(
        CatalogDocumentFormat::NoIntroDat(NoIntroDatMode::V4Compatible),
        NO_INTRO_DAT,
    )?;
    let mut names = Vec::new();
    let mut ids = Vec::new();
    for explanation in app::explain_relationships(&fixture.database)? {
        let RelationshipEndpoint::CatalogRecord(subject) = explanation.claim.subject else {
            return Err("DAT parent subject has no actual catalog set".into());
        };
        match explanation.claim.target {
            RelationshipEndpoint::CatalogRecord(target) => {
                assert_eq!(explanation.source_field.as_deref(), Some("cloneof"));
                assert!(target.owner_set_id.is_none());
                names.push(target.key.as_str().to_owned());
            }
            RelationshipEndpoint::NoIntroDatIdReference {
                snapshot,
                declaring_set,
                declared_id,
            } => {
                assert_eq!(snapshot, fixture.snapshot);
                assert_eq!(Some(declaring_set), subject.owner_set_id);
                assert_eq!(explanation.source_field.as_deref(), Some("cloneofid"));
                assert_eq!(
                    explanation.claim.evidence,
                    RelationshipEvidence::SourceFieldReference {
                        source_field: "cloneofid".into(),
                        target_name: declared_id.clone()
                    }
                );
                ids.push(declared_id);
            }
            _ => return Err("DAT parent literal was coerced into another endpoint".into()),
        }
        assert!(explanation.source_location.is_some());
    }
    names.sort();
    ids.sort();
    assert_eq!(names, ["", "missing"]);
    assert_eq!(ids, ["", "0007", "7"]);
    Ok(())
}

#[test]
fn native_declarations_have_one_value_and_provenance_owner() -> TestResult {
    let fixture = Fixture::new(CatalogDocumentFormat::ClrMamePro, CMP)?;
    let mut connection = fixture.connection()?;
    for table in ["logiqx_rom_claims", "logiqx_disk_claims", "cmp_rom_claims"] {
        let count = sql_query(format!(
            "SELECT count(*) AS count FROM pragma_table_info('{table}') WHERE name='merge_name'"
        ))
        .get_result::<Count>(&mut connection)?
        .count;
        assert_eq!(count, 0, "{table} duplicates the native merge literal");
    }
    for table in [
        "clrmamepro_set_links",
        "clrmamepro_rom_merges",
        "software_clone_links",
        "no_intro_dat_set_links",
        "no_intro_archive_clone_markers",
        "no_intro_archive_clone_links",
        "no_intro_archive_merge_links",
    ] {
        let count = sql_query(format!("SELECT count(*) AS count FROM pragma_table_info('{table}') WHERE name IN ('source_line','source_column','source_order','is_quoted','source_field')"))
            .get_result::<Count>(&mut connection)?.count;
        assert_eq!(count, 0, "{table} duplicates CMP field provenance");
    }
    let sample_table = sql_query("SELECT count(*) AS count FROM sqlite_schema WHERE type='table' AND name='cmp_sample_parent_links'")
        .get_result::<Count>(&mut connection)?.count;
    assert_eq!(sample_table, 0, "CMP sampleof has two value owners");
    for (table, column) in [
        ("software_items", "clone_of"),
        ("no_intro_dat_games", "cloneof_text"),
        ("no_intro_dat_games", "cloneofid_text"),
    ] {
        let count = sql_query(format!(
            "SELECT count(*) AS count FROM pragma_table_info('{table}') WHERE name='{column}'"
        ))
        .get_result::<Count>(&mut connection)?
        .count;
        assert_eq!(
            count, 0,
            "{table}.{column} duplicates a native parent literal"
        );
    }
    Ok(())
}

fn assert_published_native_facts_are_sealed(
    format: CatalogDocumentFormat,
    source: &str,
    tables: &[(&str, &str)],
) -> TestResult {
    let fixture = Fixture::new(format, source)?;
    let mut connection = fixture.connection()?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;
    let before = app::explain_relationships(&fixture.database)?;
    for (table, literal_column) in tables {
        let count = sql_query(format!("SELECT count(*) AS count FROM {table}"))
            .get_result::<Count>(&mut connection)?
            .count;
        assert!(count > 0, "guard witness needs an actual {table} owner");
        let columns = sql_query(format!(
            "SELECT name FROM pragma_table_xinfo('{table}') WHERE hidden=0 ORDER BY cid"
        ))
        .load::<Column>(&mut connection)?
        .into_iter()
        .map(|column| column.name)
        .collect::<Vec<_>>()
        .join(",");
        for query in [
            format!("INSERT OR REPLACE INTO {table}({columns}) SELECT {columns} FROM {table}"),
            format!("UPDATE {table} SET {literal_column}='changed'"),
            format!("DELETE FROM {table}"),
        ] {
            assert!(
                sql_query(&query).execute(&mut connection).is_err(),
                "native mutation accepted: {query}"
            );
        }
    }
    assert_eq!(app::explain_relationships(&fixture.database)?, before);
    Ok(())
}

#[test]
fn logiqx_actual_owners_and_identities_reject_replacement_and_mutation() -> TestResult {
    assert_published_native_facts_are_sealed(
        CatalogDocumentFormat::Logiqx(mame_coalesce::logiqx::LogiqxMode::ObservedCompatible),
        LOGIQX,
        &[
            ("logiqx_set_links", "target_name"),
            ("logiqx_file_merges", "merge_name"),
        ],
    )
}

#[test]
fn cmp_actual_owners_and_identities_reject_replacement_and_mutation() -> TestResult {
    assert_published_native_facts_are_sealed(
        CatalogDocumentFormat::ClrMamePro,
        CMP,
        &[
            ("clrmamepro_set_links", "target_name"),
            ("clrmamepro_rom_merges", "merge_name"),
        ],
    )
}

#[test]
fn software_actual_owners_reject_replacement_and_mutation_without_pragmas() -> TestResult {
    assert_published_native_facts_are_sealed(
        CatalogDocumentFormat::MameSoftwareListXml,
        SOFTWARE,
        &[("software_clone_links", "target_name")],
    )
}

#[test]
fn dat_actual_owners_reject_replacement_and_mutation_without_pragmas() -> TestResult {
    assert_published_native_facts_are_sealed(
        CatalogDocumentFormat::NoIntroDat(NoIntroDatMode::V4Compatible),
        NO_INTRO_DAT,
        &[("no_intro_dat_set_links", "target_literal")],
    )
}

#[test]
fn database_archive_owners_reject_replacement_and_mutation_without_pragmas() -> TestResult {
    assert_published_native_facts_are_sealed(
        CatalogDocumentFormat::NoIntroDatabase(NoIntroDatabaseMode::ObservedCompatible),
        NO_INTRO_DATABASE,
        &[
            ("no_intro_archive_clone_links", "declared_target_number"),
            ("no_intro_archive_merge_links", "declared_mergeof"),
            ("no_intro_archive_clone_markers", "marker"),
        ],
    )
}

#[test]
fn orphan_database_archive_identity_prevents_publication_without_pragmas() -> TestResult {
    let fixture = Fixture::new(
        CatalogDocumentFormat::NoIntroDatabase(NoIntroDatabaseMode::ObservedCompatible),
        NO_INTRO_DATABASE,
    )?;
    let mut connection = fixture.connection()?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;
    remove_publication(&fixture, &mut connection)?;
    connection.batch_execute("SAVEPOINT orphan_archive")?;
    register_test_identity(
        &mut connection,
        &fixture.snapshot,
        "no_intro_database_archive_clone",
    )?;
    assert_native_guard(
        publish(&fixture, &mut connection),
        "reported source relationships require complete native identity ownership",
    );
    connection.batch_execute("ROLLBACK TO orphan_archive; RELEASE orphan_archive")?;
    assert_eq!(
        publish(&fixture, &mut connection)?,
        1,
        "rollback restores complete publishable native owners"
    );
    Ok(())
}

#[test]
fn database_archive_links_independently_reject_fresh_children_after_eof_or_publication()
-> TestResult {
    for (table, literal_column, reference_kind) in [
        (
            "no_intro_archive_clone_links",
            "declared_target_number",
            "no_intro_database_archive_clone",
        ),
        (
            "no_intro_archive_merge_links",
            "declared_mergeof",
            "no_intro_database_archive_mergeof",
        ),
    ] {
        for pending in [false, true] {
            let fixture = Fixture::new(
                CatalogDocumentFormat::NoIntroDatabase(NoIntroDatabaseMode::ObservedCompatible),
                NO_INTRO_DATABASE,
            )?;
            let original = app::explain_relationships(&fixture.database)?;
            let mut connection = fixture.connection()?;
            connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;
            let archive = sql_query(format!(
                "SELECT archive_id AS count FROM {table} ORDER BY archive_id LIMIT 1"
            ))
            .get_result::<Count>(&mut connection)?
            .count;
            if pending {
                remove_publication(&fixture, &mut connection)?;
            }
            // Bypass registry insertion and the existing link's delete seal in this
            // disposable fixture. Keep the actual archive and matching position.
            connection.batch_execute(&format!("SAVEPOINT late_archive; DROP TRIGGER catalog_relationships_insert_guard; DROP TRIGGER reported_catalog_relationships_insert_guard; DROP TRIGGER {table}_immutable_delete"))?;
            if !pending {
                // Publication must reject the fresh child independently of EOF.
                connection.batch_execute(
                    "DROP TRIGGER no_intro_database_parse_counts_immutable_delete",
                )?;
                assert_eq!(
                    sql_query("DELETE FROM no_intro_database_parse_counts WHERE snapshot_key=?")
                        .bind::<Text, _>(fixture.snapshot.as_str())
                        .execute(&mut connection)?,
                    1
                );
            }
            assert_eq!(sql_query("SELECT COUNT(*) AS count FROM no_intro_database_parse_counts WHERE snapshot_key=?").bind::<Text,_>(fixture.snapshot.as_str()).get_result::<Count>(&mut connection)?.count, i64::from(pending));
            assert_eq!(
                sql_query(
                    "SELECT COUNT(*) AS count FROM snapshot_publications WHERE snapshot_key=?"
                )
                .bind::<Text, _>(fixture.snapshot.as_str())
                .get_result::<Count>(&mut connection)?
                .count,
                i64::from(!pending)
            );
            sql_query(format!("DELETE FROM {table} WHERE archive_id=?"))
                .bind::<BigInt, _>(archive)
                .execute(&mut connection)?;
            let identity =
                register_test_identity(&mut connection, &fixture.snapshot, reference_kind)?;
            assert_native_guard(sql_query(format!("INSERT INTO {table}(archive_id,relationship_id,{literal_column}) VALUES (?,?,'new-parent')")).bind::<BigInt,_>(archive).bind::<BigInt,_>(identity).execute(&mut connection), "unused reported identity");
            connection.batch_execute("ROLLBACK TO late_archive; RELEASE late_archive")?;
            if pending {
                assert_eq!(publish(&fixture, &mut connection)?, 1);
            }
            assert_eq!(app::explain_relationships(&fixture.database)?, original);
        }
    }
    Ok(())
}

#[test]
fn database_archive_readiness_requires_its_actual_attribute_position() -> TestResult {
    let fixture = Fixture::new(
        CatalogDocumentFormat::NoIntroDatabase(NoIntroDatabaseMode::ObservedCompatible),
        NO_INTRO_DATABASE,
    )?;
    let mut connection = fixture.connection()?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;
    let keys = app::explain_relationships(&fixture.database)?
        .into_iter()
        .map(|row| row.assertion_key)
        .collect::<Vec<_>>();
    assert_eq!(keys.len(), 5);
    let readiness = concat!(
        "SELECT is_published AS count FROM (WITH requested(assertion_key) AS (VALUES (?)) ",
        include_str!("../src/storage/db/relationship_readiness.sql"),
        ")"
    );
    for key in &keys {
        assert_eq!(
            sql_query(readiness)
                .bind::<Text, _>(key.as_str())
                .get_result::<Count>(&mut connection)?
                .count,
            1
        );
    }
    // Corrupt only this isolated fixture to exercise readiness independently
    // of the native position's normal immutability seal.
    connection.batch_execute("DROP TRIGGER no_intro_archive_field_positions_immutable_delete; DELETE FROM no_intro_archive_field_positions WHERE field_kind IN (30,31)")?;
    for key in &keys {
        assert_eq!(
            sql_query(readiness)
                .bind::<Text, _>(key.as_str())
                .get_result::<Count>(&mut connection)?
                .count,
            0,
            "registry identity alone must not publish a positionless declaration"
        );
    }
    Ok(())
}

fn assert_readiness_checks_native_owner_edition(
    format: CatalogDocumentFormat,
    source: &str,
    expected_keys: usize,
) -> TestResult {
    let fixture = Fixture::new(format, source)?;
    let mut connection = fixture.connection()?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;
    let owner = sql_query("SELECT set_id AS count FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) WHERE snapshot_key=? ORDER BY set_id LIMIT 1")
        .bind::<Text,_>(fixture.snapshot.as_str()).get_result::<Count>(&mut connection)?.count;
    let archives =
        sql_query("SELECT archive_id AS count FROM no_intro_archive_descriptions WHERE set_id=?")
            .bind::<BigInt, _>(owner)
            .load::<Count>(&mut connection)?;
    let keys = app::explain_relationships(&fixture.database)?
        .into_iter()
        .filter(|row| match &row.claim.subject {
            RelationshipEndpoint::CatalogRecord(record) => {
                record.owner_set_id.is_some_and(|id| id.as_i64() == owner)
            }
            RelationshipEndpoint::CatalogMediaEntry { .. } => matches!(&row.claim.target,
            RelationshipEndpoint::CatalogMergeReference { set_id, .. } if set_id.as_i64()==owner),
            RelationshipEndpoint::NoIntroArchive { archive_id, .. } => archives
                .iter()
                .any(|archive| archive.count == archive_id.as_i64()),
            _ => false,
        })
        .map(|row| row.assertion_key)
        .collect::<Vec<_>>();
    assert_eq!(
        keys.len(),
        expected_keys,
        "every native declaration on the actual owner must be checked"
    );
    let readiness = concat!(
        "SELECT is_published AS count FROM (WITH requested(assertion_key) AS (VALUES (?)) ",
        include_str!("../src/storage/db/relationship_readiness.sql"),
        ")"
    );
    for key in &keys {
        assert_eq!(
            sql_query(readiness)
                .bind::<Text, _>(key.as_str())
                .get_result::<Count>(&mut connection)?
                .count,
            1
        );
    }
    sql_query("INSERT INTO catalogs(catalog_key,source_key,display_name) SELECT 'corrupt-owner-edition',source_key,'Corrupt owner edition' FROM catalogs WHERE catalog_key=?")
        .bind::<Text,_>(fixture.request.catalog_key.as_str()).execute(&mut connection)?;
    sql_query("INSERT INTO catalog_snapshots(snapshot_key,catalog_key,document_key,interpretation_key,coverage_id) SELECT 'corrupt-owner-edition','corrupt-owner-edition',document_key,interpretation_key,coverage_id FROM catalog_snapshots WHERE snapshot_key=?")
        .bind::<Text,_>(fixture.snapshot.as_str()).execute(&mut connection)?;
    sql_query("INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) SELECT 'corrupt-owner-edition',groups.kind,0 FROM catalog_sets AS sets JOIN catalog_set_groups AS groups USING(set_group_id) WHERE sets.set_id=?")
        .bind::<BigInt,_>(owner).execute(&mut connection)?;
    // Bypass this one seal in the isolated fixture. The production readiness
    // query must independently inspect actual owners rather than trust the seal.
    connection.batch_execute("DROP TRIGGER catalog_sets_reject_replacement")?;
    sql_query("INSERT OR REPLACE INTO catalog_sets(set_id,set_group_id,source_element_kind,list_order,set_name,source_line,source_column) SELECT set_id,(SELECT set_group_id FROM catalog_set_groups WHERE snapshot_key='corrupt-owner-edition'),source_element_kind,0,set_name,source_line,source_column FROM catalog_sets WHERE set_id=?")
        .bind::<BigInt,_>(owner).execute(&mut connection)?;
    for key in &keys {
        assert_eq!(
            sql_query(readiness)
                .bind::<Text, _>(key.as_str())
                .get_result::<Count>(&mut connection)?
                .count,
            0,
            "registry edition alone must not make {key:?} ready"
        );
    }
    Ok(())
}

#[test]
fn logiqx_readiness_independently_verifies_native_owner_edition() -> TestResult {
    assert_readiness_checks_native_owner_edition(
        CatalogDocumentFormat::Logiqx(mame_coalesce::logiqx::LogiqxMode::ObservedCompatible),
        LOGIQX,
        6,
    )
}

#[test]
fn cmp_readiness_independently_verifies_native_owner_edition() -> TestResult {
    assert_readiness_checks_native_owner_edition(CatalogDocumentFormat::ClrMamePro, CMP, 4)
}

#[test]
fn software_readiness_independently_verifies_native_owner_edition() -> TestResult {
    assert_readiness_checks_native_owner_edition(
        CatalogDocumentFormat::MameSoftwareListXml,
        SOFTWARE,
        1,
    )
}

#[test]
fn dat_readiness_independently_verifies_native_owner_edition() -> TestResult {
    assert_readiness_checks_native_owner_edition(
        CatalogDocumentFormat::NoIntroDat(NoIntroDatMode::V4Compatible),
        NO_INTRO_DAT,
        2,
    )
}

#[test]
fn database_archive_readiness_independently_verifies_native_owner_edition() -> TestResult {
    assert_readiness_checks_native_owner_edition(
        CatalogDocumentFormat::NoIntroDatabase(NoIntroDatabaseMode::ObservedCompatible),
        NO_INTRO_DATABASE,
        4,
    )
}

#[test]
fn cmp_explanations_reuse_exact_keyword_order_quote_and_field_locations() -> TestResult {
    let fixture = Fixture::new(
        CatalogDocumentFormat::ClrMamePro,
        "GaMe (\n NaMe \"child\"\n ClOnEoF \"\"\n SaMpLeOf \"missing\"\n RoM ( NaMe \"first.bin\" SiZe 1 MeRgE \"\" )\n)",
    )?;
    let mut connection = fixture.connection()?;
    let positions = sql_query(
        "SELECT registry.assertion_key,position.source_field,position.source_order, \
                position.is_quoted,position.source_line,position.source_column \
         FROM catalog_relationships registry JOIN clrmamepro_set_links link USING(relationship_id) \
         JOIN cmp_set_field_positions position ON position.record_id=link.set_id \
          AND position.field_kind=CASE link.link_kind WHEN 'cloneof' THEN 1 ELSE 6 END \
         UNION ALL \
         SELECT registry.assertion_key,position.source_field,position.source_order, \
                position.is_quoted,position.source_line,position.source_column \
         FROM catalog_relationships registry JOIN clrmamepro_rom_merges declaration USING(relationship_id) \
         JOIN cmp_rom_field_positions position USING(occurrence_id) WHERE position.field_kind=6",
    ).load::<FieldPosition>(&mut connection)?;
    assert_eq!(positions.len(), 3);
    let explanations = app::explain_relationships(&fixture.database)?;
    for position in positions {
        let explanation = explanations
            .iter()
            .find(|row| row.assertion_key.as_str() == position.assertion_key)
            .ok_or("actual native position has no explanation")?;
        let location = explanation
            .source_location
            .ok_or("native field position lost")?;
        assert_eq!(
            (location.line, location.column),
            (position.source_line, position.source_column)
        );
        assert_eq!(position.is_quoted, 1);
        match position.source_field.as_str() {
            "ClOnEoF" => assert_eq!(position.source_order, 1),
            "SaMpLeOf" | "MeRgE" => assert_eq!(position.source_order, 2),
            field => return Err(format!("original field spelling lost: {field}").into()),
        }
    }
    Ok(())
}

fn assert_readiness_plan_is_keyed(connection: &mut SqliteConnection, key: &str) -> TestResult {
    const LOOKUP: &str = concat!(
        "WITH requested(assertion_key) AS (VALUES (?)) ",
        include_str!("../src/storage/db/relationship_readiness.sql")
    );
    let plan = sql_query(format!("EXPLAIN QUERY PLAN {LOOKUP}"))
        .bind::<Text, _>(key)
        .load::<QueryPlan>(connection)?;
    let details = plan.into_iter().map(|row| row.detail).collect::<Vec<_>>();
    for alias in [
        "registry",
        "identity",
        "link",
        "reference",
        "declaration",
        "position",
        "payload",
        "native",
        "sets",
        "occurrence",
        "mame_machine_links",
        "mame_device_references",
        "mame_rom_merges",
        "mame_disk_merges",
        "logiqx_set_links",
        "logiqx_device_references",
        "logiqx_file_merges",
        "clrmamepro_set_links",
        "clrmamepro_rom_merges",
        "software_clone_links",
        "no_intro_dat_set_links",
        "no_intro_archive_clone_links",
        "no_intro_archive_merge_links",
    ] {
        assert!(
            !details.iter().any(|line| line == &format!("SCAN {alias}")
                || line.starts_with(&format!("SCAN {alias} "))),
            "published native key must seek actual owners: {details:?}"
        );
    }
    Ok(())
}

fn add_unrelated_native_owners(fixture: &Fixture) -> TestResult {
    for (format, key, prefix, item, suffix) in [
        (
            CatalogDocumentFormat::MameListXml,
            "unrelated-mame",
            "<mame build='unrelated' mameconfig='10'>",
            "<machine name='SEEDNAME' cloneof='missing'><description>Unrelated</description><rom name='rom' size='1' merge='missing'/><disk name='disk' merge='missing'/></machine>",
            "</mame>",
        ),
        (
            CatalogDocumentFormat::Logiqx(mame_coalesce::logiqx::LogiqxMode::ObservedCompatible),
            "unrelated-logiqx",
            "<datafile>",
            "<game name='SEEDNAME' cloneof='missing'><description>Unrelated</description><rom name='rom' size='1' merge='missing'/><disk name='disk' merge='missing'/></game>",
            "</datafile>",
        ),
        (
            CatalogDocumentFormat::ClrMamePro,
            "unrelated-cmp",
            "",
            "game ( name SEEDNAME cloneof missing rom ( name rom size 1 merge missing ) ) ",
            "",
        ),
        (
            CatalogDocumentFormat::MameSoftwareListXml,
            "unrelated-software",
            "<softwarelist name='unrelated'>",
            "<software name='SEEDNAME' cloneof='missing'><description>Unrelated</description><year>2026</year><publisher>Test</publisher></software>",
            "</softwarelist>",
        ),
        (
            CatalogDocumentFormat::NoIntroDat(NoIntroDatMode::V4Compatible),
            "unrelated-dat",
            "<datafile><header><id>1</id><name>Unrelated</name><description>Unrelated</description><version>1</version></header>",
            "<game name='SEEDNAME' cloneof='missing' cloneofid='0007'><description>Unrelated</description><rom name='rom'/></game>",
            "</datafile>",
        ),
        (
            CatalogDocumentFormat::NoIntroDatabase(NoIntroDatabaseMode::ObservedCompatible),
            "unrelated-database",
            "<datafile>",
            "<game name='SEEDNAME'><archive number='0001' clone='0007' mergeof=''/></game>",
            "</datafile>",
        ),
    ] {
        let mut source = prefix.to_owned();
        for index in 0..100 {
            source.push_str(&item.replace("SEEDNAME", &format!("unrelated-{index}")));
        }
        source.push_str(suffix);
        let path = Utf8PathBuf::try_from(fixture.directory.path().join(format!("{key}.dat")))?;
        std::fs::write(&path, source)?;
        let mut request = fixture.request.clone();
        request.document_path = path;
        request.format = format;
        request.catalog_key = CatalogKey::new(key);
        request.catalog_display_name = key.into();
        let report = app::import_catalog(&fixture.database, &request)?;
        let diagnostics =
            sql_query("SELECT message AS name FROM import_diagnostics WHERE run_key=?")
                .bind::<Text, _>(report.run_key.to_string())
                .load::<Column>(&mut fixture.connection()?)?
                .into_iter()
                .map(|row| row.name)
                .collect::<Vec<_>>();
        assert_eq!(
            report.status,
            CatalogImportStatus::Succeeded,
            "{key}: {diagnostics:?}"
        );
    }
    Ok(())
}

fn assert_native_lookup_plans_are_keyed(
    format: CatalogDocumentFormat,
    source: &str,
    projection: &str,
) -> TestResult {
    let fixture = Fixture::new(format, source)?;
    let mut connection = fixture.connection()?;
    let actual_key = app::explain_relationships(&fixture.database)?
        .into_iter()
        .find(|row| matches!(row.claim.origin, RelationshipOrigin::SourceAssertion { .. }))
        .ok_or("actual native lookup key missing")?
        .assertion_key;
    add_unrelated_native_owners(&fixture)?;
    connection.batch_execute("ANALYZE")?;
    for (column, value) in [
        ("assertion_key", actual_key.as_str()),
        ("source_snapshot_key", fixture.snapshot.as_str()),
    ] {
        let plan = sql_query(format!(
            "EXPLAIN QUERY PLAN SELECT * FROM {projection} WHERE {column}=?"
        ))
        .bind::<Text, _>(value)
        .load::<QueryPlan>(&mut connection)?;
        let details = plan.into_iter().map(|row| row.detail).collect::<Vec<_>>();
        assert!(
            details
                .iter()
                .any(|line| line.starts_with("SEARCH registry ")),
            "{column} must seek real registry identity: {details:?}"
        );
        for alias in [
            "registry",
            "link",
            "reference",
            "declaration",
            "position",
            "owner",
            "payload",
            "rom_parent",
            "clone_parent",
            "logiqx_set_links",
            "software_clone_links",
            "no_intro_dat_set_links",
            "no_intro_archive_clone_links",
            "no_intro_archive_merge_links",
            "archive",
            "game",
            "export",
            "clone",
            "merge",
        ] {
            assert!(
                !details.iter().any(|line| line == &format!("SCAN {alias}")
                    || line.starts_with(&format!("SCAN {alias} "))),
                "{column} unexpectedly scans native catalog rows: {details:?}"
            );
        }
    }
    let inverse = sql_query("EXPLAIN QUERY PLAN SELECT * FROM reported_catalog_relationship_raw_owners WHERE snapshot_key=?")
        .bind::<Text,_>(fixture.snapshot.as_str()).load::<QueryPlan>(&mut connection)?;
    let details = inverse
        .into_iter()
        .map(|row| row.detail)
        .collect::<Vec<_>>();
    assert!(
        details
            .iter()
            .any(|line| line.starts_with("SEARCH groups ")),
        "inverse closure must seek the actual edition: {details:?}"
    );
    for alias in [
        "groups",
        "sets",
        "occurrence",
        "link",
        "reference",
        "declaration",
        "archive",
    ] {
        assert!(
            !details.iter().any(|line| line == &format!("SCAN {alias}")
                || line.starts_with(&format!("SCAN {alias} "))),
            "inverse closure must not scan unrelated native catalogs: {details:?}"
        );
    }
    assert_readiness_plan_is_keyed(&mut connection, actual_key.as_str())
}

#[test]
fn logiqx_native_queries_seek_actual_identity_and_edition_keys() -> TestResult {
    assert_native_lookup_plans_are_keyed(
        CatalogDocumentFormat::Logiqx(mame_coalesce::logiqx::LogiqxMode::ObservedCompatible),
        LOGIQX,
        "logiqx_cmp_source_relationships",
    )
}

#[test]
fn cmp_native_queries_seek_actual_identity_and_edition_keys() -> TestResult {
    assert_native_lookup_plans_are_keyed(
        CatalogDocumentFormat::ClrMamePro,
        CMP,
        "logiqx_cmp_source_relationships",
    )
}

#[test]
fn software_native_queries_seek_actual_identity_and_edition_keys() -> TestResult {
    assert_native_lookup_plans_are_keyed(
        CatalogDocumentFormat::MameSoftwareListXml,
        SOFTWARE,
        "software_dat_source_relationships",
    )
}

#[test]
fn dat_native_queries_seek_actual_identity_and_edition_keys() -> TestResult {
    assert_native_lookup_plans_are_keyed(
        CatalogDocumentFormat::NoIntroDat(NoIntroDatMode::V4Compatible),
        NO_INTRO_DAT,
        "software_dat_source_relationships",
    )
}

#[test]
fn database_archive_queries_seek_actual_identity_and_edition_keys() -> TestResult {
    assert_native_lookup_plans_are_keyed(
        CatalogDocumentFormat::NoIntroDatabase(NoIntroDatabaseMode::ObservedCompatible),
        NO_INTRO_DATABASE,
        "no_intro_database_source_relationships",
    )
}

#[test]
fn cmp_merge_readiness_requires_the_actual_rom_claim_kind() -> TestResult {
    let fixture = Fixture::new(CatalogDocumentFormat::ClrMamePro, CMP)?;
    let mut connection = fixture.connection()?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;
    let merge = app::explain_relationships(&fixture.database)?
        .into_iter()
        .find(|row| row.claim.relation_type == RelationshipType::SourceMerge)
        .ok_or("native CMP merge witness missing")?;
    let RelationshipEndpoint::CatalogMediaEntry {
        occurrence_id: occurrence,
        ..
    } = merge.claim.subject
    else {
        return Err("native CMP merge is not occurrence-owned".into());
    };
    let readiness = concat!(
        "SELECT is_published AS count FROM (WITH requested(assertion_key) AS (VALUES (?)) ",
        include_str!("../src/storage/db/relationship_readiness.sql"),
        ")"
    );
    assert_eq!(
        sql_query(readiness)
            .bind::<Text, _>(merge.assertion_key.as_str())
            .get_result::<Count>(&mut connection)?
            .count,
        1
    );
    connection.batch_execute("DROP TRIGGER asset_occurrences_are_immutable_update")?;
    sql_query("UPDATE asset_occurrences SET claim_kind='cmp_sample' WHERE occurrence_id=?")
        .bind::<BigInt, _>(occurrence.database_value())
        .execute(&mut connection)?;
    assert_eq!(
        sql_query(readiness)
            .bind::<Text, _>(merge.assertion_key.as_str())
            .get_result::<Count>(&mut connection)?
            .count,
        0,
        "a CMP sample occurrence cannot make a ROM merge identity ready"
    );
    Ok(())
}

fn assert_cmp_sampleof_only(literal: &str) -> TestResult {
    let fixture = Fixture::new(
        CatalogDocumentFormat::ClrMamePro,
        &format!("game ( name child sampleof \"{literal}\" )"),
    )?;
    let explanations = app::explain_relationships(&fixture.database)?;
    assert_eq!(explanations.len(), 1);
    let dependency = &explanations[0];
    assert_eq!(
        dependency.claim.relation_type,
        RelationshipType::RuntimeDependency
    );
    assert_eq!(dependency.source_field.as_deref(), Some("sampleof"));
    assert!(
        matches!(&dependency.claim.target, RelationshipEndpoint::CatalogRecord(record)
        if record.key.as_str()==literal && record.owner_set_id.is_none())
    );
    let mut connection = fixture.connection()?;
    let declared = sql_query(
        "SELECT count(*) AS count FROM cmp_set_declared_field_presence WHERE field_kind=1",
    )
    .get_result::<Count>(&mut connection)?
    .count;
    assert_eq!(
        declared, 0,
        "sampleof must not fabricate a cloneof declaration"
    );
    Ok(())
}

#[test]
fn cmp_sampleof_only_preserves_an_unresolved_parent_without_cloneof() -> TestResult {
    assert_cmp_sampleof_only("missing-parent")
}

#[test]
fn cmp_sampleof_only_preserves_an_empty_parent_without_cloneof() -> TestResult {
    assert_cmp_sampleof_only("")
}
