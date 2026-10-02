use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Nullable, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    database::Database,
    domain::{
        CatalogKey, CatalogRecordKind, CatalogScope, CatalogSnapshotDiff, PublishingSourceKey,
        RelationshipAssertionKey, RelationshipClaim, RelationshipEndpoint, RelationshipEvidence,
        RelationshipOrigin, RelationshipReview, RelationshipReviewDecision, RelationshipRule,
        RelationshipType, SnapshotKey,
    },
};
use std::fmt::Write as _;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const PC_XML: &str = r#"<datafile>
  <header><version>synthetic-pc-v1</version></header>
  <game name="clone-without-own-id" clone="0007"/>
  <game name="marker-with-merge" clone="P" mergeof="0000000000000000000000000000000000000042"/>
  <game name="matching-archive" id="0007"/>
</datafile>"#;

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

#[derive(QueryableByName)]
struct NativeOwner {
    #[diesel(sql_type = Text)]
    assertion_key: String,
    #[diesel(sql_type = BigInt)]
    relationship_id: i64,
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = Text)]
    set_name: String,
    #[diesel(sql_type = Text)]
    source_reference_kind: String,
    #[diesel(sql_type = Text)]
    literal: String,
}

#[derive(QueryableByName)]
struct TableDefinition {
    #[diesel(sql_type = Nullable<Text>)]
    sql: Option<String>,
}

#[derive(QueryableByName)]
struct SnapshotSeed {
    #[diesel(sql_type = Text)]
    catalog_key: String,
    #[diesel(sql_type = Text)]
    document_key: String,
    #[diesel(sql_type = Text)]
    interpretation_key: String,
    #[diesel(sql_type = Nullable<Text>)]
    acquisition_key: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    declared_version: Option<String>,
    #[diesel(sql_type = BigInt)]
    coverage_id: i64,
}

#[derive(QueryableByName)]
struct IdValue {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

#[derive(QueryableByName)]
struct StoredLiteral {
    #[diesel(sql_type = Text)]
    storage_type: String,
    #[diesel(sql_type = Text)]
    literal: String,
}

#[derive(QueryableByName)]
struct QueryPlan {
    #[diesel(sql_type = Text)]
    detail: String,
}

struct PendingPcSnapshot {
    key: String,
    group_id: i64,
    first_set_id: i64,
}

#[derive(Clone, Copy)]
enum PcLinkTable {
    Clone,
    MergeOf,
}

impl PcLinkTable {
    const fn name(self) -> &'static str {
        match self {
            Self::Clone => "no_intro_pc_clone_links",
            Self::MergeOf => "no_intro_pc_merge_links",
        }
    }

    const fn reference_kind(self) -> &'static str {
        match self {
            Self::Clone => "no_intro_pc_clone",
            Self::MergeOf => "no_intro_pc_mergeof",
        }
    }
}

struct ImportedPc {
    database: Database,
    database_path: Utf8PathBuf,
    snapshot: SnapshotKey,
}

impl ImportedPc {
    fn new(directory: &tempfile::TempDir, catalog: &str, source: &str) -> TestResult<Self> {
        let document_path =
            Utf8PathBuf::from_path_buf(directory.path().join(format!("{catalog}.xml")))
                .map_err(|_| "non-UTF-8 document path")?;
        let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
            .map_err(|_| "non-UTF-8 database path")?;
        std::fs::write(&document_path, source)?;

        let database = Database::open(&database_path)?;
        let report = app::import_catalog(
            &database,
            &CatalogImportRequest {
                document_path,
                format: CatalogDocumentFormat::NoIntroPcXml,
                source_key: PublishingSourceKey::new("synthetic-pc-relationships"),
                source_display_name: "Synthetic P/C relationship witness".into(),
                catalog_key: CatalogKey::new(catalog),
                catalog_display_name: format!("Synthetic P/C {catalog}"),
                scope: CatalogScope::Complete,
            },
        )?;
        assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
        Ok(Self {
            database,
            database_path,
            snapshot: report
                .snapshot_key
                .ok_or("successful import has no snapshot")?,
        })
    }

    fn connection(&self) -> TestResult<SqliteConnection> {
        Ok(SqliteConnection::establish(self.database_path.as_str())?)
    }
}

fn insert_unpublished_pc_owner(
    connection: &mut SqliteConnection,
    source_snapshot: &SnapshotKey,
) -> TestResult<PendingPcSnapshot> {
    insert_pc_snapshot_owner(connection, source_snapshot, "pending-pc-literal-guard")
}

