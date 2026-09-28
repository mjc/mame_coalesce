//! Versioned, snapshot-pinned target views built from pure MAME layout plans.

use std::{cmp::Ordering, io::Write, num::NonZeroU16};

use serde::{Deserialize, Serialize};

use crate::{
    build::mame_layout::{
        CoalescedAssetProvenance, MameLayoutDiagnostic, MameLayoutPlan, MameSetLayoutPolicy,
        ResolvedMachineSet,
    },
    domain::{LogicalEntry, LogicalPath, SetName, SnapshotKey},
    machine_dependencies::DependencyClosure,
};

/// Current on-disk envelope version for a target view manifest.
pub const VIEW_MANIFEST_FORMAT_VERSION: u32 = 1;
/// Maximum encoded size accepted by the manifest decoder.
pub const MAX_VIEW_MANIFEST_BYTES: usize = 64 * 1024 * 1024;

/// The concrete MAME behavior this manifest is planned against.
///
/// This deliberately identifies a specific release rather than making a generic emulator
/// compatibility claim. MAME 0.289's software-item lookup accepts list/item-named locations
/// under `rompath`; see <https://docs.mamedev.org/usingmame/assetsearch.html>.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetProfile {
    #[serde(rename = "mame-0.289")]
    Mame0289,
}

impl TargetProfile {
    #[must_use]
    pub const fn emulator_name(self) -> &'static str {
        match self {
            Self::Mame0289 => "MAME",
        }
    }

    #[must_use]
    pub const fn emulator_version(self) -> &'static str {
        match self {
            Self::Mame0289 => "0.289",
        }
    }

    /// Stable identifier suitable for logs and manifests.
    #[must_use]
    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::Mame0289 => "mame-0.289",
        }
    }
}

/// How source relationships affect the selected view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyPolicy {
    IncludeRuntimeClosure,
}

/// Naming behavior understood by the selected emulator profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NamingConvention {
    MameRomPathSearch,
}

/// Which source representation the target view selects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepresentationChoice {
    PreserveCatalogComponents,
}

/// Evidence required when selecting content for this view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidencePolicy {
    SnapshotRequirementsAndObservedContent,
}

/// Non-zero, typed revision for one independently evolving view policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PolicyVersion(NonZeroU16);

impl PolicyVersion {
    pub const V1: Self = Self(NonZeroU16::MIN);

    #[must_use]
    pub const fn new(value: u16) -> Option<Self> {
        match NonZeroU16::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    #[must_use]
    pub const fn get(self) -> u16 {
        self.0.get()
    }
}

/// Versions for policies that influence manifest identity or planned output.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewPolicyVersions {
    pub dependency: PolicyVersion,
    pub layout: PolicyVersion,
    pub naming: PolicyVersion,
    pub representation: PolicyVersion,
    pub evidence: PolicyVersion,
}

impl ViewPolicyVersions {
    pub const V1: Self = Self {
        dependency: PolicyVersion::V1,
        layout: PolicyVersion::V1,
        naming: PolicyVersion::V1,
        representation: PolicyVersion::V1,
        evidence: PolicyVersion::V1,
    };
}

/// Format-neutral content grouping and diagnostics captured from the pure layout planner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewLayout {
    policy: MameSetLayoutPolicy,
    request_roots: Vec<SetName>,
    groups: Vec<ViewGroup>,
    coalesced_provenance: Vec<CoalescedAssetProvenance>,
    diagnostics: Vec<MameLayoutDiagnostic>,
}

impl ViewLayout {
    #[must_use]
    pub const fn policy(&self) -> MameSetLayoutPolicy {
        self.policy
    }

    #[must_use]
    pub fn groups(&self) -> &[ViewGroup] {
        &self.groups
    }

    #[must_use]
    pub fn coalesced_provenance(&self) -> &[CoalescedAssetProvenance] {
        &self.coalesced_provenance
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[MameLayoutDiagnostic] {
        &self.diagnostics
    }
}

/// A format-neutral directory/group in a view.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewGroup {
    path: LogicalPath,
    entries: Vec<LogicalEntry>,
}

impl ViewGroup {
    #[must_use]
    pub const fn path(&self) -> &LogicalPath {
        &self.path
    }

    #[must_use]
    pub fn entries(&self) -> &[LogicalEntry] {
        &self.entries
    }
}

/// Inputs consumed together by the MAME layout planner and pinned manifest builder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TargetViewRequest<'a> {
    snapshot: &'a SnapshotKey,
    selected_roots: &'a [SetName],
    layout_policy: MameSetLayoutPolicy,
    closures: &'a [DependencyClosure],
    sets: &'a [ResolvedMachineSet],
}

impl<'a> TargetViewRequest<'a> {
    #[must_use]
    pub const fn mame_0289(
        snapshot: &'a SnapshotKey,
        selected_roots: &'a [SetName],
        layout_policy: MameSetLayoutPolicy,
        closures: &'a [DependencyClosure],
        sets: &'a [ResolvedMachineSet],
    ) -> Self {
        Self {
            snapshot,
            selected_roots,
            layout_policy,
            closures,
            sets,
        }
    }
}

/// Plan a target view and capture its metadata from the same request, preventing mismatched pins.
#[must_use]
pub fn plan_view(request: TargetViewRequest<'_>) -> ViewManifest {
    let layout = crate::build::mame_layout::plan_mame_layout(
        request.layout_policy,
        request.snapshot,
        request.selected_roots,
        request.closures,
        request.sets,
    );
    ViewManifest::from_layout(request.selected_roots.iter().cloned(), layout)
}

/// An immutable request and result pinned to one exact catalog interpretation.
///
/// This is a descriptive planning artifact, not authority to execute against refreshed
/// inventory. Materialization must independently revalidate selected content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewManifest {
    profile: TargetProfile,
    snapshot: SnapshotKey,
    selected_roots: Vec<SetName>,
    dependency_policy: DependencyPolicy,
    naming: NamingConvention,
    representation: RepresentationChoice,
    evidence: EvidencePolicy,
    policy_versions: ViewPolicyVersions,
    layout: ViewLayout,
}

