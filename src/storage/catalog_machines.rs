//! Bounded queries for published native MAME machine facts.

mod queries;
mod reader;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod roundtrip_tests;

use thiserror::Error;

use crate::{database::Database, domain::SnapshotKey};

pub use crate::{
    domain::{CatalogRegistryId, CatalogSetId},
    logiqx::RecordLocation as MameSourceLocation,
    mame::{
        Adjuster, Analog, Chip, ChipKind, ConditionRelation, Device, DeviceExtension,
        DeviceInstance, DeviceReference, Display, DisplayKind, DisplayRotation, Driver,
        DriverQuality, Feature, FeatureKind, FeatureStatus, Input, InputControl, MachineBiosSet,
        MachineCondition, MachineFacts, MachineFlags, MachineSpecification,
        MachineSpecificationElement, MachineSwitchKind, MameBoolean, Port, RamOption, Sample,
        SaveState, Slot, SlotOption, SoftwareList, SoftwareListStatus, Sound,
    },
    storage::catalog_files::OccurrenceId,
};

const MAX_PAGE_SIZE: usize = 500;

/// A validated maximum number of machines returned by one query.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MachinePageLimit(usize);

impl MachinePageLimit {
    /// Create a page limit in the supported range `1..=500`.
    pub fn new(value: usize) -> Result<Self, MachineQueryError> {
        if (1..=MAX_PAGE_SIZE).contains(&value) {
            Ok(Self(value))
        } else {
            Err(MachineQueryError::InvalidPageLimit {
                requested: value,
                maximum: MAX_PAGE_SIZE,
            })
        }
    }

    fn database_value(self) -> Result<i64, MachineQueryError> {
        i64::try_from(self.0).map_err(|_| MachineQueryError::PageLimitOverflow)
    }
}

/// Continuation for a machine page, pinned to its snapshot and registry generation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineCursor {
    generation: CatalogRegistryId,
    snapshot: SnapshotKey,
    order: i64,
    owner_id: CatalogSetId,
}

/// Native MAME document facts and their source location, returned once per page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MameDocumentFacts {
    pub build: Option<String>,
    pub debug: bool,
    pub debug_specified: bool,
    pub config_version: String,
    pub location: MameSourceLocation,
}

/// Snapshot identity and publication provenance shared by one page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineSnapshot {
    pub registry_id: CatalogRegistryId,
    pub snapshot_key: SnapshotKey,
    pub source_key: crate::domain::PublishingSourceKey,
    pub source_name: String,
    pub catalog_key: crate::domain::CatalogKey,
    pub catalog_name: String,
    pub document_key: crate::domain::DocumentKey,
    pub interpretation_key: crate::domain::ParserInterpretationKey,
    pub format: String,
    pub header: MameDocumentFacts,
}

/// A native MAME root relationship or ordered device reference.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MachineDependency {
    CloneOf {
        target_name: String,
        location: MameSourceLocation,
    },
    RomOf {
        target_name: String,
        location: MameSourceLocation,
    },
    SampleOf {
        target_name: String,
        location: MameSourceLocation,
    },
    DeviceReference(DeviceReference),
}

/// Reference to a persisted ROM or disk occurrence, without copying its payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineAssetReference {
    pub occurrence_id: OccurrenceId,
    pub kind: MachineAssetKind,
    pub source_order: i64,
    pub location: MameSourceLocation,
}

/// Native MAME asset claim family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MachineAssetKind {
    Rom,
    Disk,
}

/// A native DIP or configuration switch and its ordered child facts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineSwitch {
    pub kind: MachineSwitchKind,
    pub name: String,
    pub tag: String,
    pub mask: String,
    pub source_order: i64,
    pub location: MameSourceLocation,
    pub condition: Option<MachineCondition>,
    pub locations: Vec<MachineSwitchLocation>,
    pub values: Vec<MachineSwitchValue>,
}

/// The actual child element used for a switch's location declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MachineSwitchLocationKind {
    DipLocation,
    ConfigurationLocation,
}

/// A native switch location, retaining compatibility configuration locations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineSwitchLocation {
    pub kind: MachineSwitchLocationKind,
    pub name: String,
    pub number: String,
    pub inverted: bool,
    pub inverted_specified: bool,
    pub source_order: i64,
    pub location: MameSourceLocation,
}

/// The actual value child element used by a DIP/configuration switch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MachineSwitchValueKind {
    DipValue,
    ConfigurationSetting,
}

/// A native switch value with its element kind and optional condition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineSwitchValue {
    pub kind: MachineSwitchValueKind,
    pub name: String,
    pub value: String,
    pub default: bool,
    pub default_specified: bool,
    pub condition: Option<MachineCondition>,
    pub source_order: i64,
    pub location: MameSourceLocation,
}

/// A machine owner and every native fact queried for that owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Machine {
    pub id: CatalogSetId,
    pub name: String,
    pub list_order: i64,
    pub location: MameSourceLocation,
    pub facts: MachineFacts,
    pub dependencies: Vec<MachineDependency>,
    pub assets: Vec<MachineAssetReference>,
    pub bios_sets: Vec<MachineBiosSet>,
    pub switches: Vec<MachineSwitch>,
    pub specification: Vec<MachineSpecificationElement>,
}

/// One bounded machine page and its publication provenance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachinePage {
    pub snapshot: MachineSnapshot,
    pub machines: Vec<Machine>,
    pub next_cursor: Option<MachineCursor>,
}

/// Errors returned by native MAME machine queries.
#[derive(Debug, Error)]
pub enum MachineQueryError {
    #[error("database pool error: {0}")]
    Pool(#[from] diesel::r2d2::PoolError),
    #[error("database query failed: {0}")]
    Database(#[from] diesel::result::Error),
    #[error("page limit {requested} is outside 1..={maximum}")]
    InvalidPageLimit { requested: usize, maximum: usize },
    #[error("snapshot lookup failed: {0}")]
    Registry(#[from] crate::Error),
    #[error("snapshot {0} is not a published MAME machine edition")]
    NotPublishedMame(SnapshotKey),
    #[error("cursor belongs to a different catalog registry generation")]
    CursorRegistryMismatch,
    #[error("cursor belongs to a different snapshot")]
    CursorSnapshotMismatch,
    #[error("native MAME metadata is missing {field} for owner {owner}")]
    MissingNative { field: &'static str, owner: i64 },
    #[error("native MAME metadata has mismatched owner links for row {0}")]
    MismatchedOwner(i64),
    #[error("invalid stored {field} for owner {owner}: {value}")]
    InvalidStoredValue {
        field: &'static str,
        owner: i64,
        value: String,
    },
    #[error("native MAME source positions are inconsistent for owner {0}")]
    InvalidPositions(i64),
    #[error("page size cannot be represented by SQLite")]
    PageLimitOverflow,
}

/// Return a bounded page of machines from an exact published MAME snapshot.
pub fn machines_for_snapshot(
    database: &Database,
    snapshot: &SnapshotKey,
    cursor: Option<&MachineCursor>,
    limit: MachinePageLimit,
) -> Result<MachinePage, MachineQueryError> {
    reader::machines_for_snapshot(database, snapshot, cursor, limit)
}