fn insert_pc_snapshot_owner(
    connection: &mut SqliteConnection,
    source_snapshot: &SnapshotKey,
    snapshot_key: &str,
) -> TestResult<PendingPcSnapshot> {
    let seed = sql_query(
        "SELECT catalog_key, document_key, interpretation_key, acquisition_key, \
                declared_version, coverage_id \
         FROM catalog_snapshots WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(source_snapshot.as_str())
    .get_result::<SnapshotSeed>(connection)?;
    let snapshot = snapshot_key.to_owned();
    sql_query(
        "INSERT INTO catalog_snapshots \
         (snapshot_key,catalog_key,document_key,interpretation_key,acquisition_key, \
          declared_version,coverage_id,parent_snapshot_key) \
         VALUES (?,?,?,?,?,?,?,NULL)",
    )
    .bind::<Text, _>(&snapshot)
    .bind::<Text, _>(seed.catalog_key)
    .bind::<Text, _>(seed.document_key)
    .bind::<Text, _>(seed.interpretation_key)
    .bind::<Nullable<Text>, _>(seed.acquisition_key)
    .bind::<Nullable<Text>, _>(seed.declared_version)
    .bind::<BigInt, _>(seed.coverage_id)
    .execute(connection)?;

    let group_id =
        sql_query("SELECT COALESCE(MAX(set_group_id),0)+1 AS value FROM catalog_set_groups")
            .get_result::<IdValue>(connection)?
            .value;
    sql_query(
        "INSERT INTO catalog_set_groups(set_group_id,snapshot_key,kind,list_order) \
         VALUES (?,?,'root',0)",
    )
    .bind::<BigInt, _>(group_id)
    .bind::<Text, _>(&snapshot)
    .execute(connection)?;

    let first_set_id = insert_pc_game_owner(connection, group_id, 0, "literal-guard-owner")?;
    Ok(PendingPcSnapshot {
        key: snapshot,
        group_id,
        first_set_id,
    })
}

fn insert_pc_game_owner(
    connection: &mut SqliteConnection,
    group_id: i64,
    list_order: i64,
    name: &str,
) -> TestResult<i64> {
    let set_id = sql_query("SELECT COALESCE(MAX(set_id),0)+1 AS value FROM catalog_sets")
        .get_result::<IdValue>(connection)?
        .value;
    sql_query(
        "INSERT INTO catalog_sets \
         (set_id,set_group_id,source_element_kind,list_order,set_name,source_line,source_column) \
         VALUES (?,?,'no_intro_pc_game',?,?,1,1)",
    )
    .bind::<BigInt, _>(set_id)
    .bind::<BigInt, _>(group_id)
    .bind::<BigInt, _>(list_order)
    .bind::<Text, _>(name)
    .execute(connection)?;
    sql_query("INSERT INTO no_intro_pc_games(set_id,archive_id) VALUES (?,NULL)")
        .bind::<BigInt, _>(set_id)
        .execute(connection)?;
    Ok(set_id)
}

fn issue_pc_reference(
    connection: &mut SqliteConnection,
    snapshot: &str,
    assertion_key: &str,
    kind: PcLinkTable,
) -> TestResult<i64> {
    let identity = sql_query(
        "INSERT INTO catalog_relationships(assertion_key,origin,snapshot_key) \
         VALUES (?,'source',?) RETURNING relationship_id AS value",
    )
    .bind::<Text, _>(assertion_key)
    .bind::<Text, _>(snapshot)
    .get_result::<IdValue>(connection)?
    .value;
    sql_query(
        "INSERT INTO reported_catalog_relationships(relationship_id,source_reference_kind) \
         VALUES (?,?)",
    )
    .bind::<BigInt, _>(identity)
    .bind::<Text, _>(kind.reference_kind())
    .execute(connection)?;
    Ok(identity)
}

#[derive(Clone, Copy)]
enum LiteralValue<'a> {
    Text(&'a str),
    Blob,
}

fn insert_pc_literal(
    connection: &mut SqliteConnection,
    table: PcLinkTable,
    set_id: i64,
    relationship_id: i64,
    literal: LiteralValue<'_>,
) -> diesel::QueryResult<usize> {
    match literal {
        LiteralValue::Text(value) => sql_query(format!(
            "INSERT INTO {}(set_id,relationship_id,target_archive_id) VALUES (?,?,?)",
            table.name()
        ))
        .bind::<BigInt, _>(set_id)
        .bind::<BigInt, _>(relationship_id)
        .bind::<Text, _>(value)
        .execute(connection),
        LiteralValue::Blob => sql_query(format!(
            "INSERT INTO {}(set_id,relationship_id,target_archive_id) VALUES (?,?,X'3037')",
            table.name()
        ))
        .bind::<BigInt, _>(set_id)
        .bind::<BigInt, _>(relationship_id)
        .execute(connection),
    }
}

fn attempt_invalid_pc_literal(
    connection: &mut SqliteConnection,
    snapshot: &str,
    set_id: i64,
    table: PcLinkTable,
    assertion_key: &str,
    value: LiteralValue<'_>,
) -> TestResult {
    connection.batch_execute("SAVEPOINT pc_literal_probe")?;
    let relationship_id = issue_pc_reference(connection, snapshot, assertion_key, table)?;
    let rejected = insert_pc_literal(connection, table, set_id, relationship_id, value).is_err();
    connection.batch_execute("ROLLBACK TO pc_literal_probe; RELEASE pc_literal_probe")?;
    assert!(
        rejected,
        "{} accepted an invalid stored token",
        table.name()
    );
    Ok(())
}

fn stored_pc_literal(
    connection: &mut SqliteConnection,
    table: PcLinkTable,
    set_id: i64,
) -> TestResult<(i64, StoredLiteral)> {
    #[derive(QueryableByName)]
    struct StoredRow {
        #[diesel(sql_type = BigInt)]
        relationship_id: i64,
        #[diesel(sql_type = Text)]
        storage_type: String,
        #[diesel(sql_type = Text)]
        literal: String,
    }
    let row = sql_query(format!(
        "SELECT relationship_id, typeof(target_archive_id) AS storage_type, \
                target_archive_id AS literal FROM {} WHERE set_id = ?",
        table.name()
    ))
    .bind::<BigInt, _>(set_id)
    .get_result::<StoredRow>(connection)?;
    Ok((
        row.relationship_id,
        StoredLiteral {
            storage_type: row.storage_type,
            literal: row.literal,
        },
    ))
}

fn replace_pc_link(
    connection: &mut SqliteConnection,
    table: PcLinkTable,
    set_id: i64,
    relationship_id: i64,
    literal: &str,
) -> diesel::QueryResult<usize> {
    sql_query(format!(
        "INSERT OR REPLACE INTO {}(set_id,relationship_id,target_archive_id) VALUES (?,?,?)",
        table.name()
    ))
    .bind::<BigInt, _>(set_id)
    .bind::<BigInt, _>(relationship_id)
    .bind::<Text, _>(literal)
    .execute(connection)
}

fn attempt_pc_replace(
    connection: &mut SqliteConnection,
    pending: &PendingPcSnapshot,
    table: PcLinkTable,
    set_id: i64,
    relationship_id: Option<i64>,
    assertion_key: &str,
) -> TestResult {
    connection.batch_execute("SAVEPOINT pc_replace_probe")?;
    let replacement_id = match relationship_id {
        Some(existing) => existing,
        None => issue_pc_reference(connection, &pending.key, assertion_key, table)?,
    };
    let rejected = replace_pc_link(connection, table, set_id, replacement_id, "0009").is_err();
    connection.batch_execute("ROLLBACK TO pc_replace_probe; RELEASE pc_replace_probe")?;
    assert!(
        rejected,
        "{} allowed OR REPLACE to change native ownership",
        table.name()
    );
    Ok(())
}

#[test]
fn synthetic_pc_clone_and_merge_are_public_unresolved_source_explanations() -> TestResult {
    let directory = tempfile::tempdir()?;
    let left = ImportedPc::new(&directory, "pc-left", PC_XML)?;
    let right = ImportedPc::new(&directory, "pc-right", PC_XML)?;
    assert_ne!(left.snapshot, right.snapshot);

    let all = app::explain_relationships(&left.database)?;
    let pc_claims = pc_claims(&all);
    assert_eq!(
        pc_claims.len(),
        4,
        "each numeric clone/mergeof declaration is publicly explainable; P is only a marker"
    );

    let mut connection = left.connection()?;
    for imported in [&left, &right] {
        assert_native_snapshot_owners(&mut connection, imported)?;
        assert_all_owner_projections(&mut connection, imported)?;
        assert_global_explanation_view_projects_pc_sources(&mut connection, imported)?;
        assert_no_stored_generic_sources(&mut connection, imported)?;
    }

    for explanation in pc_claims {
        assert_public_explanation(
            explanation,
            &mut connection,
            [&left.snapshot, &right.snapshot],
        )?;
    }

    Ok(())
}

fn pc_claims(
    explanations: &[mame_coalesce::domain::RelationshipExplanation],
) -> Vec<&mame_coalesce::domain::RelationshipExplanation> {
    explanations
        .iter()
        .filter(|explanation| {
            matches!(
                explanation.claim.relation_type,
                RelationshipType::SourceParentClone | RelationshipType::AlternateRepresentationOf
            ) && explanation
                .source_field
                .as_deref()
                .is_some_and(|field| matches!(field, "clone" | "mergeof"))
        })
        .collect()
}

fn assert_native_snapshot_owners(
    connection: &mut SqliteConnection,
    imported: &ImportedPc,
) -> TestResult {
    let owners = load_native_owners(connection, &imported.snapshot)?;
    assert_eq!(owners.len(), 2);
    let clone = owner_for(&owners, "no_intro_pc_clone")?;
    let merge = owner_for(&owners, "no_intro_pc_mergeof")?;
    assert_eq!(clone.literal, "0007");
    assert_eq!(clone.set_name, "clone-without-own-id");
    assert_eq!(merge.literal, "0000000000000000000000000000000000000042");
    assert_eq!(merge.set_name, "marker-with-merge");
    assert_ne!(clone.relationship_id, merge.relationship_id);
    assert!(!clone.assertion_key.is_empty());

    let null_own_id = count_for_set(
        connection,
        "SELECT COUNT(*) AS count FROM no_intro_pc_games WHERE set_id = ? AND archive_id IS NULL",
        clone.set_id,
    )?;
    assert_eq!(null_own_id, 1);
    let marker = count_for_set(
        connection,
        "SELECT COUNT(*) AS count FROM no_intro_pc_clone_markers WHERE set_id = ?",
        merge.set_id,
    )?;
    assert_eq!(marker, 1, "a P marker may coexist with mergeof");
    Ok(())
}

fn load_native_owners(
    connection: &mut SqliteConnection,
    snapshot: &SnapshotKey,
) -> TestResult<Vec<NativeOwner>> {
    Ok(sql_query(
        "SELECT registry.assertion_key, registry.relationship_id, links.set_id, \
                sets.set_name, reported.source_reference_kind, links.target_archive_id AS literal \
         FROM catalog_relationships AS registry \
         JOIN reported_catalog_relationships AS reported USING (relationship_id) \
         JOIN no_intro_pc_clone_links AS links USING (relationship_id) \
         JOIN catalog_sets AS sets USING (set_id) \
         WHERE registry.snapshot_key = ? AND registry.origin = 'source' \
         UNION ALL \
         SELECT registry.assertion_key, registry.relationship_id, links.set_id, \
                sets.set_name, reported.source_reference_kind, links.target_archive_id \
         FROM catalog_relationships AS registry \
         JOIN reported_catalog_relationships AS reported USING (relationship_id) \
         JOIN no_intro_pc_merge_links AS links USING (relationship_id) \
         JOIN catalog_sets AS sets USING (set_id) \
         WHERE registry.snapshot_key = ? AND registry.origin = 'source'",
    )
    .bind::<Text, _>(snapshot.as_str())
    .bind::<Text, _>(snapshot.as_str())
    .load(connection)?)
}

fn owner_for<'a>(owners: &'a [NativeOwner], kind: &str) -> TestResult<&'a NativeOwner> {
    owners
        .iter()
        .find(|owner| owner.source_reference_kind == kind)
        .ok_or_else(|| format!("native P/C owner missing kind {kind}").into())
}