#[derive(Deserialize)]
struct ViewManifestBody {
    profile: TargetProfile,
    snapshot: SnapshotKey,
    selected_roots: Vec<SetName>,
    dependency_policy: DependencyPolicy,
    naming: NamingConvention,
    representation: RepresentationChoice,
    evidence: EvidencePolicy,
    policy_versions: ViewPolicyVersions,
    layout: ViewLayout,
}

#[derive(Serialize)]
struct VersionedViewManifest<'a> {
    version: u32,
    manifest: ViewManifestRef<'a>,
}

#[derive(Serialize)]
struct ViewManifestRef<'a> {
    profile: TargetProfile,
    snapshot: &'a SnapshotKey,
    selected_roots: &'a [SetName],
    dependency_policy: DependencyPolicy,
    naming: NamingConvention,
    representation: RepresentationChoice,
    evidence: EvidencePolicy,
    policy_versions: &'a ViewPolicyVersions,
    layout: &'a ViewLayout,
}

#[derive(Deserialize)]
struct VersionedViewManifestRef<'a> {
    version: u32,
    #[serde(borrow)]
    manifest: &'a serde_json::value::RawValue,
}

impl ViewManifest {
    /// Capture a MAME layout result, sorting request roots and output records canonically.
    #[must_use]
    fn from_layout(
        selected_roots: impl IntoIterator<Item = SetName>,
        plan: MameLayoutPlan,
    ) -> Self {
        let mut selected_roots = selected_roots.into_iter().collect::<Vec<_>>();
        selected_roots.sort();

        let MameLayoutPlan {
            policy,
            snapshot,
            mut groups,
            mut coalesced_provenance,
            diagnostics,
        } = plan;
        for group in &mut groups {
            group.entries.sort_by(compare_entries);
        }
        groups.sort_by(|left, right| {
            compare_group_parts(&left.path, &left.entries, &right.path, &right.entries)
        });
        coalesced_provenance.sort();
        let request_roots = selected_roots.clone();

        Self {
            profile: TargetProfile::Mame0289,
            snapshot,
            selected_roots,
            dependency_policy: DependencyPolicy::IncludeRuntimeClosure,
            naming: NamingConvention::MameRomPathSearch,
            representation: RepresentationChoice::PreserveCatalogComponents,
            evidence: EvidencePolicy::SnapshotRequirementsAndObservedContent,
            policy_versions: ViewPolicyVersions::V1,
            layout: ViewLayout {
                policy,
                request_roots,
                groups: groups
                    .into_iter()
                    .map(|group| ViewGroup {
                        path: group.path,
                        entries: group.entries,
                    })
                    .collect(),
                coalesced_provenance,
                diagnostics: diagnostics.into_iter().collect(),
            },
        }
    }

    #[must_use]
    pub const fn profile(&self) -> TargetProfile {
        self.profile
    }

    #[must_use]
    pub const fn snapshot(&self) -> &SnapshotKey {
        &self.snapshot
    }

    #[must_use]
    pub fn selected_roots(&self) -> &[SetName] {
        &self.selected_roots
    }

    #[must_use]
    pub const fn dependency_policy(&self) -> DependencyPolicy {
        self.dependency_policy
    }

    #[must_use]
    pub const fn naming(&self) -> NamingConvention {
        self.naming
    }

    #[must_use]
    pub const fn representation(&self) -> RepresentationChoice {
        self.representation
    }

    #[must_use]
    pub const fn evidence(&self) -> EvidencePolicy {
        self.evidence
    }

    #[must_use]
    pub const fn policy_versions(&self) -> &ViewPolicyVersions {
        &self.policy_versions
    }

    #[must_use]
    pub const fn layout(&self) -> &ViewLayout {
        &self.layout
    }

    /// Encode this manifest in a stable, explicitly versioned JSON envelope.
    pub fn to_json(&self) -> Result<Vec<u8>, ViewManifestError> {
        let mut bytes = Vec::new();
        self.write_json(&mut bytes)?;
        Ok(bytes)
    }

    fn write_json<W: Write>(&self, mut writer: W) -> Result<W, serde_json::Error> {
        serde_json::to_writer(
            &mut writer,
            &VersionedViewManifest {
                version: VIEW_MANIFEST_FORMAT_VERSION,
                manifest: ViewManifestRef {
                    profile: self.profile,
                    snapshot: &self.snapshot,
                    selected_roots: &self.selected_roots,
                    dependency_policy: self.dependency_policy,
                    naming: self.naming,
                    representation: self.representation,
                    evidence: self.evidence,
                    policy_versions: &self.policy_versions,
                    layout: &self.layout,
                },
            },
        )?;
        Ok(writer)
    }

    /// Decode only the current format version; unsupported versions are never guessed.
    pub fn from_json(bytes: &[u8]) -> Result<Self, ViewManifestError> {
        if bytes.len() > MAX_VIEW_MANIFEST_BYTES {
            return Err(ViewManifestError::InputTooLarge {
                maximum: MAX_VIEW_MANIFEST_BYTES,
            });
        }
        let document: VersionedViewManifestRef<'_> = serde_json::from_slice(bytes)?;
        if document.version != VIEW_MANIFEST_FORMAT_VERSION {
            return Err(ViewManifestError::UnsupportedVersion(document.version));
        }
        let body: ViewManifestBody = serde_json::from_str(document.manifest.get())?;
        let manifest = Self {
            profile: body.profile,
            snapshot: body.snapshot,
            selected_roots: body.selected_roots,
            dependency_policy: body.dependency_policy,
            naming: body.naming,
            representation: body.representation,
            evidence: body.evidence,
            policy_versions: body.policy_versions,
            layout: body.layout,
        };
        validate_imported_roots_and_policy(&manifest)?;
        validate_imported_order(&manifest)?;
        validate_imported_output_diagnostics(&manifest)?;
        validate_imported_provenance(&manifest)?;
        let mut canonical = CanonicalJson::new(bytes);
        manifest.write_json(&mut canonical)?;
        if !canonical.matches() {
            return Err(ViewManifestError::NonCanonicalEncoding);
        }
        Ok(manifest)
    }
}

