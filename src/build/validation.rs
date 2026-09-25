//! Conservative, format-neutral validation for logical plans and their destinations.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};

use crate::domain::{BuildPlan, ExpectedEvidence, LogicalPath};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentRelation {
    SameEstablishedContent,
    DifferentContent,
    UnknownContent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanIssueKind {
    UnsafeGroupPath,
    UnsafeEntryPath,
    DuplicateGroup,
    GroupFileDirectoryConflict,
    DuplicateEntry(ContentRelation),
    EntryFileDirectoryConflict,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanIssue {
    pub kind: PlanIssueKind,
    pub path: LogicalPath,
    pub conflicts_with: Option<LogicalPath>,
}

impl std::fmt::Display for PlanIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self.kind {
            PlanIssueKind::UnsafeGroupPath => "unsafe logical output group path",
            PlanIssueKind::UnsafeEntryPath => "unsafe logical entry path",
            PlanIssueKind::DuplicateGroup => "duplicate logical output group path",
            PlanIssueKind::GroupFileDirectoryConflict => "group file/directory path conflict",
            PlanIssueKind::DuplicateEntry(_) => "duplicate logical entry path",
            PlanIssueKind::EntryFileDirectoryConflict => "entry file/directory path conflict",
        };
        write!(f, "{label}: {}", self.path.as_str())?;
        if let Some(other) = &self.conflicts_with {
            write!(f, " conflicts with {}", other.as_str())?;
        }
        if let PlanIssueKind::DuplicateEntry(relation) = self.kind {
            write!(f, " ({relation:?})")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanValidation {
    pub issues: Vec<PlanIssue>,
}

impl std::fmt::Display for PlanValidation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "logical plan validation failed: ")?;
        for (index, issue) in self.issues.iter().enumerate() {
            if index > 0 {
                f.write_str("; ")?;
            }
            issue.fmt(f)?;
        }
        Ok(())
    }
}

impl std::error::Error for PlanValidation {}

/// A plan that passed the shared logical-path, collision and portable-name checks.
#[derive(Debug)]
pub struct ValidatedPlan<'plan> {
    plan: &'plan BuildPlan,
}

impl<'plan> ValidatedPlan<'plan> {
    #[must_use]
    pub const fn plan(&self) -> &'plan BuildPlan {
        self.plan
    }
}

/// Apply the repository's conservative portable naming profile and reject every collision.
/// The profile is ASCII-only and case-insensitive across common filesystems.
pub fn validate_plan(plan: &BuildPlan) -> Result<ValidatedPlan<'_>, PlanValidation> {
    let issues = inspect_plan(plan);
    if issues.is_empty() {
        Ok(ValidatedPlan { plan })
    } else {
        Err(PlanValidation { issues })
    }
}

