use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Binary, Text},
};
use mame_coalesce::{
    app,
    database::Database,
    domain::{
        CatalogContentId, CatalogKey, CatalogRecordKind, CatalogRecordRef, CatalogScope,
        CatalogSetId, ContentDigestAlgorithm, ContentIdentity, ExternalRecordRef, OccurrenceId,
        PublishingSourceKey, RelationshipClaim, RelationshipEndpoint, RelationshipEvidence,
        RelationshipOrigin, RelationshipRule, RelationshipType, SnapshotKey,
    },
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

struct Catalog {
    directory: tempfile::TempDir,
    path: Utf8PathBuf,
    database: Database,
    connection: SqliteConnection,
}

impl Catalog {
    fn new() -> TestResult<Self> {
        let directory = tempfile::tempdir()?;
        let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
        let database = Database::open(&path)?;
        let connection = SqliteConnection::establish(path.as_str())?;
        Ok(Self {
            directory,
            path,
            database,
            connection,
        })
    }

    fn count(&mut self, table: &str) -> TestResult<i64> {
        Ok(sql_query(format!("SELECT count(*) AS count FROM {table}"))
            .get_result::<Count>(&mut self.connection)?
            .count)
    }

    fn import(&self, key: &str) -> TestResult<SnapshotKey> {
        self.import_format(key, app::CatalogDocumentFormat::Logiqx(mame_coalesce::logiqx::LogiqxMode::ObservedCompatible),
            "<datafile><game name='same'><rom name='same.bin' size='1' sha1='1111111111111111111111111111111111111111'/></game><game name='same'/></datafile>")
    }

    fn import_format(
        &self,
        key: &str,
        format: app::CatalogDocumentFormat,
        source: &str,
    ) -> TestResult<SnapshotKey> {
        let document = Utf8PathBuf::try_from(self.directory.path().join(format!("{key}.xml")))?;
        std::fs::write(&document, source)?;
        let report = app::import_catalog(
            &self.database,
            &app::CatalogImportRequest {
                document_path: document,
                format,
                source_key: PublishingSourceKey::new(key),
                source_display_name: key.into(),
                catalog_key: CatalogKey::new(key),
                catalog_display_name: key.into(),
                scope: CatalogScope::Complete,
            },
        )?;
        assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
        Ok(report.snapshot_key.ok_or("snapshot missing")?)
    }
}

fn manual(subject: RelationshipEndpoint, target: RelationshipEndpoint) -> RelationshipClaim {
    RelationshipClaim {
        relation_type: RelationshipType::CatalogContinuity,
        subject,
        target,
        origin: RelationshipOrigin::UserConclusion,
        evidence: RelationshipEvidence::Rationale {
            reason: "reviewed correspondence".into(),
        },
    }
}

#[derive(QueryableByName)]
struct Owner {
    #[diesel(sql_type = BigInt)]
    set_id: i64,
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
    #[diesel(sql_type = Binary)]
    content_uuid: Vec<u8>,
}

#[test]
fn actual_catalog_targets_keep_integer_owners_and_issued_uuid_without_name_resolution() -> TestResult
{
    let mut catalog = Catalog::new()?;
    let snapshot = catalog.import("left")?;
    let other = catalog.import("right")?;
    let owner = sql_query("SELECT sets.set_id,occurrence.occurrence_id,occurrence.content_uuid FROM catalog_sets sets JOIN catalog_set_groups groups USING(set_group_id) JOIN asset_occurrences occurrence ON occurrence.record_id=sets.set_id WHERE groups.snapshot_key=?")
        .bind::<Text,_>(snapshot.as_str()).get_result::<Owner>(&mut catalog.connection)?;
    let uuid = CatalogContentId::from_bytes(
        owner
            .content_uuid
            .try_into()
            .map_err(|_| "wrong UUID width")?,
    );
    let actual_set = RelationshipEndpoint::CatalogRecord(
        CatalogRecordRef::new(snapshot.clone(), CatalogRecordKind::Set, "same")
            .with_owner(CatalogSetId::try_from(owner.set_id)?),
    );
    let actual_media = RelationshipEndpoint::CatalogMediaEntry {
        snapshot: snapshot.clone(),
        occurrence_id: OccurrenceId::try_from(owner.occurrence_id)?,
    };
    let declared_set = RelationshipEndpoint::CatalogRecord(CatalogRecordRef::new(
        snapshot.clone(),
        CatalogRecordKind::Set,
        "same",
    ));
    let shared = RelationshipEndpoint::SharedCatalogFile(uuid);
    let digest = RelationshipEndpoint::ContentObject(ContentIdentity::new(
        ContentDigestAlgorithm::Sha1,
        "1111111111111111111111111111111111111111",
    )?);
    for subject in [&actual_set, &actual_media, &declared_set, &shared, &digest] {
        let claim = manual(
            subject.clone(),
            RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new("review", "target")),
        );
        let key = app::record_relationship(&catalog.database, &claim)?;
        let explanation = app::explain_relationships(&catalog.database)?
            .into_iter()
            .find(|row| row.assertion_key == key)
            .ok_or("missing native claim")?;
        assert_eq!(explanation.claim, claim);
    }
    for table in [
        "catalog_set_targets",
        "catalog_media_entry_targets",
        "shared_file_targets",
        "unresolved_catalog_targets",
        "declared_digest_targets",
    ] {
        assert_eq!(catalog.count(table)?, 1, "native target subtype {table}");
    }
    assert_eq!(
        catalog.count("digest_values")?,
        1,
        "digest target reuses source binary digest, not a copy"
    );
    assert_eq!(
        catalog.count("catalog_contents")?,
        1,
        "a claim must not issue another shared UUID"
    );
    let target = RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new("review", "target"));
    for subject in [
        RelationshipEndpoint::CatalogMediaEntry {
            snapshot: other,
            occurrence_id: OccurrenceId::try_from(owner.occurrence_id)?,
        },
        RelationshipEndpoint::SharedCatalogFile(CatalogContentId::from_bytes([255; 16])),
    ] {
        assert!(
            app::record_relationship(&catalog.database, &manual(subject, target.clone())).is_err()
        );
    }
    assert_eq!(
        catalog.count("catalog_relationships")?,
        5,
        "failed ownership validation rolls back identity issuance"
    );
    drop(catalog.database);
    assert!(mame_coalesce::check_integrity(&catalog.path)?.is_clean());
    Ok(())
}