fn count_for_set(connection: &mut SqliteConnection, query: &str, set_id: i64) -> TestResult<i64> {
    Ok(sql_query(query)
        .bind::<BigInt, _>(set_id)
        .get_result::<Count>(connection)?
        .count)
}

fn relationship_id_for_set(
    connection: &mut SqliteConnection,
    table: PcLinkTable,
    set_id: i64,
) -> TestResult<i64> {
    Ok(sql_query(format!(
        "SELECT relationship_id AS value FROM {} WHERE set_id = ?",
        table.name()
    ))
    .bind::<BigInt, _>(set_id)
    .get_result::<IdValue>(connection)?
    .value)
}

fn set_id_for_name(connection: &mut SqliteConnection, set_name: &str) -> TestResult<i64> {
    Ok(
        sql_query("SELECT set_id AS value FROM catalog_sets WHERE set_name = ?")
            .bind::<Text, _>(set_name)
            .get_result::<IdValue>(connection)?
            .value,
    )
}

fn assert_all_owner_projections(
    connection: &mut SqliteConnection,
    imported: &ImportedPc,
) -> TestResult {
    for (view, expected) in [
        ("reported_catalog_relationship_owner_ids", 2),
        ("reported_catalog_relationship_raw_owners", 2),
        ("reported_catalog_relationship_owners", 2),
    ] {
        let query = match view {
            "reported_catalog_relationship_owner_ids" => format!(
                "SELECT COUNT(*) AS count FROM {view} WHERE relationship_id IN \
                 (SELECT relationship_id FROM catalog_relationships \
                  WHERE snapshot_key = ? AND origin = 'source')"
            ),
            _ => format!(
                "SELECT COUNT(*) AS count FROM {view} \
                 WHERE snapshot_key = ? AND source_reference_kind LIKE 'no_intro_pc_%'"
            ),
        };
        let actual = sql_query(query)
            .bind::<Text, _>(imported.snapshot.as_str())
            .get_result::<Count>(connection)?
            .count;
        assert_eq!(actual, expected, "P/C owner closure missing from {view}");
    }
    Ok(())
}

