use std::{cell::Cell, fmt::Write as _};

use mame_coalesce::logiqx::{self, DataFile, LogiqxMode};

#[path = "support/logiqx_dtd15.rs"]
mod dtd_fixture;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn strict_read(xml: &str) -> mame_coalesce::Result<usize> {
    let (_, games) = logiqx::read_with_mode(
        xml.as_bytes(),
        LogiqxMode::StrictDtd15,
        |_| Ok::<_, mame_coalesce::Error>(0),
        |games, _| {
            *games += 1;
            Ok(())
        },
    )?
    .into_parts();
    Ok(games)
}

#[test]
fn strict_root_requires_games_and_header_before_them() -> TestResult {
    for xml in [
        "<datafile/>",
        "<datafile></datafile>",
        "<datafile><header><name/><description/><version/><author/></header></datafile>",
        "<datafile><game name='ok'><description/></game><header><name/><description/><version/><author/></header></datafile>",
    ] {
        assert!(strict_read(xml).is_err(), "strict root accepted {xml}");
    }
    assert_eq!(
        strict_read("<datafile><game name='ok'><description/></game></datafile>")?,
        1
    );
    Ok(())
}

#[test]
fn strict_game_requires_description_even_without_media() -> TestResult {
    for xml in [
        "<datafile><game name='ok'/></datafile>",
        "<datafile><game name='ok'><year>1980</year></game></datafile>",
        "<datafile><game name='ok'><rom name='a' size='1'/></game></datafile>",
    ] {
        assert!(
            strict_read(xml).is_err(),
            "missing description accepted {xml}"
        );
    }
    let data = DataFile::from_reader_with_mode(
        "<datafile><game name=''><description/></game></datafile>".as_bytes(),
        LogiqxMode::StrictDtd15,
    )?;
    assert_eq!(data.games().len(), 1);
    assert_eq!(data.games().first().ok_or("missing game")?.name(), "");
    Ok(())
}

#[test]
fn compatibility_remains_explicitly_permissive() -> TestResult {
    for xml in [
        "<datafile/>",
        "<datafile><game name='sparse'/></datafile>",
        "<datafile><game name='late'/><header><name>late</name></header></datafile>",
        "<datafile><game name='vendor'><device_ref name='helper'/></game></datafile>",
    ] {
        let ordinary = DataFile::from_reader(xml.as_bytes())?;
        let explicit =
            DataFile::from_reader_with_mode(xml.as_bytes(), LogiqxMode::ObservedCompatible)?;
        assert_eq!(ordinary.games().len(), explicit.games().len());
        assert!(
            strict_read(xml).is_err(),
            "strict accepted compatibility-only input {xml}"
        );
    }
    Ok(())
}

#[test]
fn completed_strict_games_do_not_bypass_later_root_failure() {
    let calls = Cell::new(0);
    let result = logiqx::read_with_mode(
        b"<datafile><game name='written'><description/></game><header><name/><description/><version/><author/></header></datafile>",
        LogiqxMode::StrictDtd15,
        |_| Ok::<_, mame_coalesce::Error>(()),
        |(), _| {
            calls.set(calls.get() + 1);
            Ok(())
        },
    );
    assert_eq!(
        calls.get(),
        1,
        "first valid game must stream before the later root error"
    );
    assert!(
        result.is_err(),
        "no valid EOF proof after out-of-order header"
    );
}

fn header_document(children: &str) -> String {
    format!("<datafile><header>{children}</header><game name='ok'><description/></game></datafile>")
}

fn game_document(children: &str) -> String {
    format!("<datafile><game name='ok'>{children}</game></datafile>")
}

#[test]
fn header_sequence_has_four_required_pcdata_fields_and_singletons() -> TestResult {
    let required = ["name", "description", "version", "author"];
    for omitted in required {
        let mut fields = String::new();
        for name in required.iter().filter(|name| **name != omitted) {
            write!(fields, "<{name}/>")?;
        }
        assert!(
            strict_read(&header_document(&fields)).is_err(),
            "missing {omitted}"
        );
    }
    for children in [
        "<description/><name/><version/><author/>",
        "<name/><description/><author/><version/>",
        "<name/><name/><description/><version/><author/>",
        "<name/><description/><version/><author/><category/>",
        "<name/><description/><version/><author/><romcenter/><clrmamepro/>",
        "<name/><description/><version/><author/><clrmamepro/><clrmamepro/>",
        "<name/><description/><version/><author/><romcenter/><romcenter/>",
    ] {
        assert!(
            strict_read(&header_document(children)).is_err(),
            "header order: {children}"
        );
    }
    assert_eq!(
        strict_read(&header_document(
            "<name/><description/><category/><version/><date/><author/><email/><homepage/><url/><comment/><clrmamepro/><romcenter/>",
        ))?,
        1
    );
    Ok(())
}

