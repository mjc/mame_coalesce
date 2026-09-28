use indicatif::{ProgressBar, ProgressStyle};
use log::{info, warn};
use mame_coalesce::{
    app::{BuildWorkflowReport, ScanProgressEvent, SourceScanReport},
    domain::{ArtifactOutcome, PlanOutcome},
};

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
                duplicate.candidates.len()
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

#[cfg(test)]
mod tests {
    use super::*;
    use mame_coalesce::domain::{BuildReport, PlanBlockReason};

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
}
