//! Integration coverage for the MAME native-query verifier.

use std::error::Error;

use camino::Utf8PathBuf;
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    catalog_files::{DigestAlgorithm, DigestProvenance, OccurrenceDigest},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey, SnapshotKey},
};

use super::{
    MachinePageLimit, NOT_COMPARED, VerificationReport, catalog_machines, digest_verify, mame,
    verify_mame_source,
};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const FAMILY_XML: &str = include_str!("../../fixtures/specifications/mame-machine-fields.xml");
const ALL_FIELDS_XML: &str = include_str!("../../fixtures/catalog/mame/all-fields.xml");

struct Published {
    _directory: tempfile::TempDir,
    database: Database,
    database_path: Utf8PathBuf,
    source_path: Utf8PathBuf,
    snapshot: SnapshotKey,
    source: Vec<u8>,
}

impl Published {
    fn new(xml: &str) -> TestResult<Self> {
        let directory = tempfile::tempdir()?;
        let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
        let source_path = Utf8PathBuf::try_from(directory.path().join("machine.xml"))?;
        let source = xml.as_bytes().to_vec();
        std::fs::write(&source_path, &source)?;
        let database = Database::open(&database_path)?;
        let report = app::import_catalog(
            &database,
            &CatalogImportRequest {
                document_path: source_path.clone(),
                format: CatalogDocumentFormat::MameListXml,
                source_key: PublishingSourceKey::new("mame-native-query-verifier"),
                source_display_name: "MAME native query verifier fixture".into(),
                catalog_key: CatalogKey::new("mame-native-query-verifier"),
                catalog_display_name: "MAME native query verifier fixture".into(),
                scope: CatalogScope::Complete,
            },
        )?;
        if report.status != CatalogImportStatus::Succeeded {
            return Err(format!("fixture import did not publish: {:?}", report.status).into());
        }
        let snapshot = report
            .snapshot_key
            .ok_or("published snapshot key missing")?;
        Ok(Self {
            _directory: directory,
            database,
            database_path,
            source_path,
            snapshot,
            source,
        })
    }

    fn verify(&self, bytes: &[u8]) -> TestResult<VerificationReport> {
        verify_mame_source(&self.database, &self.snapshot, bytes)
    }

    fn make_source_unavailable(&self) -> TestResult {
        std::fs::remove_file(&self.source_path)?;
        let documents = format!("{}.documents", self.database_path);
        std::fs::remove_dir_all(documents)?;
        Ok(())
    }
}

fn minimal_document(machines: &str, header_attributes: &str) -> String {
    format!("<mame mameconfig='10' {header_attributes}>{machines}</mame>")
}

fn machine(name: &str) -> String {
    format!("<machine name='{name}'><description>{name}</description></machine>")
}

fn machines_xml(count: usize) -> String {
    let machines = (0..count)
        .map(|index| machine(&format!("m{index:04}")))
        .collect::<String>();
    minimal_document(&machines, "")
}

fn assert_source_mismatch(stored: &str, changed_source: &str) -> TestResult {
    let published = Published::new(stored)?;
    published.verify(stored.as_bytes())?;
    let error = published
        .verify(changed_source.as_bytes())
        .err()
        .ok_or("changed source unexpectedly matched the published catalog")?;
    assert!(
        error.to_string().contains("mismatch")
            || error.to_string().contains("extra source")
            || error.to_string().contains("extra machine")
            || error.to_string().contains("no machine at source order"),
        "unexpected failure for changed source {changed_source}: {error}"
    );
    Ok(())
}

#[test]
fn fixture_wide_typed_machine_and_media_facts_match_after_original_is_unavailable() -> TestResult {
    for fixture in [FAMILY_XML, ALL_FIELDS_XML] {
        let published = Published::new(fixture)?;
        published.make_source_unavailable()?;
        let report = published.verify(&published.source)?;
        assert!(report.machines > 0);
        assert_eq!(report.not_compared, NOT_COMPARED);
        // Existing pinned dictionary separately establishes 125 native and ten
        // compatibility field names. This verifier compares each typed position
        // returned for those fixture owners against the source parser's positions.
    }
    Ok(())
}

#[test]
fn source_mutations_detect_header_machine_default_order_and_raw_media_changes() -> TestResult {
    let base = "<mame mameconfig='10' build='b1'><machine name='sys' sourcefile='sys.cpp'><description>System</description><rom name='r.bin' size='0010' crc='bad' soundonly='no'/><disk name='d.chd' sha1='bad' writeable='yes'/><sample name='hit'/></machine></mame>";
    let cases = [
        ("build='b1'", "build='b2'"),
        ("mameconfig='10'", "mameconfig='11'"),
        ("sourcefile='sys.cpp'", "sourcefile='changed.cpp'"),
        (
            "name='sys' sourcefile='sys.cpp'",
            "sourcefile='sys.cpp' name='sys'",
        ),
        ("name='r.bin' size='0010'", "name='r.bin' size='0011'"),
        ("crc='bad'", "crc='BAD'"),
        ("soundonly='no'", "soundonly='yes'"),
        ("sha1='bad'", "sha1=''"),
        ("writeable='yes'", "writeable='no'"),
        ("<sample name='hit'/>", "<sample name='miss'/>"),
    ];
    for (before, after) in cases {
        let changed = base.replacen(before, after, 1);
        assert_ne!(changed, base, "mutation anchor {before:?} was absent");
        assert_source_mismatch(base, &changed)?;
    }

    let omitted_default = "<mame mameconfig='10'><machine name='sys'><description>System</description><rom name='r' size='1'/></machine></mame>";
    let explicit_default = omitted_default.replace("name='sys'", "name='sys' runnable='yes'");
    assert_source_mismatch(omitted_default, &explicit_default)?;

    let valid_digests = "<mame mameconfig='10'><machine name='sys'><description>System</description><rom name='r' size='16' crc='0123abcd' md5='0123456789abcdef0123456789abcdef' sha1='0123456789abcdef0123456789abcdef01234567'/></machine></mame>";
    let published = Published::new(valid_digests)?;
    published.verify(valid_digests.as_bytes())?;
    let changed_crc = valid_digests.replace("crc='0123abcd'", "crc='0123abce'");
    assert!(published.verify(changed_crc.as_bytes()).is_err());
    Ok(())
}

