use mame_coalesce::app::{DiskAuditReport, DiskAuditState};

use crate::options::ReportFormatArg;

pub fn write_disk_audit(
    report: &DiskAuditReport,
    format: ReportFormatArg,
) -> mame_coalesce::Result<()> {
    match format {
        ReportFormatArg::Json => println!("{}", serde_json::to_string_pretty(report)?),
        ReportFormatArg::Text => print!("{}", render_disk_audit(report)),
    }
    Ok(())
}

fn render_disk_audit(report: &DiskAuditReport) -> String {
    let mut output = format!(
        "Disk audit for catalog {} (snapshot {})\nSource: {}\n",
        report.catalog_key, report.snapshot_key, report.source_path
    );
    if report.disks.is_empty() {
        output.push_str("No disk requirements found.\n");
        return output;
    }
    for disk in &report.disks {
        use std::fmt::Write as _;
        let _ = writeln!(
            output,
            "{} / {}: {}{}",
            disk.set_name,
            disk.disk_name,
            state_label(disk.state),
            disk.parent_disk
                .as_ref()
                .map_or_else(String::new, |parent| format!(" (parent disk: {parent})"))
        );
        if let Some(sha1) = &disk.expected_logical_sha1 {
            let _ = writeln!(output, "  source-declared logical SHA-1: {sha1}");
        }
    }
    output
}

const fn state_label(state: DiskAuditState) -> &'static str {
    match state {
        DiskAuditState::Missing => "missing",
        DiskAuditState::UnknownDigestScope => "present; digest scope unknown",
        DiskAuditState::IdentityNotDeclared => "present; logical identity not declared",
        DiskAuditState::UnverifiedContainer => "CHD container present; logical identity unverified",
        DiskAuditState::UnsupportedContainer => {
            "unsupported disk container filename (expected .chd)"
        }
        DiskAuditState::VerifiedLogicalIdentity => "verified logical identity",
        DiskAuditState::LogicalIdentityMismatch => "logical identity mismatch",
    }
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;
    use mame_coalesce::app::{DiskAuditEntry, DiskAuditReport};

    use super::*;

    #[test]
    fn json_report_exposes_version_and_audit_state() -> mame_coalesce::Result<()> {
        let report = DiskAuditReport {
            schema_version: 1,
            catalog_key: "catalog".to_owned(),
            snapshot_key: "snapshot".to_owned(),
            source_path: Utf8PathBuf::from("/disks"),
            disks: vec![DiskAuditEntry {
                set_name: "machine".to_owned(),
                disk_name: "image".to_owned(),
                parent_disk: Some("parent".to_owned()),
                expected_logical_sha1: Some("11".repeat(20)),
                state: DiskAuditState::UnverifiedContainer,
            }],
        };

        let value = serde_json::to_value(&report)?;
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["disks"][0]["state"], "unverified_container");
        assert_eq!(value["disks"][0]["parent_disk"], "parent");
        assert!(value["disks"][0].get("observed_sha1").is_none());
        Ok(())
    }

    #[test]
    fn json_state_names_cover_all_disk_audit_outcomes() -> mame_coalesce::Result<()> {
        let states = [
            (DiskAuditState::Missing, "missing"),
            (DiskAuditState::UnknownDigestScope, "unknown_digest_scope"),
            (DiskAuditState::IdentityNotDeclared, "identity_not_declared"),
            (DiskAuditState::UnverifiedContainer, "unverified_container"),
            (
                DiskAuditState::UnsupportedContainer,
                "unsupported_container",
            ),
            (
                DiskAuditState::VerifiedLogicalIdentity,
                "verified_logical_identity",
            ),
            (
                DiskAuditState::LogicalIdentityMismatch,
                "logical_identity_mismatch",
            ),
        ];

        for (state, expected) in states {
            assert_eq!(serde_json::to_value(state)?, expected);
        }
        Ok(())
    }

    #[test]
    fn text_report_explains_that_a_chd_is_not_verified_by_presence() {
        let report = DiskAuditReport {
            schema_version: 1,
            catalog_key: "catalog".to_owned(),
            snapshot_key: "snapshot".to_owned(),
            source_path: Utf8PathBuf::from("/disks"),
            disks: vec![DiskAuditEntry {
                set_name: "machine".to_owned(),
                disk_name: "image".to_owned(),
                parent_disk: None,
                expected_logical_sha1: None,
                state: DiskAuditState::UnverifiedContainer,
            }],
        };

        assert!(render_disk_audit(&report).contains("logical identity unverified"));
    }
}
