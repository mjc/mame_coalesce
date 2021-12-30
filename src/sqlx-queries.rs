use std::path::Path;

use log::debug;
use sqlx::SqlitePool;

use super::logiqx;

// this definitely should not be one giant file

pub async fn setup_sqlx(url: String) -> SqlitePool {
        // TODO: use env var
    let connect_options = SqliteConnectOptions::new()
        .filename("coalesce.db")
        .log_statements(LevelFilter::Debug)
        .to_owned();
    let pool = SqlitePool::connect_with(connect_options).await.unwrap();
    run_migrations(&pool).await;
    pool
}


async fn run_migrations(pool: &Pool<Sqlite>) {
    sqlx::migrate!().run(pool).await.unwrap();
}

pub async fn upsert_entire_dat_file(pool: SqlitePool, data_file: &logiqx::DataFile, path: &str) {
    let data_file_id = queries::upsert_data_file(&pool, &data_file, path).await;
    let pb_style = ProgressStyle::default_bar()
        .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg} {eta_precise}");
    let game_bar = ProgressBar::new(data_file.games().len() as u64).with_style(pb_style.clone());

    for game in data_file.games().iter().progress_with(game_bar) {
        let game_id = queries::upsert_game(&pool, &game, &data_file_id).await;
        let game_queries = game
            .roms
            .iter()
            .map(|rom| queries::upsert_rom(&pool, rom, &game_id));
        future::join_all(game_queries).await;
    }
}


pub async fn upsert_data_file(pool: &SqlitePool, data_file: &logiqx::DataFile, path: &str) -> i64 {
    // gross
    let mut conn = pool.acquire().await.unwrap();
    // gross
    let file_name = Path::new(path).file_name().unwrap().to_str().unwrap();

    let id = sqlx::query!(
        r#"
        INSERT INTO data_files (
            build,
            debug,
            name,
            description,
            version,
            author,
            homepage,
            url,
            file_name
        )
        VALUES ( ?, ?, ?, ?, ?, ?, ?, ?, ? )
        ON CONFLICT(name)
            DO UPDATE SET
                build = excluded.build,
                debug = excluded.debug,
                description = excluded.description,
                version = excluded.version,
                author = excluded.author,
                homepage = excluded.homepage,
                url = excluded.url,
                file_name = excluded.file_name
         "#,
        data_file.build,
        data_file.debug,
        data_file.header().name,
        data_file.header().description,
        data_file.header().version,
        data_file.header().author,
        data_file.header().homepage,
        data_file.header().url,
        file_name
    )
    .execute(&mut conn)
    .await
    .unwrap() // gross
    .last_insert_rowid();

    // super gross
    if id == 0 {
        sqlx::query!(
            "SELECT id FROM data_files WHERE name = ?",
            data_file.header().name
        )
        .fetch_one(&mut conn)
        .await
        .unwrap()
        .id
    } else {
        id
    }
}

pub async fn upsert_game(pool: &SqlitePool, game: &logiqx::Game, data_file_id: &i64) -> i64 {
    // gross
    let mut conn = pool.acquire().await.unwrap();
    debug!("Game: {:?}, id: {:?}", &game, &data_file_id);
    // TODO: clone_of, rom_of, sample_of, rebuild_to
    let id: i64 = sqlx::query!(
        r#"
        INSERT INTO games (
            name,
            is_bios,
            board,
            year,
            manufacturer,
            data_file_id
        )
        VALUES (?, ?, ?, ?, ?, ?)
        ON CONFLICT(name) DO UPDATE SET
            is_bios = excluded.is_bios,
            board = excluded.board,
            year = excluded.year,
            manufacturer = excluded.manufacturer,
            data_file_id = excluded.data_file_id
    "#,
        game.name,
        game.isbios,
        game.board,
        game.year,
        game.manufacturer,
        data_file_id
    )
    .execute(&mut conn)
    .await
    .unwrap()
    .last_insert_rowid();
    if id == 0 {
        sqlx::query!("SELECT id FROM games WHERE name = ?", game.name)
            .fetch_one(&mut conn)
            .await
            .unwrap()
            .id
    } else {
        id
    }
}

pub async fn upsert_rom(pool: &SqlitePool, rom: &logiqx::Rom, game_id: &i64) -> i64 {
    // gross
    let mut conn = pool.acquire().await.unwrap();
    debug!("Rom: {:?}, id: {:?}", &rom, &game_id);
    // TODO: clone_of, rom_of, sample_of, rebuild_to
    let id: i64 = sqlx::query!(
        r#"
        INSERT INTO roms (
            name,
            size,
            md5,
            sha1,
            crc,
            date,
            game_id,
            updated_at,
            inserted_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, strftime('%s','now'), strftime('%s','now'))
        ON CONFLICT(name) DO UPDATE SET
    size = size,
    md5 = md5,
    sha1 = sha1,
    crc = crc,
    date = date,
    updated_at = updated_at
    "#,
        rom.name,
        rom.size,
        rom.md5,
        rom.sha1,
        rom.crc,
        rom.date,
        game_id
    )
    .execute(&mut conn)
    .await
    .unwrap()
    .last_insert_rowid();
    if id == 0 {
        sqlx::query!("SELECT id FROM roms WHERE name = ?", rom.name)
            .fetch_one(&mut conn)
            .await
            .unwrap()
            .id
    } else {
        id
    }
}
