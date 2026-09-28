use indicatif::{ProgressBar, ProgressStyle};
use log::{info, warn};
use mame_coalesce::{
    app::{BuildWorkflowReport, ScanProgressEvent, SourceScanReport},
    domain::{ArtifactOutcome, AuditReport, ObservationBasis, PlanOutcome},
    resolution::ResolutionStatus,
};
use std::fmt::Write as _;

/// Terminal progress display for source scanning.
pub struct ScanProgressReporter(ProgressBar);

impl ScanProgressReporter {
    /// Update the display from one scan progress event.
    pub fn update(&self, event: ScanProgressEvent) {
        match event {
            ScanProgressEvent::Started { files } => {
                self.0.set_length(files);
                self.0.set_position(0);
            }
            ScanProgressEvent::Advanced => self.0.inc(1),
        }
    }

    /// Clear the progress display after the scan completes.
    pub fn finish(&self) {
        self.0.finish_and_clear();
    }
}

impl Default for ScanProgressReporter {
    fn default() -> Self {
        let bar = ProgressBar::new(0);
        let style = ProgressStyle::default_bar()
            .template("[{elapsed}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg} ETA: {eta}")
            .unwrap_or_else(|_| ProgressStyle::default_bar());
        bar.set_style(style);
        Self(bar)
    }
}

impl Drop for ScanProgressReporter {
    fn drop(&mut self) {
        self.0.finish_and_clear();
    }
}

/// Log a concise summary of scan results and unmatched content.
pub fn scan_report(report: &SourceScanReport) {
    info!(
        "scanned {} ROM files at {}",
        report.observation_count, report.source_path
    );
    if report.associated_rom_count == 0 && report.observation_count > 0 {
        warn!(
            "scanned {} ROM files, but none matched imported DAT ROMs",
            report.observation_count
        );
    }
}

/// Log build diagnostics and each artifact's final outcome.
pub fn build_report(report: &BuildWorkflowReport) {
    let build = &report.build_report;
    info!("matched {} ROMs", build.matched_roms);
    if !build.missing_roms.is_empty() {
        warn!("{} ROMs are missing", build.missing_roms.len());
        for missing in &build.missing_roms {
            warn!(
                "missing ROM: game={} rom={} sha1={}",
                missing.game_name,
                missing.rom_name,
                missing
                    .sha1
                    .map_or_else(|| "not supplied".to_owned(), hex::encode)
            );
        }
    }
    if !build.duplicate_matches.is_empty() {
        warn!(
            "{} ROMs had duplicate source matches",
            build.duplicate_matches.len()
        );
        for duplicate in &build.duplicate_matches {
            warn!(
                "duplicate ROM match: rom={} selected={} candidates={}",
                duplicate.rom_name,
                duplicate.selected.display_name(),
                duplicate.candidates.len() + duplicate.omitted_candidates
            );
        }
    }
    for issue in &build.validation_issues {
        warn!("build plan validation: {issue}");
    }
    for artifact in &report.artifact_results {
        match &artifact.outcome {
            ArtifactOutcome::Completed => info!("completed output artifact: {}", artifact.path),
            ArtifactOutcome::Failed { error } => {
                warn!("failed output artifact: {}: {error}", artifact.path);
            }
            ArtifactOutcome::ReplacedButNotDurable { error } => {
                warn!(
                    "replaced output artifact, but durability is uncertain: {}: {error}",
                    artifact.path
                );
            }
            ArtifactOutcome::Unattempted => {}
        }
    }
}

/// Map a build report to success, artifact failure, or blocked-plan status.
pub fn exit_code(report: &BuildWorkflowReport) -> std::process::ExitCode {
    if report.build_report.outcome != PlanOutcome::Ready {
        std::process::ExitCode::from(2)
    } else if report.artifact_results.iter().any(|artifact| {
        matches!(
            &artifact.outcome,
            ArtifactOutcome::Failed { .. } | ArtifactOutcome::ReplacedButNotDurable { .. }
        )
    }) {
        std::process::ExitCode::from(1)
    } else {
        std::process::ExitCode::SUCCESS
    }
}

