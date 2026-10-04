use std::fmt::Write;

use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query, sql_types::Text,
};
use mame_coalesce::{
    NoIntroDatMode,
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    catalog_files::NoIntroDatEvidenceScope,
    catalog_no_intro_dat::{
        NoIntroDatForceNoDump, NoIntroDatGame, NoIntroDatGameChild, NoIntroDatHeader,
        NoIntroDatHeaderChild, NoIntroDatHeaderField, NoIntroDatPage, NoIntroDatPageLimit,
        NoIntroDatRelease, NoIntroDatRomField, NoIntroDatRomReference, games_for_snapshot,
    },
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
    logiqx::AttributeLocation,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(QueryableByName)]
struct FixtureDiagnostic {
    #[diesel(sql_type = Text)]
    message: String,
}

#[test]
fn page_limits_are_checked() {
    assert!(NoIntroDatPageLimit::new(0).is_err());
    assert!(NoIntroDatPageLimit::new(1).is_ok());
    assert!(NoIntroDatPageLimit::new(500).is_ok());
    assert!(NoIntroDatPageLimit::new(501).is_err());
    assert!(NoIntroDatPageLimit::new(usize::MAX).is_err());
}

#[test]
fn source_free_empty_pages_retain_required_header_and_all_four_modes() -> TestResult {
    for mode in [
        NoIntroDatMode::V3Strict,
        NoIntroDatMode::V3Compatible,
        NoIntroDatMode::V4Strict,
        NoIntroDatMode::V4Compatible,
    ] {
        let fixture = PublishedDat::new(&empty_fixture(mode), mode)?;
        fixture.remove_sources()?;
        let page = games_for_snapshot(
            &fixture.database,
            &fixture.snapshot,
            None,
            NoIntroDatPageLimit::new(1)?,
        )?;
        assert_eq!(page.snapshot.snapshot_key, fixture.snapshot);
        assert_eq!(page.snapshot.format, mode.as_str());
        assert_eq!(page.snapshot.source_name, "Flat source");
        assert_eq!(page.snapshot.catalog_name, "Flat catalog");
        assert_eq!(page.snapshot.declared_version.as_deref(), Some("0001"));
        assert_eq!(page.document.mode, mode);
        assert_eq!(page.document.header.source_order, 0);
        assert_eq!(page.document.header.children.len(), 5);
        assert!(
            page.document
                .header
                .children
                .iter()
                .all(|child| matches!(child, NoIntroDatHeaderChild::Text { .. }))
        );
        assert!(page.games.is_empty());
        assert!(page.next_cursor.is_none());
    }
    Ok(())
}

struct PublishedDat {
    _directory: tempfile::TempDir,
    database: Database,
    snapshot: mame_coalesce::domain::SnapshotKey,
    input: Utf8PathBuf,
    object_store: Utf8PathBuf,
}

impl PublishedDat {
    fn new(xml: &str, mode: NoIntroDatMode) -> TestResult<Self> {
        let directory = tempfile::tempdir()?;
        let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
            .map_err(|_| "database path is not UTF-8")?;
        let input = Utf8PathBuf::from_path_buf(directory.path().join("catalog.xml"))
            .map_err(|_| "input path is not UTF-8")?;
        let object_store = Utf8PathBuf::from(format!("{database_path}.documents"));
        std::fs::write(&input, xml)?;
        let database = Database::open(&database_path)?;
        let report = app::import_catalog(
            &database,
            &CatalogImportRequest {
                document_path: input.clone(),
                format: CatalogDocumentFormat::NoIntroDat(mode),
                source_key: PublishingSourceKey::new("flat-source"),
                source_display_name: "Flat source".into(),
                catalog_key: CatalogKey::new("flat-catalog"),
                catalog_display_name: "Flat catalog".into(),
                scope: CatalogScope::Complete,
            },
        )?;
        if report.status != CatalogImportStatus::Succeeded {
            let mut connection = SqliteConnection::establish(database_path.as_str())?;
            let messages = sql_query("SELECT code || ': ' || message AS message FROM import_diagnostics WHERE run_key=? ORDER BY diagnostic_key")
                .bind::<Text, _>(report.run_key.to_string()).load::<FixtureDiagnostic>(&mut connection)?;
            return Err(format!(
                "fixture import failed: {:?}",
                messages.iter().map(|row| &row.message).collect::<Vec<_>>()
            )
            .into());
        }
        let snapshot = report
            .snapshot_key
            .ok_or("successful import has no snapshot")?;
        Ok(Self {
            _directory: directory,
            database,
            snapshot,
            input,
            object_store,
        })
    }

