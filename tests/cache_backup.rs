use assert_cmd::Command;
use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query, sql_types::BigInt,
};
use mame_coalesce::database::Database;
use predicates::str::contains;
use std::path::PathBuf;
use tempfile::tempdir;

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

fn utf8(path: PathBuf) -> Result<Utf8PathBuf, std::io::Error> {
    Utf8PathBuf::from_path_buf(path)
        .map_err(|path| std::io::Error::other(path.display().to_string()))
}

#[test]
fn cache_backup_restore_and_integrity_are_available_from_the_cli()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let source = utf8(directory.path().join("source.sqlite"))?;
    let backup = utf8(directory.path().join("backup.sqlite"))?;
    let restored = utf8(directory.path().join("restored.sqlite"))?;
    Database::open(&source)?;

    Command::cargo_bin("mame_coalesce")?
        .args([
            "--cache",
            source.as_str(),
            "cache",
            "backup",
            backup.as_str(),
        ])
        .assert()
        .success();

    Command::cargo_bin("mame_coalesce")?
        .args([
            "--cache",
            restored.as_str(),
            "cache",
            "restore",
            backup.as_str(),
        ])
        .assert()
        .success();

    Command::cargo_bin("mame_coalesce")?
        .args(["--cache", restored.as_str(), "cache", "integrity"])
        .assert()
        .success()
        .stdout(contains("Durable catalog: 0 issue(s)"))
        .stdout(contains("Rebuildable inventory: 0 issue(s)"));

    let inventory_cache = utf8(directory.path().join("inventory.sqlite"))?;
    Database::open(&inventory_cache)?;
    let mut connection = SqliteConnection::establish(inventory_cache.as_str())?;
    sql_query("PRAGMA foreign_keys = OFF").execute(&mut connection)?;
    sql_query(
        "INSERT INTO roms (name, size, md5, sha1, crc, game_id) \
         VALUES ('orphan-rom', 0, zeroblob(16), zeroblob(20), zeroblob(4), 999)",
    )
    .execute(&mut connection)?;
    drop(connection);

    Command::cargo_bin("mame_coalesce")?
        .args(["--cache", inventory_cache.as_str(), "cache", "integrity"])
        .assert()
        .failure()
        .stdout(contains("Rebuildable inventory: 1 issue(s)"));

    let invalid_cache = utf8(directory.path().join("must-not-be-created.sqlite"))?;
    let invalid_backup = utf8(directory.path().join("not-a-database.sqlite"))?;
    std::fs::write(invalid_backup.as_std_path(), b"untrusted input")?;
    Command::cargo_bin("mame_coalesce")?
        .args([
            "--cache",
            invalid_cache.as_str(),
            "cache",
            "restore",
            invalid_backup.as_str(),
        ])
        .assert()
        .failure();
    assert!(!invalid_cache.exists());

    Ok(())
}

#[test]
fn integrity_uses_authoritative_ddl_without_diesel_migration_metadata()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempdir()?;
    let cache = utf8(directory.path().join("cache.sqlite"))?;
    Database::open(&cache)?;

    let mut connection = SqliteConnection::establish(cache.as_str())?;
    let migration_tables = sql_query(
        "SELECT COUNT(*) AS count FROM sqlite_schema \
         WHERE type = 'table' AND name = '__diesel_schema_migrations'",
    )
    .get_result::<CountRow>(&mut connection)?
    .count;
    let authoritative_tables = sql_query(
        "SELECT COUNT(*) AS count FROM sqlite_schema \
         WHERE type = 'table' AND name = 'snapshot_publications'",
    )
    .get_result::<CountRow>(&mut connection)?
    .count;
    assert_eq!(migration_tables, 0);
    assert_eq!(authoritative_tables, 1);

    sql_query("DROP TABLE snapshot_publications").execute(&mut connection)?;
    drop(connection);

    Command::cargo_bin("mame_coalesce")?
        .args(["--cache", cache.as_str(), "cache", "integrity"])
        .assert()
        .failure()
        .stdout(contains("schema"));

    Ok(())
}