pub(crate) fn inspect_plan(plan: &BuildPlan) -> Vec<PlanIssue> {
    let mut issues = Vec::new();
    let mut groups = BTreeMap::<String, Vec<&LogicalPath>>::new();

    for group in &plan.groups {
        let path = group.path.as_str();
        if !is_safe_relative_path(path) {
            issues.push(PlanIssue {
                kind: PlanIssueKind::UnsafeGroupPath,
                path: group.path.clone(),
                conflicts_with: None,
            });
        }

        let folded = path.to_ascii_lowercase();
        groups.entry(folded).or_default().push(&group.path);

        for entry in &group.entries {
            if !is_safe_relative_path(entry.path.as_str()) {
                issues.push(PlanIssue {
                    kind: PlanIssueKind::UnsafeEntryPath,
                    path: entry.path.clone(),
                    conflicts_with: None,
                });
            }
        }

        let mut entries = BTreeMap::<String, Vec<&crate::domain::LogicalEntry>>::new();
        for entry in &group.entries {
            let path = entry.path.as_str();
            let folded = path.to_ascii_lowercase();
            entries.entry(folded).or_default().push(entry);
        }
        for duplicates in entries.values_mut() {
            duplicates.sort_by(|left, right| {
                left.path
                    .cmp(&right.path)
                    .then_with(|| left.expected.cmp(&right.expected))
                    .then_with(|| left.requirement.cmp(&right.requirement))
            });
            for index in 0..duplicates.len() {
                for duplicate_index in index + 1..duplicates.len() {
                    let first = duplicates[index];
                    let duplicate = duplicates[duplicate_index];
                    issues.push(PlanIssue {
                        kind: PlanIssueKind::DuplicateEntry(content_relation(
                            &first.expected,
                            &duplicate.expected,
                        )),
                        path: duplicate.path.clone(),
                        conflicts_with: Some(first.path.clone()),
                    });
                }
            }
        }

        let entry_paths = group
            .entries
            .iter()
            .map(|entry| entry.path.as_str().to_ascii_lowercase())
            .collect::<BTreeSet<_>>();
        for path in &entry_paths {
            for slash in path.match_indices('/').map(|(index, _)| index) {
                let prefix = &path[..slash];
                if entry_paths.contains(prefix) {
                    issues.push(PlanIssue {
                        kind: PlanIssueKind::EntryFileDirectoryConflict,
                        path: LogicalPath::new(path),
                        conflicts_with: Some(LogicalPath::new(prefix)),
                    });
                }
            }
        }
    }

    for duplicates in groups.values_mut() {
        duplicates.sort();
        for index in 0..duplicates.len() {
            for duplicate_index in index + 1..duplicates.len() {
                issues.push(PlanIssue {
                    kind: PlanIssueKind::DuplicateGroup,
                    path: duplicates[duplicate_index].clone(),
                    conflicts_with: Some(duplicates[index].clone()),
                });
            }
        }
    }

    let group_paths = groups.keys().cloned().collect::<BTreeSet<_>>();
    for path in &group_paths {
        for slash in path.match_indices('/').map(|(index, _)| index) {
            let prefix = &path[..slash];
            if group_paths.contains(prefix) {
                issues.push(PlanIssue {
                    kind: PlanIssueKind::GroupFileDirectoryConflict,
                    path: LogicalPath::new(path),
                    conflicts_with: Some(LogicalPath::new(prefix)),
                });
            }
        }
    }

    issues.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| format!("{:?}", left.kind).cmp(&format!("{:?}", right.kind)))
    });
    issues
}

pub(crate) fn is_safe_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !has_windows_drive_prefix(path)
        && path.split('/').all(is_safe_component)
}

fn is_safe_component(component: &str) -> bool {
    if component.is_empty()
        || component == "."
        || component == ".."
        || component.ends_with(['.', ' '])
        || !component.is_ascii()
        || component.chars().any(|ch| {
            ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '|' | '?' | '*' | '\\')
        })
    {
        return false;
    }

    let stem = component
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        && !["COM", "LPT"].into_iter().any(|prefix| {
            stem.strip_prefix(prefix).is_some_and(|suffix| {
                suffix.len() == 1 && suffix.as_bytes()[0].is_ascii_digit() && suffix != "0"
            })
        })
}

const fn has_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn content_relation(left: &ExpectedEvidence, right: &ExpectedEvidence) -> ContentRelation {
    if left.scope != crate::domain::EvidenceScope::WholeAsset
        || right.scope != crate::domain::EvidenceScope::WholeAsset
    {
        return ContentRelation::UnknownContent;
    }

    let mut comparable = false;
    macro_rules! compare {
        ($field:ident) => {
            if let (Some(left), Some(right)) = (&left.$field, &right.$field) {
                comparable = true;
                if left != right {
                    return ContentRelation::DifferentContent;
                }
            }
        };
    }
    compare!(sha1);
    compare!(md5);
    compare!(crc);
    compare!(size);
    if comparable {
        ContentRelation::SameEstablishedContent
    } else {
        ContentRelation::UnknownContent
    }
}

