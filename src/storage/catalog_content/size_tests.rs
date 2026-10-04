use super::*;
use crate::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};
use camino::Utf8PathBuf;
use std::fmt::Write as _;

#[derive(QueryableByName)]
struct PlanRow {
    #[diesel(sql_type = Text)]
    detail: String,
}

#[derive(QueryableByName)]
struct IdentityRow {
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
    #[diesel(sql_type = Binary)]
    content_uuid: Vec<u8>,
}

#[test]
fn identity_size_lookup_seeks_native_successors_with_unrelated_software_present()
-> crate::Result<()> {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| crate::Error::InvalidPath("non-UTF-8 database path".into()))?;
    let document_path = Utf8PathBuf::from_path_buf(directory.path().join("software.xml"))
        .map_err(|_| crate::Error::InvalidPath("non-UTF-8 source path".into()))?;
    let database = Database::open(&database_path)?;
    let mut xml = String::from(
        "<softwarelist name=\"plans\"><software name=\"game\"><description>Game</description><year>2000</year><publisher>Example</publisher><part name=\"cart\" interface=\"cart\">",
    );
    for index in 0..250 {
        write!(xml, "<dataarea name=\"area{index}\" size=\"32\"><rom name=\"file{index}\" size=\"4\" sha1=\"{index:040x}\"/><rom size=\"2\" loadflag=\"continue\"/><rom size=\"2\" loadflag=\"ignore\"/><rom size=\"10\" loadflag=\"reload\"/></dataarea>")
            .map_err(|error| crate::Error::InvalidPath(error.to_string()))?;
    }
    xml.push_str("</part></software></softwarelist>");
    std::fs::write(&document_path, xml)?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::MameSoftwareListXml,
            source_key: PublishingSourceKey::new("software-size-plan"),
            source_display_name: "Software size plan".into(),
            catalog_key: CatalogKey::new("software-size-plan"),
            catalog_display_name: "Software size plan".into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    let mut connection = database.pool().get()?;
    let selected = sql_query("SELECT occurrence.occurrence_id, occurrence.content_uuid FROM software_rom_entries AS rom JOIN asset_occurrences AS occurrence USING(occurrence_id) WHERE rom.name='file0'")
        .get_result::<IdentityRow>(&mut connection)?;
    let uuid = content_id(selected.content_uuid)?;
    let sizes = sql_query(identity_size_select())
        .bind::<Binary, _>(uuid.as_bytes().as_slice())
        .load::<ContentFactsRow>(&mut connection)?;
    assert_eq!(sizes.len(), 1);
    assert_eq!(sizes[0].size, 8);
    let plans = sql_query(format!("EXPLAIN QUERY PLAN {}", identity_size_select()))
        .bind::<Binary, _>(uuid.as_bytes().as_slice())
        .load::<PlanRow>(&mut connection)?;
    assert_native_seeks(&plans);
    let point = sql_query(
        "EXPLAIN QUERY PLAN SELECT size FROM catalog_file_size_assertions WHERE occurrence_id=?",
    )
    .bind::<BigInt, _>(selected.occurrence_id)
    .load::<PlanRow>(&mut connection)?;
    assert_native_seeks(&point);
    let capture = sql_query(format!(
        "EXPLAIN QUERY PLAN {}",
        candidate_size_evidence_insert()
    ))
    .bind::<Binary, _>(uuid.as_bytes().as_slice())
    .bind::<BigInt, _>(selected.occurrence_id)
    .bind::<Binary, _>(uuid.as_bytes().as_slice())
    .load::<PlanRow>(&mut connection)?;
    assert_native_seeks(&capture);
    let snapshot = report
        .snapshot_key
        .as_ref()
        .ok_or_else(|| crate::Error::InvalidPath("snapshot missing".into()))?;
    let publication = catalog_guard_plan(
        &mut connection,
        snapshot.as_str(),
        "catalog_linked_file_size_publication",
    )?;
    assert_native_seeks(&publication);
    let issuance = catalog_guard_plan(
        &mut connection,
        snapshot.as_str(),
        "catalog_linked_file_uuid_publication",
    )?;
    assert_scoped_owner_seeks(&issuance);
    Ok(())
}

fn catalog_guard_plan(
    connection: &mut SqliteConnection,
    snapshot: &str,
    trigger: &str,
) -> crate::Result<Vec<PlanRow>> {
    let guard = catalog_guard_query(trigger)?;
    Ok(
        sql_query(format!("EXPLAIN QUERY PLAN SELECT EXISTS ({guard})"))
            .bind::<Text, _>(snapshot)
            .load::<PlanRow>(connection)?,
    )
}

fn catalog_guard_query(trigger: &str) -> crate::Result<String> {
    include_str!("../db/catalog_file_sizes.sql")
        .split(&format!("CREATE TRIGGER {trigger} "))
        .nth(1)
        .and_then(|body| body.split("WHEN EXISTS (").nth(1))
        .and_then(|body| body.split("\n)\nBEGIN").next())
        .map(|body| body.replace("NEW.snapshot_key", "?"))
        .ok_or_else(|| {
            crate::Error::InvalidPath("missing actual catalog size publication predicate".into())
        })
}

fn assert_native_seeks(plans: &[PlanRow]) {
    assert_scoped_owner_seeks(plans);
    let details = plans
        .iter()
        .map(|row| row.detail.as_str())
        .collect::<Vec<_>>();
    assert!(
        details.iter().any(|detail| detail.contains("SEARCH next")
            && detail.contains("area_id=?")
            && detail.contains("component_order=?")),
        "missing successor seek: {details:#?}"
    );
    assert!(
        !details
            .iter()
            .any(|detail| detail.contains("SEARCH next") && !detail.contains("component_order=?")),
        "area-only recursive successor search: {details:#?}"
    );
}

fn assert_scoped_owner_seeks(plans: &[PlanRow]) {
    let details = plans
        .iter()
        .map(|row| row.detail.as_str())
        .collect::<Vec<_>>();
    assert!(
        !details
            .iter()
            .any(|detail| detail.contains("MATERIALIZE catalog_file_size_assertions")),
        "global size materialization: {details:#?}"
    );
    for alias in [
        "declaration",
        "base",
        "next",
        "later",
        "area",
        "part",
        "item",
        "file_use",
        "mame_rom_claims",
        "logiqx_rom_claims",
        "cmp_rom_claims",
        "no_intro_pc_file_claims",
        "no_intro_dat_rom_claims",
        "no_intro_dump_files",
        "no_intro_release_files",
        "identity",
        "known",
        "entry",
        "occurrence",
        "sets",
        "groups",
        "issued",
    ] {
        assert!(
            !details
                .iter()
                .any(|detail| detail.starts_with(&format!("SCAN {alias}"))),
            "unscoped native owner scan: {details:#?}"
        );
    }
}
