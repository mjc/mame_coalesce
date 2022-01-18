use std::collections::{BTreeMap, HashSet};
use std::fs;

use crate::models::*;
use crate::{db::*, logiqx};

use camino::{Utf8Path, Utf8PathBuf};
use diesel::result::Error;
use diesel::{prelude::*, sql_query};

// TODO: return Result
pub fn traverse_and_insert_data_file(pool: &DbPool, logiqx_data_file: logiqx::DataFile) -> i32 {
    use crate::schema::{data_files::dsl::*, games::dsl::*, roms::dsl::*};
    use diesel::replace_into;

    let data_file_name = logiqx_data_file.file_name().unwrap();

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
        // it's an absurd job but somequery's gotta do it
        sql_query(
            r#"
            UPDATE games AS cloned
                SET parent_id = (
                    select games.id from games WHERE cloned.clone_of = games.name
                )"#,
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

pub fn load_parents(
    pool: &DbPool,
    data_file_path: &Utf8Path,
) -> BTreeMap<Game, HashSet<(Rom, RomFile)>> {
    use crate::schema::{self, games::dsl::*, rom_files::dsl::rom_files, roms::dsl::roms};
    let conn = pool.get().unwrap();

    let full_path = Utf8PathBuf::from_path_buf(fs::canonicalize(data_file_path).unwrap()).unwrap();
    let df_path = full_path.to_string();

    // TODO: This is fucking horrible
    // TODO:: .filter(sql("..."))
    let df = schema::data_files::dsl::data_files
        .filter(schema::data_files::dsl::file_name.eq(df_path))
        .first::<DataFile>(&conn)
        .unwrap();

    // TODO: scope by commandline path!
    let query_results: BTreeMap<Game, (Rom, RomFile)> = games
        .filter(data_file_id.eq(df.id()))
        .inner_join(roms.inner_join(rom_files))
        .group_by(schema::rom_files::dsl::sha1)
        .load(&conn)
        .unwrap()
        .into_iter()
        .collect();

    let (by_parent, _) = query_results.into_iter().fold(
        (BTreeMap::default(), None),
        |(mut grouped, mut parent), (game, (rom, rom_file))| {
            if game.parent_id.is_none() {
                parent = Some(game);
            }
            let entry = grouped
                .entry(parent.as_ref().unwrap().clone())
                .or_insert_with(HashSet::new);
            (*entry).insert((rom, rom_file));
            (grouped, parent)
        },
    );

    by_parent
}
