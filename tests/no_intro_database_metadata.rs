use camino::Utf8PathBuf;
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    catalog_files::{
        self, CatalogFileOccurrence, NoIntroDatabaseDigestValue, NoIntroDatabaseFilePayload,
        NoIntroDatabaseReleaseFile, NoIntroDatabaseSourceFile,
    },
    catalog_no_intro_database::{
        self, NoIntroDatabaseArchive, NoIntroDatabaseDumpSource, NoIntroDatabaseGame,
        NoIntroDatabaseGameChild, NoIntroDatabasePage, NoIntroDatabasePageLimit,
        NoIntroDatabaseRelease,
    },
    database::Database,
    domain::{CatalogKey, CatalogScope, OccurrenceId, PublishingSourceKey},
    no_intro_db_xml::{ArchiveClone, DeclaredText, NoIntroDatabaseMode},
};
use std::fmt::Debug;

#[path = "support/no_intro_database_fixture.rs"]
mod no_intro_database_fixture;

use no_intro_database_fixture::{
    ARCHIVE_FIELDS, DUMP_DETAILS_FIELDS, DUMP_DETAILS_FIELDS_B, DUMP_FILE_FIELDS,
    DUMP_SERIAL_FIELDS, DUMP_SERIAL_FIELDS_B, HASHES_A, HASHES_B, HASHES_RELEASE,
    RELEASE_DETAILS_FIELDS, RELEASE_FILE_FIELDS, RELEASE_SERIAL_FIELDS, all_fields_xml,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

struct PositionExpectation {
    owner: String,
    field: String,
    marker: String,
    marker_occurrence: usize,
}

fn field_values_from_declared_fields(
    actual: &[(&str, Option<&str>)],
    fields: &[&str],
    prefix: &str,
) {
    for field in fields {
        let value = actual
            .iter()
            .find_map(|(name, value)| (*name == *field).then_some(*value))
            .flatten();
        assert_eq!(
            value,
            Some(format!("{prefix}:{field}").as_str()),
            "field {field}"
        );
    }
}

fn field_is_empty(actual: &[(&str, Option<&str>)], field: &str) {
    assert_eq!(
        actual
            .iter()
            .find_map(|(name, value)| (*name == field).then_some(*value)),
        Some(Some("")),
        "{field} is explicitly empty"
    );
}

fn field_is_absent(actual: &[(&str, Option<&str>)], field: &str) {
    assert_eq!(
        actual
            .iter()
            .find_map(|(name, value)| (*name == field).then_some(*value)),
        Some(None),
        "{field} is absent"
    );
}

macro_rules! declared_values {
    ($owner:expr; $($field:ident),+ $(,)?) => {
        [$(
            (stringify!($field), $owner.$field.as_ref().map(DeclaredText::as_str)),
        )+]
    };
}

macro_rules! string_values {
    ($owner:expr; $($field:ident),+ $(,)?) => {
        [$(
            (stringify!($field), $owner.$field.as_deref()),
        )+]
    };
}

fn variant_name(field: &str) -> String {
    match field {
        "archivename" => "ArchiveName".to_owned(),
        "d_date" => "DumpDate".to_owned(),
        "d_date_info" => "DumpDateInfo".to_owned(),
        "dirname" => "Directory".to_owned(),
        "forcename" => "ForceName".to_owned(),
        "forcescenename" => "ForceSceneName".to_owned(),
        "gameid1" => "GameId1".to_owned(),
        "gameid2" => "GameId2".to_owned(),
        "mergeof" => "MergeOf".to_owned(),
        "nfocrc" => "NfoCrc".to_owned(),
        "nfoname" => "NfoName".to_owned(),
        "nfosize" => "NfoLegacySize".to_owned(),
        "nodump" => "NoDump".to_owned(),
        "originalformat" => "OriginalFormat".to_owned(),
        "r_date" => "ReleaseDate".to_owned(),
        "r_date_info" => "ReleaseDateInfo".to_owned(),
        "rominfo" => "RomInfo".to_owned(),
        "mediastamp" => "MediaStamp".to_owned(),
        "pcb_serial" => "PcbSerial".to_owned(),
        "romchip_serial1" => "RomChipSerial1".to_owned(),
        "romchip_serial2" => "RomChipSerial2".to_owned(),
        "savechip_serial" => "SaveChipSerial".to_owned(),
        _ => field
            .split('_')
            .map(|part| {
                let mut characters = part.chars();
                characters
                    .next()
                    .map(|first| first.to_uppercase().collect::<String>() + characters.as_str())
                    .unwrap_or_default()
            })
            .collect(),
    }
}

fn position(
    family: &str,
    field: &str,
    value: &str,
    marker_occurrence: usize,
) -> PositionExpectation {
    PositionExpectation {
        owner: family.to_owned(),
        field: variant_name(field),
        marker: format!("{field}='{value}'"),
        marker_occurrence,
    }
}

fn prefixed_positions(family: &str, fields: &[&str], prefix: &str) -> Vec<PositionExpectation> {
    fields
        .iter()
        .map(|field| position(family, field, &format!("{prefix}:{field}"), 0))
        .collect()
}

fn qname_location(xml: &str, marker: &str, occurrence: usize) -> TestResult<(i64, i64)> {
    let offset = xml
        .match_indices(marker)
        .nth(occurrence)
        .map(|(offset, _)| offset)
        .ok_or_else(|| format!("fixture marker {marker:?} occurrence {occurrence} is missing"))?;
    let prefix = &xml[..offset];
    let line = i64::try_from(prefix.bytes().filter(|byte| *byte == b'\n').count() + 1)?;
    let column = i64::try_from(
        prefix
            .rsplit('\n')
            .next()
            .ok_or("fixture prefix has no final line")?
            .chars()
            .count()
            + 1,
    )?;
    Ok((line, column))
}

fn assert_positions<Field: Debug>(
    xml: &str,
    actual: &[mame_coalesce::logiqx::AttributePosition<Field>],
    expected: &[PositionExpectation],
) -> TestResult {
    assert_eq!(actual.len(), expected.len());
    for (source_order, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(
            format!("{:?}", actual.field),
            expected.field,
            "{} attribute field",
            expected.owner
        );
        assert_eq!(actual.source_order, source_order);
        assert_eq!(
            (actual.location.line, actual.location.column),
            qname_location(xml, &expected.marker, expected.marker_occurrence)?,
            "QName position for {}.{}",
            expected.owner,
            expected.field
        );
    }
    Ok(())
}

fn digest_value(literal: &str) -> TestResult<NoIntroDatabaseDigestValue> {
    let bytes = literal.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.as_chunks::<2>().0 {
        let pair = std::str::from_utf8(pair)?;
        decoded.push(u8::from_str_radix(pair, 16)?);
    }
    Ok(NoIntroDatabaseDigestValue::Valid(decoded))
}

fn assert_archive_values(archive: &NoIntroDatabaseArchive, xml: &str) -> TestResult<usize> {
    let values = declared_values!(archive.description;
        additional, adult, aftermarket, alt, bios, categories, complete, dat, datter_note,
        description, devstatus, gameid1, gameid2, langchecked, languages, licensed, listed,
        mergename, name, name_alt, number, physical, region, regparent, showlang, special1,
        special2, sticky_note, version1, version2, mergeof,
    );
    assert_eq!(values.len(), ARCHIVE_FIELDS.len() + 2);
    for field in ARCHIVE_FIELDS {
        let expected = if *field == "additional" {
            "archive-a:additional".to_owned()
        } else {
            format!("archive-a:{field}")
        };
        assert_eq!(
            values
                .iter()
                .find_map(|(name, value)| (*name == *field).then_some(*value)),
            Some(Some(expected.as_str())),
            "archive field {field}"
        );
    }
    field_is_empty(&values, "adult");
    assert_eq!(
        archive
            .description
            .mergeof
            .as_ref()
            .map(DeclaredText::as_str),
        Some("opaque-merge-declaration")
    );
    let clone = archive
        .description
        .clone
        .as_ref()
        .ok_or("archive clone attribute is missing")?;
    assert!(matches!(clone, ArchiveClone::ParentMarker(value) if value.as_str() == "P"));

    let mut expected = Vec::with_capacity(32);
    for field in ARCHIVE_FIELDS {
        expected.push(position("archive", field, &format!("archive-a:{field}"), 0));
        if *field == "additional" {
            expected.push(position("archive", "adult", "", 0));
        }
    }
    expected.push(position("archive", "clone", "P", 0));
    expected.push(position(
        "archive",
        "mergeof",
        "opaque-merge-declaration",
        0,
    ));
    assert_positions(xml, &archive.attribute_positions, &expected)?;
    assert!(archive.clone_relationship_id.is_none());
    assert!(archive.merge_relationship_id.is_some());
    Ok(expected.len())
}

fn assert_dump_source_values(
    source: &NoIntroDatabaseDumpSource,
    prefix: &str,
    xml: &str,
) -> TestResult<usize> {
    let serial_prefix = prefix.replace("dump-details", "dump-serials");
    let details = source.details.as_ref().ok_or("dump details are missing")?;
    let detail_values = declared_values!(details;
        comment1, comment2, d_date, d_date_info, dumper, id, link1, link2, link3, media_title,
        nodump, origin, originalformat, project, r_date, r_date_info, region, rominfo, section,
        tool,
    );
    if prefix == "dump-details-a" {
        field_values_from_declared_fields(&detail_values, DUMP_DETAILS_FIELDS, prefix);
    } else {
        field_values_from_declared_fields(&detail_values, DUMP_DETAILS_FIELDS_B, prefix);
        field_is_empty(&detail_values, "id");
        field_is_absent(&detail_values, "d_date");
    }

    let serials = source.serials.as_ref().ok_or("dump serials are missing")?;
    let serial_values = declared_values!(serials;
        box_barcode, box_serial, chip_serial, digital_serial1, digital_serial2, lockout_serial,
        media_serial1, media_serial2, media_serial3, mediastamp, pcb_serial, romchip_serial1,
        romchip_serial2, savechip_serial,
    );
    if serial_prefix == "dump-serials-a" {
        field_values_from_declared_fields(&serial_values, DUMP_SERIAL_FIELDS, &serial_prefix);
    } else {
        field_values_from_declared_fields(&serial_values, DUMP_SERIAL_FIELDS_B, &serial_prefix);
        field_is_empty(&serial_values, "box_serial");
        field_is_absent(&serial_values, "savechip_serial");
    }

    let details_positions = if prefix == "dump-details-a" {
        prefixed_positions("dump-details", DUMP_DETAILS_FIELDS, prefix)
    } else {
        let fields = DUMP_DETAILS_FIELDS
            .iter()
            .copied()
            .filter(|field| *field != "d_date")
            .collect::<Vec<_>>();
        let mut positions = prefixed_positions("dump-details", &fields, prefix);
        if let Some(id) = positions.iter_mut().find(|position| position.field == "Id") {
            "id=''".clone_into(&mut id.marker);
        }
        positions
    };
    let serial_fields = if prefix == "dump-details-a" {
        DUMP_SERIAL_FIELDS.to_vec()
    } else {
        DUMP_SERIAL_FIELDS
            .iter()
            .copied()
            .filter(|field| *field != "savechip_serial")
            .collect::<Vec<_>>()
    };
    let mut serial_positions = prefixed_positions("dump-serials", &serial_fields, &serial_prefix);
    if prefix != "dump-details-a"
        && let Some(box_serial) = serial_positions
            .iter_mut()
            .find(|position| position.field == "BoxSerial")
    {
        "box_serial=''".clone_into(&mut box_serial.marker);
    }
    let details_count = details_positions.len();
    assert_positions(xml, &source.details_attribute_positions, &details_positions)?;
    assert_positions(xml, &source.serials_attribute_positions, &serial_positions)?;

    Ok(if prefix == "dump-details-a" {
        DUMP_DETAILS_FIELDS.len() + DUMP_SERIAL_FIELDS.len()
    } else {
        details_count + serial_positions.len()
    })
}

fn assert_release_values(release: &NoIntroDatabaseRelease, xml: &str) -> TestResult<usize> {
    let details = release
        .details
        .as_ref()
        .ok_or("release details are missing")?;
    let detail_values = string_values!(details;
        archivename, category, comment, date, dirname, group, id, nfo_size, nfoname, nfosize,
        origin, originalformat, region, rominfo, tool,
    );
    field_values_from_declared_fields(&detail_values, RELEASE_DETAILS_FIELDS, "release-details-a");
    assert_eq!(details.nfo_crc32, Some(digest_value("CcDdEeFf")?));
    assert_eq!(
        details.nfocrc,
        Some(NoIntroDatabaseDigestValue::Invalid("not-a-hash".to_owned()))
    );

    let serials = release
        .serials
        .as_ref()
        .ok_or("release serials are missing")?;
    let serial_values = declared_values!(serials;
        box_barcode, box_serial, media_serial1, mediastamp, pcb_serial, romchip_serial1,
    );
    field_values_from_declared_fields(&serial_values, RELEASE_SERIAL_FIELDS, "release-serials-a");

    let mut details_positions = prefixed_positions(
        "release-details",
        RELEASE_DETAILS_FIELDS,
        "release-details-a",
    );
    details_positions.push(position("release-details", "nfo_crc32", "CcDdEeFf", 0));
    details_positions.push(position("release-details", "nfocrc", "not-a-hash", 0));
    assert_positions(xml, &details.attribute_positions, &details_positions)?;
    let serial_positions = prefixed_positions(
        "release-serials",
        RELEASE_SERIAL_FIELDS,
        "release-serials-a",
    );
    assert_positions(xml, &release.serials_attribute_positions, &serial_positions)?;
    Ok(details_positions.len() + serial_positions.len())
}

fn assert_source_file_values(
    source: &NoIntroDatabaseSourceFile,
    prefix: &str,
    hashes: &[&str],
    numeric_size: i64,
) -> TestResult {
    let values = string_values!(source;
        bad, date, extension, filter, forcename, forcescenename, format, header, item, mia,
        note, origin_size, serial, unique, update_type, version,
    );
    field_values_from_declared_fields(&values, DUMP_FILE_FIELDS, prefix);
    assert_eq!(source.id.as_deref(), Some("same-publisher-id"));
    assert_eq!(
        source.source_size.as_deref(),
        Some(if numeric_size == 3 { "0003" } else { "0005" })
    );
    assert_eq!(source.size, Some(numeric_size));
    assert_eq!(
        source.evidence_scope,
        catalog_files::NoIntroDatabaseEvidenceScope::Unknown
    );
    assert_eq!(source.digests.crc32, Some(digest_value(hashes[0])?));
    assert_eq!(source.digests.md5, Some(digest_value(hashes[1])?));
    assert_eq!(source.digests.sha1, Some(digest_value(hashes[2])?));
    assert_eq!(source.digests.sha256, Some(digest_value(hashes[3])?));
    if numeric_size == 3 {
        assert_eq!(source.origin_sha256, Some(digest_value(hashes[4])?));
    } else {
        assert_eq!(
            source.origin_sha256,
            Some(NoIntroDatabaseDigestValue::Invalid(
                "not-a-digest".to_owned()
            ))
        );
    }
    Ok(())
}

fn assert_release_file_values(file: &NoIntroDatabaseReleaseFile) -> TestResult {
    let values = string_values!(file;
        bad, extension, forcename, forcescenename, format, header, id, item, note, serial,
        update_type, version,
    );
    field_values_from_declared_fields(&values, RELEASE_FILE_FIELDS, "release-file-a");
    assert_eq!(file.source_size.as_deref(), Some("0004"));
    assert_eq!(file.size, Some(4));
    assert_eq!(
        file.evidence_scope,
        catalog_files::NoIntroDatabaseEvidenceScope::Unknown
    );
    assert_eq!(file.digests.crc32, Some(digest_value("EeFf0011")?));
    assert_eq!(file.digests.md5, Some(digest_value(HASHES_RELEASE[0])?));
    assert_eq!(file.digests.sha1, Some(digest_value(HASHES_RELEASE[1])?));
    assert_eq!(file.digests.sha256, Some(digest_value(HASHES_RELEASE[2])?));
    Ok(())
}

fn occurrence(
    files: &[CatalogFileOccurrence],
    id: OccurrenceId,
) -> TestResult<&CatalogFileOccurrence> {
    files
        .iter()
        .find(|file| file.occurrence_id == id)
        .ok_or_else(|| format!("catalog file occurrence {} is missing", id.database_value()).into())
}

fn assert_source_file_payload(
    files: &[CatalogFileOccurrence],
    source: &NoIntroDatabaseDumpSource,
    file_index: usize,
    prefix: &str,
    hashes: &[&str; 5],
    size: i64,
) -> TestResult {
    let file_ref = source
        .files
        .get(file_index)
        .ok_or("source file reference is missing")?;
    let occurrence = occurrence(files, file_ref.occurrence_id)?;
    assert!(occurrence.content_id.is_none());
    let payload = occurrence
        .no_intro_database_file
        .as_ref()
        .ok_or("source file payload is missing")?;
    let NoIntroDatabaseFilePayload::Source(payload) = payload else {
        return Err("source file reference resolved to a non-source payload".into());
    };
    assert_eq!(payload.dump_source_id, source.id);
    assert_eq!(payload.source_order, file_ref.source_order);
    assert_eq!(
        occurrence.provenance.occurrence_order,
        file_ref.occurrence_order
    );
    assert_source_file_values(payload, prefix, hashes, size)
}

fn assert_release_file_payload(
    files: &[CatalogFileOccurrence],
    release: &NoIntroDatabaseRelease,
) -> TestResult {
    let file_ref = release
        .files
        .first()
        .ok_or("release file reference is missing")?;
    let occurrence = occurrence(files, file_ref.occurrence_id)?;
    assert!(occurrence.content_id.is_none());
    let payload = occurrence
        .no_intro_database_file
        .as_ref()
        .ok_or("release file payload is missing")?;
    let NoIntroDatabaseFilePayload::Release(payload) = payload else {
        return Err("release file reference resolved to a non-release payload".into());
    };
    assert_eq!(payload.release_id, release.id);
    assert_eq!(payload.source_order, file_ref.source_order);
    assert_eq!(
        occurrence.provenance.occurrence_order,
        file_ref.occurrence_order
    );
    assert_release_file_values(payload)
}

fn assert_file_positions<Field: Debug>(
    xml: &str,
    actual: &[mame_coalesce::logiqx::AttributePosition<Field>],
    expected: &[PositionExpectation],
) -> TestResult {
    assert_positions(xml, actual, expected)
}

fn assert_game_child_order(game: &NoIntroDatabaseGame, expected: &[i64]) {
    assert_eq!(
        game.children
            .iter()
            .map(NoIntroDatabaseGameChild::source_order)
            .collect::<Vec<_>>(),
        expected
    );
}

fn read_fixture_without_sources(
    directory: &tempfile::TempDir,
    xml: &str,
) -> TestResult<(Database, NoIntroDatabasePage)> {
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let document_path = Utf8PathBuf::try_from(directory.path().join("export.xml"))?;
    std::fs::write(&document_path, xml)?;
    let database = Database::open(&database_path)?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path: document_path.clone(),
            format: CatalogDocumentFormat::NoIntroDatabase(NoIntroDatabaseMode::ObservedCompatible),
            source_key: PublishingSourceKey::new("no-intro-metadata-witness"),
            source_display_name: "No-Intro metadata field witness".to_owned(),
            catalog_key: CatalogKey::new("no-intro-metadata-witness"),
            catalog_display_name: "No-Intro metadata field witness".to_owned(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, CatalogImportStatus::Succeeded);
    let snapshot = report
        .snapshot_key
        .ok_or("successful import has no snapshot")?;
    let unavailable_original = directory.path().join("unavailable-original.xml");
    std::fs::rename(&document_path, unavailable_original)?;
    let retained_objects = Utf8PathBuf::from(format!("{}.documents", database_path.as_str()));
    let unavailable_objects = directory.path().join("unavailable-objects");
    assert!(retained_objects.is_dir());
    std::fs::rename(&retained_objects, unavailable_objects)?;
    assert!(app::load_snapshot_source(&database, &snapshot).is_err());

    let page = catalog_no_intro_database::games_for_snapshot(
        &database,
        &snapshot,
        None,
        NoIntroDatabasePageLimit::new(2)?,
    )?;
    Ok((database, page))
}

fn game_dump_source<'a>(
    game: &'a NoIntroDatabaseGame,
    missing_message: &str,
) -> TestResult<&'a NoIntroDatabaseDumpSource> {
    game.children
        .iter()
        .find_map(|child| match child {
            NoIntroDatabaseGameChild::DumpSource(value) => Some(value),
            _ => None,
        })
        .ok_or_else(|| missing_message.into())
}

