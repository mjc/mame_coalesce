//! Catalog-scoped MAME machine dependencies, separate from parent/clone ancestry.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::domain::{SetName, SnapshotKey};

/// The source-declared kind of a machine's runtime dependency edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MachineDependencyKind {
    RomOf,
    DeviceReference,
}

impl MachineDependencyKind {
    #[must_use]
    pub const fn source_field(self) -> &'static str {
        match self {
            Self::RomOf => "romof",
            Self::DeviceReference => "device_ref",
        }
    }
}

/// A machine's normalized runtime dependency, retaining its source field.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MachineDependency {
    pub kind: MachineDependencyKind,
    pub target: SetName,
}

/// A normalized set record from one immutable catalog snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineSet {
    pub name: SetName,
    pub is_bios: bool,
    pub is_device: bool,
    /// Kept as source ancestry; it is deliberately not traversed as a dependency.
    pub parent_clone: Option<SetName>,
    pub dependencies: Vec<MachineDependency>,
    /// Relationships retained by the source but not supported as runtime edges.
    pub unsupported_relationships: Vec<(String, SetName)>,
}

impl MachineSet {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: SetName::new(name),
            is_bios: false,
            is_device: false,
            parent_clone: None,
            dependencies: Vec::new(),
            unsupported_relationships: Vec::new(),
        }
    }
}

/// A graph is always bound to exactly one catalog snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineDependencyCatalog {
    pub snapshot: SnapshotKey,
    pub completeness: SnapshotCompleteness,
    pub sets: Vec<MachineSet>,
}

/// How confidently absence from this snapshot can be interpreted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "sets", rename_all = "snake_case")]
pub enum SnapshotCompleteness {
    Complete,
    /// A declared finite set scope; `None` means the filter could not be decoded.
    Filtered(Option<BTreeSet<SetName>>),
    Partial,
    Unknown,
}

impl MachineDependencyCatalog {
    #[must_use]
    pub const fn new(snapshot: SnapshotKey, sets: Vec<MachineSet>) -> Self {
        Self {
            snapshot,
            completeness: SnapshotCompleteness::Complete,
            sets,
        }
    }

    #[must_use]
    pub const fn with_completeness(
        snapshot: SnapshotKey,
        completeness: SnapshotCompleteness,
        sets: Vec<MachineSet>,
    ) -> Self {
        Self {
            snapshot,
            completeness,
            sets,
        }
    }

    /// Resolve the transitive runtime closure without consulting local ROM files.
    #[must_use]
    pub fn resolve(&self, root: &SetName) -> DependencyClosure {
        let mut by_name = BTreeMap::<&SetName, Vec<&MachineSet>>::new();
        for set in &self.sets {
            by_name.entry(&set.name).or_default().push(set);
        }

        let mut result = DependencyClosure {
            snapshot: self.snapshot.clone(),
            completeness: self.completeness.clone(),
            root: root.clone(),
            sets: BTreeSet::new(),
            edges: BTreeSet::new(),
            diagnostics: BTreeSet::new(),
        };
        let mut visiting = Vec::new();
        let mut visited = BTreeSet::new();
        Self::visit(
            root,
            &self.completeness,
            &by_name,
            &mut visiting,
            &mut visited,
            &mut result,
        );
        result
    }

    fn visit(
        name: &SetName,
        completeness: &SnapshotCompleteness,
        by_name: &BTreeMap<&SetName, Vec<&MachineSet>>,
        visiting: &mut Vec<SetName>,
        visited: &mut BTreeSet<SetName>,
        result: &mut DependencyClosure,
    ) {
        let Some(matches) = by_name.get(name) else {
            result.diagnostics.insert(absence_diagnostic(
                completeness,
                name,
                visiting.last().cloned(),
            ));
            return;
        };
        if matches.len() != 1 {
            result
                .diagnostics
                .insert(DependencyDiagnostic::AmbiguousSet {
                    name: name.clone(),
                    matches: matches.len(),
                    required_by: visiting.last().cloned(),
                });
            return;
        }
        if let Some(cycle_start) = visiting.iter().position(|ancestor| ancestor == name) {
            let mut cycle = visiting[cycle_start..].to_vec();
            cycle.push(name.clone());
            result
                .diagnostics
                .insert(DependencyDiagnostic::Cycle { path: cycle });
            return;
        }
        if !visited.insert(name.clone()) {
            return;
        }

        let set = matches[0];
        result.sets.insert(name.clone());
        visiting.push(name.clone());
        for (field, target) in &set.unsupported_relationships {
            result
                .diagnostics
                .insert(DependencyDiagnostic::UnsupportedRelationship {
                    set: name.clone(),
                    field: field.clone(),
                    target: target.clone(),
                });
        }
        let mut dependencies = set.dependencies.clone();
        dependencies.sort();
        for dependency in &dependencies {
            result.edges.insert(ResolvedMachineDependency {
                from: name.clone(),
                to: dependency.target.clone(),
                kind: dependency.kind,
                target_is_bios: by_name
                    .get(&dependency.target)
                    .is_some_and(|targets| targets.len() == 1 && targets[0].is_bios),
                target_is_device: by_name
                    .get(&dependency.target)
                    .is_some_and(|targets| targets.len() == 1 && targets[0].is_device),
            });
            Self::visit(
                &dependency.target,
                completeness,
                by_name,
                visiting,
                visited,
                result,
            );
        }
        visiting.pop();
    }
}

