use camino::Utf8PathBuf;
use clap::{Args, Parser, Subcommand, ValueEnum};
use mame_coalesce::domain::{
    BuildMode, MatchingPolicy, OutputContainer, SetName, SetSelection, ZipCompression,
};

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
    #[arg(
        long = "source-root",
        value_name = "DIR",
        help = "Additional ordered source root (repeatable)"
    )]
    pub additional_source_roots: Vec<Utf8PathBuf>,
    #[arg(short, long, default_value_t = 0, help = "Refresh scan worker count")]
    pub jobs: usize,
    #[arg(long, help = "Refresh and persist source observations before auditing")]
    pub refresh: bool,
    #[arg(long, value_enum, default_value_t = MatchingPolicyArg::Sha1Compatibility, help = "Evidence matching policy")]
    pub matching_policy: MatchingPolicyArg,
    #[arg(long, value_enum, default_value_t = AuditFormatArg::Human, help = "Audit report format")]
    pub format: AuditFormatArg,
    #[arg(
        long = "set",
        value_name = "NAME",
        help = "Select this exact set name (repeatable)"
    )]
    pub set_names: Vec<String>,
}

impl AuditArgs {
    pub fn set_selection(&self) -> SetSelection {
        set_selection(&self.set_names)
    }
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
    Scan(CacheScanArgs),
    /// Build from DAT and source rows already present in the cache.
    Build(CacheBuildArgs),
}

#[derive(Clone, Debug, Args)]
pub struct BuildArgs {
    #[arg(value_name = "dat", help = "Logiqx DAT file to import")]
    pub dat: Utf8PathBuf,
    #[arg(value_name = "source", help = "ROM source directory to scan")]
    pub source: Utf8PathBuf,
    #[arg(
        long = "source-root",
        value_name = "DIR",
        help = "Additional ordered source root (repeatable)"
    )]
    pub additional_source_roots: Vec<Utf8PathBuf>,
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
    #[arg(
        long = "source-root",
        value_name = "DIR",
        help = "Additional ordered source root (repeatable)"
    )]
    pub additional_source_roots: Vec<Utf8PathBuf>,
    #[arg(value_name = "out", help = "Destination directory for output ZIPs")]
    pub out: Utf8PathBuf,
    #[command(flatten)]
    pub options: BuildOptions,
}

#[derive(Clone, Debug, Args)]
pub struct CacheScanArgs {
    #[arg(value_name = "source", help = "ROM source directory to scan")]
    pub source: Utf8PathBuf,
    #[arg(
        long = "source-root",
        value_name = "DIR",
        help = "Additional ordered source root (repeatable)"
    )]
    pub additional_source_roots: Vec<Utf8PathBuf>,
    #[arg(short, long, default_value_t = 0, help = "Scan worker count")]
    pub jobs: usize,
    #[arg(long, help = "Reuse unchanged bare-file observations from this cache")]
    pub reuse_unchanged: bool,
    #[arg(
        long,
        value_name = "FILE",
        requires = "reuse_unchanged",
        help = "Force rehash of this file even when reuse metadata matches (repeatable)"
    )]
    pub force_rehash: Vec<Utf8PathBuf>,
}

#[derive(Clone, Debug, Args)]
pub struct BuildOptions {
    #[arg(long, value_enum, default_value_t = OutputContainerArg::Zip, help = "Output container format")]
    pub output_container: OutputContainerArg,
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
    #[arg(
        long = "set",
        value_name = "NAME",
        help = "Select this exact set name (repeatable)"
    )]
    pub set_names: Vec<String>,
}

impl BuildOptions {
    pub fn set_selection(&self) -> SetSelection {
        set_selection(&self.set_names)
    }
}

fn set_selection(names: &[String]) -> SetSelection {
    if names.is_empty() {
        SetSelection::All
    } else {
        SetSelection::exact_names(names.iter().cloned().map(SetName::new))
    }
}

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum OutputContainerArg {
    #[default]
    Zip,
    Directory,
}

impl From<OutputContainerArg> for OutputContainer {
    fn from(container: OutputContainerArg) -> Self {
        match container {
            OutputContainerArg::Zip => Self::Zip,
            OutputContainerArg::Directory => Self::Directory,
        }
    }
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