pub fn audit_report(report: &AuditReport) -> String {
    let mut output = String::new();
    match report.observation_basis() {
        ObservationBasis::Cached => {
            output
                .push_str("Audit against cached observations (ROM bytes were not freshly read)\n");
        }
        ObservationBasis::FreshScan { scan_run } => {
            let _ = writeln!(
                output,
                "Audit after a fresh source scan (run {})",
                scan_run.to_storage_key()
            );
        }
    }
    let build = report.report();
    let _ = writeln!(
        output,
        "Matched: {}  Unresolved: {}  Equivalent duplicate matches: {}",
        build.matched_roms,
        build.missing_roms.len(),
        build.duplicate_matches.len()
    );
    if build.outcome != PlanOutcome::Ready {
        let _ = writeln!(output, "Plan readiness: {:?}", build.outcome);
        for issue in &build.validation_issues {
            let _ = writeln!(output, "  validation issue: {issue}");
        }
    }
    for resolution in &build.resolutions {
        render_audit_resolution(&mut output, resolution);
    }
    output
}

fn render_audit_resolution(
    output: &mut String,
    resolution: &mame_coalesce::resolution::RequirementResolution,
) {
    let requirement = &resolution.requirement;
    match &resolution.status {
        ResolutionStatus::Matched {
            selected,
            strength,
            equivalent_copies,
            omitted_equivalent_copies,
            omitted_assessments,
            ..
        } => {
            let _ = writeln!(
                output,
                "Matched: {}/{} <- {} ({strength:?})",
                requirement.game_name(),
                requirement.rom_name(),
                selected.display_name()
            );
            render_expected(output, &requirement.expected);
            render_observed(output, &selected.observed);
            if equivalent_copies.len() > 1 {
                for copy in equivalent_copies {
                    let _ = writeln!(output, "  equivalent copy: {}", copy.display_name());
                }
            }
            if *omitted_equivalent_copies > 0 || *omitted_assessments > 0 {
                let _ = writeln!(
                    output,
                    "  additional evidence details omitted: copies={omitted_equivalent_copies} assessments={omitted_assessments}"
                );
            }
        }
        ResolutionStatus::Ambiguous {
            candidates,
            omitted_candidates,
        } => {
            let _ = writeln!(
                output,
                "Ambiguous: {}/{}",
                requirement.game_name(),
                requirement.rom_name()
            );
            render_expected(output, &requirement.expected);
            render_candidates(output, candidates);
            render_omitted_candidates(output, *omitted_candidates);
        }
        ResolutionStatus::Conflicting {
            candidates,
            omitted_candidates,
        } => {
            let _ = writeln!(
                output,
                "Conflicting evidence: {}/{}",
                requirement.game_name(),
                requirement.rom_name()
            );
            render_expected(output, &requirement.expected);
            render_candidates(output, candidates);
            render_omitted_candidates(output, *omitted_candidates);
        }
        ResolutionStatus::Missing {
            reason,
            assessments,
            omitted_assessments,
        } => {
            let _ = writeln!(
                output,
                "Missing: {}/{} ({reason:?})",
                requirement.game_name(),
                requirement.rom_name()
            );
            render_expected(output, &requirement.expected);
            render_candidates(output, assessments);
            render_omitted_candidates(output, *omitted_assessments);
        }
    }
}

fn render_omitted_candidates(output: &mut String, omitted: usize) {
    if omitted > 0 {
        let _ = writeln!(output, "  additional evidence details omitted: {omitted}");
    }
}

fn render_candidates(
    output: &mut String,
    candidates: &[mame_coalesce::resolution::SourceAssessment],
) {
    for candidate in candidates {
        let _ = writeln!(
            output,
            "  candidate: {} strength={:?} agrees={:?} conflicts={:?}",
            candidate.source.display_name(),
            candidate.strength,
            candidate.agreements,
            candidate.conflicts
        );
        render_observed(output, &candidate.source.observed);
    }
}

