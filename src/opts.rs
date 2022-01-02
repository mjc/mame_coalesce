use std::path::PathBuf;
pub use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "mame_coalesce",
    about = "A commandline app for merging ROMs for emulators like mame."
)]
pub(crate) struct Opt {
    pub(crate) datafile: String,
    #[structopt(parse(from_os_str))]
    pub(crate) path: PathBuf,
    #[structopt(parse(from_os_str))]
    pub(crate) destination: Option<PathBuf>,
}
impl Opt {
    pub fn default_destination(path: &PathBuf) -> PathBuf {
        [path.to_str().expect("Path is fucked somehow"), "merged"]
            .iter()
            .collect()
    }
}