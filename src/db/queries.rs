use crate::models::*;
use crate::{db::*, logiqx};

use diesel::{prelude::*, result::Error, sql_query};

// this should definitely not be one giant file

pub fn traverse_and_insert_data_file(
    pool: &DbPool,
    logiqx_data_file: logiqx::DataFile,
    data_file_name: &str,
) {
    use crate::schema::{data_files::dsl::*, games::dsl::*, roms::dsl::*};
    use diesel::replace_into;

    let new_data_file = NewDataFile::from_logiqx(&logiqx_data_file, data_file_name);

    let conn = &pool.get().unwrap();

    conn.transaction::<_, Error, _>(|| {
        let df_id = replace_into(data_files)
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