struct CanonicalJson<'a> {
    input: &'a [u8],
    offset: usize,
    matches: bool,
}

impl<'a> CanonicalJson<'a> {
    const fn new(input: &'a [u8]) -> Self {
        Self {
            input,
            offset: 0,
            matches: true,
        }
    }

    const fn matches(&self) -> bool {
        self.matches && self.offset == self.input.len()
    }
}

impl Write for CanonicalJson<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let end = self.offset.saturating_add(bytes.len());
        if self.input.get(self.offset..end) != Some(bytes) {
            self.matches = false;
        }
        self.offset = end;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn validate_imported_roots_and_policy(manifest: &ViewManifest) -> Result<(), ViewManifestError> {
    if manifest.selected_roots != manifest.layout.request_roots {
        return Err(ViewManifestError::MismatchedSelectedRoots);
    }
    if let Some(root) = manifest
        .layout
        .diagnostics
        .iter()
        .filter_map(MameLayoutDiagnostic::selected_root)
        .find(|root| !manifest.selected_roots.contains(root))
    {
        return Err(ViewManifestError::DiagnosticRootNotSelected { root: root.clone() });
    }
    if manifest
        .layout
        .diagnostics
        .iter()
        .filter_map(diagnostic_snapshot)
        .any(|snapshot| snapshot != &manifest.snapshot)
    {
        return Err(ViewManifestError::DiagnosticSnapshotMismatch);
    }
    if let Some((policy, version)) = [
        ("dependency", manifest.policy_versions.dependency),
        ("layout", manifest.policy_versions.layout),
        ("naming", manifest.policy_versions.naming),
        ("representation", manifest.policy_versions.representation),
        ("evidence", manifest.policy_versions.evidence),
    ]
    .into_iter()
    .find(|(_, version)| *version != PolicyVersion::V1)
    {
        return Err(ViewManifestError::UnsupportedPolicyVersion {
            policy,
            version: version.get(),
        });
    }
    if let Some(group) = manifest
        .layout
        .groups
        .windows(2)
        .find(|pair| pair[0].path == pair[1].path)
    {
        return Err(ViewManifestError::DuplicateGroupPath {
            path: group[0].path.clone(),
        });
    }
    Ok(())
}

const fn diagnostic_snapshot(diagnostic: &MameLayoutDiagnostic) -> Option<&SnapshotKey> {
    match diagnostic {
        MameLayoutDiagnostic::SnapshotMismatch { expected, .. }
        | MameLayoutDiagnostic::ResolvedSetSnapshotMismatch { expected, .. } => Some(expected),
        MameLayoutDiagnostic::MissingResolvedSet { snapshot, .. }
        | MameLayoutDiagnostic::AmbiguousResolvedSet { snapshot, .. } => Some(snapshot),
        MameLayoutDiagnostic::InvalidAssetIdentity { identity, .. }
        | MameLayoutDiagnostic::DuplicateAssetIdentity { identity, .. } => Some(&identity.snapshot),
        _ => None,
    }
}

fn validate_imported_order(manifest: &ViewManifest) -> Result<(), ViewManifestError> {
    if manifest
        .selected_roots
        .windows(2)
        .any(|pair| pair[0] > pair[1])
        || manifest.layout.groups.windows(2).any(|pair| {
            compare_group_parts(
                &pair[0].path,
                &pair[0].entries,
                &pair[1].path,
                &pair[1].entries,
            ) == Ordering::Greater
        })
        || manifest.layout.groups.iter().any(|group| {
            group
                .entries
                .windows(2)
                .any(|pair| compare_entries(&pair[0], &pair[1]) != Ordering::Less)
        })
        || manifest
            .layout
            .diagnostics
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || manifest.layout.coalesced_provenance.windows(2).any(|pair| {
            pair[0].group > pair[1].group
                || (pair[0].group == pair[1].group && pair[0].path >= pair[1].path)
        })
        || manifest.layout.coalesced_provenance.iter().any(|record| {
            record
                .requirements
                .windows(2)
                .any(|pair| pair[0].requirement >= pair[1].requirement)
        })
    {
        return Err(ViewManifestError::NonCanonicalOrdering);
    }
    Ok(())
}