fn assert_empty_release(release: &NoIntroDatabaseRelease, xml: &str) -> TestResult {
    assert_eq!(
        release
            .details
            .as_ref()
            .and_then(|details| details.comment.as_deref()),
        Some("")
    );
    assert_eq!(
        release
            .serials
            .as_ref()
            .and_then(|serials| serials.media_serial1.as_ref())
            .map(DeclaredText::as_str),
        Some("")
    );
    assert!(
        release
            .details
            .as_ref()
            .is_some_and(|details| details.nfo_crc32.is_none() && details.nfocrc.is_none())
    );

    let release_details = release
        .details
        .as_ref()
        .ok_or("second release details are missing")?;
    assert_positions(
        xml,
        &release_details.attribute_positions,
        &[position("release-details", "comment", "", 0)],
    )?;
    release
        .serials
        .as_ref()
        .ok_or("second release serials are missing")?;
    assert_positions(
        xml,
        &release.serials_attribute_positions,
        &[position("release-serials", "media_serial1", "", 0)],
    )?;

    Ok(())
}

fn assert_second_archive_clone(archive: &NoIntroDatabaseArchive) -> TestResult {
    let second_clone = archive
        .description
        .clone
        .as_ref()
        .ok_or("second archive's clone literal is missing")?;
    assert!(
        matches!(second_clone, ArchiveClone::OtherValue(value) if value.as_str() == "ambiguous-parent-number")
    );
    assert!(archive.clone_relationship_id.is_some());
    assert!(archive.merge_relationship_id.is_none());
    assert_eq!(
        archive.description.name.as_ref().map(DeclaredText::as_str),
        Some("second-archive")
    );

    Ok(())
}

