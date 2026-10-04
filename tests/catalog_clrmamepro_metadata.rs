use std::fmt::Write;

use camino::Utf8PathBuf;
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    catalog_clrmamepro::{
        ClrMameProFieldPosition, ClrMameProForceMerging, ClrMameProForceNoDump,
        ClrMameProForceZipping, ClrMameProHeaderField, ClrMameProPage, ClrMameProPageLimit,
        ClrMameProParentKind, ClrMameProSetChild, ClrMameProSetField, sets_for_snapshot,
    },
    catalog_files::{
        ClrMameProDumpStatus, ClrMameProEvidenceScope, ClrMameProFilePayload, ClrMameProRomField,
        DigestAlgorithm, OccurrenceId, occurrences_for_ids,
    },
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey, SnapshotKey},
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn checked<T>(values: &[T], index: usize) -> TestResult<&T> {
    values.get(index).ok_or_else(|| {
        format!(
            "missing fixture item at index {index} ({} items)",
            values.len()
        )
        .into()
    })
}

const HEADER_FIELDS: [(ClrMameProHeaderField, i64, &str); 15] = [
    (ClrMameProHeaderField::Name, 0, "name"),
    (ClrMameProHeaderField::Description, 1, "description"),
    (ClrMameProHeaderField::Version, 2, "version"),
    (ClrMameProHeaderField::Date, 3, "date"),
    (ClrMameProHeaderField::Author, 4, "author"),
    (ClrMameProHeaderField::Email, 5, "email"),
    (ClrMameProHeaderField::Homepage, 6, "homepage"),
    (ClrMameProHeaderField::Url, 7, "url"),
    (ClrMameProHeaderField::Comment, 8, "comment"),
    (ClrMameProHeaderField::Category, 9, "category"),
    (ClrMameProHeaderField::HeaderDefinition, 10, "header"),
    (ClrMameProHeaderField::ForceMerging, 11, "forcemerging"),
    (ClrMameProHeaderField::ForceZipping, 12, "forcezipping"),
    (ClrMameProHeaderField::ForcePacking, 13, "forcepacking"),
    (ClrMameProHeaderField::ForceNoDump, 14, "forcenodump"),
];

const SET_FIELDS: [(ClrMameProSetField, i64, &str); 12] = [
    (ClrMameProSetField::Name, 0, "name"),
    (ClrMameProSetField::CloneOf, 1, "cloneof"),
    (ClrMameProSetField::Description, 2, "description"),
    (ClrMameProSetField::Year, 3, "year"),
    (ClrMameProSetField::Manufacturer, 4, "manufacturer"),
    (ClrMameProSetField::RebuildTo, 5, "rebuildto"),
    (ClrMameProSetField::SampleOf, 6, "sampleof"),
    (ClrMameProSetField::Region, 7, "region"),
    (ClrMameProSetField::ReleaseYear, 8, "releaseyear"),
    (ClrMameProSetField::ReleaseMonth, 9, "releasemonth"),
    (ClrMameProSetField::ReleaseDay, 10, "releaseday"),
    (ClrMameProSetField::Serial, 11, "serial"),
];

const ROM_FIELDS: [(ClrMameProRomField, i64, &str); 12] = [
    (ClrMameProRomField::Name, 0, "name"),
    (ClrMameProRomField::Size, 1, "size"),
    (ClrMameProRomField::Crc, 2, "crc"),
    (ClrMameProRomField::Crc32, 3, "crc32"),
    (ClrMameProRomField::Md5, 4, "md5"),
    (ClrMameProRomField::Sha1, 5, "sha1"),
    (ClrMameProRomField::Merge, 6, "merge"),
    (ClrMameProRomField::Date, 7, "date"),
    (ClrMameProRomField::Serial, 8, "serial"),
    (ClrMameProRomField::Status, 9, "status"),
    (ClrMameProRomField::NoDump, 10, "nodump"),
    (ClrMameProRomField::BadDump, 11, "baddump"),
];

struct Catalog {
    directory: tempfile::TempDir,
    path: Utf8PathBuf,
    database: Database,
}

impl Catalog {
    fn new() -> TestResult<Self> {
        let directory = tempfile::tempdir()?;
        let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
        let database = Database::open(&path)?;
        Ok(Self {
            directory,
            path,
            database,
        })
    }

    fn import(&self, key: &str, contents: &str) -> TestResult<SnapshotKey> {
        let input = Utf8PathBuf::try_from(self.directory.path().join(format!("{key}.dat")))?;
        std::fs::write(&input, contents)?;
        let report = app::import_catalog(
            &self.database,
            &CatalogImportRequest {
                document_path: input,
                format: CatalogDocumentFormat::ClrMamePro,
                source_key: PublishingSourceKey::new(key),
                source_display_name: format!("source {key}"),
                catalog_key: CatalogKey::new(key),
                catalog_display_name: format!("catalog {key}"),
                scope: CatalogScope::Complete,
            },
        )?;
        if report.status != CatalogImportStatus::Succeeded {
            return Err(format!(
                "CMP fixture {key:?} failed to import ({} diagnostics)",
                report.diagnostic_count
            )
            .into());
        }
        report
            .snapshot_key
            .ok_or_else(|| format!("successful CMP fixture {key:?} has no snapshot").into())
    }

