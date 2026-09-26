use diesel::{
    SqliteConnection,
    connection::{AnsiTransactionManager, SimpleConnection, TransactionManager},
    r2d2::ConnectionManager,
};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

use super::Pool;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

#[derive(Debug)]
struct EnableForeignKeys;

impl diesel::r2d2::CustomizeConnection<SqliteConnection, diesel::r2d2::Error>
    for EnableForeignKeys
{
    fn on_acquire(&self, conn: &mut SqliteConnection) -> Result<(), diesel::r2d2::Error> {
        conn.batch_execute("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 5000")
            .map_err(diesel::r2d2::Error::QueryError)
    }
}

fn run_startup_migrations(conn: &mut SqliteConnection) -> crate::Result<()> {
    let pending = conn
        .pending_migrations(MIGRATIONS)
        .map_err(|error| crate::Error::Migration(error.to_string()))?;

    for migration in pending {
        if let Err(error) = conn.run_migration(migration.as_ref()) {
            let migration_error = format!("{}: {error}", migration.name());
            return match AnsiTransactionManager::rollback_transaction(conn) {
                Ok(()) | Err(diesel::result::Error::NotInTransaction) => {
                    Err(crate::Error::Migration(migration_error))
                }
                Err(rollback_error) => Err(crate::Error::Migration(format!(
                    "{migration_error}; failed to roll back migration: {rollback_error}"
                ))),
            };
        }
    }

    Ok(())
}

