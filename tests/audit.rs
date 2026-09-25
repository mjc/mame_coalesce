use std::{fs, io, process::Command};

use assert_cmd::prelude::*;
use camino::Utf8PathBuf;
use mame_coalesce::{
    app::{
        self, AuditRefresh, AuditRequest, BuildPlanRequest, DatImportRequest, SourceScanRequest,
    },
    database::Database,
    domain::{
        AuditReport, BuildMode, MatchingPolicy, MissingContentPolicy, ObservationBasis, PlanOutcome,
    },
    resolution::ResolutionStatus,
};
use predicates::str::contains;

const AUDIT_DAT: &str = r#"<?xml version="1.0"?><datafile>
<header><name>Audit fixture</name></header>
<game name="game"><rom name="game.rom" size="3" crc="352441c2" md5="900150983cd24fb0d6963f7d28e17f72" sha1="a9993e364706816aba3e25717850c26c9cd0d89d"/></game>
<game name="missing"><rom name="missing.rom" size="7" crc="00000000" md5="00000000000000000000000000000000" sha1="0000000000000000000000000000000000000000"/></game>
</datafile>"#;

struct Fixture {
    temp: tempfile::TempDir,
    database: Database,
    dat: Utf8PathBuf,
    source: Utf8PathBuf,
}

impl Fixture {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let root = Utf8PathBuf::try_from(temp.path().to_path_buf())?;
        let dat = root.join("catalog.dat");
        let source = root.join("roms");
        fs::create_dir(&source)?;
        fs::write(&dat, AUDIT_DAT)?;
        fs::write(source.join("game.rom"), b"abc")?;
        fs::write(source.join("copy.rom"), b"abc")?;
        let database = Database::open(&root.join("cache.db"))?;
        app::import_dat(
            &database,
            &DatImportRequest {
                dat_path: dat.clone(),
            },
        )?;
        app::scan_source(
            &database,
            &SourceScanRequest {
                source_path: source.clone(),
                jobs: 1,
            },
        )?;
        Ok(Self {
            temp,
            database,
            dat,
            source,
        })
    }

    fn request(&self, refresh: AuditRefresh) -> AuditRequest {
        AuditRequest {
            dat_path: self.dat.clone(),
            source_path: self.source.clone(),
            refresh,
            matching_policy: MatchingPolicy::Sha1Compatibility,
            jobs: 1,
        }
    }

    fn evidence_aware_request(&self) -> AuditRequest {
        AuditRequest {
            matching_policy: MatchingPolicy::EvidenceAware,
            ..self.request(AuditRefresh::Refresh)
        }
    }
}

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mame_coalesce"))
}

#[test]
fn audit_uses_cached_observations_by_default_and_refresh_is_explicit()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    let build_plan = app::plan_build(
        &fixture.database,
        &BuildPlanRequest {
            dat_path: fixture.dat.clone(),
            source_path: fixture.source.clone(),
            mode: BuildMode::ParentBundles,
            matching_policy: MatchingPolicy::Sha1Compatibility,
            missing_policy: MissingContentPolicy::AllowPartial,
        },
    )?;
    fs::write(fixture.source.join("game.rom"), b"changed")?;
    fs::write(fixture.source.join("copy.rom"), b"changed")?;

    let cached = app::audit(&fixture.database, &fixture.request(AuditRefresh::Cached))?;
    assert_eq!(cached.observation_basis(), &ObservationBasis::Cached);
    assert_eq!(cached.report().matched_roms, build_plan.report.matched_roms);
    assert_eq!(cached.report().missing_roms, build_plan.report.missing_roms);
    assert_eq!(
        cached.report().resolutions.len(),
        build_plan.report.resolutions.len()
    );
    assert_eq!(
        cached.report().resolutions[0].status,
        build_plan.report.resolutions[0].status
    );
    assert_eq!(
        cached.report().resolutions[1].status,
        build_plan.report.resolutions[1].status
    );
    assert_eq!(cached.report().matched_roms, 1);
    assert_eq!(cached.report().missing_roms.len(), 1);
    assert_eq!(cached.report().outcome, PlanOutcome::Ready);
    assert_eq!(cached.report().duplicate_matches.len(), 1);
    assert_eq!(fs::read(fixture.source.join("game.rom"))?, b"changed");
    assert!(matches!(
        cached.report().resolutions[1].status,
        ResolutionStatus::Missing { .. }
    ));

    let refreshed = app::audit(&fixture.database, &fixture.evidence_aware_request())?;
    let refreshed_plan = app::plan_build(
        &fixture.database,
        &BuildPlanRequest {
            dat_path: fixture.dat.clone(),
            source_path: fixture.source.clone(),
            mode: BuildMode::ParentBundles,
            matching_policy: MatchingPolicy::EvidenceAware,
            missing_policy: MissingContentPolicy::AllowPartial,
        },
    )?;
    assert!(matches!(
        refreshed.observation_basis(),
        ObservationBasis::FreshScan { .. }
    ));
    assert_eq!(refreshed.report().matched_roms, 0);
    assert_eq!(refreshed.report().missing_roms.len(), 2);
    assert_eq!(
        refreshed.report().resolutions.len(),
        refreshed_plan.report.resolutions.len()
    );
    for (audit, build) in refreshed
        .report()
        .resolutions
        .iter()
        .zip(&refreshed_plan.report.resolutions)
    {
        assert_eq!(audit.status, build.status);
    }
    assert!(matches!(
        refreshed.report().resolutions[0].status,
        ResolutionStatus::Conflicting { .. }
    ));
    let cached_after_refresh =
        app::audit(&fixture.database, &fixture.request(AuditRefresh::Cached))?;
    assert_eq!(
        cached_after_refresh.observation_basis(),
        &ObservationBasis::Cached
    );
    assert_eq!(cached_after_refresh.report().missing_roms.len(), 2);
    assert_eq!(fs::read(fixture.source.join("game.rom"))?, b"changed");
    Ok(())
}