#[test]
fn game_child_sequence_preserves_distinct_repeated_families() -> TestResult {
    let repeated = concat!(
        "<comment/><comment/><description/><year/><manufacturer/>",
        "<release name='' region=''/><release name='' region=''/>",
        "<biosset name='' description=''/><biosset name='' description=''/>",
        "<rom name='' size=''/><rom name='' size='bad'/>",
        "<disk name=''/><disk name=''/><sample name=''/><sample name=''/>",
        "<archive name=''/><archive name=''/>",
    );
    assert_eq!(strict_read(&game_document(repeated))?, 1);
    for children in [
        "<description/><comment/>",
        "<year/><description/>",
        "<description/><manufacturer/><year/>",
        "<description/><description/>",
        "<description/><year/><year/>",
        "<description/><manufacturer/><manufacturer/>",
        "<description/><biosset name='' description=''/><release name='' region=''/>",
        "<description/><rom name='' size=''/><biosset name='' description=''/>",
        "<description/><disk name=''/><rom name='' size=''/>",
        "<description/><sample name=''/><disk name=''/>",
        "<description/><archive name=''/><sample name=''/>",
    ] {
        assert!(
            strict_read(&game_document(children)).is_err(),
            "game order: {children}"
        );
    }
    Ok(())
}

#[test]
fn required_cdata_means_present_not_nonempty_or_valid_digest() -> TestResult {
    for (element, attributes) in [
        ("release", &["name", "region"][..]),
        ("biosset", &["name", "description"][..]),
        ("rom", &["name", "size"][..]),
        ("disk", &["name"][..]),
        ("sample", &["name"][..]),
        ("archive", &["name"][..]),
    ] {
        for omitted in attributes {
            let mut fields = String::new();
            for name in attributes.iter().filter(|name| *name != omitted) {
                write!(fields, " {name}=''")?;
            }
            let child = format!("<description/><{element}{fields}/>");
            assert!(
                strict_read(&game_document(&child)).is_err(),
                "{element} lacks {omitted}"
            );
        }
        let mut fields = String::new();
        for name in attributes {
            write!(fields, " {name}=''")?;
        }
        assert_eq!(
            strict_read(&game_document(&format!(
                "<description/><{element}{fields}/>"
            )))?,
            1
        );
    }
    assert_eq!(
        strict_read(&game_document(
            "<description/><rom name='a' size='invalid' crc='bad' md5='' sha1='also-bad'/><disk name='d' md5='bad' sha1=''/>",
        ))?,
        1
    );
    Ok(())
}

#[test]
fn empty_and_pcdata_content_models_do_not_ignore_nested_data() -> TestResult {
    for child in [
        "<rom name='' size=''> </rom>",
        "<rom name='' size=''>text</rom>",
        "<rom name='' size=''><sample name=''/></rom>",
        "<rom name='' size=''><!----></rom>",
        "<rom name='' size=''><?inside ok?></rom>",
        "<rom name='' size=''><![CDATA[]]></rom>",
        "<disk name=''><![CDATA[ ]]></disk>",
        "<release name='' region=''> </release>",
        "<biosset name='' description=''><description/></biosset>",
        "<sample name=''>text</sample>",
        "<archive name=''><rom name='' size=''/></archive>",
    ] {
        assert!(
            strict_read(&game_document(&format!("<description/>{child}"))).is_err(),
            "EMPTY: {child}"
        );
    }
    for children in [
        "<description><year/></description>",
        "<comment><sample name=''/></comment><description/>",
        "<description/><year><description/></year>",
        "<description/><manufacturer><rom name='' size=''/></manufacturer>",
    ] {
        assert!(
            strict_read(&game_document(children)).is_err(),
            "PCDATA: {children}"
        );
    }
    assert_eq!(
        strict_read(&game_document(
            "<!--before--><description>a<![CDATA[<&]]><?inside ok?>b</description><!--after--><rom name='' size=''></rom>",
        ))?,
        1
    );
    Ok(())
}