fn assert_global_explanation_view_projects_pc_sources(
    connection: &mut SqliteConnection,
    imported: &ImportedPc,
) -> TestResult {
    for (column, scope) in [
        ("subject_snapshot_key", "subject"),
        ("target_snapshot_key", "target"),
    ] {
        let query = format!(
            "SELECT COUNT(*) AS count FROM relationship_assertion_explanations \
             WHERE source_snapshot_key = ? AND {column} = ?"
        );
        let count = sql_query(query)
            .bind::<Text, _>(imported.snapshot.as_str())
            .bind::<Text, _>(imported.snapshot.as_str())
            .get_result::<Count>(connection)?
            .count;
        assert_eq!(count, 2, "P/C declarations missing from {scope} scope");
    }
    Ok(())
}

fn assert_no_stored_generic_sources(
    connection: &mut SqliteConnection,
    imported: &ImportedPc,
) -> TestResult {
    let count = sql_query(
        "SELECT COUNT(*) AS count FROM relationship_assertions \
         WHERE source_snapshot_key = ? AND origin = 'source_assertion'",
    )
    .bind::<Text, _>(imported.snapshot.as_str())
    .get_result::<Count>(connection)?
    .count;
    assert_eq!(count, 0);
    Ok(())
}

fn assert_public_explanation(
    explanation: &mame_coalesce::domain::RelationshipExplanation,
    connection: &mut SqliteConnection,
    snapshots: [&SnapshotKey; 2],
) -> TestResult {
    let source_field = explanation
        .source_field
        .as_deref()
        .ok_or("P/C source field is missing")?;
    let (expected_type, expected_kind, expected_set) = match source_field {
        "clone" => (
            RelationshipType::SourceParentClone,
            "no_intro_pc_clone",
            "clone-without-own-id",
        ),
        "mergeof" => (
            RelationshipType::AlternateRepresentationOf,
            "no_intro_pc_mergeof",
            "marker-with-merge",
        ),
        other => return Err(format!("unexpected P/C source field {other}").into()),
    };
    assert_eq!(explanation.claim.relation_type, expected_type);
    let RelationshipEndpoint::CatalogRecord(subject) = &explanation.claim.subject else {
        return Err("P/C relationship subject is not its actual catalog set".into());
    };
    assert!(snapshots.contains(&&subject.snapshot));
    assert_eq!(subject.kind, CatalogRecordKind::Set);
    assert_eq!(subject.key.as_str(), expected_set);
    let set_id = subject
        .owner_set_id
        .ok_or("P/C subject lost its native set ID")?
        .as_i64();

    let RelationshipEndpoint::NoIntroArchiveReference { snapshot, literal } =
        &explanation.claim.target
    else {
        return Err("P/C archive token was resolved instead of kept literal".into());
    };
    assert_eq!(snapshot, &subject.snapshot);
    assert_eq!(
        explanation.claim.evidence,
        RelationshipEvidence::ArchiveReference {
            declared_archive_reference: literal.clone(),
            source_field: source_field.to_owned(),
        }
    );
    assert!(explanation.source_location.is_some());
    assert!(explanation.source.is_some());

    let native_owner = native_owner_for_assertion(connection, explanation.assertion_key.as_str())?;
    assert_eq!(native_owner.set_id, set_id);
    assert_eq!(native_owner.source_reference_kind, expected_kind);
    Ok(())
}

fn native_owner_for_assertion(
    connection: &mut SqliteConnection,
    assertion_key: &str,
) -> TestResult<NativeOwnerId> {
    Ok(sql_query(
        "SELECT links.set_id, reported.source_reference_kind \
         FROM reported_catalog_relationships AS reported \
         JOIN no_intro_pc_clone_links AS links USING (relationship_id) \
         WHERE reported.relationship_id = (SELECT relationship_id \
             FROM catalog_relationships WHERE assertion_key = ?) \
         UNION ALL \
         SELECT links.set_id, reported.source_reference_kind \
         FROM reported_catalog_relationships AS reported \
         JOIN no_intro_pc_merge_links AS links USING (relationship_id) \
         WHERE reported.relationship_id = (SELECT relationship_id \
             FROM catalog_relationships WHERE assertion_key = ?)",
    )
    .bind::<Text, _>(assertion_key)
    .bind::<Text, _>(assertion_key)
    .get_result(connection)?)
}

#[derive(QueryableByName)]
struct NativeOwnerId {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = Text)]
    source_reference_kind: String,
}

#[test]
fn synthetic_pc_link_schema_owns_typed_registry_ids_and_literal_guards() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("schema.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let _database = Database::open(&database_path)?;
    let mut connection = SqliteConnection::establish(database_path.as_str())?;

    for table in ["no_intro_pc_clone_links", "no_intro_pc_merge_links"] {
        let definition =
            sql_query("SELECT sql FROM sqlite_schema WHERE type = 'table' AND name = ?")
                .bind::<Text, _>(table)
                .get_result::<TableDefinition>(&mut connection)?
                .sql
                .ok_or("native P/C link table has no stored definition")?
                .to_ascii_lowercase()
                .replace([' ', '\n', '\t'], "");

        assert!(
            definition.contains("relationship_idintegernotnullunique"),
            "{table} must own one required, unique issued relationship ID"
        );
        assert!(
            definition.contains("generatedalwaysas('no_intro_pc_"),
            "{table} must generate its closed reported-reference kind"
        );
        assert!(
            definition.contains("foreignkey(relationship_id,source_reference_kind)referencesreported_catalog_relationships(relationship_id,source_reference_kind)"),
            "{table} must use the matching composite reported-reference FK"
        );
        assert!(
            definition.contains("typeof(target_archive_id)='text'"),
            "{table} must reject non-TEXT stored archive tokens"
        );
        assert!(
            definition.contains("length(cast(target_archive_idasblob))>0"),
            "{table} must reject empty stored archive tokens"
        );
        assert!(
            definition.contains("instr(target_archive_id,char(0))=0"),
            "{table} must reject embedded NUL bytes"
        );
        assert!(
            definition.contains("target_archive_idnotglob'*[^0-9]*'"),
            "{table} must accept only ASCII digit tokens without numeric coercion"
        );
    }
    Ok(())
}

