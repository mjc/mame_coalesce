use diesel::{Connection, SqliteConnection, connection::SimpleConnection};
use mame_coalesce::{
    database::Database,
    reading_rules::{FileByteContract, FormatFamily, ReadingRulesSpec, issue},
};

fn database() -> mame_coalesce::Result<(tempfile::TempDir, Database, SqliteConnection)> {
    let directory = tempfile::tempdir()?;
    let path = camino::Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))
        .map_err(|error| mame_coalesce::Error::InvalidPath(error.to_string()))?;
    let database = Database::open(&path)?;
    let mut connection = SqliteConnection::establish(path.as_str())
        .map_err(|error| mame_coalesce::Error::DatabaseSchema(error.to_string()))?;
    connection.batch_execute("PRAGMA foreign_keys=ON")?;
    Ok((directory, database, connection))
}

#[test]
fn reading_rules_join_the_imports_immediate_transaction() -> mame_coalesce::Result<()> {
    let (_directory, _database, mut connection) = database()?;
    let rules = ReadingRulesSpec {
        rules_key: "mame-0.289-observed-v3".to_owned(),
        format_family: FormatFamily::Mame,
        dialect: "observed-v3".to_owned(),
        specification_version: "0.289".to_owned(),
        parser_version: "test".to_owned(),
        rules_version: "test".to_owned(),
        replace_nul: None,
        file_byte_contract: Some(FileByteContract::Mame0289MachineRom),
    };
    connection.immediate_transaction(|connection| {
        let first = issue(connection, &rules)?;
        assert_eq!(issue(connection, &rules)?, first);
        Ok(())
    })
}

#[test]
fn reading_rules_load_preserves_explicit_repair_and_file_contract_facets()
-> mame_coalesce::Result<()> {
    let (_directory, _database, mut connection) = database()?;
    for rules in [
        ReadingRulesSpec {
            rules_key: "mame-complete-file-policy".to_owned(),
            format_family: FormatFamily::Mame,
            dialect: "observed-v3".to_owned(),
            specification_version: "0.289".to_owned(),
            parser_version: "test".to_owned(),
            rules_version: "test".to_owned(),
            replace_nul: None,
            file_byte_contract: Some(FileByteContract::Mame0289MachineRom),
        },
        ReadingRulesSpec {
            rules_key: "database-explicit-no-repair".to_owned(),
            format_family: FormatFamily::NoIntroDatabase,
            dialect: "test".to_owned(),
            specification_version: "test".to_owned(),
            parser_version: "test".to_owned(),
            rules_version: "test".to_owned(),
            replace_nul: Some(false),
            file_byte_contract: None,
        },
    ] {
        let id = issue(&mut connection, &rules)?;
        assert_eq!(
            mame_coalesce::reading_rules::load(&mut connection, id)?,
            rules
        );
    }
    Ok(())
}
