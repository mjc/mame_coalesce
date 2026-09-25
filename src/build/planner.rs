use std::collections::BTreeMap;

use crate::{
    domain::{
        BuildMode, BuildPlan, BuildReport, BuildRequest, DatRom, DuplicateMatch, MissingRom,
        SourceFile, ZipEntrySpec, ZipSpec,
    },
    resolution::{self, RequirementResolution, ResolutionStatus},
};

#[must_use]
pub fn plan_build(
    dat_roms: &[DatRom],
    source_files: &[SourceFile],
    request: &BuildRequest,
) -> BuildPlan {
    let resolutions = resolution::resolve(
        dat_roms,
        source_files,
        &request.dat_name,
        &request.source_root,
        request.matching_policy,
    );
    let report = build_report(&resolutions, request.strict);

    if request.strict && !report.missing_roms.is_empty() {
        return BuildPlan {
            zips: Vec::new(),
            report,
            dry_run: request.dry_run,
        };
    }

    BuildPlan {
        zips: plan_zip_entries(&resolutions, request.mode)
            .into_iter()
            .map(|(file_name, entries)| ZipSpec { file_name, entries })
            .collect(),
        report,
        dry_run: request.dry_run,
    }
}

fn plan_zip_entries(
    resolutions: &[RequirementResolution],
    mode: BuildMode,
) -> BTreeMap<String, Vec<ZipEntrySpec>> {
    resolutions
        .iter()
        .filter_map(|resolution| match &resolution.status {
            ResolutionStatus::Matched { selected, .. } => Some((&resolution.requirement, selected)),
            ResolutionStatus::AmbiguousWeak { .. }
            | ResolutionStatus::Conflicting { .. }
            | ResolutionStatus::Missing { .. } => None,
        })
        .fold(BTreeMap::new(), |mut entries_by_zip, (rom, selected)| {
            entries_by_zip
                .entry(format!("{}.zip", rom.bundle_name(mode)))
                .or_default()
                .push(ZipEntrySpec {
                    output_name: rom.rom_name().to_owned(),
                    source: selected.as_ref().clone(),
                });
            entries_by_zip
        })
}

