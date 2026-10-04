//! Populated production-DDL plans for the statements used by the shared loader.
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Binary, Text},
};

use super::{
    ClrMameProEvidenceScope, ClrMameProFilePayload, OccurrenceId, clrmamepro_file_payloads, queries,
};
use crate::storage::db;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(QueryableByName)]
struct ExplainRow {
    #[diesel(sql_type = BigInt)]
    parent: i64,
    #[diesel(sql_type = Text)]
    detail: String,
}

#[test]
fn populated_cmp_payload_statements_seek_requested_owners() -> TestResult {
    let pool = db::create_db_pool(":memory:")?;
    let mut connection = pool.get()?;
    connection.transaction::<_, Box<dyn std::error::Error>, _>(|connection| {
        seed_corpus(connection)?;
        connection.batch_execute("ANALYZE")?;
        let rom_id = OccurrenceId::from_database(1);
        let sample_id = OccurrenceId::from_database(2);
        let ids = [rom_id, sample_id, OccurrenceId::from_database(5)];
        let requested = queries::requested(&ids);
        for (statement, alias) in [
            (queries::owners(), "occurrence"),
            (queries::roms(), "rom"),
            (queries::samples().into(), "sample"),
            (queries::positions().into(), "position"),
            (queries::digests().into(), "assertion"),
            (queries::merges().into(), "declaration"),
        ] {
            assert_plan(connection, &format!("{requested}{statement}"), alias)?;
        }
        let payloads = clrmamepro_file_payloads(connection, &ids)?;
        assert_eq!(payloads.len(), ids.len());
        let Some(ClrMameProFilePayload::Rom(rom)) = payloads.get(&rom_id) else {
            return Err("missing populated CMP ROM payload".into());
        };
        assert_eq!(rom.size, Some(4));
        assert_eq!(rom.evidence_scope, ClrMameProEvidenceScope::WholeAsset);
        assert_eq!(rom.field_positions.len(), 4);
        assert!(rom.merge.is_some());
        let Some(ClrMameProFilePayload::Sample(sample)) = payloads.get(&sample_id) else {
            return Err("missing populated CMP sample payload".into());
        };
        assert_eq!(sample.position.source_order, 2);
        Ok(())
    })
}

fn assert_plan(connection: &mut SqliteConnection, query: &str, alias: &str) -> TestResult {
    let rows = sql_query(format!("EXPLAIN QUERY PLAN {query}")).load::<ExplainRow>(connection)?;
    let details = rows
        .iter()
        .map(|row| row.detail.to_ascii_lowercase())
        .collect::<Vec<_>>();
    let requested = rows
        .iter()
        .zip(&details)
        .position(|(row, detail)| row.parent == 0 && detail == "scan requested");
    let owner = rows.iter().zip(&details).position(|(row, detail)| {
        row.parent == 0
            && detail.starts_with(&format!("search {alias} using "))
            && (detail.contains("occurrence_id=?") || detail.contains("rowid=?"))
    });
    assert!(
        requested
            .zip(owner)
            .is_some_and(|(request, owner)| request < owner),
        "requested IDs must precede indexed {alias}: {details:?}"
    );
    if alias == "occurrence" {
        assert!(
            rows.iter().zip(&details).any(|(row, detail)| {
                row.parent > 0
                    && detail.starts_with("search sample_form using primary key ")
                    && detail.contains("occurrence_id=?")
            }),
            "sample ROM-form exclusion must use a correlated primary-key seek: {details:?}"
        );
    }
    for detail in &details {
        assert!(
            !detail.starts_with("scan ")
                || detail == "scan requested"
                || detail.contains("constant row"),
            "unrelated catalog scan in CMP payload statement: {details:?}"
        );
    }
    Ok(())
}

fn seed_corpus(connection: &mut SqliteConnection) -> TestResult {
    connection.batch_execute(
        "INSERT INTO publishing_sources(source_key,display_name) VALUES ('cmp-plan-source','CMP plans');
         INSERT INTO catalogs(catalog_key,source_key,display_name) VALUES ('cmp-plan-catalog','cmp-plan-source','CMP plans');
         INSERT INTO catalog_coverage(coverage_id,kind) VALUES (1,'complete');"
    )?;
    seed_snapshot(connection, 1, 1, 3)?;
    seed_snapshot(connection, 2, 1000, 512)?;
    for group in 3..=130 {
        seed_snapshot(connection, group, 10_000 + group, 1)?;
    }
    Ok(())
}

fn seed_snapshot(
    connection: &mut SqliteConnection,
    group: i64,
    first_id: i64,
    sets: i64,
) -> TestResult {
    let snapshot = format!("cmp-plan-snapshot-{group}");
    let document = format!("cmp-plan-document-{group}");
    let interpretation = format!("cmp-plan-parser-{group}");
    seed_registry(connection, &snapshot, &document, &interpretation)?;
    sql_query("INSERT INTO catalog_set_groups(set_group_id,snapshot_key,kind,list_order) VALUES (?,?,'root',0)")
        .bind::<BigInt, _>(group).bind::<Text, _>(&snapshot).execute(connection)?;
    sql_query(
        "INSERT INTO cmp_documents(snapshot_key,header_present,comment_count) VALUES (?,0,0)",
    )
    .bind::<Text, _>(&snapshot)
    .execute(connection)?;
    for order in 0..sets {
        seed_set(connection, group, first_id + order, order, &snapshot)?;
    }
    sql_query("INSERT INTO snapshot_publications(catalog_key,document_key,interpretation_key,snapshot_key) VALUES ('cmp-plan-catalog',?,?,?)")
        .bind::<Text, _>(&document).bind::<Text, _>(&interpretation).bind::<Text, _>(&snapshot).execute(connection)?;
    Ok(())
}