#[test]
fn rules_have_explicit_revision_identity_and_reject_conflicting_descriptions_atomically()
-> TestResult {
    for (key, revision, description) in [
        (" ", "v1", "rule"),
        ("key", "\t", "rule"),
        ("key", "v1", ""),
    ] {
        assert!(RelationshipRule::new(key, revision, description).is_err());
    }
    assert!(
        serde_json::from_str::<RelationshipRule>(
            r#"{"key":"key","revision":"","description":"Rule"}"#
        )
        .is_err()
    );
    let mut catalog = Catalog::new()?;
    let mut claim = manual(
        RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new("rules", "left")),
        RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new("rules", "right")),
    );
    for (key, revision) in [("rule-v1", "2"), ("rule", "v1-2"), ("rule-v1", "2")] {
        claim.origin = RelationshipOrigin::DerivedCandidate {
            rule: RelationshipRule::new(key, revision, "Declared rule")?,
            supporting_assertions: vec![],
        };
        app::record_relationship(&catalog.database, &claim)?;
    }
    assert_eq!(catalog.count("catalog_relationship_rules")?, 2);
    claim.origin = RelationshipOrigin::DerivedCandidate {
        rule: RelationshipRule::new("rule-v1", "2", "Different rule")?,
        supporting_assertions: vec![],
    };
    assert!(app::record_relationship(&catalog.database, &claim).is_err());
    assert_eq!(catalog.count("catalog_relationships")?, 3);
    assert_eq!(catalog.count("catalog_relationship_targets")?, 2);
    assert_eq!(catalog.count("inferred_catalog_relationships")?, 3);
    drop(catalog.database);
    assert!(mame_coalesce::check_integrity(&catalog.path)?.is_clean());
    Ok(())
}

