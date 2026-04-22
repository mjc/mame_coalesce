use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "mame_coalesce")]
#[command(about = "A tool to merge your mame roms into 1 game 1 zip format")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
    #[arg(short, long, default_value = "coalesce.db")]
    database_path: String,
}

impl Cli {
    pub fn database_path(&self) -> &str {
        self.database_path.as_ref()
    }

    pub const fn command(&self) -> &Command {
        &self.command
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(arg_required_else_help = true)]
    AddDataFile { path: Utf8PathBuf },
    ScanSource {
        #[arg(short, long, default_value_t = 0)]
        jobs: usize,
        path: Utf8PathBuf,
    },
    Rename {
        #[arg(short, long, default_value_t = false)]
        dry_run: bool,
        data_file: Utf8PathBuf,
        source: Utf8PathBuf,
        destination: Utf8PathBuf,
    },
}
