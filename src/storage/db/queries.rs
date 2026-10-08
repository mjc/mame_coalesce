use crate::storage::{db::Pool as DbPool, models::NewRomFile};

use camino::{Utf8Path, Utf8PathBuf};
use diesel::result::Error as DieselError;
use diesel::sql_types::BigInt;
use diesel::{QueryResult, QueryableByName, SqliteConnection};
use diesel::{prelude::*, sql_query};

#[derive(QueryableByName)]
struct DatabasePathRow {
    #[diesel(sql_type = diesel::sql_types::Text)]
    file: String,
}

pub fn replace_rom_files_for_source_root(
    pool: &DbPool,
    source_root: &Utf8Path,
    new_rom_files: &[NewRomFile],
) -> crate::Result<usize> {
    replace_rom_files_for_source_roots(
        pool,
        &[(source_root.as_str().to_owned(), new_rom_files.to_vec())],
    )
    .map(|counts| counts.into_iter().sum())
}

pub fn replace_rom_files_for_source_roots(
    pool: &DbPool,
    roots_and_files: &[(String, Vec<crate::storage::models::NewRomFile>)],
) -> crate::Result<Vec<usize>> {
    use crate::storage::schema::rom_files::dsl::rom_files;
    use diesel::replace_into;

    let mut conn = pool.get()?;

    Ok(conn.transaction::<_, DieselError, _>(|conn| {
        for (source_root, new_rom_files) in roots_and_files {
            delete_rom_files_for_source_root(conn, source_root)?;
            new_rom_files
                .iter()
                .map(|new_rom_file| replace_into(rom_files).values(new_rom_file).execute(conn))
                .collect::<QueryResult<Vec<usize>>>()?;
        }
        roots_and_files
            .iter()
            .map(|(source_root, _)| matching_rom_file_count(conn, source_root))
            .collect()
    })?)
}

pub fn database_file_paths(pool: &DbPool) -> crate::Result<Vec<Utf8PathBuf>> {
    let mut conn = pool.get()?;
    let rows = sql_query("SELECT file FROM pragma_database_list WHERE file != ''")
        .load::<DatabasePathRow>(&mut conn)?;

    let paths = rows
        .into_iter()
        .map(|row| Utf8PathBuf::from(row.file).canonicalize_utf8())
        .collect::<std::io::Result<Vec<_>>>()?;

    Ok(paths
        .into_iter()
        .flat_map(|path| {
            let base = path.as_str().to_owned();
            std::iter::once(path).chain(
                ["-wal", "-shm", "-journal", ".lock"]
                    .into_iter()
                    .map(move |suffix| Utf8PathBuf::from(format!("{base}{suffix}"))),
            )
        })
        .collect())
}

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

/// Counts each observation once when a catalog's latest published edition has
/// a qualified SHA-1. Latest editions use `(published_at, edition_id)` order;
/// the digest is a candidate signal, not proof of a match.
fn matching_rom_file_count(conn: &mut SqliteConnection, source_root: &str) -> QueryResult<usize> {
    let row = sql_query(
        r"
        SELECT COUNT(*) AS count
        FROM rom_files AS observed
        WHERE observed.scan_root = ?
          AND EXISTS (
              SELECT 1
              FROM candidate_qualified_file_hashes AS candidate
              JOIN catalog_media_entries AS media
                ON media.media_entry_id = candidate.media_entry_id
              JOIN catalog_source_elements AS owner
                ON owner.source_element_id = media.media_entry_id
              JOIN published_catalog_editions AS publication
                ON publication.edition_id = owner.edition_id
              JOIN shared_catalog_files AS shared_file
                ON shared_file.file_uuid = media.file_uuid
              JOIN hash_values AS digest
                ON digest.hash_id = candidate.hash_id
              WHERE digest.algorithm = 'sha1'
                AND digest.bytes = observed.sha1
                AND candidate.source_hash_field = 'sha1'
                AND NOT EXISTS (
                    SELECT 1
                    FROM published_catalog_editions AS newer
                    WHERE newer.catalog_id = publication.catalog_id
                      AND (newer.published_at > publication.published_at
                        OR (newer.published_at = publication.published_at
                          AND newer.edition_id > publication.edition_id))
                )
          )
        ",
    )
    .bind::<diesel::sql_types::Text, _>(source_root)
    .get_result::<CountRow>(conn)?;

    usize::try_from(row.count)
        .map_err(|error| diesel::result::Error::DeserializationError(Box::new(error)))
}