#[test]
fn sql_rule_metadata_matches_rust_unicode_whitespace_validation() -> TestResult {
    let mut catalog = Catalog::new()?;
    catalog
        .connection
        .batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF")?;
    let mut accepted = Vec::new();
    for character in (0..=0x0010_ffff)
        .filter_map(char::from_u32)
        .filter(|character| character.is_whitespace())
    {
        let whitespace = character.to_string();
        for (field, (key, revision, description)) in [
            ("key", (whitespace.as_str(), "v1", "Rule")),
            ("revision", ("rule", whitespace.as_str(), "Rule")),
            ("description", ("rule", "v1", whitespace.as_str())),
        ] {
            assert!(RelationshipRule::new(key, revision, description).is_err());
            catalog.connection.batch_execute("SAVEPOINT invalid_rule")?;
            let result = sql_query(
                "INSERT INTO catalog_relationship_rules(rule_key,revision,description) VALUES(?,?,?)",
            )
            .bind::<Text, _>(key)
            .bind::<Text, _>(revision)
            .bind::<Text, _>(description)
            .execute(&mut catalog.connection);
            catalog
                .connection
                .batch_execute("ROLLBACK TO invalid_rule; RELEASE invalid_rule")?;
            if result.is_ok() {
                accepted.push(format!("{field}: {character:?}"));
            }
        }
    }
    assert!(
        accepted.is_empty(),
        "SQL accepted rule metadata that its Rust reader rejects: {accepted:?}"
    );
    for text in ["\tRule\u{3000}", "\u{200b}", "\u{feff}", "\0"] {
        let rule = RelationshipRule::new(text, text, text)?;
        let claim = RelationshipClaim {
            origin: RelationshipOrigin::DerivedCandidate {
                rule,
                supporting_assertions: Vec::new(),
            },
            ..manual(
                RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new("unicode", "left")),
                RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new("unicode", "right")),
            )
        };
        let key = app::record_relationship(&catalog.database, &claim)?;
        let actual = app::explain_relationships(&catalog.database)?
            .into_iter()
            .find(|row| row.assertion_key == key)
            .ok_or("valid non-whitespace rule was not explainable")?;
        assert_eq!(actual.claim, claim);
    }
    Ok(())
}

#[test]
fn complete_targets_and_claims_are_immutable_even_without_sqlite_enforcement() -> TestResult {
    let mut catalog = Catalog::new()?;
    let claim = manual(
        RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new("guards", "left")),
        RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new("guards", "right")),
    );
    app::record_relationship(&catalog.database, &claim)?;
    catalog
        .connection
        .batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF")?;
    for sql in [
        "UPDATE catalog_relationship_targets SET kind='shared_file'",
        "DELETE FROM catalog_relationship_targets",
        "INSERT OR REPLACE INTO catalog_relationship_targets SELECT * FROM catalog_relationship_targets",
        "UPDATE external_catalog_targets SET declared_key='changed'",
        "DELETE FROM external_catalog_targets",
        "INSERT OR REPLACE INTO external_catalog_targets SELECT * FROM external_catalog_targets",
        "INSERT INTO catalog_set_targets(target_id,set_id) SELECT target_id,123456 FROM catalog_relationship_targets",
        "UPDATE manual_catalog_relationships SET relation_type='revision_of'",
        "DELETE FROM manual_catalog_relationships",
        "INSERT OR REPLACE INTO manual_catalog_relationships SELECT * FROM manual_catalog_relationships",
    ] {
        assert!(
            catalog.connection.batch_execute(sql).is_err(),
            "mutation bypass: {sql}"
        );
    }
    catalog.connection.batch_execute("INSERT INTO catalog_relationship_targets(kind) VALUES('external_record'); INSERT INTO catalog_relationships(assertion_key,origin) VALUES('incomplete-target','user')")?;
    assert!(catalog.connection.batch_execute("INSERT INTO manual_catalog_relationships(relationship_id,relation_type,from_target_id,to_target_id) SELECT registry.relationship_id,'catalog_continuity',empty.target_id,full.target_id FROM catalog_relationships registry JOIN catalog_relationship_targets empty ON empty.target_id=(SELECT max(target_id) FROM catalog_relationship_targets) JOIN external_catalog_targets full ON full.declared_key='left' WHERE registry.assertion_key='incomplete-target'").is_err());
    assert!(catalog.connection.batch_execute("INSERT INTO catalog_relationship_rationales SELECT relationship_id,'otherwise valid rationale' FROM catalog_relationships WHERE assertion_key='incomplete-target'").is_err());
    assert_eq!(app::explain_relationships(&catalog.database)?.len(), 1);
    Ok(())
}

