use std::fmt::Write as _;

use camino::Utf8PathBuf;
use diesel::{QueryableByName, RunQueryDsl, SqliteConnection, sql_query, sql_types::BigInt};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    catalog_files::{
        self, ContentOccurrenceLimit, OccurrenceId, SoftwareDumpStatus, SoftwareFileOperation,
        SoftwareFilePayload,
    },
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey, media::SourceLoadInstruction},
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const SHA1: &str = "ABCDEF0123ABCDEF0123ABCDEF0123ABCDEF0123";

struct ImportedSoftware {
    directory: tempfile::TempDir,
    database: Database,
    connection: SqliteConnection,
}

#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
}

fn import_software(xml: &str) -> TestResult<ImportedSoftware> {
    use diesel::Connection;

    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 test path")?;
    let database = Database::open(&path)?;
    let document_path = Utf8PathBuf::from_path_buf(directory.path().join("software.xml"))
        .map_err(|_| "non-UTF-8 document path")?;
    std::fs::write(&document_path, xml)?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::MameSoftwareListXml,
            source_key: PublishingSourceKey::new("software-query"),
            source_display_name: "Software query".to_owned(),
            catalog_key: CatalogKey::new("software-query"),
            catalog_display_name: "Software query".to_owned(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    Ok(ImportedSoftware {
        directory,
        database,
        connection: SqliteConnection::establish(path.as_str())?,
    })
}

fn occurrences(
    imported: &mut ImportedSoftware,
) -> TestResult<Vec<catalog_files::CatalogFileOccurrence>> {
    let ids = sql_query("SELECT occurrence_id FROM asset_occurrences ORDER BY occurrence_id")
        .load::<IdRow>(&mut imported.connection)?
        .into_iter()
        .map(|row| OccurrenceId::from_database(row.occurrence_id))
        .collect::<Vec<_>>();
    Ok(catalog_files::occurrences_for_ids(
        &imported.database,
        &ids,
    )?)
}

#[test]
fn public_queries_return_native_software_payloads() -> TestResult {
    let mut imported = import_software(&format!(
        r#"<softwarelist name="nes"><software name="game">
        <description>Game</description><year>2000</year><publisher>Example</publisher>
        <part name="cart" interface="nes_cart"><dataarea name="rom" size="0x40">
        <rom name="file.bin" size="010" offset="0x0" sha1="{SHA1}"/>
        <vendor ignored="source-order-only"/>
        <rom size="4" offset="010" loadflag="continue"/>
        <rom size="1" offset="0x10" value="ff" loadflag="fill"/>
        </dataarea><diskarea name="disks"><disk name="image" sha1="{SHA1}"/></diskarea>
        </part></software></softwarelist>"#
    ))?;
    let files = occurrences(&mut imported)?;
    assert_eq!(files.len(), 4);
    assert!(files.iter().all(|file| file.software_file.is_some()));
    let load = files.first().ok_or("missing load entry")?;
    let Some(SoftwareFilePayload::Rom(rom)) = &load.software_file else {
        return Err("load lacks ROM payload".into());
    };
    assert_eq!(rom.name.as_deref(), Some("file.bin"));
    assert_eq!(rom.size_text.as_deref(), Some("010"));
    assert_eq!(rom.size, Some(8));
    assert_eq!(rom.offset_text.as_deref(), Some("0x0"));
    assert_eq!(rom.offset, Some(0));
    assert_eq!(rom.sha1_text.as_deref(), Some(SHA1));
    assert_eq!(rom.crc_text, None);
    assert_eq!(rom.status, SoftwareDumpStatus::Good);
    assert!(!rom.status_specified);
    assert_eq!(rom.load_instruction, None);
    assert_eq!(rom.operation, SoftwareFileOperation::Load);
    assert_eq!(rom.declaration_occurrence_id, Some(load.occurrence_id));
    assert_eq!(rom.component_order, 0);
    assert_eq!(rom.source_order, 0);
    assert_eq!(
        load.provenance.native_occurrence_location,
        Some(rom.location)
    );
    let continuation = files.get(1).ok_or("missing continuation")?;
    let Some(SoftwareFilePayload::Rom(continued)) = &continuation.software_file else {
        return Err("continuation lacks ROM payload".into());
    };
    assert_eq!(continued.name, None);
    assert_eq!(continued.operation, SoftwareFileOperation::Continue);
    assert_eq!(
        continued.declaration_occurrence_id,
        Some(load.occurrence_id)
    );
    assert_eq!(continued.offset_text.as_deref(), Some("010"));
    assert_eq!(continued.offset, Some(8));
    assert_eq!(continued.size, Some(4));
    assert_eq!(continued.component_order, 1);
    assert_eq!(continued.source_order, 2);
    assert_eq!(continuation.content_id, None);
    let fill = files.get(2).ok_or("missing fill")?;
    let Some(SoftwareFilePayload::Rom(filled)) = &fill.software_file else {
        return Err("fill lacks ROM payload".into());
    };
    assert_eq!(filled.operation, SoftwareFileOperation::Fill);
    assert_eq!(filled.declaration_occurrence_id, None);
    assert_eq!(filled.value.as_deref(), Some("ff"));
    assert_eq!(filled.component_order, 2);
    assert_eq!(filled.source_order, 3);
    assert_eq!(fill.content_id, None);
    let disk = files.get(3).ok_or("missing disk")?;
    let Some(SoftwareFilePayload::Disk(image)) = &disk.software_file else {
        return Err("disk lacks native payload".into());
    };
    assert_eq!(image.name, "image");
    assert_eq!(image.sha1_text.as_deref(), Some(SHA1));
    assert_eq!(image.status, SoftwareDumpStatus::Good);
    assert!(!image.status_specified);
    assert!(!image.writeable);
    assert!(!image.writeable_specified);
    assert_eq!(disk.content_id, None);
    assert_eq!(
        disk.provenance.native_occurrence_location,
        Some(image.location)
    );
    assert_eq!(
        disk.digests.first().ok_or("missing disk digest")?.scope,
        "chd_header_sha1"
    );
    Ok(())
}

#[test]
fn software_payloads_preserve_empty_invalid_and_explicit_defaults() -> TestResult {
    let mut imported = import_software(
        r#"<softwarelist name="list"><software name="game">
        <description>Game</description><year>2000</year><publisher>Example</publisher>
        <part name="cart" interface="cart"><dataarea name="rom" size="16">
        <rom/><rom name="" size="" offset="wrong" crc="" sha1="invalid" status="good"/>
        <rom name="odd.bin" size="9223372036854775808" offset="0xG" status="baddump"/>
        </dataarea><diskarea name="disks"><disk name="" sha1="" status="good" writeable="no"/>
        <disk name="unknown" sha1="bad" status="nodump" writeable="yes"/></diskarea>
        </part></software></softwarelist>"#,
    )?;
    let files = occurrences(&mut imported)?;
    assert_eq!(files.len(), 5);
    assert!(files.iter().all(|file| file.content_id.is_none()));
    let Some(SoftwareFilePayload::Rom(absent)) =
        &files.first().ok_or("missing empty ROM")?.software_file
    else {
        return Err("empty ROM lacks native payload".into());
    };
    assert_eq!(absent.name, None);
    assert_eq!(absent.size_text, None);
    assert_eq!(absent.offset_text, None);
    assert_eq!(absent.crc_text, None);
    assert_eq!(absent.sha1_text, None);
    assert!(!absent.status_specified);
    let Some(SoftwareFilePayload::Rom(empty)) = &files
        .get(1)
        .ok_or("missing explicit empty ROM")?
        .software_file
    else {
        return Err("explicit empty ROM lacks payload".into());
    };
    assert_eq!(empty.name.as_deref(), Some(""));
    assert_eq!(empty.size_text.as_deref(), Some(""));
    assert_eq!(empty.offset_text.as_deref(), Some("wrong"));
    assert_eq!(empty.crc_text.as_deref(), Some(""));
    assert_eq!(empty.sha1_text.as_deref(), Some("invalid"));
    assert_eq!(empty.size, None);
    assert_eq!(empty.offset, None);
    assert!(empty.status_specified);
    assert_eq!(empty.status, SoftwareDumpStatus::Good);
    let Some(SoftwareFilePayload::Rom(overflow)) =
        &files.get(2).ok_or("missing overflow ROM")?.software_file
    else {
        return Err("overflow ROM lacks payload".into());
    };
    assert_eq!(overflow.size_text.as_deref(), Some("9223372036854775808"));
    assert_eq!(overflow.size, None);
    assert_eq!(overflow.offset_text.as_deref(), Some("0xG"));
    assert_eq!(overflow.offset, None);
    assert_eq!(overflow.status, SoftwareDumpStatus::BadDump);
    let Some(SoftwareFilePayload::Disk(empty_disk)) =
        &files.get(3).ok_or("missing empty disk")?.software_file
    else {
        return Err("empty disk lacks payload".into());
    };
    assert_eq!(empty_disk.name, "");
    assert_eq!(empty_disk.sha1_text.as_deref(), Some(""));
    assert!(!empty_disk.writeable);
    assert!(empty_disk.writeable_specified);
    assert!(empty_disk.status_specified);
    let Some(SoftwareFilePayload::Disk(nodump)) =
        &files.get(4).ok_or("missing nodump disk")?.software_file
    else {
        return Err("nodump disk lacks payload".into());
    };
    assert_eq!(nodump.sha1_text.as_deref(), Some("bad"));
    assert_eq!(nodump.status, SoftwareDumpStatus::NoDump);
    assert!(nodump.writeable);
    assert!(nodump.writeable_specified);
    Ok(())
}

#[test]
fn content_pages_keep_cross_list_native_payloads_and_exclude_operations() -> TestResult {
    let mut imported = import_software(&format!(
        r#"<softwarelists><softwarelist name="nes"><software name="game">
        <description>NES</description><year>2000</year><publisher>Example</publisher>
        <part name="cart" interface="cart"><dataarea name="rom" size="32">
        <rom name="nes.bin" size="8" sha1="{SHA1}"/>
        <rom size="8" loadflag="reload"/>
        </dataarea></part></software></softwarelist>
        <softwarelist name="snes"><software name="game">
        <description>SNES</description><year>2001</year><publisher>Example</publisher>
        <part name="cart" interface="cart"><dataarea name="rom" size="32">
        <rom name="snes.bin" size="010" sha1="{}" loadflag="load16_byte"/>
        </dataarea></part></software></softwarelist></softwarelists>"#,
        SHA1.to_ascii_lowercase()
    ))?;
    let all = occurrences(&mut imported)?;
    assert_eq!(all.len(), 3);
    let content_id = all
        .first()
        .ok_or("missing first declaration")?
        .content_id
        .ok_or("missing shared UUID")?;
    assert_eq!(all.get(1).ok_or("missing reload")?.content_id, None);
    assert_eq!(
        all.get(2).ok_or("missing second declaration")?.content_id,
        Some(content_id)
    );
    let first = catalog_files::occurrences_for_content(
        &imported.database,
        content_id,
        ContentOccurrenceLimit::new(1)?,
        None,
    )?;
    assert_eq!(first.occurrences.len(), 1);
    assert_eq!(first.occurrences.first(), all.first());
    let next = first
        .next_cursor
        .as_ref()
        .ok_or("missing continuation cursor")?;
    let second = catalog_files::occurrences_for_content(
        &imported.database,
        content_id,
        ContentOccurrenceLimit::new(1)?,
        Some(next),
    )?;
    assert_eq!(second.occurrences.len(), 1);
    assert_eq!(second.occurrences.first(), all.get(2));
    assert!(second.next_cursor.is_none());
    let Some(SoftwareFilePayload::Rom(rom)) = &second
        .occurrences
        .first()
        .ok_or("missing page ROM")?
        .software_file
    else {
        return Err("page lacks native ROM payload".into());
    };
    assert_eq!(rom.name.as_deref(), Some("snes.bin"));
    assert_eq!(rom.size_text.as_deref(), Some("010"));
    assert_eq!(rom.operation, SoftwareFileOperation::Load);
    assert!(rom.load_instruction.is_some());
    Ok(())
}

const LOAD_MODES: [(&str, SourceLoadInstruction, SoftwareFileOperation); 14] = [
    (
        "load16_byte",
        SourceLoadInstruction::Load16Byte,
        SoftwareFileOperation::Load,
    ),
    (
        "load16_word",
        SourceLoadInstruction::Load16Word,
        SoftwareFileOperation::Load,
    ),
    (
        "load16_word_swap",
        SourceLoadInstruction::Load16WordSwap,
        SoftwareFileOperation::Load,
    ),
    (
        "load32_byte",
        SourceLoadInstruction::Load32Byte,
        SoftwareFileOperation::Load,
    ),
    (
        "load32_word",
        SourceLoadInstruction::Load32Word,
        SoftwareFileOperation::Load,
    ),
    (
        "load32_word_swap",
        SourceLoadInstruction::Load32WordSwap,
        SoftwareFileOperation::Load,
    ),
    (
        "load32_dword",
        SourceLoadInstruction::Load32Dword,
        SoftwareFileOperation::Load,
    ),
    (
        "load64_word",
        SourceLoadInstruction::Load64Word,
        SoftwareFileOperation::Load,
    ),
    (
        "load64_word_swap",
        SourceLoadInstruction::Load64WordSwap,
        SoftwareFileOperation::Load,
    ),
    (
        "reload",
        SourceLoadInstruction::Reload,
        SoftwareFileOperation::Reload,
    ),
    (
        "continue",
        SourceLoadInstruction::Continue,
        SoftwareFileOperation::Continue,
    ),
    (
        "reload_plain",
        SourceLoadInstruction::ReloadPlain,
        SoftwareFileOperation::ReloadPlain,
    ),
    (
        "ignore",
        SourceLoadInstruction::Ignore,
        SoftwareFileOperation::Ignore,
    ),
    (
        "fill",
        SourceLoadInstruction::Fill,
        SoftwareFileOperation::Fill,
    ),
];

#[test]
fn all_declared_loadflags_have_typed_query_values_without_discarding_source_fields() -> TestResult {
    let mut xml = String::from(
        r#"<softwarelist name="list"><software name="game"><description>Game</description>
        <year>2000</year><publisher>Example</publisher><part name="cart" interface="cart">"#,
    );
    for (flag, _, _) in LOAD_MODES {
        write!(
            xml,
            r#"<dataarea name="{flag}" size="32"><rom name="base.bin" size="8"/>
            <rom name="declared.bin" size="02" offset="0x08" value="255"
            crc="invalid" sha1="" status="baddump" loadflag="{flag}"/></dataarea>"#
        )?;
    }
    xml.push_str("</part></software></softwarelist>");
    let mut imported = import_software(&xml)?;
    let files = occurrences(&mut imported)?;
    assert_eq!(files.len(), LOAD_MODES.len() * 2);
    for (index, (flag, instruction, operation)) in LOAD_MODES.iter().enumerate() {
        assert_eq!(
            SourceLoadInstruction::from_source_name(flag),
            Some(*instruction)
        );
        let base = files.get(index * 2).ok_or("missing base ROM")?;
        let file = files.get(index * 2 + 1).ok_or("missing flagged ROM")?;
        let Some(SoftwareFilePayload::Rom(rom)) = &file.software_file else {
            return Err("flagged ROM lacks payload".into());
        };
        assert_eq!(rom.load_instruction, Some(*instruction), "{flag}");
        assert_eq!(rom.operation, *operation, "{flag}");
        assert_eq!(rom.name.as_deref(), Some("declared.bin"), "{flag}");
        assert_eq!(rom.size_text.as_deref(), Some("02"), "{flag}");
        assert_eq!(rom.size, Some(2), "{flag}");
        assert_eq!(rom.offset_text.as_deref(), Some("0x08"), "{flag}");
        assert_eq!(rom.offset, Some(8), "{flag}");
        assert_eq!(rom.value.as_deref(), Some("255"), "{flag}");
        assert_eq!(rom.crc_text.as_deref(), Some("invalid"), "{flag}");
        assert_eq!(rom.sha1_text.as_deref(), Some(""), "{flag}");
        assert_eq!(rom.status, SoftwareDumpStatus::BadDump, "{flag}");
        assert!(rom.status_specified, "{flag}");
        let expected_declaration = match operation {
            SoftwareFileOperation::Load => Some(file.occurrence_id),
            SoftwareFileOperation::Fill => None,
            _ => Some(base.occurrence_id),
        };
        assert_eq!(
            rom.declaration_occurrence_id, expected_declaration,
            "{flag}"
        );
        assert_eq!(file.content_id, None, "{flag}");
    }
    Ok(())
}

#[test]
fn loadflag_conversion_rejects_unknown_empty_or_normalized_spellings() {
    for invalid in [
        "",
        "LOAD16_BYTE",
        " load16_byte",
        "load16_byte ",
        "load128_byte",
    ] {
        assert_eq!(
            SourceLoadInstruction::from_source_name(invalid),
            None,
            "{invalid:?}"
        );
    }
}

#[test]
fn unresolved_operations_keep_fields_without_borrowing_another_area_declaration() -> TestResult {
    let mut imported = import_software(
        r#"<softwarelist name="list"><software name="game"><description>Game</description>
        <year>2000</year><publisher>Example</publisher><part name="cart" interface="cart">
        <dataarea name="first" size="16"><rom name="file.bin" size="8"/></dataarea>
        <dataarea name="second" size="16"><rom name="still-source.bin" size="4"
        sha1="invalid" offset="04" loadflag="continue"/></dataarea>
        </part></software></softwarelist>"#,
    )?;
    let files = occurrences(&mut imported)?;
    let file = files.get(1).ok_or("missing unresolved continuation")?;
    let Some(SoftwareFilePayload::Rom(rom)) = &file.software_file else {
        return Err("unresolved continuation lacks payload".into());
    };
    assert_eq!(rom.operation, SoftwareFileOperation::Continue);
    assert_eq!(rom.declaration_occurrence_id, None);
    assert_eq!(rom.name.as_deref(), Some("still-source.bin"));
    assert_eq!(rom.sha1_text.as_deref(), Some("invalid"));
    assert_eq!(rom.offset, Some(4));
    assert_eq!(file.content_id, None);
    assert_eq!(
        file.provenance
            .software_owner
            .as_ref()
            .ok_or("missing area owner")?
            .area_name,
        "second"
    );
    Ok(())
}

#[test]
fn shared_content_pages_do_not_attach_software_payloads_to_root_roms() -> TestResult {
    let mut imported = import_software(&format!(
        r#"<softwarelist name="nes"><software name="game"><description>Game</description>
        <year>2000</year><publisher>Example</publisher><part name="cart" interface="cart">
        <dataarea name="rom" size="8"><rom name="software.bin" size="8" sha1="{SHA1}"/></dataarea>
        </part></software></softwarelist>"#
    ))?;
    let root_document = Utf8PathBuf::from_path_buf(imported.directory.path().join("root.xml"))
        .map_err(|_| "non-UTF-8 document path")?;
    std::fs::write(
        &root_document,
        format!(
            r#"<datafile><header><name>Root</name></header><game name="game"><rom name="root.bin" size="8" sha1="{SHA1}"/></game></datafile>"#
        ),
    )?;
    let report = app::import_catalog(
        &imported.database,
        &CatalogImportRequest {
            document_path: root_document,
            format: CatalogDocumentFormat::Logiqx(
                mame_coalesce::logiqx::LogiqxMode::ObservedCompatible,
            ),
            source_key: PublishingSourceKey::new("root-query"),
            source_display_name: "Root query".to_owned(),
            catalog_key: CatalogKey::new("root-query"),
            catalog_display_name: "Root query".to_owned(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    let files = occurrences(&mut imported)?;
    assert_eq!(files.len(), 2);
    let software = files.first().ok_or("missing software occurrence")?;
    let root = files.get(1).ok_or("missing root occurrence")?;
    let content_id = software.content_id.ok_or("missing shared UUID")?;
    assert_eq!(root.content_id, Some(content_id));
    assert!(software.software_file.is_some());
    assert_eq!(root.software_file, None);
    assert_eq!(root.provenance.software_owner, None);
    let page = catalog_files::occurrences_for_content(
        &imported.database,
        content_id,
        ContentOccurrenceLimit::new(10)?,
        None,
    )?;
    assert_eq!(page.occurrences, files);
    assert_eq!(page.next_cursor, None);
    Ok(())
}