#[test]
fn synthetic_pc_literal_guards_match_stored_text_and_keep_long_tokens() -> TestResult {
    let directory = tempfile::tempdir()?;
    let imported = ImportedPc::new(&directory, "pc-literal-guards", PC_XML)?;
    let mut connection = imported.connection()?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF")?;
    let pending = insert_unpublished_pc_owner(&mut connection, &imported.snapshot)?;
    let second_set = insert_pc_game_owner(
        &mut connection,
        pending.group_id,
        1,
        "literal-guard-rejected-owner",
    )?;

    let long_token = format!("{}7", "0".repeat(4096));
    let controls = [
        (PcLinkTable::Clone, long_token.as_str()),
        (PcLinkTable::MergeOf, "0000000042"),
    ];
    for (index, (table, literal)) in controls.into_iter().enumerate() {
        let relationship_id = issue_pc_reference(
            &mut connection,
            &pending.key,
            &format!("pc-literal-valid-{index}"),
            table,
        )?;
        insert_pc_literal(
            &mut connection,
            table,
            pending.first_set_id,
            relationship_id,
            LiteralValue::Text(literal),
        )?;
        let (stored_id, stored) = stored_pc_literal(&mut connection, table, pending.first_set_id)?;
        assert_eq!(stored_id, relationship_id);
        assert_eq!(stored.storage_type, "text");
        assert_eq!(stored.literal, literal);
    }

    for (table_index, table) in [PcLinkTable::Clone, PcLinkTable::MergeOf]
        .into_iter()
        .enumerate()
    {
        for (value_index, value) in [
            LiteralValue::Text(""),
            LiteralValue::Text("12\0"),
            LiteralValue::Text("12\0x"),
            LiteralValue::Text("１２"),
            LiteralValue::Text(" 12"),
            LiteralValue::Text("12x"),
            LiteralValue::Blob,
        ]
        .into_iter()
        .enumerate()
        {
            attempt_invalid_pc_literal(
                &mut connection,
                &pending.key,
                second_set,
                table,
                &format!("pc-literal-invalid-{table_index}-{value_index}"),
                value,
            )?;
        }
    }
    Ok(())
}

#[test]
fn synthetic_pc_owner_ids_and_native_rows_reject_replace_with_enforcement_off() -> TestResult {
    let directory = tempfile::tempdir()?;
    let imported = ImportedPc::new(&directory, "pc-replace-guards", PC_XML)?;
    let mut connection = imported.connection()?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF")?;
    let pending = insert_unpublished_pc_owner(&mut connection, &imported.snapshot)?;
    let second_set = insert_pc_game_owner(&mut connection, pending.group_id, 1, "second-pc-owner")?;

    for (index, table) in [PcLinkTable::Clone, PcLinkTable::MergeOf]
        .into_iter()
        .enumerate()
    {
        let existing_id = issue_pc_reference(
            &mut connection,
            &pending.key,
            &format!("pc-owner-original-{index}"),
            table,
        )?;
        insert_pc_literal(
            &mut connection,
            table,
            pending.first_set_id,
            existing_id,
            LiteralValue::Text("0012"),
        )?;

        attempt_pc_replace(
            &mut connection,
            &pending,
            table,
            second_set,
            Some(existing_id),
            "unused-for-existing-id",
        )?;
        attempt_pc_replace(
            &mut connection,
            &pending,
            table,
            pending.first_set_id,
            None,
            &format!("pc-owner-fresh-replacement-{index}"),
        )?;

        let (stored_id, stored) = stored_pc_literal(&mut connection, table, pending.first_set_id)?;
        assert_eq!(stored_id, existing_id);
        assert_eq!(stored.literal, "0012");
        assert_eq!(
            count_for_set(
                &mut connection,
                &format!(
                    "SELECT COUNT(*) AS count FROM {} WHERE set_id = ?",
                    table.name()
                ),
                second_set,
            )?,
            0
        );
        let physical_owners = count_for_set(
            &mut connection,
            "SELECT COUNT(*) AS count FROM reported_catalog_relationship_owner_ids \
             WHERE relationship_id = ?",
            existing_id,
        )?;
        assert_eq!(physical_owners, 1);
    }
    Ok(())
}

#[test]
fn synthetic_pc_published_owner_rejects_late_child_with_enforcement_off() -> TestResult {
    let directory = tempfile::tempdir()?;
    let imported = ImportedPc::new(&directory, "pc-late-child", PC_XML)?;
    let mut connection = imported.connection()?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF")?;
    let published_owners = load_native_owners(&mut connection, &imported.snapshot)?;
    let published_clone = owner_for(&published_owners, "no_intro_pc_clone")?;
    let pending = insert_unpublished_pc_owner(&mut connection, &imported.snapshot)?;

    connection.batch_execute("SAVEPOINT pc_late_child_probe")?;
    let relationship_id = issue_pc_reference(
        &mut connection,
        &pending.key,
        "pc-late-child-identity",
        PcLinkTable::Clone,
    )?;
    let late_child = insert_pc_literal(
        &mut connection,
        PcLinkTable::Clone,
        published_clone.set_id,
        relationship_id,
        LiteralValue::Text("0099"),
    )
    .is_err();
    connection.batch_execute("ROLLBACK TO pc_late_child_probe; RELEASE pc_late_child_probe")?;
    assert!(
        late_child,
        "published P/C owner accepted a late clone child"
    );
    assert_eq!(
        relationship_id_for_set(&mut connection, PcLinkTable::Clone, published_clone.set_id,)?,
        published_clone.relationship_id
    );
    Ok(())
}