#[test]
fn orphan_native_identity_is_an_integrity_failure_not_an_invisible_claim() -> TestResult {
    let mut catalog = Catalog::new()?;
    catalog.connection.batch_execute("INSERT INTO catalog_relationships(assertion_key,origin) VALUES('orphan-native-decision','derived')")?;
    assert!(app::explain_relationships(&catalog.database)?.is_empty());
    drop(catalog.database);
    let report = mame_coalesce::check_integrity(&catalog.path)?;
    assert!(
        report
            .durable_issues
            .iter()
            .any(|issue| issue.contains("orphan-native-decision") && issue.contains("publication")),
        "an issued identity without native payload cannot disappear from integrity: {report:?}"
    );
    assert!(
        mame_coalesce::create_backup(&catalog.path, &catalog.path.with_file_name("bad.backup"))
            .is_err()
    );
    Ok(())
}

#[derive(QueryableByName)]
struct Plan {
    #[diesel(sql_type = Text)]
    detail: String,
}

#[test]
fn edition_scopes_seek_native_target_and_claim_indices_after_unrelated_population() -> TestResult {
    let mut catalog = Catalog::new()?;
    let left = catalog.import("selected")?;
    let right = catalog.import("unrelated")?;
    for snapshot in [&left, &right] {
        let claim = manual(
            RelationshipEndpoint::CatalogRecord(CatalogRecordRef::new(
                snapshot.clone(),
                CatalogRecordKind::Set,
                "same",
            )),
            RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new("scope", "target")),
        );
        app::record_relationship(&catalog.database, &claim)?;
    }
    for index in 0..150 {
        let claim = manual(
            RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new(
                "unrelated",
                format!("{index}/left"),
            )),
            RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new(
                "unrelated",
                format!("{index}/right"),
            )),
        );
        app::record_relationship(&catalog.database, &claim)?;
    }
    catalog.connection.batch_execute("ANALYZE")?;
    let query = concat!(
        "WITH requested_decision_snapshots(snapshot_key) AS (VALUES (?)), requested_relationship_kinds(kind) AS (VALUES ('catalog_set'),('catalog_media_entry'),('no_intro_archive')), ",
        include_str!("../src/storage/db/relationship_scope.sql"),
        " SELECT count(*) AS count FROM scoped_generic_assertions",
    );
    assert_eq!(
        sql_query(query)
            .bind::<Text, _>(left.as_str())
            .get_result::<Count>(&mut catalog.connection)?
            .count,
        1
    );
    let details = sql_query(format!("EXPLAIN QUERY PLAN {query}"))
        .bind::<Text, _>(left.as_str())
        .load::<Plan>(&mut catalog.connection)?
        .into_iter()
        .map(|row| row.detail)
        .collect::<Vec<_>>();
    for index in [
        "inferred_relationship_from_target",
        "inferred_relationship_to_target",
        "manual_relationship_from_target",
        "manual_relationship_to_target",
    ] {
        assert!(
            details
                .iter()
                .any(|row| row.starts_with("SEARCH ") && row.contains(index)),
            "missing native claim index {index}: {details:?}"
        );
    }
    for alias in [
        "identity", "groups", "sets", "media", "archive", "target", "manual", "inferred",
    ] {
        assert!(
            !details.iter().any(|row| row == &format!("SCAN {alias}")),
            "unbounded {alias} scan: {details:?}"
        );
    }
    Ok(())
}