fn seed_registry(
    connection: &mut SqliteConnection,
    snapshot: &str,
    document: &str,
    interpretation: &str,
) -> TestResult {
    sql_query("INSERT INTO parser_interpretations(interpretation_key,format,rules_version) VALUES (?,'clrmamepro-dat','clrmamepro-declared-text-compat-v1')")
        .bind::<Text, _>(interpretation).execute(connection)?;
    sql_query("INSERT INTO documents(document_key) VALUES (?)")
        .bind::<Text, _>(document)
        .execute(connection)?;
    sql_query("INSERT INTO catalog_snapshots(snapshot_key,catalog_key,document_key,interpretation_key,coverage_id) VALUES (?,'cmp-plan-catalog',?,?,1)")
        .bind::<Text, _>(snapshot).bind::<Text, _>(document).bind::<Text, _>(interpretation).execute(connection)?;
    Ok(())
}

fn seed_set(
    connection: &mut SqliteConnection,
    group: i64,
    id: i64,
    order: i64,
    snapshot: &str,
) -> TestResult {
    sql_query("INSERT INTO catalog_sets(set_id,set_group_id,source_element_kind,list_order,set_name,source_line,source_column) VALUES (?,?,'cmp_set',?,'game',1,1)")
        .bind::<BigInt, _>(id).bind::<BigInt, _>(group).bind::<BigInt, _>(order).execute(connection)?;
    sql_query(
        "INSERT INTO cmp_set_facts(record_id,source_block,document_order) VALUES (?,'set',?)",
    )
    .bind::<BigInt, _>(id)
    .bind::<BigInt, _>(order)
    .execute(connection)?;
    sql_query("INSERT INTO cmp_set_field_positions(record_id,field_kind,source_field,source_order,is_quoted,source_line,source_column) VALUES (?,0,'name',0,0,1,1)")
        .bind::<BigInt, _>(id).execute(connection)?;
    seed_rom(connection, id, id * 2 - 1, snapshot)?;
    seed_sample(connection, id, id * 2)?;
    Ok(())
}

fn seed_rom(connection: &mut SqliteConnection, set: i64, id: i64, snapshot: &str) -> TestResult {
    sql_query("INSERT INTO asset_occurrences(occurrence_id,record_id,occurrence_order,claim_kind) VALUES (?,?,0,'cmp_rom')")
        .bind::<BigInt, _>(id).bind::<BigInt, _>(set).execute(connection)?;
    let bytes = u32::try_from(id)?.to_be_bytes();
    sql_query("INSERT INTO cmp_rom_claims(occurrence_id,name,size_text,crc_text,evidence_scope,evidence_provenance,nodump_present,baddump_present,source_line,source_column) VALUES (?,'rom.bin','0004',?,'whole_asset','source_declared',0,0,1,1)")
        .bind::<BigInt, _>(id).bind::<Text, _>(hex::encode_upper(bytes)).execute(connection)?;
    sql_query("INSERT INTO cmp_set_rom_positions(occurrence_id,source_order) VALUES (?,1)")
        .bind::<BigInt, _>(id)
        .execute(connection)?;
    sql_query("INSERT INTO digest_values(digest_id,algorithm,digest) VALUES (?,'crc32',?)")
        .bind::<BigInt, _>(id)
        .bind::<Binary, _>(bytes.as_slice())
        .execute(connection)?;
    sql_query("INSERT INTO occurrence_digest_assertions(occurrence_id,digest_id,scope,provenance) VALUES (?,?,'whole_asset','source_declared')")
        .bind::<BigInt, _>(id).bind::<BigInt, _>(id).execute(connection)?;
    seed_merge(connection, id, snapshot)?;
    for (field, keyword, order) in [
        (0, "name", 0),
        (1, "size", 1),
        (2, "crc", 3),
        (6, "merge", 4),
    ] {
        sql_query("INSERT INTO cmp_rom_field_positions(occurrence_id,field_kind,source_field,source_order,is_quoted,source_line,source_column) VALUES (?,?,?,?,0,1,1)")
            .bind::<BigInt, _>(id).bind::<BigInt, _>(field).bind::<Text, _>(keyword).bind::<BigInt, _>(order).execute(connection)?;
    }
    Ok(())
}

fn seed_merge(connection: &mut SqliteConnection, id: i64, snapshot: &str) -> TestResult {
    sql_query("INSERT INTO catalog_relationships(relationship_id,assertion_key,origin,snapshot_key) VALUES (?,?,'source',?)")
        .bind::<BigInt, _>(id).bind::<Text, _>(format!("cmp-plan-merge-{id}")).bind::<Text, _>(snapshot).execute(connection)?;
    sql_query("INSERT INTO reported_catalog_relationships(relationship_id,source_reference_kind) VALUES (?,'clrmamepro_rom_merge')")
        .bind::<BigInt, _>(id).execute(connection)?;
    sql_query("INSERT INTO clrmamepro_rom_merges(occurrence_id,merge_name,relationship_id) VALUES (?,'parent.bin',?)")
        .bind::<BigInt, _>(id).bind::<BigInt, _>(id).execute(connection)?;
    Ok(())
}

fn seed_sample(connection: &mut SqliteConnection, set: i64, id: i64) -> TestResult {
    sql_query("INSERT INTO asset_occurrences(occurrence_id,record_id,occurrence_order,claim_kind) VALUES (?,?,1,'cmp_sample')")
        .bind::<BigInt, _>(id).bind::<BigInt, _>(set).execute(connection)?;
    sql_query("INSERT INTO cmp_samples(occurrence_id,sample_name,source_field,source_order,is_quoted,source_line,source_column) VALUES (?,'','SaMpLe',2,1,1,1)")
        .bind::<BigInt, _>(id).execute(connection)?;
    Ok(())
}
