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

use std::fs;
use std::path::PathBuf;
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

impl Opt {
    pub fn default_destination(path: &PathBuf) -> PathBuf {
        [path.to_str().expect("Path is fucked somehow"), "merged"]
            .iter()
            .collect()
    }
}
fn main() {
    let opt = Opt::from_args();

    let destination = match opt.destination {
        None => Opt::default_destination(&opt.path),
        Some(x) => x,
    };

    fs::create_dir_all(&destination).expect("Couldn't create destination directory");

    println!("Using datafile: {}", opt.datafile);
    println!("Looking in path: {}", opt.path.to_str().unwrap());
    println!("Saving zips to path: {}", destination.to_str().unwrap());

    let datafile = logiqx::load_datafile(opt.datafile).expect("Couldn't load datafile");
    let files = rom::files(opt.path);

    println!(
        "sha1 of last file: {:?}",
        files
            .last()
            .expect("Somehow there are no files")
            .sha1
            .as_ref()
            .unwrap()
    );

    let bundles = rom::Bundle::from_datafile(&datafile, &files);

    rom::zip::write_all_zip(bundles, &destination);
}