#[test]
fn published_rule_cannot_be_replaced_through_its_unique_revision_identity() -> TestResult {
    let mut catalog = Catalog::new()?;
    let mut claim = manual(
        RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new("rule-guard", "left")),
        RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new("rule-guard", "right")),
    );
    claim.origin = RelationshipOrigin::DerivedCandidate {
        rule: RelationshipRule::new("declared-rule", "v1", "Declared rule")?,
        supporting_assertions: vec![],
    };
    app::record_relationship(&catalog.database, &claim)?;
    let before = app::explain_relationships(&catalog.database)?;
    catalog
        .connection
        .batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF")?;
    catalog.connection.batch_execute("INSERT INTO catalog_relationship_rules(rule_id,rule_key,revision,description) VALUES(9999,'control','v1','Valid new identity')")?;
    assert!(catalog.connection.batch_execute("INSERT OR REPLACE INTO catalog_relationship_rules(rule_id,rule_key,revision,description) SELECT 10000,rule_key,revision,description FROM catalog_relationship_rules WHERE rule_key='declared-rule'").is_err(), "fresh primary key cannot bypass rule revision replacement protection");
    assert_eq!(app::explain_relationships(&catalog.database)?, before);
    Ok(())
}

#[test]
fn all_unresolved_shapes_reject_fresh_id_replacements_of_published_targets() -> TestResult {
    let mut catalog = Catalog::new()?;
    let snapshot = catalog.import("unresolved-guard")?;
    for (kind, key) in [
        (CatalogRecordKind::Set, "missing".into()),
        (
            CatalogRecordKind::SoftwareItem,
            serde_json::to_string(&("", ""))?,
        ),
        (
            CatalogRecordKind::AssetRequirement,
            serde_json::to_string(&("same", "", 0))?,
        ),
    ] {
        app::record_relationship(
            &catalog.database,
            &manual(
                RelationshipEndpoint::CatalogRecord(CatalogRecordRef::new(
                    snapshot.clone(),
                    kind,
                    key,
                )),
                RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new(
                    "unresolved-guard",
                    "target",
                )),
            ),
        )?;
    }
    let before = app::explain_relationships(&catalog.database)?;
    catalog
        .connection
        .batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF")?;
    let mut accepted = Vec::new();
    for kind in ["catalog_set", "software_item", "asset_requirement"] {
        catalog.connection.batch_execute("SAVEPOINT fresh_target; INSERT INTO catalog_relationship_targets(target_id,kind) VALUES(10000,'unresolved_catalog')")?;
        let result=sql_query("INSERT OR REPLACE INTO unresolved_catalog_targets(target_id,kind,snapshot_key,record_kind,declared_name,declared_set_name,declared_list_name,declared_media_order) SELECT 10000,kind,snapshot_key,record_kind,declared_name,declared_set_name,declared_list_name,declared_media_order FROM unresolved_catalog_targets WHERE record_kind=?")
            .bind::<Text,_>(kind).execute(&mut catalog.connection);
        if result.is_ok() {
            accepted.push(kind);
        }
        catalog
            .connection
            .batch_execute("ROLLBACK TO fresh_target; RELEASE fresh_target")?;
    }
    assert!(
        accepted.is_empty(),
        "unique literal identities bypassed replacement guards: {accepted:?}"
    );
    assert_eq!(app::explain_relationships(&catalog.database)?, before);
    Ok(())
}