fn render_expected(output: &mut String, evidence: &mame_coalesce::domain::ExpectedEvidence) {
    let _ = writeln!(
        output,
        "  expected: sha1={} md5={} crc={} size={:?}",
        evidence
            .sha1
            .map_or_else(|| "unknown".to_owned(), hex::encode),
        evidence
            .md5
            .as_ref()
            .map_or_else(|| "unknown".to_owned(), |digest| hex::encode(digest.0)),
        evidence
            .crc
            .as_ref()
            .map_or_else(|| "unknown".to_owned(), |digest| hex::encode(digest.0)),
        evidence.size
    );
}

fn render_observed(output: &mut String, evidence: &mame_coalesce::domain::ObservedContent) {
    let _ = writeln!(
        output,
        "  observed ({:?}/{:?}): sha1={} md5={} crc={} xxh3={} size={:?}",
        evidence.scope,
        evidence.provenance,
        evidence
            .sha1
            .map_or_else(|| "unknown".to_owned(), hex::encode),
        evidence
            .md5
            .as_ref()
            .map_or_else(|| "unknown".to_owned(), |digest| hex::encode(digest.0)),
        evidence
            .crc
            .as_ref()
            .map_or_else(|| "unknown".to_owned(), |digest| hex::encode(digest.0)),
        hex::encode(evidence.xxh3),
        evidence.size
    );
}

pub fn audit_exit_code(report: &AuditReport) -> std::process::ExitCode {
    if report.report().missing_roms.is_empty() && report.report().outcome == PlanOutcome::Ready {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::from(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mame_coalesce::{
        build::validation::{PlanIssue, PlanIssueKind},
        domain::{AuditReport, BuildReport, LogicalPath, ObservationBasis, PlanBlockReason},
    };

    /// Construct a minimal ready report for exit-status mapping tests.
    fn report() -> BuildWorkflowReport {
        BuildWorkflowReport {
            written_paths: Vec::new(),
            artifact_results: Vec::new(),
            build_report: BuildReport::default(),
            scan_report: None,
        }
    }

    /// Verifies ready, failed, uncertain, and blocked reports map to their CLI statuses.
    #[test]
    fn process_status_maps_ready_partial_and_blocked_builds() {
        let ready = report();
        assert_eq!(exit_code(&ready), std::process::ExitCode::SUCCESS);

        let mut partial = report();
        partial
            .artifact_results
            .push(mame_coalesce::domain::ArtifactResult {
                path: "failed.zip".to_owned(),
                outcome: ArtifactOutcome::Failed {
                    error: "test failure".to_owned(),
                },
            });
        assert_eq!(exit_code(&partial), std::process::ExitCode::from(1));

        let mut uncertain = report();
        uncertain
            .artifact_results
            .push(mame_coalesce::domain::ArtifactResult {
                path: "replaced.zip".to_owned(),
                outcome: ArtifactOutcome::ReplacedButNotDurable {
                    error: "sync failed".to_owned(),
                },
            });
        assert_eq!(exit_code(&uncertain), std::process::ExitCode::from(1));

        let mut blocked = report();
        blocked.build_report.outcome = PlanOutcome::Blocked(PlanBlockReason::MissingContent);
        assert_eq!(exit_code(&blocked), std::process::ExitCode::from(2));
    }

    #[test]
    fn audit_reports_blocked_plan_and_returns_unresolved_status() {
        let report = AuditReport::new(
            ObservationBasis::Cached,
            BuildReport {
                validation_issues: vec![PlanIssue {
                    kind: PlanIssueKind::UnsafeGroupPath,
                    path: LogicalPath::new("../unsafe.zip"),
                    conflicts_with: None,
                }],
                outcome: PlanOutcome::Blocked(PlanBlockReason::InvalidPlan),
                ..BuildReport::default()
            },
        );

        let human = audit_report(&report);

        assert_eq!(audit_exit_code(&report), std::process::ExitCode::from(1));
        assert!(human.contains("Plan readiness: Blocked(InvalidPlan)"));
        assert!(human.contains("validation issue: unsafe output zip file name"));
    }
}
