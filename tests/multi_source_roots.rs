use std::{fs, process::Command};

use camino::Utf8PathBuf;
use mame_coalesce::{
    app::{
        self, AuditRefresh, AuditRequest, BuildPlanRequest, BuildWorkflowRequest, DatImportRequest,
        SourceRootSelection,
    },
    database::Database,
    domain::{BuildMode, MatchingPolicy, MissingContentPolicy, ObservationBasis, ZipCompression},
    resolution::ResolutionStatus,
};

fn catalog_dat() -> String {
    let abc = hex::encode(mame_coalesce::hashes::sha1_bytes(b"abc"));
    let def = hex::encode(mame_coalesce::hashes::sha1_bytes(b"def"));
    format!(
        r#"<?xml version="1.0"?><datafile><header><name>Roots fixture</name></header>
        <game name="game-a"><rom name="game-a.rom" size="3" crc="00000000" md5="00000000000000000000000000000000" sha1="{abc}"/></game>
        <game name="game-b"><rom name="game-b.rom" size="3" crc="00000000" md5="00000000000000000000000000000000" sha1="{def}"/></game>
        </datafile>"#
    )
}

const fn selection(primary: Utf8PathBuf, additional: Vec<Utf8PathBuf>) -> SourceRootSelection {
    SourceRootSelection {
        primary,
        additional,
    }
}