#[test]
fn element_only_owners_reject_character_data_but_accept_xml_space() -> TestResult {
    for xml in [
        "<datafile>text<game name='ok'><description/></game></datafile>",
        "<datafile><![CDATA[text]]><game name='ok'><description/></game></datafile>",
        "<datafile><![CDATA[ ]]><game name='ok'><description/></game></datafile>",
        "<datafile>&#32;<game name='ok'><description/></game></datafile>",
        "<datafile><header>text<name/><description/><version/><author/></header><game name='ok'><description/></game></datafile>",
        "<datafile><game name='ok'>text<description/></game></datafile>",
        "<datafile><game name='ok'><![CDATA[ ]]><description/></game></datafile>",
        "<datafile><game name='ok'>&#32;<description/></game></datafile>",
        "<datafile><game name='ok'>&#xA0;<description/></game></datafile>",
    ] {
        assert!(strict_read(xml).is_err(), "element-only content: {xml}");
    }
    assert_eq!(
        strict_read(
            "<datafile> \t\n\r<game name='ok'> \t\n\r<description/></game> \t\n\r</datafile>"
        )?,
        1
    );
    Ok(())
}

#[test]
fn strict_unknown_names_and_compatibility_fields_are_not_silently_skipped() {
    for xml in [
        "<datafile vendor='v'><game name='ok'><description/></game></datafile>",
        "<datafile><header vendor='v'><name/><description/><version/><author/></header><game name='ok'><description/></game></datafile>",
        "<datafile><game name='ok' vendor='v'><description/></game></datafile>",
        "<datafile><game name='ok'><description vendor='v'/></game></datafile>",
        "<datafile><game name='ok'><description/><rom name='' size='' serial='compat'/></game></datafile>",
        "<datafile><game name='ok'><description/><device_ref name='compat'/></game></datafile>",
        "<datafile><game name='ok'><description/></game><file_name>compat.dat</file_name></datafile>",
        "<datafile><game name='ok'><description/></game><sha1>1111111111111111111111111111111111111111</sha1></datafile>",
        "<datafile><unknown/><game name='ok'><description/></game></datafile>",
        "<datafile xmlns:v='urn:vendor'><v:game name='ok'><description/></v:game></datafile>",
        "<datafile xmlns='urn:vendor'><game name='ok'><description/></game></datafile>",
        "<datafile><game name='ok'><description/><disk name='' size='1'/></game></datafile>",
    ] {
        assert!(strict_read(xml).is_err(), "undeclared field: {xml}");
    }
}

fn owner_document(owner: &str, attributes: &str) -> String {
    match owner {
        "datafile" => {
            format!("<datafile {attributes}><game name='ok'><description/></game></datafile>")
        }
        "game" => {
            format!("<datafile><game name='ok' {attributes}><description/></game></datafile>")
        }
        "clrmamepro" | "romcenter" => header_document(&format!(
            "<name/><description/><version/><author/><{owner} {attributes}/>",
        )),
        "release" => game_document(&format!(
            "<description/><release name='' region='' {attributes}/>"
        )),
        "biosset" => game_document(&format!(
            "<description/><biosset name='' description='' {attributes}/>"
        )),
        "rom" => game_document(&format!(
            "<description/><rom name='' size='invalid' {attributes}/>"
        )),
        _ => game_document(&format!("<description/><{owner} name='' {attributes}/>")),
    }
}

const ENUMERATIONS: [(&str, &str, &[&str]); 15] = [
    ("datafile", "debug", &["yes", "no"]),
    ("clrmamepro", "forcemerging", &["none", "split", "full"]),
    (
        "clrmamepro",
        "forcenodump",
        &["obsolete", "required", "ignore"],
    ),
    ("clrmamepro", "forcepacking", &["zip", "unzip"]),
    ("romcenter", "rommode", &["merged", "split", "unmerged"]),
    ("romcenter", "biosmode", &["merged", "split", "unmerged"]),
    ("romcenter", "samplemode", &["merged", "unmerged"]),
    ("romcenter", "lockrommode", &["yes", "no"]),
    ("romcenter", "lockbiosmode", &["yes", "no"]),
    ("romcenter", "locksamplemode", &["yes", "no"]),
    ("game", "isbios", &["yes", "no"]),
    ("release", "default", &["yes", "no"]),
    ("biosset", "default", &["yes", "no"]),
    ("rom", "status", &["baddump", "nodump", "good", "verified"]),
    ("disk", "status", &["baddump", "nodump", "good", "verified"]),
];

