#![allow(clippy::expect_used)]

use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Text},
};
use mame_coalesce::database::Database;

fn connection() -> (tempfile::TempDir, SqliteConnection) {
    let directory = tempfile::tempdir().expect("temporary database");
    let path =
        Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite")).expect("UTF-8 path");
    drop(Database::open(&path).expect("create directly from schema"));
    let mut conn = SqliteConnection::establish(path.as_str()).expect("connect");
    conn.batch_execute("PRAGMA foreign_keys = ON")
        .expect("foreign keys");
    conn.batch_execute(
        "INSERT INTO publishing_sources VALUES ('source','Publisher',NULL);
         INSERT INTO catalogs VALUES ('catalog','source','Catalog');
         INSERT INTO documents(document_key) VALUES ('document');
         INSERT INTO parser_interpretations(interpretation_key,format,rules_version)
         VALUES ('parser-mame','mame-listxml','mame-observed-compat-declared-text-v2');
         INSERT INTO parser_interpretations(interpretation_key,format)
         VALUES ('parser-software','mame-softwarelist-xml');
         INSERT INTO catalog_coverage(coverage_id,kind) VALUES (1,'complete');
         INSERT INTO catalog_snapshots(
             snapshot_key,catalog_key,document_key,interpretation_key,coverage_id
         ) VALUES ('mame-snapshot','catalog','document','parser-mame',1),
                  ('software-snapshot','catalog','document','parser-software',1);
         INSERT INTO software_documents VALUES ('software-snapshot','plural_lists');
         INSERT INTO software_wrapper_headers(snapshot_key,build) VALUES ('software-snapshot',NULL);",
    )
    .expect("snapshot parents");
    (directory, conn)
}

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

#[derive(QueryableByName)]
struct TriggerSql {
    #[diesel(sql_type = Text)]
    sql: String,
}

#[test]
fn catalog_has_no_second_mutable_dat_model() {
    let (_directory, mut conn) = connection();
    let rows = sql_query(
        "SELECT count(*) AS count FROM sqlite_schema \
         WHERE type = 'table' AND name IN ('data_files','games','roms','archive_files')",
    )
    .get_result::<CountRow>(&mut conn)
    .expect("stored catalog owners");
    assert_eq!(rows.count, 0, "catalog facts have one native owner");
}

