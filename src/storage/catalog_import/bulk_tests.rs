use std::{
    collections::BTreeMap,
    fmt::Write as _,
    sync::{Arc, Mutex},
};

use diesel::{Connection, RunQueryDsl, connection::InstrumentationEvent, sql_query};

use crate::{
    app::{CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(diesel::QueryableByName)]
struct Counts {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    machines: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    options: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    positions: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    digests: i64,
}

#[derive(diesel::QueryableByName)]
struct Count {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    count: i64,
}

#[derive(diesel::QueryableByName)]
struct Facts {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    linked: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    identities: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    claims: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    contradictions: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    disputes: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    candidate_sizes: i64,
}

fn document(count: usize, valid_eof: bool) -> Result<String, std::fmt::Error> {
    let mut xml = String::from("<mame build='0.289' debug='no' mameconfig='10'>");
    for index in 0..count {
        write!(
            xml,
            "<machine name='machine-{index}'><description>Machine {index}</description>"
        )?;
        write!(
            xml,
            "<rom name='rom-{index}' size='8' crc='{index:08x}' sha1='{index:040x}'/>"
        )?;
        write!(xml, "<device_ref name='device-{index}' tag=':device'/>")?;
        xml.push_str("<slot name='slot'>");
        for option in 0..4 {
            write!(
                xml,
                "<slotoption name='option-{index}-{option}' devname='device-{index}'/>"
            )?;
        }
        xml.push_str("</slot></machine>");
    }
    xml.push_str(if valid_eof { "</mame>" } else { "<broken" });
    Ok(xml)
}

fn request(path: camino::Utf8PathBuf) -> CatalogImportRequest {
    CatalogImportRequest {
        document_path: path,
        format: CatalogDocumentFormat::MameListXml,
        source_key: PublishingSourceKey::new("bulk-source"),
        source_display_name: "Bulk source".into(),
        catalog_key: CatalogKey::new("bulk-catalog"),
        catalog_display_name: "Bulk catalog".into(),
        scope: CatalogScope::Complete,
    }
}

#[test]
#[expect(
    clippy::expect_used,
    reason = "test instrumentation must fail on poisoned counts"
)]
fn streaming_import_batches_small_owners_and_preserves_the_tail() -> TestResult {
    let database = Database::in_memory()?;
    let executions = Arc::new(Mutex::new(BTreeMap::<&'static str, usize>::new()));
    let observed = Arc::clone(&executions);
    database
        .pool()
        .get()?
        .set_instrumentation(move |event: InstrumentationEvent<'_>| {
            if let InstrumentationEvent::StartQuery { query, .. } = event {
                let sql = query.to_string();
                let table = sql
                    .strip_prefix("INSERT INTO ")
                    .or_else(|| sql.strip_prefix("INSERT OR IGNORE INTO "))
                    .and_then(|rest| rest.split(['(', ' ', '\n']).next());
                let group = match table {
                    Some("mame_machine_slot_options") => Some("options"),
                    Some("mame_machine_slot_options_attribute_positions") => {
                        Some("option positions")
                    }
                    Some("digest_values") => Some("digest dictionary"),
                    _ => None,
                };
                if let Some(group) = group {
                    *observed
                        .lock()
                        .expect("query counts")
                        .entry(group)
                        .or_default() += 1;
                }
            }
        });
    let directory = tempfile::tempdir()?;
    let path = camino::Utf8PathBuf::try_from(directory.path().join("bulk.xml"))?;
    std::fs::write(&path, document(65, true)?)?;
    let report = super::import(database.pool(), &request(path))?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    let executions = executions.lock().expect("query counts");
    eprintln!("bulk insert executions: {executions:?}");
    for group in ["options", "option positions", "digest dictionary"] {
        let count = executions
            .get(group)
            .copied()
            .expect("the import must execute the measured writer");
        assert!(
            count <= 16,
            "{group} must be batched across owners, not executed per child: {executions:?}"
        );
    }
    drop(executions);
    let mut conn = database.pool().get()?;
    let counts = sql_query("SELECT (SELECT count(*) FROM mame_machines) AS machines, (SELECT count(*) FROM mame_machine_slot_options) AS options, (SELECT count(*) FROM mame_machine_slot_options_attribute_positions) AS positions, (SELECT count(*) FROM occurrence_digest_assertions) AS digests")
        .get_result::<Counts>(&mut conn)?;
    assert_eq!(
        (
            counts.machines,
            counts.options,
            counts.positions,
            counts.digests
        ),
        (65, 260, 520, 130)
    );
    Ok(())
}

#[test]
fn late_parse_failure_rolls_back_already_flushed_batches() -> TestResult {
    let database = Database::in_memory()?;
    let directory = tempfile::tempdir()?;
    let path = camino::Utf8PathBuf::try_from(directory.path().join("broken.xml"))?;
    std::fs::write(&path, document(65, false)?)?;
    let report = super::import(database.pool(), &request(path))?;
    assert_eq!(report.status, CatalogImportStatus::Failed);
    let count = sql_query("SELECT (SELECT count(*) FROM catalog_sets) + (SELECT count(*) FROM asset_occurrences) + (SELECT count(*) FROM catalog_contents) + (SELECT count(*) FROM digest_values) + (SELECT count(*) FROM catalog_relationships) + (SELECT count(*) FROM snapshot_publications) AS count")
        .get_result::<Count>(&mut database.pool().get()?)?;
    assert_eq!(count.count, 0);
    Ok(())
}

#[test]
fn cross_batch_dedup_preserves_conflict_evidence_and_source_facts() -> TestResult {
    let database = Database::in_memory()?;
    let mut xml = String::from("<mame mameconfig='10'>");
    for index in 0..66 {
        let size = if index == 64 { 9 } else { 8 };
        write!(
            xml,
            "<machine name='machine-{index}'><description>Machine {index}</description><rom name='rom-{index}' size='{size}' crc='12345678' sha1='0123456789012345678901234567890123456789'/></machine>"
        )?;
    }
    xml.push_str("</mame>");
    let directory = tempfile::tempdir()?;
    let path = camino::Utf8PathBuf::try_from(directory.path().join("conflicts.xml"))?;
    std::fs::write(&path, xml)?;
    let report = super::import(database.pool(), &request(path))?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    let facts = sql_query("SELECT (SELECT count(content_uuid) FROM asset_occurrences) AS linked, (SELECT count(*) FROM catalog_contents) AS identities, (SELECT count(*) FROM mame_rom_claims) AS claims, (SELECT count(*) FROM occurrence_content_conflicts WHERE reason='contradictory_assertions') AS contradictions, (SELECT count(*) FROM occurrence_content_conflicts WHERE reason='disputed_alias') AS disputes, (SELECT count(*) FROM occurrence_content_conflict_sizes WHERE role='candidate') AS candidate_sizes")
        .get_result::<Facts>(&mut database.pool().get()?)?;
    assert_eq!((facts.linked, facts.identities, facts.claims), (64, 1, 66));
    assert_eq!(
        (facts.contradictions, facts.disputes, facts.candidate_sizes),
        (1, 1, 128)
    );
    Ok(())
}