fn assert_resolution_equivalent(
    fresh: &mame_coalesce::resolution::RequirementResolution,
    cached: &mame_coalesce::resolution::RequirementResolution,
) {
    match (&fresh.status, &cached.status) {
        (
            ResolutionStatus::Matched {
                selected: fresh, ..
            },
            ResolutionStatus::Matched {
                selected: cached, ..
            },
        ) => {
            assert_eq!(fresh.location, cached.location);
            assert_eq!(fresh.source_root, cached.source_root);
            assert_eq!(fresh.observed, cached.observed);
        }
        (
            ResolutionStatus::Missing { reason: fresh, .. },
            ResolutionStatus::Missing { reason: cached, .. },
        ) => assert_eq!(fresh, cached),
        (fresh, cached) => assert_eq!(
            std::mem::discriminant(fresh),
            std::mem::discriminant(cached),
            "audit and build must classify a requirement the same way"
        ),
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn build_and_audit_resolve_across_ordered_canonical_roots_and_dedup_overlap()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let root = Utf8PathBuf::try_from(temp.path().to_path_buf())?;
    let dat = root.join("catalog.dat");
    let outer = root.join("outer");
    let nested = outer.join("nested");
    let extra = root.join("extra");
    fs::create_dir_all(&nested)?;
    fs::create_dir(&extra)?;
    fs::write(&dat, catalog_dat())?;
    fs::write(nested.join("game-a.rom"), b"abc")?;
    fs::write(extra.join("game-b.rom"), b"def")?;
    fs::write(extra.join("game-a-copy.rom"), b"abc")?;
    let database = Database::open(&root.join("cache.db"))?;
    app::import_dat(
        &database,
        &DatImportRequest {
            dat_path: dat.clone(),
        },
    )?;

    let roots = selection(
        outer.clone(),
        vec![
            extra.clone(),
            nested.clone(),
            outer.join("."),
            nested.join(".."),
        ],
    );
    let scans = app::scan_sources_with_progress(&database, &roots, 1, &|_| {})?;
    assert_eq!(scans.len(), 3, "canonical root aliases are scanned once");
    assert_eq!(
        scans
            .iter()
            .map(|scan| scan.observation_count)
            .sum::<usize>(),
        4,
        "overlapping roots retain distinct scan provenance while aliases scan only once"
    );

    let build_request = BuildWorkflowRequest {
        dat_path: dat.clone(),
        source_path: outer.clone(),
        destination_path: root.join("output"),
        mode: BuildMode::ParentBundles,
        compression: ZipCompression::Store,
        dry_run: false,
        strict: true,
    };
    let built = app::build_with_roots(&database, &build_request, &roots)?;
    assert_eq!(built.build_report.matched_roms, 2);
    assert!(built.build_report.missing_roms.is_empty());
    assert_eq!(built.written_paths.len(), 2);
    assert_eq!(built.build_report.duplicate_matches.len(), 1);
    let duplicate = &built.build_report.duplicate_matches[0];
    assert_eq!(
        duplicate.selected.location.path(),
        nested.join("game-a.rom").canonicalize_utf8()?.as_str()
    );
    assert_eq!(duplicate.candidates.len(), 2);
    assert_eq!(
        duplicate.candidates[0].location,
        duplicate.selected.location
    );
    assert_eq!(
        duplicate.candidates[1].location.path(),
        extra.join("game-a-copy.rom").canonicalize_utf8()?.as_str()
    );
    let repeated_plan = app::plan_build_with_roots(
        &database,
        &BuildPlanRequest {
            dat_path: dat.clone(),
            source_path: outer.clone(),
            mode: BuildMode::ParentBundles,
            matching_policy: MatchingPolicy::Sha1Compatibility,
            missing_policy: MissingContentPolicy::AllowPartial,
        },
        &roots,
    )?;
    assert_eq!(
        repeated_plan.report.duplicate_matches,
        built.build_report.duplicate_matches
    );
    let selected_a = built
        .build_report
        .resolutions
        .iter()
        .find_map(|resolution| match &resolution.status {
            ResolutionStatus::Matched { selected, .. }
                if resolution.requirement.key.set().name() == "game-a" =>
            {
                Some(selected.source_root.clone())
            }
            _ => None,
        })
        .ok_or("game-a did not resolve")?;
    assert_eq!(selected_a.as_str(), outer.canonicalize_utf8()?.as_str());

    let audit = app::audit_with_roots_and_progress(
        &database,
        &AuditRequest {
            dat_path: dat.clone(),
            source_path: outer.clone(),
            refresh: AuditRefresh::Refresh,
            matching_policy: MatchingPolicy::Sha1Compatibility,
            jobs: 1,
        },
        &roots,
        &|_| {},
    )?;
    assert!(matches!(
        audit.observation_basis(),
        ObservationBasis::FreshScans { scan_runs } if scan_runs.len() == 3
    ));
    assert_eq!(audit.report().matched_roms, 2);
    assert_eq!(audit.report().missing_roms, built.build_report.missing_roms);
    for (audit_resolution, build_resolution) in audit
        .report()
        .resolutions
        .iter()
        .zip(&built.build_report.resolutions)
    {
        assert_resolution_equivalent(audit_resolution, build_resolution);
    }
    let reversed_plan = app::plan_build_with_roots(
        &database,
        &BuildPlanRequest {
            dat_path: dat.clone(),
            source_path: nested.clone(),
            mode: BuildMode::ParentBundles,
            matching_policy: MatchingPolicy::Sha1Compatibility,
            missing_policy: MissingContentPolicy::AllowPartial,
        },
        &selection(nested.clone(), vec![outer.clone(), extra.clone()]),
    )?;
    let selected_nested_first = match &reversed_plan.report.resolutions[0].status {
        ResolutionStatus::Matched { selected, .. } => &selected.source_root,
        _ => return Err("game-a was not matched by audit".into()),
    };
    assert_eq!(
        selected_nested_first.as_str(),
        nested.canonicalize_utf8()?.as_str()
    );

    let cache_path = root.join("cache.db");
    let cli_output = Command::new(env!("CARGO_BIN_EXE_mame_coalesce"))
        .args([
            "--cache",
            cache_path.as_str(),
            "audit",
            dat.as_str(),
            outer.as_str(),
            "--source-root",
            extra.as_str(),
            "--source-root",
            nested.as_str(),
            "--refresh",
            "--format",
            "json",
        ])
        .output()?;
    assert_eq!(cli_output.status.code(), Some(0));
    let cli_report: serde_json::Value = serde_json::from_slice(&cli_output.stdout)?;
    assert_eq!(
        cli_report["observation_basis"]["FreshScans"]["scan_runs"]
            .as_array()
            .map(Vec::len),
        Some(3)
    );

    let single_root_plan = app::plan_build(
        &database,
        &BuildPlanRequest {
            dat_path: dat,
            source_path: outer,
            mode: BuildMode::ParentBundles,
            matching_policy: MatchingPolicy::Sha1Compatibility,
            missing_policy: MissingContentPolicy::AllowPartial,
        },
    )?;
    assert_eq!(single_root_plan.report.matched_roms, 1);
    Ok(())
}

#[test]
fn failed_root_scan_preserves_every_requested_cached_scope()
-> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let root = Utf8PathBuf::try_from(temp.path().to_path_buf())?;
    let dat = root.join("catalog.dat");
    let first = root.join("first");
    let second = root.join("second");
    fs::create_dir(&first)?;
    fs::create_dir(&second)?;
    fs::write(&dat, catalog_dat())?;
    fs::write(first.join("game-a.rom"), b"abc")?;
    fs::write(second.join("game-b.rom"), b"def")?;
    let database = Database::open(&root.join("cache.db"))?;
    app::import_dat(
        &database,
        &DatImportRequest {
            dat_path: dat.clone(),
        },
    )?;
    let roots = selection(first.clone(), vec![second.clone()]);
    app::scan_sources_with_progress(&database, &roots, 1, &|_| {})?;

    fs::write(first.join("game-a.rom"), b"changed")?;
    fs::write(second.join("broken.zip"), b"PK\x03\x04invalid")?;
    assert!(app::scan_sources_with_progress(&database, &roots, 1, &|_| {}).is_err());

    let cached = app::plan_build(
        &database,
        &BuildPlanRequest {
            dat_path: dat.clone(),
            source_path: first,
            mode: BuildMode::ParentBundles,
            matching_policy: MatchingPolicy::Sha1Compatibility,
            missing_policy: MissingContentPolicy::AllowPartial,
        },
    )?;
    assert_eq!(cached.report.matched_roms, 1);
    let cached_multi = app::audit_with_roots_and_progress(
        &database,
        &AuditRequest {
            dat_path: dat,
            source_path: roots.primary.clone(),
            refresh: AuditRefresh::Cached,
            matching_policy: MatchingPolicy::Sha1Compatibility,
            jobs: 1,
        },
        &roots,
        &|_| {},
    )?;
    assert_eq!(cached_multi.report().matched_roms, 2);
    Ok(())
}
