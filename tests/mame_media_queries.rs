use camino::Utf8PathBuf;
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    catalog_files::{
        CatalogFileOccurrence, ContentOccurrenceLimit, DigestAlgorithm, MameBoolean,
        MameDiskCompatibility, MameDumpStatus, MameFilePayload, MameRomCompatibility,
        MameRomEvidenceScope, OccurrenceId, occurrences_for_content, occurrences_for_ids,
    },
    catalog_machines::{MachinePageLimit, machines_for_snapshot},
    database::Database,
    disk::DiskDigestScope,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey, SnapshotKey},
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
type FilesByName<'a> = std::collections::BTreeMap<&'a str, &'a CatalogFileOccurrence>;

#[test]
fn machine_asset_references_hydrate_typed_mame_declarations_without_xml() -> TestResult {
    let directory = tempfile::tempdir()?;
    let document_path = Utf8PathBuf::try_from(directory.path().join("mame.xml"))?;
    let original_xml = r#"<mame mameconfig="10">
  <machine name="media">
    <description>Media fixture</description>
    <rom name="standard.bin" size="3" crc="12345678" sha1="fedcba9876543210fedcba9876543210fedcba98"/>
    <rom name="valid.bin" size="0007" crc="AbCd1234" md5="00112233445566778899aabbccddeeff" sha1="0123456789abcdef0123456789abcdef01234567" offset="0x8000"/>
    <rom name="invalid.bin" size="many" crc="bad" md5="x" sha1="x" offset="bad"/>
    <rom name="empty.bin" size="" crc="" md5="" sha1="" offset=""/>
    <rom name="raw-only.bin" md5="aabbccddeeff00112233445566778899"/>
    <rom name="legacy.bin" size="0007" crc="AbCd1234" sha1="abcdef0123456789abcdef0123456789abcdef01" offset="0x8000" soundonly="no" dispose="yes" loadflag="LOAD16_BYTE" value="0x42" inverted="no" ovha="0x80" nothread="yes"/>
    <rom name="baddump.bin" status="baddump"/>
    <rom name="nodump.bin" status="nodump"/>
    <disk name="media.chd" sha1="0123456789abcdef0123456789abcdef01234567" region="cdrom" index="2" writable="no" writeable="yes"/>
  </machine>
</mame>"#;
    std::fs::write(&document_path, original_xml)?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&database_path)?;
    let snapshot = import_snapshot(&database, document_path.clone())?;

    // Removing the source proves the consumer gets declarations from native owner rows.
    std::fs::remove_file(document_path)?;
    assert_native_media_queries(&database, &snapshot)?;
    assert_media_history(&database, directory.path(), original_xml, &snapshot)?;
    Ok(())
}