#[test]
fn published_logiqx_occurrences_reject_late_native_payloads() {
    let (_directory, mut conn) = connection();
    conn.batch_execute(
        "INSERT INTO parser_interpretations(interpretation_key,format) VALUES ('parser-logiqx','logiqx');
         INSERT INTO catalog_snapshots(snapshot_key,catalog_key,document_key,interpretation_key,coverage_id)
         VALUES ('logiqx-snapshot','catalog','document','parser-logiqx',1);
         INSERT INTO catalog_set_groups(set_group_id,snapshot_key,kind,list_order)
         VALUES (1,'logiqx-snapshot','root',0);
         INSERT INTO catalog_sets(set_id,set_group_id,source_element_kind,list_order,set_name,source_line,source_column)
         VALUES (1,1,'logiqx_game',0,'game',1,1);
         INSERT INTO logiqx_games(set_id) VALUES(1);
         INSERT INTO logiqx_game_attribute_positions(set_id,field_kind,source_order,source_line,source_column)
         VALUES(1,0,0,1,7);
         INSERT INTO logiqx_document_facts(snapshot_key) VALUES('logiqx-snapshot');
         INSERT INTO asset_occurrences(occurrence_id,record_id,occurrence_order,claim_kind)
         VALUES (1,1,0,'logiqx_rom'),(2,1,1,'logiqx_disk'),(3,1,2,'logiqx_sample');",
    )
    .expect("stage occurrences before publication");
    let publication = "INSERT INTO snapshot_publications(catalog_key,document_key,interpretation_key,snapshot_key)
         VALUES ('catalog','document','parser-logiqx','logiqx-snapshot')";
    let rejected = conn
        .batch_execute(publication)
        .expect_err("missing native payloads must prevent publication");
    assert!(rejected.to_string().contains("exact position closure"));
    // Deliberately corrupt only the publication seal, then restore its guard.
    // Native parents are real and no payload PK exists: a late insertion must
    // fail because the snapshot is published, not because of missing ancestry
    // or a duplicate key. Ordinary writes cannot create this malformed state.
    let guard = sql_query(
        "SELECT sql FROM sqlite_schema WHERE name='logiqx_attribute_positions_publication_guard'",
    )
    .get_result::<TriggerSql>(&mut conn)
    .expect("publication guard")
    .sql;
    conn.batch_execute("DROP TRIGGER logiqx_attribute_positions_publication_guard")
        .expect("temporarily bypass fixture publication guard");
    let staged = conn.batch_execute(publication);
    conn.batch_execute(&guard)
        .expect("restore exact publication guard");
    staged.expect("stage explicitly corrupt published fixture");
    for statement in [
        "INSERT INTO logiqx_rom_claims(occurrence_id,name,evidence_scope,evidence_provenance,source_line,source_column)
         VALUES (1,'late.rom','whole_asset','source_declared',1,1)",
        "INSERT INTO logiqx_disk_claims(occurrence_id,name,evidence_scope,evidence_provenance,source_line,source_column)
         VALUES (2,'late.disk','disk_data','source_declared',1,1)",
        "INSERT INTO logiqx_sample_claims(occurrence_id,name,source_order,source_line,source_column)
         VALUES (3,'late.sample',2,1,1)",
    ] {
        assert!(conn.batch_execute(statement).is_err(), "accepted: {statement}");
    }
}

#[test]
fn native_sets_have_scoped_integer_identity_and_typed_detail_owners() {
    let (_directory, mut conn) = connection();
    conn.batch_execute(
        "INSERT INTO catalog_set_groups(set_group_id,snapshot_key,kind,list_order)
         VALUES (1,'mame-snapshot','root',0),
                (2,'software-snapshot','software_list',0),
                (3,'software-snapshot','software_list',1);
         INSERT INTO software_lists(namespace_id,source_order,name,source_line,source_column)
         VALUES (2,0,'first',1,1),(3,1,'second',1,1);
         INSERT INTO catalog_sets(set_id,set_group_id,source_element_kind,list_order,set_name,source_line,source_column)
         VALUES (1,1,'mame_machine',0,'machine',2,1),
                (2,2,'software_item',0,'same',3,1),
                (3,3,'software_item',0,'same',4,1),
                (4,2,'software_item',1,'same',5,1);
         INSERT INTO mame_machines(
             set_id,description,description_source_order,description_line,description_column,
             is_device,is_device_specified,runnable,runnable_specified,
             is_bios,is_bios_specified,is_mechanical,is_mechanical_specified,
             attributes_line,attributes_column
         ) VALUES (1,'Machine',0,2,1,0,0,1,0,0,0,0,0,2,1);
         INSERT INTO software_items(record_id,source_order,supported,supported_specified,description,year,publisher)
         VALUES (2,0,'yes',0,'First game','1980','Publisher'),
                (3,0,'yes',0,'Second game','1980','Publisher'),
                (4,1,'yes',0,'Same-list duplicate','1981','Publisher');
         INSERT INTO software_item_text_positions(record_id,field_kind,source_order,source_line,source_column)
         SELECT record_id,field_kind,field_kind,1,1 FROM software_items CROSS JOIN
         (SELECT 0 AS field_kind UNION ALL SELECT 1 UNION ALL SELECT 2);
         INSERT INTO software_list_attribute_positions(namespace_id,field_kind,source_order,source_line,source_column)
         SELECT namespace_id,0,0,1,1 FROM software_lists;
         INSERT INTO software_item_attribute_positions(record_id,field_kind,source_order,source_line,source_column)
         SELECT record_id,0,0,1,1 FROM software_items;",
    )
    .expect("scoped set identities and native detail owners");

    let scoped_sets = sql_query(
        "SELECT COUNT(*) AS count FROM catalog_set_groups \
         JOIN catalog_sets USING (set_group_id) WHERE set_name = 'same'",
    )
    .get_result::<CountRow>(&mut conn)
    .expect("scoped sets");
    assert_eq!(scoped_sets.count, 3);
    let software_details = sql_query("SELECT COUNT(*) AS count FROM software_items")
        .get_result::<CountRow>(&mut conn)
        .expect("software item details");
    assert_eq!(software_details.count, 3);

    conn.batch_execute(
        "INSERT INTO snapshot_publications(
             catalog_key,document_key,interpretation_key,snapshot_key
         ) VALUES ('catalog','document','parser-software','software-snapshot')",
    )
    .expect("publish software snapshot");
    assert!(
        conn.batch_execute(
            "INSERT INTO catalog_set_groups(set_group_id,snapshot_key,kind,list_order)
             VALUES (4,'software-snapshot','software_list',2)",
        )
        .is_err()
    );
    assert!(conn
        .batch_execute(
            "INSERT INTO catalog_sets(set_id,set_group_id,source_element_kind,list_order,set_name,source_line,source_column)
             VALUES (5,2,'software_item',2,'after publication',6,1)",
        )
        .is_err());

    assert!(conn
        .batch_execute(
            "INSERT INTO catalog_sets(set_id,set_group_id,source_element_kind,list_order,set_name,source_line,source_column)
             VALUES (5,1,'software_item',1,'wrong group',6,1)",
        )
        .is_err());
}