fn delete_rom_files_for_source_root(
    conn: &mut SqliteConnection,
    source_root: &str,
) -> QueryResult<usize> {
    sql_query(
        r"
        DELETE FROM rom_files
        WHERE scan_root = ?
            OR (
                scan_root IS NULL
                AND (path = ?
            OR (
                substr(path, 1, length(?)) = ?
                AND (substr(path, length(?) + 1, 1) = '/' OR ? = '/')
            )))
        ",
    )
    .bind::<diesel::sql_types::Text, _>(source_root)
    .bind::<diesel::sql_types::Text, _>(source_root)
    .bind::<diesel::sql_types::Text, _>(source_root)
    .bind::<diesel::sql_types::Text, _>(source_root)
    .bind::<diesel::sql_types::Text, _>(source_root)
    .bind::<diesel::sql_types::Text, _>(source_root)
    .execute(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::{Connection, connection::SimpleConnection};

    #[test]
    fn database_file_paths_include_sqlite_and_application_lock_paths()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let database_path = temp_dir.path().join("coalesce.db");
        let database_url = database_path
            .to_str()
            .ok_or("temporary database path is not UTF-8")?;
        let pool = crate::storage::db::create_db_pool(database_url)?;
        let canonical = Utf8PathBuf::from(database_url).canonicalize_utf8()?;

        let paths = database_file_paths(&pool)?;

        assert!(paths.contains(&canonical));
        assert!(paths.contains(&Utf8PathBuf::from(format!("{}-wal", canonical.as_str()))));
        assert!(paths.contains(&Utf8PathBuf::from(format!("{}-shm", canonical.as_str()))));
        assert!(paths.contains(&Utf8PathBuf::from(format!(
            "{}-journal",
            canonical.as_str()
        ))));
        assert!(paths.contains(&Utf8PathBuf::from(format!("{}.lock", canonical.as_str()))));
        Ok(())
    }

    #[test]
    fn matching_scan_counts_each_observation_once_from_latest_published_qualified_sha1s()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = SqliteConnection::establish(":memory:")?;
        conn.batch_execute(include_str!("inventory.sql"))?;
        conn.batch_execute(
            r"
            CREATE TABLE catalog_editions(
                edition_id INTEGER PRIMARY KEY,
                catalog_id INTEGER NOT NULL
            );
            CREATE TABLE published_catalog_editions(
                edition_id INTEGER PRIMARY KEY,
                catalog_id INTEGER NOT NULL,
                published_at TEXT NOT NULL
            );
            CREATE TABLE catalog_source_elements(
                source_element_id INTEGER PRIMARY KEY,
                edition_id INTEGER NOT NULL REFERENCES catalog_editions(edition_id)
            );
            CREATE TABLE shared_catalog_files(file_uuid BLOB PRIMARY KEY);
            CREATE TABLE catalog_media_entries(
                media_entry_id INTEGER PRIMARY KEY REFERENCES catalog_source_elements(source_element_id),
                file_uuid BLOB REFERENCES shared_catalog_files(file_uuid)
            );
            CREATE TABLE hash_values(
                hash_id INTEGER PRIMARY KEY,
                algorithm TEXT NOT NULL,
                bytes BLOB NOT NULL
            );
            CREATE TABLE catalog_entry_hashes(
                reported_hash_id INTEGER PRIMARY KEY,
                media_entry_id INTEGER NOT NULL REFERENCES catalog_media_entries(media_entry_id),
                source_hash_field TEXT NOT NULL,
                field_occurrence INTEGER NOT NULL,
                presence TEXT NOT NULL,
                hash_scope TEXT NOT NULL,
                hash_id INTEGER NOT NULL REFERENCES hash_values(hash_id)
            );
            CREATE TABLE candidate_native_file_byte_coverage(media_entry_id INTEGER PRIMARY KEY);
            CREATE TABLE candidate_canonical_hash_positions(
                reported_hash_id INTEGER PRIMARY KEY,
                media_entry_id INTEGER NOT NULL,
                source_hash_field TEXT NOT NULL,
                field_occurrence INTEGER NOT NULL
            );
            CREATE VIEW candidate_qualified_file_hashes AS
            SELECT declaration.reported_hash_id, declaration.media_entry_id,
                   declaration.hash_id, declaration.hash_scope,
                   declaration.source_hash_field
            FROM catalog_entry_hashes AS declaration
            JOIN hash_values AS value ON value.hash_id=declaration.hash_id
            WHERE declaration.presence='value'
              AND declaration.source_hash_field='sha1'
              AND declaration.hash_scope IN ('whole_file','whole_asset')
              AND value.algorithm='sha1' AND length(value.bytes)=20
              AND EXISTS (
                  SELECT 1 FROM candidate_native_file_byte_coverage AS coverage
                  WHERE coverage.media_entry_id=declaration.media_entry_id
              )
              AND EXISTS (
                  SELECT 1 FROM candidate_canonical_hash_positions AS position
                  WHERE position.reported_hash_id=declaration.reported_hash_id
                    AND position.media_entry_id=declaration.media_entry_id
                    AND position.source_hash_field=declaration.source_hash_field
                    AND position.field_occurrence=declaration.field_occurrence
              );

            INSERT INTO catalog_editions VALUES
                (1,1), (2,1), (3,2), (4,3), (5,4), (6,5), (7,6),
                (8,7), (9,7);
            INSERT INTO published_catalog_editions VALUES
                (1,1,'2026-01-01T00:00:00Z'),
                (2,1,'2026-01-01T00:00:00Z'),
                (3,2,'2026-01-15T00:00:00Z'),
                (4,3,'2026-01-15T00:00:00Z'),
                (5,4,'2026-01-15T00:00:00Z'),
                (7,6,'2026-01-15T00:00:00Z'),
                (8,7,'2026-02-01T00:00:00Z'),
                (9,7,'2026-01-15T00:00:00Z');
            INSERT INTO catalog_source_elements VALUES
                (101,1), (102,2), (103,3), (104,3),
                (105,4), (106,5), (107,6), (108,7),
                (109,8), (110,9);
            INSERT INTO shared_catalog_files VALUES
                (X'99999999999999999999999999999999'),
                (X'11111111111111111111111111111111'),
                (X'22222222222222222222222222222222'),
                (X'33333333333333333333333333333333'),
                (X'44444444444444444444444444444444'),
                (X'55555555555555555555555555555555'),
                (X'66666666666666666666666666666666'),
                (X'77777777777777777777777777777777'),
                (X'88888888888888888888888888888888');
            INSERT INTO catalog_media_entries VALUES
                (101,X'99999999999999999999999999999999'),
                (102,X'22222222222222222222222222222222'),
                (103,X'11111111111111111111111111111111'),
                (104,X'11111111111111111111111111111111'),
                (105,X'33333333333333333333333333333333'),
                (106,X'44444444444444444444444444444444'),
                (107,X'55555555555555555555555555555555'),
                (108,X'66666666666666666666666666666666'),
                (109,X'77777777777777777777777777777777'),
                (110,X'88888888888888888888888888888888');
            INSERT INTO hash_values VALUES
                (1,'sha1',X'0101010101010101010101010101010101010101'),
                (2,'sha1',X'0202020202020202020202020202020202020202'),
                (3,'sha1',X'0303030303030303030303030303030303030303'),
                (4,'sha1',X'0404040404040404040404040404040404040404');
            INSERT INTO catalog_entry_hashes VALUES
                (201,101,'sha1',0,'value','whole_file',3),
                (202,102,'sha1',0,'value','whole_file',2),
                (203,103,'sha1',0,'value','whole_file',1),
                (204,104,'sha1',0,'value','whole_asset',1),
                (205,105,'sha1',0,'value','unknown',1),
                (206,106,'sha1',0,'value','whole_file',1),
                (207,107,'sha1',0,'value','whole_file',1),
                (208,108,'sha1',0,'value','whole_file',2),
                (209,109,'sha1',0,'value','whole_file',4),
                (210,110,'sha1',0,'value','whole_file',2);
            INSERT INTO candidate_native_file_byte_coverage VALUES
                (101), (102), (103), (104), (105), (106), (107), (108), (109), (110);
            INSERT INTO candidate_canonical_hash_positions VALUES
                (201,101,'sha1',0), (202,102,'sha1',0), (203,103,'sha1',0),
                (204,104,'sha1',0), (205,105,'sha1',0), (207,107,'sha1',0),
                (208,108,'sha1',0), (209,109,'sha1',0), (210,110,'sha1',0);
            INSERT INTO rom_files (
                parent_path,path,name,sha1,xxhash3,in_archive,scan_root,scan_run,
                observed_size,source_fingerprint,scan_provenance
            ) VALUES (
                '/scan','/scan/game.rom','game.rom',
                X'0101010101010101010101010101010101010101',
                X'0101010101010101',FALSE,'/scan','run-1',20,
                X'0101010101010101010101010101010101010101',
                'streamed_sha1_xxh3_v1'
            ), (
                '/scan','/scan/superseded.rom','superseded.rom',
                X'0303030303030303030303030303030303030303',
                X'0303030303030303',FALSE,'/scan','run-1',20,
                X'0303030303030303030303030303030303030303',
                'streamed_sha1_xxh3_v1'
            ), (
                '/scan','/scan/publication-order.rom','publication-order.rom',
                X'0404040404040404040404040404040404040404',
                X'0404040404040404',FALSE,'/scan','run-1',20,
                X'0404040404040404040404040404040404040404',
                'streamed_sha1_xxh3_v1'
            );
            ",
        )?;

        assert_eq!(matching_rom_file_count(&mut conn, "/scan")?, 2);
        Ok(())
    }
}