fn populated_pc_document(rows: usize) -> TestResult<String> {
    let mut xml = String::from("<datafile><header><version>query-plan</version></header>");
    for index in 0..rows {
        write!(
            xml,
            "<game name='owner-{index}' id='{index:06}' clone='{index:06}'/>"
        )?;
    }
    xml.push_str("</datafile>");
    Ok(xml)
}

fn pc_history_document(clone: &str, mergeof: &str, vendor: &str) -> String {
    format!(
        "<datafile><header><version>history</version></header>\
         <game name='history-child' clone='{clone}' mergeof='{mergeof}' vendor='{vendor}'/>\
         <game name='clone-parent' id='0007'/><game name='merge-parent' id='0008'/>\
         </datafile>"
    )
}

fn pc_build_document(include_clone_parent: bool) -> String {
    let clone_parent = if include_clone_parent {
        "<game name='clone-parent' id='0007'/>"
    } else {
        ""
    };
    format!(
        "<datafile><header><version>build-parent</version></header>\
         <game name='build-child' clone='0007' mergeof='0008'>\
           <rom name='child.rom' size='3' sha1='a9993e364706816aba3e25717850c26c9cd0d89d'/>\
         </game>{clone_parent}<game name='merge-parent' id='0008'/></datafile>"
    )
}

fn history_record<'a>(
    diff: &'a CatalogSnapshotDiff,
    set_name: &str,
) -> TestResult<&'a mame_coalesce::domain::SnapshotRecordDiff> {
    diff.records
        .iter()
        .find(|record| record.set_name == set_name)
        .ok_or_else(|| format!("snapshot history missing {set_name}").into())
}

fn source_fields_and_targets(
    explanations: &[mame_coalesce::domain::RelationshipExplanation],
    snapshot: &SnapshotKey,
    subject_name: &str,
) -> TestResult<Vec<(String, String)>> {
    let mut declarations = explanations
        .iter()
        .filter(|explanation| {
            matches!(
                &explanation.claim.subject,
                RelationshipEndpoint::CatalogRecord(subject)
                    if &subject.snapshot == snapshot && subject.key.as_str() == subject_name
            )
        })
        .map(|explanation| {
            let RelationshipEndpoint::NoIntroArchiveReference { literal, .. } =
                &explanation.claim.target
            else {
                return Err("P/C history target was not preserved as a literal".into());
            };
            Ok((
                explanation
                    .source_field
                    .clone()
                    .ok_or("P/C history field is missing")?,
                literal.clone(),
            ))
        })
        .collect::<TestResult<Vec<_>>>()?;
    declarations.sort();
    Ok(declarations)
}

fn source_assertion_keys(imported: &ImportedPc) -> TestResult<Vec<String>> {
    source_assertion_keys_at(&imported.database_path, &imported.snapshot)
}

fn source_assertion_keys_at(
    database_path: &Utf8PathBuf,
    snapshot: &SnapshotKey,
) -> TestResult<Vec<String>> {
    #[derive(QueryableByName)]
    struct KeyRow {
        #[diesel(sql_type = Text)]
        assertion_key: String,
    }
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    Ok(sql_query(
        "SELECT identity.assertion_key FROM catalog_relationships AS identity \
         JOIN reported_catalog_relationships AS reported USING(relationship_id) \
         WHERE identity.snapshot_key = ? AND identity.origin = 'source' \
           AND reported.source_reference_kind IN ('no_intro_pc_clone','no_intro_pc_mergeof') \
         ORDER BY identity.relationship_id",
    )
    .bind::<Text, _>(snapshot.as_str())
    .load::<KeyRow>(&mut connection)?
    .into_iter()
    .map(|row| row.assertion_key)
    .collect())
}

