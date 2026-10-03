use std::cell::Cell;

use mame_coalesce::logiqx::{self, RecordLocation};

#[test]
fn final_metadata_follows_streamed_games_without_a_second_walk() -> mame_coalesce::Result<()> {
    let xml = b"<datafile build='build' debug='no'><game name='first'/><header><name>Late</name><version/></header><file_name>native.dat</file_name><sha1>1111111111111111111111111111111111111111</sha1></datafile>";
    let validated = logiqx::read_with(
        xml,
        |root| {
            assert_eq!(root.build, Some("build"));
            assert_eq!(root.debug, Some("no"));
            Ok::<_, mame_coalesce::Error>(Vec::new())
        },
        |names, record| {
            names.push(record.game.name().to_owned());
            Ok(())
        },
    )?;
    let (metadata, names) = validated.into_parts();
    assert_eq!(names, ["first"]);
    assert_eq!(metadata.header()?.name(), "Late");
    assert_eq!(metadata.header()?.version().map(String::as_str), Some(""));
    assert_eq!(metadata.file_name(), Some("native.dat"));
    assert_eq!(metadata.sha1(), Some([0x11; 20].as_slice()));
    Ok(())
}

#[test]
fn compatibility_empty_document_remains_distinct_from_empty_header_text()
-> mame_coalesce::Result<()> {
    for xml in [
        b"<datafile/>".as_slice(),
        b"<datafile><header><name/></header></datafile>",
    ] {
        let (metadata, count) = logiqx::read_with(
            xml,
            |_| Ok::<_, mame_coalesce::Error>(0),
            |count, _| {
                *count += 1;
                Ok(())
            },
        )?
        .into_parts();
        assert_eq!(count, 0);
        assert_eq!(
            metadata.header_opt().map(logiqx::Header::name),
            (xml != b"<datafile/>").then_some("")
        );
    }
    Ok(())
}

#[test]
fn completed_callback_does_not_prove_late_metadata_or_eof_valid() {
    for tail in [
        "<game name='unfinished'>",
        "<header><name>A</name></header><header><name>B</name></header></datafile>",
        "<file_name>A</file_name><file_name>B</file_name></datafile>",
        "<sha1>1111111111111111111111111111111111111111</sha1><sha1>2222222222222222222222222222222222222222</sha1></datafile>",
        "<sha1>invalid</sha1></datafile>",
        "</datafile><another-root/>",
        "</datafile>trailing text",
    ] {
        let calls = Cell::new(0);
        let xml = format!("<datafile><game name='completed'/>{tail}");
        let result = logiqx::read_with(
            xml.as_bytes(),
            |_| Ok::<_, mame_coalesce::Error>(()),
            |(), _| {
                calls.set(calls.get() + 1);
                Ok(())
            },
        );
        assert_eq!(calls.get(), 1, "completed game callback: {tail}");
        assert!(result.is_err(), "no EOF proof: {tail}");
    }
}

#[test]
fn callback_error_stops_before_next_game_or_later_parse_error() {
    let calls = Cell::new(0);
    let result = logiqx::read_with(
        b"<datafile><game name='first'/><game name='unfinished'>",
        |_| Ok::<_, mame_coalesce::Error>(()),
        |(), _| {
            calls.set(calls.get() + 1);
            Err(mame_coalesce::Error::XmlValidation(
                "callback stopped".into(),
            ))
        },
    );
    assert_eq!(calls.get(), 1);
    assert!(
        matches!(result, Err(mame_coalesce::Error::XmlValidation(message)) if message == "callback stopped")
    );
}

#[test]
fn streamed_positions_are_identical_for_utf8_bom_and_utf16() -> mame_coalesce::Result<()> {
    let xml = "<datafile>\n  <game name='café'>\n    <device_ref name='音'/>\n    <rom name='ゲーム' size='1'/>\n  </game>\n</datafile>";
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
        let (_, count) = logiqx::read_with(
            &bytes,
            |_| Ok::<_, mame_coalesce::Error>(0),
            |count, record| {
                assert_eq!(record.game.name(), "café");
                assert_eq!(record.location, RecordLocation { line: 2, column: 3 });
                assert_eq!(
                    record.device_ref_locations,
                    [RecordLocation { line: 3, column: 5 }]
                );
                assert_eq!(
                    record.rom_locations,
                    [RecordLocation { line: 4, column: 5 }]
                );
                assert!(record.unsupported_attributes.is_empty());
                *count += 1;
                Ok(())
            },
        )?
        .into_parts();
        assert_eq!(count, 1);
    }
    Ok(())
}
