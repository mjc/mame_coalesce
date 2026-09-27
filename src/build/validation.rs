//! Conservative, format-neutral validation for logical plans and their destinations.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::{Component, PathBuf},
};

use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};

use crate::domain::{BuildPlan, ExpectedEvidence, LogicalPath, SourceLocation};

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
            PlanIssueKind::UnsafeGroupPath => "unsafe output zip file name",
            PlanIssueKind::UnsafeEntryPath => "unsafe zip entry name",
            PlanIssueKind::DuplicateGroup => "duplicate output zip file name",
            PlanIssueKind::GroupFileDirectoryConflict => "ZIP artifact file/directory conflict",
            PlanIssueKind::DuplicateEntry(_) => "duplicate zip entry name",
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

/// A destination proven disjoint from every declared root and actual plan source.
#[derive(Debug)]
pub(crate) struct CheckedPlanDestination {
    path: Utf8PathBuf,
    sources: Vec<Utf8PathBuf>,
}

impl CheckedPlanDestination {
    pub(crate) fn path(&self) -> &Utf8Path {
        &self.path
    }

    pub(crate) fn sources(&self) -> &[Utf8PathBuf] {
        &self.sources
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
            // Find one witness of each possible relation in linear time. There are only
            // four comparable evidence fields, so indexing every non-empty projection
            // costs a fixed amount per entry rather than comparing every pair.
            for (first, duplicate) in entry_collision_witnesses(duplicates) {
                let relation = content_relation(&first.expected, &duplicate.expected);
                issues.push(PlanIssue {
                    kind: PlanIssueKind::DuplicateEntry(relation),
                    path: duplicate.path.clone(),
                    conflicts_with: Some(first.path.clone()),
                });
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
        if let [first, duplicate, ..] = duplicates.as_slice() {
            issues.push(PlanIssue {
                kind: PlanIssueKind::DuplicateGroup,
                path: (*duplicate).clone(),
                conflicts_with: Some((*first).clone()),
            });
        }
    }

    issues.extend(group_artifact_conflicts(plan));

    issues.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| format!("{:?}", left.kind).cmp(&format!("{:?}", right.kind)))
    });
    issues
}

