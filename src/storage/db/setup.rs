use diesel::{SqliteConnection, connection::SimpleConnection, r2d2::ConnectionManager};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

use super::Pool;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

#[derive(Debug)]
struct EnableForeignKeys;

impl diesel::r2d2::CustomizeConnection<SqliteConnection, diesel::r2d2::Error>
    for EnableForeignKeys
{
    fn on_acquire(&self, conn: &mut SqliteConnection) -> Result<(), diesel::r2d2::Error> {
        conn.batch_execute("PRAGMA foreign_keys = ON")
            .map_err(diesel::r2d2::Error::QueryError)
    }
}

pub fn create_db_pool(database_url: &str) -> crate::Result<Pool> {
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    let pool = Pool::builder()
        .connection_customizer(Box::new(EnableForeignKeys))
        .build(manager)?;
    {
        let mut conn = pool.get()?;
        conn.run_pending_migrations(MIGRATIONS)
            .map_err(|e| crate::Error::Migration(e.to_string()))?;
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

    fn count(conn: &mut SqliteConnection, table: &str) -> QueryResult<i64> {
        sql_query(format!("SELECT COUNT(*) AS count FROM {table}"))
            .get_result::<CountRow>(conn)
            .map(|row| row.count)
    }

    fn sql_fails(conn: &mut SqliteConnection, statement: &str) -> bool {
        sql_query(statement).execute(conn).is_err()
    }

    #[test]
    fn additive_migration_preserves_a_populated_legacy_cache()
    -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        conn.batch_execute("PRAGMA foreign_keys = ON")?;
        let migrations = MIGRATIONS.migrations()?;
        assert!(!migrations.is_empty());
        conn.applied_migrations()?;
        for migration in &migrations[..migrations.len() - 1] {
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

        conn.run_pending_migrations(MIGRATIONS)?;
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
                 VALUES ('logiqx-v1', 'logiqx'), ('logiqx-v2', 'logiqx');
             INSERT INTO catalog_snapshots
                 (snapshot_key, catalog_key, document_key, interpretation_key,
                  declared_version, scope_kind)
                 VALUES ('snapshot-a1', 'catalog-a', 'document-a', 'logiqx-v1', '1.0', 'unknown'),
                        ('snapshot-a2', 'catalog-a', 'document-a', 'logiqx-v1', '1.0', 'unknown'),
                        ('snapshot-a3', 'catalog-a', 'document-a', 'logiqx-v2', NULL, 'unknown');
             INSERT INTO import_runs
                 (run_key, catalog_key, document_key, interpretation_key, snapshot_key, status)
                 VALUES ('run-a1', 'catalog-a', 'document-a', 'logiqx-v1', 'snapshot-a1', 'succeeded'),
                        ('run-a2', 'catalog-a', 'document-a', 'logiqx-v1', 'snapshot-a2', 'succeeded'),
                        ('run-a3', 'catalog-a', 'document-a', 'logiqx-v2', NULL, 'succeeded');",
        )?;

        assert_eq!(count(&mut conn, "publishing_sources")?, 2);
        assert_eq!(count(&mut conn, "catalogs")?, 2);
        assert_eq!(count(&mut conn, "catalog_snapshots")?, 3);
        assert_eq!(count(&mut conn, "import_runs")?, 3);
        assert_eq!(
            sql_query(
                "SELECT COUNT(*) AS count FROM import_runs \
                 WHERE interpretation_key = 'logiqx-v1'",
            )
            .get_result::<CountRow>(&mut conn)?
            .count,
            2
        );
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
}