#[test]
fn software_items_and_export_archives_keep_actual_owners_and_source_free_roundtrips() -> TestResult
{
    use mame_coalesce::domain::NoIntroArchiveId;
    #[derive(QueryableByName)]
    struct NativeItem {
        #[diesel(sql_type = BigInt)]
        set_id: i64,
        #[diesel(sql_type = Text)]
        list_name: String,
    }
    let mut catalog = Catalog::new()?;
    let software = catalog.import_format("software", app::CatalogDocumentFormat::MameSoftwareListXml,
        "<softwarelists><softwarelist name='first'><software name='same'><description>Title</description><year>2026</year><publisher>Publisher</publisher></software></softwarelist><softwarelist name='second'><software name='same'><description>Title</description><year>2026</year><publisher>Publisher</publisher></software></softwarelist></softwarelists>")?;
    let export = catalog.import_format("archives", app::CatalogDocumentFormat::NoIntroDatabase(mame_coalesce::no_intro_db_xml::NoIntroDatabaseMode::ObservedCompatible),
        "<datafile><game name='same'><archive number='0001'/><archive number='0001'/></game></datafile>")?;
    let items = sql_query("SELECT sets.set_id,lists.name AS list_name FROM catalog_sets sets JOIN software_lists lists ON lists.namespace_id=sets.set_group_id JOIN catalog_set_groups groups USING(set_group_id) WHERE groups.snapshot_key=? ORDER BY groups.list_order")
        .bind::<Text,_>(software.as_str()).load::<NativeItem>(&mut catalog.connection)?;
    let archives = sql_query(
        "SELECT archive_id AS count FROM no_intro_archive_descriptions ORDER BY archive_id",
    )
    .load::<Count>(&mut catalog.connection)?;
    assert_eq!(items.len(), 2);
    assert_eq!(archives.len(), 2);
    let destination =
        RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new("native-owner", "review"));
    for item in &items {
        let record = CatalogRecordRef::new(
            software.clone(),
            CatalogRecordKind::SoftwareItem,
            serde_json::to_string(&(&item.list_name, "same"))?,
        )
        .with_owner(CatalogSetId::try_from(item.set_id)?);
        app::record_relationship(
            &catalog.database,
            &manual(
                RelationshipEndpoint::CatalogRecord(record),
                destination.clone(),
            ),
        )?;
    }
    for archive in &archives {
        app::record_relationship(
            &catalog.database,
            &manual(
                RelationshipEndpoint::NoIntroArchive {
                    snapshot: export.clone(),
                    archive_id: NoIntroArchiveId::try_from(archive.count)?,
                },
                destination.clone(),
            ),
        )?;
    }
    assert_eq!(catalog.count("catalog_set_targets")?, 2);
    assert_eq!(
        catalog.count("no_intro_archive_targets")?,
        2,
        "repeated publisher numbers cannot merge real archive owners"
    );
    let before = app::explain_relationships(&catalog.database)?;
    assert_eq!(before.len(), 4);
    for key in ["software", "archives"] {
        std::fs::remove_file(catalog.directory.path().join(format!("{key}.xml")))?;
    }
    assert_eq!(app::explain_relationships(&catalog.database)?, before);
    let wrong_list = CatalogRecordRef::new(
        software.clone(),
        CatalogRecordKind::SoftwareItem,
        serde_json::to_string(&("second", "same"))?,
    )
    .with_owner(CatalogSetId::try_from(items[0].set_id)?);
    assert!(
        app::record_relationship(
            &catalog.database,
            &manual(
                RelationshipEndpoint::CatalogRecord(wrong_list),
                destination.clone()
            )
        )
        .is_err()
    );
    assert!(
        app::record_relationship(
            &catalog.database,
            &manual(
                RelationshipEndpoint::NoIntroArchive {
                    snapshot: software,
                    archive_id: NoIntroArchiveId::try_from(archives[0].count)?
                },
                destination
            )
        )
        .is_err()
    );
    assert_eq!(catalog.count("catalog_relationships")?, 4);
    drop(catalog.database);
    assert!(mame_coalesce::check_integrity(&catalog.path)?.is_clean());
    Ok(())
}