fn build_report(resolutions: &[RequirementResolution], strict: bool) -> BuildReport {
    let mut report = resolutions
        .iter()
        .fold(BuildReport::default(), |mut report, resolution| {
            match &resolution.status {
                ResolutionStatus::Matched {
                    selected,
                    equivalent_copies,
                    ..
                } => {
                    report.matched_roms += 1;
                    if equivalent_copies.len() > 1 {
                        report.duplicate_matches.push(DuplicateMatch {
                            rom_name: resolution.requirement.rom_name().to_owned(),
                            selected: selected.as_ref().clone(),
                            candidates: equivalent_copies.clone(),
                        });
                    }
                }
                ResolutionStatus::AmbiguousWeak { .. }
                | ResolutionStatus::Conflicting { .. }
                | ResolutionStatus::Missing { .. } => {
                    report.missing_roms.push(MissingRom {
                        game_name: resolution.requirement.game_name().to_owned(),
                        rom_name: resolution.requirement.rom_name().to_owned(),
                        sha1: resolution.requirement.sha1().copied(),
                    });
                }
            }
            report
        });

    if strict && !report.missing_roms.is_empty() {
        report.exit_code = 2;
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::BuildMode;
    use crate::domain::{
        ArchiveBackend, ArchiveMemberSelector, CatalogKey, ExpectedEvidence, MatchingPolicy,
        ObservedContent, RequirementKey, SetKey, SourceLocation,
    };
    use proptest::prelude::*;

    #[derive(Clone, Copy)]
    enum SourceKind {
        BareFile,
        ZipEntry,
        ArchiveEntry,
    }

    fn digest(value: &str) -> crate::hashes::Sha1Digest {
        crate::hashes::sha1_bytes(value.as_bytes())
    }

    fn request(mode: BuildMode) -> BuildRequest {
        BuildRequest {
            dat_name: "dat-a".to_owned(),
            source_root: crate::domain::SourceRoot::new("/src-a"),
            mode,
            matching_policy: MatchingPolicy::Sha1Compatibility,
            dry_run: false,
            strict: false,
        }
    }

    fn rom(game_name: &str, parent_name: Option<&str>, rom_name: &str, sha1: &str) -> DatRom {
        DatRom {
            catalog_name: "dat-a".to_owned(),
            key: RequirementKey::new(SetKey::new(CatalogKey::fresh(), game_name), rom_name),
            parent_name: parent_name.map(str::to_owned),
            set_metadata: crate::domain::SetMetadata::default(),
            role: crate::domain::AssetRole::Rom,
            component_order: Some(0),
            expected: ExpectedEvidence {
                sha1: Some(digest(sha1)),
                ..ExpectedEvidence::default()
            },
        }
    }

    fn source(
        root: &str,
        path: &str,
        entry_name: Option<&str>,
        sha1: &str,
        kind: SourceKind,
    ) -> SourceFile {
        let location = match kind {
            SourceKind::BareFile => SourceLocation::BareFile {
                path: path.to_owned(),
            },
            SourceKind::ZipEntry => SourceLocation::ArchiveMember {
                path: path.to_owned(),
                backend: ArchiveBackend::Zip,
                selector: ArchiveMemberSelector::IndexAndName {
                    index: 0,
                    name: entry_name.unwrap_or_default().to_owned(),
                },
            },
            SourceKind::ArchiveEntry => SourceLocation::ArchiveMember {
                path: path.to_owned(),
                backend: ArchiveBackend::SevenZip,
                selector: ArchiveMemberSelector::IndexAndName {
                    index: 0,
                    name: entry_name.unwrap_or_default().to_owned(),
                },
            },
        };
        SourceFile {
            source_root: crate::domain::SourceRoot::new(root),
            location,
            observed: ObservedContent {
                scope: crate::domain::EvidenceScope::WholeAsset,
                provenance: crate::domain::EvidenceProvenance::Computed,
                size: None,
                crc: None,
                md5: None,
                sha1: Some(digest(sha1)),
                xxh3: [0; 8],
            },
            fingerprint: None,
            scan_run: None,
            scan_provenance: None,
        }
    }

    #[test]
    fn missing_roms_are_reported_without_failing_by_default() {
        let dat_roms = [
            rom("parent", None, "present.rom", "sha1-present"),
            rom("parent", None, "missing.rom", "sha1-missing"),
        ];
        let source_files = [source(
            "/src-a",
            "/src-a/present.rom",
            None,
            "sha1-present",
            SourceKind::BareFile,
        )];

        let plan = plan_build(&dat_roms, &source_files, &request(BuildMode::ParentBundles));

        assert_eq!(plan.report.exit_code, 0);
        assert_eq!(plan.report.matched_roms, 1);
        assert_eq!(plan.report.missing_roms.len(), 1);
        assert_eq!(plan.report.missing_roms[0].rom_name, "missing.rom");
        assert!(plan.writes_files());
    }

    #[test]
    fn no_selected_dat_rows_produces_empty_successful_plan() {
        let dat_roms = [DatRom {
            catalog_name: "other-dat".to_owned(),
            key: RequirementKey::new(SetKey::new(CatalogKey::fresh(), "game"), "game.rom"),
            parent_name: None,
            set_metadata: crate::domain::SetMetadata::default(),
            role: crate::domain::AssetRole::Rom,
            component_order: Some(0),
            expected: ExpectedEvidence {
                sha1: Some(digest("sha1")),
                ..ExpectedEvidence::default()
            },
        }];

        let plan = plan_build(&dat_roms, &[], &request(BuildMode::ParentBundles));

        assert_eq!(plan.report.exit_code, 0);
        assert_eq!(plan.report.matched_roms, 0);
        assert!(plan.report.missing_roms.is_empty());
        assert!(plan.zips.is_empty());
        assert!(!plan.writes_files());
    }

    #[test]
    fn dry_run_plans_never_report_writes_files() {
        let dat_roms = [rom("parent", None, "present.rom", "sha1-present")];
        let source_files = [source(
            "/src-a",
            "/src-a/present.rom",
            None,
            "sha1-present",
            SourceKind::BareFile,
        )];
        let mut build_request = request(BuildMode::ParentBundles);
        build_request.dry_run = true;

        let plan = plan_build(&dat_roms, &source_files, &build_request);

        assert_eq!(plan.report.exit_code, 0);
        assert_eq!(plan.zips.len(), 1);
        assert!(!plan.writes_files());
    }

    #[test]
    fn strict_missing_roms_write_nothing_and_exit_two() {
        let dat_roms = [rom("parent", None, "missing.rom", "sha1-missing")];
        let mut build_request = request(BuildMode::ParentBundles);
        build_request.strict = true;

        let plan = plan_build(&dat_roms, &[], &build_request);

        assert_eq!(plan.report.exit_code, 2);
        assert_eq!(plan.report.missing_roms.len(), 1);
        assert!(plan.zips.is_empty());
        assert!(!plan.writes_files());
    }

    #[test]
    fn duplicate_matches_prefer_bare_file_then_canonical_path() {
        let dat_roms = [rom("parent", None, "dup.rom", "sha1-dup")];
        let source_files = [
            source(
                "/src-a",
                "/src-a/z.zip",
                Some("dup.rom"),
                "sha1-dup",
                SourceKind::ZipEntry,
            ),
            source(
                "/src-a",
                "/src-a/bare-b.rom",
                None,
                "sha1-dup",
                SourceKind::BareFile,
            ),
            source(
                "/src-a",
                "/src-a/bare-a.rom",
                None,
                "sha1-dup",
                SourceKind::BareFile,
            ),
        ];

        let plan = plan_build(&dat_roms, &source_files, &request(BuildMode::ParentBundles));

        assert_eq!(plan.report.duplicate_matches.len(), 1);
        assert_eq!(
            plan.report.duplicate_matches[0].selected.location.path(),
            "/src-a/bare-a.rom"
        );
        assert_eq!(
            plan.zips[0].entries[0].source.location.path(),
            "/src-a/bare-a.rom"
        );
    }

    #[test]
    fn duplicate_matches_prefer_source_kind_path_and_entry_name() {
        let dat_roms = [rom("parent", None, "dup.rom", "sha1-dup")];
        let source_files = [
            source(
                "/src-a",
                "/src-a/c.7z",
                Some("dup.rom"),
                "sha1-dup",
                SourceKind::ArchiveEntry,
            ),
            source(
                "/src-a",
                "/src-a/a.zip",
                Some("z.rom"),
                "sha1-dup",
                SourceKind::ZipEntry,
            ),
            source(
                "/src-a",
                "/src-a/a.zip",
                Some("a.rom"),
                "sha1-dup",
                SourceKind::ZipEntry,
            ),
        ];

        let plan = plan_build(&dat_roms, &source_files, &request(BuildMode::ParentBundles));

        let duplicate = &plan.report.duplicate_matches[0];
        assert_eq!(duplicate.selected.location.path(), "/src-a/a.zip");
        assert_eq!(duplicate.selected.location.member_name(), Some("a.rom"));
        assert_eq!(
            duplicate
                .candidates
                .iter()
                .map(SourceFile::display_name)
                .collect::<Vec<_>>(),
            vec![
                "/src-a/a.zip:a.rom",
                "/src-a/a.zip:z.rom",
                "/src-a/c.7z:dup.rom",
            ]
        );
    }

    #[test]
    fn duplicate_matches_use_full_source_priority_order() {
        let dat_roms = [rom("parent", None, "dup.rom", "sha1-dup")];
        let source_files = [
            source(
                "/src-a",
                "/src-a/c.7z",
                Some("dup.rom"),
                "sha1-dup",
                SourceKind::ArchiveEntry,
            ),
            source(
                "/src-a",
                "/src-a/b.zip",
                Some("dup.rom"),
                "sha1-dup",
                SourceKind::ZipEntry,
            ),
            source(
                "/src-a",
                "/src-a/a.zip",
                Some("z.rom"),
                "sha1-dup",
                SourceKind::ZipEntry,
            ),
            source(
                "/src-a",
                "/src-a/a.zip",
                Some("a.rom"),
                "sha1-dup",
                SourceKind::ZipEntry,
            ),
            source(
                "/src-a",
                "/src-a/z.rom",
                None,
                "sha1-dup",
                SourceKind::BareFile,
            ),
            source(
                "/src-a",
                "/src-a/a.rom",
                None,
                "sha1-dup",
                SourceKind::BareFile,
            ),
        ];

        let plan = plan_build(&dat_roms, &source_files, &request(BuildMode::ParentBundles));
        let reordered_plan = plan_build(
            &dat_roms,
            &source_files.iter().rev().cloned().collect::<Vec<_>>(),
            &request(BuildMode::ParentBundles),
        );

        let duplicate = &plan.report.duplicate_matches[0];
        assert_eq!(plan, reordered_plan);
        assert_eq!(duplicate.selected.display_name(), "/src-a/a.rom");
        assert_eq!(
            duplicate
                .candidates
                .iter()
                .map(SourceFile::display_name)
                .collect::<Vec<_>>(),
            vec![
                "/src-a/a.rom",
                "/src-a/z.rom",
                "/src-a/a.zip:a.rom",
                "/src-a/a.zip:z.rom",
                "/src-a/b.zip:dup.rom",
                "/src-a/c.7z:dup.rom",
            ]
        );
    }

    #[test]
    fn build_planning_scopes_matches_to_selected_source() {
        let dat_roms = [rom("parent", None, "scoped.rom", "sha1-scoped")];
        let source_files = [source(
            "/src-b",
            "/src-b/scoped.rom",
            None,
            "sha1-scoped",
            SourceKind::BareFile,
        )];

        let plan = plan_build(&dat_roms, &source_files, &request(BuildMode::ParentBundles));

        assert_eq!(plan.report.matched_roms, 0);
        assert_eq!(plan.report.missing_roms.len(), 1);
        assert!(plan.zips.is_empty());
    }

    #[test]
    fn source_root_matching_respects_path_boundaries() {
        let dat_roms = [rom("parent", None, "prefix.rom", "sha1-prefix")];
        let source_files = [source(
            "/src-a-other",
            "/src-a-other/prefix.rom",
            None,
            "sha1-prefix",
            SourceKind::BareFile,
        )];

        let plan = plan_build(&dat_roms, &source_files, &request(BuildMode::ParentBundles));

        assert_eq!(plan.report.matched_roms, 0);
        assert_eq!(plan.report.missing_roms.len(), 1);
        assert!(plan.zips.is_empty());
    }

    #[test]
    fn source_root_matching_accepts_descendant_canonical_paths() {
        let dat_roms = [rom("parent", None, "nested.rom", "sha1-nested")];
        let source_files = [source(
            "/other-root",
            "/src-a/nested/nested.rom",
            None,
            "sha1-nested",
            SourceKind::BareFile,
        )];

        let plan = plan_build(&dat_roms, &source_files, &request(BuildMode::ParentBundles));

        assert_eq!(plan.report.matched_roms, 1);
        assert!(plan.report.missing_roms.is_empty());
        assert_eq!(
            plan.zips[0].entries[0].source.location.path(),
            "/src-a/nested/nested.rom"
        );
    }

    #[test]
    fn parent_bundle_mode_groups_clones_under_parent_zip() {
        let dat_roms = [
            rom("parent", None, "parent.rom", "sha1-parent"),
            rom("clone", Some("parent"), "clone.rom", "sha1-clone"),
        ];
        let source_files = [
            source(
                "/src-a",
                "/src-a/parent.rom",
                None,
                "sha1-parent",
                SourceKind::BareFile,
            ),
            source(
                "/src-a",
                "/src-a/clone.rom",
                None,
                "sha1-clone",
                SourceKind::BareFile,
            ),
        ];

        let plan = plan_build(&dat_roms, &source_files, &request(BuildMode::ParentBundles));

        assert_eq!(plan.zips.len(), 1);
        assert_eq!(plan.zips[0].file_name, "parent.zip");
        assert_eq!(plan.zips[0].entries.len(), 2);
    }

    #[test]
    fn per_game_mode_writes_one_zip_per_game() {
        let dat_roms = [
            rom("parent", None, "parent.rom", "sha1-parent"),
            rom("clone", Some("parent"), "clone.rom", "sha1-clone"),
        ];
        let source_files = [
            source(
                "/src-a",
                "/src-a/parent.rom",
                None,
                "sha1-parent",
                SourceKind::BareFile,
            ),
            source(
                "/src-a",
                "/src-a/clone.rom",
                None,
                "sha1-clone",
                SourceKind::BareFile,
            ),
        ];

        let plan = plan_build(&dat_roms, &source_files, &request(BuildMode::PerGame));

        assert_eq!(plan.zips.len(), 2);
        assert_eq!(plan.zips[0].file_name, "clone.zip");
        assert_eq!(plan.zips[1].file_name, "parent.zip");
    }

    #[test]
    fn overlapping_dats_scope_to_requested_dat() {
        let dat_roms = [
            DatRom {
                catalog_name: "dat-a".to_owned(),
                key: RequirementKey::new(SetKey::new(CatalogKey::fresh(), "game"), "shared.rom"),
                parent_name: None,
                set_metadata: crate::domain::SetMetadata::default(),
                role: crate::domain::AssetRole::Rom,
                component_order: Some(0),
                expected: ExpectedEvidence {
                    sha1: Some(digest("sha1-a")),
                    ..ExpectedEvidence::default()
                },
            },
            DatRom {
                catalog_name: "dat-b".to_owned(),
                key: RequirementKey::new(SetKey::new(CatalogKey::fresh(), "game"), "shared.rom"),
                parent_name: None,
                set_metadata: crate::domain::SetMetadata::default(),
                role: crate::domain::AssetRole::Rom,
                component_order: Some(0),
                expected: ExpectedEvidence {
                    sha1: Some(digest("sha1-b")),
                    ..ExpectedEvidence::default()
                },
            },
        ];
        let source_files = [source(
            "/src-a",
            "/src-a/shared.rom",
            None,
            "sha1-a",
            SourceKind::BareFile,
        )];

        let plan = plan_build(&dat_roms, &source_files, &request(BuildMode::ParentBundles));

        assert_eq!(plan.report.matched_roms, 1);
        assert!(plan.report.missing_roms.is_empty());
        assert_eq!(plan.zips[0].entries[0].output_name, "shared.rom");
    }

    #[test]
    fn one_source_file_can_match_multiple_dats() {
        let dat_roms = [
            DatRom {
                catalog_name: "dat-a".to_owned(),
                key: RequirementKey::new(SetKey::new(CatalogKey::fresh(), "game-a"), "a.rom"),
                parent_name: None,
                set_metadata: crate::domain::SetMetadata::default(),
                role: crate::domain::AssetRole::Rom,
                component_order: Some(0),
                expected: ExpectedEvidence {
                    sha1: Some(digest("sha1-shared")),
                    ..ExpectedEvidence::default()
                },
            },
            DatRom {
                catalog_name: "dat-b".to_owned(),
                key: RequirementKey::new(SetKey::new(CatalogKey::fresh(), "game-b"), "b.rom"),
                parent_name: None,
                set_metadata: crate::domain::SetMetadata::default(),
                role: crate::domain::AssetRole::Rom,
                component_order: Some(0),
                expected: ExpectedEvidence {
                    sha1: Some(digest("sha1-shared")),
                    ..ExpectedEvidence::default()
                },
            },
        ];
        let source_files = [source(
            "/src-a",
            "/src-a/shared.rom",
            None,
            "sha1-shared",
            SourceKind::BareFile,
        )];
        let mut dat_b_request = request(BuildMode::ParentBundles);
        dat_b_request.dat_name = "dat-b".to_owned();

        let first_plan = plan_build(&dat_roms, &source_files, &request(BuildMode::ParentBundles));
        let second_plan = plan_build(&dat_roms, &source_files, &dat_b_request);

        assert_eq!(first_plan.report.matched_roms, 1);
        assert_eq!(second_plan.report.matched_roms, 1);
        assert_eq!(first_plan.zips[0].entries[0].output_name, "a.rom");
        assert_eq!(second_plan.zips[0].entries[0].output_name, "b.rom");
    }

    #[test]
    fn reordering_inputs_does_not_change_plan() {
        let dat_roms = [
            rom("z-game", None, "z.rom", "sha1-z"),
            rom("a-game", None, "a.rom", "sha1-a"),
            rom("m-game", None, "m.rom", "sha1-m"),
        ];
        let reordered_dat_roms = [
            dat_roms[2].clone(),
            dat_roms[0].clone(),
            dat_roms[1].clone(),
        ];
        let source_files = [
            source(
                "/src-a",
                "/src-a/z.rom",
                None,
                "sha1-z",
                SourceKind::BareFile,
            ),
            source(
                "/src-a",
                "/src-a/a.rom",
                None,
                "sha1-a",
                SourceKind::BareFile,
            ),
            source(
                "/src-a",
                "/src-a/m.rom",
                None,
                "sha1-m",
                SourceKind::BareFile,
            ),
        ];
        let reordered_source_files = [
            source_files[1].clone(),
            source_files[2].clone(),
            source_files[0].clone(),
        ];

        let first_plan = plan_build(&dat_roms, &source_files, &request(BuildMode::PerGame));
        let second_plan = plan_build(
            &reordered_dat_roms,
            &reordered_source_files,
            &request(BuildMode::PerGame),
        );

        assert_eq!(first_plan, second_plan);
    }

    proptest! {
        #[test]
        fn planner_output_is_stable_under_reversed_equivalent_rows(reverse in any::<bool>()) {
            let mut dat_roms = vec![
                rom("z-game", None, "z.rom", "sha1-z"),
                rom("a-game", None, "a.rom", "sha1-a"),
                rom("m-game", None, "m.rom", "sha1-m"),
            ];
            let mut source_files = vec![
                source("/src-a", "/src-a/z.rom", None, "sha1-z", SourceKind::BareFile),
                source("/src-a", "/src-a/a.rom", None, "sha1-a", SourceKind::BareFile),
                source("/src-a", "/src-a/m.rom", None, "sha1-m", SourceKind::BareFile),
            ];
            if reverse {
                dat_roms.reverse();
                source_files.reverse();
            }

            let expected = plan_build(
                &[
                    rom("z-game", None, "z.rom", "sha1-z"),
                    rom("a-game", None, "a.rom", "sha1-a"),
                    rom("m-game", None, "m.rom", "sha1-m"),
                ],
                &[
                    source("/src-a", "/src-a/z.rom", None, "sha1-z", SourceKind::BareFile),
                    source("/src-a", "/src-a/a.rom", None, "sha1-a", SourceKind::BareFile),
                    source("/src-a", "/src-a/m.rom", None, "sha1-m", SourceKind::BareFile),
                ],
                &request(BuildMode::PerGame),
            );
            let actual = plan_build(&dat_roms, &source_files, &request(BuildMode::PerGame));

            prop_assert_eq!(actual, expected);
        }
    }
}