fn assert_first_game_metadata(first: &NoIntroDatabaseGame, xml: &str) -> TestResult<usize> {
    let first_archives = first
        .children
        .iter()
        .filter_map(|child| match child {
            NoIntroDatabaseGameChild::Archive(value) => Some(value),
            _ => None,
        })
        .collect::<Vec<_>>();
    let first_source = game_dump_source(first, "first game's dump source is missing")?;
    let releases = first
        .children
        .iter()
        .filter_map(|child| match child {
            NoIntroDatabaseGameChild::Release(value) => Some(value),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(first_archives.len(), 2);
    assert_eq!(releases.len(), 2);
    let [first_archive, second_archive] = first_archives.as_slice() else {
        return Err("first game must have two archives".into());
    };
    let [first_release, second_release] = releases.as_slice() else {
        return Err("first game must have two releases".into());
    };

    let mut position_total = 0;
    position_total += assert_archive_values(first_archive, xml)?;
    assert_eq!(
        first_archive.clone_relationship_id, None,
        "P marker has no invented parent relationship"
    );
    position_total += assert_dump_source_values(first_source, "dump-details-a", xml)?;
    position_total += assert_release_values(first_release, xml)?;
    assert_empty_release(second_release, xml)?;
    assert_second_archive_clone(second_archive)?;
    Ok(position_total)
}

fn assert_second_game_metadata(second: &NoIntroDatabaseGame, xml: &str) -> TestResult {
    let second_archive = second
        .children
        .iter()
        .find_map(|child| match child {
            NoIntroDatabaseGameChild::Archive(value) => Some(value),
            _ => None,
        })
        .ok_or("second game's archive is missing")?;
    assert!(second_archive.description.clone.is_none());
    assert!(second_archive.clone_relationship_id.is_none());
    assert!(second_archive.merge_relationship_id.is_none());
    assert_eq!(
        second_archive
            .description
            .number
            .as_ref()
            .map(DeclaredText::as_str),
        Some("same-publisher-id")
    );
    let second_source = game_dump_source(second, "second game's dump source is missing")?;
    let _ = assert_dump_source_values(second_source, "dump-details-b", xml)?;
    Ok(())
}

fn assert_file_payloads(
    database: &Database,
    file_ids: &[OccurrenceId; 3],
    first_source: &NoIntroDatabaseDumpSource,
    release: &NoIntroDatabaseRelease,
    second_source: &NoIntroDatabaseDumpSource,
) -> TestResult {
    assert_ne!(file_ids[0], file_ids[2]);
    let file_payloads = catalog_files::occurrences_for_ids(database, file_ids)?;
    assert_eq!(file_payloads.len(), 3);
    assert_source_file_payload(&file_payloads, first_source, 0, "dump-file-a", &HASHES_A, 3)?;
    assert_release_file_payload(&file_payloads, release)?;
    let source_b_hashes = [
        HASHES_B[0],
        HASHES_B[1],
        HASHES_B[2],
        HASHES_B[3],
        "not-a-digest",
    ];
    assert_source_file_payload(
        &file_payloads,
        second_source,
        0,
        "dump-file-b",
        &source_b_hashes,
        5,
    )?;

    Ok(())
}

fn assert_source_file_positions(
    first_source: &NoIntroDatabaseDumpSource,
    second_source: &NoIntroDatabaseDumpSource,
    xml: &str,
) -> TestResult<usize> {
    let first_source_file = first_source
        .files
        .first()
        .ok_or("first source file reference is missing")?;
    let second_source_file = second_source
        .files
        .first()
        .ok_or("second source file reference is missing")?;
    let mut primary_file_positions =
        prefixed_positions("dump-file", DUMP_FILE_FIELDS, "dump-file-a");
    primary_file_positions.extend([
        position("dump-file", "id", "same-publisher-id", 0),
        position("dump-file", "size", "0003", 0),
        position("dump-file", "crc32", HASHES_A[0], 0),
        position("dump-file", "md5", HASHES_A[1], 0),
        position("dump-file", "sha1", HASHES_A[2], 0),
        position("dump-file", "sha256", HASHES_A[3], 0),
        position("dump-file", "origin_sha256", HASHES_A[4], 0),
    ]);
    assert_file_positions(
        xml,
        &first_source_file.attribute_positions,
        &primary_file_positions,
    )?;

    let mut repeated_file_positions =
        prefixed_positions("dump-file", DUMP_FILE_FIELDS, "dump-file-b");
    repeated_file_positions.extend([
        position("dump-file", "id", "same-publisher-id", 1),
        position("dump-file", "size", "0005", 0),
        position("dump-file", "crc32", HASHES_B[0], 0),
        position("dump-file", "md5", HASHES_B[1], 0),
        position("dump-file", "sha1", HASHES_B[2], 0),
        position("dump-file", "sha256", HASHES_B[3], 0),
        position("dump-file", "origin_sha256", "not-a-digest", 0),
    ]);
    assert_file_positions(
        xml,
        &second_source_file.attribute_positions,
        &repeated_file_positions,
    )?;

    Ok(primary_file_positions.len())
}

fn assert_release_file_positions(release: &NoIntroDatabaseRelease, xml: &str) -> TestResult<usize> {
    let release_file = release
        .files
        .first()
        .ok_or("release file reference is missing")?;
    let mut release_file_positions =
        prefixed_positions("release-file", RELEASE_FILE_FIELDS, "release-file-a");
    release_file_positions.extend([
        position("release-file", "size", "0004", 0),
        position("release-file", "crc32", "EeFf0011", 0),
        position("release-file", "md5", HASHES_RELEASE[0], 0),
        position("release-file", "sha1", HASHES_RELEASE[1], 0),
        position("release-file", "sha256", HASHES_RELEASE[2], 0),
    ]);
    assert_file_positions(
        xml,
        &release_file.attribute_positions,
        &release_file_positions,
    )?;

    Ok(release_file_positions.len())
}

fn assert_game_files(
    database: &Database,
    first: &NoIntroDatabaseGame,
    second: &NoIntroDatabaseGame,
    xml: &str,
) -> TestResult<usize> {
    let first_source = game_dump_source(first, "first game's dump source is missing")?;
    let second_source = game_dump_source(second, "second game's dump source is missing")?;
    let release = first
        .children
        .iter()
        .find_map(|child| match child {
            NoIntroDatabaseGameChild::Release(value) => Some(value),
            _ => None,
        })
        .ok_or("first game's release is missing")?;
    let first_source_file = first_source
        .files
        .first()
        .ok_or("first source file reference is missing")?;
    let release_file = release
        .files
        .first()
        .ok_or("release file reference is missing")?;
    let second_source_file = second_source
        .files
        .first()
        .ok_or("second source file reference is missing")?;
    assert_eq!(first_source_file.occurrence_order, 0);
    assert_eq!(first_source_file.source_order, 2);
    assert_eq!(release_file.occurrence_order, 1);
    assert_eq!(release_file.source_order, 2);
    assert_eq!(second_source_file.occurrence_order, 0);
    assert_eq!(second_source_file.source_order, 2);

    let file_ids = [
        first_source_file.occurrence_id,
        release_file.occurrence_id,
        second_source_file.occurrence_id,
    ];
    assert_file_payloads(database, &file_ids, first_source, release, second_source)?;
    let source_position_count = assert_source_file_positions(first_source, second_source, xml)?;
    let release_position_count = assert_release_file_positions(release, xml)?;
    Ok(source_position_count + release_position_count)
}

#[test]
fn published_metadata_pages_expose_all_native_fields_and_qname_positions_without_sources()
-> TestResult {
    let xml = all_fields_xml();
    let directory = tempfile::tempdir()?;
    let (database, page) = read_fixture_without_sources(&directory, &xml)?;
    assert_eq!(page.games.len(), 2);
    assert!(page.next_cursor.is_none());
    assert_eq!(page.document.mode, NoIntroDatabaseMode::ObservedCompatible);
    assert_eq!(
        page.document.envelope,
        mame_coalesce::no_intro_db_xml::EnvelopeKind::SingleDatafile
    );

    let first = page.games.first().ok_or("first game is missing")?;
    let second = page.games.get(1).ok_or("second game is missing")?;
    assert_eq!(first.name.as_str(), "same-publisher-name");
    assert_eq!(second.name.as_str(), "same-publisher-name");
    assert_ne!(first.id, second.id);
    assert_game_child_order(first, &[0, 1, 2, 3, 4]);
    assert_game_child_order(second, &[0, 1]);

    let mut position_total = assert_first_game_metadata(first, &xml)?;
    assert_second_game_metadata(second, &xml)?;
    position_total += assert_game_files(&database, first, second, &xml)?;
    assert_eq!(
        position_total, 129,
        "89 metadata plus 40 file attribute positions"
    );
    Ok(())
}
