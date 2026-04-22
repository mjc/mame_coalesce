use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;

use crate::{
    Error,
    models::{DataFile, Game, NewDataFile, NewGame, NewRom, NewRomFile, Rom, RomFile},
};
use crate::{db::Pool as DbPool, logiqx};

use DieselError::NotFound;
use camino::{Utf8Path, Utf8PathBuf};
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
                    select games.id from games WHERE cloned.clone_of = games.name
                )",
        )
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

pub fn load_parents(
    pool: &DbPool,
    data_file_path: &Utf8Path,
) -> crate::Result<BTreeMap<Game, HashSet<(Rom, RomFile)>>> {
    use crate::schema::{
        self,
        games::dsl::{data_file_id, games},
        rom_files::dsl::rom_files,
        roms::dsl::roms,
    };
    let mut conn = pool.get()?;

    let canonicalized = fs::canonicalize(data_file_path)?;
    let full_path = Utf8PathBuf::from_path_buf(canonicalized).map_err(|_| {
        Error::InvalidPath("couldn't parse path to data file as unicode.".to_owned())
    })?;

    let df = schema::data_files::dsl::data_files
        .filter(schema::data_files::dsl::file_name.eq(full_path.as_str()))
        .first::<DataFile>(&mut conn)?;

    // Two-pass approach: first collect all parents by name, then group ROMs under their parent.
    // This avoids the ordering assumption that parents appear before clones in query results.
    let query_results: Vec<(Game, (Rom, RomFile))> = games
        .filter(data_file_id.eq(df.id))
        .inner_join(roms.inner_join(rom_files))
        .load(&mut conn)?;

    // First pass: collect all parent games (those with no parent_id)
    let parent_by_name: HashMap<String, Game> = query_results
        .iter()
        .filter(|(game, _)| game.parent_id.is_none())
        .map(|(game, _)| (game.name.clone(), game.clone()))
        .collect();

    // Second pass: group each row under its parent game
    let mut by_parent: BTreeMap<Game, HashSet<(Rom, RomFile)>> = BTreeMap::new();
    for (game, (rom, rom_file)) in query_results {
        let parent_key = if game.parent_id.is_none() {
            Some(game.clone())
        } else {
            game.clone_of
                .as_deref()
                .and_then(|parent_name| parent_by_name.get(parent_name))
                .cloned()
        };

        if let Some(parent) = parent_key {
            by_parent.entry(parent).or_default().insert((rom, rom_file));
        }
    }

    Ok(by_parent)
}
