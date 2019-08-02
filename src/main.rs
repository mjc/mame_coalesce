extern crate rayon;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_xml_rs;
extern crate sha1;

extern crate structopt;

extern crate walkdir;
extern crate zip;

use std::path::PathBuf;
use std::fs;
use structopt::StructOpt;

mod logiqx;
mod rom;

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

fn main() {
    let opt = Opt::from_args();
    println!("{:?}", opt);
    let default_destination: PathBuf =
        [opt.path.to_str().expect("Path is fucked somehow"), "merged"]
            .iter()
            .collect();
    let destination = match opt.destination {
        Some(x) => x,
        None => default_destination,
    };
    fs::create_dir_all(&destination).expect("Couldn't create destination directory");
    println!("Using datafile: {}", opt.datafile);
    println!("Looking in path: {}", opt.path.to_str().unwrap());

    let data = logiqx::load_datafile(opt.datafile).expect("Couldn't load datafile");
    let mut bundles = rom::Bundle::from_datafile(&data);

    let mut files = rom::list_files(opt.path);
    println!("Files to check: {}", files.len());

    rom::compute_all_sha1(&mut files);

    let files_by_sha1 = rom::files_by_sha1(&files);

    println!(
        "sha1 of last file: {:?}",
        files.last().unwrap().sha1.as_ref().unwrap()
    );

    rom::add_matches_to_bundles(&mut bundles, &files_by_sha1);

    rom::zip::write_all_zip(bundles, &destination);
}
