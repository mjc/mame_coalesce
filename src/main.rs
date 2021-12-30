extern crate indicatif;
extern crate rayon;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_xml_rs;
extern crate sha1;

extern crate structopt;

extern crate walkdir;
extern crate zip;

use async_std::stream;
use async_std::stream::StreamExt;
use log::LevelFilter;
use sqlx::{sqlite::SqliteConnectOptions, ConnectOptions, Pool, Sqlite, SqlitePool};
use std::path::PathBuf;
use std::{env, fs};
use structopt::StructOpt;

mod logiqx;
mod queries;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "mame_coalesce",
    about = "A commandline app for merging ROMs for emulators like mame."
)]
struct Opt {
    datafile: String,
    #[structopt(parse(from_os_str))]
    path: PathBuf,
    #[structopt(parse(from_os_str))]
    destination: Option<PathBuf>,
}

impl Opt {
    pub fn default_destination(path: &PathBuf) -> PathBuf {
        [path.to_str().expect("Path is fucked somehow"), "merged"]
            .iter()
            .collect()
    }
}

async fn run_migrations(pool: &Pool<Sqlite>) {
    sqlx::migrate!().run(pool).await.unwrap();
}

#[async_std::main]
async fn main() {
    pretty_env_logger::init();
    let opt = Opt::from_args();

    let destination = match opt.destination {
        None => Opt::default_destination(&opt.path),
        Some(x) => x,
    };

    fs::create_dir_all(&destination).expect("Couldn't create destination directory");

    let _db_url = match env::var("DATABASE_URL") {
        Ok(url) => url,
        Err(_) => "sqlite://coalesce.db".to_string(),
    };

    // TODO: use env var
    let connect_options = SqliteConnectOptions::new()
        .filename("coalesce.db")
        .log_statements(LevelFilter::Debug)
        .to_owned();
    let pool = SqlitePool::connect_with(connect_options).await.unwrap();
    run_migrations(&pool).await;

    println!("Using datafile: {}", opt.datafile);
    println!("Looking in path: {}", opt.path.to_str().unwrap());
    println!("Saving zips to path: {}", destination.to_str().unwrap());

    let data_file = logiqx::load_datafile(&opt.datafile).expect("Couldn't load datafile");
    upsert_entire_dat_file(pool, &data_file, &opt.datafile).await;
}

async fn upsert_entire_dat_file(pool: SqlitePool, data_file: &logiqx::DataFile, path: &str) {
    let data_file_id = queries::upsert_data_file(&pool, &data_file, path).await;

    for game in data_file.games().iter() {
        println!("id: {:?}", &data_file_id);
        queries::upsert_game(&pool, &game, &data_file_id).await;
    }
}