    fn page(
        &self,
        snapshot: &SnapshotKey,
        cursor: Option<&mame_coalesce::catalog_clrmamepro::ClrMameProCursor>,
        limit: usize,
    ) -> Result<ClrMameProPage, mame_coalesce::catalog_clrmamepro::ClrMameProQueryError> {
        sets_for_snapshot(
            &self.database,
            snapshot,
            cursor,
            ClrMameProPageLimit::new(limit)?,
        )
    }

    fn hide_source_and_objects(&self, key: &str) -> TestResult {
        std::fs::rename(
            self.directory.path().join(format!("{key}.dat")),
            self.directory.path().join(format!("{key}.unavailable")),
        )?;
        let objects = Utf8PathBuf::from(format!("{}.documents", self.path));
        if objects.exists() {
            std::fs::rename(objects, self.directory.path().join("objects-unavailable"))?;
        }
        Ok(())
    }
}

fn source_location(
    source: &str,
    needle: &str,
) -> TestResult<mame_coalesce::catalog_files::SourceLocation> {
    source_location_occurrence(source, needle, 0)
}

fn source_location_occurrence(
    source: &str,
    needle: &str,
    occurrence: usize,
) -> TestResult<mame_coalesce::catalog_files::SourceLocation> {
    let offset = source
        .match_indices(needle)
        .filter_map(|(offset, _)| {
            let boundary =
                |character: char| character.is_whitespace() || matches!(character, '(' | ')' | ';');
            let before = source.get(..offset)?.chars().next_back();
            let after = source
                .get(offset.checked_add(needle.len())?..)?
                .chars()
                .next();
            (before.is_none_or(boundary) && after.is_none_or(boundary)).then_some(offset)
        })
        .nth(occurrence)
        .ok_or_else(|| format!("fixture location token {needle:?} is absent"))?;
    let prefix = source
        .get(..offset)
        .ok_or("fixture token offset is invalid")?;
    let line = i64::try_from(prefix.bytes().filter(|byte| *byte == b'\n').count() + 1)?;
    let column = prefix
        .rsplit('\n')
        .next()
        .ok_or("fixture line prefix is absent")?
        .chars()
        .count()
        + 1;
    Ok(mame_coalesce::catalog_files::SourceLocation {
        line,
        column: i64::try_from(column)?,
    })
}

fn assert_position(
    position: &ClrMameProFieldPosition,
    source: &str,
    token: &str,
    source_field: &str,
    source_order: usize,
    quoted: bool,
) -> TestResult {
    assert_eq!(position.source_field, source_field);
    assert_eq!(position.source_order, source_order);
    assert_eq!(position.is_quoted, quoted);
    assert_eq!(position.location, source_location(source, token)?);
    Ok(())
}

fn field_children(
    set: &mame_coalesce::catalog_clrmamepro::ClrMameProSet,
) -> Vec<(ClrMameProSetField, String)> {
    set.children
        .iter()
        .filter_map(|child| match child {
            ClrMameProSetChild::Field { field, value } => Some((*field, value.value.clone())),
            _ => None,
        })
        .collect()
}

fn media_ids(page: &ClrMameProPage) -> Vec<OccurrenceId> {
    page.sets
        .iter()
        .flat_map(|set| &set.children)
        .filter_map(|child| match child {
            ClrMameProSetChild::Rom(rom) => Some(rom.occurrence_id),
            ClrMameProSetChild::Sample(sample) => Some(sample.occurrence_id),
            _ => None,
        })
        .collect()
}

#[test]
fn page_limits_are_checked() {
    assert!(ClrMameProPageLimit::new(0).is_err());
    assert!(ClrMameProPageLimit::new(1).is_ok());
    assert!(ClrMameProPageLimit::new(500).is_ok());
    assert!(ClrMameProPageLimit::new(501).is_err());
    assert!(ClrMameProPageLimit::new(usize::MAX).is_err());
}

#[test]
fn source_free_named_empty_set_does_not_invent_a_header_or_media() -> TestResult {
    let catalog = Catalog::new()?;
    let source = "set ( name \"\" )";
    let snapshot = catalog.import("empty-set", source)?;
    catalog.hide_source_and_objects("empty-set")?;
    let page = catalog.page(&snapshot, None, 1)?;
    assert_eq!(page.snapshot.snapshot_key, snapshot);
    assert_eq!(page.snapshot.source_name, "source empty-set");
    assert_eq!(page.snapshot.catalog_name, "catalog empty-set");
    assert_eq!(page.snapshot.declared_version, None);
    assert!(!page.document.header_present);
    assert!(page.document.header.is_none());
    assert!(page.document.comments.is_empty());
    assert_eq!(page.sets.len(), 1);
    assert_eq!(
        field_children(checked(&page.sets, 0)?),
        [(ClrMameProSetField::Name, String::new())]
    );
    assert!(
        checked(&page.sets, 0)?
            .children
            .iter()
            .all(|child| !matches!(
                child,
                ClrMameProSetChild::Rom(_) | ClrMameProSetChild::Sample(_)
            ))
    );
    assert!(page.next_cursor.is_none());
    Ok(())
}