#[test]
fn every_enumeration_accepts_exact_vocabulary_and_rejects_unknown_values() -> TestResult {
    for (owner, attribute, allowed) in ENUMERATIONS {
        assert_eq!(
            strict_read(&owner_document(owner, ""))?,
            1,
            "default {owner}/{attribute}"
        );
        for value in allowed {
            assert_eq!(
                strict_read(&owner_document(owner, &format!("{attribute}='{value}'")))?,
                1,
                "{owner}/{attribute}={value}"
            );
        }
        for value in ["", "unknown", "TRUE", "No", "good bad"] {
            let xml = owner_document(owner, &format!("{attribute}='{value}'"));
            assert!(
                strict_read(&xml).is_err(),
                "invalid {owner}/{attribute}={value}"
            );
        }
    }
    Ok(())
}

#[test]
fn enum_normalization_collapses_only_xml_normalized_space() -> TestResult {
    for (owner, attribute, allowed) in ENUMERATIONS {
        let value = allowed
            .first()
            .ok_or("missing independent enum vocabulary")?;
        for spelling in [
            format!("  {value}  "),
            format!("\t{value}\r\n"),
            format!("&#32;{value}&#x20;"),
        ] {
            assert_eq!(
                strict_read(&owner_document(owner, &format!("{attribute}='{spelling}'")))?,
                1,
                "normalized {owner}/{attribute}={spelling}"
            );
        }
        for spelling in [
            format!("&#9;{value}"),
            format!("{value}&#10;"),
            format!("&#13;{value}"),
            format!("&#xA0;{value}"),
        ] {
            let xml = owner_document(owner, &format!("{attribute}='{spelling}'"));
            assert!(
                strict_read(&xml).is_err(),
                "numeric whitespace reference was incorrectly folded: {xml}"
            );
        }
    }
    Ok(())
}

#[test]
fn all_forty_four_declared_attributes_fit_one_standard_document() -> TestResult {
    let xml = dtd_fixture::ALL_DECLARED_FIELDS;
    assert_eq!(strict_read(xml)?, 1);
    let data = DataFile::from_reader_with_mode(xml.as_bytes(), LogiqxMode::StrictDtd15)?;
    assert_eq!(data.games().len(), 1);
    assert_eq!(data.header()?.name(), "");
    Ok(())
}

#[test]
fn strict_transport_decoding_preserves_scalar_text_and_qname_coordinates() -> TestResult {
    let xml = "<datafile>\r\n  <game name='café' isbios=' yes '>\r\n    <description>音</description>\r\n    <rom name='ゲーム' size='01' status=' good '/>\r\n  </game>\r\n</datafile>";
    let mut variants = vec![xml.as_bytes().to_vec()];
    variants.push([b"\xef\xbb\xbf".as_slice(), xml.as_bytes()].concat());
    for big_endian in [false, true] {
        let mut bytes = if big_endian {
            vec![0xfe, 0xff]
        } else {
            vec![0xff, 0xfe]
        };
        bytes.extend(xml.encode_utf16().flat_map(|unit| {
            if big_endian {
                unit.to_be_bytes()
            } else {
                unit.to_le_bytes()
            }
        }));
        variants.push(bytes);
    }
    for bytes in variants {
        let (_, games) = logiqx::read_with_mode(
            &bytes,
            LogiqxMode::StrictDtd15,
            |_| Ok::<_, mame_coalesce::Error>(0),
            |games, located| {
                assert_eq!(located.game.name(), "café");
                assert_eq!(
                    located.location,
                    logiqx::RecordLocation { line: 2, column: 3 }
                );
                let bios = located
                    .game
                    .attribute_positions()
                    .iter()
                    .find(|position| position.field == logiqx::GameAttribute::IsBios)
                    .ok_or_else(|| {
                        mame_coalesce::Error::XmlValidation("isbios QName missing".into())
                    })?;
                assert_eq!(bios.location.line, 2);
                assert_eq!(bios.location.column, 21);
                assert_eq!(
                    located.rom_locations,
                    [logiqx::RecordLocation { line: 4, column: 5 }]
                );
                *games += 1;
                Ok(())
            },
        )?
        .into_parts();
        assert_eq!(games, 1);
    }
    Ok(())
}

#[test]
fn strict_valid_eof_cannot_ignore_another_root_or_trailing_text() {
    for tail in [
        "<another/>",
        "text",
        "<!DOCTYPE another>",
        "<game name='later'><description/></game>",
        "\u{00a0}",
        "\u{2003}",
    ] {
        let xml = format!("<datafile><game name='ok'><description/></game></datafile>{tail}");
        assert!(
            strict_read(&xml).is_err(),
            "invalid after-root tail accepted: {tail}"
        );
    }
}

