use std::collections::BTreeMap;

use crate::domain::{
    BuildPlan, BuildReport, BuildRequest, DatRom, DuplicateMatch, MissingRom, SourceFile,
    ZipEntrySpec, ZipSpec,
};

#[must_use]
pub fn plan_build(
    dat_roms: &[DatRom],
    source_files: &[SourceFile],
    request: &BuildRequest,
) -> BuildPlan {
    let mut report = BuildReport::default();
    let mut zips_by_name = BTreeMap::<String, Vec<ZipEntrySpec>>::new();
    let source_by_sha1 = sources_for_root(source_files, &request.source_root);
    let selected_roms = selected_dat_roms(dat_roms, &request.dat_name);

    for rom in selected_roms {
        let candidates = source_by_sha1.get(&rom.sha1).cloned().unwrap_or_default();

        if candidates.is_empty() {
            report.missing_roms.push(MissingRom {
                game_name: rom.game_name,
                rom_name: rom.rom_name,
                sha1: rom.sha1,
            });
            continue;
        }

        let selected = candidates[0].clone();
        if candidates.len() > 1 {
            report.duplicate_matches.push(DuplicateMatch {
                rom_name: rom.rom_name.clone(),
                selected: selected.clone(),
                candidates,
            });
        }

        report.matched_roms += 1;
        let bundle_name = rom.bundle_name(request.mode).to_owned();
        zips_by_name
            .entry(format!("{bundle_name}.zip"))
            .or_default()
            .push(ZipEntrySpec {
                output_name: rom.rom_name,
                source: selected,
            });
    }

    if request.strict && !report.missing_roms.is_empty() {
        report.exit_code = 2;
        return BuildPlan {
            zips: Vec::new(),
            report,
            dry_run: request.dry_run,
        };
    }

    BuildPlan {
        zips: zips_by_name
            .into_iter()
            .map(|(file_name, entries)| ZipSpec { file_name, entries })
            .collect(),
        report,
        dry_run: request.dry_run,
    }
}

fn selected_dat_roms(dat_roms: &[DatRom], dat_name: &str) -> Vec<DatRom> {
    let mut selected = dat_roms
        .iter()
        .filter(|rom| rom.dat_name == dat_name)
        .cloned()
        .collect::<Vec<_>>();
    selected.sort();
    selected
}

fn sources_for_root(
    source_files: &[SourceFile],
    source_root: &str,
) -> BTreeMap<String, Vec<SourceFile>> {
    let mut source_by_sha1 = BTreeMap::<String, Vec<SourceFile>>::new();
    for source in source_files
        .iter()
        .filter(|source| source.source_root == source_root)
    {
        source_by_sha1
            .entry(source.sha1.clone())
            .or_default()
            .push(source.clone());
    }

    for candidates in source_by_sha1.values_mut() {
        candidates.sort_by(|left, right| {
            left.kind
                .priority()
                .cmp(&right.kind.priority())
                .then_with(|| left.canonical_path.cmp(&right.canonical_path))
                .then_with(|| left.entry_name.cmp(&right.entry_name))
        });
    }

    source_by_sha1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{BuildMode, SourceKind};

    fn request(mode: BuildMode) -> BuildRequest {
        BuildRequest {
            dat_name: "dat-a".to_owned(),
            source_root: "/src-a".to_owned(),
            mode,
            dry_run: false,
            strict: false,
        }
    }

    fn rom(game_name: &str, parent_name: Option<&str>, rom_name: &str, sha1: &str) -> DatRom {
        DatRom {
            dat_name: "dat-a".to_owned(),
            game_name: game_name.to_owned(),
            parent_name: parent_name.map(str::to_owned),
            rom_name: rom_name.to_owned(),
            sha1: sha1.to_owned(),
        }
    }

    fn source(
        root: &str,
        path: &str,
        entry_name: Option<&str>,
        sha1: &str,
        kind: SourceKind,
    ) -> SourceFile {
        SourceFile {
            source_root: root.to_owned(),
            canonical_path: path.to_owned(),
            entry_name: entry_name.map(str::to_owned),
            sha1: sha1.to_owned(),
            kind,
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
            plan.report.duplicate_matches[0].selected.canonical_path,
            "/src-a/bare-a.rom"
        );
        assert_eq!(
            plan.zips[0].entries[0].source.canonical_path,
            "/src-a/bare-a.rom"
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
                dat_name: "dat-a".to_owned(),
                game_name: "game".to_owned(),
                parent_name: None,
                rom_name: "shared.rom".to_owned(),
                sha1: "sha1-a".to_owned(),
            },
            DatRom {
                dat_name: "dat-b".to_owned(),
                game_name: "game".to_owned(),
                parent_name: None,
                rom_name: "shared.rom".to_owned(),
                sha1: "sha1-b".to_owned(),
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
                dat_name: "dat-a".to_owned(),
                game_name: "game-a".to_owned(),
                parent_name: None,
                rom_name: "a.rom".to_owned(),
                sha1: "sha1-shared".to_owned(),
            },
            DatRom {
                dat_name: "dat-b".to_owned(),
                game_name: "game-b".to_owned(),
                parent_name: None,
                rom_name: "b.rom".to_owned(),
                sha1: "sha1-shared".to_owned(),
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
}
