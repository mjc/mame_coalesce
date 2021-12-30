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

use sea_orm::{Database, DatabaseConnection};
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::{env, fs};
use structopt::StructOpt;

mod logiqx;
mod rom;

mod entities;

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

async fn run_migrations() {
    let pool = SqlitePool::connect("mssql://").await.unwrap();
    sqlx::migrate!().run(&pool).await.unwrap()
}

#[async_std::main]
async fn main() {
    let opt = Opt::from_args();

    let destination = match opt.destination {
        None => Opt::default_destination(&opt.path),
        Some(x) => x,
    };

    fs::create_dir_all(&destination).expect("Couldn't create destination directory");

    let db_url = match env::var("DATABASE_URL") {
        Ok(url) => url,
        Err(_) => "sqlite://coalesce.db".to_string(),
    };

    run_migrations().await;
    let db: DatabaseConnection = Database::connect(db_url).await.unwrap();

    println!("Using datafile: {}", opt.datafile);
    println!("Looking in path: {}", opt.path.to_str().unwrap());
    println!("Saving zips to path: {}", destination.to_str().unwrap());
}
