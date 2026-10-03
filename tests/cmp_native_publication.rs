#![allow(clippy::expect_used)]

use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Double, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{
        CatalogKey, CatalogScope, PublishingSourceKey, RelationshipAssertionKey, SnapshotKey,
    },
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const CMP_SOURCE: &str = r"; first document comment
clrmamepro (
  name Catalog description Description version v1 date 2024-01-02
  author Author email author@example.test homepage https://home.test
  url https://url.test comment Header category Arcade header DAT
  forcemerging full forcezipping zip forcepacking split forcenodump required
)
; second document comment
GAME (
  NAME set-a CLONEOF parent DESCRIPTION Description YEAR 1984
  MANUFACTURER Maker REBUILDTO rebuild SAMPLEOF samples REGION USA
  RELEASEYEAR 1982 RELEASEMONTH 01 RELEASEDAY 09 SERIAL serial
  SAMPLE sample.wav ROM ( NAME a.bin SIZE 8 )
)";

#[derive(QueryableByName)]
struct IdValue {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

#[derive(QueryableByName)]
struct TextValue {
    #[diesel(sql_type = Text)]
    value: String,
}

#[derive(QueryableByName)]
struct ParentLink {
    #[diesel(sql_type = Text)]
    link_kind: String,
    #[diesel(sql_type = Text)]
    target_name: String,
}

#[derive(Clone, Copy)]
enum Omission {
    None,
    HeaderPosition(i64),
    SetPosition(i64),
    RomPosition,
    RomNamePosition,
    SetFacts,
    DocumentFacts,
    HeaderPresence,
    HeaderDirectives,
    Comment,
    CommentGap,
}

#[derive(Clone, Copy)]
enum DuplicateLayout {
    None,
    ScalarSampleRom,
    HeaderSet,
}

fn setup(
    format: CatalogDocumentFormat,
    contents: &str,
) -> TestResult<(tempfile::TempDir, Database, SnapshotKey)> {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let database = Database::open(&database_path)?;
    let document_path = Utf8PathBuf::from_path_buf(directory.path().join("source.dat"))
        .map_err(|_| "non-UTF-8 document path")?;
    std::fs::write(&document_path, contents)?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path,
            format,
            source_key: PublishingSourceKey::new("cmp-native-publication-test"),
            source_display_name: "CMP native publication test".into(),
            catalog_key: CatalogKey::new("cmp-native-publication-test"),
            catalog_display_name: "CMP native publication test".into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    Ok((
        directory,
        database,
        report
            .snapshot_key
            .ok_or("successful import lacked snapshot")?,
    ))
}

fn connect(directory: &tempfile::TempDir) -> TestResult<SqliteConnection> {
    Ok(SqliteConnection::establish(
        directory
            .path()
            .join("catalog.sqlite")
            .to_str()
            .ok_or("non-UTF-8 database path")?,
    )?)
}

fn insert_cmp_sample_occurrence(
    connection: &mut SqliteConnection,
    set_id: i64,
    occurrence_order: i64,
) -> TestResult<i64> {
    Ok(sql_query(
        "INSERT INTO asset_occurrences(record_id,occurrence_order,claim_kind,content_uuid) \
         VALUES(?,?,'cmp_sample',NULL) RETURNING occurrence_id AS value",
    )
    .bind::<BigInt, _>(set_id)
    .bind::<BigInt, _>(occurrence_order)
    .get_result::<IdValue>(connection)?
    .value)
}

fn insert_pending_owner(
    connection: &mut SqliteConnection,
    source: &SnapshotKey,
    omission: Omission,
    duplicate: DuplicateLayout,
) -> TestResult<i64> {
    sql_query("PRAGMA foreign_keys=ON").execute(connection)?;
    insert_pending_snapshot(connection, source)?;

    copy_document_owner(connection, source, omission)?;
    copy_set_owner(connection, source, omission, duplicate)
}

fn insert_pending_snapshot(connection: &mut SqliteConnection, source: &SnapshotKey) -> TestResult {
    sql_query(
        "INSERT INTO catalogs(catalog_key,source_key,display_name) \
         SELECT 'pending',source_key,'Pending' FROM catalog_snapshots \
         JOIN catalogs USING(catalog_key) WHERE snapshot_key=?",
    )
    .bind::<Text, _>(source.as_str())
    .execute(connection)?;
    sql_query(
        "INSERT INTO catalog_snapshots(snapshot_key,catalog_key,document_key,interpretation_key,coverage_id) \
         SELECT 'pending','pending',document_key,interpretation_key,coverage_id \
         FROM catalog_snapshots WHERE snapshot_key=?",
    )
    .bind::<Text, _>(source.as_str())
    .execute(connection)?;
    Ok(())
}

fn copy_document_owner(
    connection: &mut SqliteConnection,
    source: &SnapshotKey,
    omission: Omission,
) -> TestResult {
    if !matches!(omission, Omission::DocumentFacts) {
        sql_query(
            "INSERT INTO cmp_documents(snapshot_key,header_present,comment_count) \
             SELECT 'pending',CASE WHEN ? THEN 0 ELSE header_present END,comment_count \
             FROM cmp_documents WHERE snapshot_key=?",
        )
        .bind::<BigInt, _>(i64::from(matches!(omission, Omission::HeaderPresence)))
        .bind::<Text, _>(source.as_str())
        .execute(connection)?;
    }

    sql_query(
        "INSERT INTO cmp_header_facts(snapshot_key,source_block,source_order,source_line,source_column,\
         name,description,version,date,author,email,homepage,url,comment,category) \
         SELECT 'pending',source_block,source_order,source_line,source_column,\
         name,description,version,date,author,email,homepage,url,comment,category \
         FROM cmp_header_facts WHERE snapshot_key=?",
    ).bind::<Text, _>(source.as_str()).execute(connection)?;
    if !matches!(omission, Omission::HeaderDirectives) {
        sql_query(
            "INSERT INTO cmp_header_directives(snapshot_key,header_definition,forcemerging,forcezipping,forcepacking,forcenodump) \
             SELECT 'pending',header_definition,forcemerging,forcezipping,forcepacking,forcenodump \
             FROM cmp_header_directives WHERE snapshot_key=?",
        ).bind::<Text, _>(source.as_str()).execute(connection)?;
    }
    let omitted_field = match omission {
        Omission::HeaderPosition(field) => field,
        _ => -1,
    };
    sql_query(
        "INSERT INTO cmp_header_field_positions(snapshot_key,field_kind,source_field,source_order,is_quoted,source_line,source_column) \
         SELECT 'pending',field_kind,source_field,source_order,is_quoted,source_line,source_column \
         FROM cmp_header_field_positions WHERE snapshot_key=? AND field_kind<>? AND (? OR field_kind<10)",
    ).bind::<Text, _>(source.as_str()).bind::<BigInt, _>(omitted_field)
        .bind::<BigInt, _>(i64::from(!matches!(omission, Omission::HeaderDirectives)))
        .execute(connection)?;
    sql_query(
        "INSERT INTO cmp_comments(snapshot_key,comment_order,text,source_line,source_column) \
         SELECT 'pending',CASE WHEN ? THEN comment_order+3 ELSE comment_order END,text,source_line,source_column \
         FROM cmp_comments WHERE snapshot_key=? AND (? OR comment_order<>1)",
    )
    .bind::<BigInt, _>(i64::from(matches!(omission, Omission::CommentGap)))
    .bind::<Text, _>(source.as_str())
    .bind::<BigInt, _>(i64::from(!matches!(omission, Omission::Comment)))
    .execute(connection)?;

    Ok(())
}

fn copy_set_owner(
    connection: &mut SqliteConnection,
    source: &SnapshotKey,
    omission: Omission,
    duplicate: DuplicateLayout,
) -> TestResult<i64> {
    sql_query(
        "INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) VALUES('pending','root',0)",
    )
    .execute(connection)?;
    let set_id = sql_query(
        "INSERT INTO catalog_sets(set_group_id,source_element_kind,list_order,set_name,source_line,source_column) \
         SELECT target.set_group_id,source.source_element_kind,source.list_order,source.set_name,source.source_line,source.source_column \
         FROM catalog_sets AS source JOIN catalog_set_groups AS source_group USING(set_group_id) \
         JOIN catalog_set_groups AS target ON target.snapshot_key='pending' \
         WHERE source_group.snapshot_key=? RETURNING set_id AS value",
    )
    .bind::<Text, _>(source.as_str())
    .get_result::<IdValue>(connection)?
    .value;

    if !matches!(omission, Omission::SetFacts) {
        sql_query(
            "INSERT INTO cmp_set_facts(record_id,source_block,document_order,description,year,manufacturer,rebuildto,region,release_year_text,release_month_text,release_day_text,serial) \
             SELECT ?,source_block,CASE WHEN ?='header_set' THEN (SELECT source_order FROM cmp_header_facts WHERE snapshot_key='pending') ELSE document_order END,description,year,manufacturer,rebuildto,region,release_year_text,release_month_text,release_day_text,serial \
             FROM cmp_set_facts WHERE record_id=(SELECT set_id FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) WHERE snapshot_key=? LIMIT 1)",
        )
        .bind::<BigInt, _>(set_id)
        .bind::<Text, _>(match duplicate { DuplicateLayout::HeaderSet => "header_set", _ => "none" })
        .bind::<Text, _>(source.as_str())
        .execute(connection)?;
    }

    if !matches!(omission, Omission::SetFacts) {
        copy_parent_links(connection, source, set_id)?;
        let omitted_field = match omission {
            Omission::SetPosition(field) => field,
            _ => -1,
        };
        sql_query(
            "INSERT INTO cmp_set_field_positions(record_id,field_kind,source_field,source_order,is_quoted,source_line,source_column) \
             SELECT ?,field_kind,source_field,source_order,is_quoted,source_line,source_column \
             FROM cmp_set_field_positions WHERE record_id=(SELECT set_id FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) WHERE snapshot_key=? LIMIT 1) AND field_kind<>?",
        )
        .bind::<BigInt, _>(set_id)
        .bind::<Text, _>(source.as_str())
        .bind::<BigInt, _>(omitted_field)
        .execute(connection)?;
    }

    let source_set = sql_query(
        "SELECT set_id AS value FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) WHERE snapshot_key=? LIMIT 1",
    )
    .bind::<Text, _>(source.as_str())
    .get_result::<IdValue>(connection)?
    .value;
    let source_sample_occurrence = sql_query(
        "SELECT samples.occurrence_id AS value FROM cmp_samples AS samples \
         JOIN asset_occurrences AS occurrence USING (occurrence_id) \
         WHERE occurrence.record_id=?",
    )
    .bind::<BigInt, _>(source_set)
    .get_result::<IdValue>(connection)?
    .value;
    let sample_occurrence = copy_occurrence(connection, set_id, source_sample_occurrence)?;
    sql_query(
        "INSERT INTO cmp_samples(occurrence_id,sample_name,source_field,source_order,is_quoted,source_line,source_column) \
         SELECT ?,sample_name,source_field,CASE WHEN ?='duplicate' THEN (SELECT source_order FROM cmp_set_field_positions WHERE record_id=? AND field_kind=0) ELSE source_order END,is_quoted,source_line,source_column \
         FROM cmp_samples WHERE occurrence_id=?",
    )
    .bind::<BigInt, _>(sample_occurrence)
    .bind::<Text, _>(match duplicate {
        DuplicateLayout::ScalarSampleRom => "duplicate",
        _ => "none",
    })
    .bind::<BigInt, _>(set_id)
    .bind::<BigInt, _>(source_sample_occurrence)
    .execute(connection)?;

    let source_occurrence = sql_query(
        "SELECT occurrence_id AS value FROM asset_occurrences WHERE record_id=? AND claim_kind='cmp_rom' LIMIT 1",
    )
    .bind::<BigInt, _>(source_set)
    .get_result::<IdValue>(connection)?
    .value;
    let occurrence_id = copy_occurrence(connection, set_id, source_occurrence)?;
    sql_query(
        "INSERT INTO cmp_rom_claims(occurrence_id,name,size_text,crc_text,crc32_text,md5_text,sha1_text,evidence_scope,evidence_provenance,date,serial,status_text,nodump_present,baddump_present,source_line,source_column) \
         SELECT ?,name,size_text,crc_text,crc32_text,md5_text,sha1_text,evidence_scope,evidence_provenance,date,serial,status_text,nodump_present,baddump_present,source_line,source_column FROM cmp_rom_claims WHERE occurrence_id=?",
    )
    .bind::<BigInt, _>(occurrence_id)
    .bind::<BigInt, _>(source_occurrence)
    .execute(connection)?;
    copy_rom_owner(connection, source_occurrence, occurrence_id, omission)?;
    Ok(set_id)
}

fn copy_occurrence(
    connection: &mut SqliteConnection,
    set_id: i64,
    source_occurrence: i64,
) -> TestResult<i64> {
    Ok(sql_query(
        "INSERT INTO asset_occurrences(record_id,occurrence_order,claim_kind,content_uuid) \
         SELECT ?,occurrence_order,claim_kind,content_uuid FROM asset_occurrences WHERE occurrence_id=? \
         RETURNING occurrence_id AS value",
    )
    .bind::<BigInt, _>(set_id)
    .bind::<BigInt, _>(source_occurrence)
    .get_result::<IdValue>(connection)?
    .value)
}

fn copy_parent_links(
    connection: &mut SqliteConnection,
    source: &SnapshotKey,
    set_id: i64,
) -> TestResult {
    let links = sql_query(
        "SELECT link_kind,target_name FROM clrmamepro_set_links \
         WHERE set_id=(SELECT set_id FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) WHERE snapshot_key=? LIMIT 1)",
    ).bind::<Text, _>(source.as_str()).load::<ParentLink>(connection)?;
    for link in links {
        let relationship = sql_query(
            "INSERT INTO catalog_relationships(assertion_key,origin,snapshot_key) \
             VALUES (?,'source','pending') RETURNING relationship_id AS value",
        )
        .bind::<Text, _>(RelationshipAssertionKey::fresh().as_str())
        .get_result::<IdValue>(connection)?
        .value;
        sql_query("INSERT INTO reported_catalog_relationships(relationship_id,source_reference_kind) VALUES (?,?)")
            .bind::<BigInt,_>(relationship).bind::<Text,_>(format!("clrmamepro_{}",link.link_kind)).execute(connection)?;
        sql_query("INSERT INTO clrmamepro_set_links(set_id,link_kind,target_name,relationship_id) VALUES (?,?,?,?)")
            .bind::<BigInt,_>(set_id).bind::<Text,_>(link.link_kind).bind::<Text,_>(link.target_name)
            .bind::<BigInt,_>(relationship).execute(connection)?;
    }
    Ok(())
}

fn copy_rom_owner(
    connection: &mut SqliteConnection,
    source_occurrence: i64,
    occurrence_id: i64,
    omission: Omission,
) -> TestResult {
    sql_query(
        "INSERT INTO cmp_rom_field_positions(occurrence_id,field_kind,source_field,source_order,is_quoted,source_line,source_column) \
         SELECT ?,field_kind,source_field,source_order,is_quoted,source_line,source_column FROM cmp_rom_field_positions WHERE occurrence_id=? AND (? OR field_kind<>0)",
    )
    .bind::<BigInt, _>(occurrence_id)
    .bind::<BigInt, _>(source_occurrence)
    .bind::<BigInt, _>(i64::from(!matches!(omission, Omission::RomNamePosition)))
    .execute(connection)?;
    if !matches!(omission, Omission::RomPosition) {
        sql_query(
            "INSERT INTO cmp_set_rom_positions(occurrence_id,source_order) \
             SELECT ?,source_order \
             FROM cmp_set_rom_positions WHERE occurrence_id=?",
        )
        .bind::<BigInt, _>(occurrence_id)
        .bind::<BigInt, _>(source_occurrence)
        .execute(connection)?;
    }
    Ok(())
}

fn publish(connection: &mut SqliteConnection) -> diesel::QueryResult<usize> {
    sql_query(
        "INSERT INTO snapshot_publications(snapshot_key,catalog_key,document_key,interpretation_key) \
         SELECT snapshot_key,catalog_key,document_key,interpretation_key FROM catalog_snapshots WHERE snapshot_key='pending'",
    )
    .execute(connection)
}

fn assert_guard<T: std::fmt::Debug>(result: diesel::QueryResult<T>, expected: &str) {
    let error = result
        .expect_err("operation unexpectedly succeeded")
        .to_string();
    assert!(
        error.contains(expected),
        "expected {expected:?}, got {error}"
    );
}

#[test]
fn publication_requires_each_header_scalar_position_and_accepts_restoration() -> TestResult {
    for field_kind in 0..15_i64 {
        let (directory, _database, source) = setup(CatalogDocumentFormat::ClrMamePro, CMP_SOURCE)?;
        let mut connection = connect(&directory)?;
        insert_pending_owner(
            &mut connection,
            &source,
            Omission::HeaderPosition(field_kind),
            DuplicateLayout::None,
        )?;
        assert_guard(
            publish(&mut connection),
            "CMP document facts require complete native ownership",
        );
        sql_query(
            "INSERT INTO cmp_header_field_positions(snapshot_key,field_kind,source_field,source_order,is_quoted,source_line,source_column) \
             SELECT 'pending',field_kind,source_field,source_order,is_quoted,source_line,source_column \
             FROM cmp_header_field_positions WHERE snapshot_key=? AND field_kind=?",
        )
        .bind::<Text, _>(source.as_str())
        .bind::<BigInt, _>(field_kind)
        .execute(&mut connection)?;
        assert_eq!(publish(&mut connection)?, 1, "field kind {field_kind}");
    }
    Ok(())
}

#[test]
fn publication_requires_each_set_scalar_position_and_accepts_restoration() -> TestResult {
    for field_kind in 0..12_i64 {
        let (directory, _database, source) = setup(CatalogDocumentFormat::ClrMamePro, CMP_SOURCE)?;
        let mut connection = connect(&directory)?;
        let set_id = insert_pending_owner(
            &mut connection,
            &source,
            Omission::SetPosition(field_kind),
            DuplicateLayout::None,
        )?;
        assert_guard(
            publish(&mut connection),
            if matches!(field_kind, 1 | 6) {
                "reported source relationships require complete native identity ownership"
            } else {
                "CMP set facts require complete native ownership"
            },
        );
        sql_query(
            "INSERT INTO cmp_set_field_positions(record_id,field_kind,source_field,source_order,is_quoted,source_line,source_column) \
             SELECT ?,field_kind,source_field,source_order,is_quoted,source_line,source_column \
             FROM cmp_set_field_positions WHERE record_id=(SELECT set_id FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) WHERE snapshot_key=? LIMIT 1) AND field_kind=?",
        )
        .bind::<BigInt, _>(set_id)
        .bind::<Text, _>(source.as_str())
        .bind::<BigInt, _>(field_kind)
        .execute(&mut connection)?;
        assert_eq!(publish(&mut connection)?, 1, "field kind {field_kind}");
    }
    Ok(())
}

#[test]
fn publication_requires_document_facts_header_presence_directives_comments_and_contiguous_comments()
-> TestResult {
    for omission in [
        Omission::DocumentFacts,
        Omission::HeaderPresence,
        Omission::HeaderDirectives,
        Omission::Comment,
        Omission::CommentGap,
    ] {
        let (directory, _database, source) = setup(CatalogDocumentFormat::ClrMamePro, CMP_SOURCE)?;
        let mut connection = connect(&directory)?;
        insert_pending_owner(&mut connection, &source, omission, DuplicateLayout::None)?;
        assert_guard(
            publish(&mut connection),
            "CMP document facts require complete native ownership",
        );
    }
    Ok(())
}

#[test]
fn publication_requires_set_fact_and_rom_form_position() -> TestResult {
    for omission in [Omission::SetFacts, Omission::RomPosition] {
        let (directory, _database, source) = setup(CatalogDocumentFormat::ClrMamePro, CMP_SOURCE)?;
        let mut connection = connect(&directory)?;
        insert_pending_owner(&mut connection, &source, omission, DuplicateLayout::None)?;
        let expected = match omission {
            Omission::SetFacts | Omission::RomPosition => {
                "CMP set facts require complete native ownership"
            }
            _ => unreachable!(),
        };
        assert_guard(publish(&mut connection), expected);
    }
    Ok(())
}

#[test]
fn publication_rejects_duplicate_set_and_document_ordinals() -> TestResult {
    for duplicate in [DuplicateLayout::ScalarSampleRom, DuplicateLayout::HeaderSet] {
        let (directory, _database, source) = setup(CatalogDocumentFormat::ClrMamePro, CMP_SOURCE)?;
        let mut connection = connect(&directory)?;
        insert_pending_owner(&mut connection, &source, Omission::None, duplicate)?;
        assert_guard(
            publish(&mut connection),
            "CMP native source positions must be unique",
        );
    }
    Ok(())
}

#[test]
fn position_tables_reject_fractional_ordinals_and_closed_field_kinds() -> TestResult {
    for (table, omission, field_count) in [
        (
            "cmp_header_field_positions",
            Omission::HeaderPosition(0),
            15,
        ),
        ("cmp_set_field_positions", Omission::SetPosition(0), 12),
        ("cmp_rom_field_positions", Omission::RomNamePosition, 12),
    ] {
        let (directory, _database, source) = setup(CatalogDocumentFormat::ClrMamePro, CMP_SOURCE)?;
        let mut connection = connect(&directory)?;
        insert_pending_owner(&mut connection, &source, omission, DuplicateLayout::None)?;
        let owner = match table {
            "cmp_header_field_positions" => "'pending'",
            "cmp_set_field_positions" => {
                "(SELECT set_id FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) WHERE snapshot_key='pending' LIMIT 1)"
            }
            "cmp_rom_field_positions" => {
                "(SELECT occurrence_id FROM cmp_set_rom_positions JOIN asset_occurrences USING(occurrence_id) WHERE record_id=(SELECT set_id FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) WHERE snapshot_key='pending' LIMIT 1))"
            }
            _ => unreachable!(),
        };
        let bad_order = format!(
            "INSERT INTO {table} SELECT {owner},field_kind,source_field,0.5,is_quoted,source_line,source_column FROM {table} WHERE field_kind=0 LIMIT 1"
        );
        let error = sql_query(bad_order)
            .execute(&mut connection)
            .expect_err("fractional source ordinal accepted")
            .to_string();
        assert!(error.contains("CHECK constraint failed"), "{error}");
        // Invalid codes have no native owner; the guard runs before CHECKs.
        let kind_guard = if table == "cmp_rom_field_positions" {
            "CMP positions require a present unpublished native ROM field"
        } else {
            "CMP positions require a present unpublished native field"
        };
        for kind in ["0.5".to_owned(), field_count.to_string()] {
            let bad_kind = format!("INSERT INTO {table} VALUES({owner},{kind},'unknown',99,0,1,1)");
            assert_guard(sql_query(bad_kind).execute(&mut connection), kind_guard);
        }
        let valid = format!(
            "INSERT INTO {table} SELECT {owner},field_kind,source_field,source_order,is_quoted,source_line,source_column FROM {table} WHERE field_kind=0 LIMIT 1"
        );
        assert_eq!(sql_query(valid).execute(&mut connection)?, 1);
        assert_eq!(publish(&mut connection)?, 1, "{table}");
    }
    Ok(())
}

#[test]
fn published_native_tables_reject_late_insert_update_and_delete() -> TestResult {
    let (directory, _database, source) = setup(CatalogDocumentFormat::ClrMamePro, CMP_SOURCE)?;
    let mut connection = connect(&directory)?;
    insert_pending_owner(
        &mut connection,
        &source,
        Omission::None,
        DuplicateLayout::None,
    )?;
    assert_eq!(publish(&mut connection)?, 1);
    let cases = [
        (
            "cmp_documents",
            "UPDATE cmp_documents SET comment_count=comment_count WHERE snapshot_key=(SELECT snapshot_key FROM snapshot_publications LIMIT 1)",
            "DELETE FROM cmp_documents WHERE snapshot_key=(SELECT snapshot_key FROM snapshot_publications LIMIT 1)",
            "INSERT INTO cmp_documents SELECT snapshot_key,header_present,comment_count FROM cmp_documents WHERE snapshot_key=(SELECT snapshot_key FROM snapshot_publications LIMIT 1)",
            "CMP document facts require an unpublished matching interpretation",
        ),
        (
            "cmp_header_field_positions",
            "UPDATE cmp_header_field_positions SET source_order=source_order WHERE snapshot_key=(SELECT snapshot_key FROM snapshot_publications LIMIT 1)",
            "DELETE FROM cmp_header_field_positions WHERE snapshot_key=(SELECT snapshot_key FROM snapshot_publications LIMIT 1)",
            "INSERT INTO cmp_header_field_positions SELECT snapshot_key,field_kind,source_field,source_order,is_quoted,source_line,source_column FROM cmp_header_field_positions WHERE snapshot_key=(SELECT snapshot_key FROM snapshot_publications LIMIT 1) LIMIT 1",
            "CMP positions require a present unpublished native field",
        ),
        (
            "cmp_set_field_positions",
            "UPDATE cmp_set_field_positions SET source_order=source_order WHERE record_id=(SELECT set_id FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) LIMIT 1)",
            "DELETE FROM cmp_set_field_positions WHERE record_id=(SELECT set_id FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) LIMIT 1)",
            "INSERT INTO cmp_set_field_positions SELECT record_id,field_kind,source_field,source_order,is_quoted,source_line,source_column FROM cmp_set_field_positions WHERE record_id=(SELECT set_id FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) LIMIT 1) LIMIT 1",
            "CMP positions require a present unpublished native field",
        ),
        (
            "cmp_set_rom_positions",
            "UPDATE cmp_set_rom_positions SET source_order=source_order WHERE occurrence_id=(SELECT occurrence_id FROM cmp_set_rom_positions JOIN asset_occurrences USING(occurrence_id) JOIN catalog_sets ON set_id=record_id JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) LIMIT 1)",
            "DELETE FROM cmp_set_rom_positions WHERE occurrence_id=(SELECT occurrence_id FROM cmp_set_rom_positions JOIN asset_occurrences USING(occurrence_id) JOIN catalog_sets ON set_id=record_id JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) LIMIT 1)",
            "INSERT INTO cmp_set_rom_positions SELECT occurrence_id,source_order FROM cmp_set_rom_positions WHERE occurrence_id=(SELECT occurrence_id FROM cmp_set_rom_positions JOIN asset_occurrences USING(occurrence_id) JOIN catalog_sets ON set_id=record_id JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) LIMIT 1)",
            "CMP ROM layout requires an unpublished native ROM",
        ),
        (
            "cmp_rom_field_positions",
            "UPDATE cmp_rom_field_positions SET source_order=source_order WHERE occurrence_id=(SELECT occurrence_id FROM cmp_rom_field_positions JOIN asset_occurrences USING(occurrence_id) JOIN catalog_sets ON set_id=record_id JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) LIMIT 1)",
            "DELETE FROM cmp_rom_field_positions WHERE occurrence_id=(SELECT occurrence_id FROM cmp_rom_field_positions JOIN asset_occurrences USING(occurrence_id) JOIN catalog_sets ON set_id=record_id JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) LIMIT 1)",
            "INSERT INTO cmp_rom_field_positions SELECT occurrence_id,field_kind,source_field,source_order,is_quoted,source_line,source_column FROM cmp_rom_field_positions WHERE occurrence_id=(SELECT occurrence_id FROM cmp_rom_field_positions JOIN asset_occurrences USING(occurrence_id) JOIN catalog_sets ON set_id=record_id JOIN catalog_set_groups USING(set_group_id) JOIN snapshot_publications USING(snapshot_key) LIMIT 1) LIMIT 1",
            "CMP positions require a present unpublished native ROM field",
        ),
        (
            "cmp_comments",
            "UPDATE cmp_comments SET text=text WHERE snapshot_key=(SELECT snapshot_key FROM snapshot_publications LIMIT 1)",
            "DELETE FROM cmp_comments WHERE snapshot_key=(SELECT snapshot_key FROM snapshot_publications LIMIT 1)",
            "INSERT INTO cmp_comments SELECT snapshot_key,comment_order,text,source_line,source_column FROM cmp_comments WHERE snapshot_key=(SELECT snapshot_key FROM snapshot_publications LIMIT 1) LIMIT 1",
            "CMP document facts require an unpublished matching interpretation",
        ),
    ];
    for (table, update, delete, insert, insert_guard) in cases {
        let update_guard = match table {
            "cmp_set_rom_positions" => "CMP ROM layout is immutable",
            "cmp_header_field_positions" | "cmp_set_field_positions" => {
                "CMP field positions are immutable"
            }
            "cmp_rom_field_positions" => "CMP ROM positions are immutable",
            _ => "CMP document facts are immutable",
        };
        assert_guard(sql_query(update).execute(&mut connection), update_guard);
        assert_guard(sql_query(delete).execute(&mut connection), update_guard);
        assert_guard(sql_query(insert).execute(&mut connection), insert_guard);
    }
    Ok(())
}

#[test]
fn cmp_native_document_and_set_facts_reject_wrong_format_owners() -> TestResult {
    let xml =
        r#"<datafile><header><name>Wrong format</name></header><game name="game-a"/></datafile>"#;
    let (directory, _database, source) = setup(
        CatalogDocumentFormat::Logiqx(mame_coalesce::logiqx::LogiqxMode::ObservedCompatible),
        xml,
    )?;
    let mut connection = connect(&directory)?;
    sql_query("PRAGMA foreign_keys=ON").execute(&mut connection)?;
    insert_pending_snapshot(&mut connection, &source)?;
    let error = sql_query(
        "INSERT INTO cmp_documents(snapshot_key,header_present,comment_count) VALUES(?,0,0)",
    )
    .bind::<Text, _>("pending")
    .execute(&mut connection)
    .expect_err("CMP document facts accepted a Logiqx snapshot")
    .to_string();
    assert!(
        error.contains("CMP document facts require an unpublished matching interpretation"),
        "{error}"
    );
    sql_query(
        "INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) VALUES('pending','root',0)",
    )
    .execute(&mut connection)?;
    let set_id = sql_query(
        "INSERT INTO catalog_sets(set_group_id,source_element_kind,list_order,set_name,source_line,source_column) \
         SELECT target.set_group_id,source.source_element_kind,source.list_order,source.set_name,source.source_line,source.source_column \
         FROM catalog_sets AS source JOIN catalog_set_groups AS source_group USING(set_group_id) \
         JOIN catalog_set_groups AS target ON target.snapshot_key='pending' \
         WHERE source_group.snapshot_key=? RETURNING set_id AS value",
    )
    .bind::<Text, _>(source.as_str())
    .get_result::<IdValue>(&mut connection)?
    .value;
    let error = sql_query(
        "INSERT INTO cmp_set_facts(record_id,source_block,document_order) VALUES(?,'set',0)",
    )
    .bind::<BigInt, _>(set_id)
    .execute(&mut connection)
    .expect_err("CMP set facts accepted a Logiqx set")
    .to_string();
    assert!(
        error.contains("native details require an unpublished set of the matching format"),
        "{error}"
    );
    Ok(())
}

#[test]
fn cmp_samples_require_integer_ordinals() -> TestResult {
    for (fractional_occurrence_order, fractional_source_order) in [(true, false), (false, true)] {
        let (directory, _database, source) = setup(CatalogDocumentFormat::ClrMamePro, CMP_SOURCE)?;
        let mut connection = connect(&directory)?;
        let set_id = insert_pending_owner(
            &mut connection,
            &source,
            Omission::None,
            DuplicateLayout::None,
        )?;
        let occurrence_result = sql_query(
            "INSERT INTO asset_occurrences(record_id,occurrence_order,claim_kind,content_uuid) \
             VALUES(?,?,'cmp_sample',NULL)",
        )
        .bind::<BigInt, _>(set_id)
        .bind::<Double, _>(if fractional_occurrence_order {
            2.5
        } else {
            2.0
        })
        .execute(&mut connection);
        if fractional_occurrence_order {
            let error = occurrence_result
                .expect_err("asset_occurrences accepted a fractional CMP sample order")
                .to_string();
            assert!(error.contains("CHECK constraint failed"), "{error}");
            sql_query(
                "INSERT INTO asset_occurrences(record_id,occurrence_order,claim_kind,content_uuid) \
                 VALUES(?,2,'cmp_sample',NULL)",
            )
            .bind::<BigInt, _>(set_id)
            .execute(&mut connection)?;
        } else {
            assert_eq!(occurrence_result?, 1);
        }
        let occurrence_id = sql_query("SELECT last_insert_rowid() AS value")
            .get_result::<IdValue>(&mut connection)?
            .value;
        if fractional_source_order {
            let payload_result = sql_query(
                "INSERT INTO cmp_samples(occurrence_id,sample_name,source_field,source_order,is_quoted,source_line,source_column) \
                 VALUES(?,'fractional.wav','SAMPLE',?,0,1,1)",
            )
            .bind::<BigInt, _>(occurrence_id)
            .bind::<Double, _>(14.5)
            .execute(&mut connection);
            let error = payload_result
                .expect_err("cmp_samples accepted a fractional source_order")
                .to_string();
            assert!(error.contains("CHECK constraint failed"), "{error}");
        }
        assert_eq!(sql_query(
            "INSERT INTO cmp_samples(occurrence_id,sample_name,source_field,source_order,is_quoted,source_line,source_column) \
             VALUES(?,'control.wav','SAMPLE',14,0,1,1)",
        ).bind::<BigInt, _>(occurrence_id).execute(&mut connection)?, 1);
        assert_eq!(publish(&mut connection)?, 1);
    }
    Ok(())
}

#[test]
fn cmp_samples_require_matching_sample_occurrences() -> TestResult {
    let (directory, _database, source) = setup(CatalogDocumentFormat::ClrMamePro, CMP_SOURCE)?;
    let mut connection = connect(&directory)?;
    let set_id = insert_pending_owner(
        &mut connection,
        &source,
        Omission::None,
        DuplicateLayout::None,
    )?;
    let rom_occurrence = sql_query(
        "SELECT occurrence_id AS value FROM asset_occurrences \
         WHERE record_id=? AND claim_kind='cmp_rom'",
    )
    .bind::<BigInt, _>(set_id)
    .get_result::<IdValue>(&mut connection)?
    .value;
    assert_guard(
        sql_query(
            "INSERT INTO cmp_samples(occurrence_id,sample_name,source_field,source_order,is_quoted,source_line,source_column) \
             VALUES(?,'wrong-owner.wav','SAMPLE',14,0,1,1)",
        )
        .bind::<BigInt, _>(rom_occurrence)
        .execute(&mut connection),
        "CMP samples require an unpublished matching media entry",
    );
    assert_eq!(publish(&mut connection)?, 1);
    Ok(())
}

#[test]
fn cmp_sample_occurrences_require_native_payloads() -> TestResult {
    let (directory, _database, source) = setup(CatalogDocumentFormat::ClrMamePro, CMP_SOURCE)?;
    let mut connection = connect(&directory)?;
    let set_id = insert_pending_owner(
        &mut connection,
        &source,
        Omission::None,
        DuplicateLayout::None,
    )?;
    let sample_occurrence = insert_cmp_sample_occurrence(&mut connection, set_id, 2)?;
    assert_guard(
        publish(&mut connection),
        "CMP samples require complete native ownership",
    );
    sql_query(
        "INSERT INTO cmp_samples(occurrence_id,sample_name,source_field,source_order,is_quoted,source_line,source_column) \
         VALUES(?,'control.wav','SAMPLE',14,0,1,1)",
    )
    .bind::<BigInt, _>(sample_occurrence)
    .execute(&mut connection)?;
    assert_eq!(publish(&mut connection)?, 1);
    Ok(())
}

#[test]
fn cmp_media_order_requires_contiguous_source_order_with_valid_controls() -> TestResult {
    let sample_payload = |connection: &mut SqliteConnection, occurrence: i64, source_order: i64| {
        sql_query(
            "INSERT INTO cmp_samples(occurrence_id,sample_name,source_field,source_order,is_quoted,source_line,source_column) \
             VALUES(?,'order.wav','SAMPLE',?,0,1,1)",
        )
        .bind::<BigInt, _>(occurrence)
        .bind::<BigInt, _>(source_order)
        .execute(connection)
    };
    // A gap in media order, then a reversed sample/ROM pair. The vendor field
    // creates a free native ordinal, so neither case duplicates native layout.
    for (contents, occurrence_order, source_order) in [
        (CMP_SOURCE.to_owned(), 3, 14),
        (
            CMP_SOURCE.replace(
                "SAMPLE sample.wav ROM",
                "SAMPLE sample.wav vendor value ROM",
            ),
            2,
            13,
        ),
    ] {
        let (directory, _database, source) = setup(CatalogDocumentFormat::ClrMamePro, &contents)?;
        let mut connection = connect(&directory)?;
        let set_id = insert_pending_owner(
            &mut connection,
            &source,
            Omission::None,
            DuplicateLayout::None,
        )?;
        sql_query("SAVEPOINT invalid_order").execute(&mut connection)?;
        let occurrence = insert_cmp_sample_occurrence(&mut connection, set_id, occurrence_order)?;
        sample_payload(&mut connection, occurrence, source_order)?;
        assert_guard(
            publish(&mut connection),
            "CMP media order must match native source order",
        );
        sql_query("ROLLBACK TO invalid_order").execute(&mut connection)?;
        sql_query("RELEASE invalid_order").execute(&mut connection)?;
        // Restoration uses rollback, never an exception to native immutability.
        let control = insert_cmp_sample_occurrence(&mut connection, set_id, 2)?;
        sample_payload(&mut connection, control, 15)?;
        assert_eq!(publish(&mut connection)?, 1);
    }
    Ok(())
}

#[test]
fn cmp_media_occurrence_replace_is_rejected_with_recursive_triggers_disabled() -> TestResult {
    for primary_key_conflict in [true, false] {
        let (directory, _database, source) = setup(CatalogDocumentFormat::ClrMamePro, CMP_SOURCE)?;
        let mut connection = connect(&directory)?;
        let set_id = insert_pending_owner(
            &mut connection,
            &source,
            Omission::None,
            DuplicateLayout::None,
        )?;
        let sample_occurrence = sql_query(
            "SELECT samples.occurrence_id AS value FROM cmp_samples AS samples \
             JOIN asset_occurrences AS occurrence USING (occurrence_id) \
             WHERE occurrence.record_id=?",
        )
        .bind::<BigInt, _>(set_id)
        .get_result::<IdValue>(&mut connection)?
        .value;
        sql_query("PRAGMA recursive_triggers=OFF").execute(&mut connection)?;
        if primary_key_conflict {
            assert_guard(
                sql_query(
                    "INSERT OR REPLACE INTO asset_occurrences(occurrence_id,record_id,occurrence_order,claim_kind,content_uuid) \
                     VALUES(?,?,2,'cmp_sample',NULL)",
                )
                .bind::<BigInt, _>(sample_occurrence)
                .bind::<BigInt, _>(set_id)
                .execute(&mut connection),
                "CMP conflicting media inserts are immutable",
            );
        } else {
            assert_guard(
                sql_query(
                    "INSERT OR REPLACE INTO asset_occurrences(record_id,occurrence_order,claim_kind,content_uuid) \
                     VALUES(?,0,'cmp_sample',NULL)",
                )
                .bind::<BigInt, _>(set_id)
                .execute(&mut connection),
                "CMP conflicting media inserts are immutable",
            );
        }

        let control_occurrence = insert_cmp_sample_occurrence(&mut connection, set_id, 2)?;
        sql_query(
            "INSERT INTO cmp_samples(occurrence_id,sample_name,source_field,source_order,is_quoted,source_line,source_column) \
             VALUES(?,'control.wav','SAMPLE',14,0,1,1)",
        )
        .bind::<BigInt, _>(control_occurrence)
        .execute(&mut connection)?;
        assert_eq!(publish(&mut connection)?, 1);
    }
    Ok(())
}

#[test]
fn cmp_sample_source_declared_digests_are_rejected_but_computed_are_allowed() -> TestResult {
    let (directory, _database, source) = setup(CatalogDocumentFormat::ClrMamePro, CMP_SOURCE)?;
    let mut connection = connect(&directory)?;
    let set_id = insert_pending_owner(
        &mut connection,
        &source,
        Omission::None,
        DuplicateLayout::None,
    )?;
    let sample_occurrence = sql_query(
        "SELECT samples.occurrence_id AS value FROM cmp_samples AS samples \
         JOIN asset_occurrences AS occurrence USING (occurrence_id) \
         WHERE occurrence.record_id=?",
    )
    .bind::<BigInt, _>(set_id)
    .get_result::<IdValue>(&mut connection)?
    .value;
    let digest_id = sql_query(
        "INSERT INTO digest_values(algorithm,digest) VALUES('sha1',zeroblob(20)) \
         RETURNING digest_id AS value",
    )
    .get_result::<IdValue>(&mut connection)?
    .value;
    assert_guard(
        sql_query(
            "INSERT INTO occurrence_digest_assertions(occurrence_id,digest_id,scope,provenance) \
             VALUES(?,?,'whole_asset','source_declared')",
        )
        .bind::<BigInt, _>(sample_occurrence)
        .bind::<BigInt, _>(digest_id)
        .execute(&mut connection),
        "CMP scalar samples cannot declare digests",
    );
    assert_eq!(
        sql_query(
            "INSERT INTO occurrence_digest_assertions(occurrence_id,digest_id,scope,provenance) \
         VALUES(?,?,'whole_asset','computed')",
        )
        .bind::<BigInt, _>(sample_occurrence)
        .bind::<BigInt, _>(digest_id)
        .execute(&mut connection)?,
        1
    );
    assert_eq!(publish(&mut connection)?, 1);
    Ok(())
}

#[test]
fn cmp_samples_require_sample_keyword() -> TestResult {
    let (directory, _database, source) = setup(CatalogDocumentFormat::ClrMamePro, CMP_SOURCE)?;
    let mut connection = connect(&directory)?;
    let set_id = insert_pending_owner(
        &mut connection,
        &source,
        Omission::None,
        DuplicateLayout::None,
    )?;
    sql_query(
        "INSERT INTO asset_occurrences(record_id,occurrence_order,claim_kind,content_uuid) \
         VALUES(?,2,'cmp_sample',NULL)",
    )
    .bind::<BigInt, _>(set_id)
    .execute(&mut connection)?;
    let occurrence_id = sql_query("SELECT last_insert_rowid() AS value")
        .get_result::<IdValue>(&mut connection)?
        .value;
    let error = sql_query(
        "INSERT INTO cmp_samples(occurrence_id,sample_name,source_field,source_order,is_quoted,source_line,source_column) \
         VALUES(?,'invalid-keyword.wav','NOTSAMPLE',14,0,1,1)",
    )
    .bind::<BigInt, _>(occurrence_id)
    .execute(&mut connection)
    .expect_err("cmp_samples accepted a non-SAMPLE source keyword")
    .to_string();
    assert!(error.contains("CHECK constraint failed"), "{error}");
    assert_eq!(sql_query(
        "INSERT INTO cmp_samples(occurrence_id,sample_name,source_field,source_order,is_quoted,source_line,source_column) \
         VALUES(?,'control.wav','sAmPlE',14,0,1,1)",
    ).bind::<BigInt, _>(occurrence_id).execute(&mut connection)?, 1);
    assert_eq!(publish(&mut connection)?, 1);
    Ok(())
}

#[test]
fn cmp_header_facts_require_integer_source_coordinates() -> TestResult {
    for fractional_coordinate in ["source_line", "source_column"] {
        let (directory, _database, source) = setup(CatalogDocumentFormat::ClrMamePro, CMP_SOURCE)?;
        let mut connection = connect(&directory)?;
        sql_query(
            "INSERT INTO catalogs(catalog_key,source_key,display_name) \
             SELECT 'pending',source_key,'Pending' FROM catalog_snapshots \
             JOIN catalogs USING(catalog_key) WHERE snapshot_key=?",
        )
        .bind::<Text, _>(source.as_str())
        .execute(&mut connection)?;
        sql_query(
            "INSERT INTO catalog_snapshots(snapshot_key,catalog_key,document_key,interpretation_key,coverage_id) \
             SELECT 'pending','pending',document_key,interpretation_key,coverage_id \
             FROM catalog_snapshots WHERE snapshot_key=?",
        )
        .bind::<Text, _>(source.as_str())
        .execute(&mut connection)?;
        let (source_line, source_column) = match fractional_coordinate {
            "source_line" => (0.5, 1.0),
            "source_column" => (1.0, 0.5),
            _ => unreachable!(),
        };
        let result = sql_query(format!(
            "INSERT INTO cmp_header_facts(snapshot_key,source_block,source_order,source_line,source_column) \
             VALUES('pending','clrmamepro',0,{source_line},{source_column})"
        ))
        .execute(&mut connection);
        let error = result
            .expect_err("cmp_header_facts accepted a fractional source coordinate")
            .to_string();
        assert!(error.contains("CHECK constraint failed"), "{error}");
        assert_eq!(sql_query(
            "INSERT INTO cmp_header_facts(snapshot_key,source_block,source_order,source_line,source_column) \
             VALUES('pending','clrmamepro',0,1,1)",
        ).execute(&mut connection)?, 1);
    }
    Ok(())
}

#[test]
fn draft_cmp_facts_reject_replace_with_recursive_triggers_disabled() -> TestResult {
    let (directory, _database, source) = setup(CatalogDocumentFormat::ClrMamePro, CMP_SOURCE)?;
    let mut connection = connect(&directory)?;
    let set_id = insert_pending_owner(
        &mut connection,
        &source,
        Omission::None,
        DuplicateLayout::None,
    )?;
    sql_query("PRAGMA recursive_triggers=OFF").execute(&mut connection)?;
    let occurrence_id =
        sql_query("SELECT occurrence_id AS value FROM asset_occurrences WHERE record_id=? AND claim_kind='cmp_rom'")
            .bind::<BigInt, _>(set_id)
            .get_result::<IdValue>(&mut connection)?
            .value;
    let sample_occurrence_id = sql_query(
        "SELECT occurrence_id AS value FROM cmp_samples \
         WHERE occurrence_id IN (SELECT occurrence_id FROM asset_occurrences WHERE record_id=?)",
    )
    .bind::<BigInt, _>(set_id)
    .get_result::<IdValue>(&mut connection)?
    .value;
    let cases = [
        ("cmp_documents", "snapshot_key='pending'".to_owned()),
        ("cmp_header_facts", "snapshot_key='pending'".to_owned()),
        ("cmp_header_directives", "snapshot_key='pending'".to_owned()),
        (
            "cmp_comments",
            "snapshot_key='pending' AND comment_order=0".to_owned(),
        ),
        ("cmp_set_facts", format!("record_id={set_id}")),
        ("clrmamepro_set_links", format!("set_id={set_id}")),
        (
            "cmp_samples",
            format!("occurrence_id={sample_occurrence_id}"),
        ),
        (
            "cmp_header_field_positions",
            "snapshot_key='pending' AND field_kind=0".to_owned(),
        ),
        (
            "cmp_set_field_positions",
            format!("record_id={set_id} AND field_kind=0"),
        ),
        ("cmp_rom_claims", format!("occurrence_id={occurrence_id}")),
        (
            "cmp_rom_field_positions",
            format!("occurrence_id={occurrence_id} AND field_kind=0"),
        ),
        (
            "cmp_set_rom_positions",
            format!("occurrence_id={occurrence_id}"),
        ),
    ];
    for (table, predicate) in cases {
        // table_xinfo also lists virtual columns: exclude those from INSERT,
        // but compare them with the native columns for complete preservation.
        let columns = sql_query(
            "SELECT name AS value FROM pragma_table_xinfo(?) WHERE hidden=0 ORDER BY cid",
        )
        .bind::<Text, _>(table)
        .load::<TextValue>(&mut connection)?
        .into_iter()
        .map(|row| row.value)
        .collect::<Vec<_>>()
        .join(",");
        let replace = format!(
            "INSERT OR REPLACE INTO {table}({columns}) SELECT {columns} FROM {table} WHERE {predicate}"
        );
        assert_guard(
            sql_query(replace).execute(&mut connection),
            "CMP conflicting native inserts are immutable",
        );
    }
    let rewrite = sql_query("INSERT OR REPLACE INTO cmp_comments(snapshot_key,comment_order,text,source_line,source_column) VALUES('pending',0,'mutated',1,1)")
        .execute(&mut connection);
    assert_guard(rewrite, "CMP conflicting native inserts are immutable");
    let comment = sql_query(
        "SELECT text AS value FROM cmp_comments WHERE snapshot_key='pending' AND comment_order=0",
    )
    .get_result::<TextValue>(&mut connection)?
    .value;
    assert_eq!(comment, "; first document comment");
    let coverage =
        sql_query("SELECT comment_count AS value FROM cmp_documents WHERE snapshot_key='pending'")
            .get_result::<IdValue>(&mut connection)?
            .value;
    assert_eq!(coverage, 2);
    assert_eq!(publish(&mut connection)?, 1);
    Ok(())
}

#[test]
fn draft_cmp_positions_reject_replacing_another_fields_source_ordinal() -> TestResult {
    for (table, omission, owner) in [
        (
            "cmp_header_field_positions",
            Omission::HeaderPosition(0),
            "'pending'",
        ),
        (
            "cmp_set_field_positions",
            Omission::SetPosition(0),
            "(SELECT set_id FROM catalog_sets JOIN catalog_set_groups USING(set_group_id) WHERE snapshot_key='pending')",
        ),
        (
            "cmp_rom_field_positions",
            Omission::RomNamePosition,
            "(SELECT occurrence_id FROM asset_occurrences JOIN catalog_sets ON set_id=record_id JOIN catalog_set_groups USING(set_group_id) WHERE snapshot_key='pending' AND claim_kind='cmp_rom')",
        ),
    ] {
        let (directory, _database, source) = setup(CatalogDocumentFormat::ClrMamePro, CMP_SOURCE)?;
        let mut connection = connect(&directory)?;
        insert_pending_owner(&mut connection, &source, omission, DuplicateLayout::None)?;
        sql_query("PRAGMA recursive_triggers=OFF").execute(&mut connection)?;
        let replace = format!(
            "INSERT OR REPLACE INTO {table} SELECT {owner},0,source_field,1,is_quoted,source_line,source_column FROM {table} WHERE field_kind=0 LIMIT 1"
        );
        assert_guard(
            sql_query(replace).execute(&mut connection),
            "CMP conflicting native inserts are immutable",
        );
        let retained = sql_query(format!(
            "SELECT count(*) AS value FROM {table} WHERE field_kind=1 AND source_order=1"
        ))
        .get_result::<IdValue>(&mut connection)?
        .value;
        assert_eq!(
            retained, 2,
            "both original and draft ordinal owners remain in {table}"
        );
        let restore = format!(
            "INSERT INTO {table} SELECT {owner},field_kind,source_field,source_order,is_quoted,source_line,source_column FROM {table} WHERE field_kind=0 LIMIT 1"
        );
        assert_eq!(sql_query(restore).execute(&mut connection)?, 1);
        assert_eq!(publish(&mut connection)?, 1);
    }
    Ok(())
}
