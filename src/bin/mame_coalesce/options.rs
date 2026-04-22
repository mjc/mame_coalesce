use camino::Utf8PathBuf;
use clap::{Args, Parser, Subcommand, ValueEnum};
use mame_coalesce::domain::BuildMode;

#[derive(Parser)]
#[command(name = "mame_coalesce")]
#[command(about = "Merge MAME ROMs into 1 game 1 zip format")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
    #[arg(short, long, default_value = "coalesce.db")]
    database_path: String,
}

impl Cli {
    #[must_use]
    pub fn database_path(&self) -> &str {
        self.database_path.as_ref()
    }

    #[must_use]
    pub const fn command(&self) -> &Command {
        &self.command
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Dat {
        #[command(subcommand)]
        command: DatCommand,
    },
    Source {
        #[command(subcommand)]
        command: SourceCommand,
    },
    Build(BuildArgs),
    Run(RunArgs),
}

#[derive(Debug, Subcommand)]
pub enum DatCommand {
    Import {
        #[arg(value_name = "dat-path")]
        dat_path: Utf8PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub enum SourceCommand {
    Scan {
        #[arg(value_name = "source-path")]
        source_path: Utf8PathBuf,
        #[arg(short, long, default_value_t = 0)]
        jobs: usize,
    },
}

#[derive(Clone, Debug, Args)]
pub struct BuildArgs {
    #[arg(long, value_name = "dat-path-or-name")]
    pub dat: Utf8PathBuf,
    #[arg(long, value_name = "source-path")]
    pub source: Utf8PathBuf,
    #[arg(long, value_name = "destination")]
    pub out: Utf8PathBuf,
    #[arg(long, value_enum, default_value_t = ModeArg::ParentBundles)]
    pub mode: ModeArg,
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
    #[arg(long, default_value_t = false)]
    pub strict: bool,
}

#[derive(Clone, Debug, Args)]
pub struct RunArgs {
    #[arg(long, value_name = "dat-path")]
    pub dat: Utf8PathBuf,
    #[arg(long, value_name = "source-path")]
    pub source: Utf8PathBuf,
    #[arg(long, value_name = "destination")]
    pub out: Utf8PathBuf,
    #[arg(long, value_enum, default_value_t = ModeArg::ParentBundles)]
    pub mode: ModeArg,
    #[arg(short, long, default_value_t = 0)]
    pub jobs: usize,
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
    #[arg(long, default_value_t = false)]
    pub strict: bool,
}

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum ModeArg {
    #[default]
    ParentBundles,
    PerGame,
}

impl From<ModeArg> for BuildMode {
    fn from(mode: ModeArg) -> Self {
        match mode {
            ModeArg::ParentBundles => Self::ParentBundles,
            ModeArg::PerGame => Self::PerGame,
        }
    }
}