fn absence_diagnostic(
    completeness: &SnapshotCompleteness,
    name: &SetName,
    required_by: Option<SetName>,
) -> DependencyDiagnostic {
    match completeness {
        SnapshotCompleteness::Complete => DependencyDiagnostic::MissingSet {
            name: name.clone(),
            required_by,
        },
        SnapshotCompleteness::Filtered(Some(included)) if included.contains(name) => {
            DependencyDiagnostic::MissingSet {
                name: name.clone(),
                required_by,
            }
        }
        SnapshotCompleteness::Filtered(Some(_)) => DependencyDiagnostic::OutOfScopeSet {
            name: name.clone(),
            required_by,
        },
        SnapshotCompleteness::Filtered(None)
        | SnapshotCompleteness::Partial
        | SnapshotCompleteness::Unknown => DependencyDiagnostic::UnknownSet {
            name: name.clone(),
            required_by,
        },
    }
}

/// A resolved, snapshot-scoped runtime edge.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ResolvedMachineDependency {
    pub from: SetName,
    pub to: SetName,
    pub kind: MachineDependencyKind,
    pub target_is_bios: bool,
    pub target_is_device: bool,
}

/// Why a dependency closure could not be fully resolved or interpreted.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DependencyDiagnostic {
    MissingSet {
        name: SetName,
        required_by: Option<SetName>,
    },
    OutOfScopeSet {
        name: SetName,
        required_by: Option<SetName>,
    },
    UnknownSet {
        name: SetName,
        required_by: Option<SetName>,
    },
    AmbiguousSet {
        name: SetName,
        matches: usize,
        required_by: Option<SetName>,
    },
    Cycle {
        path: Vec<SetName>,
    },
    UnsupportedRelationship {
        set: SetName,
        field: String,
        target: SetName,
    },
}

