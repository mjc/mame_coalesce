use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey, RelationshipType, SnapshotKey},
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const XML: &str = include_str!("../fixtures/specifications/mame-machine-fields.xml");

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

struct Fixture {
    directory: tempfile::TempDir,
    path: Utf8PathBuf,
    document: Utf8PathBuf,
    database: Database,
    snapshot: SnapshotKey,
}

fn request(document_path: Utf8PathBuf) -> CatalogImportRequest {
    CatalogImportRequest {
        document_path,
        format: CatalogDocumentFormat::MameListXml,
        source_key: PublishingSourceKey::new("mame-attribute-relationships"),
        source_display_name: "MAME attribute relationships".to_owned(),
        catalog_key: CatalogKey::new("mame-attribute-relationships"),
        catalog_display_name: "MAME attribute relationships".to_owned(),
        scope: CatalogScope::Complete,
    }
}

fn fixture() -> TestResult<Fixture> {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let document = Utf8PathBuf::try_from(directory.path().join("mame.xml"))?;
    std::fs::write(&document, XML)?;
    let database = Database::open(&path)?;
    let report = app::import_catalog(&database, &request(document.clone()))?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    Ok(Fixture {
        directory,
        path,
        document,
        database,
        snapshot: report.snapshot_key.ok_or("published snapshot missing")?,
    })
}

fn character_location(xml: &str, needle: &str) -> TestResult<(i64, i64)> {
    let offset = xml
        .find(needle)
        .ok_or("attribute token missing from fixture")?;
    let prefix = xml
        .get(..offset)
        .ok_or("attribute token offset is not a UTF-8 boundary")?;
    Ok((
        i64::try_from(
            prefix
                .chars()
                .filter(|&character| character == '\n')
                .count(),
        )? + 1,
        i64::try_from(
            prefix
                .rsplit('\n')
                .next()
                .ok_or("last source line")?
                .chars()
                .count(),
        )? + 1,
    ))
}

#[test]
fn public_relationship_explanations_use_exact_attribute_qnames_and_decoded_locations() -> TestResult
{
    use mame_coalesce::domain::RelationshipOrigin;

    let fixture = fixture()?;
    let expected = [
        ("cloneof", "cloneof=\"parent\""),
        ("romof", "romof=\"parent\""),
        ("sampleof", "sampleof=\"parent\""),
        ("device_ref", "name=\"parent\" tag=\":device\""),
        ("merge", "merge=\"parent.rom\""),
        ("merge", "merge=\"parent.disk\""),
    ];
    let explanations = app::explain_relationships(&fixture.database)?;
    let mut actual = explanations
        .iter()
        .filter(|explanation| explanation.source.is_some())
        .filter_map(|explanation| {
            explanation
                .source_field
                .as_deref()
                .map(|field| (field, explanation))
        })
        .filter(|(_, explanation)| {
            matches!(
                &explanation.claim.origin,
                RelationshipOrigin::SourceAssertion { .. }
            )
        })
        .map(|(field, explanation)| {
            let location = explanation
                .source_location
                .ok_or("native relationship explanation lost its attribute coordinates")?;
            assert!(location.line > 0 && location.column > 0);
            Ok((field.to_owned(), location.line, location.column))
        })
        .collect::<TestResult<Vec<_>>>()?;
    actual.sort();

    let mut wanted = expected
        .iter()
        .map(|(field, needle)| {
            let (line, column) = character_location(XML, needle)?;
            Ok(((*field).to_owned(), line, column))
        })
        .collect::<TestResult<Vec<_>>>()?;
    wanted.sort();
    assert_eq!(actual, wanted, "QName and opening-tag attribute locations");

    assert_eq!(
        explanations
            .iter()
            .filter(|explanation| explanation.claim.relation_type
                == RelationshipType::SourceParentClone)
            .count(),
        1
    );
    assert_eq!(
        explanations
            .iter()
            .filter(|explanation| explanation.claim.relation_type
                == RelationshipType::RuntimeDependency)
            .count(),
        3,
        "romof, sampleof, and device_ref declarations remain separate assertions"
    );
    assert_eq!(
        explanations
            .iter()
            .filter(|explanation| explanation.claim.relation_type == RelationshipType::SourceMerge)
            .count(),
        2,
        "ROM and Disk merge literals each have a source assertion"
    );

    // Neither the input nor the retained original can supply query witnesses.
    std::fs::remove_file(&fixture.document)?;
    std::fs::rename(
        format!("{}.documents", fixture.path),
        fixture.directory.path().join("unavailable.documents"),
    )?;
    assert!(app::load_snapshot_source(&fixture.database, &fixture.snapshot).is_err());
    assert!(!fixture.document.exists());
    assert!(!fixture.directory.path().join("systems/α.cpp").exists());
    assert_eq!(app::explain_relationships(&fixture.database)?, explanations);
    Ok(())
}