#[test]
fn strict_preamble_is_xml_misc_not_arbitrary_skipped_events() -> TestResult {
    let root = "<datafile><game name='ok'><description/></game></datafile>";
    for prefix in [
        "text",
        "<![CDATA[ ]]>",
        "&#32;",
        "\u{00a0}",
        "<?xml version='1.0'?>text",
    ] {
        let xml = format!("{prefix}{root}");
        assert!(
            strict_read(&xml).is_err(),
            "invalid pre-root data accepted: {prefix}"
        );
    }
    assert_eq!(
        strict_read(&format!(
            "<?xml version='1.0'?>\n<!--before--><?catalog test?>{root}<!--after--><?done yes?>"
        ))?,
        1
    );
    Ok(())
}

#[test]
fn declarations_cannot_appear_inside_the_root() {
    for declaration in ["<?xml version='1.0'?>", "<!DOCTYPE datafile>"] {
        let xml =
            format!("<datafile>{declaration}<game name='ok'><description/></game></datafile>");
        assert!(
            strict_read(&xml).is_err(),
            "declaration inside root accepted: {declaration}"
        );
    }
}

#[test]
fn declarations_cannot_disappear_inside_native_subtrees() {
    for children in [
        "<!DOCTYPE datafile><description/>",
        "<description><!DOCTYPE datafile></description>",
        "<description/><rom name='' size=''><!DOCTYPE datafile></rom>",
        "<description/><sample name=''><!DOCTYPE datafile></sample>",
    ] {
        let xml = game_document(children);
        assert!(
            strict_read(&xml).is_err(),
            "declaration disappeared from native subtree: {children}"
        );
    }
}

#[test]
fn doctype_root_name_matches_the_document_without_network_loading() -> TestResult {
    let root = "<datafile><game name='ok'><description/></game></datafile>";
    for doctype in [
        "<!DOCTYPE other>",
        "<!DOCTYPE other SYSTEM 'http://example.invalid/no-fetch.dtd'>",
    ] {
        assert!(
            strict_read(&format!("{doctype}{root}")).is_err(),
            "wrong root declaration accepted: {doctype}"
        );
    }
    assert_eq!(
        strict_read(&format!(
            "<!DOCTYPE datafile SYSTEM 'http://example.invalid/no-fetch.dtd'>{root}"
        ))?,
        1
    );
    Ok(())
}

#[test]
fn standalone_yes_does_not_depend_on_external_defaults_or_normalization() -> TestResult {
    let prolog = "<?xml version='1.0' standalone='yes'?><!DOCTYPE datafile SYSTEM 'http://example.invalid/no-fetch.dtd'>";
    for root in [
        "<datafile><game name='ok'><description/></game></datafile>",
        "<datafile debug='no'><game name='ok'><description/></game></datafile>",
        "<datafile debug='no'><game name='ok' isbios='no'><description/><rom name='a' size='1'/></game></datafile>",
        "<datafile debug=' no '><game name='ok' isbios='no'><description/></game></datafile>",
        "<datafile debug='no'> <game name='ok' isbios='no'><description/></game></datafile>",
        "<datafile debug='no'><game name='ok' isbios='no'> <description/></game></datafile>",
    ] {
        assert!(
            strict_read(&format!("{prolog}{root}")).is_err(),
            "standalone depends on external declaration: {root}"
        );
    }
    let independent = "<datafile debug='no'><game name='ok' isbios='no'><description> permitted PCDATA spaces </description><rom name='a' size='1' status='good'/></game></datafile>";
    assert_eq!(strict_read(&format!("{prolog}{independent}"))?, 1);
    let ordinary = "<?xml version='1.0' standalone='no'?><!DOCTYPE datafile SYSTEM 'http://example.invalid/no-fetch.dtd'><datafile> <game name='ok'> <description/></game></datafile>";
    assert_eq!(strict_read(ordinary)?, 1);
    Ok(())
}

