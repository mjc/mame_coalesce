use log::info;

use crate::models::*;
use crate::{logiqx, models};

use crate::schema;
use diesel::{prelude::*, r2d2::ConnectionManager};

// this should definitely not be one giant file

pub fn traverse_and_insert_data_file(
    pool: &r2d2::Pool<ConnectionManager<SqliteConnection>>,
    logiqx_data_file: logiqx::DataFile,
    data_file_name: &str,
) {
    let conn = pool.get().unwrap();

    let new_data_file = NewDataFile::from_logiqx(&logiqx_data_file, data_file_name);

    // TODO: if sha1 doesn't match, don't skip games
    match lookup_data_file(&conn, &new_data_file) {
        Some(_data_file) => (),
        None => {
            let games = &logiqx_data_file.games();
            let db_data_file = insert_data_file(&conn, new_data_file).unwrap();
            iterate_logiqx_games(&conn, games, db_data_file.id());
        }
    }
}

fn iterate_logiqx_games(conn: &SqliteConnection, games: &[logiqx::Game], data_file_id: &i32) {
    games.iter().for_each(|game| {
        let g_id = insert_game(&conn, game, data_file_id);
        game.roms().iter().for_each(|logiqx_rom: &logiqx::Rom| {
            let new_rom = NewRom::from_logiqx(logiqx_rom, g_id);
            insert_rom(&conn, new_rom);
        })
    });
}

fn lookup_data_file(
    conn: &SqliteConnection,
    new_data_file: &models::NewDataFile,
) -> Option<DataFile> {
    use schema::data_files::dsl::*;

    data_files
        .filter(sha1.eq(new_data_file.sha1()))
        .or_filter(name.eq(new_data_file.name()))
        .limit(1)
        .first::<DataFile>(conn)
        .ok()
}

fn insert_data_file(
    conn: &SqliteConnection,
    new_data_file: models::NewDataFile,
) -> Option<DataFile> {
    use schema::{data_files, data_files::dsl::*};

    let insert_id = diesel::insert_into(data_files::table)
        .values(&new_data_file)
        .execute(conn)
        .optional()
        .unwrap_or(None);

    match insert_id {
        Some(data_file_id) => data_files
            .filter(id.eq(data_file_id as i32))
            .limit(1)
            .first::<DataFile>(conn)
            .ok(),
        None => {
            let df_id = diesel::update(data_files.filter(name.eq(new_data_file.name())))
                .set(&new_data_file)
                .execute(conn)
                .expect("Error updating DataFile");
            data_files
                .filter(id.eq(df_id as i32))
                .limit(1)
                .first::<DataFile>(conn)
                .ok()
        }
    }
}

// this should be a bulk insert with on_conflict but
// 1. I don't care (15 seconds for just games isn't terrible)
// 2. on_conflict for sqlite isn't in diesel 1.4
fn insert_game(conn: &SqliteConnection, game: &logiqx::Game, df_id: &i32) -> usize {
    use schema::{games, games::dsl::*};

    let new_game = NewGame::from_logiqx(game, df_id);

    let insert_id = diesel::insert_into(games::table)
        .values(&new_game)
        .execute(conn)
        .optional()
        .unwrap_or(None);

    match insert_id {
        Some(game_id) => game_id,
        None => diesel::update(games.filter(name.eq(game.name())))
            .set(&new_game)
            .execute(conn)
            .expect("Error updating Game"),
    }
}

fn insert_rom(conn: &SqliteConnection, new_rom: models::NewRom) -> usize {
    use schema::{roms, roms::dsl::*};

    let insert_id = diesel::insert_into(roms::table)
        .values(&new_rom)
        .execute(conn)
        .optional()
        .unwrap_or(None);

    match insert_id {
        Some(rom_id) => rom_id,
        None => diesel::update(roms.filter(name.eq(&new_rom.name)))
            .set(&new_rom)
            .execute(conn)
            .expect("Error updating Game"),
    }
}

// TODO: this should be one struct, not two, and not have to convert.
pub fn import_rom_file(conn: &SqliteConnection, rom_file: &NewRomFile) {
    use schema::rom_files::dsl::*;

    let already_current = diesel::select(diesel::dsl::exists(
        rom_files.filter(sha1.eq(&rom_file.sha1)),
    ))
    .get_result(conn);

    match already_current {
        Ok(true) => (),
        Ok(false) | Err(_) => {
            let rf_id = insert_rom_file(&conn, &rom_file);
            info!("rom_file_id: {:?}", rf_id);
        }
    }
}

fn insert_rom_file(conn: &SqliteConnection, rom_file: &NewRomFile) -> usize {
    use schema::{rom_files, rom_files::dsl::*};

    let insert_id = diesel::insert_into(rom_files::table)
        .values(rom_file)
        .execute(conn)
        .ok();

    // TODO: try and match as many hashes as possible? match sha1?
    // TODO: investigate prevalence of crc, sha1, md5
    // TODO: add blake3?
    match insert_id {
        Some(rom_file_id) => rom_file_id,
        None => diesel::update(rom_files.filter(sha1.eq(&rom_file.sha1)))
            .set(rom_file)
            .execute(conn)
            .expect("Error updating DataFile"),
    }
}
