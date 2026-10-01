#![allow(clippy::expect_used)]

use camino::Utf8PathBuf;
use diesel::{Connection, SqliteConnection, connection::SimpleConnection};
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
         INSERT INTO parser_interpretations(interpretation_key,format) VALUES ('parser','mame_xml');
         INSERT INTO catalog_snapshots(snapshot_key,catalog_key,document_key,interpretation_key)
         VALUES ('snapshot','catalog','document','parser');",
    )
    .expect("parents");
    (directory, conn)
}

#[test]
fn records_have_scoped_integer_identity_not_global_name_identity() {
    let (_directory, mut conn) = connection();
    conn.batch_execute(
        "INSERT INTO record_namespaces(namespace_id,snapshot_key,kind,source_order,source_name)
         VALUES (1,'snapshot','software_list',0,'first'),(2,'snapshot','software_list',1,'second');
         INSERT INTO records(record_id,namespace_id,kind,source_order,source_name,source_line,source_column)
         VALUES (1,1,'software_item',0,'same',1,1),(2,2,'software_item',0,'same',2,1);
         INSERT INTO records(namespace_id,kind,source_order,source_name,source_line,source_column)
         VALUES (1,'software_item',1,'same',3,1);",
    ).expect("same source name can denote distinct ordered native records");
    assert!(conn.batch_execute(
        "INSERT INTO records(namespace_id,kind,source_order,source_name,source_line,source_column)
         VALUES (1,'mame_machine',2,'wrong namespace',4,1)",
    ).is_err());
}

#[test]
fn occurrences_cannot_borrow_another_formats_native_claim_or_content_for_an_operation() {
    let (_directory, mut conn) = connection();
    conn.batch_execute(
        "INSERT INTO record_namespaces(namespace_id,snapshot_key,kind,source_order,source_name)
         VALUES (1,'snapshot','root',0,NULL),(2,'snapshot','software_list',0,'list');
         INSERT INTO records(record_id,namespace_id,kind,source_order,source_name,source_line,source_column)
         VALUES (1,1,'mame_machine',0,'machine',1,1),(2,2,'software_item',0,'game',2,1);
         INSERT INTO catalog_contents(content_uuid) VALUES (zeroblob(16));
         INSERT INTO asset_occurrences(record_id,occurrence_order,claim_kind)
         VALUES (1,0,'mame_rom'),(2,0,'software_rom_entry');",
    ).expect("native occurrences");
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
         VALUES (2,1,'software_rom_operation',zeroblob(16))",
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
