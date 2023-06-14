#![deny(elided_lifetimes_in_paths, clippy::all)]
#![warn(
    clippy::all,
    clippy::nursery,
    clippy::decimal_literal_representation,
    clippy::expect_used,
    clippy::filetype_is_file,
    clippy::str_to_string,
    clippy::string_to_string,
    clippy::unneeded_field_pattern,
    clippy::unwrap_used
)]

extern crate indicatif;
extern crate rayon;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_xml_rs;
extern crate sha1;

extern crate walkdir;
extern crate zip;

#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

use log::warn;

use std::{error, result::Result};

pub mod db;
pub mod hashes;
pub mod logger;
pub mod logiqx;
pub mod models;
pub mod operations;
pub mod progress;
pub mod schema;

type MameResult<T> = Result<T, Box<dyn error::Error>>;
