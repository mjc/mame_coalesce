use std::collections::BTreeMap;

use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, connection::SimpleConnection,
    sql_query, sql_types::Text,
};

use crate::storage::{catalog_ids::EditionId, db};

#[derive(QueryableByName)]
struct DeleteGuard {
    #[diesel(sql_type = Text)]
    name: String,
}

// Fault injection in this test's private database. Production guards remain
// installed; the reader must independently reject an already damaged file.
fn remove_delete_guards(connection: &mut SqliteConnection, table: &str) -> crate::Result<()> {
    let guards = sql_query(
        "SELECT name FROM sqlite_schema WHERE type='trigger' AND tbl_name=? \
         AND instr(upper(sql),'BEFORE DELETE')>0",
    )
    .bind::<Text, _>(table)
    .load::<DeleteGuard>(connection)?;
    for guard in guards {
        let quoted = guard.name.replace('"', "\"\"");
        connection.batch_execute(&format!("DROP TRIGGER \"{quoted}\""))?;
    }
    Ok(())
}

fn published_edition_with_invalid_unlinked_rom() -> crate::Result<SqliteConnection> {
    let mut connection = SqliteConnection::establish(":memory:")
        .map_err(|error| crate::Error::DatabaseSchema(error.to_string()))?;
    db::initialize_database(&mut connection)?;
    connection.batch_execute(
        "INSERT INTO catalog_publishers VALUES(1,'fixture','Fixture publisher',NULL);
         INSERT INTO catalogs VALUES(1,1,'fixture:requirements','Requirements fixture');
         INSERT INTO catalog_source_files
             (source_file_id,sha256,byte_length,object_key,codec) VALUES
             (1,zeroblob(32),64,'sha256/requirements-older','zstd'),
             (2,randomblob(32),64,'sha256/requirements-pinned','zstd');
         INSERT INTO catalog_reading_rules VALUES
             (1,'requirements-mame','mame','fixture','0','fixture','1');
         INSERT INTO catalog_coverage VALUES(1,'complete');
         INSERT INTO catalog_editions VALUES
             (101,1,1,1,1,NULL,NULL),
             (102,1,2,1,1,NULL,NULL);",
    )?;

    // The newer publication intentionally has no ROMs. Reading edition 102
    // must not silently reselect edition 101 based on publication time.
    crate::storage::test_catalog::publish_minimal_mame(
        &mut connection,
        EditionId::try_from(101).expect("positive fixture edition"),
        "2026-02-02T00:00:00Z",
    )?;
    crate::storage::test_catalog::publish_minimal_mame_with_rom(
        &mut connection,
        EditionId::try_from(102).expect("positive fixture edition"),
        "2026-02-01T00:00:00Z",
    )?;
    Ok(connection)
}

#[test]
fn root_requirements_pin_exact_publication_and_retain_unlinked_invalid_rom_claims()
-> crate::Result<()> {
    let mut connection = published_edition_with_invalid_unlinked_rom()?;
    let edition_id = EditionId::try_from(102).expect("positive fixture edition");

    let requirements = super::load_root_requirements(&mut connection, edition_id)?;

    assert_eq!(requirements.edition_id, edition_id);
    assert_eq!(requirements.coverage, crate::domain::CatalogScope::Complete);
    assert_eq!(
        requirements.reading_rules.format_family,
        crate::storage::reading_rules::FormatFamily::Mame
    );
    assert_eq!(requirements.reading_rules.replace_nul, None);
    assert_eq!(requirements.reading_rules.file_byte_contract, None);
    assert_eq!(requirements.sets.len(), 2);
    let root = &requirements.sets[0];
    assert_eq!(root.set_id.as_i64(), 1021);
    assert_eq!(root.group_id.as_i64(), 1020);
    assert_eq!(root.name, "machine");
    assert_eq!(root.order, 0);
    assert_eq!(root.roms.len(), 1);
    assert_eq!(requirements.sets[1].set_id.as_i64(), 1025);
    assert_eq!(requirements.sets[1].name, "empty-machine");
    assert!(requirements.sets[1].roms.is_empty());

    let rom = &root.roms[0];
    assert_eq!(rom.media_entry_id.as_i64(), 1023);
    assert_eq!(rom.parent_set_id, root.set_id);
    assert_eq!(rom.order, 1);
    assert_eq!(rom.name, "fixture.rom");
    assert_eq!(rom.size_text.as_deref(), Some("4"));
    assert_eq!(rom.file_uuid, None);
    assert_eq!(rom.hashes.len(), 1);
    assert_eq!(rom.hashes[0].field, super::HashField::Crc);
    assert_eq!(rom.hashes[0].presence, super::HashPresence::Invalid);
    assert_eq!(rom.hashes[0].scope, super::HashScope::Unknown);
    assert_eq!(rom.hashes[0].reported_text.as_deref(), Some("not-hex"));
    assert_eq!(rom.hashes[0].value, None);
    Ok(())
}

