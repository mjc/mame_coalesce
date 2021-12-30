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

#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

use diesel::{prelude::*, SqliteConnection};
use dotenv::dotenv;

use std::path::{Path, PathBuf};
use std::{env, fs};
use structopt::StructOpt;

pub mod logiqx;

pub mod models;
pub mod queries;
pub mod schema;

use queries::traverse_and_insert_data_file;

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

fn main() {
    dotenv().ok();
    pretty_env_logger::init();
    let opt = Opt::from_args();

    let destination = match opt.destination {
        None => Opt::default_destination(&opt.path),
        Some(x) => x,
    };

    fs::create_dir_all(&destination).expect("Couldn't create destination directory");

    println!("Using datafile: {}", opt.datafile);
    println!("Looking in path: {}", opt.path.to_str().unwrap());
    println!("Saving zips to path: {}", destination.to_str().unwrap());

    let data_file = logiqx::load_datafile(&opt.datafile).expect("Couldn't load datafile");

    let conn = establish_connection();

    let file_name = Path::new(&opt.datafile)
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();

    traverse_and_insert_data_file(conn, data_file, file_name);
}

embed_migrations!("migrations");
pub fn establish_connection() -> SqliteConnection {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let connection = SqliteConnection::establish(&database_url)
        .expect(&format!("Error connecting to {}", database_url));
    let _migration_result = embedded_migrations::run(&connection);
    connection
}