fn entry_collision_witnesses<'a>(
    entries: &[&'a crate::domain::LogicalEntry],
) -> Vec<(
    &'a crate::domain::LogicalEntry,
    &'a crate::domain::LogicalEntry,
)> {
    use crate::domain::EvidenceScope;

    let mut witnesses = [None; 3];
    let mut by_projection = HashMap::<(u8, u8, Vec<u8>), &crate::domain::LogicalEntry>::new();
    let mut first_by_mask = [None; 16];
    let mut first_by_field: [Option<(Vec<u8>, &'a crate::domain::LogicalEntry)>; 4] =
        std::array::from_fn(|_| None);
    let mut first_entry = None;
    let mut first_non_whole = None;

    for entry in entries {
        let expected = &entry.expected;
        if expected.scope != EvidenceScope::WholeAsset {
            if let Some(first) = first_entry {
                witnesses[2].get_or_insert((first, *entry));
            }
            first_non_whole.get_or_insert(*entry);
            first_entry.get_or_insert(*entry);
            continue;
        }
        if let Some(first) = first_non_whole {
            witnesses[2].get_or_insert((first, *entry));
        }
        first_entry.get_or_insert(*entry);

        let mask = evidence_mask(expected);
        for other_mask in 0_u8..16 {
            if mask & other_mask == 0
                && let Some(first) = first_by_mask[usize::from(other_mask)]
            {
                witnesses[2].get_or_insert((first, *entry));
            }
        }

        for other_mask in 1_u8..16 {
            let shared = mask & other_mask;
            if let Some(first) =
                by_projection.get(&(other_mask, shared, evidence_projection(expected, shared)))
            {
                let witness = if shared & 0b011 != 0 {
                    &mut witnesses[0]
                } else if shared & 0b1100 != 0 {
                    &mut witnesses[2]
                } else {
                    continue;
                };
                witness.get_or_insert((*first, *entry));
            }
        }

        for (field, first_for_field) in first_by_field.iter_mut().enumerate() {
            if mask & (1 << field) == 0 {
                continue;
            }
            let value = evidence_projection(expected, 1 << field);
            if let Some((first_value, first)) = first_for_field {
                if first_value != &value {
                    witnesses[1].get_or_insert((*first, *entry));
                }
            } else {
                *first_for_field = Some((value, *entry));
            }
        }

        first_by_mask[mask as usize].get_or_insert(*entry);
        for projection in 1_u8..16 {
            if mask & projection == projection {
                by_projection
                    .entry((mask, projection, evidence_projection(expected, projection)))
                    .or_insert(*entry);
            }
        }
    }

    witnesses.into_iter().flatten().collect()
}

fn evidence_mask(evidence: &ExpectedEvidence) -> u8 {
    u8::from(evidence.sha1.is_some())
        | (u8::from(evidence.md5.is_some()) << 1)
        | (u8::from(evidence.crc.is_some()) << 2)
        | (u8::from(evidence.size.is_some()) << 3)
}

fn evidence_projection(evidence: &ExpectedEvidence, mask: u8) -> Vec<u8> {
    let mut key = Vec::with_capacity(40);
    if mask & 1 != 0
        && let Some(sha1) = evidence.sha1
    {
        key.extend_from_slice(&sha1);
    }
    if mask & 2 != 0
        && let Some(md5) = evidence.md5
    {
        key.extend_from_slice(&md5.0);
    }
    if mask & 4 != 0
        && let Some(crc) = evidence.crc
    {
        key.extend_from_slice(&crc.0);
    }
    if mask & 8 != 0
        && let Some(size) = evidence.size
    {
        key.extend_from_slice(&size.to_be_bytes());
    }
    key
}

fn group_artifact_conflicts(plan: &BuildPlan) -> Vec<PlanIssue> {
    let artifact_paths = plan
        .groups
        .iter()
        .map(|group| format!("{}.zip", group.path.as_str()).to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let mut issues = Vec::new();
    for path in &artifact_paths {
        for slash in path.match_indices('/').map(|(index, _)| index) {
            let prefix = &path[..slash];
            if artifact_paths.contains(prefix) {
                issues.push(PlanIssue {
                    kind: PlanIssueKind::GroupFileDirectoryConflict,
                    path: LogicalPath::new(path.strip_suffix(".zip").unwrap_or(path)),
                    conflicts_with: Some(LogicalPath::new(
                        prefix.strip_suffix(".zip").unwrap_or(prefix),
                    )),
                });
            }
        }
    }
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
    !matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) && !["COM", "LPT"].into_iter().any(|prefix| {
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

    let mut matching_strong_digest = false;
    macro_rules! compare {
        ($field:ident) => {
            if let (Some(left), Some(right)) = (&left.$field, &right.$field) {
                if left != right {
                    return ContentRelation::DifferentContent;
                }
            }
        };
    }
    macro_rules! compare_digest {
        ($field:ident, $strong:expr) => {
            if let (Some(left), Some(right)) = (&left.$field, &right.$field) {
                matching_strong_digest |= $strong;
                if left != right {
                    return ContentRelation::DifferentContent;
                }
            }
        };
    }
    compare_digest!(sha1, true);
    compare_digest!(md5, true);
    compare_digest!(crc, false);
    compare!(size);
    if matching_strong_digest {
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
    checked_destination(sources, destination).map(drop)
}

pub(crate) fn checked_destination(
    sources: &[&Utf8Path],
    destination: &Utf8Path,
) -> crate::Result<Utf8PathBuf> {
    let destination = canonicalize_destination(destination)?;
    for source in sources {
        // Source files may have disappeared since the plan was created. Resolve all
        // existing path components (including symlinks), but preserve a missing
        // suffix so the writer can report that artifact as a per-file failure.
        let source = canonicalize_destination(source)?;
        if source.starts_with(&destination) || destination.starts_with(&source) {
            return Err(crate::Error::InvalidPath(format!(
                "source/destination overlap is not allowed: source={source} destination={destination}"
            )));
        }
    }
    Ok(destination)
}

pub(crate) fn checked_plan_destination(
    plan: &BuildPlan,
    destination: &Utf8Path,
) -> crate::Result<CheckedPlanDestination> {
    let mut source_roots = BTreeSet::new();
    let mut source_paths = BTreeSet::new();
    for entry in plan.groups.iter().flat_map(|group| &group.entries) {
        let declared_root = Utf8Path::new(entry.source.source_root.as_str()).canonicalize_utf8()?;
        let source_path =
            canonicalize_destination(Utf8Path::new(source_location_path(&entry.source.location)))?;
        if !source_path.starts_with(&declared_root) {
            return Err(crate::Error::InvalidPath(format!(
                "plan source is outside its declared source root: source={source_path} root={declared_root}"
            )));
        }
        source_roots.insert(declared_root);
        source_paths.insert(source_path);
    }

    let mut checked_sources = source_roots.clone();
    checked_sources.extend(source_paths);
    let checked_sources = checked_sources.into_iter().collect::<Vec<_>>();
    let source_refs = checked_sources
        .iter()
        .map(Utf8PathBuf::as_path)
        .collect::<Vec<_>>();
    let path = checked_destination(&source_refs, destination)?;
    Ok(CheckedPlanDestination {
        path,
        sources: source_roots.into_iter().collect(),
    })
}

fn source_location_path(location: &SourceLocation) -> &str {
    match location {
        SourceLocation::BareFile { path }
        | SourceLocation::ArchiveMember { path, .. }
        | SourceLocation::LegacyUnknown { path, .. } => path,
    }
}

pub(crate) fn canonicalize_destination(destination: &Utf8Path) -> crate::Result<Utf8PathBuf> {
    let absolute = if destination.is_absolute() {
        destination.to_path_buf()
    } else {
        Utf8PathBuf::try_from(std::env::current_dir()?.join(destination))
            .map_err(|_| crate::Error::InvalidPath("destination path is not UTF-8".to_owned()))?
    };
    let mut resolved = PathBuf::new();
    for component in absolute.as_std_path().components() {
        match component {
            Component::Prefix(prefix) => resolved.push(prefix.as_os_str()),
            Component::RootDir => resolved.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                resolved.pop();
            }
            Component::Normal(name) => {
                let candidate = resolved.join(name);
                match fs::canonicalize(&candidate) {
                    Ok(canonical) => resolved = canonical,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        match fs::symlink_metadata(&candidate) {
                            Ok(metadata) if metadata.file_type().is_symlink() => {
                                return Err(crate::Error::InvalidPath(format!(
                                    "dangling symlink in path: {}",
                                    candidate.display()
                                )));
                            }
                            Err(metadata_error)
                                if metadata_error.kind() == std::io::ErrorKind::NotFound =>
                            {
                                resolved.push(name);
                            }
                            Err(metadata_error) => return Err(metadata_error.into()),
                            Ok(_) => return Err(error.into()),
                        }
                    }
                    Err(error) => return Err(error.into()),
                }
            }
        }
    }
    Utf8PathBuf::try_from(resolved)
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
        for name in [
            "../outside",
            "CON.txt",
            "CONIN$",
            "CONOUT$",
            "bad.",
            "bad ",
            "a\\b",
            "É.rom",
        ] {
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
            group("games/parent.zip/clone", Vec::new()),
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
    fn validates_collisions_after_appending_zip_extension() {
        let compatible = empty_plan(vec![group("a", Vec::new()), group("a/b", Vec::new())]);
        assert!(
            !inspect_plan(&compatible)
                .iter()
                .any(|issue| { issue.kind == PlanIssueKind::GroupFileDirectoryConflict })
        );

        let conflict = empty_plan(vec![group("a", Vec::new()), group("a.zip/b", Vec::new())]);
        assert!(
            inspect_plan(&conflict)
                .iter()
                .any(|issue| { issue.kind == PlanIssueKind::GroupFileDirectoryConflict })
        );
    }

    #[test]
    fn size_alone_does_not_establish_identical_content() {
        let left = ExpectedEvidence {
            scope: crate::domain::EvidenceScope::WholeAsset,
            size: Some(123),
            ..ExpectedEvidence::default()
        };
        let right = left.clone();
        assert_eq!(
            content_relation(&left, &right),
            ContentRelation::UnknownContent
        );

        let different_size = ExpectedEvidence {
            size: Some(124),
            ..right
        };
        assert_eq!(
            content_relation(&left, &different_size),
            ContentRelation::DifferentContent
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
    fn matching_crc_without_stronger_digest_does_not_establish_content_identity() {
        let left = ExpectedEvidence {
            scope: crate::domain::EvidenceScope::WholeAsset,
            crc: Some(crate::domain::Crc32Digest([1; 4])),
            size: Some(1024),
            ..ExpectedEvidence::default()
        };
        let right = left.clone();

        assert_eq!(
            content_relation(&left, &right),
            ContentRelation::UnknownContent
        );
        assert_eq!(
            content_relation(
                &left,
                &ExpectedEvidence {
                    crc: Some(crate::domain::Crc32Digest([2; 4])),
                    ..right
                }
            ),
            ContentRelation::DifferentContent
        );
    }

    #[test]
    fn matching_crc_only_duplicates_are_reported_as_unknown_content() {
        let expected = ExpectedEvidence {
            scope: crate::domain::EvidenceScope::WholeAsset,
            crc: Some(crate::domain::Crc32Digest([1; 4])),
            size: Some(1024),
            ..ExpectedEvidence::default()
        };
        let plan = empty_plan(vec![group(
            "set",
            vec![
                entry("rom.bin", expected.clone()),
                entry("rom.bin", expected),
            ],
        )]);

        assert!(inspect_plan(&plan).iter().any(|issue| {
            issue.kind == PlanIssueKind::DuplicateEntry(ContentRelation::UnknownContent)
        }));
        assert!(validate_plan(&plan).is_err());
    }

    #[test]
    fn matching_size_only_duplicates_are_reported_as_unknown_content() {
        let expected = ExpectedEvidence {
            scope: crate::domain::EvidenceScope::WholeAsset,
            size: Some(1024),
            ..ExpectedEvidence::default()
        };
        let plan = empty_plan(vec![group(
            "set",
            vec![
                entry("rom.bin", expected.clone()),
                entry("rom.bin", expected),
            ],
        )]);

        assert!(inspect_plan(&plan).iter().any(|issue| {
            issue.kind == PlanIssueKind::DuplicateEntry(ContentRelation::UnknownContent)
        }));
        assert!(validate_plan(&plan).is_err());
    }

    #[test]
    fn crc_only_match_does_not_consume_strong_identity_witness() {
        let crc_only = ExpectedEvidence {
            scope: crate::domain::EvidenceScope::WholeAsset,
            crc: Some(crate::domain::Crc32Digest([1; 4])),
            ..ExpectedEvidence::default()
        };
        let matching_md5 = ExpectedEvidence {
            scope: crate::domain::EvidenceScope::WholeAsset,
            md5: Some(crate::domain::Md5Digest([2; 16])),
            ..ExpectedEvidence::default()
        };
        let entries = [
            entry("rom.bin", crc_only.clone()),
            entry("rom.bin", crc_only),
            entry("rom.bin", matching_md5.clone()),
            entry("rom.bin", matching_md5),
        ];
        let entries = entries.iter().collect::<Vec<_>>();

        assert!(
            entry_collision_witnesses(&entries)
                .iter()
                .any(|(left, right)| {
                    content_relation(&left.expected, &right.expected)
                        == ContentRelation::SameEstablishedContent
                })
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
    fn large_casefolded_group_class_has_one_stable_diagnostic() {
        let groups = (0..10_000)
            .map(|bits| {
                let name = (0..14)
                    .map(|bit| if bits & (1 << bit) == 0 { 'a' } else { 'A' })
                    .collect::<String>();
                group(&name, Vec::new())
            })
            .collect::<Vec<_>>();
        let original = empty_plan(groups.clone());
        let reversed = empty_plan(groups.into_iter().rev().collect());

        let issues = inspect_plan(&original);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].kind, PlanIssueKind::DuplicateGroup);
        assert_eq!(inspect_plan(&reversed), issues);
        assert!(validate_plan(&original).is_err());
    }

    #[test]
    fn large_casefolded_entry_class_has_bounded_content_diagnostics() {
        let expected = ExpectedEvidence {
            scope: crate::domain::EvidenceScope::WholeAsset,
            sha1: Some([1; 20]),
            ..ExpectedEvidence::default()
        };
        let mut entries = vec![
            entry("rom.bin", ExpectedEvidence::default()),
            entry("rom.bin", expected.clone()),
            entry(
                "rom.bin",
                ExpectedEvidence {
                    sha1: Some([2; 20]),
                    ..expected.clone()
                },
            ),
        ];
        entries.extend((0..9_997).map(|_| entry("rom.bin", expected.clone())));
        let original = empty_plan(vec![group("set", entries.clone())]);
        let reversed = empty_plan(vec![group("set", entries.into_iter().rev().collect())]);

        let issues = inspect_plan(&original);
        assert_eq!(issues.len(), 3);
        for relation in [
            ContentRelation::SameEstablishedContent,
            ContentRelation::DifferentContent,
            ContentRelation::UnknownContent,
        ] {
            assert!(issues.iter().any(|issue| {
                issue.kind == PlanIssueKind::DuplicateEntry(relation)
                    && issue.path.as_str() == "rom.bin"
                    && issue
                        .conflicts_with
                        .as_ref()
                        .is_some_and(|path| path.as_str() == "rom.bin")
            }));
        }
        assert_eq!(inspect_plan(&reversed), issues);
        assert!(validate_plan(&original).is_err());
    }

    #[test]
    fn bounded_entry_diagnostics_find_relations_between_later_evidence_shapes() {
        let entries = vec![
            entry("rom.bin", ExpectedEvidence::default()),
            entry(
                "rom.bin",
                ExpectedEvidence {
                    scope: crate::domain::EvidenceScope::WholeAsset,
                    md5: Some(crate::domain::Md5Digest([0; 16])),
                    ..ExpectedEvidence::default()
                },
            ),
            entry(
                "rom.bin",
                ExpectedEvidence {
                    scope: crate::domain::EvidenceScope::WholeAsset,
                    crc: Some(crate::domain::Crc32Digest([0; 4])),
                    ..ExpectedEvidence::default()
                },
            ),
            entry(
                "rom.bin",
                ExpectedEvidence {
                    scope: crate::domain::EvidenceScope::WholeAsset,
                    crc: Some(crate::domain::Crc32Digest([0; 4])),
                    md5: Some(crate::domain::Md5Digest([1; 16])),
                    ..ExpectedEvidence::default()
                },
            ),
            entry(
                "rom.bin",
                ExpectedEvidence {
                    scope: crate::domain::EvidenceScope::WholeAsset,
                    md5: Some(crate::domain::Md5Digest([0; 16])),
                    ..ExpectedEvidence::default()
                },
            ),
        ];
        let plan = empty_plan(vec![group("set", entries)]);

        let issues = inspect_plan(&plan);
        assert_eq!(issues.len(), 3);
        for relation in [
            ContentRelation::SameEstablishedContent,
            ContentRelation::DifferentContent,
            ContentRelation::UnknownContent,
        ] {
            assert!(
                issues
                    .iter()
                    .any(|issue| { issue.kind == PlanIssueKind::DuplicateEntry(relation) })
            );
        }
        assert!(validate_plan(&plan).is_err());
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

            let target = source.join("child");
            std::fs::create_dir_all(&target)?;
            let parent_alias = temp.path().join("parent-alias");
            symlink(&target, &parent_alias)?;
            let traversal = Utf8PathBuf::try_from(parent_alias.join("../new"))
                .map_err(|_| std::io::Error::other("temporary path is not UTF-8"))?;
            assert!(ensure_sources_disjoint_from_destination(&[source], &traversal).is_err());
        }
        Ok(())
    }

    #[test]
    fn checked_plan_destination_allows_a_missing_source_inside_its_root() -> crate::Result<()> {
        let temp = tempfile::tempdir()?;
        let source_dir = temp.path().join("input");
        std::fs::create_dir_all(&source_dir)?;
        let source_root = Utf8Path::from_path(&source_dir)
            .ok_or_else(|| std::io::Error::other("temporary path is not UTF-8"))?;
        let missing_source = source_root.join("not-yet-present.rom");
        let destination_path = temp.path().join("output");
        let destination = Utf8Path::from_path(&destination_path)
            .ok_or_else(|| std::io::Error::other("temporary path is not UTF-8"))?;
        let mut source = entry("game.rom", ExpectedEvidence::default()).source;
        source.source_root = crate::domain::SourceRoot::new(source_root.as_str());
        source.location = crate::domain::SourceLocation::BareFile {
            path: missing_source.to_string(),
        };
        let mut planned_entry = entry("game.rom", ExpectedEvidence::default());
        planned_entry.source = source;
        let plan = empty_plan(vec![group("set", vec![planned_entry])]);

        let checked = checked_plan_destination(&plan, destination)?;
        assert_eq!(checked.path(), destination);
        assert_eq!(checked.sources(), &[source_root.to_path_buf()]);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn checked_plan_destination_rejects_a_dangling_source_symlink() -> crate::Result<()> {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir()?;
        let source_dir = temp.path().join("input");
        std::fs::create_dir_all(&source_dir)?;
        let source_root = Utf8Path::from_path(&source_dir)
            .ok_or_else(|| std::io::Error::other("temporary path is not UTF-8"))?;
        let first_source = source_root.join("first.rom");
        std::fs::write(&first_source, b"first")?;
        let destination_path = temp.path().join("output");
        let destination = Utf8Path::from_path(&destination_path)
            .ok_or_else(|| std::io::Error::other("temporary path is not UTF-8"))?;
        let dangling_source = source_root.join("generated.zip");
        symlink(destination.join("first.zip"), &dangling_source)?;

        let make_source = |path: &Utf8Path| {
            let mut source = entry("game.rom", ExpectedEvidence::default()).source;
            source.source_root = crate::domain::SourceRoot::new(source_root.as_str());
            source.location = crate::domain::SourceLocation::BareFile {
                path: path.to_string(),
            };
            source
        };
        let mut first = entry("first.rom", ExpectedEvidence::default());
        first.source = make_source(&first_source);
        let mut second = entry("second.rom", ExpectedEvidence::default());
        second.source = make_source(&dangling_source);
        let plan = empty_plan(vec![
            group("first", vec![first]),
            group("second", vec![second]),
        ]);

        assert!(checked_plan_destination(&plan, destination).is_err());
        Ok(())
    }
}