fn damage_cloneof_witness(kind: &str) -> TestResult<Fixture> {
    let fixture = fixture()?;
    let control = app::explain_relationships(&fixture.database)?;
    assert!(control.iter().any(|explanation| {
        explanation.source_field.as_deref() == Some("cloneof")
            && explanation.source_location.is_some()
    }));
    let mut connection = SqliteConnection::establish(fixture.path.as_str())?;
    connection.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;
    match kind {
        "missing" => {
            connection.batch_execute(
                "DROP TRIGGER mame_machines_attribute_positions_attribute_position_delete",
            )?;
            sql_query(
                "DELETE FROM mame_machines_attribute_positions \
                 WHERE set_id=(SELECT set_id FROM catalog_sets \
                     JOIN catalog_set_groups USING(set_group_id) \
                     WHERE snapshot_key=? AND set_name='complete') AND field_kind=6",
            )
            .bind::<Text, _>(fixture.snapshot.as_str())
            .execute(&mut connection)?;
        }
        "ordinal" | "coordinate" => {
            connection.batch_execute(
                "DROP TRIGGER mame_machines_attribute_positions_attribute_position_update; \
                 PRAGMA ignore_check_constraints=ON",
            )?;
            let column = if kind == "ordinal" {
                "source_order"
            } else {
                "source_column"
            };
            sql_query(format!(
                "UPDATE mame_machines_attribute_positions SET {column}=1.5 \
                 WHERE set_id=(SELECT set_id FROM catalog_sets \
                     JOIN catalog_set_groups USING(set_group_id) \
                     WHERE snapshot_key=? AND set_name='complete') AND field_kind=6"
            ))
            .bind::<Text, _>(fixture.snapshot.as_str())
            .execute(&mut connection)?;
        }
        _ => return Err("unknown position corruption case".into()),
    }
    let retained = sql_query(
        "SELECT count(*) AS count FROM mame_machine_links AS link \
         JOIN catalog_sets AS sets ON sets.set_id=link.set_id \
         JOIN catalog_set_groups AS groups USING(set_group_id) \
         WHERE groups.snapshot_key=? AND sets.set_name='complete' AND link.link_kind='cloneof'",
    )
    .bind::<Text, _>(fixture.snapshot.as_str())
    .get_result::<Count>(&mut connection)?;
    assert_eq!(
        retained.count, 1,
        "damage only the required attribute witness"
    );
    Ok(fixture)
}

#[test]
fn public_explanations_reject_a_missing_required_attribute_witness() -> TestResult {
    let damaged = damage_cloneof_witness("missing")?;
    assert!(
        app::explain_relationships(&damaged.database).is_err(),
        "a retained source assertion without its required position must fail public explanation"
    );
    Ok(())
}

#[test]
fn public_explanations_reject_fractional_attribute_ordinals_and_coordinates() -> TestResult {
    for kind in ["ordinal", "coordinate"] {
        let damaged = damage_cloneof_witness(kind)?;
        assert!(
            app::explain_relationships(&damaged.database).is_err(),
            "fractional {kind} must fail public explanation for the retained source assertion"
        );
    }
    Ok(())
}

#[test]
fn late_eof_import_rolls_back_mame_attribute_positions_and_relationship_registry() -> TestResult {
    use mame_coalesce::{RestorePolicy, create_backup, restore_backup};

    let fixture = fixture()?;
    let backup = Utf8PathBuf::try_from(fixture.directory.path().join("before.backup"))?;
    let restored_path = Utf8PathBuf::try_from(fixture.directory.path().join("before.sqlite"))?;
    create_backup(&fixture.path, &backup)?;
    let before = app::explain_relationships(&fixture.database)?;
    let before_counts = catalog_counts(&fixture.path)?;

    let failed_document = Utf8PathBuf::try_from(fixture.directory.path().join("late-eof.xml"))?;
    std::fs::write(
        &failed_document,
        "<mame mameconfig='10'><machine name='complete' cloneof='late-parent'><description>Late</description><rom name='late.rom' size='1' sha1='5555555555555555555555555555555555555555' merge='late-merge'/></machine><machine name='broken'>",
    )?;
    let report = app::import_catalog(&fixture.database, &request(failed_document))?;
    assert_eq!(report.status, CatalogImportStatus::Failed);
    assert!(report.snapshot_key.is_none());

    assert_eq!(catalog_counts(&fixture.path)?, before_counts);
    assert_eq!(app::explain_relationships(&fixture.database)?, before);
    restore_backup(&backup, &restored_path, RestorePolicy::CreateNew)?;
    assert_eq!(catalog_counts(&restored_path)?, before_counts);
    let restored = Database::open(&restored_path)?;
    assert_eq!(
        app::load_snapshot_source(&restored, &fixture.snapshot)?,
        XML.as_bytes()
    );
    assert_eq!(app::explain_relationships(&restored)?, before);
    Ok(())
}

fn catalog_counts(path: &Utf8PathBuf) -> TestResult<Vec<i64>> {
    let mut connection = SqliteConnection::establish(path.as_str())?;
    let tables = [
        "mame_actual_attribute_positions",
        "mame_machines_attribute_positions",
        "mame_device_references_attribute_positions",
        "mame_rom_claims_attribute_positions",
        "mame_disk_claims_attribute_positions",
        "catalog_relationships",
        "reported_catalog_relationships",
        "mame_machine_links",
        "mame_device_references",
        "mame_rom_merges",
        "mame_disk_merges",
        "file_id_registries",
        "catalog_contents",
        "asset_occurrences",
    ];
    tables
        .iter()
        .map(|table| {
            Ok(sql_query(format!("SELECT count(*) AS count FROM {table}"))
                .get_result::<Count>(&mut connection)?
                .count)
        })
        .collect()
}