#[test]
fn audit_cli_writes_only_versioned_json_to_stdout_and_labels_refresh()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    let cache_path = fixture.temp.path().join("cache.db");
    let cache_path = camino::Utf8Path::from_path(&cache_path)
        .ok_or_else(|| io::Error::other("cache path is not UTF-8"))?;

    let output = cli()
        .args([
            "--cache",
            cache_path.as_str(),
            "audit",
            fixture.dat.as_str(),
            fixture.source.as_str(),
            "--format",
            "json",
        ])
        .output()?;
    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json["schema_version"], 1);
    assert_eq!(json["observation_basis"], "Cached");
    assert_eq!(json["report"]["matched_roms"], 1);
    assert_eq!(
        json["report"]["missing_roms"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(
        json["report"]["duplicate_matches"].as_array().map(Vec::len),
        Some(1)
    );
    assert!(output.stderr.is_empty());

    fs::write(fixture.source.join("game.rom"), b"changed")?;
    fs::write(fixture.source.join("copy.rom"), b"changed")?;
    let output = cli()
        .args([
            "--cache",
            cache_path.as_str(),
            "audit",
            fixture.dat.as_str(),
            fixture.source.as_str(),
            "--refresh",
            "--matching-policy",
            "evidence-aware",
            "--format",
            "json",
        ])
        .output()?;
    assert_eq!(output.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert!(json["observation_basis"]["FreshScan"]["scan_run"].is_string());
    assert_eq!(json["report"]["matched_roms"], 0);
    assert!(output.stderr.is_empty());
    assert_eq!(fs::read(fixture.source.join("game.rom"))?, b"changed");
    assert_eq!(
        json["report"]["resolutions"][0]["status"]["Conflicting"]["candidates"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );
    Ok(())
}

#[test]
fn audit_json_schema_fixture_round_trips_and_rejects_unknown_versions()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = include_bytes!("fixtures/audit/empty-v1.json");
    let report = AuditReport::from_json(fixture)?;
    assert_eq!(report.schema_version(), 1);
    assert_eq!(report.observation_basis(), &ObservationBasis::Cached);
    let encoded = report.to_json()?;
    assert_eq!(
        std::str::from_utf8(&encoded)?,
        std::str::from_utf8(fixture)?.trim_end()
    );
    assert_eq!(AuditReport::from_json(&encoded)?, report);
    let unknown_version =
        std::str::from_utf8(fixture)?.replace("\"schema_version\": 1", "\"schema_version\": 99");
    assert!(matches!(
        AuditReport::from_json(unknown_version.as_bytes()),
        Err(mame_coalesce::Error::UnsupportedAuditVersion(99))
    ));
    Ok(())
}

#[test]
fn audit_human_format_shows_resolution_and_observation_basis()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    let cache_path = fixture.temp.path().join("cache.db");
    let output = cli()
        .args([
            "--cache",
            cache_path.to_str().ok_or("cache path is not UTF-8")?,
            "audit",
            fixture.dat.as_str(),
            fixture.source.as_str(),
        ])
        .assert()
        .code(1)
        .stdout(contains("cached observations"))
        .stdout(contains("Matched: game/game.rom"))
        .stdout(contains("equivalent copy:"))
        .stdout(contains("Missing: missing/missing.rom"))
        .get_output()
        .stdout
        .clone();
    assert!(!output.is_empty());
    Ok(())
}
