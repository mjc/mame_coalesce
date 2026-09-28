use mame_coalesce::{
    domain::{
        AssetRole, CatalogKey, Crc32Digest, DatRom, EvidenceProvenance, EvidenceScope,
        ExpectedEvidence, Md5Digest, ObservedContent, RequirementKey, SetKey, SetMetadata,
        SourceFile, SourceLocation, SourcePhysicalPath, SourceRoot,
    },
    hashes::sha1_bytes,
    resolution::{MatchStrength, MatchingPolicy, MissingReason, ResolutionStatus, resolve},
};

fn requirement(expected: ExpectedEvidence) -> DatRom {
    DatRom {
        catalog_name: "confidence-fixture".to_owned(),
        key: RequirementKey::new(
            SetKey::new(CatalogKey::new("confidence-fixture"), "game"),
            "game.rom",
        ),
        parent_name: None,
        set_metadata: SetMetadata::default(),
        role: AssetRole::Rom,
        component_order: Some(0),
        expected,
    }
}

fn source(path: &str, observed: ObservedContent) -> SourceFile {
    let location = SourceLocation::BareFile {
        path: path.to_owned(),
    };
    SourceFile {
        source_root: SourceRoot::new("/roms"),
        physical_path: SourcePhysicalPath::from_location(&location),
        location,
        observed,
        fingerprint: None,
        scan_run: None,
        scan_provenance: None,
        bare_file_cache_stamp: None,
    }
}

fn observed(
    crc: Option<[u8; 4]>,
    md5: Option<[u8; 16]>,
    sha1: Option<[u8; 20]>,
) -> ObservedContent {
    ObservedContent {
        scope: EvidenceScope::WholeAsset,
        provenance: EvidenceProvenance::Computed,
        size: Some(3),
        crc: crc.map(Crc32Digest),
        md5: md5.map(Md5Digest),
        sha1,
        xxh3: [0; 8],
    }
}

#[test]
fn evidence_aware_confidence_falls_back_safely_and_keeps_legacy_default() {
    let expected_sha1 = sha1_bytes(b"catalog bytes");
    let expected = requirement(ExpectedEvidence {
        scope: EvidenceScope::WholeAsset,
        provenance: EvidenceProvenance::SourceDeclared,
        size: Some(3),
        crc: Some(Crc32Digest([0, 0, 0, 7])),
        md5: Some(Md5Digest([3; 16])),
        sha1: Some(expected_sha1),
        ..ExpectedEvidence::default()
    });
    let inventory = [
        source("/roms/md5-only.rom", observed(None, Some([3; 16]), None)),
        source(
            "/roms/conflicting-sha1.rom",
            observed(None, Some([3; 16]), Some(sha1_bytes(b"different bytes"))),
        ),
    ];
    let root = SourceRoot::new("/roms");

    let aware = resolve(
        std::slice::from_ref(&expected),
        &inventory,
        "confidence-fixture",
        &root,
        MatchingPolicy::EvidenceAware,
    );
    assert!(matches!(
        &aware[0].status,
        ResolutionStatus::Matched {
            selected,
            strength: MatchStrength::Md5,
            assessments,
            ..
        } if selected.location.path() == "/roms/md5-only.rom"
            && assessments.iter().any(|candidate| !candidate.conflicts.is_empty())
    ));

    assert_eq!(MatchingPolicy::default(), MatchingPolicy::Sha1Compatibility);
    let compatible = resolve(
        &[expected],
        &inventory,
        "confidence-fixture",
        &root,
        MatchingPolicy::default(),
    );
    assert!(matches!(
        compatible[0].status,
        ResolutionStatus::Missing {
            reason: MissingReason::NoMatchingSource,
            ..
        }
    ));
}

#[test]
fn stronger_sha1_wins_over_matching_md5() {
    let root = SourceRoot::new("/roms");
    let stronger_sha1 = sha1_bytes(b"stronger match");
    let ranked_requirement = requirement(ExpectedEvidence {
        scope: EvidenceScope::WholeAsset,
        size: Some(3),
        md5: Some(Md5Digest([4; 16])),
        sha1: Some(stronger_sha1),
        ..ExpectedEvidence::default()
    });
    let ranked_inventory = [
        source("/roms/md5.rom", observed(None, Some([4; 16]), None)),
        source(
            "/roms/sha1.rom",
            observed(None, Some([5; 16]), Some(stronger_sha1)),
        ),
    ];
    let ranked = resolve(
        &[ranked_requirement],
        &ranked_inventory,
        "confidence-fixture",
        &root,
        MatchingPolicy::EvidenceAware,
    );
    assert!(matches!(
        &ranked[0].status,
        ResolutionStatus::Matched {
            selected,
            strength: MatchStrength::Sha1,
            ..
        } if selected.location.path() == "/roms/sha1.rom"
    ));
}

#[test]
fn crc_plus_size_is_weak_and_unresolved_collisions_are_ambiguous() {
    let root = SourceRoot::new("/roms");
    let crc_only = requirement(ExpectedEvidence {
        scope: EvidenceScope::WholeAsset,
        size: Some(3),
        crc: Some(Crc32Digest([0, 0, 0, 7])),
        ..ExpectedEvidence::default()
    });
    let collisions = [
        source("/roms/a.rom", observed(Some([0, 0, 0, 7]), None, None)),
        source("/roms/b.rom", observed(Some([0, 0, 0, 7]), None, None)),
    ];
    let unique = resolve(
        std::slice::from_ref(&crc_only),
        &[source(
            "/roms/unique.rom",
            observed(Some([0, 0, 0, 7]), None, None),
        )],
        "confidence-fixture",
        &root,
        MatchingPolicy::EvidenceAware,
    );
    assert!(matches!(
        &unique[0].status,
        ResolutionStatus::Matched {
            strength: MatchStrength::CrcAndSize,
            ..
        }
    ));

    let weak = resolve(
        &[crc_only],
        &collisions,
        "confidence-fixture",
        &root,
        MatchingPolicy::EvidenceAware,
    );
    assert!(matches!(
        &weak[0].status,
        ResolutionStatus::Ambiguous { candidates, .. } if candidates.len() == 2
    ));
}
