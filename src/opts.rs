use std::path::PathBuf;
use clap::Parser;


#[derive(Debug, Parser)]
pub(crate) struct Opt {
    pub(crate) datafile: String,
    #[clap(parse(from_os_str))]
    pub(crate) path: PathBuf,
    #[clap(parse(from_os_str))]
    pub(crate) destination: PathBuf,
}
