use crate::{
    logiqx,
    storage::{
        db::Pool as DbPool, models::NewDataFile, models::NewGame, models::NewRom,
        models::NewRomFile,
    },
};

use camino::{Utf8Path, Utf8PathBuf};
use diesel::result::Error as DieselError;
use diesel::{QueryResult, QueryableByName, SqliteConnection};
use diesel::{prelude::*, sql_query};

#[derive(QueryableByName)]
struct DatabasePathRow {
    #[diesel(sql_type = diesel::sql_types::Text)]
    file: String,
}

pub fn traverse_and_insert_data_file(
    pool: &DbPool,
    logiqx_data_file: &logiqx::DataFile,
) -> crate::Result<i32> {
    let new_data_file = NewDataFile::from_logiqx(logiqx_data_file);
    let mut conn = pool.get()?;

    conn.transaction::<_, crate::Error, _>(|conn| {
        delete_existing_data_file_children(conn, new_data_file.name())?;
        let data_file_id = upsert_data_file(conn, &new_data_file)?;
        insert_games_and_roms(conn, logiqx_data_file, data_file_id)?;
        update_parent_links(conn, data_file_id)?;
        associate_rom_files(conn)?;
        Ok(data_file_id)
    })
}

fn delete_existing_data_file_children(conn: &mut SqliteConnection, name: &str) -> QueryResult<()> {
    use crate::storage::schema::data_files::dsl as data_files_dsl;

    if let Some(existing_data_file_id) = data_files_dsl::data_files
        .filter(data_files_dsl::name.eq(name))
        .select(data_files_dsl::id)
        .first::<i32>(conn)
        .optional()?
    {
        delete_data_file_children(conn, existing_data_file_id)?;
    }

    Ok(())
}

fn delete_data_file_children(conn: &mut SqliteConnection, data_file_id: i32) -> QueryResult<()> {
    use crate::storage::schema::{
        games::dsl as games_dsl, rom_files::dsl as rom_files_dsl, roms::dsl as roms_dsl,
    };

    let game_ids = games_dsl::games
        .filter(games_dsl::data_file_id.eq(data_file_id))
        .select(games_dsl::id)
        .load::<i32>(conn)?;
    let rom_ids = roms_dsl::roms
        .filter(roms_dsl::game_id.eq_any(&game_ids))
        .select(roms_dsl::id)
        .load::<i32>(conn)?;

    if !rom_ids.is_empty() {
        diesel::update(rom_files_dsl::rom_files.filter(rom_files_dsl::rom_id.eq_any(&rom_ids)))
            .set(rom_files_dsl::rom_id.eq::<Option<i32>>(None))
            .execute(conn)?;
    }

    if !game_ids.is_empty() {
        diesel::delete(roms_dsl::roms.filter(roms_dsl::game_id.eq_any(game_ids))).execute(conn)?;
    }

    diesel::delete(games_dsl::games.filter(games_dsl::data_file_id.eq(data_file_id)))
        .execute(conn)?;

    Ok(())
}

fn upsert_data_file(
    conn: &mut SqliteConnection,
    new_data_file: &NewDataFile<'_>,
) -> QueryResult<i32> {
    use crate::storage::schema::data_files::dsl as data_files_dsl;
    use diesel::replace_into;

    replace_into(data_files_dsl::data_files)
        .values(new_data_file)
        .execute(conn)?;

    data_files_dsl::data_files
        .filter(data_files_dsl::name.eq(new_data_file.name()))
        .select(data_files_dsl::id)
        .first(conn)
}

