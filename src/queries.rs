use crate::logiqx;
use crate::models::*;

use diesel::{prelude::*, r2d2::ConnectionManager, result::Error};
use r2d2::Pool;

// this should definitely not be one giant file

pub fn traverse_and_insert_data_file(
    pool: &r2d2::Pool<ConnectionManager<SqliteConnection>>,
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

pub fn import_rom_files(
    pool: &Pool<ConnectionManager<SqliteConnection>>,
    new_rom_files: &[NewRomFile],
) {
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
        Ok(true)
    })
    .unwrap();
}
