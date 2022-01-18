use camino::Utf8PathBuf;
use clap::{AppSettings, Parser, Subcommand};

#[derive(Parser)]
#[clap(name = "mame_coalesce")]
#[clap(about = "A tool to merge your mame roms into 1 game 1 zip format")]
pub(crate) struct Cli {
    #[clap(subcommand)]
    pub(crate) command: Command,
    #[clap(short, long, default_value = "coalesce.db")]
    pub(crate) database_path: String,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    #[clap(setting(AppSettings::ArgRequiredElseHelp))]
    AddDataFile { path: Utf8PathBuf },
    ScanSource {
        #[clap(short, long, parse(try_from_str), default_value_t = true)]
        parallel: bool,
        path: Utf8PathBuf,
    },
    Rename {
        #[clap(short, long, parse(try_from_str), default_value_t = false)]
        dry_run: bool,
        data_file: Utf8PathBuf,
        source: Utf8PathBuf,
        destination: Utf8PathBuf,
    },
}