    fn remove_sources(&self) -> TestResult {
        let directory = self.input.parent().ok_or("input has no parent")?;
        std::fs::rename(&self.input, directory.join("original-unavailable.xml"))?;
        std::fs::rename(&self.object_store, directory.join("objects-unavailable"))?;
        Ok(())
    }

    fn page(
        &self,
        limit: usize,
    ) -> TestResult<mame_coalesce::catalog_no_intro_dat::NoIntroDatPage> {
        Ok(games_for_snapshot(
            &self.database,
            &self.snapshot,
            None,
            NoIntroDatPageLimit::new(limit)?,
        )?)
    }
}

fn empty_fixture(mode: NoIntroDatMode) -> String {
    let header = match mode {
        NoIntroDatMode::V3Strict | NoIntroDatMode::V4Strict => {
            "<header><id>1</id><name>Empty</name><description></description>\
             <version>0001</version><author></author></header>"
        }
        NoIntroDatMode::V3Compatible | NoIntroDatMode::V4Compatible => {
            "<header><id>1</id><vendor:gap/><name>Empty</name><description></description>\
             <version>0001</version><author></author></header>"
        }
    };
    if mode == NoIntroDatMode::V3Compatible || mode == NoIntroDatMode::V4Compatible {
        format!("<datafile xmlns:vendor='urn:vendor'>{header}</datafile>")
    } else {
        format!("<datafile>{header}</datafile>")
    }
}

#[test]
fn compatible_public_page_maps_all_native_metadata_and_exact_positions() -> TestResult {
    let xml = all_fields_fixture();
    let fixture = PublishedDat::new(&xml, NoIntroDatMode::V4Compatible)?;
    fixture.remove_sources()?;
    let page = fixture.page(10)?;

    assert_eq!(
        page.document.schema_location.as_deref(),
        Some("urn:dat catalog.xsd")
    );
    assert_eq!(page.document.location, xml_location(&xml, "<datafile")?);
    assert_eq!(page.document.header.source_order, 0);
    assert_eq!(
        page.document.header.location,
        xml_location(&xml, "<header")?
    );
    assert_eq!(
        page.snapshot.declared_version.as_deref(),
        Some(" version-raw ")
    );
    check_header_fields(&page, &xml)?;
    check_header_directives(&page.document.header, &xml)?;
    check_game_metadata(&page.games, &xml)?;
    let game = page.games.first().ok_or("missing metadata game")?;
    check_releases(game, &xml)?;
    check_rom_metadata(game, &xml)?;
    Ok(())
}

fn check_header_fields(page: &NoIntroDatPage, xml: &str) -> TestResult {
    let header_text: Vec<_> = page
        .document
        .header
        .children
        .iter()
        .filter_map(|child| match child {
            NoIntroDatHeaderChild::Text { field, value } => Some((*field, value)),
            _ => None,
        })
        .collect();
    assert_eq!(
        header_text
            .iter()
            .map(|(field, _)| *field)
            .collect::<Vec<_>>(),
        [
            NoIntroDatHeaderField::Id,
            NoIntroDatHeaderField::Name,
            NoIntroDatHeaderField::Description,
            NoIntroDatHeaderField::Version,
            NoIntroDatHeaderField::Date,
            NoIntroDatHeaderField::Author,
            NoIntroDatHeaderField::Homepage,
            NoIntroDatHeaderField::Url,
            NoIntroDatHeaderField::Trademarks,
            NoIntroDatHeaderField::Piracy,
            NoIntroDatHeaderField::Subset,
            NoIntroDatHeaderField::Comment,
        ]
    );
    let header_values = [
        "0001",
        " Header name ",
        "Header café",
        " version-raw ",
        "2025",
        "Author",
        "https://example.test",
        "url-literal",
        "Marks",
        "Piracy",
        "Subset",
        "Note",
    ];
    for ((_, value), expected) in header_text.iter().zip(header_values) {
        assert_eq!(value.value, expected);
    }
    assert_eq!(
        header_text
            .iter()
            .map(|(_, value)| value.source_order)
            .collect::<Vec<_>>(),
        [0, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]
    );
    assert_eq!(
        page.document
            .header
            .children
            .iter()
            .map(NoIntroDatHeaderChild::source_order)
            .collect::<Vec<_>>(),
        [0, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]
    );
    for (field, value) in &header_text {
        assert_eq!(
            value.location,
            xml_location(xml, &format!("<{}", header_field_tag(*field)))?
        );
    }

    Ok(())
}

