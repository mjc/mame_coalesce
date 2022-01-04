use std::path::PathBuf;

use crate::models::*;
use crate::{db::*, logiqx};

use diesel::{prelude::*, result::Error, sql_query};

// this should definitely not be one giant file

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
        df_id = replace_into(data_files)
            .values(&new_data_file)
            .execute(conn)
            .unwrap() as i32;

        logiqx_data_file.games().iter().for_each(|game| {
            let new_game = NewGame::from_logiqx(game, &df_id);
            let g_id = replace_into(games).values(new_game).execute(conn).unwrap() as i32;
            game.roms().iter().for_each(|rom| {
                let new_rom = NewRom::from_logiqx(rom, &g_id);
                replace_into(roms).values(new_rom).execute(conn).unwrap() as i32;
            });
        });
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

pub fn load_rom_files(pool: &DbPool, _df_id: i32, source_path: &PathBuf) -> Vec<RomFile> {
    use crate::schema::rom_files::dsl::*;
    let conn = pool.get().unwrap();
    let p = source_path.to_str().unwrap().to_string();

    // TODO: this should be respecting the data_file_id
    rom_files
        .filter(parent_path.eq(p))
        .get_results(&conn)
        .unwrap()
}