pub fn create_db_pool(database_url: &str) -> crate::Result<Pool> {
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    let pool = Pool::builder()
        .connection_customizer(Box::new(EnableForeignKeys))
        .build(manager)?;
    {
        let mut conn = pool.get()?;
        run_startup_migrations(&mut conn)?;
    }
    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::{
        connection::SimpleConnection,
        migration::MigrationSource,
        prelude::*,
        sql_query,
        sql_types::{BigInt, Integer},
    };

    const IDENTITY_MIGRATION: &str = "2026-09-24-000000_create_catalog_identity_schema";
    const SCOPED_NAMES_MIGRATION: &str = "2026-04-22-153500_scope_game_and_rom_names";

    #[derive(QueryableByName)]
    struct CountRow {
        #[diesel(sql_type = BigInt)]
        count: i64,
    }

    #[derive(QueryableByName)]
    struct IdRow {
        #[diesel(sql_type = Integer)]
        id: i32,
    }

    #[derive(QueryableByName)]
    struct TextRow {
        #[diesel(sql_type = diesel::sql_types::Text)]
        value: String,
    }

    fn count(conn: &mut SqliteConnection, table: &str) -> QueryResult<i64> {
        sql_query(format!("SELECT COUNT(*) AS count FROM {table}"))
            .get_result::<CountRow>(conn)
            .map(|row| row.count)
    }

    fn sql_fails(conn: &mut SqliteConnection, statement: &str) -> bool {
        sql_query(statement).execute(conn).is_err()
    }

    fn seed_then_delete_high_rebuilt_ids(conn: &mut SqliteConnection) -> QueryResult<()> {
        conn.batch_execute(
            "INSERT INTO games (id, name) VALUES (23, 'surviving-game');
             INSERT INTO games (id, name) VALUES (90023, 'deleted-high-game');
             DELETE FROM games WHERE id = 90023;
             INSERT INTO roms (id, name, size, md5, sha1, crc)
                 VALUES (31, 'surviving.rom', 1, X'01', X'02', X'03');
             INSERT INTO roms (id, name, size, md5, sha1, crc)
                 VALUES (90031, 'deleted-high.rom', 1, X'04', X'05', X'06');
             DELETE FROM roms WHERE id = 90031;
             INSERT INTO rom_files
                 (id, parent_path, path, name, sha1, xxhash3, in_archive)
                 VALUES (41, '/roms', '/roms/surviving.rom', 'surviving.rom',
                         X'02', X'04', 0);
             INSERT INTO rom_files
                 (id, parent_path, path, name, sha1, xxhash3, in_archive)
                 VALUES (90041, '/roms', '/roms/deleted-high.rom',
                         'deleted-high.rom', X'05', X'06', 0);
             DELETE FROM rom_files WHERE id = 90041;",
        )
    }

    fn assert_rebuilt_ids_advance_past(conn: &mut SqliteConnection) -> QueryResult<()> {
        conn.batch_execute(
            "INSERT INTO games (name) VALUES ('new-game');
             INSERT INTO roms (name, size, md5, sha1, crc)
                 VALUES ('new.rom', 1, X'11', X'12', X'13');
             INSERT INTO rom_files
                 (parent_path, path, name, sha1, xxhash3, in_archive)
                 VALUES ('/roms', '/roms/new.rom', 'new.rom', X'12', X'14', 0);",
        )?;

        for (table, name, prior) in [
            ("games", "new-game", 90_023),
            ("roms", "new.rom", 90_031),
            ("rom_files", "new.rom", 90_041),
        ] {
            let id = sql_query(format!("SELECT id FROM {table} WHERE name = '{name}'"))
                .get_result::<IdRow>(conn)?
                .id;
            assert!(id > prior, "{table} reused {id}, at or below {prior}");
        }
        Ok(())
    }

    fn assert_snapshot_parent_is_catalog_scoped(conn: &mut SqliteConnection) -> QueryResult<()> {
        conn.batch_execute(
            "INSERT INTO catalog_snapshots \
                 (snapshot_key, catalog_key, document_key, interpretation_key, \
                  parent_snapshot_key, scope_kind) \
             VALUES ('snapshot-a4', 'catalog-a', 'document-b', 'logiqx-v2', \
                     'snapshot-a1', 'unknown')",
        )?;
        assert_eq!(
            sql_query(
                "SELECT COUNT(*) AS count FROM catalog_snapshots \
                 WHERE snapshot_key = 'snapshot-a4' \
                   AND parent_snapshot_key = 'snapshot-a1'",
            )
            .get_result::<CountRow>(conn)?
            .count,
            1
        );
        assert!(sql_fails(
            conn,
            "INSERT INTO catalog_snapshots \
                 (snapshot_key, catalog_key, document_key, interpretation_key, \
                  parent_snapshot_key, scope_kind) \
             VALUES ('snapshot-cross-catalog-parent', 'catalog-b', 'document-b', \
                     'logiqx-v1', 'snapshot-a1', 'unknown')",
        ));
        Ok(())
    }

    fn assert_snapshot_publication_identity_is_unique(
        conn: &mut SqliteConnection,
    ) -> QueryResult<()> {
        sql_query(
            "INSERT INTO catalog_snapshots \
             (snapshot_key, catalog_key, document_key, interpretation_key, scope_kind) \
             VALUES ('snapshot-a2', 'catalog-a', 'document-a', 'logiqx-v1', 'unknown')",
        )
        .execute(conn)?;
        sql_query(
            "INSERT INTO snapshot_publications \
             (catalog_key, document_key, interpretation_key, snapshot_key) \
             VALUES ('catalog-a', 'document-a', 'logiqx-v1', 'snapshot-a1')",
        )
        .execute(conn)?;
        assert!(sql_fails(
            conn,
            "INSERT INTO snapshot_publications \
                 (catalog_key, document_key, interpretation_key, snapshot_key) \
             VALUES ('catalog-a', 'document-a', 'logiqx-v1', 'snapshot-a2')",
        ));
        assert!(sql_fails(
            conn,
            "INSERT OR REPLACE INTO snapshot_publications \
                 (catalog_key, document_key, interpretation_key, snapshot_key) \
             VALUES ('catalog-a', 'document-a', 'logiqx-v1', 'snapshot-a2')",
        ));
        Ok(())
    }

    #[test]
    fn additive_migration_preserves_a_populated_legacy_cache()
    -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        conn.batch_execute("PRAGMA foreign_keys = ON")?;
        let migrations = MIGRATIONS.migrations()?;
        assert!(!migrations.is_empty());
        let identity_migration_index = migrations
            .iter()
            .position(|migration| migration.name().to_string() == IDENTITY_MIGRATION)
            .ok_or("identity migration not found")?;
        conn.applied_migrations()?;
        for migration in &migrations[..identity_migration_index] {
            conn.run_migration(migration.as_ref())?;
        }
        conn.batch_execute(
            "INSERT INTO data_files (id, name, version) VALUES (17, 'Legacy DAT', 'v1');
             INSERT INTO games (id, name, data_file_id) VALUES (23, 'legacy-set', 17);
             INSERT INTO roms (id, name, size, md5, sha1, crc, game_id)
                 VALUES (31, 'legacy.rom', 3, X'01', X'02', X'03', 23);
             INSERT INTO rom_files
                 (id, parent_path, path, name, sha1, xxhash3, in_archive, rom_id)
                 VALUES (41, '/roms', '/roms/legacy.rom', 'legacy.rom', X'02', X'04', 0, 31);",
        )?;
        run_startup_migrations(&mut conn)?;
        assert_eq!(count(&mut conn, "data_files")?, 1);
        assert_eq!(count(&mut conn, "games")?, 1);
        assert_eq!(count(&mut conn, "roms")?, 1);
        assert_eq!(count(&mut conn, "rom_files")?, 1);
        assert_eq!(
            sql_query("SELECT id FROM data_files WHERE id = 17")
                .get_result::<IdRow>(&mut conn)?
                .id,
            17
        );
        assert_eq!(
            sql_query("SELECT id FROM games WHERE id = 23 AND data_file_id = 17")
                .get_result::<IdRow>(&mut conn)?
                .id,
            23
        );
        assert_eq!(
            sql_query("SELECT id FROM roms WHERE id = 31 AND game_id = 23")
                .get_result::<IdRow>(&mut conn)?
                .id,
            31
        );
        assert_eq!(
            sql_query("SELECT id FROM rom_files WHERE id = 41 AND rom_id = 31")
                .get_result::<IdRow>(&mut conn)?
                .id,
            41
        );
        assert_eq!(count(&mut conn, "publishing_sources")?, 0);
        assert!(conn.run_pending_migrations(MIGRATIONS)?.is_empty());
        Ok(())
    }

    #[test]
    fn scoped_names_migration_preserves_populated_legacy_rows_with_foreign_keys_enabled()
    -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        conn.batch_execute("PRAGMA foreign_keys = ON")?;
        let migrations = MIGRATIONS.migrations()?;
        let scoped_names_index = migrations
            .iter()
            .position(|migration| {
                migration
                    .name()
                    .to_string()
                    .starts_with("2026-04-22-153500_scope_game_and_rom_names")
            })
            .ok_or("scoped names migration not found")?;

        conn.applied_migrations()?;
        for migration in &migrations[..scoped_names_index] {
            conn.run_migration(migration.as_ref())?;
        }
        conn.batch_execute(
            "INSERT INTO data_files (id, name, version) VALUES (17, 'Legacy DAT', 'v1');
             INSERT INTO games (id, name, data_file_id) VALUES (23, 'legacy-set', 17);
             INSERT INTO roms (id, name, size, md5, sha1, crc, game_id)
                 VALUES (31, 'legacy.rom', 3, X'01', X'02', X'03', 23);
             INSERT INTO archive_files (id, path, sha1) VALUES (37, '/legacy.zip', X'04');
             UPDATE roms SET archive_file_id = 37 WHERE id = 31;
             INSERT INTO rom_files
                 (id, parent_path, path, name, sha1, xxhash3, in_archive, rom_id)
                 VALUES (41, '/roms', '/roms/legacy.rom', 'legacy.rom', X'02', X'04', 0, 31);",
        )?;

        conn.run_migration(migrations[scoped_names_index].as_ref())?;
        assert_eq!(count(&mut conn, "data_files")?, 1);
        assert_eq!(count(&mut conn, "games")?, 1);
        assert_eq!(count(&mut conn, "roms")?, 1);
        assert_eq!(count(&mut conn, "archive_files")?, 1);
        assert_eq!(count(&mut conn, "rom_files")?, 1);
        assert_eq!(
            sql_query("SELECT id FROM data_files WHERE id = 17")
                .get_result::<IdRow>(&mut conn)?
                .id,
            17
        );
        assert_eq!(
            sql_query("SELECT id FROM games WHERE id = 23 AND data_file_id = 17")
                .get_result::<IdRow>(&mut conn)?
                .id,
            23
        );
        assert_eq!(
            sql_query(
                "SELECT id FROM roms WHERE id = 31 AND game_id = 23 AND archive_file_id = 37"
            )
            .get_result::<IdRow>(&mut conn)?
            .id,
            31
        );
        assert_eq!(
            sql_query("SELECT id FROM archive_files WHERE id = 37")
                .get_result::<IdRow>(&mut conn)?
                .id,
            37
        );
        assert_eq!(
            sql_query("SELECT id FROM rom_files WHERE id = 41 AND rom_id = 31")
                .get_result::<IdRow>(&mut conn)?
                .id,
            41
        );
        let foreign_keys_enabled =
            sql_query("SELECT foreign_keys AS count FROM pragma_foreign_keys")
                .get_result::<CountRow>(&mut conn)?
                .count;
        assert_eq!(foreign_keys_enabled, 1);
        assert_eq!(count(&mut conn, "pragma_foreign_key_check")?, 0);
        Ok(())
    }

    #[test]
    fn scoped_names_migration_preserves_autoincrement_high_water_marks()
    -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        let migrations = MIGRATIONS.migrations()?;
        let scoped_names_index = migrations
            .iter()
            .position(|migration| {
                migration
                    .name()
                    .to_string()
                    .starts_with(SCOPED_NAMES_MIGRATION)
            })
            .ok_or("scoped names migration not found")?;
        conn.applied_migrations()?;
        for migration in &migrations[..scoped_names_index] {
            conn.run_migration(migration.as_ref())?;
        }
        seed_then_delete_high_rebuilt_ids(&mut conn)?;

        conn.run_migration(migrations[scoped_names_index].as_ref())?;
        assert_rebuilt_ids_advance_past(&mut conn)?;
        Ok(())
    }

    #[test]
    fn failed_startup_migration_reenables_foreign_keys()
    -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        conn.batch_execute("PRAGMA foreign_keys = ON")?;
        let migrations = MIGRATIONS.migrations()?;
        let scoped_names_index = migrations
            .iter()
            .position(|migration| {
                migration
                    .name()
                    .to_string()
                    .starts_with(SCOPED_NAMES_MIGRATION)
            })
            .ok_or("scoped names migration not found")?;
        conn.applied_migrations()?;
        for migration in &migrations[..scoped_names_index] {
            conn.run_migration(migration.as_ref())?;
        }
        conn.batch_execute("CREATE TABLE games_scoped (id INTEGER PRIMARY KEY)")?;
        let applied_before = conn.applied_migrations()?;

        assert!(run_startup_migrations(&mut conn).is_err());
        assert_eq!(conn.applied_migrations()?, applied_before);
        assert_eq!(
            sql_query(
                "SELECT COUNT(*) AS count FROM sqlite_master \
                 WHERE type = 'table' AND name = 'games_scoped'",
            )
            .get_result::<CountRow>(&mut conn)?
            .count,
            1
        );
        assert_eq!(
            sql_query("SELECT foreign_keys AS count FROM pragma_foreign_keys")
                .get_result::<CountRow>(&mut conn)?
                .count,
            1
        );
        assert_eq!(count(&mut conn, "pragma_foreign_key_check")?, 0);
        Ok(())
    }

    #[test]
    fn startup_migration_rejects_legacy_orphans_without_changing_the_cache()
    -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        let migrations = MIGRATIONS.migrations()?;
        let scoped_names_index = migrations
            .iter()
            .position(|migration| {
                migration
                    .name()
                    .to_string()
                    .starts_with(SCOPED_NAMES_MIGRATION)
            })
            .ok_or("scoped names migration not found")?;
        conn.applied_migrations()?;
        for migration in &migrations[..scoped_names_index] {
            conn.run_migration(migration.as_ref())?;
        }

        conn.batch_execute(
            "PRAGMA foreign_keys = OFF;
             INSERT INTO data_files (id, name, version) VALUES (17, 'Legacy DAT', 'v1');
             INSERT INTO games (id, name, data_file_id) VALUES (23, 'legacy-set', 17);
             INSERT INTO roms (id, name, size, md5, sha1, crc, game_id)
                 VALUES (31, 'orphan.rom', 3, X'01', X'02', X'03', 999);",
        )?;
        let applied_before = conn.applied_migrations()?;
        assert_eq!(count(&mut conn, "pragma_foreign_key_check")?, 1);

        conn.batch_execute("PRAGMA foreign_keys = ON")?;
        let migration_result = run_startup_migrations(&mut conn);
        assert!(
            migration_result.is_err(),
            "startup accepted a legacy foreign key violation"
        );

        assert_eq!(
            conn.applied_migrations()?,
            applied_before,
            "startup applied a migration despite rejecting a legacy foreign key violation"
        );
        assert_eq!(
            sql_query(
                "SELECT COUNT(*) AS count FROM sqlite_master \
                 WHERE type = 'table' AND name = 'games_scoped'",
            )
            .get_result::<CountRow>(&mut conn)?
            .count,
            0
        );
        assert_eq!(count(&mut conn, "games")?, 1);
        assert_eq!(count(&mut conn, "roms")?, 1);
        assert_eq!(count(&mut conn, "pragma_foreign_key_check")?, 1);
        assert_eq!(
            sql_query(
                "SELECT COUNT(*) AS count FROM pragma_foreign_key_check \
                 WHERE \"table\" = 'roms' AND parent = 'games'",
            )
            .get_result::<CountRow>(&mut conn)?
            .count,
            1
        );
        assert_eq!(
            sql_query("SELECT foreign_keys AS count FROM pragma_foreign_keys")
                .get_result::<CountRow>(&mut conn)?
                .count,
            1
        );
        Ok(())
    }

    #[test]
    fn failed_scoped_names_rollback_is_atomic_and_reenables_foreign_keys()
    -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        conn.batch_execute("PRAGMA foreign_keys = ON")?;
        run_startup_migrations(&mut conn)?;

        conn.batch_execute(
            "INSERT INTO data_files (id, name, version)
                 VALUES (17, 'First DAT', 'v1'), (18, 'Second DAT', 'v1');
             INSERT INTO games (id, name, data_file_id)
                 VALUES (23, 'same-game', 17), (24, 'same-game', 18);
             INSERT INTO archive_files (id, path, sha1) VALUES (37, '/legacy.zip', X'04');
             INSERT INTO roms (id, name, size, md5, sha1, crc, game_id)
                 VALUES (31, 'same.rom', 3, X'01', X'02', X'03', 23),
                        (32, 'same.rom', 3, X'04', X'05', X'06', 24);
             UPDATE roms SET archive_file_id = 37 WHERE id = 31;
             INSERT INTO rom_files
                 (id, parent_path, path, name, sha1, xxhash3, in_archive, rom_id)
                 VALUES (41, '/roms', '/roms/same.rom', 'same.rom', X'02', X'04', 0, 31);",
        )?;

        let migrations = MIGRATIONS.migrations()?;
        let scoped_names_migration = migrations
            .iter()
            .find(|migration| {
                migration
                    .name()
                    .to_string()
                    .starts_with("2026-04-22-153500_scope_game_and_rom_names")
            })
            .ok_or("scoped names migration not found")?;
        let applied_before = conn.applied_migrations()?;
        let revert_result = conn.revert_migration(scoped_names_migration.as_ref());
        let revert_error = match revert_result {
            Ok(_) => String::new(),
            Err(error) => error.to_string(),
        };
        assert!(
            revert_error.contains("UNIQUE constraint failed"),
            "rollback did not fail because of global-name uniqueness: {revert_error}"
        );

        assert_eq!(count(&mut conn, "data_files")?, 2);
        assert_eq!(count(&mut conn, "games")?, 2);
        assert_eq!(count(&mut conn, "roms")?, 2);
        assert_eq!(count(&mut conn, "archive_files")?, 1);
        assert_eq!(count(&mut conn, "rom_files")?, 1);
        assert_eq!(
            sql_query("SELECT COUNT(*) AS count FROM roms WHERE name = 'same.rom'")
                .get_result::<CountRow>(&mut conn)?
                .count,
            2
        );
        assert_eq!(conn.applied_migrations()?, applied_before);
        assert_eq!(
            sql_query(
                "SELECT COUNT(*) AS count FROM sqlite_master \
                 WHERE type = 'index' AND name IN \
                     ('roms_game_name_unique', 'games_data_file_name_unique')",
            )
            .get_result::<CountRow>(&mut conn)?
            .count,
            2
        );
        assert_eq!(
            sql_query("SELECT foreign_keys AS count FROM pragma_foreign_keys")
                .get_result::<CountRow>(&mut conn)?
                .count,
            1
        );
        assert_eq!(count(&mut conn, "pragma_foreign_key_check")?, 0);
        Ok(())
    }

    #[test]
    fn scoped_names_revert_preserves_populated_rows_with_foreign_keys_enabled()
    -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        conn.batch_execute("PRAGMA foreign_keys = ON")?;
        conn.run_pending_migrations(MIGRATIONS)?;
        conn.batch_execute(
            "INSERT INTO data_files (id, name, version) VALUES (17, 'Legacy DAT', 'v1');
             INSERT INTO games (id, name, data_file_id) VALUES (23, 'legacy-set', 17);
             INSERT INTO archive_files (id, path, sha1) VALUES (37, '/legacy.zip', X'04');
             INSERT INTO roms (id, name, size, md5, sha1, crc, game_id, archive_file_id)
                 VALUES (31, 'legacy.rom', 3, X'01', X'02', X'03', 23, 37);
             INSERT INTO rom_files
                 (id, parent_path, path, name, sha1, xxhash3, in_archive, rom_id)
                 VALUES (41, '/roms', '/roms/legacy.rom', 'legacy.rom', X'02', X'04', 0, 31);",
        )?;

        let migrations = MIGRATIONS.migrations()?;
        let scoped_names_migration = migrations
            .iter()
            .find(|migration| {
                migration
                    .name()
                    .to_string()
                    .starts_with(SCOPED_NAMES_MIGRATION)
            })
            .ok_or("scoped names migration not found")?;
        conn.revert_migration(scoped_names_migration.as_ref())?;

        assert_eq!(count(&mut conn, "data_files")?, 1);
        assert_eq!(count(&mut conn, "games")?, 1);
        assert_eq!(count(&mut conn, "roms")?, 1);
        assert_eq!(count(&mut conn, "archive_files")?, 1);
        assert_eq!(count(&mut conn, "rom_files")?, 1);
        assert_eq!(
            sql_query(
                "SELECT id FROM roms WHERE id = 31 AND game_id = 23 AND archive_file_id = 37"
            )
            .get_result::<IdRow>(&mut conn)?
            .id,
            31
        );
        assert_eq!(
            sql_query("SELECT id FROM rom_files WHERE id = 41 AND rom_id = 31")
                .get_result::<IdRow>(&mut conn)?
                .id,
            41
        );
        assert_eq!(
            sql_query("SELECT foreign_keys AS count FROM pragma_foreign_keys")
                .get_result::<CountRow>(&mut conn)?
                .count,
            1
        );
        assert_eq!(count(&mut conn, "pragma_foreign_key_check")?, 0);
        assert!(
            conn.applied_migrations()?
                .iter()
                .all(|name| name.to_string() != SCOPED_NAMES_MIGRATION)
        );
        Ok(())
    }

    #[test]
    fn scoped_names_revert_preserves_autoincrement_high_water_marks()
    -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        conn.run_pending_migrations(MIGRATIONS)?;
        seed_then_delete_high_rebuilt_ids(&mut conn)?;
        let scoped_names_migration = MIGRATIONS
            .migrations()?
            .into_iter()
            .find(|migration| {
                migration
                    .name()
                    .to_string()
                    .starts_with(SCOPED_NAMES_MIGRATION)
            })
            .ok_or("scoped names migration not found")?;

        conn.revert_migration(scoped_names_migration.as_ref())?;
        assert_rebuilt_ids_advance_past(&mut conn)?;
        Ok(())
    }

    #[test]
    fn machine_asset_migration_preserves_existing_rom_requirements()
    -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        conn.batch_execute("PRAGMA foreign_keys = ON")?;
        conn.run_pending_migrations(MIGRATIONS)?;
        let machine_asset_migration = MIGRATIONS
            .migrations()?
            .into_iter()
            .find(|migration| {
                migration.name().to_string() == "2026-09-24-000003_mame_machine_asset_semantics"
            })
            .ok_or("machine asset migration not found")?;
        conn.revert_migration(machine_asset_migration.as_ref())?;
        conn.batch_execute(
            "INSERT INTO publishing_sources (source_key, display_name)
                 VALUES ('source', 'Source');
             INSERT INTO catalogs (catalog_key, source_key, display_name)
                 VALUES ('catalog', 'source', 'Catalog');
             INSERT INTO documents (document_key) VALUES ('document');
             INSERT INTO parser_interpretations (interpretation_key, format)
                 VALUES ('interpretation', 'logiqx');
             INSERT INTO catalog_snapshots
                 (snapshot_key, catalog_key, document_key, interpretation_key, scope_kind)
                 VALUES ('snapshot', 'catalog', 'document', 'interpretation', 'unknown');
             INSERT INTO snapshot_sets
                 (snapshot_key, set_name, metadata_json, source_line, source_column)
                 VALUES ('snapshot', 'set', '{}', 1, 1);
             INSERT INTO asset_requirements
                 (snapshot_key, set_name, component_order, asset_name, role,
                  evidence_scope, evidence_provenance, source_line, source_column)
                 VALUES ('snapshot', 'set', 0, 'rom.bin', 'rom', 'whole_asset',
                         'source_declared', 2, 3);",
        )?;
        conn.run_pending_migrations(MIGRATIONS)?;
        let migrated = sql_query(
            "SELECT metadata_json AS value FROM asset_requirements WHERE asset_name = 'rom.bin'",
        )
        .get_result::<TextRow>(&mut conn)?;
        assert_eq!(migrated.value, "{}");
        assert_eq!(count(&mut conn, "asset_requirements")?, 1);
        assert!(
            conn.batch_execute(
                "INSERT INTO asset_requirements
                 (snapshot_key, set_name, component_order, asset_name, role,
                  evidence_scope, evidence_provenance, source_line, source_column)
             VALUES ('snapshot', 'set', 1, 'disk.chd', 'disk', 'disk_data',
                     'source_declared', 4, 5);",
            )
            .is_ok()
        );
        assert!(
            conn.batch_execute(
                "UPDATE asset_requirements SET asset_name = 'changed' WHERE component_order = 0;",
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn chd_scope_migration_relabels_existing_disk_evidence_without_touching_roms()
    -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        conn.batch_execute("PRAGMA foreign_keys = ON")?;
        conn.run_pending_migrations(MIGRATIONS)?;
        let chd_scope_migration = MIGRATIONS
            .migrations()?
            .into_iter()
            .find(|migration| migration.name().to_string() == "2026-09-26-000000_chd_digest_scope")
            .ok_or("CHD scope migration not found")?;
        conn.revert_migration(chd_scope_migration.as_ref())?;
        conn.batch_execute(
            "INSERT INTO publishing_sources (source_key, display_name)
                 VALUES ('source', 'Source');
             INSERT INTO catalogs (catalog_key, source_key, display_name)
                 VALUES ('catalog', 'source', 'Catalog');
             INSERT INTO documents (document_key) VALUES ('document');
             INSERT INTO parser_interpretations (interpretation_key, format)
                 VALUES ('interpretation', 'mame');
             INSERT INTO catalog_snapshots
                 (snapshot_key, catalog_key, document_key, interpretation_key, scope_kind)
                 VALUES ('snapshot', 'catalog', 'document', 'interpretation', 'unknown');
             INSERT INTO snapshot_sets
                 (snapshot_key, set_name, metadata_json, source_line, source_column)
                 VALUES ('snapshot', 'set', '{}', 1, 1);
             INSERT INTO asset_requirements
                 (snapshot_key, set_name, component_order, asset_name, role,
                  evidence_scope, evidence_provenance, source_line, source_column)
                 VALUES ('snapshot', 'set', 0, 'rom.bin', 'rom', 'whole_asset',
                         'source_declared', 2, 3),
                        ('snapshot', 'set', 1, 'disk.chd', 'disk', 'disk_data',
                         'source_declared', 4, 5);
             INSERT INTO software_lists
                 (snapshot_key, list_name, list_order, source_line, source_column)
                 VALUES ('snapshot', 'list', 0, 1, 1);
             INSERT INTO software_items
                 (snapshot_key, list_name, item_name, item_order, description, year,
                  publisher, info_json, shared_features_json, source_line, source_column)
                 VALUES ('snapshot', 'list', 'item', 0, 'Item', '2000', 'Publisher',
                         '[]', '[]', 1, 1);
             INSERT INTO software_parts
                 (snapshot_key, list_name, item_name, part_name, part_order,
                  interface, features_json, source_line, source_column)
                 VALUES ('snapshot', 'list', 'item', 'part', 0, 'disk', '[]', 1, 1);
             INSERT INTO software_areas
                 (snapshot_key, list_name, item_name, part_name, area_name,
                  area_kind, area_order, source_line, source_column)
                 VALUES ('snapshot', 'list', 'item', 'part', 'media', 'disk', 0, 1, 1);
             INSERT INTO software_components
                 (snapshot_key, list_name, item_name, part_name, area_order,
                  area_kind, area_name, component_order, component_kind,
                  component_name, source_line, source_column)
                 VALUES ('snapshot', 'list', 'item', 'part', 0, 'disk', 'media',
                         0, 'disk', 'software.chd', 2, 3);",
        )?;
        conn.run_pending_migrations(MIGRATIONS)?;
        let rom = sql_query(
            "SELECT evidence_scope AS value FROM asset_requirements WHERE asset_name = 'rom.bin'",
        )
        .get_result::<TextRow>(&mut conn)?;
        let disk = sql_query(
            "SELECT evidence_scope AS value FROM asset_requirements WHERE asset_name = 'disk.chd'",
        )
        .get_result::<TextRow>(&mut conn)?;
        let software_disk = sql_query(
            "SELECT evidence_scope AS value FROM software_components \
             WHERE component_name = 'software.chd'",
        )
        .get_result::<TextRow>(&mut conn)?;
        assert_eq!(rom.value, "whole_asset");
        assert_eq!(disk.value, "chd_header_sha1");
        assert_eq!(software_disk.value, "chd_header_sha1");
        assert!(sql_fails(
            &mut conn,
            "UPDATE asset_requirements SET evidence_scope = 'unknown' WHERE role = 'disk'"
        ));
        conn.revert_migration(chd_scope_migration.as_ref())?;
        let rolled_back_disk = sql_query(
            "SELECT evidence_scope AS value FROM asset_requirements WHERE asset_name = 'disk.chd'",
        )
        .get_result::<TextRow>(&mut conn)?;
        assert_eq!(rolled_back_disk.value, "disk_data");
        Ok(())
    }

    #[test]
    fn identity_keys_separate_names_versions_interpretations_and_runs()
    -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        conn.batch_execute("PRAGMA foreign_keys = ON")?;
        conn.run_pending_migrations(MIGRATIONS)?;
        conn.batch_execute(
            "INSERT INTO publishing_sources (source_key, display_name)
                 VALUES ('source-a', 'Same Publisher'), ('source-b', 'Same Publisher');
             INSERT INTO catalogs (catalog_key, source_key, display_name)
                 VALUES ('catalog-a', 'source-a', 'Same Catalog'),
                        ('catalog-b', 'source-b', 'Same Catalog');
             INSERT INTO documents (document_key) VALUES ('document-a');
             INSERT INTO documents (document_key) VALUES ('document-b');
             INSERT INTO acquisitions (acquisition_key, source_key, document_key)
                 VALUES ('acquisition-a', 'source-a', 'document-a'),
                        ('acquisition-b', 'source-b', 'document-b');
             INSERT INTO parser_interpretations (interpretation_key, format)
                 VALUES ('logiqx-v1', 'logiqx'), ('logiqx-v2', 'logiqx'),
                        ('logiqx-v3', 'logiqx');
             INSERT INTO catalog_snapshots
                 (snapshot_key, catalog_key, document_key, interpretation_key,
                  declared_version, scope_kind)
                 VALUES ('snapshot-a1', 'catalog-a', 'document-a', 'logiqx-v1', '1.0', 'unknown'),
                        ('snapshot-a3', 'catalog-a', 'document-a', 'logiqx-v2', NULL, 'unknown');
             INSERT INTO import_runs
                 (run_key, catalog_key, document_key, interpretation_key, snapshot_key, status)
                 VALUES ('run-a1', 'catalog-a', 'document-a', 'logiqx-v1', 'snapshot-a1', 'succeeded'),
                        ('run-a3', 'catalog-a', 'document-a', 'logiqx-v2', NULL, 'succeeded');",
        )?;

        assert_eq!(count(&mut conn, "publishing_sources")?, 2);
        assert_eq!(count(&mut conn, "catalogs")?, 2);
        assert_eq!(count(&mut conn, "catalog_snapshots")?, 2);
        assert_eq!(count(&mut conn, "import_runs")?, 2);
        assert_eq!(
            sql_query(
                "SELECT COUNT(*) AS count FROM import_runs \
                 WHERE interpretation_key = 'logiqx-v1'",
            )
            .get_result::<CountRow>(&mut conn)?
            .count,
            1
        );
        assert_snapshot_parent_is_catalog_scoped(&mut conn)?;
        assert_snapshot_publication_identity_is_unique(&mut conn)?;
        assert!(sql_fails(
            &mut conn,
            "INSERT INTO catalogs (catalog_key, source_key, display_name) \
             VALUES ('catalog-a', 'source-b', 'Conflicting key')",
        ));
        assert!(sql_fails(
            &mut conn,
            "INSERT OR REPLACE INTO catalog_snapshots \
                 (snapshot_key, catalog_key, document_key, interpretation_key, \
                  declared_version, scope_kind) \
             VALUES ('snapshot-a1', 'catalog-a', 'document-a', 'logiqx-v1', '2.0', 'unknown')",
        ));
        assert!(sql_fails(
            &mut conn,
            "UPDATE catalog_snapshots SET declared_version = '2.0' \
             WHERE snapshot_key = 'snapshot-a3'",
        ));
        assert!(sql_fails(
            &mut conn,
            "UPDATE parser_interpretations SET parser_version = 'changed' \
             WHERE interpretation_key = 'logiqx-v1'",
        ));
        conn.batch_execute(
            "UPDATE parser_interpretations SET parser_version = 'initialized' \
             WHERE interpretation_key = 'logiqx-v3'",
        )?;
        assert!(sql_fails(
            &mut conn,
            "DELETE FROM catalog_snapshots WHERE snapshot_key = 'snapshot-a3'",
        ));
        assert!(sql_fails(
            &mut conn,
            "INSERT INTO import_runs \
                 (run_key, catalog_key, document_key, interpretation_key, snapshot_key, status) \
                 VALUES ('run-mismatched-lineage', 'catalog-b', 'document-a', \
                         'logiqx-v1', 'snapshot-a1', 'succeeded')",
        ));
        assert!(sql_fails(
            &mut conn,
            "INSERT INTO catalog_snapshots \
                 (snapshot_key, catalog_key, document_key, interpretation_key, \
                  acquisition_key, scope_kind) \
                 VALUES ('snapshot-mismatched-acquisition', 'catalog-a', 'document-a', \
                         'logiqx-v1', 'acquisition-b', 'unknown')",
        ));
        assert!(sql_fails(
            &mut conn,
            "INSERT INTO import_runs \
                 (run_key, catalog_key, document_key, interpretation_key, \
                  acquisition_key, status) \
                 VALUES ('run-mismatched-acquisition', 'catalog-a', 'document-a', \
                         'logiqx-v1', 'acquisition-b', 'succeeded')",
        ));
        Ok(())
    }

    #[test]
    fn snapshot_publication_migration_preserves_duplicate_legacy_identities()
    -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        conn.batch_execute("PRAGMA foreign_keys = ON")?;
        let migrations = MIGRATIONS.migrations()?;
        let snapshot_index = migrations
            .iter()
            .position(|migration| {
                migration.name().to_string() == "2026-09-24-000002_publish_logiqx_snapshots"
            })
            .ok_or("snapshot migration not found")?;
        conn.applied_migrations()?;
        for migration in &migrations[..snapshot_index] {
            conn.run_migration(migration.as_ref())?;
        }
        conn.batch_execute(
            "INSERT INTO publishing_sources (source_key, display_name) \
                 VALUES ('legacy-source', 'Legacy source'); \
             INSERT INTO catalogs (catalog_key, source_key, display_name) \
                 VALUES ('legacy-catalog', 'legacy-source', 'Legacy catalog'); \
             INSERT INTO documents (document_key) VALUES ('legacy-document'); \
             INSERT INTO parser_interpretations (interpretation_key, format) \
                 VALUES ('legacy-logiqx', 'logiqx'); \
             INSERT INTO catalog_snapshots \
                 (snapshot_key, catalog_key, document_key, interpretation_key) \
                 VALUES ('legacy-snapshot-a', 'legacy-catalog', 'legacy-document', 'legacy-logiqx'), \
                        ('legacy-snapshot-b', 'legacy-catalog', 'legacy-document', 'legacy-logiqx');",
        )?;

        conn.run_pending_migrations(MIGRATIONS)?;
        assert_eq!(count(&mut conn, "catalog_snapshots")?, 2);
        assert_eq!(count(&mut conn, "snapshot_publications")?, 0);
        sql_query(
            "INSERT INTO snapshot_publications \
             (catalog_key, document_key, interpretation_key, snapshot_key) \
             VALUES ('legacy-catalog', 'legacy-document', 'legacy-logiqx', 'legacy-snapshot-a')",
        )
        .execute(&mut conn)?;
        assert!(sql_fails(
            &mut conn,
            "INSERT INTO snapshot_publications \
                 (catalog_key, document_key, interpretation_key, snapshot_key) \
             VALUES ('legacy-catalog', 'legacy-document', 'legacy-logiqx', 'legacy-snapshot-b')",
        ));
        Ok(())
    }

    #[test]
    fn identity_migration_reverts_with_parent_and_child_snapshots()
    -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        conn.batch_execute("PRAGMA foreign_keys = ON")?;
        let migrations = MIGRATIONS.migrations()?;
        assert!(!migrations.is_empty());
        let identity_migration_index = migrations
            .iter()
            .position(|migration| migration.name().to_string() == IDENTITY_MIGRATION)
            .ok_or("identity migration not found")?;
        conn.applied_migrations()?;
        for migration in &migrations[..identity_migration_index] {
            conn.run_migration(migration.as_ref())?;
        }
        let identity_migration = &migrations[identity_migration_index];
        conn.run_migration(identity_migration.as_ref())?;
        conn.batch_execute(
            "INSERT INTO publishing_sources (source_key, display_name)
                 VALUES ('source-a', 'Publisher');
             INSERT INTO catalogs (catalog_key, source_key, display_name)
                 VALUES ('catalog-a', 'source-a', 'Catalog');
             INSERT INTO documents (document_key) VALUES ('document-a');
             INSERT INTO parser_interpretations (interpretation_key, format)
                 VALUES ('logiqx-v1', 'logiqx');
             INSERT INTO catalog_snapshots
                 (snapshot_key, catalog_key, document_key, interpretation_key)
                 VALUES ('snapshot-parent', 'catalog-a', 'document-a', 'logiqx-v1');
             INSERT INTO catalog_snapshots
                 (snapshot_key, catalog_key, document_key, interpretation_key, parent_snapshot_key)
                 VALUES ('snapshot-child', 'catalog-a', 'document-a', 'logiqx-v1', 'snapshot-parent');",
        )?;

        conn.revert_migration(identity_migration.as_ref())?;
        assert_eq!(
            sql_query(
                "SELECT COUNT(*) AS count FROM sqlite_master \
                 WHERE type = 'table' AND name = 'catalog_snapshots'",
            )
            .get_result::<CountRow>(&mut conn)?
            .count,
            0
        );
        assert_eq!(
            sql_query("SELECT foreign_keys AS count FROM pragma_foreign_keys")
                .get_result::<CountRow>(&mut conn)?
                .count,
            1
        );
        Ok(())
    }
}