#[test]
fn decoded_mame_digests_require_exact_source_declared_assertions() -> TestResult {
    let expected_bytes = [0x01, 0x23, 0xab, 0xcd];
    let expected = [(
        DigestAlgorithm::Crc32,
        expected_bytes.as_slice(),
        "whole_asset",
    )];
    let digest = |value: &[u8], scope: &str, provenance| OccurrenceDigest {
        algorithm: DigestAlgorithm::Crc32,
        value: value.to_vec(),
        scope: scope.into(),
        provenance,
    };

    let matching = [digest(
        &expected_bytes,
        "whole_asset",
        DigestProvenance::SourceDeclared,
    )];
    digest_verify::compare_declared_digests("machine.assets[0].digests", expected, &matching)?;

    let wrong_bytes = [0x01, 0x23, 0xab, 0xce];
    let corruptions = [
        Vec::new(),
        vec![digest(
            &expected_bytes,
            "unknown",
            DigestProvenance::SourceDeclared,
        )],
        vec![digest(
            &wrong_bytes,
            "whole_asset",
            DigestProvenance::SourceDeclared,
        )],
        vec![
            digest(
                &expected_bytes,
                "whole_asset",
                DigestProvenance::SourceDeclared,
            ),
            digest(
                &expected_bytes,
                "whole_asset",
                DigestProvenance::SourceDeclared,
            ),
        ],
        vec![digest(
            &expected_bytes,
            "whole_asset",
            DigestProvenance::Computed,
        )],
        vec![digest(
            &expected_bytes,
            "whole_asset",
            DigestProvenance::Unknown,
        )],
    ];
    for actual in corruptions {
        assert!(digest_verify::compare_declared_digests(
            "machine.assets[0].digests",
            expected,
            &actual,
        )
        .is_err());
    }
    Ok(())
}

#[test]
fn skipped_extension_count_includes_rom_and_disk_owners() -> TestResult {
    let cases = [
        (
            "<mame mameconfig='10'><machine name='g'><description>G</description><rom name='r' size='1' vendor='x'/></machine></mame>",
            1,
        ),
        (
            "<mame mameconfig='10' vendor='root'><vendor_root/><machine name='g' vendor='machine'><description>G</description><rom name='r' size='1' vendor='rom'><vendor_rom/></rom><disk name='d' vendor='disk'><vendor_disk/></disk></machine></mame>",
            7,
        ),
    ];
    for (xml, expected) in cases {
        let published = Published::new(xml)?;
        let report = published.verify(&published.source)?;
        assert_eq!(
            report.source_extensions_not_compared, expected,
            "missing skipped owners: {xml}"
        );
    }
    Ok(())
}

#[test]
fn mixed_media_reordering_is_not_hidden_by_payload_batching() -> TestResult {
    let stored = "<mame mameconfig='10'><machine name='sys'><description>System</description><rom name='r' size='1'/><disk name='d'/><sample name='s'/></machine></mame>";
    let reordered = "<mame mameconfig='10'><machine name='sys'><description>System</description><sample name='s'/><rom name='r' size='1'/><disk name='d'/></machine></mame>";
    assert_source_mismatch(stored, reordered)
}

#[test]
fn exact_and_over_page_boundary_roundtrip_and_catalog_tails_are_checked() -> TestResult {
    for count in [1, 63, 64, 65, 128, 129] {
        let xml = machines_xml(count);
        let published = Published::new(&xml)?;
        let report = published.verify(&published.source)?;
        assert_eq!(report.machines, count);
        let first = catalog_machines::machines_for_snapshot(
            &published.database,
            &published.snapshot,
            None,
            MachinePageLimit::new(64)?,
        )?;
        assert!(first.machines.len() <= 64);
        assert_eq!(first.next_cursor.is_some(), count > 64);
    }

    let stored = machines_xml(65);
    assert_source_mismatch(&stored, &machines_xml(64))?;
    assert_source_mismatch(&machines_xml(64), &stored)?;
    Ok(())
}

#[test]
fn zero_source_machines_and_late_eof_are_errors_not_verified_prefixes() -> TestResult {
    let zero = b"<mame mameconfig='10'></mame>";
    assert!(mame::read_with::<_, Box<dyn Error>>(zero, |_| Ok(()), |_, _| Ok(())).is_err());

    let valid = minimal_document(&machine("first"), "");
    let published = Published::new(&valid)?;
    let late_eof = format!("{}<", valid.trim_end_matches("</mame>"));
    assert!(published.verify(late_eof.as_bytes()).is_err());
    Ok(())
}
