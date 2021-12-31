use std::convert::TryInto;

use diesel::SqliteConnection;

use crate::models::*;
use crate::{logiqx, models};

use crate::schema;
use diesel::prelude::*;

use indicatif::{ProgressBar, ProgressIterator, ProgressStyle};

// this should definitely not be one giant file

pub fn traverse_and_insert_data_file(
    conn: SqliteConnection,
    data_file: logiqx::DataFile,
    file_name: &str,
) {
    let progress_style = ProgressStyle::default_bar()
        .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg} {eta_precise}");
    let pb = ProgressBar::new(data_file.games().len() as u64).with_style(progress_style);

    let df_id = insert_data_file(&conn, &data_file, &file_name);

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

fn insert_data_file(conn: &SqliteConnection, data_file: &logiqx::DataFile, df_name: &str) -> usize {
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

fn insert_game(conn: &SqliteConnection, game: &logiqx::Game, df_id: &usize) -> usize {
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

fn insert_rom(conn: &SqliteConnection, rom: &logiqx::Rom, g_id: &usize) -> usize {
    use schema::{roms, roms::dsl::*};

    let new_rom = (
        name.eq(rom.name()),
        size.eq(rom.size()),
        md5.eq(rom.md5()),
        sha1.eq(rom.sha1()),
        crc.eq(rom.crc()),
        game_id.eq(*g_id as i32),
    );

    let insert_id = diesel::insert_into(roms::table)
        .values(&new_rom)
        .execute(conn)
        .optional()
        .unwrap_or(None);

    match insert_id {
        Some(rom_id) => rom_id,
        None => diesel::update(roms.filter(name.eq(rom.name())))
            .set(new_rom)
            .execute(conn)
            .expect("Error updating Game"),
    }
}

// fn insert_file(conn: &SqliteConnection, rom_file: &files::RomFile, df_name: &str) -> usize {
//     use schema::{files, files::dsl::*};

//     let new_file = (
//         path.eq(rom_file.path()),
//         name.eq(rom_file.name()),
//         crc.eq(rom_file.crc()),
//         sha1.eq(rom_file.sha1()),
//         md5.eq(rom_file.md5()),
//         in_archive.eq(rom_file.in_archive()),
//     );

//     let insert_id = diesel::insert_into(files::table)
//         .values(&new_file)
//         .execute(conn)
//         .optional()
//         .unwrap_or(None);

//     match insert_id {
//         Some(rom_file_id) => rom_file_id,
//         None => diesel::update(files.filter(name.eq(rom_file.header().name())))
//             .set(new_file)
//             .execute(conn)
//             .expect("Error updating DataFile"),
//     }
// }