fn validate_imported_output_diagnostics(manifest: &ViewManifest) -> Result<(), ViewManifestError> {
    let expected = crate::build::validation::inspect_group_parts(
        manifest
            .layout
            .groups
            .iter()
            .map(|group| (&group.path, group.entries.as_slice())),
    )
    .into_iter()
    .collect::<std::collections::BTreeSet<_>>();
    let actual = manifest
        .layout
        .diagnostics
        .iter()
        .filter_map(|diagnostic| match diagnostic {
            MameLayoutDiagnostic::OutputValidation { issue } => Some(issue.clone()),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    if actual != expected {
        return Err(ViewManifestError::InconsistentOutputDiagnostics);
    }
    Ok(())
}

fn validate_imported_provenance(manifest: &ViewManifest) -> Result<(), ViewManifestError> {
    for provenance in &manifest.layout.coalesced_provenance {
        let group_index = manifest
            .layout
            .groups
            .binary_search_by(|group| group.path.cmp(&provenance.group));
        let Ok(group_index) = group_index else {
            return Err(ViewManifestError::InvalidCoalescedProvenance);
        };
        let group = &manifest.layout.groups[group_index];
        let first_entry = group
            .entries
            .partition_point(|entry| entry.path < provenance.path);
        let provenance_entry = group.entries[first_entry..]
            .iter()
            .take_while(|entry| entry.path == provenance.path)
            .find(|entry| {
                provenance
                    .requirements
                    .iter()
                    .any(|evidence| evidence.requirement == entry.requirement)
            });
        if !provenance_entry.is_some_and(|entry| {
            provenance.is_consistent_with(entry, entry.requirement.set().catalog())
        }) {
            return Err(ViewManifestError::InvalidCoalescedProvenance);
        }
    }
    Ok(())
}

fn compare_entries(left: &LogicalEntry, right: &LogicalEntry) -> Ordering {
    left.path
        .cmp(&right.path)
        .then_with(|| left.requirement.cmp(&right.requirement))
        .then_with(|| left.source.cmp(&right.source))
        .then_with(|| left.expected.cmp(&right.expected))
        .then_with(|| left.selection.cmp(&right.selection))
}

fn compare_group_parts(
    left_path: &LogicalPath,
    left_entries: &[LogicalEntry],
    right_path: &LogicalPath,
    right_entries: &[LogicalEntry],
) -> Ordering {
    left_path.cmp(right_path).then_with(|| {
        left_entries
            .iter()
            .zip(right_entries)
            .map(|(left, right)| compare_entries(left, right))
            .find(|order| *order != Ordering::Equal)
            .unwrap_or_else(|| left_entries.len().cmp(&right_entries.len()))
    })
}

#[derive(Debug, thiserror::Error)]
pub enum ViewManifestError {
    #[error("view manifest exceeds the maximum encoded size of {maximum} bytes")]
    InputTooLarge { maximum: usize },
    #[error("unsupported view manifest format version {0}")]
    UnsupportedVersion(u32),
    #[error("view manifest is not in canonical order")]
    NonCanonicalOrdering,
    #[error("view manifest JSON contains unknown or non-canonical data")]
    NonCanonicalEncoding,
    #[error("view manifest output diagnostics do not match its groups")]
    InconsistentOutputDiagnostics,
    #[error("view manifest coalesced provenance does not match its groups")]
    InvalidCoalescedProvenance,
    #[error("view manifest roots do not match the roots used to plan its layout")]
    MismatchedSelectedRoots,
    #[error("view manifest diagnostic refers to unselected root {root:?}")]
    DiagnosticRootNotSelected { root: SetName },
    #[error("view manifest diagnostic snapshot does not match the pinned snapshot")]
    DiagnosticSnapshotMismatch,
    #[error("view manifest contains duplicate output group path {path:?}")]
    DuplicateGroupPath { path: LogicalPath },
    #[error("unsupported {policy} policy version {version}")]
    UnsupportedPolicyVersion { policy: &'static str, version: u16 },
    #[error("invalid view manifest JSON: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    use crate::{
        domain::{
            CatalogKey, CatalogScope, DocumentKey, EvidenceProvenance, EvidenceScope,
            ExpectedEvidence, LogicalEntry, MatchingPolicy, ObservedContent, OutputGroup,
            ParserInterpretationKey, RequirementKey, SelectionProvenance, SetKey, SourceFile,
            SourceLocation, SourcePhysicalPath, SourceRoot,
        },
        machine_dependencies::MachineDependencyCatalog,
        resolution::MatchStrength,
    };

    fn snapshot(name: &str) -> SnapshotKey {
        let catalog = CatalogKey::new("mame");
        let document = DocumentKey::from_bytes(name.as_bytes());
        let interpretation =
            ParserInterpretationKey::for_format("mame-machine-xml", &CatalogScope::Complete);
        SnapshotKey::new(&catalog, &document, &interpretation)
    }

    fn manifest(snapshot: &SnapshotKey) -> ViewManifest {
        let roots = [SetName::new("nes"), SetName::new("c64")];
        let closures = [];
        let sets = [];
        plan_view(TargetViewRequest::mame_0289(
            snapshot,
            &roots,
            MameSetLayoutPolicy::NonMerged,
            &closures,
            &sets,
        ))
    }

    fn manifest_from_plan(
        roots: &[SetName],
        groups: Vec<OutputGroup>,
        coalesced_provenance: Vec<CoalescedAssetProvenance>,
    ) -> ViewManifest {
        ViewManifest::from_layout(
            roots.iter().cloned(),
            MameLayoutPlan {
                policy: MameSetLayoutPolicy::NonMerged,
                snapshot: snapshot("snapshot"),
                groups,
                coalesced_provenance,
                diagnostics: BTreeSet::new(),
            },
        )
    }

    fn entry() -> LogicalEntry {
        let location = SourceLocation::BareFile {
            path: "/roms/same.rom".to_owned(),
        };
        LogicalEntry {
            path: LogicalPath::new("same.rom"),
            source: SourceFile {
                source_root: SourceRoot::new("/roms"),
                physical_path: SourcePhysicalPath::from_location(&location),
                location,
                observed: ObservedContent {
                    scope: EvidenceScope::WholeAsset,
                    provenance: EvidenceProvenance::Computed,
                    size: Some(1),
                    crc: None,
                    md5: None,
                    sha1: Some([1; 20]),
                    xxh3: [0; 8],
                },
                fingerprint: None,
                scan_run: None,
                scan_provenance: None,
                bare_file_cache_stamp: None,
            },
            requirement: RequirementKey::new(
                SetKey::new(CatalogKey::new("mame"), "set"),
                "same.rom",
            ),
            expected: ExpectedEvidence {
                scope: EvidenceScope::WholeAsset,
                provenance: EvidenceProvenance::SourceDeclared,
                sha1: Some([1; 20]),
                ..ExpectedEvidence::default()
            },
            selection: SelectionProvenance {
                policy: MatchingPolicy::Sha1Compatibility,
                strength: MatchStrength::Sha1,
                assessments: Vec::new(),
                omitted_assessments: 0,
            },
        }
    }

    fn coalesced_requirement(
        entry: &LogicalEntry,
    ) -> crate::build::mame_layout::CoalescedRequirementEvidence {
        crate::build::mame_layout::CoalescedRequirementEvidence {
            requirement: entry.requirement.clone(),
            expected: entry.expected.clone(),
        }
    }

    #[test]
    fn profile_identifies_a_concrete_emulator_release_and_contract() {
        let profile = TargetProfile::Mame0289;
        assert_eq!(profile.stable_id(), "mame-0.289");
        assert_eq!(profile.emulator_name(), "MAME");
        assert_eq!(profile.emulator_version(), "0.289");
    }

    #[test]
    fn serialization_round_trips_with_an_explicit_version() -> Result<(), ViewManifestError> {
        let expected = manifest(&snapshot("catalog-snapshot-v1"));
        let encoded = expected.to_json()?;
        let json: serde_json::Value = serde_json::from_slice(&encoded)?;
        assert_eq!(json["version"], VIEW_MANIFEST_FORMAT_VERSION);
        assert_eq!(json["manifest"]["profile"], "mame-0.289");
        let actual = ViewManifest::from_json(&encoded)?;
        assert_eq!(actual, expected);
        assert!(actual.selected_roots()[0] < actual.selected_roots()[1]);
        Ok(())
    }

    #[test]
    fn extra_stale_closure_diagnostics_round_trip() -> Result<(), ViewManifestError> {
        let target_snapshot = snapshot("target");
        let unselected_closure = MachineDependencyCatalog::new(snapshot("other"), Vec::new())
            .resolve(&SetName::new("unselected"));
        let roots = [SetName::new("c64"), SetName::new("nes")];
        let closures = [unselected_closure];
        let sets = [];
        let expected = plan_view(TargetViewRequest::mame_0289(
            &target_snapshot,
            &roots,
            MameSetLayoutPolicy::NonMerged,
            &closures,
            &sets,
        ));

        assert!(expected.layout().diagnostics().iter().any(|diagnostic| {
            matches!(diagnostic, MameLayoutDiagnostic::SnapshotMismatch { root, .. }
                if root == &SetName::new("unselected"))
        }));
        assert_eq!(ViewManifest::from_json(&expected.to_json()?)?, expected);
        Ok(())
    }

    #[test]
    fn unsupported_format_versions_are_rejected() -> Result<(), ViewManifestError> {
        let mut json: serde_json::Value =
            serde_json::from_slice(&manifest(&snapshot("snapshot")).to_json()?)?;
        json["version"] = serde_json::json!(VIEW_MANIFEST_FORMAT_VERSION + 1);
        assert!(matches!(
            ViewManifest::from_json(&serde_json::to_vec(&json)?),
            Err(ViewManifestError::UnsupportedVersion(version))
                if version == VIEW_MANIFEST_FORMAT_VERSION + 1
        ));
        Ok(())
    }

    #[test]
    fn noncanonical_manifest_order_is_rejected() -> Result<(), ViewManifestError> {
        let mut json: serde_json::Value =
            serde_json::from_slice(&manifest(&snapshot("snapshot")).to_json()?)?;
        json["manifest"]["selected_roots"] = serde_json::json!(["nes", "c64"]);
        json["manifest"]["layout"]["request_roots"] = serde_json::json!(["nes", "c64"]);
        assert!(matches!(
            ViewManifest::from_json(&serde_json::to_vec(&json)?),
            Err(ViewManifestError::NonCanonicalOrdering)
        ));
        Ok(())
    }

    #[test]
    fn reordered_diagnostics_are_rejected() -> Result<(), ViewManifestError> {
        let mut json: serde_json::Value =
            serde_json::from_slice(&manifest(&snapshot("snapshot")).to_json()?)?;
        json["manifest"]["layout"]["diagnostics"] = serde_json::json!([
            { "kind": "missing_closure", "root": "nes" },
            { "kind": "missing_closure", "root": "c64" }
        ]);
        assert!(matches!(
            ViewManifest::from_json(&serde_json::to_vec(&json)?),
            Err(ViewManifestError::NonCanonicalOrdering)
        ));
        Ok(())
    }

    #[test]
    fn imported_roots_must_match_the_planned_layout() -> Result<(), ViewManifestError> {
        let mut json: serde_json::Value =
            serde_json::from_slice(&manifest(&snapshot("snapshot")).to_json()?)?;
        json["manifest"]["selected_roots"] = serde_json::json!(["arcade", "nes"]);
        assert!(matches!(
            ViewManifest::from_json(&serde_json::to_vec(&json)?),
            Err(ViewManifestError::MismatchedSelectedRoots)
        ));
        Ok(())
    }

    #[test]
    fn root_diagnostics_must_belong_to_a_selected_root() -> Result<(), ViewManifestError> {
        let mut json: serde_json::Value =
            serde_json::from_slice(&manifest(&snapshot("snapshot")).to_json()?)?;
        let changed_roots = serde_json::json!(["arcade", "nes"]);
        json["manifest"]["selected_roots"] = changed_roots.clone();
        json["manifest"]["layout"]["request_roots"] = changed_roots;
        assert!(matches!(
            ViewManifest::from_json(&serde_json::to_vec(&json)?),
            Err(ViewManifestError::DiagnosticRootNotSelected { root })
                if root == SetName::new("c64")
        ));
        Ok(())
    }

    #[test]
    fn diagnostic_snapshots_must_match_the_manifest_pin() {
        let pinned = snapshot("pinned");
        let foreign = snapshot("foreign");
        let root = SetName::new("nes");
        let diagnostics = [
            MameLayoutDiagnostic::SnapshotMismatch {
                root: root.clone(),
                expected: foreign.clone(),
                actual: pinned.clone(),
            },
            MameLayoutDiagnostic::ResolvedSetSnapshotMismatch {
                root: root.clone(),
                set: SetName::new("nes"),
                expected: foreign.clone(),
                actual: pinned.clone(),
            },
            MameLayoutDiagnostic::MissingResolvedSet {
                snapshot: foreign.clone(),
                set: SetName::new("nes"),
                required_by: root.clone(),
            },
            MameLayoutDiagnostic::AmbiguousResolvedSet {
                snapshot: foreign,
                set: SetName::new("nes"),
                matches: 2,
                required_by: SetName::new("nes"),
            },
            MameLayoutDiagnostic::InvalidAssetIdentity {
                root: root.clone(),
                set: SetName::new("nes"),
                identity: crate::build::mame_layout::AssetRequirementIdentity::new(
                    snapshot("foreign"),
                    SetName::new("nes"),
                    "nes.rom",
                    0,
                ),
                requirement: crate::domain::RequirementKey::new(
                    crate::domain::SetKey::new(crate::domain::CatalogKey::new("mame"), "nes"),
                    "nes.rom",
                ),
            },
            MameLayoutDiagnostic::DuplicateAssetIdentity {
                root,
                set: SetName::new("nes"),
                identity: crate::build::mame_layout::AssetRequirementIdentity::new(
                    snapshot("foreign"),
                    SetName::new("nes"),
                    "nes.rom",
                    0,
                ),
            },
        ];

        for diagnostic in diagnostics {
            let mut imported = manifest(&pinned);
            imported.layout.diagnostics.push(diagnostic);
            assert!(matches!(
                validate_imported_roots_and_policy(&imported),
                Err(ViewManifestError::DiagnosticSnapshotMismatch)
            ));
        }
    }

    #[test]
    fn oversized_manifests_are_rejected_before_decoding() {
        let input = vec![0; MAX_VIEW_MANIFEST_BYTES + 1];
        assert!(matches!(
            ViewManifest::from_json(&input),
            Err(ViewManifestError::InputTooLarge {
                maximum: MAX_VIEW_MANIFEST_BYTES
            })
        ));
    }

    #[test]
    fn duplicate_output_group_paths_are_rejected() -> Result<(), ViewManifestError> {
        let mut json: serde_json::Value =
            serde_json::from_slice(&manifest(&snapshot("snapshot")).to_json()?)?;
        json["manifest"]["layout"]["groups"] = serde_json::json!([
            { "path": "c64", "entries": [] },
            { "path": "c64", "entries": [] }
        ]);
        assert!(matches!(
            ViewManifest::from_json(&serde_json::to_vec(&json)?),
            Err(ViewManifestError::DuplicateGroupPath { path })
                if path == LogicalPath::new("c64")
        ));
        Ok(())
    }

    #[test]
    fn imported_group_diagnostics_must_match_layout_validation() -> Result<(), ViewManifestError> {
        let roots = [SetName::new("set")];
        let expected = manifest_from_plan(
            &roots,
            vec![OutputGroup {
                path: LogicalPath::new("../escape"),
                entries: Vec::new(),
            }],
            Vec::new(),
        );

        assert!(matches!(
            ViewManifest::from_json(&expected.to_json()?),
            Err(ViewManifestError::InconsistentOutputDiagnostics)
        ));
        Ok(())
    }

    #[test]
    fn valid_output_diagnostics_round_trip_in_manifest_order() -> Result<(), ViewManifestError> {
        let groups = vec![
            OutputGroup {
                path: LogicalPath::new("A"),
                entries: Vec::new(),
            },
            OutputGroup {
                path: LogicalPath::new("a"),
                entries: Vec::new(),
            },
            OutputGroup {
                path: LogicalPath::new("z/../bad"),
                entries: Vec::new(),
            },
        ];
        let diagnostics = crate::build::validation::inspect_groups(&groups)
            .into_iter()
            .map(|issue| MameLayoutDiagnostic::OutputValidation { issue })
            .collect();
        let roots = [
            SetName::new("A"),
            SetName::new("a"),
            SetName::new("z/../bad"),
        ];
        let planned = ViewManifest::from_layout(
            roots,
            MameLayoutPlan {
                policy: MameSetLayoutPolicy::NonMerged,
                snapshot: snapshot("snapshot"),
                groups,
                coalesced_provenance: Vec::new(),
                diagnostics,
            },
        );

        assert_eq!(ViewManifest::from_json(&planned.to_json()?)?, planned);
        Ok(())
    }

    #[test]
    fn unknown_fields_are_rejected_at_envelope_and_nested_levels()
    -> Result<(), Box<dyn std::error::Error>> {
        let canonical =
            std::str::from_utf8(&manifest(&snapshot("snapshot")).to_json()?)?.to_owned();
        let envelope = canonical.replacen('{', "{\"future\":true,", 1);
        assert!(matches!(
            ViewManifest::from_json(envelope.as_bytes()),
            Err(ViewManifestError::NonCanonicalEncoding)
        ));

        let nested = canonical.replace(
            "\"groups\":[]",
            "\"groups\":[{\"path\":\"set\",\"entries\":[],\"future\":true}]",
        );
        assert!(matches!(
            ViewManifest::from_json(nested.as_bytes()),
            Err(ViewManifestError::NonCanonicalEncoding)
        ));
        Ok(())
    }

    #[test]
    fn provenance_must_reference_a_coalesced_output_entry() -> Result<(), ViewManifestError> {
        let roots = [SetName::new("set")];
        let selected_entry = entry();
        let record = CoalescedAssetProvenance {
            group: LogicalPath::new("missing"),
            path: LogicalPath::new("same.rom"),
            requirements: vec![coalesced_requirement(&selected_entry)],
        };
        let expected = manifest_from_plan(&roots, Vec::new(), vec![record]);

        assert!(matches!(
            ViewManifest::from_json(&expected.to_json()?),
            Err(ViewManifestError::InvalidCoalescedProvenance)
        ));
        Ok(())
    }

    #[test]
    fn provenance_requirements_must_be_canonical() -> Result<(), Box<dyn std::error::Error>> {
        let roots = [SetName::new("set")];
        let selected_entry = entry();
        let second = RequirementKey::new(SetKey::new(CatalogKey::new("mame"), "set"), "other.rom");
        let mut requirements = vec![
            coalesced_requirement(&selected_entry),
            crate::build::mame_layout::CoalescedRequirementEvidence {
                requirement: second,
                expected: selected_entry.expected.clone(),
            },
        ];
        requirements.sort();
        let record = CoalescedAssetProvenance {
            group: LogicalPath::new("set"),
            path: LogicalPath::new("same.rom"),
            requirements,
        };
        let expected = manifest_from_plan(
            &roots,
            vec![OutputGroup {
                path: LogicalPath::new("set"),
                entries: vec![selected_entry],
            }],
            vec![record],
        );
        let canonical = expected.to_json()?;
        let mut reordered: serde_json::Value = serde_json::from_slice(&canonical)?;
        reordered["manifest"]["layout"]["coalesced_provenance"][0]["requirements"]
            .as_array_mut()
            .ok_or_else(|| std::io::Error::other("requirements must be an array"))?
            .reverse();
        assert!(matches!(
            ViewManifest::from_json(&serde_json::to_vec(&reordered)?),
            Err(ViewManifestError::NonCanonicalOrdering)
        ));

        let mut duplicated: serde_json::Value = serde_json::from_slice(&canonical)?;
        let requirements =
            duplicated["manifest"]["layout"]["coalesced_provenance"][0]["requirements"]
                .as_array_mut()
                .ok_or_else(|| std::io::Error::other("requirements must be an array"))?;
        requirements.push(requirements[0].clone());
        assert!(matches!(
            ViewManifest::from_json(&serde_json::to_vec(&duplicated)?),
            Err(ViewManifestError::NonCanonicalOrdering)
        ));
        Ok(())
    }

    #[test]
    fn every_coalesced_requirement_must_share_whole_asset_evidence() -> Result<(), ViewManifestError>
    {
        let roots = [SetName::new("set")];
        let selected_entry = entry();
        let mut requirements = vec![
            coalesced_requirement(&selected_entry),
            crate::build::mame_layout::CoalescedRequirementEvidence {
                requirement: RequirementKey::new(
                    SetKey::new(CatalogKey::new("mame"), "set"),
                    "other.rom",
                ),
                expected: selected_entry.expected.clone(),
            },
        ];
        requirements.sort();
        let record = CoalescedAssetProvenance {
            group: LogicalPath::new("set"),
            path: LogicalPath::new("same.rom"),
            requirements,
        };
        let planned = manifest_from_plan(
            &roots,
            vec![OutputGroup {
                path: LogicalPath::new("set"),
                entries: vec![selected_entry],
            }],
            vec![record],
        );
        let mut forged: serde_json::Value = serde_json::from_slice(&planned.to_json()?)?;
        forged["manifest"]["layout"]["coalesced_provenance"][0]["requirements"][1]["expected"]["sha1"] =
            serde_json::json!(vec![2; 20]);

        let result = ViewManifest::from_json(&serde_json::to_vec(&forged)?);
        assert!(
            matches!(&result, Err(ViewManifestError::InvalidCoalescedProvenance)),
            "unexpected decode result: {result:?}"
        );
        Ok(())
    }

    #[test]
    fn coalesced_provenance_requires_exact_selected_evidence() -> Result<(), ViewManifestError> {
        let roots = [SetName::new("set")];
        let selected_entry = entry();
        let mut selected_evidence = coalesced_requirement(&selected_entry);
        selected_evidence.expected.size = Some(10);
        let mut requirements = vec![
            selected_evidence,
            crate::build::mame_layout::CoalescedRequirementEvidence {
                requirement: RequirementKey::new(
                    SetKey::new(CatalogKey::new("mame"), "set"),
                    "other.rom",
                ),
                expected: selected_entry.expected.clone(),
            },
        ];
        requirements.sort();
        let record = CoalescedAssetProvenance {
            group: LogicalPath::new("set"),
            path: LogicalPath::new("same.rom"),
            requirements,
        };
        let planned = manifest_from_plan(
            &roots,
            vec![OutputGroup {
                path: LogicalPath::new("set"),
                entries: vec![selected_entry],
            }],
            vec![record],
        );

        assert!(matches!(
            ViewManifest::from_json(&planned.to_json()?),
            Err(ViewManifestError::InvalidCoalescedProvenance)
        ));
        Ok(())
    }

    #[test]
    fn coalesced_provenance_rejects_duplicate_identity_keys() -> Result<(), ViewManifestError> {
        let roots = [SetName::new("set")];
        let selected_entry = entry();
        let other = crate::build::mame_layout::CoalescedRequirementEvidence {
            requirement: RequirementKey::new(
                SetKey::new(CatalogKey::new("mame"), "set"),
                "other.rom",
            ),
            expected: selected_entry.expected.clone(),
        };
        let mut requirements = vec![coalesced_requirement(&selected_entry), other.clone()];
        requirements.sort();
        let record = CoalescedAssetProvenance {
            group: LogicalPath::new("set"),
            path: LogicalPath::new("same.rom"),
            requirements,
        };
        let conflicting_record = CoalescedAssetProvenance {
            requirements: vec![coalesced_requirement(&selected_entry), {
                let mut changed = other;
                changed.expected.size = Some(10);
                changed
            }],
            ..record.clone()
        };
        let duplicate_records =
            manifest_from_plan(&roots, Vec::new(), vec![record.clone(), conflicting_record]);
        assert!(matches!(
            ViewManifest::from_json(&duplicate_records.to_json()?),
            Err(ViewManifestError::NonCanonicalOrdering)
        ));

        let duplicate_requirement = CoalescedAssetProvenance {
            requirements: vec![coalesced_requirement(&selected_entry), {
                let mut changed = coalesced_requirement(&selected_entry);
                changed.expected.size = Some(10);
                changed
            }],
            ..record
        };
        let duplicate_requirement =
            manifest_from_plan(&roots, Vec::new(), vec![duplicate_requirement]);
        assert!(matches!(
            ViewManifest::from_json(&duplicate_requirement.to_json()?),
            Err(ViewManifestError::NonCanonicalOrdering)
        ));
        Ok(())
    }

    #[test]
    fn duplicate_entries_are_rejected() -> Result<(), ViewManifestError> {
        let roots = [SetName::new("set")];
        let expected = manifest_from_plan(
            &roots,
            vec![OutputGroup {
                path: LogicalPath::new("set"),
                entries: vec![entry(), entry()],
            }],
            Vec::new(),
        );
        assert!(matches!(
            ViewManifest::from_json(&expected.to_json()?),
            Err(ViewManifestError::NonCanonicalOrdering)
        ));
        Ok(())
    }

    #[test]
    fn duplicate_provenance_records_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let roots = [SetName::new("set")];
        let selected_entry = entry();
        let record = CoalescedAssetProvenance {
            group: LogicalPath::new("set"),
            path: LogicalPath::new("same.rom"),
            requirements: vec![coalesced_requirement(&selected_entry)],
        };
        let expected = manifest_from_plan(&roots, Vec::new(), vec![record.clone(), record.clone()]);
        assert!(matches!(
            ViewManifest::from_json(&expected.to_json()?),
            Err(ViewManifestError::NonCanonicalOrdering)
        ));

        let mut ordered_records: serde_json::Value = serde_json::from_slice(
            &manifest_from_plan(
                &[SetName::new("set")],
                Vec::new(),
                vec![
                    CoalescedAssetProvenance {
                        group: LogicalPath::new("a"),
                        ..record.clone()
                    },
                    CoalescedAssetProvenance {
                        group: LogicalPath::new("z"),
                        ..record
                    },
                ],
            )
            .to_json()?,
        )?;
        ordered_records["manifest"]["layout"]["coalesced_provenance"]
            .as_array_mut()
            .ok_or_else(|| std::io::Error::other("provenance must be an array"))?
            .reverse();
        assert!(matches!(
            ViewManifest::from_json(&serde_json::to_vec(&ordered_records)?),
            Err(ViewManifestError::NonCanonicalOrdering)
        ));
        Ok(())
    }

    #[test]
    fn unsupported_policy_revisions_are_rejected() -> Result<(), ViewManifestError> {
        for policy in [
            "dependency",
            "layout",
            "naming",
            "representation",
            "evidence",
        ] {
            let mut json: serde_json::Value =
                serde_json::from_slice(&manifest(&snapshot("snapshot")).to_json()?)?;
            json["manifest"]["policy_versions"][policy] = serde_json::json!(2);
            assert!(matches!(
                ViewManifest::from_json(&serde_json::to_vec(&json)?),
                Err(ViewManifestError::UnsupportedPolicyVersion {
                    policy: unsupported,
                    version: 2
                }) if unsupported == policy
            ));
        }
        Ok(())
    }

    #[test]
    fn a_new_snapshot_does_not_retarget_an_existing_manifest() {
        let old_snapshot = snapshot("old");
        let pinned = manifest(&old_snapshot);
        let _newer_import_view = manifest(&snapshot("new"));
        assert_eq!(pinned.snapshot(), &old_snapshot);
    }

    #[test]
    fn zip_output_is_not_a_pure_layout_or_manifest_policy() -> Result<(), ViewManifestError> {
        let json = manifest(&snapshot("snapshot")).to_json()?;
        assert!(!json.windows(b"zip".len()).any(|window| window == b"zip"));
        assert!(
            !json
                .windows(b"directory_output".len())
                .any(|window| window == b"directory_output")
        );
        Ok(())
    }

    #[test]
    fn roots_are_canonicalized_without_changing_snapshot_identity() {
        let snapshot = snapshot("snapshot");
        let roots = [SetName::new("nes"), SetName::new("c64")];
        let reversed_roots = roots.iter().rev().cloned().collect::<Vec<_>>();
        let closures = [];
        let sets = [];
        let forward = plan_view(TargetViewRequest::mame_0289(
            &snapshot,
            &roots,
            MameSetLayoutPolicy::NonMerged,
            &closures,
            &sets,
        ));
        let reverse = plan_view(TargetViewRequest::mame_0289(
            &snapshot,
            &reversed_roots,
            MameSetLayoutPolicy::NonMerged,
            &closures,
            &sets,
        ));
        assert_eq!(forward, reverse);
    }

    #[test]
    fn canonical_entry_order_includes_expected_evidence() -> Result<(), ViewManifestError> {
        let selected_roots = [SetName::new("set")];
        let mut expected_variant = entry();
        expected_variant.expected.sha1 = Some([2; 20]);
        let plan = MameLayoutPlan {
            policy: MameSetLayoutPolicy::NonMerged,
            snapshot: snapshot("snapshot"),
            groups: vec![OutputGroup {
                path: LogicalPath::new("set"),
                entries: vec![entry(), expected_variant],
            }],
            coalesced_provenance: Vec::new(),
            diagnostics: BTreeSet::new(),
        };
        let mut reversed_plan = plan.clone();
        reversed_plan.groups[0].entries.reverse();
        let forward = ViewManifest::from_layout(selected_roots.iter().cloned(), plan);
        let reverse = ViewManifest::from_layout(selected_roots.iter().cloned(), reversed_plan);

        assert_eq!(forward.to_json()?, reverse.to_json()?);
        Ok(())
    }

    #[test]
    fn policy_versions_cannot_be_zero() {
        assert_eq!(PolicyVersion::new(0), None);
        assert_eq!(PolicyVersion::new(2).map(PolicyVersion::get), Some(2));
    }
}