/// Stable closure contents and all explicit diagnostics for one requested set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyClosure {
    pub snapshot: SnapshotKey,
    pub completeness: SnapshotCompleteness,
    pub root: SetName,
    pub sets: BTreeSet<SetName>,
    pub edges: BTreeSet<ResolvedMachineDependency>,
    pub diagnostics: BTreeSet<DependencyDiagnostic>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog(sets: Vec<MachineSet>) -> MachineDependencyCatalog {
        MachineDependencyCatalog::new(
            SnapshotKey::new(
                &crate::domain::CatalogKey::new("test"),
                &crate::domain::DocumentKey::from_bytes(b"dependencies"),
                &crate::domain::ParserInterpretationKey::for_format(
                    "test",
                    &crate::domain::CatalogScope::Unknown,
                ),
            ),
            sets,
        )
    }

    fn edge(kind: MachineDependencyKind, target: &str) -> MachineDependency {
        MachineDependency {
            kind,
            target: SetName::new(target),
        }
    }

    #[test]
    fn resolves_shared_bios_and_device_closure_in_stable_order() {
        let mut first = MachineSet::new("first");
        first.dependencies = vec![
            edge(MachineDependencyKind::DeviceReference, "sound"),
            edge(MachineDependencyKind::RomOf, "bios"),
        ];
        let mut second = MachineSet::new("second");
        second.dependencies = vec![edge(MachineDependencyKind::RomOf, "bios")];
        let mut bios = MachineSet::new("bios");
        bios.is_bios = true;
        let mut device = MachineSet::new("sound");
        device.is_device = true;

        let input = vec![first, second, bios, device];
        let first_result = catalog(input.clone()).resolve(&SetName::new("first"));
        let mut permuted = input.clone();
        permuted.reverse();
        permuted[3].dependencies.reverse();
        let permuted_result = catalog(permuted).resolve(&SetName::new("first"));
        let second_result = catalog(input).resolve(&SetName::new("second"));
        assert_eq!(
            first_result
                .sets
                .iter()
                .map(SetName::as_str)
                .collect::<Vec<_>>(),
            ["bios", "first", "sound"]
        );
        assert_eq!(first_result, permuted_result);
        assert!(first_result.diagnostics.is_empty());
        assert!(first_result.edges.iter().any(|dependency| {
            dependency.kind == MachineDependencyKind::RomOf
                && dependency.to == SetName::new("bios")
                && dependency.target_is_bios
        }));
        assert!(first_result.edges.iter().any(|dependency| {
            dependency.kind == MachineDependencyKind::DeviceReference && dependency.target_is_device
        }));
        assert_eq!(
            second_result.sets,
            BTreeSet::from([SetName::new("bios"), SetName::new("second")])
        );
    }

    #[test]
    fn clone_ancestry_is_retained_but_not_traversed() {
        let mut clone = MachineSet::new("clone");
        clone.parent_clone = Some(SetName::new("parent"));
        let result = catalog(vec![clone]).resolve(&SetName::new("clone"));
        assert_eq!(result.sets, BTreeSet::from([SetName::new("clone")]));
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn reports_missing_ambiguous_and_unsupported_relationships() {
        let mut root = MachineSet::new("root");
        root.dependencies = vec![edge(MachineDependencyKind::RomOf, "absent")];
        root.unsupported_relationships
            .push(("sampleof".into(), SetName::new("samples")));
        let result = catalog(vec![root]).resolve(&SetName::new("root"));
        assert!(
            result
                .diagnostics
                .contains(&DependencyDiagnostic::MissingSet {
                    name: SetName::new("absent"),
                    required_by: Some(SetName::new("root")),
                })
        );
        assert!(
            result
                .diagnostics
                .contains(&DependencyDiagnostic::UnsupportedRelationship {
                    set: SetName::new("root"),
                    field: "sampleof".into(),
                    target: SetName::new("samples"),
                })
        );

        let ambiguous = catalog(vec![MachineSet::new("dup"), MachineSet::new("dup")])
            .resolve(&SetName::new("dup"));
        assert!(
            ambiguous
                .diagnostics
                .contains(&DependencyDiagnostic::AmbiguousSet {
                    name: SetName::new("dup"),
                    matches: 2,
                    required_by: None,
                })
        );
    }

    #[test]
    fn reports_cycles_without_losing_reachable_sets() {
        let mut first = MachineSet::new("first");
        first.dependencies = vec![edge(MachineDependencyKind::RomOf, "second")];
        let mut second = MachineSet::new("second");
        second.dependencies = vec![edge(MachineDependencyKind::RomOf, "first")];
        let result = catalog(vec![second, first]).resolve(&SetName::new("first"));
        assert!(result.diagnostics.contains(&DependencyDiagnostic::Cycle {
            path: vec![
                SetName::new("first"),
                SetName::new("second"),
                SetName::new("first"),
            ],
        }));
        assert_eq!(result.sets.len(), 2);
    }

    #[test]
    fn equal_names_in_different_snapshots_never_cross_resolve() {
        let mut root = MachineSet::new("root");
        root.dependencies = vec![edge(MachineDependencyKind::RomOf, "bios")];
        let first = catalog(vec![root.clone(), MachineSet::new("bios")]);
        let second = MachineDependencyCatalog::new(
            SnapshotKey::new(
                &crate::domain::CatalogKey::new("other"),
                &crate::domain::DocumentKey::from_bytes(b"other"),
                &crate::domain::ParserInterpretationKey::for_format(
                    "test",
                    &crate::domain::CatalogScope::Unknown,
                ),
            ),
            vec![root],
        );
        assert_ne!(first.snapshot, second.snapshot);
        assert_eq!(first.resolve(&SetName::new("root")).sets.len(), 2);
        assert!(second.resolve(&SetName::new("root")).diagnostics.contains(
            &DependencyDiagnostic::MissingSet {
                name: SetName::new("bios"),
                required_by: Some(SetName::new("root")),
            }
        ));
    }
}
