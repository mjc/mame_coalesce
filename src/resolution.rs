//! Pure, deterministic matching of catalog requirements to source observations.

use camino::Utf8Path;
use serde::{Deserialize, Serialize};

use crate::domain::{DatRom, EvidenceProvenance, EvidenceScope, SourceFile, SourceRoot};

pub use crate::domain::MatchingPolicy;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchStrength {
    Sha1,
    Md5,
    CrcAndSize,
}

impl MatchStrength {
    const fn rank(self) -> u8 {
        match self {
            Self::Sha1 => 3,
            Self::Md5 => 2,
            Self::CrcAndSize => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceField {
    Sha1,
    Md5,
    Crc,
    Size,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissingReason {
    NoExpectedContentEvidence,
    UnsupportedExpectedScope,
    NoComparableObservedEvidence,
    InsufficientEvidence,
    NoMatchingSource,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceAssessment {
    pub source: SourceFile,
    pub strength: Option<MatchStrength>,
    pub agreements: Vec<EvidenceField>,
    pub conflicts: Vec<EvidenceField>,
    pub comparable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolutionStatus {
    Matched {
        selected: Box<SourceFile>,
        strength: MatchStrength,
        equivalent_copies: Vec<SourceFile>,
        assessments: Vec<SourceAssessment>,
    },
    AmbiguousWeak {
        candidates: Vec<SourceAssessment>,
    },
    Conflicting {
        candidates: Vec<SourceAssessment>,
    },
    Missing {
        reason: MissingReason,
        assessments: Vec<SourceAssessment>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequirementResolution {
    pub requirement: DatRom,
    pub status: ResolutionStatus,
}

/// Resolve one catalog's requirements against one already-selected source root.
///
/// The result owns the requirement, candidates and selected source so callers may retain it
/// independently of database rows or scanner buffers. No presentation or exit policy is applied.
#[must_use]
pub fn resolve(
    dat_roms: &[DatRom],
    source_files: &[SourceFile],
    catalog_name: &str,
    source_root: &SourceRoot,
    policy: MatchingPolicy,
) -> Vec<RequirementResolution> {
    let mut requirements = dat_roms
        .iter()
        .filter(|rom| rom.catalog_name() == catalog_name)
        .collect::<Vec<_>>();
    requirements.sort();

    let mut sources = source_files
        .iter()
        .filter(|source| source_in_root(source, source_root))
        .collect::<Vec<_>>();
    sources.sort_by(|left, right| source_order(left, right));

    requirements
        .into_iter()
        .map(|requirement| RequirementResolution {
            requirement: requirement.clone(),
            status: resolve_one(requirement, &sources, policy),
        })
        .collect()
}

fn resolve_one(
    requirement: &DatRom,
    sources: &[&SourceFile],
    policy: MatchingPolicy,
) -> ResolutionStatus {
    if policy == MatchingPolicy::Sha1Compatibility {
        return resolve_sha1_compatibility(requirement, sources);
    }

    let expected = &requirement.expected;
    if expected.scope != EvidenceScope::WholeAsset {
        return ResolutionStatus::Missing {
            reason: MissingReason::UnsupportedExpectedScope,
            assessments: Vec::new(),
        };
    }
    let has_expected_digest =
        expected.sha1.is_some() || expected.md5.is_some() || expected.crc.is_some();
    if !has_expected_digest {
        return ResolutionStatus::Missing {
            reason: MissingReason::NoExpectedContentEvidence,
            assessments: Vec::new(),
        };
    }

    let assessments = sources
        .iter()
        .map(|source| assess_source(expected, source))
        .collect::<Vec<_>>();
    let mut matches = assessments
        .iter()
        .filter_map(|candidate| candidate.strength.map(|strength| (candidate, strength)))
        .collect::<Vec<_>>();
    matches.sort_by(|(left, left_strength), (right, right_strength)| {
        right_strength
            .rank()
            .cmp(&left_strength.rank())
            .then_with(|| source_order(&left.source, &right.source))
    });

    let Some((selected, strength)) = matches.first().copied() else {
        if assessments
            .iter()
            .any(|candidate| !candidate.conflicts.is_empty())
        {
            return ResolutionStatus::Conflicting {
                candidates: assessments,
            };
        }
        let reason = if assessments.iter().any(|candidate| candidate.comparable) {
            if assessments
                .iter()
                .any(|candidate| !candidate.agreements.is_empty())
            {
                MissingReason::InsufficientEvidence
            } else {
                MissingReason::NoMatchingSource
            }
        } else if assessments.is_empty() {
            MissingReason::NoMatchingSource
        } else {
            MissingReason::NoComparableObservedEvidence
        };
        return ResolutionStatus::Missing {
            reason,
            assessments,
        };
    };

    let strongest = matches
        .iter()
        .take_while(|(_, candidate_strength)| candidate_strength.rank() == strength.rank())
        .map(|(candidate, _)| *candidate)
        .collect::<Vec<_>>();

    if strength == MatchStrength::CrcAndSize && strongest.len() > 1 {
        return ResolutionStatus::AmbiguousWeak {
            candidates: strongest.into_iter().cloned().collect(),
        };
    }

    ResolutionStatus::Matched {
        selected: Box::new(selected.source.clone()),
        strength,
        equivalent_copies: strongest
            .into_iter()
            .map(|candidate| candidate.source.clone())
            .collect(),
        assessments,
    }
}

fn resolve_sha1_compatibility(requirement: &DatRom, sources: &[&SourceFile]) -> ResolutionStatus {
    let Some(expected_sha1) = requirement.sha1() else {
        return ResolutionStatus::Missing {
            reason: MissingReason::NoExpectedContentEvidence,
            assessments: Vec::new(),
        };
    };
    let copies = sources
        .iter()
        .filter(|source| source.observed.sha1.as_ref() == Some(expected_sha1))
        .map(|source| (*source).clone())
        .collect::<Vec<_>>();
    let Some(selected) = copies.first() else {
        return ResolutionStatus::Missing {
            reason: MissingReason::NoMatchingSource,
            assessments: Vec::new(),
        };
    };
    ResolutionStatus::Matched {
        selected: Box::new(selected.clone()),
        strength: MatchStrength::Sha1,
        equivalent_copies: copies,
        assessments: Vec::new(),
    }
}

fn assess_source(
    expected: &crate::domain::ExpectedEvidence,
    source: &SourceFile,
) -> SourceAssessment {
    let observed = &source.observed;
    let supported = observed.scope == EvidenceScope::WholeAsset
        && matches!(
            observed.provenance,
            EvidenceProvenance::Computed | EvidenceProvenance::SourceDeclared
        );
    if !supported {
        return SourceAssessment {
            source: source.clone(),
            strength: None,
            agreements: Vec::new(),
            conflicts: Vec::new(),
            comparable: false,
        };
    }

    let mut conflicts = Vec::new();
    let sha1_equal = compare(
        expected.sha1,
        observed.sha1,
        EvidenceField::Sha1,
        &mut conflicts,
    );
    let md5_equal = compare(
        expected.md5,
        observed.md5,
        EvidenceField::Md5,
        &mut conflicts,
    );
    let crc_equal = compare(
        expected.crc,
        observed.crc,
        EvidenceField::Crc,
        &mut conflicts,
    );
    let size_equal = compare(
        expected.size,
        observed.size,
        EvidenceField::Size,
        &mut conflicts,
    );
    let comparable = !conflicts.is_empty()
        || expected.sha1.is_some() && observed.sha1.is_some()
        || expected.md5.is_some() && observed.md5.is_some()
        || expected.crc.is_some() && observed.crc.is_some()
        || expected.size.is_some() && observed.size.is_some();
    let mut agreements = Vec::new();
    if sha1_equal {
        agreements.push(EvidenceField::Sha1);
    }
    if md5_equal {
        agreements.push(EvidenceField::Md5);
    }
    if crc_equal {
        agreements.push(EvidenceField::Crc);
    }
    if size_equal {
        agreements.push(EvidenceField::Size);
    }

    let strength = if sha1_equal {
        Some(MatchStrength::Sha1)
    } else if md5_equal {
        Some(MatchStrength::Md5)
    } else if crc_equal && size_equal {
        Some(MatchStrength::CrcAndSize)
    } else {
        None
    };

    // A contradictory stronger digest vetoes fallback to weaker evidence. Once stronger
    // evidence matches, retain lower-priority contradictions as explanation, not vetoes.
    let strength = match strength {
        Some(MatchStrength::Sha1) => Some(MatchStrength::Sha1),
        Some(MatchStrength::Md5) if !conflicts.contains(&EvidenceField::Sha1) => {
            Some(MatchStrength::Md5)
        }
        Some(MatchStrength::CrcAndSize)
            if !conflicts.contains(&EvidenceField::Sha1)
                && !conflicts.contains(&EvidenceField::Md5) =>
        {
            Some(MatchStrength::CrcAndSize)
        }
        Some(_) | None => None,
    };

    SourceAssessment {
        source: source.clone(),
        strength,
        agreements,
        conflicts,
        comparable,
    }
}

fn compare<T: PartialEq>(
    expected: Option<T>,
    observed: Option<T>,
    field: EvidenceField,
    conflicts: &mut Vec<EvidenceField>,
) -> bool {
    match (expected, observed) {
        (Some(expected), Some(observed)) if expected == observed => true,
        (Some(_), Some(_)) => {
            conflicts.push(field);
            false
        }
        _ => false,
    }
}

fn source_order(left: &SourceFile, right: &SourceFile) -> std::cmp::Ordering {
    left.location
        .priority()
        .cmp(&right.location.priority())
        .then_with(|| left.location.path().cmp(right.location.path()))
        .then_with(|| {
            left.location
                .member_name()
                .cmp(&right.location.member_name())
        })
        .then_with(|| left.location.cmp(&right.location))
        .then_with(|| left.source_root.cmp(&right.source_root))
        .then_with(|| left.observed.cmp(&right.observed))
        .then_with(|| left.fingerprint.cmp(&right.fingerprint))
        .then_with(|| left.scan_run.cmp(&right.scan_run))
        .then_with(|| left.scan_provenance.cmp(&right.scan_provenance))
}

fn source_in_root(source: &SourceFile, source_root: &SourceRoot) -> bool {
    let source_root = Utf8Path::new(source_root.as_str());
    Utf8Path::new(source.location.path()).starts_with(source_root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        AssetRole, CatalogKey, ExpectedEvidence, ObservedContent, RequirementKey, SetKey,
        SetMetadata, SourceLocation,
    };
    use crate::hashes::Sha1Digest;

    fn requirement(mut expected: ExpectedEvidence) -> DatRom {
        if expected.scope == EvidenceScope::Unknown {
            expected.scope = EvidenceScope::WholeAsset;
        }
        DatRom {
            catalog_name: "catalog".to_owned(),
            key: RequirementKey::new(SetKey::new(CatalogKey::fresh(), "set"), "game.rom"),
            parent_name: None,
            set_metadata: SetMetadata::default(),
            role: AssetRole::Rom,
            component_order: Some(0),
            expected,
        }
    }

    fn source(path: &str, observed: ObservedContent) -> SourceFile {
        SourceFile {
            source_root: SourceRoot::new("/roms"),
            location: SourceLocation::BareFile {
                path: path.to_owned(),
            },
            observed,
            fingerprint: None,
            scan_run: None,
            scan_provenance: None,
        }
    }

    fn computed(
        size: Option<u64>,
        crc: Option<[u8; 4]>,
        md5: Option<[u8; 16]>,
        sha1: Option<Sha1Digest>,
    ) -> ObservedContent {
        ObservedContent {
            scope: EvidenceScope::WholeAsset,
            provenance: EvidenceProvenance::Computed,
            size,
            crc: crc.map(crate::domain::Crc32Digest),
            md5: md5.map(crate::domain::Md5Digest),
            sha1,
            xxh3: [0; 8],
        }
    }

    fn result(status: &ResolutionStatus) -> &ResolutionStatus {
        status
    }

    #[test]
    fn compatibility_policy_keeps_sha1_authoritative_over_conflicting_metadata() {
        let rom = requirement(ExpectedEvidence {
            size: Some(99),
            crc: Some(crate::domain::Crc32Digest([0, 0, 0, 7])),
            sha1: Some(crate::hashes::sha1_bytes(b"rom")),
            ..ExpectedEvidence::default()
        });
        let inventory = [source(
            "/roms/a.rom",
            computed(
                Some(1),
                Some([0, 0, 0, 9]),
                None,
                Some(crate::hashes::sha1_bytes(b"rom")),
            ),
        )];

        let resolutions = resolve(
            &[rom],
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::Sha1Compatibility,
        );

        assert!(matches!(
            result(&resolutions[0].status),
            ResolutionStatus::Matched {
                strength: MatchStrength::Sha1,
                ..
            }
        ));
    }

    #[test]
    fn evidence_aware_policy_uses_md5_only_when_observed_sha1_does_not_contradict() {
        let md5 = [3; 16];
        let rom = requirement(ExpectedEvidence {
            md5: Some(crate::domain::Md5Digest(md5)),
            sha1: Some(crate::hashes::sha1_bytes(b"expected")),
            ..ExpectedEvidence::default()
        });
        let inventory = [
            source(
                "/roms/a.rom",
                computed(
                    None,
                    None,
                    Some(md5),
                    Some(crate::hashes::sha1_bytes(b"wrong")),
                ),
            ),
            source("/roms/b.rom", computed(None, None, Some(md5), None)),
        ];

        let resolutions = resolve(
            &[rom],
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::EvidenceAware,
        );

        assert!(matches!(
            result(&resolutions[0].status),
            ResolutionStatus::Matched {
                selected,
                strength: MatchStrength::Md5,
                ..
            } if selected.location.path() == "/roms/b.rom"
        ));
    }

    #[test]
    fn matching_stronger_evidence_retains_lower_priority_conflict_for_explanation() {
        let sha1 = crate::hashes::sha1_bytes(b"same content");
        let rom = requirement(ExpectedEvidence {
            md5: Some(crate::domain::Md5Digest([1; 16])),
            sha1: Some(sha1),
            ..ExpectedEvidence::default()
        });
        let inventory = [source(
            "/roms/a.rom",
            computed(None, None, Some([2; 16]), Some(sha1)),
        )];

        let resolutions = resolve(
            &[rom],
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::EvidenceAware,
        );

        assert!(matches!(
            &resolutions[0].status,
            ResolutionStatus::Matched {
                strength: MatchStrength::Sha1,
                assessments,
                ..
            } if assessments[0].conflicts == [EvidenceField::Md5]
        ));
    }

    #[test]
    fn duplicate_strong_matches_are_equivalent_copies_with_stable_selection() {
        let sha1 = crate::hashes::sha1_bytes(b"same content");
        let rom = requirement(ExpectedEvidence {
            sha1: Some(sha1),
            ..ExpectedEvidence::default()
        });
        let inventory = [
            source("/roms/z.rom", computed(None, None, None, Some(sha1))),
            source("/roms/a.rom", computed(None, None, None, Some(sha1))),
        ];

        let resolutions = resolve(
            &[rom],
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::EvidenceAware,
        );

        assert!(matches!(
            &resolutions[0].status,
            ResolutionStatus::Matched {
                selected,
                equivalent_copies,
                ..
            } if selected.location.path() == "/roms/a.rom" && equivalent_copies.len() == 2
        ));
    }

    #[test]
    fn weak_crc_and_size_matches_are_ambiguous_not_content_identity() {
        let rom = requirement(ExpectedEvidence {
            size: Some(3),
            crc: Some(crate::domain::Crc32Digest([0, 0, 0, 7])),
            ..ExpectedEvidence::default()
        });
        let inventory = [
            source(
                "/roms/a.rom",
                computed(Some(3), Some([0, 0, 0, 7]), None, None),
            ),
            source(
                "/roms/b.rom",
                computed(Some(3), Some([0, 0, 0, 7]), None, None),
            ),
        ];

        let resolutions = resolve(
            &[rom],
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::EvidenceAware,
        );

        assert!(matches!(
            &resolutions[0].status,
            ResolutionStatus::AmbiguousWeak { candidates } if candidates.len() == 2
        ));
    }

    #[test]
    fn crc_without_size_is_insufficient_evidence_to_select_a_source() {
        let rom = requirement(ExpectedEvidence {
            crc: Some(crate::domain::Crc32Digest([0, 0, 0, 7])),
            ..ExpectedEvidence::default()
        });
        let inventory = [source(
            "/roms/a.rom",
            computed(Some(3), Some([0, 0, 0, 7]), None, None),
        )];

        let resolutions = resolve(
            &[rom],
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::EvidenceAware,
        );

        assert!(matches!(
            &resolutions[0].status,
            ResolutionStatus::Missing {
                reason: MissingReason::InsufficientEvidence,
                ..
            }
        ));
    }

    #[test]
    fn no_expected_or_observed_evidence_is_reported_without_guessing() {
        let empty_requirement = requirement(ExpectedEvidence::default());
        let hash_only = requirement(ExpectedEvidence {
            sha1: Some(crate::hashes::sha1_bytes(b"rom")),
            ..ExpectedEvidence::default()
        });
        let inventory = [source("/roms/a.rom", computed(None, None, None, None))];

        let empty_result = resolve(
            &[empty_requirement],
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::EvidenceAware,
        );
        let hash_result = resolve(
            &[hash_only],
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::EvidenceAware,
        );

        assert!(matches!(
            empty_result[0].status,
            ResolutionStatus::Missing {
                reason: MissingReason::NoExpectedContentEvidence,
                ..
            }
        ));
        assert!(matches!(
            hash_result[0].status,
            ResolutionStatus::Missing {
                reason: MissingReason::NoComparableObservedEvidence,
                ..
            }
        ));

        let mut legacy_observation =
            computed(None, None, None, Some(crate::hashes::sha1_bytes(b"rom")));
        legacy_observation.provenance = EvidenceProvenance::Unknown;
        let legacy_result = resolve(
            &[requirement(ExpectedEvidence {
                sha1: Some(crate::hashes::sha1_bytes(b"rom")),
                ..ExpectedEvidence::default()
            })],
            &[source("/roms/legacy.rom", legacy_observation)],
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::EvidenceAware,
        );
        assert!(matches!(
            legacy_result[0].status,
            ResolutionStatus::Missing {
                reason: MissingReason::NoComparableObservedEvidence,
                ..
            }
        ));
    }

    #[test]
    fn evidence_aware_policy_rejects_unsupported_expected_scope() {
        let mut disk_requirement = requirement(ExpectedEvidence {
            sha1: Some(crate::hashes::sha1_bytes(b"rom")),
            ..ExpectedEvidence::default()
        });
        disk_requirement.expected.scope = EvidenceScope::DiskData;
        let inventory = [source(
            "/roms/disk.chd",
            computed(None, None, None, Some(crate::hashes::sha1_bytes(b"rom"))),
        )];

        let resolutions = resolve(
            &[disk_requirement],
            &inventory,
            "catalog",
            &SourceRoot::new("/roms"),
            MatchingPolicy::EvidenceAware,
        );

        assert!(matches!(
            resolutions[0].status,
            ResolutionStatus::Missing {
                reason: MissingReason::UnsupportedExpectedScope,
                ..
            }
        ));
    }

    #[test]
    fn resolver_scopes_catalog_and_root_and_orders_candidates() {
        let sha1 = crate::hashes::sha1_bytes(b"rom");
        let mut rom = requirement(ExpectedEvidence {
            sha1: Some(sha1),
            ..ExpectedEvidence::default()
        });
        rom.catalog_name = "catalog-a".to_owned();
        let other_catalog = requirement(rom.expected.clone());
        let inventory = [
            source("/roms-other/a.rom", computed(None, None, None, Some(sha1))),
            source("/roms/z.rom", computed(None, None, None, Some(sha1))),
            source("/roms/a.rom", computed(None, None, None, Some(sha1))),
        ];

        let resolutions = resolve(
            &[other_catalog.clone(), rom.clone()],
            &inventory,
            "catalog-a",
            &SourceRoot::new("/roms"),
            MatchingPolicy::Sha1Compatibility,
        );

        assert_eq!(resolutions.len(), 1);
        assert!(matches!(
            &resolutions[0].status,
            ResolutionStatus::Matched { selected, .. }
                if selected.location.path() == "/roms/a.rom"
        ));
        let reordered = resolve(
            &[rom, other_catalog],
            &inventory.iter().rev().cloned().collect::<Vec<_>>(),
            "catalog-a",
            &SourceRoot::new("/roms"),
            MatchingPolicy::Sha1Compatibility,
        );
        assert_eq!(resolutions, reordered);
    }
}