fn check_header_directives(header: &NoIntroDatHeader, xml: &str) -> TestResult {
    let clrmamepro = header
        .children
        .iter()
        .find_map(|child| match child {
            NoIntroDatHeaderChild::ClrMamePro(value) => Some(value),
            _ => None,
        })
        .ok_or("missing clrmamepro directive")?;
    assert_eq!(clrmamepro.source_order, 13);
    assert_eq!(clrmamepro.location, xml_location(xml, "<clrmamepro")?);
    assert_declared(
        clrmamepro
            .forcenodump
            .as_ref()
            .ok_or("missing forcenodump")?,
        " unknown-token ",
        0,
        xml,
        "<clrmamepro",
        "forcenodump=",
    )?;
    assert!(clrmamepro.header.is_none());
    assert_eq!(clrmamepro.forcenodump_effective, None);
    let romcenter = header
        .children
        .iter()
        .find_map(|child| match child {
            NoIntroDatHeaderChild::RomCenter(value) => Some(value),
            _ => None,
        })
        .ok_or("missing romcenter directive")?;
    assert_eq!(romcenter.source_order, 14);
    assert_declared(
        romcenter.plugin.as_ref().ok_or("missing plugin")?,
        "",
        0,
        xml,
        "<romcenter",
        "plugin=",
    )?;

    Ok(())
}

fn check_game_metadata(games: &[NoIntroDatGame], xml: &str) -> TestResult {
    assert_eq!(games.len(), 2);
    let [game, second_game] = games else {
        return Err("expected two duplicate-name game records".into());
    };
    assert_eq!(game.list_order, 0);
    assert_eq!(game.source_order, 2);
    assert_eq!(second_game.source_order, 3);
    assert_eq!(game.location, xml_location(xml, "<game ")?);
    check_game_attributes(game, xml)?;
    check_duplicate_game(game, second_game, xml)?;
    let description = game
        .children
        .iter()
        .find_map(|child| match child {
            NoIntroDatGameChild::Description(value) => Some(value),
            _ => None,
        })
        .ok_or("missing game description")?;
    assert_eq!(description.value, "Game description");
    assert_eq!(description.source_order, 0);
    assert_eq!(
        description.location,
        xml_location_after(xml, "<game ", "<description>")?
    );

    check_repeated_game_text(game);
    Ok(())
}

fn check_game_attributes(game: &NoIntroDatGame, xml: &str) -> TestResult {
    assert_declared(&game.name, " duplicate-name ", 1, xml, "<game ", "name=")?;
    assert_declared(
        game.publisher_id
            .as_ref()
            .ok_or("missing game id attribute")?,
        "0007",
        2,
        xml,
        "<game ",
        "id=",
    )?;
    assert_declared(
        &game.cloneof.as_ref().ok_or("missing cloneof")?.target,
        " Parent literal ",
        3,
        xml,
        "<game ",
        "cloneof=",
    )?;
    assert!(
        game.cloneof
            .as_ref()
            .ok_or("missing cloneof")?
            .relationship_id
            > 0
    );
    assert_declared(
        &game.cloneofid.as_ref().ok_or("missing cloneofid")?.target,
        "0003",
        4,
        xml,
        "<game ",
        "cloneofid=",
    )?;
    assert!(
        game.cloneofid
            .as_ref()
            .ok_or("missing cloneofid")?
            .relationship_id
            > 0
    );
    Ok(())
}

fn check_duplicate_game(
    game: &NoIntroDatGame,
    second_game: &NoIntroDatGame,
    xml: &str,
) -> TestResult {
    assert_declared(
        &second_game.name,
        " duplicate-name ",
        0,
        xml,
        "<game name=' duplicate-name '",
        "name=",
    )?;
    assert_declared(
        second_game
            .publisher_id
            .as_ref()
            .ok_or("second game id missing")?,
        "0007",
        1,
        xml,
        "<game name=' duplicate-name '",
        "id=",
    )?;
    assert_ne!(game.id, second_game.id);
    assert_eq!(game.name.value, second_game.name.value);
    assert_eq!(
        game.publisher_id.as_ref().map(|value| value.value.as_str()),
        second_game
            .publisher_id
            .as_ref()
            .map(|value| value.value.as_str())
    );
    assert_eq!(game.list_order, 0);
    assert_eq!(second_game.list_order, 1);
    Ok(())
}