/// Reject output roots equal to, containing, or contained by any source root.
pub fn ensure_sources_disjoint_from_destination(
    sources: &[&Utf8Path],
    destination: &Utf8Path,
) -> crate::Result<()> {
    let destination = canonicalize_destination(destination)?;
    for source in sources {
        let source = source.canonicalize_utf8()?;
        if source.starts_with(&destination) || destination.starts_with(&source) {
            return Err(crate::Error::InvalidPath(format!(
                "source/destination overlap is not allowed: source={source} destination={destination}"
            )));
        }
    }
    Ok(())
}

fn canonicalize_destination(destination: &Utf8Path) -> crate::Result<Utf8PathBuf> {
    let absolute = if destination.is_absolute() {
        destination.to_path_buf()
    } else {
        Utf8PathBuf::try_from(std::env::current_dir()?.join(destination))
            .map_err(|_| crate::Error::InvalidPath("destination path is not UTF-8".to_owned()))?
    };
    let normalized = normalize_absolute(absolute.as_std_path())?;
    let mut ancestor = normalized;
    let mut suffix = Vec::<String>::new();
    while match fs::symlink_metadata(&ancestor) {
        Ok(_) => false,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
        Err(error) => return Err(error.into()),
    } {
        let Some(name) = ancestor.file_name().map(str::to_owned) else {
            return Err(crate::Error::InvalidPath(format!(
                "destination has no existing ancestor: {destination}"
            )));
        };
        suffix.push(name);
        if !ancestor.pop() {
            return Err(crate::Error::InvalidPath(format!(
                "destination has no existing ancestor: {destination}"
            )));
        }
    }

    let mut canonical = ancestor.canonicalize_utf8()?;
    for component in suffix.iter().rev() {
        canonical.push(component);
    }
    Ok(canonical)
}

