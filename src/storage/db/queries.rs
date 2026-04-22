use crate::models::{NewDataFile, NewGame, NewRom, NewRomFile};
use crate::{db::Pool as DbPool, logiqx};

use diesel::result::Error as DieselError;
use diesel::{QueryResult, SqliteConnection};
use diesel::{prelude::*, sql_query};

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
        if let Some(existing_data_file_id) = data_files
            .filter(crate::schema::data_files::dsl::name.eq(new_data_file.name()))
            .select(crate::schema::data_files::dsl::id)
            .first::<i32>(conn)
            .optional()?
        {
            delete_data_file_children(conn, existing_data_file_id)?;
        }

        replace_into(data_files)
            .values(&new_data_file)
            .execute(conn)?;

        df_id = data_files
            .filter(crate::schema::data_files::dsl::name.eq(new_data_file.name()))
            .select(crate::schema::data_files::dsl::id)
            .first(conn)?;

        for game in logiqx_data_file.games() {
            let new_game = NewGame::from_logiqx(game, df_id);
            replace_into(games).values(new_game).execute(conn)?;

            let g_id = games
                .filter(crate::schema::games::dsl::data_file_id.eq(df_id))
                .filter(crate::schema::games::dsl::name.eq(game.name()))
                .select(crate::schema::games::dsl::id)
                .first(conn)?;

            for rom in game.roms() {
                let new_rom = NewRom::from_logiqx(rom, g_id);
                replace_into(roms).values(new_rom).execute(conn)?;
            }
        }

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

        associate_rom_files(conn)?;

        Ok(df_id)
    })
}

fn delete_data_file_children(conn: &mut SqliteConnection, data_file_id: i32) -> QueryResult<()> {
    use crate::schema::{
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

pub fn import_rom_files(pool: &DbPool, new_rom_files: &[NewRomFile]) -> crate::Result<usize> {
    use crate::schema::rom_files::dsl::rom_files;
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

fn associate_rom_files(conn: &mut SqliteConnection) -> QueryResult<usize> {
    sql_query("UPDATE rom_files SET rom_id = roms.id FROM roms WHERE rom_files.sha1 = roms.sha1")
        .execute(conn)
}