#[test]
fn public_metadata_exposes_all_closed_field_ledgers_and_native_values() -> TestResult {
    let catalog = Catalog::new()?;
    let source = concat!(
        "; lexical before\n",
        "clrmamepro ( name \"Catalog 名\" description \"A description\" version \"v1\" ",
        "date \"2024-02-29\" author \"Author\" email \"a@example.test\" ",
        "homepage \"https://home.test\" url \"https://url.test\" comment \"header comment\" ",
        "category \"Arcade\" header \"DAT\" forcemerging \"none\" ",
        "forcezipping \"unzip\" forcepacking \"split\" forcenodump \"required\" ",
        "vendor \"ignored gap\" )\n",
        "; lexical between\n",
        "GAME ( NAME \"set 名\" cloneof \"clone-parent\" description \"A set\" ",
        "year 1984 manufacturer \"Example\" rebuildto parent sampleof \"sample-parent\" ",
        "region USA releaseyear 1982 releasemonth 01 releaseday 09 serial \"0007\" ",
        "rom ( name \"asset.bin\" size 00016 crc AABBCCDD crc32 aabbccdd ",
        "md5 BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB sha1 AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA ",
        "merge \"merged.bin\" date \"\" serial \"001\" status \"verified\" ",
        "nodump baddump vendor \"gap\" ) )\n",
        "; lexical after\n"
    );
    let snapshot = catalog.import("all-fields", source)?;
    catalog.hide_source_and_objects("all-fields")?;
    let page = catalog.page(&snapshot, None, 1)?;
    assert_document_metadata(&page, source)?;
    let set = page.sets.first().ok_or("fixture set disappeared")?;
    assert!(set.id.as_i64() > 0);
    assert_set_metadata(set, source)?;
    assert_rom_metadata(set, source)?;
    assert_eq!(page.sets.len(), 1);
    assert!(page.next_cursor.is_none());
    Ok(())
}

fn assert_document_metadata(page: &ClrMameProPage, source: &str) -> TestResult {
    let header = page
        .document
        .header
        .as_ref()
        .ok_or("fixture header disappeared")?;
    assert!(page.document.header_present);
    assert_eq!(page.snapshot.declared_version.as_deref(), Some("v1"));
    assert_eq!(header.source_block, "clrmamepro");
    assert_eq!(header.source_order, 0);
    assert_eq!(header.location, source_location(source, "clrmamepro")?);
    assert_eq!(
        header
            .fields
            .iter()
            .map(|field| field.field)
            .collect::<Vec<_>>(),
        HEADER_FIELDS.map(|(field, _, _)| field)
    );
    assert_eq!(
        header
            .fields
            .iter()
            .map(|field| field.value.value.as_str())
            .collect::<Vec<_>>(),
        [
            "Catalog 名",
            "A description",
            "v1",
            "2024-02-29",
            "Author",
            "a@example.test",
            "https://home.test",
            "https://url.test",
            "header comment",
            "Arcade",
            "DAT",
            "none",
            "unzip",
            "split",
            "required",
        ]
    );
    let raw_values = [
        "\"Catalog 名\"",
        "\"A description\"",
        "\"v1\"",
        "\"2024-02-29\"",
        "\"Author\"",
        "\"a@example.test\"",
        "\"https://home.test\"",
        "\"https://url.test\"",
        "\"header comment\"",
        "\"Arcade\"",
        "\"DAT\"",
        "\"none\"",
        "\"unzip\"",
        "\"split\"",
        "\"required\"",
    ];
    for (order, (((_, code, keyword), value), raw_value)) in HEADER_FIELDS
        .iter()
        .zip(&header.fields)
        .zip(raw_values)
        .enumerate()
    {
        assert_eq!(*code, value.field as i64);
        assert_position(
            &value.value.position,
            source,
            raw_value,
            keyword,
            order,
            true,
        )?;
    }
    assert_eq!(
        header.forcemerging_effective,
        Some(ClrMameProForceMerging::None)
    );
    assert_eq!(
        header.forcezipping_effective,
        Some(ClrMameProForceZipping::Unzip)
    );
    assert_eq!(
        header.forcenodump_effective,
        Some(ClrMameProForceNoDump::Required)
    );
    assert_document_comments(page, source)
}

fn assert_document_comments(page: &ClrMameProPage, source: &str) -> TestResult {
    assert_eq!(page.document.comment_count, 3);
    assert_eq!(
        page.document
            .comments
            .iter()
            .map(|comment| comment.text.as_str())
            .collect::<Vec<_>>(),
        ["; lexical before", "; lexical between", "; lexical after"]
    );
    for (comment, marker) in page.document.comments.iter().zip([
        "; lexical before",
        "; lexical between",
        "; lexical after",
    ]) {
        assert_eq!(comment.location, source_location(source, marker)?);
    }
    Ok(())
}

