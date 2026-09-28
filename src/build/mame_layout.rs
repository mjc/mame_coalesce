//! Pure MAME layout planning over snapshot-scoped dependencies and resolved assets.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    build::validation::PlanIssue,
    domain::{
        CatalogKey, EvidenceScope, ExpectedEvidence, LogicalEntry, LogicalPath, OutputGroup,
        RequirementKey, SetName, SnapshotKey,
    },
    machine_dependencies::{DependencyClosure, DependencyDiagnostic},
};

/// Explicit MAME archive policy.
///
/// Semantics follow the [MAME ROM-set documentation](https://docs.mamedev.org/usingmame/aboutromsets.html#parents-clones-splitting-and-merging).
/// Legacy build modes intentionally remain separate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MameSetLayoutPolicy {
    /// Store a parent and the selected clones together under the parent's set name.
    Merged,
    /// Store each required set separately, omitting only proven inherited clone assets.
    Split,
    /// Put a selected machine and its runtime dependency closure in one self-contained group.
    NonMerged,
}

/// Resolved content for one set in one immutable catalog snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedMachineSet {
    pub snapshot: SnapshotKey,
    /// Catalog identity used by every contained requirement key.
    pub catalog: CatalogKey,
    pub name: SetName,
    /// Preserved source assertion; it does not add the parent to runtime closure.
    pub parent_clone: Option<SetName>,
    pub entries: Vec<ResolvedMachineAsset>,
    /// Requirements that did not resolve to local content.
    pub missing_assets: Vec<MissingMachineAsset>,
}

/// Exact immutable identity of one asset requirement in a catalog snapshot.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AssetRequirementIdentity {
    pub snapshot: SnapshotKey,
    pub set: SetName,
    pub asset: String,
    pub component_order: i64,
}

/// Resolved bytes plus their source component identity and optional typed merge assertion.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedMachineAsset {
    pub identity: AssetRequirementIdentity,
    pub entry: LogicalEntry,
    pub merge_target: Option<AssetRequirementIdentity>,
}

/// An unresolved requirement retains the same component identity as resolved assets.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingMachineAsset {
    pub identity: AssetRequirementIdentity,
    pub requirement: RequirementKey,
    #[serde(default)]
    pub expected: ExpectedEvidence,
    #[serde(default)]
    pub merge_target: Option<AssetRequirementIdentity>,
}

impl ResolvedMachineAsset {
    #[must_use]
    pub const fn new(identity: AssetRequirementIdentity, entry: LogicalEntry) -> Self {
        Self {
            identity,
            entry,
            merge_target: None,
        }
    }
}

impl AssetRequirementIdentity {
    #[must_use]
    pub fn new(
        snapshot: SnapshotKey,
        set: SetName,
        asset: impl Into<String>,
        component_order: i64,
    ) -> Self {
        Self {
            snapshot,
            set,
            asset: asset.into(),
            component_order,
        }
    }
}

impl ResolvedMachineSet {
    #[must_use]
    pub const fn new(snapshot: SnapshotKey, catalog: CatalogKey, name: SetName) -> Self {
        Self {
            snapshot,
            catalog,
            name,
            parent_clone: None,
            entries: Vec::new(),
            missing_assets: Vec::new(),
        }
    }
}

/// Why a non-merged group cannot safely contain two requirements at one path.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PathCollisionEvidence {
    DifferentSha1,
    ConflictingExpectedEvidence,
    InsufficientSha1,
    CaseInsensitivePath,
}

/// Expected-content fields that contradict one another.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentEvidenceConflict {
    Scope,
    Size,
    Crc,
    Md5,
    Sha1,
    InsufficientMatchEvidence,
    ConflictingSiblingDeclaration,
}

/// A conflict or incomplete input discovered while planning a MAME layout.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MameLayoutDiagnostic {
    SnapshotMismatch {
        root: SetName,
        expected: SnapshotKey,
        actual: SnapshotKey,
    },
    ResolvedSetSnapshotMismatch {
        root: SetName,
        set: SetName,
        expected: SnapshotKey,
        actual: SnapshotKey,
    },
    ResolvedSetCatalogMismatch {
        root: SetName,
        set: SetName,
        expected: CatalogKey,
        actual: CatalogKey,
    },
    Dependency {
        root: SetName,
        diagnostic: DependencyDiagnostic,
    },
    MissingClosure {
        root: SetName,
    },
    AmbiguousClosure {
        root: SetName,
        matches: usize,
    },
    DuplicateRoot {
        root: SetName,
    },
    MissingResolvedSet {
        snapshot: SnapshotKey,
        set: SetName,
        required_by: SetName,
    },
    AmbiguousResolvedSet {
        snapshot: SnapshotKey,
        set: SetName,
        matches: usize,
        required_by: SetName,
    },
    MissingCloneParent {
        root: SetName,
        child: SetName,
        parent: SetName,
    },
    MissingMergeTarget {
        root: SetName,
        child: RequirementKey,
        target: LogicalPath,
    },
    AmbiguousMergeTarget {
        root: SetName,
        child: RequirementKey,
        target: LogicalPath,
    },
    InvalidAssetIdentity {
        root: SetName,
        set: SetName,
        identity: AssetRequirementIdentity,
        requirement: RequirementKey,
    },
    DuplicateAssetIdentity {
        root: SetName,
        set: SetName,
        identity: AssetRequirementIdentity,
    },
    MergeContentMismatch {
        root: SetName,
        child: RequirementKey,
        target: LogicalPath,
        conflicts: Vec<ContentEvidenceConflict>,
    },
    AmbiguousCloneParent {
        root: SetName,
        child: SetName,
        parent: SetName,
        matches: usize,
    },
    CloneCycle {
        root: SetName,
        sets: Vec<SetName>,
    },
    MissingAssets {
        root: SetName,
        set: SetName,
        requirements: Vec<RequirementKey>,
    },
    RequirementSetMismatch {
        root: SetName,
        expected_set: SetName,
        actual_set: SetName,
        requirement: RequirementKey,
    },
    LogicalPathCollision {
        root: SetName,
        path: LogicalPath,
        existing: RequirementKey,
        incoming: RequirementKey,
        evidence: PathCollisionEvidence,
    },
    OutputValidation {
        issue: PlanIssue,
    },
}

impl MameLayoutDiagnostic {
    /// Return the selected machine root this diagnostic belongs to, when applicable.
    #[must_use]
    pub const fn selected_root(&self) -> Option<&SetName> {
        match self {
            Self::SnapshotMismatch { .. } | Self::OutputValidation { .. } => None,
            Self::ResolvedSetSnapshotMismatch { root, .. }
            | Self::ResolvedSetCatalogMismatch { root, .. }
            | Self::Dependency { root, .. }
            | Self::MissingClosure { root }
            | Self::AmbiguousClosure { root, .. }
            | Self::DuplicateRoot { root }
            | Self::MissingCloneParent { root, .. }
            | Self::MissingMergeTarget { root, .. }
            | Self::AmbiguousMergeTarget { root, .. }
            | Self::InvalidAssetIdentity { root, .. }
            | Self::DuplicateAssetIdentity { root, .. }
            | Self::MergeContentMismatch { root, .. }
            | Self::AmbiguousCloneParent { root, .. }
            | Self::CloneCycle { root, .. }
            | Self::MissingAssets { root, .. }
            | Self::RequirementSetMismatch { root, .. }
            | Self::LogicalPathCollision { root, .. } => Some(root),
            Self::MissingResolvedSet { required_by, .. }
            | Self::AmbiguousResolvedSet { required_by, .. } => Some(required_by),
        }
    }
}

/// Multiple catalog requirements represented by one established identical output entry.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CoalescedAssetProvenance {
    pub group: LogicalPath,
    pub path: LogicalPath,
    pub requirements: Vec<CoalescedRequirementEvidence>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CoalescedRequirementEvidence {
    pub requirement: RequirementKey,
    pub expected: ExpectedEvidence,
}

impl CoalescedAssetProvenance {
    pub(crate) fn is_consistent_with(
        &self,
        entry: &LogicalEntry,
        catalog: &crate::domain::CatalogKey,
    ) -> bool {
        if self.requirements.len() < 2 {
            return false;
        }
        let Some(selected_evidence) = self
            .requirements
            .iter()
            .find(|evidence| evidence.requirement == entry.requirement)
        else {
            return false;
        };
        if selected_evidence.expected != entry.expected {
            return false;
        }

        let mut summary = EvidenceSummary::from_expected(&entry.expected);
        for evidence in &self.requirements {
            if evidence.requirement.set().catalog() != catalog
                || !summary.supports_sha1_coalescing(&evidence.expected)
                || !summary.conflicts_with(&evidence.expected).is_empty()
            {
                return false;
            }
            summary.include(&evidence.expected);
        }
        true
    }
}

/// Format-neutral MAME set groups, coalescing provenance, and explicit diagnostics.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MameLayoutPlan {
    pub policy: MameSetLayoutPolicy,
    pub snapshot: SnapshotKey,
    pub groups: Vec<OutputGroup>,
    pub coalesced_provenance: Vec<CoalescedAssetProvenance>,
    pub diagnostics: BTreeSet<MameLayoutDiagnostic>,
}

