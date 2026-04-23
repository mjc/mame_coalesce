use crate::hashes::Sha1Digest;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DatRom {
    pub dat_name: String,
    pub game_name: String,
    pub parent_name: Option<String>,
    pub rom_name: String,
    pub sha1: Sha1Digest,
}

impl DatRom {
    #[must_use]
    pub fn bundle_name(&self, mode: BuildMode) -> &str {
        match mode {
            BuildMode::ParentBundles => self.parent_name.as_deref().unwrap_or(&self.game_name),
            BuildMode::PerGame => &self.game_name,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BuildMode {
    #[default]
    ParentBundles,
    PerGame,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ZipCompression {
    #[default]
    Deflate,
    Store,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    BareFile,
    ZipEntry,
    ArchiveEntry,
}

impl SourceKind {
    #[must_use]
    pub const fn priority(self) -> u8 {
        match self {
            Self::BareFile => 0,
            Self::ZipEntry => 1,
            Self::ArchiveEntry => 2,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFile {
    pub source_root: String,
    pub canonical_path: String,
    pub entry_name: Option<String>,
    pub sha1: Sha1Digest,
    pub kind: SourceKind,
}

impl SourceFile {
    #[must_use]
    pub fn display_name(&self) -> String {
        self.entry_name.as_ref().map_or_else(
            || self.canonical_path.clone(),
            |entry| format!("{}:{entry}", self.canonical_path),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildRequest {
    pub dat_name: String,
    pub source_root: String,
    pub mode: BuildMode,
    pub dry_run: bool,
    pub strict: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildPlan {
    pub zips: Vec<ZipSpec>,
    pub report: BuildReport,
    pub dry_run: bool,
}

impl BuildPlan {
    #[must_use]
    pub const fn writes_files(&self) -> bool {
        !self.dry_run && !self.zips.is_empty() && self.report.exit_code == 0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZipSpec {
    pub file_name: String,
    pub entries: Vec<ZipEntrySpec>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZipEntrySpec {
    pub output_name: String,
    pub source: SourceFile,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BuildReport {
    pub missing_roms: Vec<MissingRom>,
    pub duplicate_matches: Vec<DuplicateMatch>,
    pub matched_roms: usize,
    pub exit_code: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissingRom {
    pub game_name: String,
    pub rom_name: String,
    pub sha1: Sha1Digest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DuplicateMatch {
    pub rom_name: String,
    pub selected: SourceFile,
    pub candidates: Vec<SourceFile>,
}