fn assert_set_metadata(
    set: &mame_coalesce::catalog_clrmamepro::ClrMameProSet,
    source: &str,
) -> TestResult {
    assert_eq!(set.source_block, "GAME");
    assert_eq!(set.source_order, 1);
    assert_eq!(set.location, source_location(source, "GAME")?);
    assert_eq!(
        field_children(set),
        [
            (ClrMameProSetField::Name, "set 名".into()),
            (ClrMameProSetField::Description, "A set".into()),
            (ClrMameProSetField::Year, "1984".into()),
            (ClrMameProSetField::Manufacturer, "Example".into()),
            (ClrMameProSetField::RebuildTo, "parent".into()),
            (ClrMameProSetField::Region, "USA".into()),
            (ClrMameProSetField::ReleaseYear, "1982".into()),
            (ClrMameProSetField::ReleaseMonth, "01".into()),
            (ClrMameProSetField::ReleaseDay, "09".into()),
            (ClrMameProSetField::Serial, "0007".into()),
        ]
    );
    let mut positions = set
        .children
        .iter()
        .filter_map(|child| match child {
            ClrMameProSetChild::Field { field, value } => Some((*field, &value.position)),
            ClrMameProSetChild::Parent(parent) => Some((
                match parent.kind {
                    ClrMameProParentKind::CloneOf => ClrMameProSetField::CloneOf,
                    ClrMameProParentKind::SampleOf => ClrMameProSetField::SampleOf,
                },
                &parent.target.position,
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    positions.sort_by_key(|(_, position)| position.source_order);
    let raw_values = [
        ("\"set 名\"", true),
        ("\"clone-parent\"", true),
        ("\"A set\"", true),
        ("1984", false),
        ("\"Example\"", true),
        ("parent", false),
        ("\"sample-parent\"", true),
        ("USA", false),
        ("1982", false),
        ("01", false),
        ("09", false),
        ("\"0007\"", true),
    ];
    assert_eq!(positions.len(), SET_FIELDS.len());
    for (order, (((field, code, keyword), (actual_field, position)), (raw_value, quoted))) in
        SET_FIELDS.iter().zip(positions).zip(raw_values).enumerate()
    {
        assert_eq!(*field, actual_field);
        assert_eq!(*code, *field as i64);
        let source_keyword = if *field == ClrMameProSetField::Name {
            "NAME"
        } else {
            keyword
        };
        assert_position(position, source, raw_value, source_keyword, order, quoted)?;
    }
    assert_set_parents(set)
}

fn assert_set_parents(set: &mame_coalesce::catalog_clrmamepro::ClrMameProSet) -> TestResult {
    let parents = set
        .children
        .iter()
        .filter_map(|child| match child {
            ClrMameProSetChild::Parent(parent) => Some((
                parent.kind,
                parent.target.value.as_str(),
                parent.relationship_id.database_value(),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        parents
            .iter()
            .map(|(kind, name, _)| (*kind, *name))
            .collect::<Vec<_>>(),
        [
            (ClrMameProParentKind::CloneOf, "clone-parent"),
            (ClrMameProParentKind::SampleOf, "sample-parent"),
        ]
    );
    assert_eq!(parents.len(), 2);
    assert!(parents.iter().all(|(_, _, id)| *id > 0));
    assert_ne!(checked(&parents, 0)?.2, checked(&parents, 1)?.2);
    Ok(())
}

fn assert_rom_metadata(
    set: &mame_coalesce::catalog_clrmamepro::ClrMameProSet,
    source: &str,
) -> TestResult {
    let rom = set
        .children
        .iter()
        .find_map(|child| match child {
            ClrMameProSetChild::Rom(rom) => Some(rom),
            _ => None,
        })
        .ok_or("fixture ROM disappeared")?;
    let payload = &rom.payload;
    assert_eq!(payload.location, source_location(source, "rom")?);
    assert_eq!(
        (
            payload.name.as_str(),
            payload.size_text.as_deref(),
            payload.size
        ),
        ("asset.bin", Some("00016"), Some(16))
    );
    assert_eq!(payload.crc_text.as_deref(), Some("AABBCCDD"));
    assert_eq!(payload.crc32_text.as_deref(), Some("aabbccdd"));
    assert_eq!(
        payload.md5_text.as_deref(),
        Some("BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB")
    );
    assert_eq!(
        payload.sha1_text.as_deref(),
        Some("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")
    );
    assert_eq!(payload.date.as_deref(), Some(""));
    assert_eq!(payload.serial.as_deref(), Some("001"));
    assert_eq!(payload.status_text.as_deref(), Some("verified"));
    assert!(payload.nodump_present && payload.baddump_present);
    assert_eq!(payload.dump_status, None);
    assert_eq!(payload.evidence_scope, ClrMameProEvidenceScope::WholeAsset);
    assert_eq!(
        payload
            .merge
            .as_ref()
            .map(|merge| merge.merge_name.as_str()),
        Some("merged.bin")
    );
    assert_eq!(
        payload
            .field_positions
            .iter()
            .map(|position| position.field)
            .collect::<Vec<_>>(),
        ROM_FIELDS.map(|(field, _, _)| field)
    );
    let raw_values = [
        ("\"asset.bin\"", true),
        ("00016", false),
        ("AABBCCDD", false),
        ("aabbccdd", false),
        ("BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB", false),
        ("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", false),
        ("\"merged.bin\"", true),
        ("\"\"", true),
        ("\"001\"", true),
        ("\"verified\"", true),
        ("nodump", false),
        ("baddump", false),
    ];
    for (order, (((field, code, keyword), position), (raw_value, quoted))) in ROM_FIELDS
        .iter()
        .zip(&payload.field_positions)
        .zip(raw_values)
        .enumerate()
    {
        assert_eq!(*code, *field as i64);
        assert_eq!(*field, position.field);
        assert_position(
            &position.position,
            source,
            raw_value,
            keyword,
            order,
            quoted,
        )?;
    }
    Ok(())
}

#[test]
fn headers_comments_and_sets_keep_document_order_across_cursor_pages() -> TestResult {
    let catalog = Catalog::new()?;
    let source = concat!(
        "; before\n",
        "game ( name first )\n",
        "; before header\n",
        "clrmamepro ( name \"\" version \"v2\" )\n",
        "; after header\n",
        "set ( name second )\n",
        "game ( name third )\n"
    );
    let snapshot = catalog.import("header-between", source)?;
    catalog.hide_source_and_objects("header-between")?;
    let first = catalog.page(&snapshot, None, 1)?;
    let cursor = first
        .next_cursor
        .as_ref()
        .ok_or("first page has no continuation")?;
    let second = catalog.page(&snapshot, Some(cursor), 1)?;
    let third = catalog.page(&snapshot, second.next_cursor.as_ref(), 1)?;
    assert_eq!(checked(&first.sets, 0)?.source_order, 0);
    assert_eq!(checked(&second.sets, 0)?.source_order, 2);
    assert_eq!(checked(&third.sets, 0)?.source_order, 3);
    assert_eq!(
        [
            checked(&field_children(checked(&first.sets, 0)?), 0)?
                .1
                .as_str(),
            checked(&field_children(checked(&second.sets, 0)?), 0)?
                .1
                .as_str(),
            checked(&field_children(checked(&third.sets, 0)?), 0)?
                .1
                .as_str(),
        ],
        ["first", "second", "third"]
    );
    assert_eq!(first.document, second.document);
    assert_eq!(second.document, third.document);
    let header = first
        .document
        .header
        .as_ref()
        .ok_or("between-sets header missing")?;
    assert_eq!(header.source_order, 1);
    assert_eq!(checked(&header.fields, 0)?.value.value, "");
    assert!(checked(&header.fields, 0)?.value.position.is_quoted);
    assert_eq!(first.snapshot.declared_version.as_deref(), Some("v2"));
    assert_eq!(
        first
            .document
            .comments
            .iter()
            .map(|comment| comment.text.as_str())
            .collect::<Vec<_>>(),
        ["; before", "; before header", "; after header"]
    );
    assert!(third.next_cursor.is_none());
    Ok(())
}

#[test]
fn header_before_and_after_set_has_exact_document_form_position() -> TestResult {
    let catalog = Catalog::new()?;
    let before = catalog.import(
        "header-leading",
        "clrmamepro ( name first ) game ( name one )",
    )?;
    let after = catalog.import(
        "header-trailing",
        "game ( name one ) clrmamepro ( name last )",
    )?;
    catalog.hide_source_and_objects("header-leading")?;
    catalog.hide_source_and_objects("header-trailing")?;
    let before_page = catalog.page(&before, None, 1)?;
    assert_eq!(
        before_page
            .document
            .header
            .as_ref()
            .ok_or("leading header missing")?
            .source_order,
        0
    );
    assert_eq!(checked(&before_page.sets, 0)?.source_order, 1);
    let after_page = catalog.page(&after, None, 1)?;
    assert_eq!(checked(&after_page.sets, 0)?.source_order, 0);
    assert_eq!(
        after_page
            .document
            .header
            .as_ref()
            .ok_or("trailing header missing")?
            .source_order,
        1
    );
    Ok(())
}

#[test]
fn present_empty_header_has_native_presence_and_default_directive_state() -> TestResult {
    let catalog = Catalog::new()?;
    let snapshot = catalog.import("present-empty-header", "clrmamepro ( ) game ( name set )")?;
    catalog.hide_source_and_objects("present-empty-header")?;
    let page = catalog.page(&snapshot, None, 1)?;
    assert!(page.document.header_present);
    let header = page
        .document
        .header
        .as_ref()
        .ok_or("present empty header missing")?;
    assert_eq!(header.source_block, "clrmamepro");
    assert_eq!(header.source_order, 0);
    assert!(header.fields.is_empty());
    assert_eq!(
        header.forcenodump_effective,
        Some(ClrMameProForceNoDump::Obsolete)
    );
    assert_eq!(header.forcemerging_effective, None);
    assert_eq!(header.forcezipping_effective, None);
    Ok(())
}

#[test]
fn unknown_vendor_fields_leave_case_sensitive_source_keywords_and_order_gaps() -> TestResult {
    let catalog = Catalog::new()?;
    let source = concat!(
        "clrmamepro ( NAME \"Catalog\" vendor \"header gap\" VERSION \"v3\" )\n",
        "game ( NAME set vendor \"set gap\" SERIAL \"0001\" ",
        "rom ( NAME a.bin vendor \"rom gap\" CRC AABBCCDD ) )"
    );
    let snapshot = catalog.import("vendor-gaps", source)?;
    catalog.hide_source_and_objects("vendor-gaps")?;
    let page = catalog.page(&snapshot, None, 1)?;
    let header = page
        .document
        .header
        .as_ref()
        .ok_or("vendor fixture header missing")?;
    assert_eq!(
        header
            .fields
            .iter()
            .map(|value| (
                value.field,
                value.value.position.source_field.as_str(),
                value.value.position.source_order
            ))
            .collect::<Vec<_>>(),
        [
            (ClrMameProHeaderField::Name, "NAME", 0),
            (ClrMameProHeaderField::Version, "VERSION", 2),
        ]
    );
    let set_positions = checked(&page.sets, 0)?
        .children
        .iter()
        .filter_map(|child| match child {
            ClrMameProSetChild::Field { field, value } => Some((
                *field,
                value.position.source_field.as_str(),
                value.position.source_order,
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        set_positions,
        [
            (ClrMameProSetField::Name, "NAME", 0),
            (ClrMameProSetField::Serial, "SERIAL", 2)
        ]
    );
    let rom = checked(&page.sets, 0)?
        .children
        .iter()
        .find_map(|child| match child {
            ClrMameProSetChild::Rom(rom) => Some(rom),
            _ => None,
        })
        .ok_or("vendor fixture ROM missing")?;
    assert_eq!(
        rom.payload
            .field_positions
            .iter()
            .map(|position| (
                position.field,
                position.position.source_field.as_str(),
                position.position.source_order
            ))
            .collect::<Vec<_>>(),
        [
            (ClrMameProRomField::Name, "NAME", 0),
            (ClrMameProRomField::Crc, "CRC", 2)
        ]
    );
    Ok(())
}

#[test]
fn empty_default_unknown_and_declared_header_directives_remain_distinct() -> TestResult {
    let catalog = Catalog::new()?;
    let default = catalog.import(
        "directive-default",
        "clrmamepro ( name catalog ) game ( name a )",
    )?;
    let unknown = catalog.import(
        "directive-unknown",
        "clrmamepro ( name catalog forcemerging mystery forcezipping mystery forcepacking mystery forcenodump mystery ) game ( name a )",
    )?;
    let declared = catalog.import(
        "directive-declared",
        "clrmamepro ( name catalog forcemerging full forcezipping zip forcepacking split forcenodump obsolete ) game ( name a )",
    )?;
    for key in [
        "directive-default",
        "directive-unknown",
        "directive-declared",
    ] {
        catalog.hide_source_and_objects(key)?;
    }
    let default_header = catalog
        .page(&default, None, 1)?
        .document
        .header
        .ok_or("default header missing")?;
    assert_eq!(
        default_header
            .fields
            .iter()
            .map(|field| field.field)
            .collect::<Vec<_>>(),
        [ClrMameProHeaderField::Name]
    );
    assert_eq!(
        default_header.forcenodump_effective,
        Some(ClrMameProForceNoDump::Obsolete)
    );
    assert_eq!(default_header.forcemerging_effective, None);
    assert_eq!(default_header.forcezipping_effective, None);
    let unknown_header = catalog
        .page(&unknown, None, 1)?
        .document
        .header
        .ok_or("unknown header missing")?;
    assert_eq!(unknown_header.forcemerging_effective, None);
    assert_eq!(unknown_header.forcezipping_effective, None);
    assert_eq!(unknown_header.forcenodump_effective, None);
    assert_eq!(
        unknown_header
            .fields
            .iter()
            .map(|field| field.value.value.as_str())
            .collect::<Vec<_>>(),
        ["catalog", "mystery", "mystery", "mystery", "mystery"]
    );
    let declared_header = catalog
        .page(&declared, None, 1)?
        .document
        .header
        .ok_or("declared header missing")?;
    assert_eq!(
        declared_header
            .fields
            .iter()
            .map(|field| field.value.value.as_str())
            .collect::<Vec<_>>(),
        ["catalog", "full", "zip", "split", "obsolete"]
    );
    assert_eq!(
        declared_header.forcemerging_effective,
        Some(ClrMameProForceMerging::Full)
    );
    assert_eq!(
        declared_header.forcezipping_effective,
        Some(ClrMameProForceZipping::Zip)
    );
    assert_eq!(
        declared_header.forcenodump_effective,
        Some(ClrMameProForceNoDump::Obsolete)
    );
    Ok(())
}

#[test]
fn mixed_children_preserve_repeated_empty_names_flags_and_independent_declarations() -> TestResult {
    let catalog = Catalog::new()?;
    let source = concat!(
        "clrmamepro ( name catalog )\n",
        "set ( name repeated cloneof \"\" sampleof \"\" serial \"0008\" ",
        "rom ( name \"\" size 0000 crc AABBCCDD crc32 aabbccdd date \"\" serial \"0009\" ) ",
        "sample \"\" rom ( name \"\" size 0001 crc AABBCCDE crc32 aabbccdf ",
        "status \"nodump\" ) sample \"intro.wav\" sample \"intro.wav\" )\n",
        "set ( name repeated rom ( name \"same.bin\" size 2 crc 01020304 nodump ) )"
    );
    let snapshot = catalog.import("mixed", source)?;
    catalog.hide_source_and_objects("mixed")?;
    let page = catalog.page(&snapshot, None, 2)?;
    assert_eq!(page.sets.len(), 2);
    let first = checked(&page.sets, 0)?;
    let second = checked(&page.sets, 1)?;
    assert_ne!(first.id, second.id);
    assert_eq!(checked(&field_children(first), 0)?.1, "repeated");
    assert_eq!(checked(&field_children(second), 0)?.1, "repeated");
    let parents = first
        .children
        .iter()
        .filter_map(|child| match child {
            ClrMameProSetChild::Parent(parent) => Some((parent.kind, parent.target.value.as_str())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        parents,
        [
            (ClrMameProParentKind::CloneOf, ""),
            (ClrMameProParentKind::SampleOf, "")
        ]
    );
    let ordered = first
        .children
        .iter()
        .map(ClrMameProSetChild::source_order)
        .collect::<Vec<_>>();
    assert_eq!(ordered, [0, 1, 2, 3, 4, 5, 6, 7, 8]);
    assert_mixed_roms(first)?;
    let samples = first
        .children
        .iter()
        .filter_map(|child| match child {
            ClrMameProSetChild::Sample(sample) => Some(&sample.payload),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        samples
            .iter()
            .map(|sample| sample.name.as_str())
            .collect::<Vec<_>>(),
        ["", "intro.wav", "intro.wav"]
    );
    assert_eq!(
        samples
            .iter()
            .map(|sample| sample.position.source_order)
            .collect::<Vec<_>>(),
        [5, 7, 8]
    );
    for (sample, (marker, occurrence, order)) in samples.iter().zip([
        ("sample \"\"", 0, 5),
        ("sample \"intro.wav\"", 0, 7),
        ("sample \"intro.wav\"", 1, 8),
    ]) {
        assert_eq!(sample.position.source_field, "sample");
        assert_eq!(sample.position.source_order, order);
        assert!(sample.position.is_quoted);
        let mut expected = source_location_occurrence(source, marker, occurrence)?;
        expected.column += 7;
        assert_eq!(sample.position.location, expected);
    }
    let second_rom = second
        .children
        .iter()
        .find_map(|child| match child {
            ClrMameProSetChild::Rom(rom) => Some(&rom.payload),
            _ => None,
        })
        .ok_or("second set ROM is missing")?;
    assert_eq!(second_rom.dump_status, Some(ClrMameProDumpStatus::NoDump));
    assert!(second_rom.nodump_present);
    assert!(!second_rom.baddump_present);
    Ok(())
}

fn assert_mixed_roms(set: &mame_coalesce::catalog_clrmamepro::ClrMameProSet) -> TestResult {
    let roms = set
        .children
        .iter()
        .filter_map(|child| match child {
            ClrMameProSetChild::Rom(rom) => Some(&rom.payload),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(roms.len(), 2);
    let first = checked(&roms, 0)?;
    let second = checked(&roms, 1)?;
    assert_eq!(first.name, "");
    assert_eq!(first.size_text.as_deref(), Some("0000"));
    assert_eq!(first.size, Some(0));
    assert_eq!(first.crc_text.as_deref(), Some("AABBCCDD"));
    assert_eq!(first.crc32_text.as_deref(), Some("aabbccdd"));
    assert_eq!(first.dump_status, None);
    assert_eq!(first.evidence_scope, ClrMameProEvidenceScope::WholeAsset);
    assert_eq!(second.name, "");
    assert_eq!(second.crc_text.as_deref(), Some("AABBCCDE"));
    assert_eq!(second.crc32_text.as_deref(), Some("aabbccdf"));
    assert_eq!(second.status_text.as_deref(), Some("nodump"));
    assert_eq!(second.dump_status, Some(ClrMameProDumpStatus::NoDump));
    assert_eq!(second.evidence_scope, ClrMameProEvidenceScope::WholeAsset);
    Ok(())
}

#[test]
fn lone_nodump_with_eligible_sha1_remains_a_public_content_identity_control() -> TestResult {
    let catalog = Catalog::new()?;
    let source = "game ( name control rom ( name control.bin sha1 AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA nodump ) )";
    let snapshot = catalog.import("lone-nodump-sha1", source)?;
    catalog.hide_source_and_objects("lone-nodump-sha1")?;
    let page = catalog.page(&snapshot, None, 1)?;
    let rom = checked(&page.sets, 0)?
        .children
        .iter()
        .find_map(|child| match child {
            ClrMameProSetChild::Rom(rom) => Some(rom),
            _ => None,
        })
        .ok_or("lone nodump control ROM is missing")?;
    assert_eq!(
        rom.payload.sha1_text.as_deref(),
        Some("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")
    );
    assert!(rom.payload.nodump_present);
    assert_eq!(rom.payload.dump_status, Some(ClrMameProDumpStatus::NoDump));
    assert_eq!(
        rom.payload.evidence_scope,
        ClrMameProEvidenceScope::WholeAsset
    );
    let occurrence = occurrences_for_ids(&catalog.database, &[rom.occurrence_id])?
        .into_iter()
        .next()
        .ok_or("lone nodump control bulk occurrence is missing")?;
    assert!(occurrence.content_id.is_some());
    assert_eq!(occurrence.digests.len(), 1);
    let digest = checked(&occurrence.digests, 0)?;
    assert_eq!(digest.algorithm, DigestAlgorithm::Sha1);
    assert_eq!(digest.value, [0xAA; 20]);
    assert_eq!(digest.scope, "whole_asset");
    Ok(())
}

#[test]
fn selected_set_children_cross_payload_batch_boundary_and_match_bulk_file_api() -> TestResult {
    let catalog = Catalog::new()?;
    let mut source = String::from("clrmamepro ( name catalog ) game ( name large ");
    for index in 0..405 {
        write!(source, "sample \"sample-{index:03}.wav\" ")?;
    }
    source.push(')');
    let snapshot = catalog.import("large-children", &source)?;
    catalog.hide_source_and_objects("large-children")?;
    let page = catalog.page(&snapshot, None, 1)?;
    let samples = checked(&page.sets, 0)?
        .children
        .iter()
        .filter_map(|child| match child {
            ClrMameProSetChild::Sample(sample) => Some(sample),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(samples.len(), 405);
    assert_eq!(checked(&samples, 0)?.payload.name, "sample-000.wav");
    assert_eq!(checked(&samples, 404)?.payload.name, "sample-404.wav");
    assert_eq!(
        samples
            .iter()
            .map(|sample| sample.occurrence_order)
            .collect::<Vec<_>>(),
        (0_i64..405).collect::<Vec<_>>()
    );
    let embedded = checked(&page.sets, 0)?
        .children
        .iter()
        .filter_map(|child| match child {
            ClrMameProSetChild::Sample(sample) => {
                Some((sample.occurrence_id, sample.payload.clone()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let ids = media_ids(&page);
    let bulk = occurrences_for_ids(&catalog.database, &ids)?;
    assert_eq!(bulk.len(), 405);
    for (occurrence, (id, expected)) in bulk.iter().zip(embedded) {
        assert_eq!(occurrence.occurrence_id, id);
        assert_eq!(
            occurrence.clrmamepro_file,
            Some(ClrMameProFilePayload::Sample(expected.clone()))
        );
        assert_eq!(
            occurrence.provenance.asset_name.as_deref(),
            Some(expected.name.as_str())
        );
        assert_eq!(occurrence.provenance.set_name, "large");
        assert!(occurrence.content_id.is_none());
    }
    Ok(())
}

#[test]
fn embedded_rom_and_sample_payloads_equal_public_bulk_occurrence_payloads() -> TestResult {
    let catalog = Catalog::new()?;
    let source = "game ( name owner rom ( name a.bin size 1 crc AABBCCDD ) sample \"intro.wav\" )";
    let snapshot = catalog.import("bulk-equality", source)?;
    catalog.hide_source_and_objects("bulk-equality")?;
    let page = catalog.page(&snapshot, None, 1)?;
    let ids = media_ids(&page);
    let bulk = occurrences_for_ids(&catalog.database, &ids)?;
    assert_eq!(bulk.len(), 2);
    for occurrence in &bulk {
        let embedded = checked(&page.sets, 0)?
            .children
            .iter()
            .find_map(|child| match child {
                ClrMameProSetChild::Rom(rom) if rom.occurrence_id == occurrence.occurrence_id => {
                    Some(ClrMameProFilePayload::Rom(Box::new(rom.payload.clone())))
                }
                ClrMameProSetChild::Sample(sample)
                    if sample.occurrence_id == occurrence.occurrence_id =>
                {
                    Some(ClrMameProFilePayload::Sample(sample.payload.clone()))
                }
                _ => None,
            })
            .ok_or("bulk occurrence has no embedded equivalent")?;
        assert_eq!(occurrence.clrmamepro_file.as_ref(), Some(&embedded));
    }
    Ok(())
}

#[test]
fn headerless_bom_unicode_and_crlf_keep_lexical_comment_coordinates() -> TestResult {
    let catalog = Catalog::new()?;
    let source = "\u{feff}; café\r\ngame ( name \"雪\" )\r\n; fin\r\n";
    let snapshot = catalog.import("unicode-crlf", source)?;
    catalog.hide_source_and_objects("unicode-crlf")?;
    let page = catalog.page(&snapshot, None, 1)?;
    assert!(!page.document.header_present);
    assert!(page.document.header.is_none());
    assert_eq!(
        page.document
            .comments
            .iter()
            .map(|comment| comment.text.as_str())
            .collect::<Vec<_>>(),
        ["; café\r", "; fin\r"]
    );
    assert_eq!(
        checked(&page.document.comments, 0)?.location,
        mame_coalesce::catalog_files::SourceLocation { line: 1, column: 1 }
    );
    assert_eq!(
        checked(&page.document.comments, 1)?.location,
        mame_coalesce::catalog_files::SourceLocation { line: 3, column: 1 }
    );
    assert_eq!(
        checked(&field_children(checked(&page.sets, 0)?), 0)?.1,
        "雪"
    );
    Ok(())
}

#[test]
fn public_field_ledgers_are_independent_and_have_no_duplicate_codes_or_keywords() {
    for ledger in [
        HEADER_FIELDS
            .iter()
            .map(|(_, code, keyword)| (*code, *keyword))
            .collect::<Vec<_>>(),
        SET_FIELDS
            .iter()
            .map(|(_, code, keyword)| (*code, *keyword))
            .collect::<Vec<_>>(),
        ROM_FIELDS
            .iter()
            .map(|(_, code, keyword)| (*code, *keyword))
            .collect::<Vec<_>>(),
    ] {
        let mut codes = ledger.iter().map(|(code, _)| *code).collect::<Vec<_>>();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), ledger.len());
        let mut keywords = ledger
            .iter()
            .map(|(_, keyword)| *keyword)
            .collect::<Vec<_>>();
        keywords.sort_unstable();
        keywords.dedup();
        assert_eq!(keywords.len(), ledger.len());
    }
    assert_eq!(
        HEADER_FIELDS.map(|(_, code, _)| code),
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]
    );
    assert_eq!(
        SET_FIELDS.map(|(_, code, _)| code),
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]
    );
    assert_eq!(
        ROM_FIELDS.map(|(_, code, _)| code),
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]
    );
    assert_eq!(
        HEADER_FIELDS.map(|(field, _, _)| field as i64),
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]
    );
    assert_eq!(
        SET_FIELDS.map(|(field, _, _)| field as i64),
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]
    );
    assert_eq!(
        ROM_FIELDS.map(|(field, _, _)| field as i64),
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]
    );
}