impl MameLayoutPlan {
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

/// Plan MAME set groups according to the selected merged, split, or non-merged policy.
///
/// The closure carries source-backed relationship diagnostics. `sets` supplies only resolved
/// content and catalog assertions; this function performs no database, filesystem, or archive I/O.
#[must_use]
pub fn plan_mame_layout(
    policy: MameSetLayoutPolicy,
    snapshot: &SnapshotKey,
    roots: &[SetName],
    closures: &[DependencyClosure],
    sets: &[ResolvedMachineSet],
) -> MameLayoutPlan {
    let mut plan = MameLayoutPlan {
        policy,
        snapshot: snapshot.clone(),
        groups: Vec::new(),
        coalesced_provenance: Vec::new(),
        diagnostics: BTreeSet::new(),
    };
    let mut closures_by_root = BTreeMap::<SetName, Vec<&DependencyClosure>>::new();
    for closure in closures {
        if &closure.snapshot != snapshot {
            plan.diagnostics
                .insert(MameLayoutDiagnostic::SnapshotMismatch {
                    root: closure.root.clone(),
                    expected: snapshot.clone(),
                    actual: closure.snapshot.clone(),
                });
            continue;
        }
        closures_by_root
            .entry(closure.root.clone())
            .or_default()
            .push(closure);
    }
    let mut sets_by_key = BTreeMap::<(SnapshotKey, SetName), Vec<&ResolvedMachineSet>>::new();
    let mut sets_by_name = BTreeMap::<SetName, Vec<&ResolvedMachineSet>>::new();
    for set in sets {
        sets_by_key
            .entry((set.snapshot.clone(), set.name.clone()))
            .or_default()
            .push(set);
        sets_by_name.entry(set.name.clone()).or_default().push(set);
    }

    let mut seen_roots = BTreeSet::new();
    for root in roots {
        if !seen_roots.insert(root.clone()) {
            plan.diagnostics
                .insert(MameLayoutDiagnostic::DuplicateRoot { root: root.clone() });
        }
    }
    let mut grouped_entries = BTreeMap::<LogicalPath, Vec<(SetName, LogicalEntry)>>::new();
    for root in &seen_roots {
        let Some(root_closures) = closures_by_root.get(root) else {
            plan.diagnostics
                .insert(MameLayoutDiagnostic::MissingClosure { root: root.clone() });
            continue;
        };
        if root_closures.len() != 1 {
            plan.diagnostics
                .insert(MameLayoutDiagnostic::AmbiguousClosure {
                    root: root.clone(),
                    matches: root_closures.len(),
                });
            continue;
        }
        let closure = root_closures[0];
        match policy {
            MameSetLayoutPolicy::NonMerged => {
                let group = plan_root_group(root, closure, &sets_by_key, &sets_by_name, &mut plan);
                plan.groups.push(group);
            }
            MameSetLayoutPolicy::Split | MameSetLayoutPolicy::Merged => {
                plan_set_groups_for_root(
                    policy,
                    root,
                    closure,
                    &sets_by_key,
                    &sets_by_name,
                    &mut grouped_entries,
                    &mut plan,
                );
            }
        }
    }
    if policy != MameSetLayoutPolicy::NonMerged {
        finish_grouped_entries(grouped_entries, &mut plan);
    }
    plan.groups
        .sort_by(|left, right| left.path.cmp(&right.path));
    plan.diagnostics.extend(
        crate::build::validation::inspect_groups(&plan.groups)
            .into_iter()
            .map(|issue| MameLayoutDiagnostic::OutputValidation { issue }),
    );
    plan.coalesced_provenance.sort();
    plan
}

fn plan_set_groups_for_root(
    policy: MameSetLayoutPolicy,
    root: &SetName,
    closure: &DependencyClosure,
    sets_by_key: &BTreeMap<(SnapshotKey, SetName), Vec<&ResolvedMachineSet>>,
    sets_by_name: &BTreeMap<SetName, Vec<&ResolvedMachineSet>>,
    grouped_entries: &mut BTreeMap<LogicalPath, Vec<(SetName, LogicalEntry)>>,
    plan: &mut MameLayoutPlan,
) {
    for diagnostic in &closure.diagnostics {
        plan.diagnostics.insert(MameLayoutDiagnostic::Dependency {
            root: root.clone(),
            diagnostic: diagnostic.clone(),
        });
    }
    let expected_catalog = sets_by_key
        .get(&(closure.snapshot.clone(), root.clone()))
        .filter(|matches| matches.len() == 1)
        .map(|matches| matches[0].catalog.clone());

    for set_name in &closure.sets {
        let key = (closure.snapshot.clone(), set_name.clone());
        let Some(matches) = sets_by_key.get(&key) else {
            report_missing_set(root, set_name, &closure.snapshot, sets_by_name, plan);
            continue;
        };
        if matches.len() != 1 {
            plan.diagnostics
                .insert(MameLayoutDiagnostic::AmbiguousResolvedSet {
                    snapshot: closure.snapshot.clone(),
                    set: set_name.clone(),
                    matches: matches.len(),
                    required_by: root.clone(),
                });
            continue;
        }
        let resolved_set = matches[0];
        if let Some(expected_catalog) = &expected_catalog
            && &resolved_set.catalog != expected_catalog
        {
            plan.diagnostics
                .insert(MameLayoutDiagnostic::ResolvedSetCatalogMismatch {
                    root: root.clone(),
                    set: set_name.clone(),
                    expected: expected_catalog.clone(),
                    actual: resolved_set.catalog.clone(),
                });
            continue;
        }

        let lineage = resolve_clone_lineage(root, resolved_set, sets_by_key, sets_by_name, plan);
        let Some(base_set) = lineage.last() else {
            continue;
        };
        let family_path = base_set.name.clone();
        for (member, entries) in materialize_layout_lineage(root, lineage, plan) {
            let group_path = match policy {
                MameSetLayoutPolicy::Split => LogicalPath::new(member.as_str()),
                MameSetLayoutPolicy::Merged => LogicalPath::new(family_path.as_str()),
                MameSetLayoutPolicy::NonMerged => continue,
            };
            let group = grouped_entries.entry(group_path).or_default();
            group.extend(entries.into_iter().map(|entry| (root.clone(), entry)));
        }
    }
}

fn materialize_layout_lineage(
    root: &SetName,
    lineage: Vec<&ResolvedMachineSet>,
    plan: &mut MameLayoutPlan,
) -> Vec<(SetName, Vec<LogicalEntry>)> {
    let mut inherited_entries = BTreeMap::<LogicalPath, Vec<ResolvedMachineAsset>>::new();
    let mut output = Vec::new();

    for set in lineage.into_iter().rev() {
        let mut seen_component_orders = BTreeSet::new();
        let missing_assets = set
            .missing_assets
            .iter()
            .filter(|missing| {
                validate_asset_identity(
                    root,
                    set,
                    &missing.identity,
                    &missing.requirement,
                    &mut seen_component_orders,
                    &mut plan.diagnostics,
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut requirements = Vec::new();
        for missing in missing_assets {
            if !validate_missing_merge(root, &missing, &inherited_entries, &mut plan.diagnostics) {
                requirements.push(missing.requirement);
            }
        }
        if !requirements.is_empty() {
            requirements.sort();
            plan.diagnostics
                .insert(MameLayoutDiagnostic::MissingAssets {
                    root: root.clone(),
                    set: set.name.clone(),
                    requirements,
                });
        }

        let mut assets = set.entries.clone();
        sort_layer_assets(&mut assets);
        let mut layer = BTreeMap::<LogicalPath, Vec<ResolvedMachineAsset>>::new();
        for resolved_asset in assets {
            let entry = &resolved_asset.entry;
            if !validate_asset_identity(
                root,
                set,
                &resolved_asset.identity,
                &entry.requirement,
                &mut seen_component_orders,
                &mut plan.diagnostics,
            ) {
                continue;
            }
            if entry.requirement.game_name() != set.name.as_str() {
                plan.diagnostics
                    .insert(MameLayoutDiagnostic::RequirementSetMismatch {
                        root: root.clone(),
                        expected_set: set.name.clone(),
                        actual_set: SetName::new(entry.requirement.game_name()),
                        requirement: entry.requirement.clone(),
                    });
                continue;
            }

            match inspect_merge_assertion(
                root,
                &resolved_asset,
                &inherited_entries,
                &mut plan.diagnostics,
            ) {
                MergeAssertionResolution::Valid => {}
                MergeAssertionResolution::NotDeclared | MergeAssertionResolution::Invalid => {
                    layer
                        .entry(entry.path.clone())
                        .or_default()
                        .push(resolved_asset);
                }
            }
        }

        for assets in layer.values_mut() {
            sort_layer_assets(assets);
        }
        let layer_entries = layer
            .values()
            .flatten()
            .map(|asset| asset.entry.clone())
            .collect();
        for (path, assets) in layer {
            inherited_entries.entry(path).or_default().extend(assets);
        }
        output.push((set.name.clone(), layer_entries));
    }

    output
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum MergeAssertionResolution {
    NotDeclared,
    Valid,
    Invalid,
}

fn validate_missing_merge(
    root: &SetName,
    missing: &MissingMachineAsset,
    inherited_entries: &BTreeMap<LogicalPath, Vec<ResolvedMachineAsset>>,
    diagnostics: &mut BTreeSet<MameLayoutDiagnostic>,
) -> bool {
    let Some(merged_name) = &missing.expected.merge else {
        return false;
    };
    let target = LogicalPath::new(merged_name);
    let Some(inherited) = inherited_entries.get(&target) else {
        diagnostics.insert(MameLayoutDiagnostic::MissingMergeTarget {
            root: root.clone(),
            child: missing.requirement.clone(),
            target,
        });
        return false;
    };
    let target_asset = missing.merge_target.as_ref().map_or_else(
        || (inherited.len() == 1).then(|| &inherited[0]),
        |asserted_target| {
            inherited
                .iter()
                .find(|candidate| candidate.identity == *asserted_target)
        },
    );
    let Some(target_asset) = target_asset else {
        if missing.merge_target.is_none() && inherited.len() > 1 {
            diagnostics.insert(MameLayoutDiagnostic::AmbiguousMergeTarget {
                root: root.clone(),
                child: missing.requirement.clone(),
                target,
            });
        } else {
            diagnostics.insert(MameLayoutDiagnostic::MissingMergeTarget {
                root: root.clone(),
                child: missing.requirement.clone(),
                target,
            });
        }
        return false;
    };

    let mut conflicts = content_evidence_conflicts(&target_asset.entry.expected, &missing.expected);
    if !has_matching_content_digest(&target_asset.entry.expected, &missing.expected) {
        conflicts.push(ContentEvidenceConflict::InsufficientMatchEvidence);
    }
    if !conflicts.is_empty() {
        conflicts.sort();
        conflicts.dedup();
        diagnostics.insert(MameLayoutDiagnostic::MergeContentMismatch {
            root: root.clone(),
            child: missing.requirement.clone(),
            target,
            conflicts,
        });
        return false;
    }
    true
}

fn inspect_merge_assertion(
    root: &SetName,
    resolved_asset: &ResolvedMachineAsset,
    inherited_entries: &BTreeMap<LogicalPath, Vec<ResolvedMachineAsset>>,
    diagnostics: &mut BTreeSet<MameLayoutDiagnostic>,
) -> MergeAssertionResolution {
    let entry = &resolved_asset.entry;
    let Some(merged_name) = &entry.expected.merge else {
        return MergeAssertionResolution::NotDeclared;
    };
    let target = LogicalPath::new(merged_name);
    let Some(inherited) = inherited_entries.get(&target) else {
        diagnostics.insert(MameLayoutDiagnostic::MissingMergeTarget {
            root: root.clone(),
            child: entry.requirement.clone(),
            target,
        });
        return MergeAssertionResolution::Invalid;
    };
    let target_asset = resolved_asset.merge_target.as_ref().map_or_else(
        || (inherited.len() == 1).then(|| &inherited[0]),
        |asserted_target| {
            inherited
                .iter()
                .find(|candidate| candidate.identity == *asserted_target)
        },
    );
    let Some(target_asset) = target_asset else {
        if resolved_asset.merge_target.is_none() && inherited.len() > 1 {
            diagnostics.insert(MameLayoutDiagnostic::AmbiguousMergeTarget {
                root: root.clone(),
                child: entry.requirement.clone(),
                target,
            });
        } else {
            diagnostics.insert(MameLayoutDiagnostic::MissingMergeTarget {
                root: root.clone(),
                child: entry.requirement.clone(),
                target,
            });
        }
        return MergeAssertionResolution::Invalid;
    };

    let mut conflicts: BTreeSet<_> =
        content_evidence_conflicts(&target_asset.entry.expected, &entry.expected)
            .into_iter()
            .collect();
    let has_matching_identity =
        has_matching_content_digest(&target_asset.entry.expected, &entry.expected);
    if !has_matching_identity {
        conflicts.insert(ContentEvidenceConflict::InsufficientMatchEvidence);
    }
    if !conflicts.is_empty() {
        diagnostics.insert(MameLayoutDiagnostic::MergeContentMismatch {
            root: root.clone(),
            child: entry.requirement.clone(),
            target,
            conflicts: conflicts.into_iter().collect(),
        });
        return MergeAssertionResolution::Invalid;
    }
    MergeAssertionResolution::Valid
}

fn finish_grouped_entries(
    grouped_entries: BTreeMap<LogicalPath, Vec<(SetName, LogicalEntry)>>,
    plan: &mut MameLayoutPlan,
) {
    for (group_path, contributions) in grouped_entries {
        let mut entries = BTreeMap::<LogicalPath, LogicalEntry>::new();
        let mut case_insensitive_paths = BTreeMap::<String, LogicalPath>::new();
        let mut expected_evidence_by_path = BTreeMap::<LogicalPath, EvidenceSummary>::new();
        let mut provenance =
            BTreeMap::<LogicalPath, BTreeMap<RequirementKey, ExpectedEvidence>>::new();
        for (root, entry) in contributions {
            insert_entry(
                &root,
                &entry,
                &mut entries,
                &mut case_insensitive_paths,
                &mut expected_evidence_by_path,
                &mut provenance,
                &mut plan.diagnostics,
            );
        }
        plan.coalesced_provenance.extend(
            provenance
                .into_iter()
                .filter(|(_, requirements)| requirements.len() > 1)
                .map(|(path, requirements)| CoalescedAssetProvenance {
                    group: group_path.clone(),
                    path,
                    requirements: requirements
                        .into_iter()
                        .map(|(requirement, expected)| CoalescedRequirementEvidence {
                            requirement,
                            expected,
                        })
                        .collect(),
                }),
        );
        plan.groups.push(OutputGroup {
            path: group_path,
            entries: entries.into_values().collect(),
        });
    }
}

fn plan_root_group(
    root: &SetName,
    closure: &DependencyClosure,
    sets_by_key: &BTreeMap<(SnapshotKey, SetName), Vec<&ResolvedMachineSet>>,
    sets_by_name: &BTreeMap<SetName, Vec<&ResolvedMachineSet>>,
    plan: &mut MameLayoutPlan,
) -> OutputGroup {
    let expected_catalog = sets_by_key
        .get(&(closure.snapshot.clone(), root.clone()))
        .filter(|matches| matches.len() == 1)
        .map(|matches| matches[0].catalog.clone());
    for diagnostic in &closure.diagnostics {
        plan.diagnostics.insert(MameLayoutDiagnostic::Dependency {
            root: root.clone(),
            diagnostic: diagnostic.clone(),
        });
    }

    let group_path = LogicalPath::new(root.as_str());
    let mut entries = BTreeMap::<LogicalPath, LogicalEntry>::new();
    let mut case_insensitive_paths = BTreeMap::<String, LogicalPath>::new();
    let mut expected_evidence_by_path = BTreeMap::<LogicalPath, EvidenceSummary>::new();
    let mut provenance = BTreeMap::<LogicalPath, BTreeMap<RequirementKey, ExpectedEvidence>>::new();
    for set_name in &closure.sets {
        let key = (closure.snapshot.clone(), set_name.clone());
        let Some(matches) = sets_by_key.get(&key) else {
            report_missing_set(root, set_name, &closure.snapshot, sets_by_name, plan);
            continue;
        };
        if matches.len() != 1 {
            plan.diagnostics
                .insert(MameLayoutDiagnostic::AmbiguousResolvedSet {
                    snapshot: closure.snapshot.clone(),
                    set: set_name.clone(),
                    matches: matches.len(),
                    required_by: root.clone(),
                });
            continue;
        }
        let resolved_set = matches[0];
        if let Some(expected_catalog) = &expected_catalog
            && &resolved_set.catalog != expected_catalog
        {
            plan.diagnostics
                .insert(MameLayoutDiagnostic::ResolvedSetCatalogMismatch {
                    root: root.clone(),
                    set: set_name.clone(),
                    expected: expected_catalog.clone(),
                    actual: resolved_set.catalog.clone(),
                });
            continue;
        }
        for entry in effective_set_entries(root, resolved_set, sets_by_key, sets_by_name, plan) {
            insert_entry(
                root,
                &entry,
                &mut entries,
                &mut case_insensitive_paths,
                &mut expected_evidence_by_path,
                &mut provenance,
                &mut plan.diagnostics,
            );
        }
    }

    plan.coalesced_provenance.extend(
        provenance
            .into_iter()
            .filter(|(_, requirements)| requirements.len() > 1)
            .map(|(path, requirements)| CoalescedAssetProvenance {
                group: group_path.clone(),
                path,
                requirements: requirements
                    .into_iter()
                    .map(|(requirement, expected)| CoalescedRequirementEvidence {
                        requirement,
                        expected,
                    })
                    .collect(),
            }),
    );
    OutputGroup {
        path: group_path,
        entries: entries.into_values().collect(),
    }
}

fn report_missing_set(
    root: &SetName,
    set: &SetName,
    expected: &SnapshotKey,
    sets_by_name: &BTreeMap<SetName, Vec<&ResolvedMachineSet>>,
    plan: &mut MameLayoutPlan,
) {
    if let Some(foreign_matches) = sets_by_name.get(set).filter(|matches| {
        matches
            .iter()
            .any(|resolved| &resolved.snapshot != expected)
    }) {
        for resolved in foreign_matches {
            if &resolved.snapshot != expected {
                plan.diagnostics
                    .insert(MameLayoutDiagnostic::ResolvedSetSnapshotMismatch {
                        root: root.clone(),
                        set: set.clone(),
                        expected: expected.clone(),
                        actual: resolved.snapshot.clone(),
                    });
            }
        }
    } else {
        plan.diagnostics
            .insert(MameLayoutDiagnostic::MissingResolvedSet {
                snapshot: expected.clone(),
                set: set.clone(),
                required_by: root.clone(),
            });
    }
}

fn effective_set_entries(
    root: &SetName,
    selected: &ResolvedMachineSet,
    sets_by_key: &BTreeMap<(SnapshotKey, SetName), Vec<&ResolvedMachineSet>>,
    sets_by_name: &BTreeMap<SetName, Vec<&ResolvedMachineSet>>,
    plan: &mut MameLayoutPlan,
) -> Vec<LogicalEntry> {
    let lineage = resolve_clone_lineage(root, selected, sets_by_key, sets_by_name, plan);
    materialize_clone_lineage(root, lineage, plan)
}

fn resolve_clone_lineage<'set>(
    root: &SetName,
    selected: &'set ResolvedMachineSet,
    sets_by_key: &BTreeMap<(SnapshotKey, SetName), Vec<&'set ResolvedMachineSet>>,
    sets_by_name: &BTreeMap<SetName, Vec<&'set ResolvedMachineSet>>,
    plan: &mut MameLayoutPlan,
) -> Vec<&'set ResolvedMachineSet> {
    let mut lineage = vec![selected];
    let mut seen = BTreeMap::from([(selected.name.clone(), 0)]);
    let mut child = selected;
    while let Some(parent_name) = &child.parent_clone {
        if let Some(cycle_start) = seen.get(parent_name) {
            let mut sets: Vec<_> = lineage[*cycle_start..]
                .iter()
                .map(|set| set.name.clone())
                .collect();
            sets.push(parent_name.clone());
            plan.diagnostics.insert(MameLayoutDiagnostic::CloneCycle {
                root: root.clone(),
                sets,
            });
            break;
        }
        let key = (selected.snapshot.clone(), parent_name.clone());
        let Some(matches) = sets_by_key.get(&key) else {
            if let Some(foreign_matches) = sets_by_name.get(parent_name).filter(|matches| {
                matches
                    .iter()
                    .any(|resolved| resolved.snapshot != selected.snapshot)
            }) {
                for resolved in foreign_matches {
                    if resolved.snapshot != selected.snapshot {
                        plan.diagnostics.insert(
                            MameLayoutDiagnostic::ResolvedSetSnapshotMismatch {
                                root: root.clone(),
                                set: parent_name.clone(),
                                expected: selected.snapshot.clone(),
                                actual: resolved.snapshot.clone(),
                            },
                        );
                    }
                }
            } else {
                plan.diagnostics
                    .insert(MameLayoutDiagnostic::MissingCloneParent {
                        root: root.clone(),
                        child: child.name.clone(),
                        parent: parent_name.clone(),
                    });
            }
            break;
        };
        if matches.len() != 1 {
            plan.diagnostics
                .insert(MameLayoutDiagnostic::AmbiguousCloneParent {
                    root: root.clone(),
                    child: child.name.clone(),
                    parent: parent_name.clone(),
                    matches: matches.len(),
                });
            break;
        }
        let parent = matches[0];
        if parent.catalog != selected.catalog {
            plan.diagnostics
                .insert(MameLayoutDiagnostic::ResolvedSetCatalogMismatch {
                    root: root.clone(),
                    set: parent_name.clone(),
                    expected: selected.catalog.clone(),
                    actual: parent.catalog.clone(),
                });
            break;
        }
        child = parent;
        seen.insert(child.name.clone(), lineage.len());
        lineage.push(child);
    }

    lineage
}

fn materialize_clone_lineage(
    root: &SetName,
    lineage: Vec<&ResolvedMachineSet>,
    plan: &mut MameLayoutPlan,
) -> Vec<LogicalEntry> {
    let mut entries = BTreeMap::<LogicalPath, Vec<ResolvedMachineAsset>>::new();
    let mut missing_by_set = Vec::<(SetName, Vec<MissingMachineAsset>)>::new();
    let mut replaced_assets = BTreeSet::<AssetRequirementIdentity>::new();
    let mut is_base_set = true;
    for set in lineage.into_iter().rev() {
        let mut layer = BTreeMap::<LogicalPath, Vec<ResolvedMachineAsset>>::new();
        let mut merges = MergeAccumulator::default();
        let mut seen_component_orders = BTreeSet::new();
        let missing_assets = set
            .missing_assets
            .iter()
            .filter(|missing| {
                validate_asset_identity(
                    root,
                    set,
                    &missing.identity,
                    &missing.requirement,
                    &mut seen_component_orders,
                    &mut plan.diagnostics,
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        if !missing_assets.is_empty() {
            missing_by_set.push((set.name.clone(), missing_assets));
        }
        for resolved_asset in &set.entries {
            let entry = &resolved_asset.entry;
            if !validate_asset_identity(
                root,
                set,
                &resolved_asset.identity,
                &entry.requirement,
                &mut seen_component_orders,
                &mut plan.diagnostics,
            ) {
                continue;
            }
            if entry.requirement.game_name() != set.name.as_str() {
                plan.diagnostics
                    .insert(MameLayoutDiagnostic::RequirementSetMismatch {
                        root: root.clone(),
                        expected_set: set.name.clone(),
                        actual_set: SetName::new(entry.requirement.game_name()),
                        requirement: entry.requirement.clone(),
                    });
                continue;
            }
            let preserve_inherited_path = !is_base_set
                && merges.inspect(root, resolved_asset, &entries, &mut plan.diagnostics);
            if preserve_inherited_path {
                continue;
            }
            layer
                .entry(entry.path.clone())
                .or_default()
                .push(resolved_asset.clone());
        }
        merges.apply(
            root,
            &mut entries,
            &mut layer,
            &mut replaced_assets,
            &mut plan.diagnostics,
        );
        for assets in layer.values_mut() {
            sort_layer_assets(assets);
        }
        entries.extend(layer);
        is_base_set = false;
    }
    for (set, missing_assets) in missing_by_set {
        let mut requirements: Vec<_> = missing_assets
            .into_iter()
            .filter(|missing| !replaced_assets.contains(&missing.identity))
            .map(|missing| missing.requirement)
            .collect();
        if !requirements.is_empty() {
            requirements.sort();
            plan.diagnostics
                .insert(MameLayoutDiagnostic::MissingAssets {
                    root: root.clone(),
                    set,
                    requirements,
                });
        }
    }
    entries
        .into_values()
        .flatten()
        .map(|asset| asset.entry)
        .collect()
}

fn validate_asset_identity(
    root: &SetName,
    set: &ResolvedMachineSet,
    identity: &AssetRequirementIdentity,
    requirement: &RequirementKey,
    seen: &mut BTreeSet<(SnapshotKey, SetName, i64)>,
    diagnostics: &mut BTreeSet<MameLayoutDiagnostic>,
) -> bool {
    if identity.snapshot != set.snapshot
        || requirement.set().catalog() != &set.catalog
        || !identity.set.as_str().eq(set.name.as_str())
        || identity.asset != requirement.rom_name()
        || identity.component_order < 0
        || requirement.game_name() != set.name.as_str()
    {
        diagnostics.insert(MameLayoutDiagnostic::InvalidAssetIdentity {
            root: root.clone(),
            set: set.name.clone(),
            identity: identity.clone(),
            requirement: requirement.clone(),
        });
        return false;
    }
    if !seen.insert((
        identity.snapshot.clone(),
        identity.set.clone(),
        identity.component_order,
    )) {
        diagnostics.insert(MameLayoutDiagnostic::DuplicateAssetIdentity {
            root: root.clone(),
            set: set.name.clone(),
            identity: identity.clone(),
        });
        return false;
    }
    true
}

fn sort_layer_assets(assets: &mut [ResolvedMachineAsset]) {
    assets.sort_by(|left, right| {
        left.entry
            .requirement
            .cmp(&right.entry.requirement)
            .then_with(|| left.entry.expected.cmp(&right.entry.expected))
            .then_with(|| {
                left.entry
                    .source
                    .location
                    .priority()
                    .cmp(&right.entry.source.location.priority())
            })
            .then_with(|| left.entry.source.location.cmp(&right.entry.source.location))
            .then_with(|| {
                left.entry
                    .source
                    .source_root
                    .cmp(&right.entry.source.source_root)
            })
            .then_with(|| left.entry.source.observed.cmp(&right.entry.source.observed))
            .then_with(|| {
                left.entry
                    .source
                    .fingerprint
                    .cmp(&right.entry.source.fingerprint)
            })
            .then_with(|| left.entry.source.scan_run.cmp(&right.entry.source.scan_run))
            .then_with(|| {
                left.entry
                    .source
                    .scan_provenance
                    .cmp(&right.entry.source.scan_provenance)
            })
            .then_with(|| {
                left.entry
                    .source
                    .bare_file_cache_stamp
                    .cmp(&right.entry.source.bare_file_cache_stamp)
            })
            .then_with(|| left.entry.selection.cmp(&right.entry.selection))
            .then_with(|| left.identity.cmp(&right.identity))
    });
}

#[derive(Default)]
struct MergeAccumulator {
    accepted: BTreeMap<AssetRequirementIdentity, LogicalPath>,
    rejected: BTreeSet<AssetRequirementIdentity>,
    deferred_entries: BTreeMap<AssetRequirementIdentity, Vec<ResolvedMachineAsset>>,
}

impl MergeAccumulator {
    fn inspect(
        &mut self,
        root: &SetName,
        resolved_asset: &ResolvedMachineAsset,
        inherited_entries: &BTreeMap<LogicalPath, Vec<ResolvedMachineAsset>>,
        diagnostics: &mut BTreeSet<MameLayoutDiagnostic>,
    ) -> bool {
        let entry = &resolved_asset.entry;
        let Some(merged_name) = &entry.expected.merge else {
            return false;
        };
        let target = LogicalPath::new(merged_name);
        let Some(inherited) = inherited_entries.get(&target) else {
            diagnostics.insert(MameLayoutDiagnostic::MissingMergeTarget {
                root: root.clone(),
                child: entry.requirement.clone(),
                target,
            });
            return false;
        };
        let target_asset = resolved_asset.merge_target.as_ref().map_or_else(
            || (inherited.len() == 1).then(|| &inherited[0]),
            |asserted_target| {
                inherited
                    .iter()
                    .find(|candidate| candidate.identity == *asserted_target)
            },
        );
        let Some(target_asset) = target_asset else {
            if resolved_asset.merge_target.is_none() && inherited.len() > 1 {
                diagnostics.insert(MameLayoutDiagnostic::AmbiguousMergeTarget {
                    root: root.clone(),
                    child: entry.requirement.clone(),
                    target,
                });
            } else {
                diagnostics.insert(MameLayoutDiagnostic::MissingMergeTarget {
                    root: root.clone(),
                    child: entry.requirement.clone(),
                    target,
                });
            }
            return false;
        };
        let target_identity = resolved_asset
            .merge_target
            .clone()
            .unwrap_or_else(|| target_asset.identity.clone());
        let mut conflicts: BTreeSet<_> =
            content_evidence_conflicts(&target_asset.entry.expected, &entry.expected)
                .into_iter()
                .collect();
        let has_matching_identity =
            has_matching_content_digest(&target_asset.entry.expected, &entry.expected);
        let same_path = entry.path == target;
        let merge_is_valid = conflicts.is_empty() && has_matching_identity;
        if merge_is_valid {
            self.accepted
                .insert(target_identity.clone(), target.clone());
        } else {
            if !has_matching_identity {
                conflicts.insert(ContentEvidenceConflict::InsufficientMatchEvidence);
            }
            self.rejected.insert(target_identity.clone());
            diagnostics.insert(MameLayoutDiagnostic::MergeContentMismatch {
                root: root.clone(),
                child: entry.requirement.clone(),
                target: target.clone(),
                conflicts: conflicts.into_iter().collect(),
            });
        }
        if same_path && merge_is_valid {
            self.deferred_entries
                .entry(target_identity)
                .or_default()
                .push(resolved_asset.clone());
        }
        same_path
    }

    fn apply(
        self,
        root: &SetName,
        entries: &mut BTreeMap<LogicalPath, Vec<ResolvedMachineAsset>>,
        layer: &mut BTreeMap<LogicalPath, Vec<ResolvedMachineAsset>>,
        replaced_assets: &mut BTreeSet<AssetRequirementIdentity>,
        diagnostics: &mut BTreeSet<MameLayoutDiagnostic>,
    ) {
        for (identity, path) in &self.accepted {
            if self.rejected.contains(identity) {
                continue;
            }
            if let Some(inherited) = entries.get_mut(path) {
                inherited.retain(|asset| asset.identity != *identity);
                if inherited.is_empty() {
                    entries.remove(path);
                }
            }
            replaced_assets.insert(identity.clone());
            if let Some(merged_entries) = self.deferred_entries.get(identity) {
                if let Some(remaining_inherited) = entries.remove(path) {
                    layer
                        .entry(path.clone())
                        .or_default()
                        .extend(remaining_inherited);
                }
                layer
                    .entry(path.clone())
                    .or_default()
                    .extend(merged_entries.iter().cloned());
            }
        }
        for identity in self
            .accepted
            .keys()
            .filter(|identity| self.rejected.contains(*identity))
        {
            if let Some(displaced_entries) = self.deferred_entries.get(identity) {
                for entry in displaced_entries {
                    diagnostics.insert(MameLayoutDiagnostic::MergeContentMismatch {
                        root: root.clone(),
                        child: entry.entry.requirement.clone(),
                        target: self.accepted[identity].clone(),
                        conflicts: vec![ContentEvidenceConflict::ConflictingSiblingDeclaration],
                    });
                }
            }
        }
    }
}

fn content_evidence_conflicts(
    left: &ExpectedEvidence,
    right: &ExpectedEvidence,
) -> Vec<ContentEvidenceConflict> {
    if left.scope != right.scope {
        return vec![ContentEvidenceConflict::Scope];
    }

    let mut conflicts = Vec::new();
    if matches!((left.size, right.size), (Some(left), Some(right)) if left != right) {
        conflicts.push(ContentEvidenceConflict::Size);
    }
    if matches!((left.crc, right.crc), (Some(left), Some(right)) if left != right) {
        conflicts.push(ContentEvidenceConflict::Crc);
    }
    if matches!((left.md5, right.md5), (Some(left), Some(right)) if left != right) {
        conflicts.push(ContentEvidenceConflict::Md5);
    }
    if matches!((left.sha1, right.sha1), (Some(left), Some(right)) if left != right) {
        conflicts.push(ContentEvidenceConflict::Sha1);
    }
    conflicts
}

fn has_matching_content_digest(left: &ExpectedEvidence, right: &ExpectedEvidence) -> bool {
    left.scope == EvidenceScope::WholeAsset
        && right.scope == EvidenceScope::WholeAsset
        && (matches!((left.md5, right.md5), (Some(left), Some(right)) if left == right)
            || matches!((left.sha1, right.sha1), (Some(left), Some(right)) if left == right))
}

#[derive(Clone, Copy)]
struct EvidenceSummary {
    scope: EvidenceScope,
    size: Option<u64>,
    crc: Option<crate::domain::Crc32Digest>,
    md5: Option<crate::domain::Md5Digest>,
    sha1: Option<[u8; 20]>,
}

impl EvidenceSummary {
    const fn from_expected(expected: &ExpectedEvidence) -> Self {
        Self {
            scope: expected.scope,
            size: expected.size,
            crc: expected.crc,
            md5: expected.md5,
            sha1: expected.sha1,
        }
    }

    fn conflicts_with(self, incoming: &ExpectedEvidence) -> Vec<ContentEvidenceConflict> {
        content_evidence_conflicts(
            &ExpectedEvidence {
                scope: self.scope,
                size: self.size,
                crc: self.crc,
                md5: self.md5,
                sha1: self.sha1,
                ..ExpectedEvidence::default()
            },
            incoming,
        )
    }

    fn supports_sha1_coalescing(self, incoming: &ExpectedEvidence) -> bool {
        self.scope == EvidenceScope::WholeAsset
            && incoming.scope == EvidenceScope::WholeAsset
            && self.sha1.is_some()
            && self.sha1 == incoming.sha1
    }

    fn include(&mut self, incoming: &ExpectedEvidence) {
        self.size = self.size.or(incoming.size);
        self.crc = self.crc.or(incoming.crc);
        self.md5 = self.md5.or(incoming.md5);
        self.sha1 = self.sha1.or(incoming.sha1);
    }
}

fn insert_entry(
    root: &SetName,
    incoming: &LogicalEntry,
    entries: &mut BTreeMap<LogicalPath, LogicalEntry>,
    case_insensitive_paths: &mut BTreeMap<String, LogicalPath>,
    expected_evidence_by_path: &mut BTreeMap<LogicalPath, EvidenceSummary>,
    provenance: &mut BTreeMap<LogicalPath, BTreeMap<RequirementKey, ExpectedEvidence>>,
    diagnostics: &mut BTreeSet<MameLayoutDiagnostic>,
) {
    let path = incoming.path.clone();
    let folded_path = path.as_str().to_ascii_lowercase();
    let Some(existing_path) = case_insensitive_paths.get(&folded_path) else {
        case_insensitive_paths.insert(folded_path, path.clone());
        entries.insert(path, incoming.clone());
        expected_evidence_by_path.insert(
            incoming.path.clone(),
            EvidenceSummary::from_expected(&incoming.expected),
        );
        return;
    };
    let Some(existing) = entries.get(existing_path) else {
        case_insensitive_paths.insert(folded_path, path.clone());
        entries.insert(path, incoming.clone());
        expected_evidence_by_path.insert(
            incoming.path.clone(),
            EvidenceSummary::from_expected(&incoming.expected),
        );
        return;
    };
    if existing_path != &path {
        diagnostics.insert(MameLayoutDiagnostic::LogicalPathCollision {
            root: root.clone(),
            path,
            existing: existing.requirement.clone(),
            incoming: incoming.requirement.clone(),
            evidence: PathCollisionEvidence::CaseInsensitivePath,
        });
        return;
    }
    if existing.requirement == incoming.requirement && existing == incoming {
        return;
    }
    let existing_evidence = expected_evidence_by_path
        .get(existing_path)
        .copied()
        .unwrap_or_else(|| EvidenceSummary::from_expected(&existing.expected));
    let evidence_conflicts = existing_evidence.conflicts_with(&incoming.expected);
    if existing_evidence.supports_sha1_coalescing(&incoming.expected)
        && evidence_conflicts.is_empty()
    {
        expected_evidence_by_path
            .entry(existing_path.clone())
            .and_modify(|summary| summary.include(&incoming.expected))
            .or_insert_with(|| EvidenceSummary::from_expected(&incoming.expected));
        if existing.requirement != incoming.requirement {
            let requirements = provenance.entry(existing_path.clone()).or_insert_with(|| {
                BTreeMap::from([(existing.requirement.clone(), existing.expected.clone())])
            });
            requirements
                .entry(incoming.requirement.clone())
                .or_insert_with(|| incoming.expected.clone());
        }
        return;
    }
    diagnostics.insert(MameLayoutDiagnostic::LogicalPathCollision {
        root: root.clone(),
        path,
        existing: existing.requirement.clone(),
        incoming: incoming.requirement.clone(),
        evidence: if existing.expected.sha1.is_some()
            && incoming.expected.sha1.is_some()
            && existing.expected.sha1 != incoming.expected.sha1
        {
            PathCollisionEvidence::DifferentSha1
        } else if !evidence_conflicts.is_empty() {
            PathCollisionEvidence::ConflictingExpectedEvidence
        } else {
            PathCollisionEvidence::InsufficientSha1
        },
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        build::view_manifest::{TargetViewRequest, plan_view},
        domain::{
            ArchiveBackend, ArchiveMemberSelector, CatalogKey, Crc32Digest, EvidenceProvenance,
            EvidenceScope, ExpectedEvidence, MatchingPolicy, ObservedContent, ScanProvenance,
            SelectionProvenance, SetKey, SourceFile, SourceLocation, SourcePhysicalPath,
            SourceRoot,
        },
        machine_dependencies::{
            MachineDependency, MachineDependencyCatalog, MachineDependencyKind, MachineSet,
        },
        resolution::MatchStrength,
    };

    fn snapshot() -> SnapshotKey {
        SnapshotKey::new(
            &CatalogKey::new("layout-fixture"),
            &crate::domain::DocumentKey::from_bytes(b"layout"),
            &crate::domain::ParserInterpretationKey::for_format(
                "mame",
                &crate::domain::CatalogScope::Complete,
            ),
        )
    }

    fn alternate_snapshot() -> SnapshotKey {
        SnapshotKey::new(
            &CatalogKey::new("foreign-layout-fixture"),
            &crate::domain::DocumentKey::from_bytes(b"foreign-layout"),
            &crate::domain::ParserInterpretationKey::for_format(
                "mame",
                &crate::domain::CatalogScope::Complete,
            ),
        )
    }

    fn entry(set: &str, name: &str, digest: Option<u8>, merge: Option<&str>) -> LogicalEntry {
        let sha1 = digest.map(|byte| [byte; 20]);
        let location = SourceLocation::BareFile {
            path: format!("/roms/{name}"),
        };
        LogicalEntry {
            path: LogicalPath::new(name),
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
                    sha1,
                    xxh3: [0; 8],
                },
                fingerprint: None,
                scan_run: None,
                scan_provenance: Some(ScanProvenance::StreamedSha1Xxh3V1),
                bare_file_cache_stamp: None,
            },
            requirement: RequirementKey::new(
                SetKey::new(CatalogKey::new("layout-fixture"), set),
                name,
            ),
            expected: ExpectedEvidence {
                scope: EvidenceScope::WholeAsset,
                provenance: EvidenceProvenance::SourceDeclared,
                sha1,
                merge: merge.map(str::to_owned),
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

    fn resolved(name: &str, entries: Vec<LogicalEntry>) -> ResolvedMachineSet {
        let mut set = ResolvedMachineSet::new(
            snapshot(),
            CatalogKey::new("layout-fixture"),
            SetName::new(name),
        );
        set.entries = entries
            .into_iter()
            .enumerate()
            .map(|(component_order, entry)| {
                let identity = AssetRequirementIdentity::new(
                    set.snapshot.clone(),
                    set.name.clone(),
                    entry.requirement.rom_name(),
                    i64::try_from(component_order).unwrap_or(i64::MAX),
                );
                ResolvedMachineAsset::new(identity, entry)
            })
            .collect();
        set
    }

    fn names(plan: &MameLayoutPlan, root: &str) -> Vec<String> {
        plan.groups
            .iter()
            .find(|group| group.path.as_str() == root)
            .map(|group| {
                group
                    .entries
                    .iter()
                    .map(|entry| entry.path.as_str().to_owned())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn merged_family_fixture() -> (MachineDependencyCatalog, Vec<ResolvedMachineSet>) {
        let mut clone_a = MachineSet::new("clone-a");
        clone_a.parent_clone = Some(SetName::new("parent"));
        clone_a.dependencies = vec![MachineDependency {
            kind: MachineDependencyKind::RomOf,
            target: SetName::new("bios"),
        }];
        let mut clone_b = MachineSet::new("clone-b");
        clone_b.parent_clone = Some(SetName::new("parent"));
        clone_b.dependencies = clone_a.dependencies.clone();
        let mut unselected_clone = MachineSet::new("clone-unselected");
        unselected_clone.parent_clone = Some(SetName::new("parent"));
        let graph = MachineDependencyCatalog::new(
            snapshot(),
            vec![
                clone_a,
                clone_b,
                unselected_clone,
                MachineSet::new("parent"),
                {
                    let mut bios = MachineSet::new("bios");
                    bios.is_bios = true;
                    bios
                },
            ],
        );

        let mut resolved_clone_a = resolved(
            "clone-a",
            vec![
                entry(
                    "clone-a",
                    "parent-game.rom",
                    Some(5),
                    Some("parent-game.rom"),
                ),
                entry("clone-a", "a-only.rom", Some(7), None),
            ],
        );
        resolved_clone_a.parent_clone = Some(SetName::new("parent"));
        let mut resolved_clone_b = resolved(
            "clone-b",
            vec![
                entry(
                    "clone-b",
                    "parent-game.rom",
                    Some(5),
                    Some("parent-game.rom"),
                ),
                entry("clone-b", "b-only.rom", Some(9), None),
            ],
        );
        resolved_clone_b.parent_clone = Some(SetName::new("parent"));
        let mut unselected_clone = resolved(
            "clone-unselected",
            vec![entry(
                "clone-unselected",
                "unselected-only.rom",
                Some(10),
                None,
            )],
        );
        unselected_clone.parent_clone = Some(SetName::new("parent"));
        let sets = vec![
            resolved_clone_a,
            resolved_clone_b,
            unselected_clone,
            resolved(
                "parent",
                vec![
                    entry("parent", "parent-game.rom", Some(5), None),
                    entry("parent", "inherited.rom", Some(6), None),
                ],
            ),
            resolved("bios", vec![entry("bios", "bios.rom", Some(8), None)]),
        ];
        (graph, sets)
    }

    fn clone_plan(
        child_entries: Vec<LogicalEntry>,
        parent_entries: Vec<LogicalEntry>,
    ) -> MameLayoutPlan {
        let mut clone_set = MachineSet::new("clone");
        clone_set.parent_clone = Some(SetName::new("parent"));
        let closure = MachineDependencyCatalog::new(snapshot(), vec![clone_set])
            .resolve(&SetName::new("clone"));
        let mut child = resolved("clone", child_entries);
        child.parent_clone = Some(SetName::new("parent"));
        plan_mame_layout(
            MameSetLayoutPolicy::NonMerged,
            &snapshot(),
            &[SetName::new("clone")],
            &[closure],
            &[child, resolved("parent", parent_entries)],
        )
    }

    #[test]
    fn non_merged_duplicates_shared_bios_and_device_into_each_machine_group() {
        let mut first = MachineSet::new("first");
        first.dependencies = vec![
            MachineDependency {
                kind: MachineDependencyKind::RomOf,
                target: SetName::new("bios"),
            },
            MachineDependency {
                kind: MachineDependencyKind::DeviceReference,
                target: SetName::new("sound"),
            },
        ];
        let mut second = MachineSet::new("second");
        second.dependencies = vec![MachineDependency {
            kind: MachineDependencyKind::RomOf,
            target: SetName::new("bios"),
        }];
        let mut bios = MachineSet::new("bios");
        bios.is_bios = true;
        let mut sound = MachineSet::new("sound");
        sound.is_device = true;
        let graph = MachineDependencyCatalog::new(snapshot(), vec![first, second, bios, sound]);
        let closures = [
            graph.resolve(&SetName::new("first")),
            graph.resolve(&SetName::new("second")),
        ];
        let sets = vec![
            resolved("first", vec![entry("first", "game.rom", Some(1), None)]),
            resolved("second", vec![entry("second", "game2.rom", Some(2), None)]),
            resolved("bios", vec![entry("bios", "bios.rom", Some(3), None)]),
            resolved("sound", vec![entry("sound", "sound.rom", Some(4), None)]),
        ];

        let plan = plan_mame_layout(
            MameSetLayoutPolicy::NonMerged,
            &snapshot(),
            &[SetName::new("second"), SetName::new("first")],
            &closures,
            &sets,
        );
        assert!(plan.is_complete());
        assert_eq!(plan.groups.len(), 2);
        assert_eq!(names(&plan, "first"), ["bios.rom", "game.rom", "sound.rom"]);
        assert_eq!(names(&plan, "second"), ["bios.rom", "game2.rom"]);
        let permuted = plan_mame_layout(
            MameSetLayoutPolicy::NonMerged,
            &snapshot(),
            &[SetName::new("first"), SetName::new("second")],
            &[closures[1].clone(), closures[0].clone()],
            &[
                sets[3].clone(),
                sets[1].clone(),
                sets[2].clone(),
                sets[0].clone(),
            ],
        );
        assert_eq!(plan, permuted);
    }

    #[test]
    fn split_keeps_clone_overlays_and_runtime_dependencies_in_separate_sets() {
        let mut clone = MachineSet::new("clone");
        clone.parent_clone = Some(SetName::new("parent"));
        clone.dependencies = vec![
            MachineDependency {
                kind: MachineDependencyKind::RomOf,
                target: SetName::new("bios"),
            },
            MachineDependency {
                kind: MachineDependencyKind::DeviceReference,
                target: SetName::new("sound"),
            },
        ];
        let parent = MachineSet::new("parent");
        let mut bios = MachineSet::new("bios");
        bios.is_bios = true;
        let mut sound = MachineSet::new("sound");
        sound.is_device = true;
        let graph = MachineDependencyCatalog::new(snapshot(), vec![clone, parent, bios, sound]);
        let closure = graph.resolve(&SetName::new("clone"));

        let mut resolved_clone = resolved(
            "clone",
            vec![
                entry("clone", "clone-game.rom", Some(5), Some("parent-game.rom")),
                entry("clone", "clone-only.rom", Some(7), None),
            ],
        );
        resolved_clone.parent_clone = Some(SetName::new("parent"));
        let sets = vec![
            resolved_clone,
            resolved(
                "parent",
                vec![
                    entry("parent", "parent-game.rom", Some(5), None),
                    entry("parent", "inherited.rom", Some(6), None),
                ],
            ),
            resolved("bios", vec![entry("bios", "bios.rom", Some(8), None)]),
            resolved("sound", vec![entry("sound", "sound.rom", Some(9), None)]),
        ];

        let plan = plan_mame_layout(
            MameSetLayoutPolicy::Split,
            &snapshot(),
            &[SetName::new("clone")],
            &[closure],
            &sets,
        );

        assert!(plan.is_complete());
        assert_eq!(
            plan.groups
                .iter()
                .map(|group| group.path.as_str())
                .collect::<Vec<_>>(),
            ["bios", "clone", "parent", "sound"]
        );
        assert_eq!(names(&plan, "clone"), ["clone-only.rom"]);
        assert_eq!(names(&plan, "parent"), ["inherited.rom", "parent-game.rom"]);
        assert_eq!(names(&plan, "bios"), ["bios.rom"]);
        assert_eq!(names(&plan, "sound"), ["sound.rom"]);
    }

    #[test]
    fn split_resolves_merge_assertions_through_multiple_clone_ancestors() {
        let mut child = resolved(
            "child",
            vec![
                entry("child", "child-alias.rom", Some(5), Some("grandparent.rom")),
                entry("child", "child-only.rom", Some(7), None),
            ],
        );
        child.parent_clone = Some(SetName::new("parent"));
        let mut parent = resolved(
            "parent",
            vec![entry("parent", "parent-only.rom", Some(6), None)],
        );
        parent.parent_clone = Some(SetName::new("grandparent"));
        let grandparent = resolved(
            "grandparent",
            vec![entry("grandparent", "grandparent.rom", Some(5), None)],
        );
        let graph = MachineDependencyCatalog::new(
            snapshot(),
            vec![
                MachineSet::new("child"),
                MachineSet::new("parent"),
                MachineSet::new("grandparent"),
            ],
        );
        let closure = graph.resolve(&SetName::new("child"));

        let plan = plan_mame_layout(
            MameSetLayoutPolicy::Split,
            &snapshot(),
            &[SetName::new("child")],
            &[closure],
            &[child, parent, grandparent],
        );

        assert!(plan.is_complete());
        assert_eq!(names(&plan, "child"), ["child-only.rom"]);
        assert_eq!(names(&plan, "parent"), ["parent-only.rom"]);
        assert_eq!(names(&plan, "grandparent"), ["grandparent.rom"]);
    }

    #[test]
    fn missing_clone_assets_are_satisfied_only_by_proven_inherited_content() {
        let mut child = resolved("child", vec![]);
        child.parent_clone = Some(SetName::new("parent"));
        let requirement = RequirementKey::new(
            SetKey::new(CatalogKey::new("layout-fixture"), "child"),
            "child.rom",
        );
        child.missing_assets.push(MissingMachineAsset {
            identity: AssetRequirementIdentity::new(
                snapshot(),
                SetName::new("child"),
                "child.rom",
                0,
            ),
            requirement,
            expected: entry("child", "child.rom", Some(5), Some("parent.rom")).expected,
            merge_target: Some(AssetRequirementIdentity::new(
                snapshot(),
                SetName::new("parent"),
                "parent.rom",
                0,
            )),
        });
        let parent = resolved("parent", vec![entry("parent", "parent.rom", Some(5), None)]);
        let graph = MachineDependencyCatalog::new(
            snapshot(),
            vec![MachineSet::new("child"), MachineSet::new("parent")],
        );
        let closure = graph.resolve(&SetName::new("child"));
        let sets = [child, parent];

        let split = plan_mame_layout(
            MameSetLayoutPolicy::Split,
            &snapshot(),
            &[SetName::new("child")],
            std::slice::from_ref(&closure),
            &sets,
        );
        assert!(split.is_complete());
        assert_eq!(names(&split, "parent"), ["parent.rom"]);
        assert!(
            split
                .groups
                .iter()
                .any(|group| { group.path.as_str() == "child" && group.entries.is_empty() })
        );

        let merged = plan_mame_layout(
            MameSetLayoutPolicy::Merged,
            &snapshot(),
            &[SetName::new("child")],
            &[closure],
            &sets,
        );
        assert!(merged.is_complete());
        assert_eq!(names(&merged, "parent"), ["parent.rom"]);
        assert!(
            !merged
                .groups
                .iter()
                .any(|group| group.path.as_str() == "child")
        );
    }

    #[test]
    fn missing_merge_evidence_is_checked_only_against_ancestors() {
        let mut middle = resolved("middle", vec![]);
        middle.parent_clone = Some(SetName::new("root"));
        middle.missing_assets.push(MissingMachineAsset {
            identity: AssetRequirementIdentity::new(
                snapshot(),
                SetName::new("middle"),
                "middle.rom",
                0,
            ),
            requirement: RequirementKey::new(
                SetKey::new(CatalogKey::new("layout-fixture"), "middle"),
                "middle.rom",
            ),
            expected: entry("middle", "middle.rom", Some(5), Some("later.rom")).expected,
            merge_target: None,
        });
        let root = resolved("root", vec![]);
        let mut leaf = resolved("leaf", vec![entry("leaf", "later.rom", Some(5), None)]);
        leaf.parent_clone = Some(SetName::new("middle"));
        let graph = MachineDependencyCatalog::new(
            snapshot(),
            vec![
                MachineSet::new("root"),
                MachineSet::new("middle"),
                MachineSet::new("leaf"),
            ],
        );
        let closure = graph.resolve(&SetName::new("leaf"));
        let plan = plan_mame_layout(
            MameSetLayoutPolicy::Split,
            &snapshot(),
            &[SetName::new("leaf")],
            &[closure],
            &[root, middle, leaf],
        );

        assert!(!plan.is_complete());
        assert!(plan.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::MissingAssets { set, .. } if set.as_str() == "middle"
        )));

        let root = resolved("root", vec![entry("root", "target.rom", Some(5), None)]);
        let mut middle = resolved("middle", vec![]);
        middle.parent_clone = Some(SetName::new("root"));
        middle.missing_assets.push(MissingMachineAsset {
            identity: AssetRequirementIdentity::new(
                snapshot(),
                SetName::new("middle"),
                "middle.rom",
                0,
            ),
            requirement: RequirementKey::new(
                SetKey::new(CatalogKey::new("layout-fixture"), "middle"),
                "middle.rom",
            ),
            expected: entry("middle", "middle.rom", Some(5), Some("target.rom")).expected,
            merge_target: Some(AssetRequirementIdentity::new(
                snapshot(),
                SetName::new("root"),
                "target.rom",
                0,
            )),
        });
        let mut leaf = resolved("leaf", vec![entry("leaf", "target.rom", Some(6), None)]);
        leaf.parent_clone = Some(SetName::new("middle"));
        let graph = MachineDependencyCatalog::new(
            snapshot(),
            vec![
                MachineSet::new("root"),
                MachineSet::new("middle"),
                MachineSet::new("leaf"),
            ],
        );
        let closure = graph.resolve(&SetName::new("leaf"));
        let plan = plan_mame_layout(
            MameSetLayoutPolicy::Split,
            &snapshot(),
            &[SetName::new("leaf")],
            &[closure],
            &[root, middle, leaf],
        );

        assert!(plan.is_complete());
        assert_eq!(names(&plan, "root"), ["target.rom"]);
    }

    #[test]
    fn missing_merge_can_target_a_shadowed_grandparent_asset() {
        let grandparent = resolved(
            "grandparent",
            vec![entry("grandparent", "target.rom", Some(5), None)],
        );
        let mut parent = resolved("parent", vec![entry("parent", "target.rom", Some(6), None)]);
        parent.parent_clone = Some(SetName::new("grandparent"));
        let mut child = resolved("child", vec![]);
        child.parent_clone = Some(SetName::new("parent"));
        child.missing_assets.push(MissingMachineAsset {
            identity: AssetRequirementIdentity::new(
                snapshot(),
                SetName::new("child"),
                "child.rom",
                0,
            ),
            requirement: RequirementKey::new(
                SetKey::new(CatalogKey::new("layout-fixture"), "child"),
                "child.rom",
            ),
            expected: entry("child", "child.rom", Some(5), Some("target.rom")).expected,
            merge_target: Some(AssetRequirementIdentity::new(
                snapshot(),
                SetName::new("grandparent"),
                "target.rom",
                0,
            )),
        });
        let graph = MachineDependencyCatalog::new(
            snapshot(),
            vec![
                MachineSet::new("grandparent"),
                MachineSet::new("parent"),
                MachineSet::new("child"),
            ],
        );
        let closure = graph.resolve(&SetName::new("child"));
        let plan = plan_mame_layout(
            MameSetLayoutPolicy::Split,
            &snapshot(),
            &[SetName::new("child")],
            &[closure],
            &[grandparent, parent, child],
        );

        assert!(plan.is_complete());
        assert_eq!(names(&plan, "grandparent"), ["target.rom"]);
        assert_eq!(names(&plan, "parent"), ["target.rom"]);
        assert!(
            plan.groups
                .iter()
                .any(|group| { group.path.as_str() == "child" && group.entries.is_empty() })
        );
    }

    #[test]
    fn split_keeps_unproven_merge_assets_and_reports_the_mismatch() {
        let mut child = resolved(
            "child",
            vec![entry("child", "child.rom", Some(5), Some("parent.rom"))],
        );
        child.parent_clone = Some(SetName::new("parent"));
        let parent = resolved("parent", vec![entry("parent", "parent.rom", Some(6), None)]);
        let graph = MachineDependencyCatalog::new(
            snapshot(),
            vec![MachineSet::new("child"), MachineSet::new("parent")],
        );
        let closure = graph.resolve(&SetName::new("child"));

        let plan = plan_mame_layout(
            MameSetLayoutPolicy::Split,
            &snapshot(),
            &[SetName::new("child")],
            &[closure],
            &[child, parent],
        );

        assert!(!plan.is_complete());
        assert_eq!(names(&plan, "child"), ["child.rom"]);
        assert_eq!(names(&plan, "parent"), ["parent.rom"]);
        assert!(plan.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::MergeContentMismatch { .. }
        )));
    }

    #[test]
    fn merged_combines_selected_clone_family_once_and_keeps_dependencies_separate() {
        let (graph, sets) = merged_family_fixture();
        let roots = [
            SetName::new("clone-b"),
            SetName::new("parent"),
            SetName::new("clone-a"),
        ];
        let closures = roots
            .iter()
            .map(|root| graph.resolve(root))
            .collect::<Vec<_>>();

        let plan = plan_mame_layout(
            MameSetLayoutPolicy::Merged,
            &snapshot(),
            &roots,
            &closures,
            &sets,
        );

        assert!(plan.is_complete());
        assert_eq!(
            plan.groups
                .iter()
                .map(|group| group.path.as_str())
                .collect::<Vec<_>>(),
            ["bios", "parent"]
        );
        assert_eq!(
            names(&plan, "parent"),
            [
                "a-only.rom",
                "b-only.rom",
                "inherited.rom",
                "parent-game.rom"
            ]
        );
        assert_eq!(names(&plan, "bios"), ["bios.rom"]);
        assert!(
            names(&plan, "parent")
                .iter()
                .all(|name| name != "unselected-only.rom")
        );

        let permuted_roots = [
            SetName::new("clone-a"),
            SetName::new("clone-b"),
            SetName::new("parent"),
        ];
        let permuted_closures = permuted_roots
            .iter()
            .map(|root| graph.resolve(root))
            .collect::<Vec<_>>();
        let permuted = plan_mame_layout(
            MameSetLayoutPolicy::Merged,
            &snapshot(),
            &permuted_roots,
            &permuted_closures,
            &sets,
        );
        assert_eq!(plan, permuted);

        let permuted_sets = sets.iter().rev().cloned().collect::<Vec<_>>();
        let permuted_set_plan = plan_mame_layout(
            MameSetLayoutPolicy::Merged,
            &snapshot(),
            &roots,
            &closures,
            &permuted_sets,
        );
        assert_eq!(plan, permuted_set_plan);
    }

    #[test]
    fn pinned_view_manifest_is_stable_when_catalog_and_inventory_order_changes()
    -> Result<(), crate::build::view_manifest::ViewManifestError> {
        let (graph, sets) = merged_family_fixture();
        let roots = [
            SetName::new("clone-b"),
            SetName::new("parent"),
            SetName::new("clone-a"),
        ];
        let closures = roots
            .iter()
            .map(|root| graph.resolve(root))
            .collect::<Vec<_>>();
        let snapshot = snapshot();
        let first = plan_view(TargetViewRequest::mame_0289(
            &snapshot,
            &roots,
            MameSetLayoutPolicy::Merged,
            &closures,
            &sets,
        ));

        let reversed_roots = roots.iter().rev().cloned().collect::<Vec<_>>();
        let reversed_closures = reversed_roots
            .iter()
            .map(|root| graph.resolve(root))
            .collect::<Vec<_>>();
        let reversed_sets = sets.iter().rev().cloned().collect::<Vec<_>>();
        let second = plan_view(TargetViewRequest::mame_0289(
            &snapshot,
            &reversed_roots,
            MameSetLayoutPolicy::Merged,
            &reversed_closures,
            &reversed_sets,
        ));

        assert_eq!(first, second);
        assert_eq!(first.to_json()?, second.to_json()?);
        Ok(())
    }

    #[test]
    fn merged_reports_conflicting_same_path_assets_in_selected_clones() {
        let mut first = MachineSet::new("first");
        first.parent_clone = Some(SetName::new("parent"));
        let mut second = MachineSet::new("second");
        second.parent_clone = Some(SetName::new("parent"));
        let graph = MachineDependencyCatalog::new(
            snapshot(),
            vec![first, second, MachineSet::new("parent")],
        );
        let roots = [SetName::new("first"), SetName::new("second")];
        let closures = roots
            .iter()
            .map(|root| graph.resolve(root))
            .collect::<Vec<_>>();
        let mut first_set = resolved("first", vec![entry("first", "same.rom", Some(1), None)]);
        first_set.parent_clone = Some(SetName::new("parent"));
        let mut second_set = resolved("second", vec![entry("second", "same.rom", Some(2), None)]);
        second_set.parent_clone = Some(SetName::new("parent"));
        let sets = vec![first_set, second_set, resolved("parent", vec![])];

        let plan = plan_mame_layout(
            MameSetLayoutPolicy::Merged,
            &snapshot(),
            &roots,
            &closures,
            &sets,
        );

        assert!(!plan.is_complete());
        assert!(plan.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::LogicalPathCollision {
                evidence: PathCollisionEvidence::DifferentSha1,
                ..
            }
        )));
    }

    #[test]
    fn non_merged_keeps_parent_assertion_and_merge_content_in_child_group() {
        let mut clone = MachineSet::new("clone");
        clone.parent_clone = Some(SetName::new("parent"));
        let dependency =
            MachineDependencyCatalog::new(snapshot(), vec![clone]).resolve(&SetName::new("clone"));
        let mut merged_entry = entry("clone", "clone-game.rom", Some(5), Some("parent-game.rom"));
        merged_entry.source.location = SourceLocation::ArchiveMember {
            path: "/roms/parent.zip".into(),
            backend: ArchiveBackend::Zip,
            selector: ArchiveMemberSelector::IndexAndName {
                index: 0,
                name: "parent-game.rom".into(),
            },
        };
        let mut child = resolved("clone", vec![merged_entry]);
        child.parent_clone = Some(SetName::new("parent"));
        let parent = resolved(
            "parent",
            vec![
                entry("parent", "parent-game.rom", Some(5), None),
                entry("parent", "inherited.rom", Some(6), None),
            ],
        );
        let plan = plan_mame_layout(
            MameSetLayoutPolicy::NonMerged,
            &snapshot(),
            &[SetName::new("clone")],
            &[dependency],
            &[child, parent],
        );
        assert!(plan.is_complete());
        assert_eq!(names(&plan, "clone"), ["clone-game.rom", "inherited.rom"]);
        assert_eq!(
            plan.groups[0].entries[0].expected.merge.as_deref(),
            Some("parent-game.rom")
        );
        assert_eq!(
            plan.groups[0].entries[0].source.location.path(),
            "/roms/parent.zip"
        );
    }

    #[test]
    fn diagnoses_missing_merge_targets() {
        let child = entry("clone", "clone-game.rom", Some(5), Some("parent-game.rom"));
        let missing_target = clone_plan(vec![child], vec![]);
        assert!(missing_target.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::MissingMergeTarget { target, .. }
                if target.as_str() == "parent-game.rom"
        )));
    }

    #[test]
    fn conflicting_merge_evidence_preserves_the_inherited_entry() {
        let mut child = entry("clone", "clone-game.rom", Some(5), Some("parent-game.rom"));
        child.expected.size = Some(2);
        child.expected.crc = Some(Crc32Digest([2; 4]));
        let mut parent = entry("parent", "parent-game.rom", Some(5), None);
        parent.expected.size = Some(1);
        parent.expected.crc = Some(Crc32Digest([1; 4]));
        let plan = clone_plan(vec![child], vec![parent]);
        assert!(plan.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::MergeContentMismatch { conflicts, .. }
                if conflicts == &[
                    ContentEvidenceConflict::Size,
                    ContentEvidenceConflict::Crc,
                ]
        )));
        assert_eq!(names(&plan, "clone"), ["clone-game.rom", "parent-game.rom"]);
    }

    #[test]
    fn same_path_merge_mismatch_does_not_overwrite_the_parent() {
        let mut child = entry("clone", "parent-game.rom", Some(5), Some("parent-game.rom"));
        child.expected.size = Some(2);
        let mut parent = entry("parent", "parent-game.rom", Some(5), None);
        parent.expected.size = Some(1);
        let plan = clone_plan(vec![child], vec![parent]);
        assert!(plan.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::MergeContentMismatch { .. }
        )));
        let retained = plan
            .groups
            .iter()
            .flat_map(|group| &group.entries)
            .filter(|entry| entry.path.as_str() == "parent-game.rom")
            .collect::<Vec<_>>();
        assert_eq!(retained.len(), 1);
        assert_eq!(retained[0].requirement.game_name(), "parent");
    }

    #[test]
    fn weak_merge_evidence_preserves_the_inherited_entry() {
        let mut child = entry("clone", "clone-game.rom", None, Some("parent-game.rom"));
        child.expected.crc = Some(Crc32Digest([5; 4]));
        let mut parent = entry("parent", "parent-game.rom", None, None);
        parent.expected.crc = Some(Crc32Digest([5; 4]));
        let plan = clone_plan(vec![child], vec![parent]);
        assert!(!plan.is_complete());
        assert!(plan.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::MergeContentMismatch { conflicts, .. }
                if conflicts == &[ContentEvidenceConflict::InsufficientMatchEvidence]
        )));
        assert_eq!(names(&plan, "clone"), ["clone-game.rom", "parent-game.rom"]);
    }

    #[test]
    fn non_whole_asset_digests_do_not_validate_merges() {
        for scope in [EvidenceScope::DiskData, EvidenceScope::ChdHeaderSha1] {
            let mut child = entry("clone", "clone-game.rom", Some(5), Some("parent-game.rom"));
            child.expected.scope = scope;
            let mut parent = entry("parent", "parent-game.rom", Some(5), None);
            parent.expected.scope = scope;

            let plan = clone_plan(vec![child], vec![parent]);

            assert!(
                !plan.is_complete(),
                "{scope:?} must not prove file identity"
            );
            assert!(plan.diagnostics.iter().any(|diagnostic| matches!(
                diagnostic,
                MameLayoutDiagnostic::MergeContentMismatch { conflicts, .. }
                    if conflicts == &[ContentEvidenceConflict::InsufficientMatchEvidence]
            )));
            assert_eq!(names(&plan, "clone"), ["clone-game.rom", "parent-game.rom"]);
        }
    }

    #[test]
    fn merge_without_component_assertion_is_ambiguous_for_duplicate_names() {
        let child = entry("clone", "clone-game.rom", Some(5), Some("parent-game.rom"));
        let mut first_parent = entry("parent", "first-parent.rom", Some(5), None);
        first_parent.path = LogicalPath::new("parent-game.rom");
        first_parent.expected.size = Some(1);
        let mut second_parent = entry("parent", "second-parent.rom", Some(5), None);
        second_parent.path = LogicalPath::new("parent-game.rom");
        second_parent.expected.size = Some(2);
        let plan = clone_plan(vec![child], vec![first_parent, second_parent]);
        assert!(!plan.is_complete());
        assert!(plan.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::AmbiguousMergeTarget { target, .. }
                if target.as_str() == "parent-game.rom"
        )));
        assert_eq!(
            plan.groups[0]
                .entries
                .iter()
                .filter(|entry| entry.path.as_str() == "parent-game.rom")
                .count(),
            1
        );
        assert!(plan.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::LogicalPathCollision { path, .. }
                if path.as_str() == "parent-game.rom"
        )));
    }

    #[test]
    fn typed_merge_assertion_preserves_unrelated_same_path_components() {
        let mut child_entry = entry("clone", "clone-game.rom", Some(5), Some("parent-game.rom"));
        child_entry.path = LogicalPath::new("parent-game.rom");
        child_entry.expected.size = Some(1);
        let mut child = resolved("clone", vec![child_entry]);
        child.parent_clone = Some(SetName::new("parent"));
        child.entries[0].merge_target = Some(AssetRequirementIdentity::new(
            snapshot(),
            SetName::new("parent"),
            "parent-game.rom",
            0,
        ));

        let mut target = entry("parent", "parent-game.rom", Some(5), None);
        target.expected.size = Some(1);
        let mut sibling = entry("parent", "sibling.rom", Some(5), None);
        sibling.path = LogicalPath::new("parent-game.rom");
        sibling.expected.size = Some(2);
        let parent = resolved("parent", vec![target, sibling]);
        let mut clone_set = MachineSet::new("clone");
        clone_set.parent_clone = Some(SetName::new("parent"));
        let closure = MachineDependencyCatalog::new(snapshot(), vec![clone_set])
            .resolve(&SetName::new("clone"));

        let plan = plan_mame_layout(
            MameSetLayoutPolicy::NonMerged,
            &snapshot(),
            &[SetName::new("clone")],
            &[closure],
            &[child, parent],
        );

        assert!(plan.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::LogicalPathCollision { path, existing, incoming, .. }
                if path.as_str() == "parent-game.rom"
                    && (existing.rom_name() == "sibling.rom"
                        || incoming.rom_name() == "sibling.rom")
        )));
        assert!(!plan.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::MergeContentMismatch { .. }
                | MameLayoutDiagnostic::AmbiguousMergeTarget { .. }
        )));
        assert!(!plan.is_complete());
    }

    #[test]
    fn typed_merge_does_not_hide_same_named_missing_component() {
        let mut child = resolved(
            "clone",
            vec![entry(
                "clone",
                "clone-game.rom",
                Some(5),
                Some("parent-game.rom"),
            )],
        );
        child.parent_clone = Some(SetName::new("parent"));
        child.entries[0].merge_target = Some(AssetRequirementIdentity::new(
            snapshot(),
            SetName::new("parent"),
            "parent-game.rom",
            0,
        ));
        let mut parent = resolved(
            "parent",
            vec![entry("parent", "parent-game.rom", Some(5), None)],
        );
        let missing_requirement = RequirementKey::new(
            SetKey::new(CatalogKey::new("layout-fixture"), "parent"),
            "parent-game.rom",
        );
        parent.missing_assets.push(MissingMachineAsset {
            identity: AssetRequirementIdentity::new(
                snapshot(),
                SetName::new("parent"),
                "parent-game.rom",
                1,
            ),
            requirement: missing_requirement.clone(),
            expected: ExpectedEvidence::default(),
            merge_target: None,
        });
        let mut clone_set = MachineSet::new("clone");
        clone_set.parent_clone = Some(SetName::new("parent"));
        let closure = MachineDependencyCatalog::new(snapshot(), vec![clone_set])
            .resolve(&SetName::new("clone"));

        let plan = plan_mame_layout(
            MameSetLayoutPolicy::NonMerged,
            &snapshot(),
            &[SetName::new("clone")],
            &[closure],
            &[child, parent],
        );

        assert!(!plan.is_complete());
        assert!(plan.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::MissingAssets { set, requirements, .. }
                if set.as_str() == "parent" && requirements.contains(&missing_requirement)
        )));
    }

    #[test]
    fn sibling_merge_declarations_validate_against_the_same_inherited_layer() {
        let mut clone_set = MachineSet::new("clone");
        clone_set.parent_clone = Some(SetName::new("parent"));
        let closure = MachineDependencyCatalog::new(snapshot(), vec![clone_set])
            .resolve(&SetName::new("clone"));
        let mut child = resolved(
            "clone",
            vec![
                entry("clone", "child-a.rom", Some(5), Some("parent-game.rom")),
                entry("clone", "child-b.rom", Some(5), Some("parent-game.rom")),
            ],
        );
        child.parent_clone = Some(SetName::new("parent"));
        let parent = resolved(
            "parent",
            vec![entry("parent", "parent-game.rom", Some(5), None)],
        );
        let plan = plan_mame_layout(
            MameSetLayoutPolicy::NonMerged,
            &snapshot(),
            &[SetName::new("clone")],
            std::slice::from_ref(&closure),
            &[child.clone(), parent.clone()],
        );
        let reversed = plan_mame_layout(
            MameSetLayoutPolicy::NonMerged,
            &snapshot(),
            &[SetName::new("clone")],
            std::slice::from_ref(&closure),
            &[
                {
                    child.entries.reverse();
                    child
                },
                parent,
            ],
        );
        assert!(plan.is_complete());
        assert_eq!(plan, reversed);
        assert_eq!(names(&plan, "clone"), ["child-a.rom", "child-b.rom"]);
        assert!(!plan.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::MissingMergeTarget { .. }
        )));
    }

    #[test]
    fn mixed_sibling_merges_diagnose_a_displaced_same_path_entry() {
        let accepted = entry("clone", "parent-game.rom", Some(5), Some("parent-game.rom"));
        let rejected = entry("clone", "clone-only.rom", Some(6), Some("parent-game.rom"));
        let parent = entry("parent", "parent-game.rom", Some(5), None);
        let plan = clone_plan(vec![accepted, rejected], vec![parent]);
        assert!(plan.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::MergeContentMismatch { child, conflicts, .. }
                if child.rom_name() == "parent-game.rom"
                    && conflicts == &[ContentEvidenceConflict::ConflictingSiblingDeclaration]
        )));
        assert_eq!(names(&plan, "clone"), ["clone-only.rom", "parent-game.rom"]);
    }

    #[test]
    fn same_requirement_and_evidence_choose_source_deterministically() {
        let graph = MachineDependencyCatalog::new(snapshot(), vec![MachineSet::new("root")]);
        let closure = graph.resolve(&SetName::new("root"));
        let mut first = entry("root", "same.rom", Some(5), None);
        first.source.location = SourceLocation::BareFile {
            path: "/roms/z.rom".into(),
        };
        let mut second = first.clone();
        second.source.location = SourceLocation::BareFile {
            path: "/roms/a.rom".into(),
        };
        let make_plan = |entries: Vec<LogicalEntry>| {
            plan_mame_layout(
                MameSetLayoutPolicy::NonMerged,
                &snapshot(),
                &[SetName::new("root")],
                std::slice::from_ref(&closure),
                &[resolved("root", entries)],
            )
        };
        let forward = make_plan(vec![first.clone(), second.clone()]);
        let reversed = make_plan(vec![second, first.clone()]);
        assert_eq!(forward, reversed);
        assert_eq!(
            forward.groups[0].entries[0].source.location.path(),
            "/roms/a.rom"
        );

        let compatibility = first.clone();
        let mut evidence_aware = first;
        evidence_aware.selection.policy = MatchingPolicy::EvidenceAware;
        let forward = make_plan(vec![compatibility.clone(), evidence_aware.clone()]);
        let reversed = make_plan(vec![evidence_aware, compatibility]);
        assert_eq!(forward, reversed);
        assert_eq!(
            forward.groups[0].entries[0].selection.policy,
            MatchingPolicy::Sha1Compatibility
        );
    }

    #[test]
    fn diagnoses_unsafe_entries_and_file_directory_conflicts() {
        let graph = MachineDependencyCatalog::new(snapshot(), vec![MachineSet::new("root")]);
        let closure = graph.resolve(&SetName::new("root"));
        let mut unsafe_entry = entry("root", "escape.rom", Some(1), None);
        unsafe_entry.path = LogicalPath::new("../escape.rom");
        let mut file = entry("root", "file.rom", Some(2), None);
        file.path = LogicalPath::new("rom");
        let mut nested_file = entry("root", "track.rom", Some(3), None);
        nested_file.path = LogicalPath::new("rom/track");
        let plan = plan_mame_layout(
            MameSetLayoutPolicy::NonMerged,
            &snapshot(),
            &[SetName::new("root")],
            &[closure],
            &[resolved("root", vec![unsafe_entry, file, nested_file])],
        );
        assert!(!plan.is_complete());
        assert!(plan.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::OutputValidation { issue }
                if issue.kind == crate::build::validation::PlanIssueKind::UnsafeEntryPath
        )));
        assert!(plan.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::OutputValidation { issue }
                if issue.kind == crate::build::validation::PlanIssueKind::EntryFileDirectoryConflict
        )));
    }

    #[test]
    fn diagnoses_missing_and_cyclic_clone_ancestry() {
        let mut clone = resolved("clone", vec![]);
        clone.parent_clone = Some(SetName::new("parent"));
        let closure = MachineDependencyCatalog::new(snapshot(), vec![MachineSet::new("clone")])
            .resolve(&SetName::new("clone"));
        let missing = plan_mame_layout(
            MameSetLayoutPolicy::NonMerged,
            &snapshot(),
            &[SetName::new("clone")],
            std::slice::from_ref(&closure),
            std::slice::from_ref(&clone),
        );
        assert!(missing.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::MissingCloneParent { parent, .. }
                if parent.as_str() == "parent"
        )));

        let mut parent = resolved("parent", vec![]);
        parent.parent_clone = Some(SetName::new("clone"));
        let cyclic = plan_mame_layout(
            MameSetLayoutPolicy::NonMerged,
            &snapshot(),
            &[SetName::new("clone")],
            &[closure],
            &[clone, parent],
        );
        assert!(cyclic.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::CloneCycle { sets, .. }
                if sets.iter().map(SetName::as_str).collect::<Vec<_>>() == ["clone", "parent", "clone"]
        )));
    }

    #[test]
    fn reports_same_set_and_case_insensitive_path_collisions_deterministically() {
        let graph = MachineDependencyCatalog::new(snapshot(), vec![MachineSet::new("root")]);
        let closure = graph.resolve(&SetName::new("root"));
        let mut first = entry("root", "first.rom", Some(1), None);
        first.path = LogicalPath::new("duplicate.rom");
        let mut second = entry("root", "second.rom", Some(2), None);
        second.path = LogicalPath::new("duplicate.rom");
        let mut upper = entry("root", "upper.rom", Some(3), None);
        upper.path = LogicalPath::new("ROM.bin");
        let mut lower = entry("root", "lower.rom", Some(4), None);
        lower.path = LogicalPath::new("rom.bin");
        let content = resolved("root", vec![first, second, upper, lower]);

        let forward = plan_mame_layout(
            MameSetLayoutPolicy::NonMerged,
            &snapshot(),
            &[SetName::new("root")],
            std::slice::from_ref(&closure),
            std::slice::from_ref(&content),
        );
        let mut reverse_content = content;
        reverse_content.entries.reverse();
        let reversed = plan_mame_layout(
            MameSetLayoutPolicy::NonMerged,
            &snapshot(),
            &[SetName::new("root")],
            &[closure],
            &[reverse_content],
        );
        assert_eq!(forward, reversed);
        assert!(forward.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::LogicalPathCollision {
                evidence: PathCollisionEvidence::DifferentSha1,
                ..
            }
        )));
        assert!(forward.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::LogicalPathCollision {
                evidence: PathCollisionEvidence::CaseInsensitivePath,
                ..
            }
        )));
    }

    #[test]
    fn diagnoses_resolved_sets_from_another_snapshot() {
        let closure = MachineDependencyCatalog::new(snapshot(), vec![MachineSet::new("root")])
            .resolve(&SetName::new("root"));
        let mut foreign = resolved("root", vec![]);
        foreign.snapshot = alternate_snapshot();
        let plan = plan_mame_layout(
            MameSetLayoutPolicy::NonMerged,
            &snapshot(),
            &[SetName::new("root")],
            &[closure],
            &[foreign],
        );
        assert!(plan.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::ResolvedSetSnapshotMismatch {
                set,
                actual,
                ..
            } if set.as_str() == "root" && actual == &alternate_snapshot()
        )));
    }

    #[test]
    fn rejects_asset_requirements_from_another_catalog() {
        let graph = MachineDependencyCatalog::new(snapshot(), vec![MachineSet::new("root")]);
        let closure = graph.resolve(&SetName::new("root"));
        let mut foreign = entry("root", "asset.rom", Some(1), None);
        foreign.requirement = RequirementKey::new(
            SetKey::new(CatalogKey::new("foreign-catalog"), "root"),
            "asset.rom",
        );

        let plan = plan_mame_layout(
            MameSetLayoutPolicy::NonMerged,
            &snapshot(),
            &[SetName::new("root")],
            &[closure],
            &[resolved("root", vec![foreign])],
        );

        assert!(!plan.is_complete());
        assert!(plan.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::InvalidAssetIdentity { requirement, .. }
                if requirement.set().catalog() == &CatalogKey::new("foreign-catalog")
        )));
    }

    #[test]
    fn rejects_dependency_sets_from_another_catalog() {
        let mut root = MachineSet::new("root");
        root.dependencies.push(MachineDependency {
            kind: MachineDependencyKind::RomOf,
            target: SetName::new("bios"),
        });
        let closure =
            MachineDependencyCatalog::new(snapshot(), vec![root, MachineSet::new("bios")])
                .resolve(&SetName::new("root"));
        let mut foreign_bios = resolved("bios", vec![entry("bios", "bios.rom", Some(2), None)]);
        foreign_bios.catalog = CatalogKey::new("foreign-catalog");

        let plan = plan_mame_layout(
            MameSetLayoutPolicy::NonMerged,
            &snapshot(),
            &[SetName::new("root")],
            &[closure],
            &[resolved("root", Vec::new()), foreign_bios],
        );

        assert!(!plan.is_complete());
        assert!(plan.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::ResolvedSetCatalogMismatch {
                set,
                expected,
                actual,
                ..
            } if set.as_str() == "bios"
                && expected == &CatalogKey::new("layout-fixture")
                && actual == &CatalogKey::new("foreign-catalog")
        )));
    }

    #[test]
    fn rejects_clone_parents_from_another_catalog() {
        let closure = MachineDependencyCatalog::new(snapshot(), vec![MachineSet::new("root")])
            .resolve(&SetName::new("root"));
        let mut root = resolved("root", vec![entry("root", "root.rom", Some(1), None)]);
        root.parent_clone = Some(SetName::new("parent"));
        let mut foreign_parent =
            resolved("parent", vec![entry("parent", "parent.rom", Some(2), None)]);
        foreign_parent.catalog = CatalogKey::new("foreign-catalog");

        let plan = plan_mame_layout(
            MameSetLayoutPolicy::NonMerged,
            &snapshot(),
            &[SetName::new("root")],
            &[closure],
            &[root, foreign_parent],
        );

        assert!(!plan.is_complete());
        assert!(plan.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::ResolvedSetCatalogMismatch {
                set,
                expected,
                actual,
                ..
            } if set.as_str() == "parent"
                && expected == &CatalogKey::new("layout-fixture")
                && actual == &CatalogKey::new("foreign-catalog")
        )));
        assert_eq!(names(&plan, "root"), ["root.rom"]);
    }

    #[test]
    fn rejects_mismatched_and_duplicate_component_identities() {
        let closure = MachineDependencyCatalog::new(snapshot(), vec![MachineSet::new("root")])
            .resolve(&SetName::new("root"));
        let mut content = resolved(
            "root",
            vec![
                entry("root", "first.rom", Some(1), None),
                entry("root", "second.rom", Some(2), None),
                entry("root", "third.rom", Some(3), None),
            ],
        );
        content.entries[0].identity.snapshot = alternate_snapshot();
        content.entries[2].identity.component_order = content.entries[1].identity.component_order;
        let plan = plan_mame_layout(
            MameSetLayoutPolicy::NonMerged,
            &snapshot(),
            &[SetName::new("root")],
            &[closure],
            &[content],
        );
        assert!(plan.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::InvalidAssetIdentity { .. }
        )));
        assert!(plan.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::DuplicateAssetIdentity { .. }
        )));
        assert!(!plan.is_complete());
    }

    #[test]
    fn reports_dependency_and_missing_asset_diagnostics_without_dropping_context() {
        let mut root = MachineSet::new("root");
        root.dependencies = vec![MachineDependency {
            kind: MachineDependencyKind::DeviceReference,
            target: SetName::new("missing-device"),
        }];
        let closure = MachineDependencyCatalog::with_completeness(
            snapshot(),
            crate::machine_dependencies::SnapshotCompleteness::Complete,
            vec![root],
        )
        .resolve(&SetName::new("root"));
        let mut root_content = resolved("root", vec![]);
        let requirement = RequirementKey::new(
            SetKey::new(CatalogKey::new("layout-fixture"), "root"),
            "required.rom",
        );
        root_content.missing_assets.push(MissingMachineAsset {
            identity: AssetRequirementIdentity::new(
                snapshot(),
                SetName::new("root"),
                "required.rom",
                0,
            ),
            requirement,
            expected: ExpectedEvidence::default(),
            merge_target: None,
        });
        let plan = plan_mame_layout(
            MameSetLayoutPolicy::NonMerged,
            &snapshot(),
            &[SetName::new("root")],
            std::slice::from_ref(&closure),
            &[root_content],
        );
        assert!(plan.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::Dependency {
                diagnostic: DependencyDiagnostic::MissingSet { name, .. },
                ..
            } if name.as_str() == "missing-device"
        )));
        assert!(plan.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::MissingAssets { root, set, .. }
                if root.as_str() == "root" && set.as_str() == "root"
        )));
    }

    #[test]
    fn forwards_cycle_and_unsupported_relationship_diagnostics() {
        let mut root = MachineSet::new("root");
        root.dependencies = vec![MachineDependency {
            kind: MachineDependencyKind::RomOf,
            target: SetName::new("child"),
        }];
        root.unsupported_relationships
            .push(("sampleof".into(), SetName::new("samples")));
        let mut child = MachineSet::new("child");
        child.dependencies = vec![MachineDependency {
            kind: MachineDependencyKind::RomOf,
            target: SetName::new("root"),
        }];
        let closure = MachineDependencyCatalog::with_completeness(
            snapshot(),
            crate::machine_dependencies::SnapshotCompleteness::Complete,
            vec![root, child],
        )
        .resolve(&SetName::new("root"));
        let plan = plan_mame_layout(
            MameSetLayoutPolicy::NonMerged,
            &snapshot(),
            &[SetName::new("root")],
            &[closure],
            &[resolved("root", vec![]), resolved("child", vec![])],
        );
        assert!(plan.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::Dependency {
                diagnostic: DependencyDiagnostic::Cycle { .. },
                ..
            }
        )));
        assert!(plan.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::Dependency {
                diagnostic: DependencyDiagnostic::UnsupportedRelationship { field, .. },
                ..
            } if field == "sampleof"
        )));
    }

    #[test]
    fn coalesces_only_identical_sha1_and_retains_both_requirements() {
        let mut root = MachineSet::new("root");
        root.dependencies = vec![MachineDependency {
            kind: MachineDependencyKind::DeviceReference,
            target: SetName::new("device"),
        }];
        let graph =
            MachineDependencyCatalog::new(snapshot(), vec![root, MachineSet::new("device")]);
        let closure = graph.resolve(&SetName::new("root"));
        let sets = vec![
            resolved("root", vec![entry("root", "shared.rom", Some(8), None)]),
            resolved("device", vec![entry("device", "shared.rom", Some(8), None)]),
        ];
        let plan = plan_mame_layout(
            MameSetLayoutPolicy::NonMerged,
            &snapshot(),
            &[SetName::new("root")],
            std::slice::from_ref(&closure),
            &sets,
        );
        assert!(plan.is_complete());
        assert_eq!(plan.groups[0].entries.len(), 1);
        assert_eq!(plan.coalesced_provenance.len(), 1);
        assert_eq!(plan.coalesced_provenance[0].requirements.len(), 2);

        let conflict = plan_mame_layout(
            MameSetLayoutPolicy::NonMerged,
            &snapshot(),
            &[SetName::new("root")],
            &[graph.resolve(&SetName::new("root"))],
            &[
                resolved("root", vec![entry("root", "shared.rom", Some(8), None)]),
                resolved("device", vec![entry("device", "shared.rom", Some(9), None)]),
            ],
        );
        assert!(conflict.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::LogicalPathCollision {
                evidence: PathCollisionEvidence::DifferentSha1,
                ..
            }
        )));

        let unknown = plan_mame_layout(
            MameSetLayoutPolicy::NonMerged,
            &snapshot(),
            &[SetName::new("root")],
            std::slice::from_ref(&closure),
            &[
                resolved("root", vec![entry("root", "shared.rom", None, None)]),
                resolved("device", vec![entry("device", "shared.rom", None, None)]),
            ],
        );
        assert!(unknown.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::LogicalPathCollision {
                evidence: PathCollisionEvidence::InsufficientSha1,
                ..
            }
        )));

        let mut root_entry = entry("root", "shared.rom", Some(8), None);
        root_entry.expected.size = Some(10);
        root_entry.expected.crc = Some(Crc32Digest([1; 4]));
        let mut device_entry = entry("device", "shared.rom", Some(8), None);
        device_entry.expected.size = Some(11);
        device_entry.expected.crc = Some(Crc32Digest([2; 4]));
        let conflicting_metadata = plan_mame_layout(
            MameSetLayoutPolicy::NonMerged,
            &snapshot(),
            &[SetName::new("root")],
            std::slice::from_ref(&closure),
            &[
                resolved("root", vec![root_entry]),
                resolved("device", vec![device_entry]),
            ],
        );
        assert!(
            conflicting_metadata
                .diagnostics
                .iter()
                .any(|diagnostic| matches!(
                    diagnostic,
                    MameLayoutDiagnostic::LogicalPathCollision {
                        evidence: PathCollisionEvidence::ConflictingExpectedEvidence,
                        ..
                    }
                ))
        );
    }

    #[test]
    fn non_whole_asset_sha1_does_not_coalesce_same_path_requirements() {
        for scope in [EvidenceScope::DiskData, EvidenceScope::ChdHeaderSha1] {
            let mut root = MachineSet::new("root");
            root.dependencies = vec![MachineDependency {
                kind: MachineDependencyKind::DeviceReference,
                target: SetName::new("device"),
            }];
            let graph =
                MachineDependencyCatalog::new(snapshot(), vec![root, MachineSet::new("device")]);
            let closure = graph.resolve(&SetName::new("root"));
            let mut root_entry = entry("root", "shared.rom", Some(8), None);
            root_entry.expected.scope = scope;
            let mut device_entry = entry("device", "shared.rom", Some(8), None);
            device_entry.expected.scope = scope;

            let plan = plan_mame_layout(
                MameSetLayoutPolicy::NonMerged,
                &snapshot(),
                &[SetName::new("root")],
                &[closure],
                &[
                    resolved("root", vec![root_entry]),
                    resolved("device", vec![device_entry]),
                ],
            );

            assert!(
                !plan.is_complete(),
                "{scope:?} must not prove file identity"
            );
            assert!(plan.diagnostics.iter().any(|diagnostic| matches!(
                diagnostic,
                MameLayoutDiagnostic::LogicalPathCollision {
                    evidence: PathCollisionEvidence::InsufficientSha1,
                    ..
                }
            )));
            assert!(plan.coalesced_provenance.is_empty());
        }
    }

    #[test]
    fn non_whole_asset_merge_digests_never_omit_assets_under_any_policy() {
        for policy in [
            MameSetLayoutPolicy::NonMerged,
            MameSetLayoutPolicy::Split,
            MameSetLayoutPolicy::Merged,
        ] {
            for scope in [EvidenceScope::DiskData, EvidenceScope::ChdHeaderSha1] {
                let mut clone_set = MachineSet::new("clone");
                clone_set.parent_clone = Some(SetName::new("parent"));
                let graph = MachineDependencyCatalog::new(
                    snapshot(),
                    vec![clone_set, MachineSet::new("parent")],
                );
                let closure = graph.resolve(&SetName::new("clone"));
                let mut child = entry("clone", "child.chd", Some(8), Some("parent.chd"));
                child.expected.scope = scope;
                let mut parent = entry("parent", "parent.chd", Some(8), None);
                parent.expected.scope = scope;
                let mut resolved_clone = resolved("clone", vec![child]);
                resolved_clone.parent_clone = Some(SetName::new("parent"));

                let plan = plan_mame_layout(
                    policy,
                    &snapshot(),
                    &[SetName::new("clone")],
                    &[closure],
                    &[resolved_clone, resolved("parent", vec![parent])],
                );

                assert!(!plan.is_complete(), "{policy:?} accepted {scope:?}");
                assert!(plan.diagnostics.iter().any(|diagnostic| matches!(
                    diagnostic,
                    MameLayoutDiagnostic::MergeContentMismatch { conflicts, .. }
                        if conflicts.contains(&ContentEvidenceConflict::InsufficientMatchEvidence)
                )));
                let output_group = match policy {
                    MameSetLayoutPolicy::Merged => "parent",
                    MameSetLayoutPolicy::Split | MameSetLayoutPolicy::NonMerged => "clone",
                };
                assert!(
                    names(&plan, output_group).contains(&"child.chd".to_owned()),
                    "{policy:?} discarded the child asset with {scope:?} evidence"
                );
            }
        }
    }

    #[test]
    fn coalescing_compares_against_every_prior_requirement() {
        let graph = MachineDependencyCatalog::new(snapshot(), vec![MachineSet::new("root")]);
        let closure = graph.resolve(&SetName::new("root"));
        let mut first = entry("root", "first.rom", Some(8), None);
        first.path = LogicalPath::new("shared.rom");
        let mut second = entry("root", "second.rom", Some(8), None);
        second.path = LogicalPath::new("shared.rom");
        second.expected.size = Some(1);
        let mut third = entry("root", "third.rom", Some(8), None);
        third.path = LogicalPath::new("shared.rom");
        third.expected.size = Some(2);
        let plan = plan_mame_layout(
            MameSetLayoutPolicy::NonMerged,
            &snapshot(),
            &[SetName::new("root")],
            &[closure],
            &[resolved("root", vec![first, second, third])],
        );
        assert!(!plan.is_complete());
        assert!(plan.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MameLayoutDiagnostic::LogicalPathCollision {
                evidence: PathCollisionEvidence::ConflictingExpectedEvidence,
                ..
            }
        )));
    }

    #[test]
    fn coalesces_many_requirements_for_one_shared_path() {
        const REQUIREMENTS: usize = 4_096;

        let graph = MachineDependencyCatalog::new(snapshot(), vec![MachineSet::new("root")]);
        let closure = graph.resolve(&SetName::new("root"));
        let assets = (0..REQUIREMENTS)
            .map(|index| {
                let mut asset = entry("root", &format!("asset-{index}.rom"), Some(7), None);
                asset.path = LogicalPath::new("shared.rom");
                asset
            })
            .collect();

        let plan = plan_mame_layout(
            MameSetLayoutPolicy::NonMerged,
            &snapshot(),
            &[SetName::new("root")],
            &[closure],
            &[resolved("root", assets)],
        );

        assert!(plan.is_complete());
        assert_eq!(plan.groups[0].entries.len(), 1);
        assert_eq!(
            plan.coalesced_provenance[0].requirements.len(),
            REQUIREMENTS
        );
    }
}
