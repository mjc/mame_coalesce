use crate::hashes::Sha1Digest;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocumentKey([u8; 32]);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocumentDigest([u8; 32]);

impl DocumentDigest {
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl DocumentKey {
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self(DocumentDigest::from_bytes(bytes).0)
    }

    #[must_use]
    pub const fn digest(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Display for DocumentKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "sha256:{}", hex::encode(self.0))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PublishingSourceKey(String);

impl PublishingSourceKey {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AcquisitionKey(uuid::Uuid);

impl AcquisitionKey {
    #[must_use]
    pub fn fresh() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl std::fmt::Display for AcquisitionKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CatalogKey(String);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ParserInterpretationKey(String);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SnapshotKey(String);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ImportRunKey(uuid::Uuid);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SetName(String);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetName(String);

impl SetName {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AssetName {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl CatalogKey {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn fresh() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CatalogKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl ParserInterpretationKey {
    #[must_use]
    pub fn logiqx_v1(scope: &CatalogScope) -> Self {
        Self::for_format("logiqx", scope)
    }

    #[must_use]
    pub fn for_format(format: &str, scope: &CatalogScope) -> Self {
        let (scope_kind, scope_details) = scope.as_storage();
        Self(stable_key(
            "parser-interpretation-v1",
            &[
                format,
                env!("CARGO_PKG_VERSION"),
                "normalization-v1",
                scope_kind,
                scope_details.as_deref().unwrap_or_default(),
            ],
        ))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl SnapshotKey {
    #[must_use]
    pub fn new(
        catalog: &CatalogKey,
        document: &DocumentKey,
        interpretation: &ParserInterpretationKey,
    ) -> Self {
        Self(stable_key(
            "catalog-snapshot-v1",
            &[
                catalog.as_str(),
                &document.to_string(),
                interpretation.as_str(),
            ],
        ))
    }

    pub(crate) const fn from_persisted(value: String) -> Self {
        Self(value)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SnapshotKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl ImportRunKey {
    #[must_use]
    pub fn fresh() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl std::fmt::Display for ImportRunKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

fn stable_key(domain: &str, parts: &[&str]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain.as_bytes());
    for part in parts {
        hash.update(part.len().to_string().as_bytes());
        hash.update(b":");
        hash.update(part.as_bytes());
    }
    format!("sha256:{}", hex::encode(hash.finalize()))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CatalogScope {
    Unknown,
    Complete,
    Filtered(serde_json::Value),
    Partial(serde_json::Value),
}

impl CatalogScope {
    #[must_use]
    pub fn as_storage(&self) -> (&'static str, Option<String>) {
        match self {
            Self::Unknown => ("unknown", None),
            Self::Complete => ("complete", None),
            Self::Filtered(details) => ("filtered", Some(details.to_string())),
            Self::Partial(details) => ("partial", Some(details.to_string())),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SetKey {
    catalog: CatalogKey,
    name: SetName,
}

impl SetKey {
    #[must_use]
    pub fn new(catalog: CatalogKey, name: impl Into<String>) -> Self {
        Self {
            catalog,
            name: SetName(name.into()),
        }
    }

    #[must_use]
    pub const fn catalog(&self) -> &CatalogKey {
        &self.catalog
    }

    #[must_use]
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    #[must_use]
    pub const fn set_name(&self) -> &SetName {
        &self.name
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RequirementKey {
    set: SetKey,
    asset: AssetName,
}

impl RequirementKey {
    #[must_use]
    pub fn new(set: SetKey, rom: impl Into<String>) -> Self {
        Self {
            set,
            asset: AssetName(rom.into()),
        }
    }

    #[must_use]
    pub const fn set(&self) -> &SetKey {
        &self.set
    }

    #[must_use]
    pub fn game_name(&self) -> &str {
        self.set.name()
    }

    #[must_use]
    pub fn rom_name(&self) -> &str {
        self.asset.as_str()
    }

    #[must_use]
    pub const fn asset_name(&self) -> &AssetName {
        &self.asset
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Crc32Digest(pub [u8; 4]);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Md5Digest(pub [u8; 16]);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EvidenceScope {
    WholeAsset,
    DiskData,
    Track,
    #[default]
    Unknown,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EvidenceProvenance {
    SourceDeclared,
    Computed,
    LegacyCache,
    #[default]
    Unknown,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExpectedEvidence {
    pub scope: EvidenceScope,
    pub provenance: EvidenceProvenance,
    pub size: Option<u64>,
    pub crc: Option<Crc32Digest>,
    pub md5: Option<Md5Digest>,
    pub sha1: Option<Sha1Digest>,
    pub merge: Option<String>,
    pub dump_status: Option<String>,
    pub serial: Option<String>,
    pub date: Option<String>,
}

impl ExpectedEvidence {
    pub fn from_logiqx(rom: &crate::logiqx::Rom) -> crate::Result<Self> {
        fn digest<const N: usize>(
            value: Option<&[u8]>,
            algorithm: &str,
            name: &str,
        ) -> crate::Result<Option<[u8; N]>> {
            value.map_or(Ok(None), |bytes| {
                let actual = bytes.len();
                bytes.try_into().map(Some).map_err(|_| {
                    crate::Error::InvalidHash(format!(
                        "Logiqx ROM {name} has {actual} {algorithm} bytes; expected {N}"
                    ))
                })
            })
        }

        Ok(Self {
            scope: EvidenceScope::WholeAsset,
            provenance: EvidenceProvenance::SourceDeclared,
            size: rom.size(),
            crc: digest(rom.crc(), "CRC", rom.name())?.map(Crc32Digest),
            md5: digest(rom.md5(), "MD5", rom.name())?.map(Md5Digest),
            sha1: digest(rom.sha1(), "SHA1", rom.name())?,
            merge: rom.merge().map(str::to_owned),
            dump_status: rom.status().map(str::to_owned),
            serial: rom.serial().map(str::to_owned),
            date: rom.date().map(str::to_owned),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ObservedContent {
    pub scope: EvidenceScope,
    pub provenance: EvidenceProvenance,
    pub size: Option<u64>,
    pub crc: Option<Crc32Digest>,
    pub md5: Option<Md5Digest>,
    pub sha1: Option<Sha1Digest>,
    pub xxh3: crate::hashes::Xxh3Digest,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DatRom {
    pub catalog_name: String,
    pub key: RequirementKey,
    pub parent_name: Option<String>,
    pub set_metadata: SetMetadata,
    pub role: AssetRole,
    pub component_order: Option<u32>,
    pub expected: ExpectedEvidence,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct SetMetadata {
    pub source_file: Option<String>,
    pub is_bios: Option<String>,
    pub rom_of: Option<String>,
    pub sample_of: Option<String>,
    pub board: Option<String>,
    pub rebuild_to: Option<String>,
    pub description: Option<String>,
    pub year: Option<String>,
    pub manufacturer: Option<String>,
    pub device_refs: Vec<String>,
}

impl SetMetadata {
    #[must_use]
    pub fn from_logiqx(game: &crate::logiqx::Game) -> Self {
        Self {
            source_file: game.sourcefile_opt().map(str::to_owned),
            is_bios: game.isbios_opt().map(str::to_owned),
            rom_of: game.romof_opt().map(str::to_owned),
            sample_of: game.sampleof_opt().map(str::to_owned),
            board: game.board_opt().map(str::to_owned),
            rebuild_to: game.rebuildto_opt().map(str::to_owned),
            description: game.description_opt().map(str::to_owned),
            year: game.year_opt().map(str::to_owned),
            manufacturer: game.manufacturer_opt().map(str::to_owned),
            device_refs: game.device_refs().map(str::to_owned).collect(),
        }
    }
}

impl DatRom {
    pub fn from_logiqx(
        catalog_key: CatalogKey,
        catalog_name: impl Into<String>,
        game: &crate::logiqx::Game,
        rom: &crate::logiqx::Rom,
        component_order: usize,
    ) -> crate::Result<Self> {
        let component_order = u32::try_from(component_order)
            .map_err(|_| crate::Error::InvalidPath("too many media components".into()))?;
        Ok(Self {
            catalog_name: catalog_name.into(),
            key: RequirementKey::new(SetKey::new(catalog_key, game.name()), rom.name()),
            parent_name: game.cloneof().map(str::to_owned),
            set_metadata: SetMetadata::from_logiqx(game),
            role: AssetRole::Rom,
            component_order: Some(component_order),
            expected: ExpectedEvidence::from_logiqx(rom)?,
        })
    }
}

impl DatRom {
    #[must_use]
    pub fn catalog_name(&self) -> &str {
        &self.catalog_name
    }

    #[must_use]
    pub fn game_name(&self) -> &str {
        self.key.game_name()
    }

    #[must_use]
    pub fn rom_name(&self) -> &str {
        self.key.rom_name()
    }

    #[must_use]
    pub const fn sha1(&self) -> Option<&Sha1Digest> {
        self.expected.sha1.as_ref()
    }

    #[must_use]
    pub fn bundle_name(&self, mode: BuildMode) -> &str {
        match mode {
            BuildMode::ParentBundles => self
                .parent_name
                .as_deref()
                .unwrap_or_else(|| self.key.game_name()),
            BuildMode::PerGame => self.key.game_name(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AssetRole {
    #[default]
    Rom,
    Disk,
    Other,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BuildMode {
    #[default]
    ParentBundles,
    PerGame,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MatchingPolicy {
    /// Match only SHA1, ignoring size/CRC/MD5 disagreements for historical compatibility.
    #[default]
    Sha1Compatibility,
    /// Compare mutually available evidence, veto weaker fallback on stronger contradictions,
    /// and retain lower-priority disagreements as explanations after a stronger match.
    EvidenceAware,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ZipCompression {
    #[default]
    Deflate,
    Store,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArchiveBackend {
    Zip,
    SevenZip,
    Rar,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceRoot(String);

impl SourceRoot {
    #[must_use]
    pub fn new(canonical_path: impl Into<String>) -> Self {
        Self(canonical_path.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScanRunKey(uuid::Uuid);

impl ScanRunKey {
    #[must_use]
    pub fn fresh() -> Self {
        Self(uuid::Uuid::new_v4())
    }

    #[must_use]
    pub fn to_storage_key(self) -> String {
        self.0.to_string()
    }

    #[must_use]
    pub fn from_storage_key(value: &str) -> Option<Self> {
        uuid::Uuid::parse_str(value).ok().map(Self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceFingerprint(Sha1Digest);

impl SourceFingerprint {
    #[must_use]
    pub const fn new(digest: Sha1Digest) -> Self {
        Self(digest)
    }

    #[must_use]
    pub const fn digest(self) -> Sha1Digest {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScanProvenance {
    StreamedSha1Xxh3V1,
}

impl ScanProvenance {
    #[must_use]
    pub const fn storage_key(self) -> &'static str {
        match self {
            Self::StreamedSha1Xxh3V1 => "streamed_sha1_xxh3_v1",
        }
    }

    #[must_use]
    pub fn from_storage_key(value: &str) -> Option<Self> {
        match value {
            "streamed_sha1_xxh3_v1" => Some(Self::StreamedSha1Xxh3V1),
            _ => None,
        }
    }
}

impl ArchiveBackend {
    #[must_use]
    pub fn from_storage_key(value: &str) -> Option<Self> {
        match value {
            "zip" => Some(Self::Zip),
            "7z" => Some(Self::SevenZip),
            "rar" => Some(Self::Rar),
            _ => None,
        }
    }

    #[must_use]
    pub const fn storage_key(self) -> &'static str {
        match self {
            Self::Zip => "zip",
            Self::SevenZip => "7z",
            Self::Rar => "rar",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArchiveMemberSelector {
    IndexAndName { index: u64, name: String },
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SourceLocation {
    BareFile {
        path: String,
    },
    ArchiveMember {
        path: String,
        backend: ArchiveBackend,
        selector: ArchiveMemberSelector,
    },
    LegacyUnknown {
        path: String,
        member_name: Option<String>,
    },
}

impl SourceLocation {
    #[must_use]
    pub fn path(&self) -> &str {
        match self {
            Self::BareFile { path }
            | Self::ArchiveMember { path, .. }
            | Self::LegacyUnknown { path, .. } => path,
        }
    }

    #[must_use]
    pub const fn priority(&self) -> u8 {
        match self {
            Self::BareFile { .. } => 0,
            Self::ArchiveMember {
                backend: ArchiveBackend::Zip,
                ..
            } => 1,
            Self::ArchiveMember {
                backend: ArchiveBackend::SevenZip,
                ..
            } => 2,
            Self::ArchiveMember {
                backend: ArchiveBackend::Rar,
                ..
            } => 3,
            Self::LegacyUnknown { .. } => u8::MAX,
        }
    }

    #[must_use]
    pub fn member_name(&self) -> Option<&str> {
        match self {
            Self::ArchiveMember {
                selector: ArchiveMemberSelector::IndexAndName { name, .. },
                ..
            } => Some(name),
            Self::LegacyUnknown { member_name, .. } => member_name.as_deref(),
            Self::BareFile { .. } => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFile {
    pub source_root: SourceRoot,
    pub location: SourceLocation,
    pub observed: ObservedContent,
    pub fingerprint: Option<SourceFingerprint>,
    pub scan_run: Option<ScanRunKey>,
    pub scan_provenance: Option<ScanProvenance>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceObservation {
    pub source_root: SourceRoot,
    pub scan_run: ScanRunKey,
    pub location: SourceLocation,
    pub observed: ObservedContent,
    pub fingerprint: SourceFingerprint,
    pub scan_provenance: ScanProvenance,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompleteSourceScan {
    source_root: SourceRoot,
    scan_run: ScanRunKey,
    observations: Vec<SourceObservation>,
}

impl CompleteSourceScan {
    pub(crate) fn new(
        source_root: SourceRoot,
        scan_run: ScanRunKey,
        observations: Vec<SourceObservation>,
    ) -> crate::Result<Self> {
        if observations.iter().any(|observation| {
            observation.source_root != source_root || observation.scan_run != scan_run
        }) {
            return Err(crate::Error::InvalidPath(
                "completed source scan contains observations from another root or run".to_owned(),
            ));
        }
        Ok(Self {
            source_root,
            scan_run,
            observations,
        })
    }

    #[must_use]
    pub const fn source_root(&self) -> &SourceRoot {
        &self.source_root
    }

    #[must_use]
    pub const fn scan_run(&self) -> ScanRunKey {
        self.scan_run
    }

    #[must_use]
    pub fn observations(&self) -> &[SourceObservation] {
        &self.observations
    }
}

impl SourceFile {
    #[must_use]
    pub fn display_name(&self) -> String {
        self.location.member_name().map_or_else(
            || self.location.path().to_owned(),
            |entry| format!("{}:{entry}", self.location.path()),
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildRequest {
    pub dat_name: String,
    pub source_root: SourceRoot,
    pub mode: BuildMode,
    pub matching_policy: MatchingPolicy,
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
    pub sha1: Option<Sha1Digest>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DuplicateMatch {
    pub rom_name: String,
    pub selected: SourceFile,
    pub candidates: Vec<SourceFile>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_conversion_keeps_partial_evidence_scope_metadata_and_component_order()
    -> crate::Result<()> {
        let xml = r#"<datafile><header><name>Display name</name></header>
<game name="same" cloneof="parent" sourcefile="driver.c" isbios="yes" romof="bios-set">
  <description>Machine</description><year>1984</year><manufacturer>Maker</manufacturer>
  <device_ref name="sound"/>
  <rom name="first.bin" size="4294967296" crc="12345678" md5="00000000000000000000000000000000" sha1="1111111111111111111111111111111111111111" merge="parent.bin" status="nodump" serial="serial" date="2024"/>
  <rom name="second.bin"/>
</game></datafile>"#;
        let parsed = crate::logiqx::DataFile::from_reader(xml.as_bytes())?;
        let game = parsed
            .games()
            .first()
            .ok_or_else(|| crate::Error::InvalidPath("test fixture has no game".to_owned()))?;
        let first_rom = game
            .roms()
            .first()
            .ok_or_else(|| crate::Error::InvalidPath("test fixture has no first ROM".to_owned()))?;
        let second_rom = game.roms().get(1).ok_or_else(|| {
            crate::Error::InvalidPath("test fixture has no second ROM".to_owned())
        })?;
        let catalog_a = CatalogKey::fresh();
        let catalog_b = CatalogKey::fresh();
        let first = DatRom::from_logiqx(catalog_a.clone(), "Display name", game, first_rom, 0)?;
        let second = DatRom::from_logiqx(catalog_a, "Display name", game, second_rom, 1)?;
        let same_names_other_catalog =
            DatRom::from_logiqx(catalog_b, "Display name", game, first_rom, 0)?;

        assert_ne!(first.key, same_names_other_catalog.key);
        assert_eq!(
            first.key.set().name(),
            same_names_other_catalog.key.set().name()
        );
        assert_eq!(
            first.key.rom_name(),
            same_names_other_catalog.key.rom_name()
        );
        assert_eq!(first.role, AssetRole::Rom);
        assert_eq!(first.component_order, Some(0));
        assert_eq!(second.component_order, Some(1));
        assert_eq!(first.expected.size, Some(4_294_967_296));
        assert_eq!(first.expected.scope, EvidenceScope::WholeAsset);
        assert_eq!(
            first.expected.provenance,
            EvidenceProvenance::SourceDeclared
        );
        assert_eq!(
            first.expected.crc,
            Some(Crc32Digest([0x12, 0x34, 0x56, 0x78]))
        );
        assert_eq!(first.expected.md5, Some(Md5Digest([0; 16])));
        assert_eq!(first.expected.sha1, Some([0x11; 20]));
        let observed = ObservedContent {
            scope: EvidenceScope::WholeAsset,
            provenance: EvidenceProvenance::Computed,
            size: Some(4_294_967_296),
            crc: first.expected.crc,
            md5: first.expected.md5,
            sha1: Some([0x22; 20]),
            xxh3: [0x33; 8],
        };
        assert_eq!(first.expected.sha1, Some([0x11; 20]));
        assert_eq!(observed.sha1, Some([0x22; 20]));
        assert_ne!(first.expected.sha1, observed.sha1);
        assert_eq!(first.expected.merge.as_deref(), Some("parent.bin"));
        assert_eq!(first.expected.dump_status.as_deref(), Some("nodump"));
        assert_eq!(first.parent_name.as_deref(), Some("parent"));
        assert_eq!(first.set_metadata.source_file.as_deref(), Some("driver.c"));
        assert_eq!(first.set_metadata.is_bios.as_deref(), Some("yes"));
        assert_eq!(first.set_metadata.rom_of.as_deref(), Some("bios-set"));
        assert_eq!(first.set_metadata.device_refs, ["sound"]);
        assert_eq!(second.expected.size, None);
        assert_eq!(second.expected.sha1, None);
        Ok(())
    }
}