#[test]
fn standalone_external_policy_covers_every_default_owner_and_header_whitespace() -> TestResult {
    let declaration = "<?xml version='1.0' standalone='yes'?><!DOCTYPE datafile SYSTEM 'http://example.invalid/no-fetch.dtd'>";
    for (header, children) in [
        (
            "<header> <name/><description/><version/><author/></header>",
            "",
        ),
        (
            "<header><name/><description/><version/><author/><clrmamepro/></header>",
            "",
        ),
        (
            "<header><name/><description/><version/><author/><romcenter/></header>",
            "",
        ),
        ("", "<release name='' region=''/>"),
        ("", "<biosset name='' description=''/>"),
        ("", "<disk name=''/>"),
        ("", "<rom name='' size='' status=' good '/>"),
    ] {
        let root = format!(
            "<datafile debug='no'>{header}<game name='ok' isbios='no'><description/>{children}</game></datafile>"
        );
        assert_eq!(strict_read(&root)?, 1, "ordinary control: {root}");
        assert!(
            strict_read(&format!("{declaration}{root}")).is_err(),
            "external standalone dependence: {root}"
        );
    }
    assert_eq!(
        strict_read(&format!(
            "{declaration}{}",
            dtd_fixture::ALL_DECLARED_FIELDS
        ))?,
        1
    );
    Ok(())
}

#[test]
fn malformed_or_unsupported_doctype_declarations_cannot_seal_strict_eof() -> TestResult {
    let root = "<datafile><game name='ok'><description/></game></datafile>";
    for declaration in [
        "<!DOCTYPE datafile SYSTEM>",
        "<!DOCTYPE datafile garbage>",
        "<!DOCTYPE datafile SYSTEM 'identifier' garbage>",
        "<!DOCTYPE datafile SYSTEM'identifier'>",
        "<!DOCTYPE datafile PUBLIC 'public-only'>",
        "<!DOCTYPE datafile PUBLIC 'public' 'system' 'extra'>",
        "<!DOCTYPE datafile PUBLIC 'public\tidentifier' 'system'>",
        "<!DOCTYPE datafile PUBLIC 'non-ASCII-é' 'system'>",
        "<!DOCTYPE datafile [garbage]>",
        "<!DOCTYPE datafile [<!ELEMENT datafile ANY>]>",
        "<!DOCTYPE datafile []>",
    ] {
        assert!(
            strict_read(&format!("{declaration}{root}")).is_err(),
            "declaration accepted: {declaration}"
        );
    }
    for declaration in [
        "<!DOCTYPE datafile>",
        "<!DOCTYPE datafile SYSTEM 'http://example.invalid/no-fetch.dtd'>",
        "<!DOCTYPE datafile SYSTEM \"http://example.invalid/path[part]&literal.dtd\">",
        "<!DOCTYPE datafile PUBLIC '-//Logiqx//DTD ROM Management Datafile//EN' 'http://example.invalid/no-fetch.dtd'>",
        "<!DOCTYPE datafile PUBLIC \"pub's id\" \"system's id\">",
        "<!DOCTYPE datafile\nSYSTEM\t'identifier' \r\n>",
    ] {
        assert_eq!(
            strict_read(&format!("{declaration}{root}"))?,
            1,
            "supported declaration rejected: {declaration}"
        );
    }
    Ok(())
}

#[test]
fn strict_doctype_opening_is_exact_xml_markup() -> TestResult {
    let root = "<datafile><game name='ok'><description/></game></datafile>";
    for declaration in [
        "<!doctype datafile>",
        "<!DoCtYpE datafile>",
        "<!DOCTYPEdatafile>",
    ] {
        assert!(
            strict_read(&format!("{declaration}{root}")).is_err(),
            "non-XML opening accepted: {declaration}"
        );
    }
    for declaration in [
        "<!DOCTYPE datafile>",
        "<!DOCTYPE\tdatafile>",
        "<!DOCTYPE\ndatafile>",
        "\u{feff}<!DOCTYPE datafile>",
    ] {
        assert_eq!(strict_read(&format!("{declaration}{root}"))?, 1);
    }
    Ok(())
}

#[test]
fn doctype_source_ranges_include_transport_bom_after_prolog_events() -> TestResult {
    let root = "<datafile><game name='ok'><description/></game></datafile>";
    for prolog in [
        "",
        "<?xml version='1.0'?>",
        " \n<!--before declaration-->",
        "<?xml version='1.0'?>\n<?catalog control?><!--before declaration-->",
    ] {
        for bom in ["", "\u{feff}"] {
            let xml = format!("{bom}{prolog}<!DOCTYPE datafile>{root}");
            for mode in [LogiqxMode::ObservedCompatible, LogiqxMode::StrictDtd15] {
                let data = DataFile::from_reader_with_mode(xml.as_bytes(), mode)?;
                assert_eq!(data.games().len(), 1, "{mode:?}: {xml}");
            }
        }
    }
    Ok(())
}
