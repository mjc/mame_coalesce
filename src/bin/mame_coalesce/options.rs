use camino::Utf8PathBuf;
use clap::{Args, Parser, Subcommand, ValueEnum};
use mame_coalesce::domain::{BuildMode, MatchingPolicy, ZipCompression};

#[derive(Parser)]
#[command(name = "mame_coalesce")]
#[command(about = "Merge MAME ROMs into 1 game 1 zip format")]
pub struct Cli {
    #[arg(
        long,
        env = "MAME_COALESCE_CACHE",
        global = true,
        value_name = "cache-db",
        help_heading = "Cache"
    )]
    cache: Option<Utf8PathBuf>,

    #[command(subcommand)]
    command: Command,
}

impl Cli {
    #[must_use]
    pub const fn cache(&self) -> Option<&Utf8PathBuf> {
        self.cache.as_ref()
    }

    #[must_use]
    pub const fn command(&self) -> &Command {
        &self.command
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Import a DAT, scan sources, and write merged ZIP outputs.
    Build(BuildArgs),
    /// Report catalog coverage from cached or explicitly refreshed source observations.
    Audit(AuditArgs),
    /// Manage the persistent cache explicitly.
    Cache {
        #[command(subcommand)]
        command: CacheCommand,
    },
}

#[derive(Clone, Debug, Args)]
pub struct AuditArgs {
    #[arg(
        value_name = "dat-or-name",
        help = "Imported DAT file path or DAT header name"
    )]
    pub dat: Utf8PathBuf,
    #[arg(
        value_name = "source",
        help = "Source root represented by cached observations"
    )]
    pub source: Utf8PathBuf,
    #[arg(short, long, default_value_t = 0, help = "Refresh scan worker count")]
    pub jobs: usize,
    #[arg(long, help = "Refresh and persist source observations before auditing")]
    pub refresh: bool,
    #[arg(long, value_enum, default_value_t = MatchingPolicyArg::Sha1Compatibility, help = "Evidence matching policy")]
    pub matching_policy: MatchingPolicyArg,
    #[arg(long, value_enum, default_value_t = AuditFormatArg::Human, help = "Audit report format")]
    pub format: AuditFormatArg,
}

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum AuditFormatArg {
    #[default]
    Human,
    Json,
}

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum MatchingPolicyArg {
    #[default]
    Sha1Compatibility,
    EvidenceAware,
}

impl From<MatchingPolicyArg> for MatchingPolicy {
    fn from(policy: MatchingPolicyArg) -> Self {
        match policy {
            MatchingPolicyArg::Sha1Compatibility => Self::Sha1Compatibility,
            MatchingPolicyArg::EvidenceAware => Self::EvidenceAware,
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum CacheCommand {
    /// Import or replace a DAT in the cache.
    Import {
        #[arg(value_name = "dat", help = "Logiqx DAT file to import")]
        dat: Utf8PathBuf,
    },
    /// Refresh cached ROM-file rows for a source root.
    Scan {
        #[arg(value_name = "source", help = "ROM source directory to scan")]
        source: Utf8PathBuf,
        #[arg(short, long, default_value_t = 0, help = "Scan worker count")]
        jobs: usize,
    },
    /// Build from DAT and source rows already present in the cache.
    Build(CacheBuildArgs),
}

#[derive(Clone, Debug, Args)]
pub struct BuildArgs {
    #[arg(value_name = "dat", help = "Logiqx DAT file to import")]
    pub dat: Utf8PathBuf,
    #[arg(value_name = "source", help = "ROM source directory to scan")]
    pub source: Utf8PathBuf,
    #[arg(value_name = "out", help = "Destination directory for output ZIPs")]
    pub out: Utf8PathBuf,
    #[arg(short, long, default_value_t = 0, help = "Scan worker count")]
    pub jobs: usize,
    #[command(flatten)]
    pub options: BuildOptions,
}

#[derive(Clone, Debug, Args)]
pub struct CacheBuildArgs {
    #[arg(
        value_name = "dat-or-name",
        help = "Imported DAT file path or DAT header name"
    )]
    pub dat: Utf8PathBuf,
    #[arg(value_name = "source", help = "Previously scanned source directory")]
    pub source: Utf8PathBuf,
    #[arg(value_name = "out", help = "Destination directory for output ZIPs")]
    pub out: Utf8PathBuf,
    #[command(flatten)]
    pub options: BuildOptions,
}

#[derive(Clone, Debug, Args)]
pub struct BuildOptions {
    #[arg(long, value_enum, default_value_t = LayoutArg::ParentBundles, help = "Output ZIP layout")]
    pub layout: LayoutArg,
    #[arg(long, value_enum, default_value_t = CompressionArg::Deflate, help = "Output ZIP compression")]
    pub compression: CompressionArg,
    #[arg(long, value_enum, default_value_t = MissingArg::Warn, help = "Missing ROM policy")]
    pub missing: MissingArg,
    #[arg(
        long,
        default_value_t = false,
        help = "Plan and report without writing files"
    )]
    pub dry_run: bool,
}

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum LayoutArg {
    #[default]
    ParentBundles,
    PerGame,
}

impl From<LayoutArg> for BuildMode {
    fn from(layout: LayoutArg) -> Self {
        match layout {
            LayoutArg::ParentBundles => Self::ParentBundles,
            LayoutArg::PerGame => Self::PerGame,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum CompressionArg {
    #[default]
    Deflate,
    Store,
}

impl From<CompressionArg> for ZipCompression {
    fn from(compression: CompressionArg) -> Self {
        match compression {
            CompressionArg::Deflate => Self::Deflate,
            CompressionArg::Store => Self::Store,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum MissingArg {
    #[default]
    Warn,
    Fail,
}

impl MissingArg {
    #[must_use]
    pub const fn strict(self) -> bool {
        match self {
            Self::Warn => false,
            Self::Fail => true,
        }
    }
}
