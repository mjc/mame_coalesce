//! Pure MAME layout planning over snapshot-scoped dependencies and resolved assets.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    domain::{LogicalEntry, LogicalPath, OutputGroup, RequirementKey, SetName, SnapshotKey},
    machine_dependencies::{DependencyClosure, DependencyDiagnostic},
};

/// Explicit MAME archive policy. Legacy build modes intentionally remain separate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MameSetLayoutPolicy {
    /// Put a selected machine and its runtime dependency closure in one self-contained group.
    NonMerged,
}

/// Resolved content for one set in one immutable catalog snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedMachineSet {
    pub snapshot: SnapshotKey,
    pub name: SetName,
    /// Preserved source assertion; it does not add the parent to runtime closure.
    pub parent_clone: Option<SetName>,
    pub entries: Vec<LogicalEntry>,
    /// Requirements that did not resolve to local content.
    pub missing_assets: Vec<RequirementKey>,
}

impl ResolvedMachineSet {
    #[must_use]
    pub const fn new(snapshot: SnapshotKey, name: SetName) -> Self {
        Self {
            snapshot,
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
    InsufficientSha1,
    CaseInsensitivePath,
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
}

/// Multiple catalog requirements represented by one established identical output entry.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CoalescedAssetProvenance {
    pub group: LogicalPath,
    pub path: LogicalPath,
    pub requirements: BTreeSet<RequirementKey>,
}

/// Format-neutral non-merged groups, coalescing provenance, and explicit diagnostics.
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

/// Plan one self-contained output group per selected machine, including its shared dependencies.
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
        if !seen_roots.insert(root) {
            plan.diagnostics
                .insert(MameLayoutDiagnostic::DuplicateRoot { root: root.clone() });
            continue;
        }
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
        let group = plan_root_group(root, closure, &sets_by_key, &sets_by_name, &mut plan);
        plan.groups.push(group);
    }
    plan.groups
        .sort_by(|left, right| left.path.cmp(&right.path));
    plan.coalesced_provenance.sort();
    plan
}

fn plan_root_group(
    root: &SetName,
    closure: &DependencyClosure,
    sets_by_key: &BTreeMap<(SnapshotKey, SetName), Vec<&ResolvedMachineSet>>,
    sets_by_name: &BTreeMap<SetName, Vec<&ResolvedMachineSet>>,
    plan: &mut MameLayoutPlan,
) -> OutputGroup {
    for diagnostic in &closure.diagnostics {
        plan.diagnostics.insert(MameLayoutDiagnostic::Dependency {
            root: root.clone(),
            diagnostic: diagnostic.clone(),
        });
    }

    let group_path = LogicalPath::new(root.as_str());
    let mut entries = BTreeMap::<LogicalPath, LogicalEntry>::new();
    let mut provenance = BTreeMap::<LogicalPath, BTreeSet<RequirementKey>>::new();
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
        for entry in effective_set_entries(root, resolved_set, sets_by_key, sets_by_name, plan) {
            insert_entry(
                root,
                &entry,
                &mut entries,
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
                requirements,
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
        child = matches[0];
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
    let mut entries = BTreeMap::<LogicalPath, Vec<LogicalEntry>>::new();
    let mut missing_by_set = Vec::<(SetName, Vec<RequirementKey>)>::new();
    let mut replaced_assets = BTreeSet::<String>::new();
    let mut is_base_set = true;
    for set in lineage.into_iter().rev() {
        let mut layer = BTreeMap::<LogicalPath, Vec<LogicalEntry>>::new();
        if !set.missing_assets.is_empty() {
            missing_by_set.push((set.name.clone(), set.missing_assets.clone()));
        }
        for entry in &set.entries {
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
            if let Some(merged_name) = &entry.expected.merge {
                entries.remove(&LogicalPath::new(merged_name));
                if !is_base_set {
                    replaced_assets.insert(merged_name.clone());
                }
            }
            // A clone declaration takes precedence over the inherited asset at that path.
            if !is_base_set {
                replaced_assets.insert(entry.path.as_str().to_owned());
            }
            layer
                .entry(entry.path.clone())
                .or_default()
                .push(entry.clone());
        }
        for entries in layer.values_mut() {
            entries.sort_by(|left, right| {
                left.requirement
                    .cmp(&right.requirement)
                    .then_with(|| left.expected.cmp(&right.expected))
            });
        }
        entries.extend(layer);
        is_base_set = false;
    }
    for (set, requirements) in missing_by_set {
        let mut requirements: Vec<_> = requirements
            .into_iter()
            .filter(|requirement| !replaced_assets.contains(requirement.rom_name()))
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
    entries.into_values().flatten().collect()
}

fn insert_entry(
    root: &SetName,
    incoming: &LogicalEntry,
    entries: &mut BTreeMap<LogicalPath, LogicalEntry>,
    provenance: &mut BTreeMap<LogicalPath, BTreeSet<RequirementKey>>,
    diagnostics: &mut BTreeSet<MameLayoutDiagnostic>,
) {
    let path = incoming.path.clone();
    let Some((existing_path, existing)) = entries
        .iter()
        .find(|(existing_path, _)| existing_path.as_str().eq_ignore_ascii_case(path.as_str()))
    else {
        entries.insert(path.clone(), incoming.clone());
        provenance
            .entry(path)
            .or_default()
            .insert(incoming.requirement.clone());
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
    if existing.expected.sha1.is_some() && existing.expected.sha1 == incoming.expected.sha1 {
        if existing.requirement != incoming.requirement {
            let requirements = provenance.entry(path).or_default();
            requirements.insert(existing.requirement.clone());
            requirements.insert(incoming.requirement.clone());
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
        } else {
            PathCollisionEvidence::InsufficientSha1
        },
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::{
            ArchiveBackend, ArchiveMemberSelector, CatalogKey, EvidenceProvenance, EvidenceScope,
            ExpectedEvidence, MatchingPolicy, ObservedContent, ScanProvenance, SelectionProvenance,
            SetKey, SourceFile, SourceLocation, SourceRoot,
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
        LogicalEntry {
            path: LogicalPath::new(name),
            source: SourceFile {
                source_root: SourceRoot::new("/roms"),
                location: SourceLocation::BareFile {
                    path: format!("/roms/{name}"),
                },
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
            },
        }
    }

    fn resolved(name: &str, entries: Vec<LogicalEntry>) -> ResolvedMachineSet {
        let mut set = ResolvedMachineSet::new(snapshot(), SetName::new(name));
        set.entries = entries;
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
        let reverse_content = resolved("root", content.entries.iter().cloned().rev().collect());
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
        root_content.missing_assets.push(RequirementKey::new(
            SetKey::new(CatalogKey::new("layout-fixture"), "root"),
            "required.rom",
        ));
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
            &[closure],
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
    }
}
