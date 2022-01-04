use crate::logiqx;
use crate::models::*;

use diesel::{prelude::*, r2d2::ConnectionManager};

// this should definitely not be one giant file

pub fn traverse_and_insert_data_file(
    pool: &r2d2::Pool<ConnectionManager<SqliteConnection>>,
    logiqx_data_file: logiqx::DataFile,
    data_file_name: &str,
) {
    use crate::schema::{data_files::dsl::*, games::dsl::*, roms::dsl::*};
    use diesel::replace_into;

    let new_data_file = NewDataFile::from_logiqx(&logiqx_data_file, data_file_name);

    // TODO: investigate why this is slower
    // TODO: parallelize
    // TODO: transaction?

    let conn = &pool.get().unwrap();

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
}