fn insert_games_and_roms(
    conn: &mut SqliteConnection,
    logiqx_data_file: &logiqx::DataFile,
    data_file_id: i32,
) -> QueryResult<()> {
    use crate::storage::schema::{games::dsl as games_dsl, roms::dsl as roms_dsl};
    use diesel::replace_into;

    logiqx_data_file.games().iter().try_for_each(|game| {
        let new_game = NewGame::from_logiqx(game, data_file_id);
        replace_into(games_dsl::games)
            .values(new_game)
            .execute(conn)?;

        let game_id = games_dsl::games
            .filter(games_dsl::data_file_id.eq(data_file_id))
            .filter(games_dsl::name.eq(game.name()))
            .select(games_dsl::id)
            .first(conn)?;

        game.roms().iter().try_for_each(|rom| {
            let new_rom = NewRom::from_logiqx(rom, game_id);
            replace_into(roms_dsl::roms).values(new_rom).execute(conn)?;
            Ok(())
        })
    })
}

fn update_parent_links(conn: &mut SqliteConnection, data_file_id: i32) -> QueryResult<usize> {
    sql_query(
        r"
        UPDATE games AS cloned
            SET parent_id = (
                SELECT parent.id
                FROM games AS parent
                WHERE parent.name = cloned.clone_of
                    AND parent.data_file_id = cloned.data_file_id
            )
            WHERE cloned.data_file_id = ?",
    )
    .bind::<diesel::sql_types::Integer, _>(data_file_id)
    .execute(conn)
}

#[cfg(test)]
pub fn import_rom_files(pool: &DbPool, new_rom_files: &[NewRomFile]) -> crate::Result<usize> {
    use crate::storage::schema::rom_files::dsl::rom_files;
    use diesel::replace_into;

    let mut conn = pool.get()?;

    Ok(conn.transaction::<_, DieselError, _>(|conn| {
        new_rom_files
            .iter()
            .map(|new_rom_file| replace_into(rom_files).values(new_rom_file).execute(conn))
            .collect::<QueryResult<Vec<usize>>>()?;
        associate_rom_files(conn)
    })?)
}

pub fn replace_rom_files_for_source_root(
    pool: &DbPool,
    source_root: &Utf8Path,
    new_rom_files: &[NewRomFile],
) -> crate::Result<usize> {
    use crate::storage::schema::rom_files::dsl::rom_files;
    use diesel::replace_into;

    let mut conn = pool.get()?;

    Ok(conn.transaction::<_, DieselError, _>(|conn| {
        delete_rom_files_for_source_root(conn, source_root.as_str())?;
        new_rom_files
            .iter()
            .map(|new_rom_file| replace_into(rom_files).values(new_rom_file).execute(conn))
            .collect::<QueryResult<Vec<usize>>>()?;
        associate_rom_files(conn)
    })?)
}

pub fn database_file_paths(pool: &DbPool) -> crate::Result<Vec<Utf8PathBuf>> {
    let mut conn = pool.get()?;
    let rows = sql_query("SELECT file FROM pragma_database_list WHERE file != ''")
        .load::<DatabasePathRow>(&mut conn)?;

    Ok(rows
        .into_iter()
        .filter_map(|row| Utf8PathBuf::from(row.file).canonicalize_utf8().ok())
        .flat_map(|path| {
            let base = path.as_str().to_owned();
            std::iter::once(path).chain(
                ["-wal", "-shm", "-journal"]
                    .into_iter()
                    .map(move |suffix| Utf8PathBuf::from(format!("{base}{suffix}"))),
            )
        })
        .collect())
}

fn associate_rom_files(conn: &mut SqliteConnection) -> QueryResult<usize> {
    sql_query("UPDATE rom_files SET rom_id = roms.id FROM roms WHERE rom_files.sha1 = roms.sha1")
        .execute(conn)
}

fn delete_rom_files_for_source_root(
    conn: &mut SqliteConnection,
    source_root: &str,
) -> QueryResult<usize> {
    sql_query(
        r"
        DELETE FROM rom_files
        WHERE parent_path = ?
            OR (
                length(parent_path) > length(?)
                AND substr(parent_path, 1, length(?)) = ?
                AND substr(parent_path, length(?) + 1, 1) = '/'
            )
        ",
    )
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
    fn database_file_paths_include_sqlite_sidecar_paths() -> Result<(), Box<dyn std::error::Error>>
    {
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
        Ok(())
    }
}