fn check_repeated_game_text(game: &NoIntroDatGame) {
    assert_eq!(
        game.children
            .iter()
            .map(NoIntroDatGameChild::source_order)
            .collect::<Vec<_>>(),
        [0, 2, 3, 4, 5, 6, 7, 8, 9]
    );
    let categories: Vec<_> = game
        .children
        .iter()
        .filter_map(|child| match child {
            NoIntroDatGameChild::Category(value) => Some(value),
            _ => None,
        })
        .collect();
    assert_eq!(
        categories
            .iter()
            .map(|value| value.family_order)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert_eq!(
        categories
            .iter()
            .map(|value| value.value.source_order)
            .collect::<Vec<_>>(),
        [2, 3]
    );
    assert_eq!(
        categories
            .iter()
            .map(|value| value.value.value.as_str())
            .collect::<Vec<_>>(),
        ["", " arcade "]
    );
    let identifiers: Vec<_> = game
        .children
        .iter()
        .filter_map(|child| match child {
            NoIntroDatGameChild::Identifier(value) => Some(value),
            _ => None,
        })
        .collect();
    assert_eq!(
        identifiers
            .iter()
            .map(|value| value.family_order)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert_eq!(
        identifiers
            .iter()
            .map(|value| value.value.source_order)
            .collect::<Vec<_>>(),
        [4, 5]
    );
    assert_eq!(
        identifiers
            .iter()
            .map(|value| value.value.value.as_str())
            .collect::<Vec<_>>(),
        ["0007", "0007"]
    );
}

fn check_releases(game: &NoIntroDatGame, xml: &str) -> TestResult {
    let releases: Vec<_> = game
        .children
        .iter()
        .filter_map(|child| match child {
            NoIntroDatGameChild::Release(value) => Some(value),
            _ => None,
        })
        .collect();
    assert_eq!(releases.len(), 2);
    let [first_release, second_release] = releases.as_slice() else {
        return Err("expected two releases".into());
    };
    assert_eq!(
        releases
            .iter()
            .map(|release| release.key.game_id())
            .collect::<Vec<_>>(),
        [game.id, game.id]
    );
    assert_eq!(
        releases
            .iter()
            .map(|release| release.key.release_order())
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert_eq!(
        releases
            .iter()
            .map(|release| (release.name.value.as_str(), release.region.value.as_str()))
            .collect::<Vec<_>>(),
        [("Release A", "Region A"), ("", "Region B")]
    );
    assert_eq!(
        releases
            .iter()
            .map(|release| release.source_order)
            .collect::<Vec<_>>(),
        [6, 8]
    );
    assert_eq!(
        releases
            .iter()
            .map(|release| release.location)
            .collect::<Vec<_>>(),
        [
            xml_location(xml, "<release name='Release A'")?,
            xml_location(xml, "<release name=''")?,
        ]
    );
    check_release_attributes(
        first_release,
        "Release A",
        "Region A",
        "<release name='Release A'",
        xml,
    )?;
    check_release_attributes(second_release, "", "Region B", "<release name=''", xml)?;
    Ok(())
}

fn check_release_attributes(
    release: &NoIntroDatRelease,
    name: &str,
    region: &str,
    owner: &str,
    xml: &str,
) -> TestResult {
    assert_declared(&release.name, name, 0, xml, owner, "name=")?;
    assert_declared(&release.region, region, 1, xml, owner, "region=")?;
    Ok(())
}

fn check_rom_metadata(game: &NoIntroDatGame, xml: &str) -> TestResult {
    let roms: Vec<_> = game
        .children
        .iter()
        .filter_map(|child| match child {
            NoIntroDatGameChild::Rom(value) => Some(value),
            _ => None,
        })
        .collect();
    assert_eq!(roms.len(), 2);
    let rom = roms.first().ok_or("missing metadata ROM")?;
    assert_eq!(rom.occurrence_order, 0);
    assert_eq!(rom.source_order, 7);
    assert_eq!(rom.location, xml_location(xml, "<rom v:before")?);
    check_rom_payload(rom);
    check_rom_positions(rom, xml)?;
    check_whole_file_rom(game)?;
    Ok(())
}

fn check_rom_payload(rom: &NoIntroDatRomReference) {
    assert_eq!(rom.payload.name, "rom-name");
    assert_eq!(rom.payload.size_text.as_deref(), Some(" +4294967296 "));
    assert_eq!(rom.payload.size, Some(4_294_967_296));
    assert_eq!(rom.payload.evidence_scope, NoIntroDatEvidenceScope::Unknown);
    assert_eq!(rom.payload.crc_text.as_deref(), Some("abcdef01"));
    assert_eq!(rom.payload.md5_text.as_deref(), Some("not a digest"));
    assert_eq!(
        rom.payload.sha1_text.as_deref(),
        Some("0123456789abcdef0123456789abcdef01234567")
    );
    assert_eq!(rom.payload.sha256_text.as_deref(), Some(""));
    assert_eq!(rom.payload.status_text.as_deref(), Some("custom status"));
    assert_eq!(rom.payload.serial_text.as_deref(), Some(""));
    assert_eq!(rom.payload.header_text.as_deref(), Some("filter"));
    assert_eq!(rom.payload.date_text.as_deref(), Some("2024-02"));
    assert_eq!(rom.payload.mia_text.as_deref(), Some("yes"));
}

fn check_rom_positions(rom: &NoIntroDatRomReference, xml: &str) -> TestResult {
    assert_eq!(
        rom.attribute_positions
            .iter()
            .map(|position| position.field)
            .collect::<Vec<_>>(),
        [
            NoIntroDatRomField::Name,
            NoIntroDatRomField::Size,
            NoIntroDatRomField::Crc,
            NoIntroDatRomField::Md5,
            NoIntroDatRomField::Sha1,
            NoIntroDatRomField::Sha256,
            NoIntroDatRomField::Status,
            NoIntroDatRomField::Serial,
            NoIntroDatRomField::Header,
            NoIntroDatRomField::Date,
            NoIntroDatRomField::Mia,
        ]
    );
    assert_eq!(
        rom.attribute_positions
            .iter()
            .map(|position| position.source_order)
            .collect::<Vec<_>>(),
        (1..=11).collect::<Vec<_>>()
    );
    for (position, attribute) in rom.attribute_positions.iter().zip([
        "name=", "size=", "crc=", "md5=", "sha1=", "sha256=", "status=", "serial=", "header=",
        "date=", "mia=",
    ]) {
        assert_eq!(
            position.location,
            xml_attribute_location(xml, "<rom v:before", attribute)?
        );
    }

    Ok(())
}

fn check_whole_file_rom(game: &NoIntroDatGame) -> TestResult {
    let whole_file = game
        .children
        .iter()
        .find_map(|child| match child {
            NoIntroDatGameChild::Rom(value) if value.payload.name == "whole.bin" => Some(value),
            _ => None,
        })
        .ok_or("missing whole-file ROM")?;
    assert_eq!(
        whole_file.payload.evidence_scope,
        NoIntroDatEvidenceScope::WholeFile
    );
    assert_eq!(whole_file.payload.size, None);
    assert_eq!(whole_file.payload.size_text, None);
    assert_eq!(whole_file.payload.crc_text.as_deref(), Some("12345678"));
    assert_eq!(
        whole_file.payload.md5_text.as_deref(),
        Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
    );
    Ok(())
}

#[test]
fn directives_distinguish_absence_empty_and_virtual_defaults() -> TestResult {
    let cases = [
        ("", false, false, None, None, None),
        (
            "<clrmamepro/><romcenter/>",
            true,
            true,
            None,
            None,
            Some(NoIntroDatForceNoDump::Obsolete),
        ),
        (
            "<clrmamepro forcenodump='' header=''/><romcenter plugin='' />",
            true,
            true,
            Some(""),
            Some(""),
            None,
        ),
        (
            "<clrmamepro forcenodump='required'/><romcenter plugin='plug'/>",
            true,
            true,
            Some("required"),
            Some("plug"),
            Some(NoIntroDatForceNoDump::Required),
        ),
    ];
    for (body, has_clrmamepro, has_romcenter, declared_force, declared_plugin, effective_force) in
        cases
    {
        let xml = format!(
            "<datafile>{}<game name='g'><description/><rom name='g.bin'/></game></datafile>",
            header_with_directives(body)
        );
        let fixture = PublishedDat::new(&xml, NoIntroDatMode::V4Compatible)?;
        let page = fixture.page(1)?;
        let clrmamepro = page
            .document
            .header
            .children
            .iter()
            .find_map(|child| match child {
                NoIntroDatHeaderChild::ClrMamePro(value) => Some(value),
                _ => None,
            });
        let romcenter = page
            .document
            .header
            .children
            .iter()
            .find_map(|child| match child {
                NoIntroDatHeaderChild::RomCenter(value) => Some(value),
                _ => None,
            });
        assert_eq!(clrmamepro.is_some(), has_clrmamepro);
        assert_eq!(romcenter.is_some(), has_romcenter);
        if let Some(value) = clrmamepro {
            assert_eq!(
                value.forcenodump.as_ref().map(|text| text.value.as_str()),
                declared_force
            );
            let declared_header = if body.contains("header=") {
                Some("")
            } else {
                None
            };
            assert_eq!(
                value.header.as_ref().map(|text| text.value.as_str()),
                declared_header
            );
            assert_eq!(value.forcenodump_effective, effective_force);
        }
        if let Some(value) = romcenter {
            assert_eq!(
                value.plugin.as_ref().map(|text| text.value.as_str()),
                declared_plugin
            );
        }
    }
    Ok(())
}

#[test]
fn compatible_present_empty_header_fields_are_distinct_from_absence() -> TestResult {
    let fixture = PublishedDat::new(
        "<datafile><header><id>0001</id><name/><description/><version/></header></datafile>",
        NoIntroDatMode::V4Compatible,
    )?;
    fixture.remove_sources()?;
    let page = fixture.page(1)?;
    assert_eq!(page.document.header.source_order, 0);
    assert_eq!(page.document.header.children.len(), 4);
    assert_eq!(page.document.header.children.iter().filter(|child| matches!(child, NoIntroDatHeaderChild::Text { value, .. } if value.value.is_empty())).count(), 3);
    assert!(page.games.is_empty());
    Ok(())
}

#[test]
fn embedded_payload_preserves_size_and_digest_declarations_without_scope_inference() -> TestResult {
    let xml = format!(
        "<datafile>{}<game name='sizes'><description/>\
         <rom name='missing-size' crc='12345678'/>\
         <rom name='empty-size' size='' crc='' md5=''/>\
         <rom name='invalid-size' size='twelve' md5='not a digest'/>\
         <rom name='oversized-size' size='9223372036854775808' \
         sha1='0123456789ABCDEF0123456789ABCDEF01234567'/>\
         </game></datafile>",
        strict_header(),
    );
    let fixture = PublishedDat::new(&xml, NoIntroDatMode::V4Compatible)?;
    let page = fixture.page(1)?;
    let game = page.games.first().ok_or("missing game")?;
    let payloads: Vec<_> = game
        .children
        .iter()
        .filter_map(|child| match child {
            NoIntroDatGameChild::Rom(rom) => Some(&rom.payload),
            _ => None,
        })
        .collect();
    assert_eq!(payloads.len(), 4);
    assert_eq!(
        payloads
            .iter()
            .map(|payload| payload.name.as_str())
            .collect::<Vec<_>>(),
        [
            "missing-size",
            "empty-size",
            "invalid-size",
            "oversized-size",
        ]
    );
    assert_eq!(
        payloads
            .iter()
            .map(|payload| payload.size_text.as_deref())
            .collect::<Vec<_>>(),
        [None, Some(""), Some("twelve"), Some("9223372036854775808"),]
    );
    assert_eq!(
        payloads
            .iter()
            .map(|payload| payload.size)
            .collect::<Vec<_>>(),
        [None, None, None, None,]
    );
    let mut payload_iter = payloads.iter();
    let missing_size = payload_iter.next().ok_or("missing size case")?;
    let empty_size = payload_iter.next().ok_or("empty size case missing")?;
    let invalid_size = payload_iter.next().ok_or("invalid size case missing")?;
    let oversized_size = payload_iter.next().ok_or("oversized size case missing")?;
    assert_eq!(missing_size.crc_text.as_deref(), Some("12345678"));
    assert_eq!(missing_size.md5_text, None);
    assert_eq!(empty_size.crc_text.as_deref(), Some(""));
    assert_eq!(empty_size.md5_text.as_deref(), Some(""));
    assert_eq!(invalid_size.md5_text.as_deref(), Some("not a digest"));
    assert_eq!(
        oversized_size.sha1_text.as_deref(),
        Some("0123456789abcdef0123456789abcdef01234567")
    );
    assert!(
        payloads
            .iter()
            .all(|payload| payload.evidence_scope == NoIntroDatEvidenceScope::WholeFile)
    );
    Ok(())
}

#[test]
fn empty_local_or_global_header_filters_are_unknown_and_absent_filters_are_whole_file() -> TestResult
{
    let cases = [
        (
            "<clrmamepro/><romcenter/>",
            "<rom name='local' header='' sha1='0123456789ABCDEF0123456789ABCDEF01234567'/>",
            NoIntroDatEvidenceScope::Unknown,
        ),
        (
            "<clrmamepro header=''/><romcenter/>",
            "<rom name='global' sha1='0123456789ABCDEF0123456789ABCDEF01234567'/>",
            NoIntroDatEvidenceScope::Unknown,
        ),
        (
            "<clrmamepro/><romcenter/>",
            "<rom name='unfiltered' sha1='0123456789ABCDEF0123456789ABCDEF01234567'/>",
            NoIntroDatEvidenceScope::WholeFile,
        ),
    ];
    for (directives, rom, expected_scope) in cases {
        let xml = format!(
            "<datafile>{}<game name='scope'><description/>{rom}</game></datafile>",
            header_with_directives(directives),
        );
        let fixture = PublishedDat::new(&xml, NoIntroDatMode::V4Compatible)?;
        let page = fixture.page(1)?;
        let game = page.games.first().ok_or("missing scope fixture game")?;
        let payload = game
            .children
            .iter()
            .find_map(|child| match child {
                NoIntroDatGameChild::Rom(value) => Some(&value.payload),
                _ => None,
            })
            .ok_or("missing ROM payload")?;
        assert_eq!(payload.evidence_scope, expected_scope);
        assert_eq!(
            payload.sha1_text.as_deref(),
            Some("0123456789abcdef0123456789abcdef01234567")
        );
    }
    Ok(())
}

#[test]
fn a_selected_game_returns_every_rom_across_the_bind_batch_boundary() -> TestResult {
    let mut roms = String::new();
    for index in 0..401 {
        write!(roms, "<rom name='rom-{index}' size='1' crc='12345678'/>")?;
    }
    let xml = format!(
        "<datafile>{}<game name='large'><description/>{roms}</game></datafile>",
        strict_header()
    );
    let fixture = PublishedDat::new(&xml, NoIntroDatMode::V4Compatible)?;
    let page = fixture.page(1)?;
    assert_eq!(page.games.len(), 1);
    let game = page.games.first().ok_or("missing large fixture game")?;
    let roms: Vec<_> = game
        .children
        .iter()
        .filter_map(|child| match child {
            NoIntroDatGameChild::Rom(rom) => Some(rom),
            _ => None,
        })
        .collect();
    assert_eq!(roms.len(), 401);
    assert_eq!(roms.first().map(|rom| rom.occurrence_order), Some(0));
    assert_eq!(roms.last().map(|rom| rom.occurrence_order), Some(400));
    Ok(())
}

fn all_fields_fixture() -> String {
    "<datafile xmlns:v='urn:vendor' xmlns:xsi='http://www.w3.org/2001/XMLSchema-instance' \
     xsi:schemaLocation='urn:dat catalog.xsd' v:root='root'>\
     <header\tv:before='h'><id>0001</id><v:gap/><name> Header name </name>\
     <description>Header café</description><version> version-raw </version>\
     <date>2025</date><author>Author</author><homepage>https://example.test</homepage>\
     <url>url-literal</url><trademarks>Marks</trademarks><piracy>Piracy</piracy>\
     <subset>Subset</subset><comment>Note</comment>\
     <clrmamepro forcenodump=' unknown-token '/><romcenter plugin=''/></header>\
     <v:root-gap/><game v:before='g' name=' duplicate-name ' id='0007' \
     cloneof=' Parent literal ' cloneofid='0003'><description>Game description</description>\
     <v:child-gap/><category/><category> arcade </category><game_id>0007</game_id>\
     <game_id>0007</game_id><release name='Release A' region='Region A'/>\
     <rom v:before='café'\tname='rom-name' size=' +4294967296 ' crc='ABCDEF01' \
     md5='not a digest' sha1='0123456789ABCDEF0123456789ABCDEF01234567' sha256='' \
     status='custom status' serial='' header='filter' date='2024-02' mia='yes'/>\
     <release name='' region='Region B'/><rom name='whole.bin' crc='12345678' \
     md5='aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'/></game>\
     <game name=' duplicate-name ' id='0007'><description>Second game</description><rom name='second.bin'/></game></datafile>".into()
}

const fn strict_header() -> &'static str {
    "<header><id>1</id><name>Catalog</name><description>Catalog description</description>\
     <version>1</version><author>Publisher</author></header>"
}

fn header_with_directives(directives: &str) -> String {
    format!(
        "<header><id>1</id><name>Catalog</name><description>About</description>\
             <version>1</version><author>Publisher</author>{directives}</header>"
    )
}

fn xml_location(xml: &str, marker: &str) -> TestResult<mame_coalesce::logiqx::RecordLocation> {
    let (line, column) = marker_position(xml, marker)?;
    Ok(mame_coalesce::logiqx::RecordLocation { line, column })
}

fn xml_attribute_location(
    xml: &str,
    owner: &str,
    attribute: &str,
) -> TestResult<AttributeLocation> {
    let owner_start = xml.find(owner).ok_or("owner marker is absent")?;
    let suffix = xml.get(owner_start..).ok_or("invalid owner offset")?;
    let tag_end = suffix.find('>').ok_or("opening tag is not closed")?;
    let opening_tag = suffix
        .get(..tag_end)
        .ok_or("invalid opening tag boundary")?;
    let local_offset = opening_tag
        .find(attribute)
        .ok_or("attribute marker is absent")?;
    let attr_offset = owner_start
        .checked_add(local_offset)
        .ok_or("attribute offset overflow")?;
    let (line, column) = position_at(xml, attr_offset)?;
    Ok(AttributeLocation { line, column })
}

fn marker_position(xml: &str, marker: &str) -> TestResult<(i64, i64)> {
    let offset = xml.find(marker).ok_or("marker is absent")?;
    position_at(xml, offset)
}

fn position_at(xml: &str, offset: usize) -> TestResult<(i64, i64)> {
    let before = xml.get(..offset).ok_or("invalid XML position boundary")?;
    let line = i64::try_from(before.bytes().filter(|byte| *byte == b'\n').count())?
        .checked_add(1)
        .ok_or("line coordinate overflow")?;
    let last_line = before.rsplit('\n').next().ok_or("last line is absent")?;
    let column = i64::try_from(last_line.chars().count())?
        .checked_add(1)
        .ok_or("column coordinate overflow")?;
    Ok((line, column))
}

fn xml_location_after(
    xml: &str,
    owner: &str,
    marker: &str,
) -> TestResult<mame_coalesce::logiqx::RecordLocation> {
    let owner_offset = xml.find(owner).ok_or("owner marker is absent")?;
    let suffix = xml.get(owner_offset..).ok_or("invalid owner offset")?;
    let local_offset = suffix.find(marker).ok_or("field marker is absent")?;
    let marker_offset = owner_offset
        .checked_add(local_offset)
        .ok_or("field offset overflow")?;
    let (line, column) = position_at(xml, marker_offset)?;
    Ok(mame_coalesce::logiqx::RecordLocation { line, column })
}

fn assert_declared(
    value: &mame_coalesce::catalog_no_intro_dat::DeclaredText,
    expected: &str,
    source_order: usize,
    xml: &str,
    owner: &str,
    marker: &str,
) -> TestResult {
    assert_eq!(value.value, expected);
    assert_eq!(value.source_order, source_order);
    assert_eq!(value.location, xml_location_after(xml, owner, marker)?);
    Ok(())
}

const fn header_field_tag(field: NoIntroDatHeaderField) -> &'static str {
    match field {
        NoIntroDatHeaderField::Id => "id",
        NoIntroDatHeaderField::Name => "name",
        NoIntroDatHeaderField::Description => "description",
        NoIntroDatHeaderField::Version => "version",
        NoIntroDatHeaderField::Date => "date",
        NoIntroDatHeaderField::Author => "author",
        NoIntroDatHeaderField::Homepage => "homepage",
        NoIntroDatHeaderField::Url => "url",
        NoIntroDatHeaderField::Trademarks => "trademarks",
        NoIntroDatHeaderField::Piracy => "piracy",
        NoIntroDatHeaderField::Subset => "subset",
        NoIntroDatHeaderField::Comment => "comment",
    }
}
