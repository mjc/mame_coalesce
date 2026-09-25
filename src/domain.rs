use crate::hashes::Sha1Digest;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

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

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CatalogKey(String);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ParserInterpretationKey(String);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SnapshotKey(String);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ImportRunKey(uuid::Uuid);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SetName(String);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AssetName(String);

impl SetName {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotRecordStatus {
    AddedWithinScope,
    RemovedWithinScope,
    Changed,
    Unchanged,
    Unknown,
    OutOfScope,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotRequirementChange {
    pub asset_name: String,
    pub previous: Option<serde_json::Value>,
    pub current: Option<serde_json::Value>,
    pub size_changed: bool,
    pub hash_changed: bool,
    pub other_evidence_changed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotRecordDiff {
    pub set_name: String,
    pub status: SnapshotRecordStatus,
    pub metadata_changed: bool,
    pub regrouped: bool,
    pub requirement_changes: Vec<SnapshotRequirementChange>,
    pub relationship_evidence: Vec<RelationshipExplanation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogSnapshotDiff {
    pub previous: SnapshotKey,
    pub current: SnapshotKey,
    pub same_scope: bool,
    pub records: Vec<SnapshotRecordDiff>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogSnapshotEntry {
    pub snapshot: SnapshotKey,
    pub document_key: String,
    pub declared_version: Option<String>,
    pub scope: CatalogScope,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RequirementKey {
    set: SetKey,
    asset: AssetName,
}

/// Select all sets or an exact set-name subset within the already selected catalog.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SetSelection {
    #[default]
    All,
    ExactNames(BTreeSet<SetName>),
}

impl SetSelection {
    #[must_use]
    pub fn exact_names(names: impl IntoIterator<Item = SetName>) -> Self {
        Self::ExactNames(names.into_iter().collect())
    }

    #[must_use]
    pub fn includes(&self, set: &SetKey) -> bool {
        match self {
            Self::All => true,
            Self::ExactNames(names) => names.contains(set.set_name()),
        }
    }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Crc32Digest(pub [u8; 4]);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Md5Digest(pub [u8; 16]);

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum EvidenceScope {
    WholeAsset,
    DiskData,
    Track,
    #[default]
    Unknown,
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum EvidenceProvenance {
    SourceDeclared,
    Computed,
    StatValidatedCache,
    LegacyCache,
    #[default]
    Unknown,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipType {
    ExactContentIdentity,
    RevisionOf,
    DumpOfIntendedRelease,
    AlternateRepresentationOf,
    SourceParentClone,
    RuntimeDependency,
    CatalogCorrection,
    CatalogContinuity,
}

impl RelationshipType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExactContentIdentity => "exact_content_identity",
            Self::RevisionOf => "revision_of",
            Self::DumpOfIntendedRelease => "dump_of_intended_release",
            Self::AlternateRepresentationOf => "alternate_representation_of",
            Self::SourceParentClone => "source_parent_clone",
            Self::RuntimeDependency => "runtime_dependency",
            Self::CatalogCorrection => "catalog_correction",
            Self::CatalogContinuity => "catalog_continuity",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogRecordKind {
    Set,
    AssetRequirement,
    SoftwareItem,
}

impl CatalogRecordKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Set => "catalog_set",
            Self::AssetRequirement => "asset_requirement",
            Self::SoftwareItem => "software_item",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RelationshipRecordKey(String);

impl RelationshipRecordKey {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CatalogRecordRef {
    pub snapshot: SnapshotKey,
    pub kind: CatalogRecordKind,
    pub key: RelationshipRecordKey,
}

impl CatalogRecordRef {
    #[must_use]
    pub fn new(snapshot: SnapshotKey, kind: CatalogRecordKind, key: impl Into<String>) -> Self {
        Self {
            snapshot,
            kind,
            key: RelationshipRecordKey::new(key),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentDigestAlgorithm {
    Crc32,
    Md5,
    Sha1,
    Sha256,
}

impl ContentDigestAlgorithm {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Crc32 => "crc32",
            Self::Md5 => "md5",
            Self::Sha1 => "sha1",
            Self::Sha256 => "sha256",
        }
    }

    const fn hex_length(self) -> usize {
        match self {
            Self::Crc32 => 8,
            Self::Md5 => 32,
            Self::Sha1 => 40,
            Self::Sha256 => 64,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct ContentIdentity {
    algorithm: ContentDigestAlgorithm,
    digest: String,
}

impl ContentIdentity {
    pub fn new(
        algorithm: ContentDigestAlgorithm,
        digest: impl Into<String>,
    ) -> crate::Result<Self> {
        let digest = digest.into();
        if digest.len() != algorithm.hex_length()
            || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(crate::Error::InvalidHash(format!(
                "{} digest must contain {} hexadecimal characters",
                algorithm.as_str(),
                algorithm.hex_length()
            )));
        }
        Ok(Self {
            algorithm,
            digest: digest.to_ascii_lowercase(),
        })
    }

    #[must_use]
    pub const fn algorithm(&self) -> ContentDigestAlgorithm {
        self.algorithm
    }

    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

impl<'de> Deserialize<'de> for ContentIdentity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct SerializedContentIdentity {
            algorithm: ContentDigestAlgorithm,
            digest: String,
        }

        let value = SerializedContentIdentity::deserialize(deserializer)?;
        Self::new(value.algorithm, value.digest).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ExternalRecordRef {
    pub namespace: String,
    pub key: RelationshipRecordKey,
}

impl ExternalRecordRef {
    #[must_use]
    pub fn new(namespace: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            key: RelationshipRecordKey::new(key),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum RelationshipEndpoint {
    CatalogRecord(CatalogRecordRef),
    ContentObject(ContentIdentity),
    ExternalRecord(ExternalRecordRef),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipReviewDecision {
    Accepted,
    Rejected,
    Withdrawn,
    Superseded,
}

impl RelationshipReviewDecision {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Withdrawn => "withdrawn",
            Self::Superseded => "superseded",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "origin", rename_all = "snake_case")]
pub enum RelationshipOrigin {
    SourceAssertion {
        snapshot: SnapshotKey,
        field: String,
        location: Option<DocumentLocation>,
    },
    DerivedCandidate {
        rule_version: String,
        supporting_assertions: Vec<RelationshipAssertionKey>,
    },
    UserConclusion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentLocation {
    pub line: i64,
    pub column: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RelationshipAssertionKey(String);

impl RelationshipAssertionKey {
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationshipClaim {
    pub relation_type: RelationshipType,
    pub subject: RelationshipEndpoint,
    pub target: RelationshipEndpoint,
    pub origin: RelationshipOrigin,
    pub evidence: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationshipReview {
    pub decision: RelationshipReviewDecision,
    pub note: String,
    pub superseded_by: Option<RelationshipAssertionKey>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationshipExplanation {
    pub assertion_key: RelationshipAssertionKey,
    pub claim: RelationshipClaim,
    pub source_field: Option<String>,
    pub source_location: Option<DocumentLocation>,
    pub source: Option<RelationshipSourceProvenance>,
    pub latest_review: Option<RelationshipReview>,
    pub review_history: Vec<RelationshipReviewEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationshipReviewEvent {
    pub review: RelationshipReview,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationshipSourceProvenance {
    pub source_key: String,
    pub source_name: String,
    pub document_key: String,
    pub declared_version: Option<String>,
    pub parser_name: Option<String>,
    pub parser_version: Option<String>,
    pub rules_version: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ObservedContent {
    pub scope: EvidenceScope,
    pub provenance: EvidenceProvenance,
    pub size: Option<u64>,
    pub crc: Option<Crc32Digest>,
    pub md5: Option<Md5Digest>,
    pub sha1: Option<Sha1Digest>,
    pub xxh3: crate::hashes::Xxh3Digest,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DatRom {
    pub catalog_name: String,
    pub key: RequirementKey,
    pub parent_name: Option<String>,
    pub set_metadata: SetMetadata,
    pub role: AssetRole,
    pub component_order: Option<u32>,
    pub expected: ExpectedEvidence,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum AssetRole {
    #[default]
    Rom,
    Disk,
    Other,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuildMode {
    #[default]
    ParentBundles,
    PerGame,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchingPolicy {
    /// Match only SHA1, ignoring size/CRC/MD5 disagreements for historical compatibility.
    #[default]
    Sha1Compatibility,
    /// Compare mutually available evidence, veto weaker fallback on stronger contradictions,
    /// and retain lower-priority disagreements as explanations after a stronger match.
    EvidenceAware,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ZipCompression {
    #[default]
    Deflate,
    Store,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputContainer {
    #[default]
    Zip,
    Directory,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ArchiveBackend {
    Zip,
    SevenZip,
    Rar,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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

/// Platform stat evidence used only to decide whether an opt-in bare-file scan may be reused.
/// It is not a content digest and never replaces build-time byte verification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct BareFileCacheStamp([u8; 32]);

impl BareFileCacheStamp {
    #[must_use]
    pub const fn new(digest: [u8; 32]) -> Self {
        Self(digest)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ScanProvenance {
    StreamedSha1Xxh3V1,
    ReusedStatValidatedV1,
}

impl ScanProvenance {
    #[must_use]
    pub const fn storage_key(self) -> &'static str {
        match self {
            Self::StreamedSha1Xxh3V1 | Self::ReusedStatValidatedV1 => "streamed_sha1_xxh3_v1",
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

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ArchiveMemberSelector {
    IndexAndName { index: u64, name: String },
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceFile {
    pub source_root: SourceRoot,
    pub location: SourceLocation,
    pub observed: ObservedContent,
    pub fingerprint: Option<SourceFingerprint>,
    pub scan_run: Option<ScanRunKey>,
    pub scan_provenance: Option<ScanProvenance>,
    pub bare_file_cache_stamp: Option<BareFileCacheStamp>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceObservation {
    pub source_root: SourceRoot,
    pub scan_run: ScanRunKey,
    pub location: SourceLocation,
    pub observed: ObservedContent,
    pub fingerprint: SourceFingerprint,
    pub scan_provenance: ScanProvenance,
    pub bare_file_cache_stamp: Option<BareFileCacheStamp>,
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
    pub source_roots: Vec<SourceRoot>,
    pub mode: BuildMode,
    pub matching_policy: MatchingPolicy,
    pub missing_policy: MissingContentPolicy,
    pub set_selection: SetSelection,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissingContentPolicy {
    #[default]
    AllowPartial,
    RequireComplete,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildPlan {
    pub groups: Vec<OutputGroup>,
    pub report: BuildReport,
}

impl BuildPlan {
    #[must_use]
    pub const fn has_outputs(&self) -> bool {
        !self.groups.is_empty()
    }

    pub const SERIALIZATION_VERSION: u32 = 2;

    /// Encode plan and report together with the schema version required to read them.
    pub fn to_json(&self) -> crate::Result<Vec<u8>> {
        #[derive(Serialize)]
        struct Document<'a> {
            version: u32,
            plan: &'a BuildPlan,
        }

        Ok(serde_json::to_vec(&Document {
            version: Self::SERIALIZATION_VERSION,
            plan: self,
        })?)
    }

    /// Decode only the current version; callers must re-resolve/revalidate before execution.
    pub fn from_json(bytes: &[u8]) -> crate::Result<Self> {
        #[derive(Deserialize)]
        struct Document {
            version: u32,
            plan: serde_json::Value,
        }

        let document: Document = serde_json::from_slice(bytes)?;
        if document.version != Self::SERIALIZATION_VERSION {
            return Err(crate::Error::UnsupportedPlanVersion(document.version));
        }
        Ok(serde_json::from_value(document.plan)?)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LogicalPath(String);

impl LogicalPath {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputGroup {
    pub path: LogicalPath,
    pub entries: Vec<LogicalEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogicalEntry {
    pub path: LogicalPath,
    pub source: SourceFile,
    pub requirement: RequirementKey,
    pub expected: ExpectedEvidence,
    pub selection: SelectionProvenance,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionProvenance {
    pub policy: MatchingPolicy,
    pub strength: crate::resolution::MatchStrength,
    pub assessments: Vec<crate::resolution::SourceAssessment>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanOutcome {
    Ready,
    Blocked(PlanBlockReason),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanBlockReason {
    MissingContent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildReport {
    pub missing_roms: Vec<MissingRom>,
    pub duplicate_matches: Vec<DuplicateMatch>,
    pub resolutions: Vec<crate::resolution::RequirementResolution>,
    /// Pure layout validation findings available before an output backend is invoked.
    pub validation_issues: Vec<crate::build::validation::PlanIssue>,
    pub matched_roms: usize,
    pub outcome: PlanOutcome,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationBasis {
    Cached,
    FreshScan { scan_run: ScanRunKey },
    FreshScans { scan_runs: Vec<RootScanRun> },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootScanRun {
    pub source_root: SourceRoot,
    pub scan_run: ScanRunKey,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AuditReport {
    schema_version: u32,
    observation_basis: ObservationBasis,
    report: BuildReport,
}

impl AuditReport {
    pub const SCHEMA_VERSION: u32 = 1;

    #[must_use]
    pub const fn new(observation_basis: ObservationBasis, report: BuildReport) -> Self {
        Self {
            schema_version: Self::SCHEMA_VERSION,
            observation_basis,
            report,
        }
    }

    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    #[must_use]
    pub const fn observation_basis(&self) -> &ObservationBasis {
        &self.observation_basis
    }

    #[must_use]
    pub const fn report(&self) -> &BuildReport {
        &self.report
    }

    pub fn to_json(&self) -> crate::Result<Vec<u8>> {
        Ok(serde_json::to_vec_pretty(self)?)
    }

    pub fn from_json(bytes: &[u8]) -> crate::Result<Self> {
        #[derive(Deserialize)]
        struct Document {
            schema_version: u32,
            observation_basis: ObservationBasis,
            report: BuildReport,
        }

        let document: Document = serde_json::from_slice(bytes)?;
        if document.schema_version != Self::SCHEMA_VERSION {
            return Err(crate::Error::UnsupportedAuditVersion(
                document.schema_version,
            ));
        }
        Ok(Self::new(document.observation_basis, document.report))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtifactOutcome {
    Completed,
    CompletedWithWarning { warning: String },
    Failed { error: String },
    Unattempted,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactResult {
    pub path: String,
    pub outcome: ArtifactOutcome,
}

impl Default for BuildReport {
    fn default() -> Self {
        Self {
            missing_roms: Vec::new(),
            duplicate_matches: Vec::new(),
            resolutions: Vec::new(),
            validation_issues: Vec::new(),
            matched_roms: 0,
            outcome: PlanOutcome::Ready,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingRom {
    pub game_name: String,
    pub rom_name: String,
    pub sha1: Option<Sha1Digest>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