fn import_snapshot(database: &Database, document_path: Utf8PathBuf) -> TestResult<SnapshotKey> {
    let report = app::import_catalog(
        database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::MameListXml,
            source_key: PublishingSourceKey::new("mame-media-query"),
            source_display_name: "MAME media query fixture".to_owned(),
            catalog_key: CatalogKey::new("mame-media-query"),
            catalog_display_name: "MAME media query fixture".to_owned(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    Ok(report.snapshot_key.ok_or("published snapshot missing")?)
}

fn assert_native_media_queries(database: &Database, snapshot: &SnapshotKey) -> TestResult {
    let page = machines_for_snapshot(database, snapshot, None, MachinePageLimit::new(5)?)?;
    let machine = page.machines.first().ok_or("machine page is empty")?;
    let ids = machine
        .assets
        .iter()
        .map(|asset| asset.occurrence_id)
        .collect::<Vec<_>>();
    let files = occurrences_for_ids(database, &ids)?;
    assert_eq!(files.len(), 9);
    assert!(files.iter().all(|file| file.mame_file.is_some()));
    assert!(occurrences_for_ids(database, &[OccurrenceId::from_database(9_876_543)])?.is_empty());

    let by_name = files
        .iter()
        .map(|file| {
            (
                file.provenance.asset_name.as_deref().unwrap_or_default(),
                file,
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_rom_identity(database, &by_name)?;
    assert_raw_rom_declarations(&by_name)?;
    assert_legacy_and_dump_evidence(&by_name)?;
    assert_disk_evidence(&by_name)?;
    Ok(())
}

fn assert_rom_identity(database: &Database, by_name: &FilesByName<'_>) -> TestResult {
    let standard = match by_name["standard.bin"]
        .mame_file
        .as_ref()
        .ok_or("standard ROM payload missing")?
    {
        MameFilePayload::Rom(rom) => rom,
        MameFilePayload::Disk(_) => return Err("ROM owner returned disk payload".into()),
    };
    assert_eq!(standard.evidence_scope, MameRomEvidenceScope::WholeFile);
    assert!(by_name["standard.bin"].content_id.is_some());

    let valid = match by_name["valid.bin"]
        .mame_file
        .as_ref()
        .ok_or("valid ROM payload missing")?
    {
        MameFilePayload::Rom(rom) => rom,
        MameFilePayload::Disk(_) => return Err("ROM owner returned disk payload".into()),
    };
    assert_eq!(valid.declarations.size_text.as_deref(), Some("0007"));
    assert_eq!(valid.size, Some(7));
    assert_eq!(valid.declarations.crc_text.as_deref(), Some("AbCd1234"));
    assert_eq!(
        valid.declarations.md5_text.as_deref(),
        Some("00112233445566778899aabbccddeeff")
    );
    assert_eq!(valid.declarations.offset_text.as_deref(), Some("0x8000"));
    assert_eq!(valid.evidence_scope, MameRomEvidenceScope::WholeFile);
    assert_eq!(valid.dump_status, MameDumpStatus::Good);
    assert_eq!(valid.compatibility, Some(MameRomCompatibility::default()));
    let content_id = by_name["valid.bin"]
        .content_id
        .ok_or("valid ROM did not get a content UUID")?;
    let filtered =
        occurrences_for_content(database, content_id, ContentOccurrenceLimit::new(10)?, None)?;
    assert!(!filtered.occurrences.is_empty());
    assert!(
        filtered
            .occurrences
            .iter()
            .all(|file| file.content_id == Some(content_id))
    );
    Ok(())
}

fn assert_raw_rom_declarations(by_name: &FilesByName<'_>) -> TestResult {
    let invalid = match by_name["invalid.bin"]
        .mame_file
        .as_ref()
        .ok_or("invalid ROM payload missing")?
    {
        MameFilePayload::Rom(rom) => rom,
        MameFilePayload::Disk(_) => return Err("ROM owner returned disk payload".into()),
    };
    assert_eq!(invalid.declarations.size_text.as_deref(), Some("many"));
    assert_eq!(invalid.declarations.crc_text.as_deref(), Some("bad"));
    assert_eq!(invalid.declarations.sha1_text.as_deref(), Some("x"));

    let empty = match by_name["empty.bin"]
        .mame_file
        .as_ref()
        .ok_or("empty ROM payload missing")?
    {
        MameFilePayload::Rom(rom) => rom,
        MameFilePayload::Disk(_) => return Err("ROM owner returned disk payload".into()),
    };
    assert_eq!(empty.declarations.size_text.as_deref(), Some(""));
    assert_eq!(empty.declarations.crc_text.as_deref(), Some(""));
    assert_eq!(empty.declarations.md5_text.as_deref(), Some(""));
    assert_eq!(empty.declarations.sha1_text.as_deref(), Some(""));
    assert_eq!(empty.declarations.offset_text.as_deref(), Some(""));

    let raw_only = match by_name["raw-only.bin"]
        .mame_file
        .as_ref()
        .ok_or("raw-only ROM payload missing")?
    {
        MameFilePayload::Rom(rom) => rom,
        MameFilePayload::Disk(_) => return Err("ROM owner returned disk payload".into()),
    };
    assert_eq!(
        raw_only.declarations.md5_text.as_deref(),
        Some("aabbccddeeff00112233445566778899")
    );
    assert_eq!(
        raw_only.compatibility,
        Some(MameRomCompatibility::default())
    );
    Ok(())
}

fn assert_legacy_and_dump_evidence(by_name: &FilesByName<'_>) -> TestResult {
    let legacy = match by_name["legacy.bin"]
        .mame_file
        .as_ref()
        .ok_or("legacy ROM payload missing")?
    {
        MameFilePayload::Rom(rom) => rom,
        MameFilePayload::Disk(_) => return Err("ROM owner returned disk payload".into()),
    };
    assert_eq!(legacy.evidence_scope, MameRomEvidenceScope::Unknown);
    assert!(by_name["legacy.bin"].content_id.is_none());
    let compatibility = legacy
        .compatibility
        .as_ref()
        .ok_or("legacy ROM compat owner missing")?;
    assert_eq!(compatibility.sound_only, Some(MameBoolean::No));
    assert_eq!(compatibility.dispose, Some(MameBoolean::Yes));
    assert_eq!(compatibility.load_flag.as_deref(), Some("LOAD16_BYTE"));
    assert_eq!(compatibility.value.as_deref(), Some("0x42"));
    assert_eq!(compatibility.inverted, Some(MameBoolean::No));
    assert_eq!(compatibility.ovha.as_deref(), Some("0x80"));
    assert_eq!(compatibility.no_thread, Some(MameBoolean::Yes));

    let bad_dump = match by_name["baddump.bin"]
        .mame_file
        .as_ref()
        .ok_or("baddump payload missing")?
    {
        MameFilePayload::Rom(rom) => rom,
        MameFilePayload::Disk(_) => return Err("ROM owner returned disk payload".into()),
    };
    assert_eq!(bad_dump.dump_status, MameDumpStatus::BadDump);
    let no_dump = match by_name["nodump.bin"]
        .mame_file
        .as_ref()
        .ok_or("nodump payload missing")?
    {
        MameFilePayload::Rom(rom) => rom,
        MameFilePayload::Disk(_) => return Err("ROM owner returned disk payload".into()),
    };
    assert_eq!(no_dump.dump_status, MameDumpStatus::NoDump);
    assert_eq!(no_dump.evidence_scope, MameRomEvidenceScope::Unknown);
    Ok(())
}

fn assert_disk_evidence(by_name: &FilesByName<'_>) -> TestResult {
    let disk = match by_name["media.chd"]
        .mame_file
        .as_ref()
        .ok_or("disk payload missing")?
    {
        MameFilePayload::Disk(disk) => disk,
        MameFilePayload::Rom(_) => return Err("disk owner returned ROM payload".into()),
    };
    assert_eq!(
        disk.declarations.sha1_text.as_deref(),
        Some("0123456789abcdef0123456789abcdef01234567")
    );
    assert_eq!(disk.evidence_scope, DiskDigestScope::ChdHeaderSha1);
    assert_eq!(disk.dump_status, MameDumpStatus::Good);
    assert_eq!(disk.region.as_deref(), Some("cdrom"));
    assert_eq!(disk.disk_index.as_deref(), Some("2"));
    assert_eq!(disk.writable, MameBoolean::No);
    assert!(disk.writable_specified);
    assert_eq!(
        disk.compatibility,
        Some(MameDiskCompatibility {
            writeable: MameBoolean::Yes
        })
    );
    assert!(by_name["media.chd"].content_id.is_none());
    assert!(by_name["media.chd"].digests.iter().any(|digest| {
        digest.algorithm == DigestAlgorithm::Sha1 && digest.scope == disk.evidence_scope.as_str()
    }));
    Ok(())
}

fn assert_media_history(
    database: &Database,
    directory: &std::path::Path,
    original_xml: &str,
    snapshot: &SnapshotKey,
) -> TestResult {
    let changed_document = Utf8PathBuf::try_from(directory.join("mame-updated.xml"))?;
    let changed_xml = original_xml
        .replace("size=\"0007\"", "size=\"8\"")
        .replace("crc=\"AbCd1234\"", "crc=\"aBcD1234\"")
        .replace(
            "md5=\"00112233445566778899aabbccddeeff\"",
            "md5=\"00112233445566778899AABBCCDDEEFF\"",
        )
        .replace(
            "sha1=\"0123456789abcdef0123456789abcdef01234567\"",
            "sha1=\"0123456789ABCDEF0123456789ABCDEF01234567\"",
        )
        .replace("offset=\"0x8000\"", "offset=\"0x9000\"");
    std::fs::write(&changed_document, &changed_xml)?;
    let changed_snapshot = import_snapshot(database, changed_document)?;
    assert_raw_declaration_history(database, snapshot, &changed_snapshot)?;

    let compatibility_document =
        Utf8PathBuf::try_from(directory.join("mame-compatibility-updated.xml"))?;
    let compatibility_xml = changed_xml
        .replace(
            "sha1=\"abcdef0123456789abcdef0123456789abcdef01\"",
            "sha1=\"bbcdef0123456789abcdef0123456789abcdef01\"",
        )
        .replace("soundonly=\"no\"", "soundonly=\"yes\"");
    std::fs::write(&compatibility_document, compatibility_xml)?;
    let compatibility_snapshot = import_snapshot(database, compatibility_document)?;
    assert_compatibility_history(database, &changed_snapshot, &compatibility_snapshot)?;
    Ok(())
}

fn assert_raw_declaration_history(
    database: &Database,
    snapshot: &SnapshotKey,
    changed_snapshot: &SnapshotKey,
) -> TestResult {
    let history = app::diff_catalog_snapshots(database, snapshot, changed_snapshot)?;
    let record = history
        .records
        .iter()
        .find(|record| record.set_name == "media")
        .ok_or("media history record missing")?;
    let change = record
        .requirement_changes
        .iter()
        .find(|change| change.asset_name == "valid.bin")
        .ok_or("raw declaration change missing from history")?;
    assert!(change.size_changed);
    assert!(!change.hash_changed);
    assert!(change.other_evidence_changed);
    let prior = requirement_fact(
        change
            .previous
            .as_ref()
            .ok_or("prior requirement missing")?,
    )?;
    let current = requirement_fact(
        change
            .current
            .as_ref()
            .ok_or("current requirement missing")?,
    )?;
    assert_eq!(prior["size"], 7);
    assert_eq!(current["size"], 8);
    assert_eq!(prior["crc"], "abcd1234");
    assert_eq!(current["crc"], "abcd1234");
    assert_eq!(prior["mame_attributes"]["size_text"], "0007");
    assert_eq!(current["mame_attributes"]["size_text"], "8");
    assert_eq!(prior["mame_attributes"]["crc_text"], "AbCd1234");
    assert_eq!(current["mame_attributes"]["crc_text"], "aBcD1234");
    assert_eq!(
        prior["mame_attributes"]["md5_text"],
        "00112233445566778899aabbccddeeff"
    );
    assert_eq!(
        current["mame_attributes"]["md5_text"],
        "00112233445566778899AABBCCDDEEFF"
    );
    assert_eq!(
        prior["mame_attributes"]["sha1_text"],
        "0123456789abcdef0123456789abcdef01234567"
    );
    assert_eq!(
        current["mame_attributes"]["sha1_text"],
        "0123456789ABCDEF0123456789ABCDEF01234567"
    );
    assert_eq!(prior["mame_attributes"]["offset_text"], "0x8000");
    assert_eq!(current["mame_attributes"]["offset_text"], "0x9000");
    Ok(())
}

fn assert_compatibility_history(
    database: &Database,
    changed_snapshot: &SnapshotKey,
    compatibility_snapshot: &SnapshotKey,
) -> TestResult {
    let compatibility_history =
        app::diff_catalog_snapshots(database, changed_snapshot, compatibility_snapshot)?;
    let legacy_change = compatibility_history
        .records
        .iter()
        .find(|record| record.set_name == "media")
        .and_then(|record| {
            record
                .requirement_changes
                .iter()
                .find(|change| change.asset_name == "legacy.bin")
        })
        .ok_or("legacy digest and compatibility change missing from history")?;
    assert!(!legacy_change.size_changed);
    assert!(legacy_change.hash_changed);
    assert!(legacy_change.other_evidence_changed);
    let legacy_prior = requirement_fact(
        legacy_change
            .previous
            .as_ref()
            .ok_or("prior legacy requirement missing")?,
    )?;
    let legacy_current = requirement_fact(
        legacy_change
            .current
            .as_ref()
            .ok_or("current legacy requirement missing")?,
    )?;
    assert_eq!(legacy_prior["mame_attributes"]["sound_only"], false);
    assert_eq!(legacy_current["mame_attributes"]["sound_only"], true);
    Ok(())
}

fn requirement_fact(value: &serde_json::Value) -> Result<&serde_json::Value, &'static str> {
    value
        .as_array()
        .and_then(|requirements| requirements.first())
        .ok_or("requirement history value was not a nonempty array")
}
