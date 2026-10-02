use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, sql_query,
    sql_types::{BigInt, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey, SnapshotKey},
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

const XML: &str = r#"<mame mameconfig="10"><machine name="child" cloneof="parent" romof="bios" sampleof="audio">
<description>Child</description><device_ref name="sound" tag=":a"/><device_ref name="sound" tag=":b"/>
</machine></mame>"#;

fn imported() -> TestResult<(tempfile::TempDir, diesel::SqliteConnection, SnapshotKey)> {
    imported_xml(XML)
}

fn imported_xml(
    xml: &str,
) -> TestResult<(tempfile::TempDir, diesel::SqliteConnection, SnapshotKey)> {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let document_path = Utf8PathBuf::try_from(directory.path().join("machines.xml"))?;
    std::fs::write(&document_path, xml)?;
    let database = Database::open(&database_path)?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::MameListXml,
            source_key: PublishingSourceKey::new("native-machine-links"),
            source_display_name: "Native links".into(),
            catalog_key: CatalogKey::new("native-machine-links"),
            catalog_display_name: "Native links".into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    let connection = diesel::SqliteConnection::establish(database_path.as_str())?;
    Ok((
        directory,
        connection,
        report.snapshot_key.ok_or("published snapshot missing")?,
    ))
}

#[test]
fn machine_links_have_one_native_source_owner_not_copied_generic_assertions() -> TestResult {
    let (_directory, mut connection, _snapshot) = imported()?;
    let copied = sql_query(
        "SELECT count(*) AS count FROM relationship_assertions AS assertion \
         JOIN mame_machines AS machine ON machine.set_id=assertion.subject_set_id \
         WHERE assertion.origin='source_assertion'",
    )
    .get_result::<Count>(&mut connection)?;
    assert_eq!(
        copied.count, 0,
        "native MAME links must own their source facts once"
    );
    let registered = sql_query(
        "SELECT count(*) AS count FROM catalog_relationships AS identity \
         JOIN reported_catalog_relationships AS reported USING(relationship_id) \
         WHERE identity.origin='source'",
    )
    .get_result::<Count>(&mut connection)?;
    assert_eq!(
        registered.count, 5,
        "every declared link/reference gets an issued identity"
    );
    let native = sql_query(
        "SELECT (SELECT count(*) FROM mame_machine_links) \
              + (SELECT count(*) FROM mame_device_references) AS count",
    )
    .get_result::<Count>(&mut connection)?;
    assert_eq!(native.count, registered.count);
    let old_table = sql_query(
        "SELECT count(*) AS count FROM sqlite_schema \
         WHERE type='table' AND name='mame_machine_dependencies'",
    )
    .get_result::<Count>(&mut connection)?;
    assert_eq!(
        old_table.count, 0,
        "no second generic native dependency payload"
    );
    Ok(())
}

#[test]
fn source_relationships_cannot_bypass_native_owners_by_omitting_owner_id() -> TestResult {
    let (_directory, mut connection, snapshot) = imported()?;
    let copied = sql_query(
        "INSERT INTO relationship_assertions \
         (assertion_key,relation_type,origin,source_snapshot_key,source_field, \
          subject_kind,source_subject_a,target_kind,source_target_a) \
         VALUES ('copied-without-owner','source_parent_clone','source_assertion',?,'cloneof', \
                 'catalog_set','child','catalog_set','parent')",
    )
    .bind::<Text, _>(snapshot.as_str())
    .execute(&mut connection);
    assert!(
        copied.is_err(),
        "source facts cannot bypass native ownership with a NULL set id"
    );
    Ok(())
}

#[test]
fn empty_references_and_repeated_machine_names_keep_distinct_native_owners() -> TestResult {
    use mame_coalesce::catalog_machines::{
        MachineDependency, MachinePageLimit, machines_for_snapshot,
    };
    let xml = r#"<mame mameconfig="10"><machine name="repeated" cloneof="" romof="" sampleof="">
        <description>First</description><device_ref name="" tag=""/></machine>
        <machine name="repeated" cloneof="parent"><description>Second</description></machine></mame>"#;
    let (directory, _connection, snapshot) = imported_xml(xml)?;
    let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&path)?;
    let page = machines_for_snapshot(&database, &snapshot, None, MachinePageLimit::new(3)?)?;
    assert_eq!(page.machines.len(), 2);
    assert_ne!(page.machines[0].id, page.machines[1].id);
    assert_eq!(page.machines[0].name, page.machines[1].name);
    assert_eq!(page.machines[0].facts.description, "First");
    assert_eq!(page.machines[1].facts.description, "Second");
    let dependencies = &page.machines[0].dependencies;
    for field in ["cloneof", "romof", "sampleof"] {
        assert!(
            dependencies
                .iter()
                .any(|dependency| match (field, dependency) {
                    ("cloneof", MachineDependency::CloneOf { target_name, .. })
                    | ("romof", MachineDependency::RomOf { target_name, .. })
                    | ("sampleof", MachineDependency::SampleOf { target_name, .. }) =>
                        target_name.is_empty(),
                    _ => false,
                })
        );
    }
    assert!(dependencies.iter().any(|dependency| matches!(dependency,
        MachineDependency::DeviceReference(reference) if reference.name.is_empty() && reference.tag.is_empty())));
    Ok(())
}

#[test]
fn merge_resolution_requires_one_parent_owner_not_just_one_matching_file() -> TestResult {
    use mame_coalesce::domain::RelationshipType;
    for ambiguous in [false, true] {
        let extra_parent = if ambiguous {
            "<machine name=\"parent\"><description>Other parent without that file</description></machine>"
        } else {
            ""
        };
        let xml = format!(
            r#"<mame mameconfig="10">
            <machine name="parent"><description>Parent</description><rom name="base" size="1"/></machine>
            {extra_parent}<machine name="child" romof="parent"><description>Child</description>
            <rom name="use" size="1" merge="base"/></machine></mame>"#
        );
        let (directory, _connection, _snapshot) = imported_xml(&xml)?;
        let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
        let database = Database::open(&path)?;
        let explanations = app::explain_relationships(&database)?;
        let merges = explanations
            .iter()
            .filter(|explanation| {
                explanation.claim.relation_type == RelationshipType::ExactContentIdentity
            })
            .count();
        assert_eq!(
            merges,
            usize::from(!ambiguous),
            "ambiguous parent ownership cannot become an exact-content claim"
        );
    }
    Ok(())
}
