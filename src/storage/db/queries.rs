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

/// Counts observations with at least one published native catalog candidate.
/// A shared whole-file SHA1 is a candidate signal, not proof of a match.
fn matching_rom_file_count(conn: &mut SqliteConnection, source_root: &str) -> QueryResult<usize> {
    let row = sql_query(
        r"
        SELECT COUNT(*) AS count
        FROM rom_files AS observed
        WHERE observed.scan_root = ?
          AND EXISTS (
              SELECT 1
              FROM asset_occurrences AS occurrence
              JOIN catalog_sets AS catalog_set
                ON catalog_set.set_id = occurrence.record_id
              JOIN catalog_set_groups AS catalog_group
                ON catalog_group.set_group_id = catalog_set.set_group_id
              JOIN snapshot_publications AS publication
                ON publication.snapshot_key = catalog_group.snapshot_key
              JOIN occurrence_digest_assertions AS assertion
                ON assertion.occurrence_id = occurrence.occurrence_id
              JOIN digest_values AS digest
                ON digest.digest_id = assertion.digest_id
              WHERE digest.algorithm = 'sha1'
                AND digest.digest = observed.sha1
                AND assertion.scope = 'whole_asset'
                AND assertion.provenance = 'source_declared'
                AND occurrence.claim_kind IN (
                    'mame_rom', 'logiqx_rom', 'cmp_rom',
                    'no_intro_pc_file', 'software_rom_entry'
                )
                AND publication.rowid = (
                    SELECT latest.rowid
                    FROM snapshot_publications AS latest
                    WHERE latest.catalog_key = publication.catalog_key
                    ORDER BY latest.rowid DESC
                    LIMIT 1
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
}