#[test]
fn root_reader_format_boundary_is_closed_and_rejects_non_root_families() {
    use crate::storage::reading_rules::FormatFamily;

    for family in [
        FormatFamily::Mame,
        FormatFamily::Logiqx,
        FormatFamily::ClrMamePro,
        FormatFamily::NoIntroDat,
        FormatFamily::NoIntroPcFixture,
    ] {
        assert!(super::ensure_supported_family(family).is_ok());
    }
    assert!(super::ensure_supported_family(FormatFamily::Software).is_err());
    assert!(super::ensure_supported_family(FormatFamily::NoIntroDatabase).is_err());
}

#[test]
fn all_supported_family_queries_resolve_against_canonical_schema() -> crate::Result<()> {
    use crate::storage::reading_rules::FormatFamily;

    let mut connection = SqliteConnection::establish(":memory:")
        .map_err(|error| crate::Error::DatabaseSchema(error.to_string()))?;
    db::initialize_database(&mut connection)?;
    let edition_id = EditionId::try_from(1)?;
    let mut sets = Vec::new();
    let indexes = BTreeMap::new();

    for family in [
        FormatFamily::Mame,
        FormatFamily::Logiqx,
        FormatFamily::ClrMamePro,
        FormatFamily::NoIntroDat,
        FormatFamily::NoIntroPcFixture,
    ] {
        super::validate_root_owner_ancestry(&mut connection, edition_id, family)?;
        super::validate_rom_owner_ancestry(&mut connection, edition_id, family)?;
        super::load_native_parents(&mut connection, edition_id, family, &mut sets, &indexes)?;
        super::load_native_roms(&mut connection, edition_id, family, &mut sets, &indexes)?;
    }
    Ok(())
}

#[test]
fn root_reader_rejects_a_missing_native_root_owner() -> crate::Result<()> {
    let mut connection = published_edition_with_invalid_unlinked_rom()?;
    remove_delete_guards(&mut connection, "mame_machines")?;
    connection.batch_execute(
        "PRAGMA foreign_keys=OFF;
         DELETE FROM mame_machines WHERE set_id=1021;",
    )?;
    let edition_id = EditionId::try_from(102)?;

    let error = super::load_root_requirements(&mut connection, edition_id)
        .expect_err("a root set without its native MAME owner must be rejected");

    assert!(
        error
            .to_string()
            .contains("root set 1021 has missing or wrong mame_machines")
    );
    Ok(())
}

#[test]
fn root_reader_rejects_a_missing_native_rom_owner() -> crate::Result<()> {
    let mut connection = published_edition_with_invalid_unlinked_rom()?;
    remove_delete_guards(&mut connection, "mame_roms")?;
    connection.batch_execute(
        "PRAGMA foreign_keys=OFF;
         DELETE FROM mame_roms WHERE media_entry_id=1023;",
    )?;
    let edition_id = EditionId::try_from(102)?;

    let error = super::load_root_requirements(&mut connection, edition_id)
        .expect_err("a retained ROM source owner without its native row must be rejected");

    assert!(
        error
            .to_string()
            .contains("ROM source element 1023 has missing or wrong mame_roms")
    );
    Ok(())
}