fn normalize_absolute(path: &Path) -> crate::Result<Utf8PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(name) => normalized.push(name),
        }
    }
    Utf8PathBuf::try_from(normalized)
        .map_err(|_| crate::Error::InvalidPath("destination path is not UTF-8".to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{BuildReport, LogicalEntry, OutputGroup, PlanBlockReason, PlanOutcome};

    fn empty_plan(groups: Vec<OutputGroup>) -> BuildPlan {
        BuildPlan {
            groups,
            report: BuildReport {
                outcome: PlanOutcome::Blocked(PlanBlockReason::MissingContent),
                ..BuildReport::default()
            },
        }
    }

    fn group(path: &str, entries: Vec<LogicalEntry>) -> OutputGroup {
        OutputGroup {
            path: LogicalPath::new(path),
            entries,
        }
    }

    fn entry(path: &str, expected: ExpectedEvidence) -> LogicalEntry {
        LogicalEntry {
            path: LogicalPath::new(path),
            source: crate::domain::SourceFile {
                source_root: crate::domain::SourceRoot::new("/unused"),
                location: crate::domain::SourceLocation::BareFile {
                    path: "/unused/file".to_owned(),
                },
                observed: crate::domain::ObservedContent {
                    scope: crate::domain::EvidenceScope::WholeAsset,
                    provenance: crate::domain::EvidenceProvenance::Computed,
                    size: None,
                    crc: None,
                    md5: None,
                    sha1: None,
                    xxh3: [0; 8],
                },
                fingerprint: None,
                scan_run: None,
                scan_provenance: None,
            },
            requirement: crate::domain::RequirementKey::new(
                crate::domain::SetKey::new(crate::domain::CatalogKey::new("test"), path),
                path,
            ),
            expected,
            selection: crate::domain::SelectionProvenance {
                policy: crate::domain::MatchingPolicy::Sha1Compatibility,
                strength: crate::resolution::MatchStrength::Sha1,
                assessments: Vec::new(),
            },
        }
    }

    #[test]
    fn rejects_unsafe_portable_names() {
        for name in ["../outside", "CON.txt", "bad.", "bad ", "a\\b", "É.rom"] {
            assert!(!is_safe_relative_path(name), "accepted {name:?}");
        }
        for name in ["safe/name.rom", "safe-name.rom", "Disk 1.rom"] {
            assert!(is_safe_relative_path(name), "rejected {name:?}");
        }
    }

    #[test]
    fn rejects_casefolded_groups_and_file_directory_prefixes() {
        let plan = empty_plan(vec![
            group("Games/Parent", Vec::new()),
            group("games/parent", Vec::new()),
            group("games/parent/clone", Vec::new()),
        ]);

        let issues = inspect_plan(&plan);
        assert!(
            issues
                .iter()
                .any(|issue| issue.kind == PlanIssueKind::DuplicateGroup)
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.kind == PlanIssueKind::GroupFileDirectoryConflict)
        );
    }

    #[test]
    fn duplicate_entries_report_same_different_and_unknown_content() {
        let expected = ExpectedEvidence {
            scope: crate::domain::EvidenceScope::WholeAsset,
            sha1: Some([1; 20]),
            ..ExpectedEvidence::default()
        };
        let other = ExpectedEvidence {
            scope: crate::domain::EvidenceScope::WholeAsset,
            sha1: Some([2; 20]),
            ..ExpectedEvidence::default()
        };
        let plan = empty_plan(vec![group(
            "set",
            vec![
                entry("same.rom", expected.clone()),
                entry("SAME.rom", expected),
                entry("same.rom", other),
                entry("same.rom", ExpectedEvidence::default()),
                entry("folder", ExpectedEvidence::default()),
                entry("folder/child.rom", ExpectedEvidence::default()),
            ],
        )]);

        let issues = inspect_plan(&plan);
        assert!(issues.iter().any(|issue| issue.kind
            == PlanIssueKind::DuplicateEntry(ContentRelation::SameEstablishedContent)));
        assert!(
            issues.iter().any(|issue| issue.kind
                == PlanIssueKind::DuplicateEntry(ContentRelation::DifferentContent))
        );
        assert!(
            issues.iter().any(|issue| issue.kind
                == PlanIssueKind::DuplicateEntry(ContentRelation::UnknownContent))
        );
        assert!(
            issues
                .iter()
                .any(|issue| issue.kind == PlanIssueKind::EntryFileDirectoryConflict)
        );
    }

    #[test]
    fn validation_diagnostics_are_stable_under_group_and_entry_permutations() {
        let expected = ExpectedEvidence {
            scope: crate::domain::EvidenceScope::WholeAsset,
            sha1: Some([1; 20]),
            ..ExpectedEvidence::default()
        };
        let first = group(
            "Games/Set",
            vec![
                entry("rom.bin", expected.clone()),
                entry("ROM.BIN", expected.clone()),
                entry("rom.bin", ExpectedEvidence::default()),
            ],
        );
        let second = group("games/set", vec![entry("other.bin", expected)]);
        let original = inspect_plan(&empty_plan(vec![first.clone(), second.clone()]));
        let reordered = inspect_plan(&empty_plan(vec![
            group("games/set", second.entries.into_iter().rev().collect()),
            group("Games/Set", first.entries.into_iter().rev().collect()),
        ]));

        assert_eq!(reordered, original);
    }

    #[test]
    fn source_destination_overlap_detects_equal_nested_and_aliased_roots() -> crate::Result<()> {
        let temp = tempfile::tempdir()?;
        let source_dir = temp.path().join("input");
        std::fs::create_dir_all(&source_dir)?;
        let source = Utf8Path::from_path(&source_dir)
            .ok_or_else(|| std::io::Error::other("temporary path is not UTF-8"))?;
        let nested = source.join("generated/deep");
        std::fs::create_dir_all(&nested)?;
        let sibling = source
            .parent()
            .ok_or_else(|| std::io::Error::other("source path has no parent"))?
            .join("input-output");
        assert!(ensure_sources_disjoint_from_destination(&[source], source).is_err());
        assert!(ensure_sources_disjoint_from_destination(&[source], &nested).is_err());
        assert!(ensure_sources_disjoint_from_destination(&[&nested], source).is_err());
        assert!(ensure_sources_disjoint_from_destination(&[source], &sibling).is_ok());

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let alias = source.join("source-alias");
            symlink(source, &alias)?;
            let destination = alias.join("generated");
            assert!(ensure_sources_disjoint_from_destination(&[source], &destination).is_err());
        }
        Ok(())
    }
}
