use camino::Utf8PathBuf;
use clap::{AppSettings, Parser, Subcommand};

#[derive(Parser)]
#[clap(name = "mame_coalesce")]
#[clap(about = "A tool to merge your mame roms into 1 game 1 zip format")]
pub struct Cli {
    #[clap(subcommand)]
    command: Command,
    #[clap(short, long, default_value = "coalesce.db")]
    database_path: String,
}

impl Cli {
    /// Get a reference to the cli's database path.
    pub fn database_path(&self) -> &str {
        self.database_path.as_ref()
    }

    /// Get a reference to the cli's command.
    pub const fn command(&self) -> &Command {
        &self.command
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[clap(setting(AppSettings::ArgRequiredElseHelp))]
    AddDataFile { path: Utf8PathBuf },
    ScanSource {
        #[clap(short, long, parse(try_from_str), default_value_t = 0)]
        jobs: usize,
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