#[test]
fn native_groups_and_list_details_require_the_matching_parser_format() {
    let (_directory, mut conn) = connection();
    conn.batch_execute(
        "INSERT INTO parser_interpretations(interpretation_key,format) VALUES ('parser-logiqx','logiqx');
         INSERT INTO catalog_snapshots(snapshot_key,catalog_key,document_key,interpretation_key,coverage_id)
         VALUES ('logiqx-snapshot','catalog','document','parser-logiqx',1);",
    )
    .expect("Logiqx snapshot");
    for statement in [
        "INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) VALUES ('mame-snapshot','software_list',0)",
        "INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) VALUES ('logiqx-snapshot','software_list',0)",
        "INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) VALUES ('software-snapshot','root',0)",
    ] {
        assert!(
            conn.batch_execute(statement).is_err(),
            "accepted: {statement}"
        );
    }
    conn.batch_execute(
        "INSERT INTO catalog_set_groups(set_group_id,snapshot_key,kind,list_order)
         VALUES (1,'mame-snapshot','root',0),(2,'logiqx-snapshot','root',0),
                (3,'software-snapshot','software_list',0);
         INSERT INTO software_lists(namespace_id,source_order,name,source_line,source_column) VALUES (3,0,'valid',1,1);",
    )
    .expect("format-matching groups and list details");
    assert!(conn.batch_execute(
        "INSERT INTO software_lists(namespace_id,source_order,name,source_line,source_column) VALUES (2,0,'wrong format',1,1)"
    ).is_err());
}