#[test]
fn synthetic_pc_reimport_keeps_issued_source_assertion_keys() -> TestResult {
    let directory = tempfile::tempdir()?;
    let imported = ImportedPc::new(&directory, "pc-reimport-keys", PC_XML)?;
    let original = source_assertion_keys(&imported)?;
    assert_eq!(original.len(), 2);
    assert!(original.iter().all(|key| !key.is_empty()));

    let document_path = Utf8PathBuf::from_path_buf(directory.path().join("pc-reimport-keys.xml"))
        .map_err(|_| "non-UTF-8 document path")?;
    let report = app::import_catalog(
        &imported.database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::NoIntroPcXml,
            source_key: PublishingSourceKey::new("synthetic-pc-relationships"),
            source_display_name: "Synthetic P/C relationship witness".into(),
            catalog_key: CatalogKey::new("pc-reimport-keys"),
            catalog_display_name: "Synthetic P/C pc-reimport-keys".into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    assert_eq!(source_assertion_keys(&imported)?, original);
    Ok(())
}

#[test]
fn synthetic_pc_source_history_tracks_clone_and_merge_edits_only() -> TestResult {
    let directory = tempfile::tempdir()?;
    let first = ImportedPc::new(
        &directory,
        "pc-history",
        &pc_history_document("0007", "0008", "vendor-one"),
    )?;
    let document_path = Utf8PathBuf::from_path_buf(directory.path().join("pc-history.xml"))
        .map_err(|_| "non-UTF-8 document path")?;
    std::fs::write(
        &document_path,
        pc_history_document("0009", "0010", "vendor-two"),
    )?;
    let report = app::import_catalog(
        &first.database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::NoIntroPcXml,
            source_key: PublishingSourceKey::new("synthetic-pc-relationships"),
            source_display_name: "Synthetic P/C relationship witness".into(),
            catalog_key: CatalogKey::new("pc-history"),
            catalog_display_name: "Synthetic P/C pc-history".into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    let second_snapshot = report
        .snapshot_key
        .ok_or("successful import has no snapshot")?;
    assert_ne!(first.snapshot, second_snapshot);

    let diff = app::diff_catalog_snapshots(&first.database, &first.snapshot, &second_snapshot)?;
    let child = history_record(&diff, "history-child")?;
    assert_eq!(
        source_fields_and_targets(
            &child.relationship_evidence,
            &second_snapshot,
            "history-child"
        )?,
        vec![
            ("clone".to_owned(), "0009".to_owned()),
            ("mergeof".to_owned(), "0010".to_owned()),
        ]
    );
    assert!(child.relationship_evidence.iter().all(|explanation| {
        matches!(
            &explanation.claim.target,
            RelationshipEndpoint::NoIntroArchiveReference { .. }
        )
    }));
    Ok(())
}

#[test]
fn synthetic_pc_build_parent_prefers_clone_then_falls_back_to_merge() -> TestResult {
    for (catalog, has_clone_parent, expected_parent) in [
        ("pc-build-clone-precedence", true, "clone-parent"),
        ("pc-build-merge-fallback", false, "merge-parent"),
    ] {
        let directory = tempfile::tempdir()?;
        let imported = ImportedPc::new(&directory, catalog, &pc_build_document(has_clone_parent))?;
        let explanations = app::explain_relationships(&imported.database)?;
        assert_eq!(
            source_fields_and_targets(&explanations, &imported.snapshot, "build-child")?,
            vec![
                ("clone".to_owned(), "0007".to_owned()),
                ("mergeof".to_owned(), "0008".to_owned()),
            ]
        );
        assert!(
            explanations
                .iter()
                .filter(|explanation| {
                    matches!(
                        &explanation.claim.subject,
                        RelationshipEndpoint::CatalogRecord(subject)
                            if subject.snapshot == imported.snapshot
                                && subject.key.as_str() == "build-child"
                    )
                })
                .all(|explanation| matches!(
                    &explanation.claim.target,
                    RelationshipEndpoint::NoIntroArchiveReference { .. }
                ))
        );

        let source_path = Utf8PathBuf::from_path_buf(directory.path().join("roms"))
            .map_err(|_| "non-UTF-8 source path")?;
        std::fs::create_dir(&source_path)?;
        let plan = app::plan_build(
            &imported.database,
            &app::BuildPlanRequest {
                dat_path: Utf8PathBuf::from_path_buf(
                    directory.path().join(format!("{catalog}.xml")),
                )
                .map_err(|_| "non-UTF-8 document path")?,
                source_path,
                mode: mame_coalesce::domain::BuildMode::PerGame,
                matching_policy: mame_coalesce::domain::MatchingPolicy::Sha1Compatibility,
                missing_policy: mame_coalesce::domain::MissingContentPolicy::AllowPartial,
                set_selection: mame_coalesce::domain::SetSelection::All,
            },
        )?;
        assert_eq!(plan.report.resolutions.len(), 1);
        assert_eq!(
            plan.report.resolutions[0]
                .requirement
                .parent_name
                .as_deref(),
            Some(expected_parent)
        );
    }
    Ok(())
}

#[test]
fn synthetic_pc_backup_restores_review_history_without_original_source_file() -> TestResult {
    let directory = tempfile::tempdir()?;
    let source = pc_history_document("0007", "0008", "vendor");
    let imported = ImportedPc::new(&directory, "pc-backup-history", &source)?;
    let keys = source_assertion_keys(&imported)?;
    let supported_key = record_supported_pc_claim(&imported, &keys)?;
    let clone_key = RelationshipAssertionKey::new(keys[0].clone());
    for (decision, note) in [
        (RelationshipReviewDecision::Rejected, "first review"),
        (RelationshipReviewDecision::Accepted, "updated review"),
    ] {
        app::review_relationship(
            &imported.database,
            &clone_key,
            &RelationshipReview {
                decision,
                note: note.to_owned(),
                superseded_by: None,
            },
        )?;
    }

    let backup_path = Utf8PathBuf::from_path_buf(directory.path().join("pc.backup.sqlite"))
        .map_err(|_| "non-UTF-8 backup path")?;
    let restored_path = Utf8PathBuf::from_path_buf(directory.path().join("restored.sqlite"))
        .map_err(|_| "non-UTF-8 restored path")?;
    let database_path = imported.database_path.clone();
    let original_document = directory.path().join("pc-backup-history.xml");
    drop(imported.database);
    std::fs::remove_file(original_document)?;
    assert!(matches!(
        mame_coalesce::create_backup(&database_path, &backup_path)?,
        mame_coalesce::BackupOutcome::Published
            | mame_coalesce::BackupOutcome::PublishedDurabilityUnconfirmed { .. }
    ));
    assert!(matches!(
        mame_coalesce::restore_backup(
            &backup_path,
            &restored_path,
            mame_coalesce::RestorePolicy::CreateNew
        )?,
        mame_coalesce::RestoreOutcome::Published
            | mame_coalesce::RestoreOutcome::PublishedDurabilityUnconfirmed { .. }
    ));
    assert!(mame_coalesce::check_integrity(&restored_path)?.is_clean());

    let restored = Database::open(&restored_path)?;
    assert_eq!(
        source_assertion_keys_at(&restored_path, &imported.snapshot)?,
        keys
    );
    let explanations = app::explain_relationships(&restored)?;
    let supported = explanations
        .iter()
        .find(|explanation| explanation.assertion_key == supported_key)
        .ok_or("restored supported P/C relationship missing")?;
    let RelationshipOrigin::DerivedCandidate {
        supporting_assertions,
        ..
    } = &supported.claim.origin
    else {
        return Err("restored P/C support lost its derived origin".into());
    };
    assert_eq!(
        supporting_assertions
            .iter()
            .map(RelationshipAssertionKey::as_str)
            .collect::<Vec<_>>(),
        keys.iter().map(String::as_str).collect::<Vec<_>>()
    );
    let reviewed = explanations
        .iter()
        .find(|explanation| explanation.assertion_key == clone_key)
        .ok_or("restored P/C clone assertion missing")?;
    assert_eq!(reviewed.review_history.len(), 2);
    assert_eq!(
        reviewed
            .latest_review
            .as_ref()
            .map(|review| review.decision),
        Some(RelationshipReviewDecision::Accepted)
    );
    assert_eq!(
        app::load_snapshot_source(&restored, &imported.snapshot)?,
        source.as_bytes()
    );
    Ok(())
}

fn record_supported_pc_claim(
    imported: &ImportedPc,
    keys: &[String],
) -> TestResult<RelationshipAssertionKey> {
    let explanations = app::explain_relationships(&imported.database)?;
    let first = explanations.first().ok_or("missing first P/C source")?;
    let second = explanations.get(1).ok_or("missing second P/C source")?;
    Ok(app::record_relationship(
        &imported.database,
        &RelationshipClaim {
            relation_type: RelationshipType::CatalogContinuity,
            subject: first.claim.subject.clone(),
            target: second.claim.subject.clone(),
            origin: RelationshipOrigin::DerivedCandidate {
                rule: RelationshipRule::new(
                    "synthetic-pc-source-comparison",
                    "v1",
                    "Compare P/C source declarations",
                )?,
                supporting_assertions: keys
                    .iter()
                    .cloned()
                    .map(RelationshipAssertionKey::new)
                    .collect(),
            },
            evidence: RelationshipEvidence::Rationale {
                reason: "Supported by both retained native P/C declarations".into(),
            },
        },
    )?)
}

#[test]
fn synthetic_pc_storage_failure_after_registry_issuance_rolls_back_import() -> TestResult {
    let directory = tempfile::tempdir()?;
    let document_path = Utf8PathBuf::from_path_buf(directory.path().join("injected-failure.xml"))
        .map_err(|_| "non-UTF-8 document path")?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("failure.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    std::fs::write(&document_path, PC_XML)?;
    let database = Database::open(&database_path)?;
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    connection.batch_execute(
        "CREATE TRIGGER inject_pc_clone_storage_failure \
         BEFORE INSERT ON no_intro_pc_clone_links \
         BEGIN SELECT CASE WHEN EXISTS ( \
           SELECT 1 FROM catalog_relationships identity \
           JOIN reported_catalog_relationships reported USING(relationship_id) \
           WHERE identity.relationship_id=NEW.relationship_id AND identity.origin='source' \
             AND reported.source_reference_kind='no_intro_pc_clone' \
         ) THEN RAISE(ABORT,'injected P/C link failure after issued registry') \
           ELSE RAISE(ABORT,'P/C link insert has no issued registry') END; END",
    )?;

    let result = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::NoIntroPcXml,
            source_key: PublishingSourceKey::new("synthetic-pc-failure"),
            source_display_name: "Synthetic P/C storage failure".into(),
            catalog_key: CatalogKey::new("pc-storage-failure"),
            catalog_display_name: "Synthetic P/C storage failure".into(),
            scope: CatalogScope::Complete,
        },
    );
    let error = result
        .err()
        .ok_or("injected P/C storage failure unexpectedly succeeded")?;
    assert!(
        error
            .to_string()
            .contains("injected P/C link failure after issued registry"),
        "storage witness did not prove issued registry ownership: {error}"
    );
    for table in [
        "catalog_relationships",
        "reported_catalog_relationships",
        "no_intro_pc_clone_links",
        "no_intro_pc_merge_links",
        "catalog_sets",
        "catalog_set_groups",
        "catalog_snapshots",
    ] {
        assert_eq!(
            sql_query(format!("SELECT COUNT(*) AS count FROM {table}"))
                .get_result::<Count>(&mut connection)?
                .count,
            0,
            "failed P/C transaction retained native rows in {table}"
        );
    }
    assert_eq!(
        sql_query("SELECT COUNT(*) AS count FROM snapshot_publications")
            .get_result::<Count>(&mut connection)?
            .count,
        0,
        "failed P/C transaction published a snapshot"
    );
    Ok(())
}

#[test]
fn synthetic_pc_native_owner_queries_are_keyed_in_both_directions() -> TestResult {
    let directory = tempfile::tempdir()?;
    let imported = ImportedPc::new(
        &directory,
        "pc-native-query-plans",
        &populated_pc_document(256)?,
    )?;
    let mut connection = imported.connection()?;
    for index in 0..256 {
        insert_pc_snapshot_owner(
            &mut connection,
            &imported.snapshot,
            &format!("unrelated-pc-plan-edition-{index}"),
        )?;
    }
    connection.batch_execute("ANALYZE")?;
    let set_id = set_id_for_name(&mut connection, "owner-0")?;
    let relationship_id = relationship_id_for_set(&mut connection, PcLinkTable::Clone, set_id)?;

    for view in [
        "reported_catalog_relationship_owners",
        "reported_catalog_relationship_owner_ids",
    ] {
        let query = format!(
            "EXPLAIN QUERY PLAN SELECT relationship_id FROM {view} WHERE relationship_id = ?"
        );
        let details = sql_query(query)
            .bind::<BigInt, _>(relationship_id)
            .load::<QueryPlan>(&mut connection)?
            .into_iter()
            .map(|row| row.detail)
            .collect::<Vec<_>>();
        assert!(
            details.iter().any(|line| {
                (line.starts_with("SEARCH link ")
                    || line.starts_with("SEARCH no_intro_pc_clone_links "))
                    && line.contains("no_intro_pc_clone_links")
                    && line.contains("relationship_id")
            }),
            "{view} must seek the actual P/C owner by issued ID: {details:?}"
        );
        assert!(
            !details.iter().any(|line| {
                (line.starts_with("SCAN link ") || line.starts_with("SCAN no_intro_pc_clone_links"))
                    && line.contains("no_intro_pc_clone_links")
            }),
            "{view} scanned the P/C link population: {details:?}"
        );
    }

    let reverse = sql_query(
        "EXPLAIN QUERY PLAN SELECT relationship_id FROM \
         reported_catalog_relationship_raw_owners WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(imported.snapshot.as_str())
    .load::<QueryPlan>(&mut connection)?
    .into_iter()
    .map(|row| row.detail)
    .collect::<Vec<_>>();
    assert!(
        reverse
            .iter()
            .any(|line| line.starts_with("SEARCH groups ")),
        "reverse owner closure must seek the requested snapshot: {reverse:?}"
    );
    Ok(())
}
