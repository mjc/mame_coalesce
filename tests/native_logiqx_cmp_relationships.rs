use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Text},
};
use mame_coalesce::{
    RestorePolicy,
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    check_integrity, create_backup,
    database::Database,
    domain::{
        CatalogKey, CatalogScope, ExternalRecordRef, PublishingSourceKey, RelationshipClaim,
        RelationshipEndpoint, RelationshipEvidence, RelationshipOrigin, RelationshipReview,
        RelationshipReviewDecision, RelationshipType, SnapshotKey,
    },
    restore_backup,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const LOGIQX: &str = r#"<datafile><game name="child" cloneof="missing-clone" romof="missing-rom" sampleof=""><description>Child</description><rom name="first.bin" size="1" merge="missing.bin"/><rom name="empty.bin" size="1" merge=""/><disk name="logical-disk" merge="missing-disk"/></game><game name="orphan"><description>Orphan</description><rom name="orphan.bin" size="1" merge="missing.bin"/></game></datafile>"#;
const CMP: &str = r#"game ( name "child" cloneof "missing-clone" sampleof "" rom ( name "first.bin" size 1 merge "missing.bin" ) rom ( name "empty.bin" size 1 merge "" ) ) game ( name "orphan" rom ( name "orphan.bin" size 1 merge "missing.bin" ) )"#;

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
        assert_eq!(report.status, CatalogImportStatus::Succeeded);
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
    assert_native_relationships(CatalogDocumentFormat::Logiqx, LOGIQX, 4, 7)
}

#[test]
fn cmp_parent_and_merge_declarations_have_once_issued_native_owners() -> TestResult {
    assert_native_relationships(CatalogDocumentFormat::ClrMamePro, CMP, 3, 5)
}

fn assert_review_backup_and_rollback(
    format: CatalogDocumentFormat,
    source: &str,
    broken: &str,
) -> TestResult {
    let fixture = Fixture::new(format, source)?;
    let original = app::explain_relationships(&fixture.database)?;
    let keys = original
        .iter()
        .filter(|row| row.claim.relation_type == RelationshipType::SourceMerge)
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
                rule_version: "native-review-v1".into(),
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
        CatalogDocumentFormat::Logiqx,
        LOGIQX,
        "<datafile><game name='late' cloneof='missing'><description>Late</description><rom name='late.bin' merge='lost'/></game>",
    )
}

#[test]
fn cmp_native_keys_survive_reviews_backup_and_late_syntax_failure() -> TestResult {
    assert_review_backup_and_rollback(
        CatalogDocumentFormat::ClrMamePro,
        CMP,
        "game ( name late cloneof missing rom ( name late.bin merge lost ) ) game (",
    )
}

#[test]
fn native_merges_have_no_duplicate_claim_columns_or_cmp_position_columns() -> TestResult {
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
    for table in ["clrmamepro_set_links", "clrmamepro_rom_merges"] {
        let count = sql_query(format!("SELECT count(*) AS count FROM pragma_table_info('{table}') WHERE name IN ('source_line','source_column','source_order','is_quoted','source_field')"))
            .get_result::<Count>(&mut connection)?.count;
        assert_eq!(count, 0, "{table} duplicates CMP field provenance");
    }
    let sample_table = sql_query("SELECT count(*) AS count FROM sqlite_schema WHERE type='table' AND name='cmp_sample_parent_links'")
        .get_result::<Count>(&mut connection)?.count;
    assert_eq!(sample_table, 0, "CMP sampleof has two value owners");
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
        CatalogDocumentFormat::Logiqx,
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

fn assert_readiness_checks_native_owner_edition(
    format: CatalogDocumentFormat,
    source: &str,
) -> TestResult {
    let fixture = Fixture::new(format, source)?;
    let mut connection = fixture.connection()?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;
    let owner = sql_query("SELECT set_id AS count FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) WHERE snapshot_key=? AND set_name='child'")
        .bind::<Text,_>(fixture.snapshot.as_str()).get_result::<Count>(&mut connection)?.count;
    let keys = app::explain_relationships(&fixture.database)?
        .into_iter()
        .filter(|row| match &row.claim.subject {
            RelationshipEndpoint::CatalogRecord(record) => {
                record.owner_set_id.is_some_and(|id| id.as_i64() == owner)
            }
            RelationshipEndpoint::CatalogMediaEntry { .. } => matches!(&row.claim.target,
            RelationshipEndpoint::CatalogMergeReference { set_id, .. } if set_id.as_i64()==owner),
            _ => false,
        })
        .map(|row| row.assertion_key)
        .collect::<Vec<_>>();
    assert!(
        keys.len() >= 4,
        "positive controls must cover native parents and media merges"
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
    sql_query("INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) VALUES ('corrupt-owner-edition','root',0)").execute(&mut connection)?;
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
    assert_readiness_checks_native_owner_edition(CatalogDocumentFormat::Logiqx, LOGIQX)
}

#[test]
fn cmp_readiness_independently_verifies_native_owner_edition() -> TestResult {
    assert_readiness_checks_native_owner_edition(CatalogDocumentFormat::ClrMamePro, CMP)
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
            CatalogDocumentFormat::Logiqx,
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

fn assert_native_lookup_plans_are_keyed(format: CatalogDocumentFormat, source: &str) -> TestResult {
    let fixture = Fixture::new(format, source)?;
    let mut connection = fixture.connection()?;
    let actual_key = app::explain_relationships(&fixture.database)?
        .into_iter()
        .find(|row| row.claim.relation_type == RelationshipType::SourceMerge)
        .ok_or("actual native lookup key missing")?
        .assertion_key;
    for (column, value) in [
        ("assertion_key", actual_key.as_str()),
        ("source_snapshot_key", fixture.snapshot.as_str()),
    ] {
        let plan = sql_query(format!(
            "EXPLAIN QUERY PLAN SELECT * FROM logiqx_cmp_source_relationships WHERE {column}=?"
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
    ] {
        assert!(
            !details.iter().any(|line| line == &format!("SCAN {alias}")
                || line.starts_with(&format!("SCAN {alias} "))),
            "inverse closure must not scan unrelated native catalogs: {details:?}"
        );
    }
    add_unrelated_native_owners(&fixture)?;
    connection.batch_execute("ANALYZE")?;
    assert_readiness_plan_is_keyed(&mut connection, actual_key.as_str())
}

#[test]
fn logiqx_native_queries_seek_actual_identity_and_edition_keys() -> TestResult {
    assert_native_lookup_plans_are_keyed(CatalogDocumentFormat::Logiqx, LOGIQX)
}

#[test]
fn cmp_native_queries_seek_actual_identity_and_edition_keys() -> TestResult {
    assert_native_lookup_plans_are_keyed(CatalogDocumentFormat::ClrMamePro, CMP)
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
