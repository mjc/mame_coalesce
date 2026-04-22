use crate::models::{NewDataFile, NewGame, NewRom, NewRomFile};
use crate::{db::Pool as DbPool, logiqx};

use DieselError::NotFound;
use diesel::result::Error as DieselError;
use diesel::{prelude::*, sql_query};
use log::warn;

pub fn traverse_and_insert_data_file(
    pool: &DbPool,
    logiqx_data_file: &logiqx::DataFile,
) -> crate::Result<i32> {
    use crate::schema::{data_files::dsl::data_files, games::dsl::games, roms::dsl::roms};
    use diesel::replace_into;

    let new_data_file = NewDataFile::from_logiqx(logiqx_data_file);
    let mut conn = pool.get()?;
    let mut df_id = -1;

    conn.transaction::<_, crate::Error, _>(|conn| {
        replace_into(data_files)
            .values(&new_data_file)
            .execute(conn)?;

        df_id = data_files
            .order(crate::schema::data_files::dsl::id.desc())
            .select(crate::schema::data_files::dsl::id)
            .first(conn)?;

        logiqx_data_file.games().iter().for_each(|game| {
            let new_game = NewGame::from_logiqx(game, df_id);
            if let Err(e) = replace_into(games).values(new_game).execute(conn) {
                warn!("Couldn't update record for game: {game:?}, error: {e}");
                return;
            }

            let g_id_result = games
                .order(crate::schema::games::dsl::id.desc())
                .select(crate::schema::games::dsl::id)
                .first(conn);

            if let Ok(g_id) = g_id_result {
                game.roms().iter().for_each(|rom| {
                    let new_rom = NewRom::from_logiqx(rom, g_id);
                    if let Err(e) = replace_into(roms).values(new_rom).execute(conn) {
                        warn!("Couldn't update record for {rom:?}, error: {e}");
                    }
                });
            }
        });

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
        .bind::<diesel::sql_types::Integer, _>(df_id)
        .execute(conn)?;

        Ok(df_id)
    })
}

pub fn import_rom_files(pool: &DbPool, new_rom_files: &[NewRomFile]) -> QueryResult<usize> {
    use crate::schema::rom_files::dsl::rom_files;
    use diesel::replace_into;

    let mut conn = pool.get().map_err(|_| NotFound)?;

    conn.transaction::<_, DieselError, _>(|conn| {
        new_rom_files
            .iter()
            .map(|new_rom_file| replace_into(rom_files).values(new_rom_file).execute(conn))
            .collect::<QueryResult<Vec<usize>>>()?;
        sql_query(
            "UPDATE rom_files SET rom_id = roms.id FROM roms WHERE rom_files.sha1 = roms.sha1",
        )
        .execute(conn)
    })
}