#[test]
fn shared_target_requires_an_issued_uuid_even_when_foreign_keys_are_disabled() -> TestResult {
    let mut catalog = Catalog::new()?;
    catalog.import("issued-uuid")?;
    catalog.connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF; INSERT INTO catalog_relationship_targets(target_id,kind) VALUES(10000,'shared_file')")?;
    assert!(catalog.connection.batch_execute("INSERT INTO shared_file_targets(target_id,content_uuid) VALUES(10000,x'ffffffffffffffffffffffffffffffff')").is_err(), "a well-formed but unissued UUID is not a catalog file owner");
    assert_eq!(catalog.connection.batch_execute("INSERT INTO shared_file_targets(target_id,content_uuid) SELECT 10000,content_uuid FROM catalog_contents"),Ok(()));
    assert_eq!(catalog.count("shared_file_targets")?, 1);
    Ok(())
}

#[test]
fn interning_equal_digest_endpoints_does_not_discard_reflexive_claims() -> TestResult {
    let mut catalog = Catalog::new()?;
    let endpoint = RelationshipEndpoint::ContentObject(ContentIdentity::new(
        ContentDigestAlgorithm::Sha1,
        "0123456789abcdef0123456789abcdef01234567",
    )?);
    let mut claim = manual(endpoint.clone(), endpoint);
    claim.relation_type = RelationshipType::ExactContentIdentity;
    for origin in [
        RelationshipOrigin::UserConclusion,
        RelationshipOrigin::DerivedCandidate {
            rule: RelationshipRule::new("reflexive-digest", "v1", "Declared same-digest witness")?,
            supporting_assertions: vec![],
        },
    ] {
        claim.origin = origin;
        let key = app::record_relationship(&catalog.database, &claim)?;
        let explanation = app::explain_relationships(&catalog.database)?
            .into_iter()
            .find(|row| row.assertion_key == key)
            .ok_or("reflexive claim lost")?;
        assert_eq!(explanation.claim, claim);
    }
    assert_eq!(catalog.count("catalog_relationship_targets")?, 1);
    assert_eq!(catalog.count("digest_values")?, 1);
    assert_eq!(catalog.count("catalog_relationships")?, 2);
    assert_eq!(
        catalog.count("catalog_contents")?,
        0,
        "unscoped digest references cannot issue UUIDs"
    );
    drop(catalog.database);
    assert!(mame_coalesce::check_integrity(&catalog.path)?.is_clean());
    Ok(())
}

#[test]
fn inferred_and_manual_claims_use_native_registry_and_typed_targets() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let database = Database::open(&path)?;
    let subject = RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new("publisher", "left"));
    let target = RelationshipEndpoint::ExternalRecord(ExternalRecordRef::new("publisher", "right"));
    let mut claim = RelationshipClaim {
        relation_type: RelationshipType::CatalogContinuity,
        subject,
        target,
        origin: RelationshipOrigin::UserConclusion,
        evidence: RelationshipEvidence::Rationale {
            reason: "reviewed correspondence".into(),
        },
    };
    let manual = app::record_relationship(&database, &claim)?;
    claim.origin = RelationshipOrigin::DerivedCandidate {
        rule: mame_coalesce::domain::RelationshipRule::new(
            "publisher-correspondence",
            "v1",
            "Publisher correspondence",
        )?,
        supporting_assertions: vec![manual],
    };
    let inferred = app::record_relationship(&database, &claim)?;
    let explanations = app::explain_relationships(&database)?;
    assert_eq!(
        explanations
            .iter()
            .find(|row| row.assertion_key == inferred)
            .ok_or("inferred claim missing")?
            .claim,
        claim
    );
    let mut connection = SqliteConnection::establish(path.as_str())?;
    assert_eq!(sql_query("SELECT count(*) AS count FROM sqlite_schema WHERE type='table' AND name='relationship_assertions'").get_result::<Count>(&mut connection)?.count, 0, "generic endpoint payloads must not remain persisted");
    for (table, expected) in [
        ("catalog_relationships", 2),
        ("manual_catalog_relationships", 1),
        ("inferred_catalog_relationships", 1),
        ("catalog_relationship_targets", 2),
        ("external_catalog_targets", 2),
    ] {
        assert_eq!(
            sql_query(format!("SELECT count(*) AS count FROM {table}"))
                .get_result::<Count>(&mut connection)?
                .count,
            expected,
            "wrong native ownership: {table}"
        );
    }
    Ok(())
}
