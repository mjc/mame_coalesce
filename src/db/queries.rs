use std::collections::HashMap;

use crate::models::*;
use crate::{db::*, logiqx};

use diesel::{prelude::*, result::Error, sql_query};

pub fn traverse_and_insert_data_file(
    pool: &DbPool,
    logiqx_data_file: logiqx::DataFile,
    data_file_name: &str,
) -> i32 {
    use crate::schema::{data_files::dsl::*, games::dsl::*, roms::dsl::*};
    use diesel::replace_into;

    let new_data_file = NewDataFile::from_logiqx(&logiqx_data_file, data_file_name);

    let conn = &pool.get().unwrap();

    // TODO: return from transaction?
    let mut df_id = -1;

    conn.transaction::<_, Error, _>(|| {
        replace_into(data_files)
            .values(&new_data_file)
            .execute(conn)
            .unwrap();

        df_id = data_files
            .order(crate::schema::data_files::dsl::id.desc())
            .select(crate::schema::data_files::dsl::id)
            .first(conn)
            .unwrap();

        logiqx_data_file.games().iter().for_each(|game| {
            let new_game = NewGame::from_logiqx(game, &df_id);
            replace_into(games).values(new_game).execute(conn).unwrap();
            let g_id = games
                .order(crate::schema::games::dsl::id.desc())
                .select(crate::schema::games::dsl::id)
                .first(conn)
                .unwrap();
            game.roms().iter().for_each(|rom| {
                let new_rom = NewRom::from_logiqx(rom, &g_id);
                replace_into(roms).values(new_rom).execute(conn).unwrap();
            });
        });

        // TODO: figure out how to do this with the dsl
        sql_query(
            "UPDATE games AS cloned SET parent_id = (select games.id from games WHERE cloned.clone_of = games.name)",
        )
        .execute(conn)
        .unwrap();

        Ok(df_id)
    })
    .unwrap();

    df_id
}

pub fn import_rom_files(pool: &DbPool, new_rom_files: &[NewRomFile]) {
    use crate::schema::rom_files::dsl::*;
    use diesel::replace_into;

    let conn = pool.get().unwrap();

    conn.transaction::<_, Error, _>(|| {
        new_rom_files.iter().for_each(|new_rom_file| {
            replace_into(rom_files)
                .values(new_rom_file)
                .execute(&conn)
                .unwrap();
        });
        // TODO: figure out how to do this with the dsl
        // TODO: this is gonna do weird shit if you have things already inserted
        sql_query(
            "UPDATE rom_files SET rom_id = roms.id FROM roms WHERE rom_files.sha1 = roms.sha1",
        )
        .execute(&conn)
        .unwrap();
        Ok(true)
    })
    .unwrap();
}

pub fn load_unpacked_games(pool: &DbPool, df_id: &i32) -> HashMap<Game, (Rom, RomFile)> {
    use crate::schema::{games::dsl::*, rom_files::dsl::rom_files, roms::dsl::roms};
    let conn = pool.get().unwrap();

    // TODO: remove is_archive check once we handle source archives correctly.
    games
        .filter(data_file_id.eq(df_id))
        .inner_join(roms.inner_join(rom_files))
        .filter(crate::schema::rom_files::in_archive.eq(false))
        .load(&conn)
        .unwrap()
        .into_iter()
        .collect()
}

pub fn load_packed_games(pool: &DbPool, df_id: &i32) -> HashMap<Game, (Rom, RomFile)> {
    use crate::schema::{games::dsl::*, rom_files::dsl::rom_files, roms::dsl::roms};
    let conn = pool.get().unwrap();

    // TODO: remove is_archive check once we handle source archives correctly.
    games
        .filter(data_file_id.eq(df_id))
        .inner_join(roms.inner_join(rom_files))
        .filter(crate::schema::rom_files::in_archive.eq(false))
        .load(&conn)
        .unwrap()
        .into_iter()
        .collect()
}
