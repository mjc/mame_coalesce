use std::collections::{BTreeMap, HashSet};
use std::{error, fs};

use crate::{
    db::SyncPooledConnection,
    logiqx,
    models::{DataFile, Game, NewDataFile, NewGame, NewRom, NewRomFile, Rom, RomFile},
    MameResult,
};

use camino::{Utf8Path, Utf8PathBuf};
use diesel::result::Error;
use diesel::{prelude::*, sql_query};
use log::warn;

// TODO: return Result
pub fn traverse_and_insert_data_file(
    conn: impl Connection<Backend = diesel::sqlite::Sqlite>,
    logiqx_data_file: &logiqx::DataFile,
) -> MameResult<i32> {
    use crate::schema::{data_files::dsl::data_files, games::dsl::games, roms::dsl::roms};
    use diesel::replace_into;

    let new_data_file = NewDataFile::from_logiqx(logiqx_data_file);

    // TODO: return from transaction?
    let mut df_id = -1;

    conn.transaction::<_, Box<dyn error::Error>, _>(|| {
        replace_into(data_files)
            .values(&new_data_file)
            .execute(&conn)?;

        df_id = data_files
            .order(crate::schema::data_files::dsl::id.desc())
            .select(crate::schema::data_files::dsl::id)
            .first(&conn)?;

        logiqx_data_file.games().iter().for_each(|game| {
            let new_game = NewGame::from_logiqx(game, df_id);
            if let Err(e) = replace_into(games).values(new_game).execute(&conn) {
                warn!("Couldn't update record for game: {game:?}, error: {e}");
                return;
            };

            let g_id_result = games
                .order(crate::schema::games::dsl::id.desc())
                .select(crate::schema::games::dsl::id)
                .first(&conn);

            if let Ok(g_id) = g_id_result {
                game.roms().iter().for_each(|rom| {
                    let new_rom = NewRom::from_logiqx(rom, g_id);
                    if let Err(e) = replace_into(roms).values(new_rom).execute(&conn) {
                        warn!("Couldn't update record for {rom:?}, error: {e}");
                    };
                });
            }
        });

        // TODO: figure out how to do this with the dsl
        // it's an absurd job but somequery's gotta do it
        sql_query(
            r#"
            UPDATE games AS cloned
                SET parent_id = (
                    select games.id from games WHERE cloned.clone_of = games.name
                )"#,
        )
        .execute(&conn)?;

        Ok(df_id)
    })
}

pub fn import_rom_files(
    conn: impl Connection<Backend = diesel::sqlite::Sqlite>,
    new_rom_files: &[NewRomFile],
) -> QueryResult<usize> {
    use crate::schema::rom_files::dsl::rom_files;
    use diesel::replace_into;

    conn.transaction::<_, Error, _>(|| {
        new_rom_files
            .iter()
            .map(|new_rom_file| replace_into(rom_files).values(new_rom_file).execute(&conn))
            .collect::<QueryResult<Vec<usize>>>()?;
        // TODO: figure out how to do this with the dsl
        // TODO: this is gonna do weird shit if you have things already inserted
        sql_query(
            "UPDATE rom_files SET rom_id = roms.id FROM roms WHERE rom_files.sha1 = roms.sha1",
        )
        .execute(&conn)
    })
}

pub fn load_parents(
    conn: impl Connection<Backend = diesel::sqlite::Sqlite>,
    data_file_path: &Utf8Path,
) -> MameResult<BTreeMap<Game, HashSet<(Rom, RomFile)>>> {
    use crate::schema::{
        self,
        games::dsl::{data_file_id, games},
        rom_files::dsl::rom_files,
        roms::dsl::roms,
    };

    let canonicalized = fs::canonicalize(data_file_path)?;
    let full_path = Utf8PathBuf::from_path_buf(canonicalized)
        .map_err(|_| "couldn't parse path to data file as unicode.")?;

    // TODO: This is fucking horrible
    // TODO:: .filter(sql("..."))
    let df = schema::data_files::dsl::data_files
        .filter(schema::data_files::dsl::file_name.eq(full_path.as_str()))
        .first::<DataFile>(&conn)?;

    // TODO: scope by commandline path!
    let query_results: BTreeMap<Game, (Rom, RomFile)> = games
        .filter(data_file_id.eq(df.id()))
        .inner_join(roms.inner_join(rom_files))
        .group_by(schema::rom_files::dsl::sha1)
        .load(&conn)?
        .into_iter()
        .collect();

    let (by_parent, _) = query_results.into_iter().fold(
        (BTreeMap::default(), None),
        |(mut grouped, mut parent), (game, (rom, rom_file))| {
            if game.parent_id.is_none() {
                parent = Some(game);
            }
            if let Some(key) = parent.clone() {
                let entry = grouped.entry(key).or_insert_with(HashSet::new);
                (*entry).insert((rom, rom_file));
            }
            (grouped, parent)
        },
    );

    Ok(by_parent)
}