#[test]
fn native_occurrences_and_details_cannot_cross_set_or_format_ownership() {
    let (_directory, mut conn) = connection();
    conn.batch_execute(
        "INSERT INTO catalog_set_groups(set_group_id,snapshot_key,kind,list_order)
         VALUES (1,'mame-snapshot','root',0),(2,'software-snapshot','software_list',0);
         INSERT INTO software_lists(namespace_id,source_order,name,source_line,source_column)
         VALUES (2,0,'list',1,1);
         INSERT INTO catalog_sets(set_id,set_group_id,source_element_kind,list_order,set_name,source_line,source_column)
         VALUES (1,1,'mame_machine',0,'machine',2,1),(2,2,'software_item',0,'game',3,1);
         INSERT INTO mame_machines(
             set_id,description,description_source_order,description_line,description_column,
             is_device,is_device_specified,runnable,runnable_specified,
             is_bios,is_bios_specified,is_mechanical,is_mechanical_specified,
             attributes_line,attributes_column
         ) VALUES (1,'Machine',0,2,1,0,0,1,0,0,0,0,0,2,1);
         INSERT INTO software_items(record_id,source_order,supported,supported_specified,description,year,publisher)
         VALUES (2,0,'yes',0,'Game','1980','Publisher');
         INSERT INTO software_parts(part_id,record_id,part_name,part_order,source_order,interface,source_line,source_column)
         VALUES (1,2,'cart',0,3,'cart',4,1);
         INSERT INTO software_areas(area_id,part_id,record_id,area_kind,area_order)
         VALUES (1,1,2,'data',0);
         INSERT INTO software_data_areas(
             area_id,area_name,source_order,declared_size_text,width,width_specified,
             endianness,endianness_specified,source_line,source_column
         ) VALUES (1,'rom',0,'8',8,0,'little',0,5,1);
         INSERT INTO catalog_contents(content_uuid) VALUES (zeroblob(16));
         INSERT INTO asset_occurrences(occurrence_id,record_id,occurrence_order,claim_kind)
         VALUES (1,1,0,'mame_rom'),(2,2,0,'software_rom_entry'),(3,2,1,'software_rom_operation');
         INSERT INTO mame_rom_claims(
             occurrence_id,name,evidence_scope,evidence_provenance,dump_status,status_specified,
             source_order,source_line,source_column,optional,optional_specified
         ) VALUES (1,'machine.rom','whole_asset','source_declared','good',0,1,6,1,0,0);
         INSERT INTO software_rom_entries(
             occurrence_id,record_id,area_id,component_order,source_order,name,evidence_scope,
             dump_status,status_specified,source_line,source_column
         ) VALUES (2,2,1,0,0,'game.rom','whole_asset','good',0,7,1),
                  (3,2,1,1,1,NULL,'whole_asset','good',0,8,1);
         INSERT INTO software_file_uses(occurrence_id,record_id,operation)
         VALUES (3,2,'continue');",
    )
    .expect("native occurrence and detail owners");

    assert!(
        conn.batch_execute(
            "INSERT INTO asset_occurrences(record_id,occurrence_order,claim_kind)
             VALUES (1,1,'software_disk_entry')",
        )
        .is_err()
    );
    assert!(
        conn.batch_execute(
            "INSERT INTO asset_occurrences(record_id,occurrence_order,claim_kind,content_uuid)
             VALUES (2,2,'software_rom_operation',zeroblob(16))",
        )
        .is_err()
    );
    assert!(
        conn.batch_execute(
            "INSERT INTO mame_rom_claims(
                 occurrence_id,name,evidence_scope,evidence_provenance,dump_status,status_specified,
                 source_order,source_line,source_column,optional,optional_specified
             ) VALUES (2,'wrong-format.rom','whole_asset','source_declared','good',0,1,8,1,0,0)",
        )
        .is_err()
    );
    assert!(
        conn.batch_execute(
            "INSERT INTO software_rom_entries(
                 occurrence_id,record_id,area_id,component_order,source_order,name,evidence_scope,
                 dump_status,status_specified,source_line,source_column
             ) VALUES (1,1,1,2,2,'wrong-area.rom','whole_asset','good',0,9,1)",
        )
        .is_err()
    );
    assert!(
        conn.batch_execute(
            "INSERT INTO occurrence_digest_assertions(occurrence_id,digest_id,scope,provenance)
             VALUES (999,999,'whole_asset','source_declared')",
        )
        .is_err()
    );
}
