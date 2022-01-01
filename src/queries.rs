use diesel_logger::LoggingConnection;
use log::info;

use crate::logiqx;
use crate::models::*;

use crate::schema;
use diesel::prelude::*;

use indicatif::{ProgressBar, ProgressIterator, ProgressStyle};

// this should definitely not be one giant file

pub fn traverse_and_insert_data_file(
    conn: &LoggingConnection<SqliteConnection>,
    data_file: logiqx::DataFile,
    data_file_name: &str,
) {
    use schema::data_files::dsl::*;

    let progress_style = ProgressStyle::default_bar()
        .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg} {eta_precise}");
    let pb = ProgressBar::new(data_file.games().len() as u64).with_style(progress_style);

    let already_current = diesel::select(diesel::dsl::exists(
        data_files.filter(sha1.eq(data_file.sha1())),
    ))
    .get_result(conn);

    match already_current {
        Ok(true) => (),
        Ok(false) | Err(_) => {
            let df_id = insert_data_file(&conn, &data_file, &data_file_name);

            for game in data_file.games().iter().progress_with(pb) {
                // this should be a bulk insert with on_conflict but
                // 1. I don't care (15 seconds for just games isn't terrible)
                // 2. on_conflict for sqlite isn't in diesel 1.4
                let g_id = insert_game(&conn, game, &df_id);
                for rom in game.roms().iter() {
                    insert_rom(&conn, rom, &g_id);
                }
            }
        }
    }
}

fn insert_data_file(
    conn: &LoggingConnection<SqliteConnection>,
    data_file: &logiqx::DataFile,
    df_name: &str,
) -> usize {
    use schema::{data_files, data_files::dsl::*};

    let new_data_file = (
        build.eq(data_file.build()),
        debug.eq(data_file.debug()),
        file_name.eq(df_name),
        name.eq(data_file.header().name()),
        description.eq(data_file.header().description()),
        version.eq(data_file.header().version()),
        author.eq(data_file.header().author()),
        homepage.eq(data_file.header().homepage()),
        url.eq(data_file.header().url()),
        sha1.eq(data_file.sha1()),
    );

    let insert_id = diesel::insert_into(data_files::table)
        .values(&new_data_file)
        .execute(conn)
        .optional()
        .unwrap_or(None);

    match insert_id {
        Some(data_file_id) => data_file_id,
        None => diesel::update(data_files.filter(name.eq(data_file.header().name())))
            .set(new_data_file)
            .execute(conn)
            .expect("Error updating DataFile"),
    }
}

fn insert_game(
    conn: &LoggingConnection<SqliteConnection>,
    game: &logiqx::Game,
    df_id: &usize,
) -> usize {
    use schema::{games, games::dsl::*};

    let new_game = (
        name.eq(game.name()),
        is_bios.eq(game.isbios()),
        clone_of.eq(game.cloneof()),
        rom_of.eq(game.romof()),
        sample_of.eq(game.sampleof()),
        board.eq(game.board()),
        rebuildto.eq(game.rebuildto()),
        year.eq(game.year()),
        manufacturer.eq(game.manufacturer()),
        data_file_id.eq(*df_id as i32),
    );

    let insert_id = diesel::insert_into(games::table)
        .values(&new_game)
        .execute(conn)
        .optional()
        .unwrap_or(None);

    match insert_id {
        Some(game_id) => game_id,
        None => diesel::update(games.filter(name.eq(game.name())))
            .set(new_game)
            .execute(conn)
            .expect("Error updating Game"),
    }
}

fn insert_rom(
    conn: &LoggingConnection<SqliteConnection>,
    rom: &logiqx::Rom,
    g_id: &usize,
) -> usize {
    use schema::{roms, roms::dsl::*};

    let new_rom = Rom {
        id: None,
        name: rom.name().to_string(),
        md5: rom.md5().to_vec(),
        sha1: rom.sha1().to_vec(),
        crc: rom.crc().to_vec(),
        date: "".to_string(),
        updated_at: None,
        inserted_at: None,
        game_id: *g_id as i32,
    };

    let insert_id = diesel::insert_into(roms::table)
        .values(&new_rom)
        .execute(conn)
        .optional()
        .unwrap_or(None);

    match insert_id {
        Some(rom_id) => rom_id,
        None => diesel::update(roms.filter(name.eq(rom.name())))
            .set(&new_rom)
            .execute(conn)
            .expect("Error updating Game"),
    }
}

// TODO: this should be one struct, not two, and not have to convert.
pub fn import_rom_file(conn: &LoggingConnection<SqliteConnection>, rom_file: &RomFile) {
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

fn insert_rom_file(conn: &LoggingConnection<SqliteConnection>, rom_file: &RomFile) -> usize {
    use schema::{rom_files, rom_files::dsl::*};

    let insert_id = diesel::insert_into(rom_files::table)
        .values(rom_file)
        .execute(conn)
        .optional()
        .unwrap();

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
