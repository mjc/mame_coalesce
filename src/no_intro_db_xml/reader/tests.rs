#![allow(clippy::expect_used, clippy::panic)]

use std::{fmt::Write, io::Write as IoWrite};

use flate2::{Compression, write::GzEncoder};

use super::*;

type CollectedDatabase = (NoIntroDatabaseDocument, Vec<DatabaseGame>);

const MODE: NoIntroDatabaseMode = NoIntroDatabaseMode::ObservedCompatible;

fn read(xml: &str) -> Result<ValidatedNoIntroDatabase<'_, CollectedDatabase>> {
    read_with(
        xml.as_bytes(),
        MODE,
        |document| Ok((document, Vec::new())),
        |state, game| {
            state.1.push(game);
            Ok(())
        },
    )
}

#[test]
fn all_ledger_fields_are_typed_and_retain_presence_order_and_digest_state() {
    let xml = r#"<?xml version="1.0"?>
<datafile>
 <header><author>A</author><piracy></piracy><trademarks>T</trademarks><url>U</url><version>V</version><author>B</author></header>
 <game name="Game">
  <archive additional="a" adult="a" aftermarket="a" alt="a" bios="a" categories="a" complete="a" dat="a" datter_note="a" description="a" devstatus="a" gameid1="a" gameid2="a" langchecked="a" languages="a" licensed="a" listed="a" mergename="a" name="a" name_alt="a" number="0001" physical="a" region="a" regparent="literal-parent" showlang="a" special1="a" special2="a" sticky_note="a" version1="a" version2="a" clone="P" mergeof="opaque"/>
  <source>
   <details comment1="a" comment2="a" d_date="a" d_date_info="a" dumper="a" id="0007" link1="a" link2="a" link3="a" media_title="a" nodump="a" origin="a" originalformat="a" project="a" r_date="a" r_date_info="a" region="a" rominfo="a" section="a" tool="a"/>
   <serials box_barcode="a" box_serial="a" chip_serial="a" digital_serial1="a" digital_serial2="a" lockout_serial="a" media_serial1="a" media_serial2="a" media_serial3="a" mediastamp="a" pcb_serial="a" romchip_serial1="a" romchip_serial2="a" savechip_serial="a"/>
   <file bad="0" crc32="AABBCCDD" date="a" extension="a" filter="a" forcename="a" forcescenename="a" format="a" header="a" id="002" item="a" md5="00000000000000000000000000000000" mia="a" note="a" origin_sha256="0000000000000000000000000000000000000000000000000000000000000000" origin_size="0002" serial="a" sha1="0000000000000000000000000000000000000000" sha256="bad" size="0003" unique="a" update_type="a" version="a"/>
  </source>
  <release>
   <details archivename="a" category="a" comment="a" date="a" dirname="a" group="a" id="0008" nfo_crc32="aabbccdd" nfo_size="0009" nfocrc="bad" nfoname="a" nfosize="0010" origin="a" originalformat="a" region="a" rominfo="a" tool="a"/>
   <serials box_barcode="a" box_serial="a" media_serial1="a" mediastamp="a" pcb_serial="a" romchip_serial1="a"/>
   <file bad="0" crc32="aabbccdd" extension="a" forcename="a" forcescenename="a" format="a" header="a" id="002" item="a" md5="00000000000000000000000000000000" note="a" serial="a" sha1="0000000000000000000000000000000000000000" sha256="0000000000000000000000000000000000000000000000000000000000000000" size="0003" update_type="a" version="a"/>
  </release>
 </game>
</datafile>"#;
    let validated = read(xml).expect("ledger fixture parses");
    let (document, games) = validated.into_inner();
    assert_eq!(document.envelope, EnvelopeKind::SingleDatafile);
    let header = document.header.expect("header retained");
    assert_eq!(header.fields.len(), 6);
    assert_eq!(header.fields[1].value.value, "");
    let game = &games[0];
    assert_eq!(
        game.archives[0]
            .number
            .as_ref()
            .expect("archive number retained")
            .value,
        "0001"
    );
    assert_eq!(
        game.archives[0]
            .regparent
            .as_ref()
            .expect("archive raw parent retained")
            .value,
        "literal-parent"
    );
    assert!(matches!(
        game.archives[0].clone.as_ref(),
        Some(ArchiveClone::ParentMarker(_))
    ));
    assert_eq!(
        game.archives[0]
            .mergeof
            .as_ref()
            .expect("archive raw merge target retained")
            .value,
        "opaque"
    );
    assert_eq!(game.source_or_release.len(), 2);
    assert_basic_source(game);
    assert_basic_release(game);
}

fn assert_basic_source(game: &DatabaseGame) {
    let SourceOrRelease::Source(source) = &game.source_or_release[0] else {
        panic!("first child is source")
    };
    assert_eq!(
        source
            .details
            .as_ref()
            .expect("source details retained")
            .id
            .as_ref()
            .expect("source details ID retained")
            .value,
        "0007"
    );
    assert_eq!(
        source
            .serials
            .as_ref()
            .expect("source serials retained")
            .savechip_serial
            .as_ref()
            .expect("source save-chip serial retained")
            .value,
        "a"
    );
    assert_eq!(
        source.files[0]
            .id
            .as_ref()
            .expect("source file ID retained")
            .value,
        "002"
    );
    assert_eq!(
        source.files[0]
            .crc32
            .as_ref()
            .expect("source file CRC32 retained")
            .value
            .as_deref(),
        Some(&[0xaa, 0xbb, 0xcc, 0xdd][..])
    );
    assert!(
        source.files[0]
            .sha256
            .as_ref()
            .expect("invalid source SHA256 retained")
            .value
            .is_none()
    );
    assert_eq!(
        source.files[0]
            .origin_size
            .as_ref()
            .expect("source origin size retained")
            .value,
        "0002"
    );
}

fn assert_basic_release(game: &DatabaseGame) {
    let SourceOrRelease::Release(release) = &game.source_or_release[1] else {
        panic!("second child is release")
    };
    assert_eq!(
        release
            .details
            .as_ref()
            .expect("release details retained")
            .nfo_crc32
            .as_ref()
            .expect("release NFO CRC32 retained")
            .value
            .as_deref(),
        Some(&[0xaa, 0xbb, 0xcc, 0xdd][..])
    );
    assert!(
        release
            .details
            .as_ref()
            .expect("release details retained")
            .nfocrc
            .as_ref()
            .expect("invalid release NFO CRC retained")
            .value
            .is_none()
    );
    assert_eq!(
        release.files[0]
            .id
            .as_ref()
            .expect("release file ID retained")
            .value,
        "002"
    );
}

fn ledger_attributes(prefix: &str, names: &[&str]) -> String {
    let mut attributes = String::new();
    for name in names {
        write!(attributes, " {name}='{prefix}:{name}'")
            .expect("writing fixture attributes into a String succeeds");
    }
    attributes
}

fn assert_ledger_fields(
    xml: &str,
    element: &str,
    prefix: &str,
    fields: &[(&str, &Option<DeclaredText>)],
) {
    let first_name = fields.first().expect("ledger owner has declared fields").0;
    let context = format!("<{element} {first_name}='{prefix}:{first_name}'");
    for (ordinal, (name, field)) in fields.iter().enumerate() {
        let field = field.as_ref().expect("present ledger field");
        assert_eq!(field.value, format!("{prefix}:{name}"));
        assert_eq!(field.source_order, ordinal);
        assert_eq!(
            field.location,
            fixture_attribute_location(xml, &context, name),
            "{element}.{name} location"
        );
    }
}

fn fixture_attribute_location(xml: &str, context: &str, attribute: &str) -> RecordLocation {
    assert_eq!(xml.matches(context).count(), 1, "unique fixture context");
    let context_offset = xml.find(context).expect("fixture context exists");
    let tag_end = xml[context_offset..]
        .find('>')
        .map(|offset| context_offset + offset)
        .expect("fixture start tag ends");
    let tag = &xml[context_offset..=tag_end];
    let attribute_offset = tag
        .find(&format!(" {attribute}="))
        .unwrap_or_else(|| panic!("fixture tag contains attribute {attribute:?}"))
        + 1;
    location_at(xml, context_offset + attribute_offset)
}

macro_rules! names {
    ($($field:ident),+ $(,)?) => { &[$(stringify!($field)),+] };
}
macro_rules! fields {
    ($owner:expr; $($field:ident),+ $(,)?) => { &[$((stringify!($field), &$owner.$field)),+] };
}

fn distinct_archive_attributes() -> String {
    ledger_attributes(
        "archive",
        names!(
            additional,
            adult,
            aftermarket,
            alt,
            bios,
            categories,
            complete,
            dat,
            datter_note,
            description,
            devstatus,
            gameid1,
            gameid2,
            langchecked,
            languages,
            licensed,
            listed,
            mergename,
            name,
            name_alt,
            number,
            physical,
            region,
            regparent,
            showlang,
            special1,
            special2,
            sticky_note,
            version1,
            version2,
            mergeof
        ),
    )
}

fn distinct_source_details_attributes() -> String {
    ledger_attributes(
        "dump-details",
        names!(
            comment1,
            comment2,
            d_date,
            d_date_info,
            dumper,
            id,
            link1,
            link2,
            link3,
            media_title,
            nodump,
            origin,
            originalformat,
            project,
            r_date,
            r_date_info,
            region,
            rominfo,
            section,
            tool
        ),
    )
}

fn distinct_source_serials_attributes() -> String {
    ledger_attributes(
        "dump-serials",
        names!(
            box_barcode,
            box_serial,
            chip_serial,
            digital_serial1,
            digital_serial2,
            lockout_serial,
            media_serial1,
            media_serial2,
            media_serial3,
            mediastamp,
            pcb_serial,
            romchip_serial1,
            romchip_serial2,
            savechip_serial
        ),
    )
}

fn distinct_source_file_attributes() -> String {
    ledger_attributes(
        "dump-file",
        names!(
            bad,
            date,
            extension,
            filter,
            forcename,
            forcescenename,
            format,
            header,
            id,
            item,
            mia,
            note,
            origin_size,
            serial,
            size,
            unique,
            update_type,
            version
        ),
    )
}

fn distinct_release_details_attributes() -> String {
    ledger_attributes(
        "release-details",
        names!(
            archivename,
            category,
            comment,
            date,
            dirname,
            group,
            id,
            nfo_size,
            nfoname,
            nfosize,
            origin,
            originalformat,
            region,
            rominfo,
            tool
        ),
    )
}

fn distinct_release_serials_attributes() -> String {
    ledger_attributes(
        "release-serials",
        names!(
            box_barcode,
            box_serial,
            media_serial1,
            mediastamp,
            pcb_serial,
            romchip_serial1
        ),
    )
}

fn distinct_release_file_attributes() -> String {
    ledger_attributes(
        "release-file",
        names!(
            bad,
            extension,
            forcename,
            forcescenename,
            format,
            header,
            id,
            item,
            note,
            serial,
            size,
            update_type,
            version
        ),
    )
}

fn distinct_ledger_xml() -> String {
    let archive = distinct_archive_attributes();
    let source_details = distinct_source_details_attributes();
    let source_serials = distinct_source_serials_attributes();
    let source_file = distinct_source_file_attributes();
    let release_details = distinct_release_details_attributes();
    let release_serials = distinct_release_serials_attributes();
    let release_file = distinct_release_file_attributes();
    format!(
        "<datafile>\n<header><author>author</author><piracy>piracy</piracy><trademarks>trademarks</trademarks><url>url</url><version>version</version></header>\n<game name='distinct'>\n<archive{archive} clone='P'/>\n<source>\n<details{source_details}/>\n<serials{source_serials}/>\n<file{source_file} crc32='AaBbCcDd' md5='{}' sha1='{}' sha256='{}' origin_sha256='{}'/>\n</source>\n<release>\n<details{release_details} nfo_crc32='CcDdEeFf' nfocrc='invalid-nfo-crc'/>\n<serials{release_serials}/>\n<file{release_file} crc32='Aa00Bb11' md5='{}' sha1='{}' sha256='{}'/>\n</release>\n</game>\n</datafile>",
        "11".repeat(16),
        "22".repeat(20),
        "33".repeat(32),
        "44".repeat(32),
        "55".repeat(16),
        "66".repeat(20),
        "77".repeat(32)
    )
}

#[test]
fn every_ledger_field_maps_to_its_own_distinct_value_and_position() {
    let xml = distinct_ledger_xml();
    let (document, games) = read(&xml)
        .expect("complete distinct ledger fixture")
        .into_inner();
    assert_distinct_header(document);
    let game = &games[0];
    assert_eq!(game.name.value, "distinct");
    assert_distinct_archive(game, &xml);
    assert_distinct_source(game, &xml);
    assert_distinct_release(game, &xml);
}

fn assert_distinct_header(document: NoIntroDatabaseDocument) {
    let header = document.header.expect("header");
    for (ordinal, (field, kind)) in header
        .fields
        .iter()
        .zip([
            HeaderFieldKind::Author,
            HeaderFieldKind::Piracy,
            HeaderFieldKind::Trademarks,
            HeaderFieldKind::Url,
            HeaderFieldKind::Version,
        ])
        .enumerate()
    {
        assert_eq!(field.kind, kind);
        assert_eq!(field.value.source_order, ordinal);
        assert_eq!(
            field.value.value,
            names!(author, piracy, trademarks, url, version)[ordinal]
        );
    }
}

fn assert_distinct_archive(game: &DatabaseGame, xml: &str) {
    let archive = &game.archives[0];
    assert_eq!(archive.source_order, 0);
    let archive_context = "<archive additional='archive:additional";
    assert_ledger_fields(
        xml,
        "archive",
        "archive",
        fields!(archive; additional, adult, aftermarket, alt, bios, categories, complete, dat, datter_note, description, devstatus, gameid1, gameid2, langchecked, languages, licensed, listed, mergename, name, name_alt, number, physical, region, regparent, showlang, special1, special2, sticky_note, version1, version2, mergeof),
    );
    let ArchiveClone::ParentMarker(marker) = archive.clone.as_ref().expect("clone marker") else {
        panic!("literal P is a marker");
    };
    assert_eq!(marker.value, "P");
    assert_eq!(marker.source_order, 31);
    assert_eq!(
        marker.location,
        fixture_attribute_location(xml, archive_context, "clone")
    );
}

fn assert_distinct_source(game: &DatabaseGame, xml: &str) {
    let SourceOrRelease::Source(source) = &game.source_or_release[0] else {
        panic!("dump source");
    };
    assert_eq!(source.source_order, 1);
    let details = source.details.as_ref().expect("dump details");
    assert_eq!(details.source_order, 0);
    assert_ledger_fields(
        xml,
        "details",
        "dump-details",
        fields!(details; comment1, comment2, d_date, d_date_info, dumper, id, link1, link2, link3, media_title, nodump, origin, originalformat, project, r_date, r_date_info, region, rominfo, section, tool),
    );
    let serials = source.serials.as_ref().expect("dump serials");
    assert_eq!(serials.source_order, 1);
    assert_ledger_fields(
        xml,
        "serials",
        "dump-serials",
        fields!(serials; box_barcode, box_serial, chip_serial, digital_serial1, digital_serial2, lockout_serial, media_serial1, media_serial2, media_serial3, mediastamp, pcb_serial, romchip_serial1, romchip_serial2, savechip_serial),
    );
    let file = &source.files[0];
    assert_eq!(file.source_order, 2);
    let file_context = "<file bad='dump-file:bad";
    assert_ledger_fields(
        xml,
        "file",
        "dump-file",
        fields!(file; bad, date, extension, filter, forcename, forcescenename, format, header, id, item, mia, note, origin_size, serial, size, unique, update_type, version),
    );
    assert_digests(
        xml,
        file_context,
        18,
        &[
            expected_digest(
                "crc32",
                file.crc32.as_ref(),
                "AaBbCcDd",
                Some(vec![0xaa, 0xbb, 0xcc, 0xdd]),
            ),
            expected_digest(
                "md5",
                file.md5.as_ref(),
                &"11".repeat(16),
                Some(vec![0x11; 16]),
            ),
            expected_digest(
                "sha1",
                file.sha1.as_ref(),
                &"22".repeat(20),
                Some(vec![0x22; 20]),
            ),
            expected_digest(
                "sha256",
                file.sha256.as_ref(),
                &"33".repeat(32),
                Some(vec![0x33; 32]),
            ),
            expected_digest(
                "origin_sha256",
                file.origin_sha256.as_ref(),
                &"44".repeat(32),
                Some(vec![0x44; 32]),
            ),
        ],
    );
}

fn assert_distinct_release(game: &DatabaseGame, xml: &str) {
    let SourceOrRelease::Release(release) = &game.source_or_release[1] else {
        panic!("release");
    };
    assert_eq!(release.source_order, 2);
    let details = release.details.as_ref().expect("release details");
    assert_eq!(details.source_order, 0);
    let details_context = "<details archivename='release-details:archivename";
    assert_ledger_fields(
        xml,
        "details",
        "release-details",
        fields!(details; archivename, category, comment, date, dirname, group, id, nfo_size, nfoname, nfosize, origin, originalformat, region, rominfo, tool),
    );
    assert_digests(
        xml,
        details_context,
        15,
        &[
            expected_digest(
                "nfo_crc32",
                details.nfo_crc32.as_ref(),
                "CcDdEeFf",
                Some(vec![0xcc, 0xdd, 0xee, 0xff]),
            ),
            expected_digest("nfocrc", details.nfocrc.as_ref(), "invalid-nfo-crc", None),
        ],
    );
    let serials = release.serials.as_ref().expect("release serials");
    assert_eq!(serials.source_order, 1);
    assert_ledger_fields(
        xml,
        "serials",
        "release-serials",
        fields!(serials; box_barcode, box_serial, media_serial1, mediastamp, pcb_serial, romchip_serial1),
    );
    let file = &release.files[0];
    assert_eq!(file.source_order, 2);
    let file_context = "<file bad='release-file:bad";
    assert_ledger_fields(
        xml,
        "file",
        "release-file",
        fields!(file; bad, extension, forcename, forcescenename, format, header, id, item, note, serial, size, update_type, version),
    );
    assert_digests(
        xml,
        file_context,
        13,
        &[
            expected_digest(
                "crc32",
                file.crc32.as_ref(),
                "Aa00Bb11",
                Some(vec![0xaa, 0x00, 0xbb, 0x11]),
            ),
            expected_digest(
                "md5",
                file.md5.as_ref(),
                &"55".repeat(16),
                Some(vec![0x55; 16]),
            ),
            expected_digest(
                "sha1",
                file.sha1.as_ref(),
                &"66".repeat(20),
                Some(vec![0x66; 20]),
            ),
            expected_digest(
                "sha256",
                file.sha256.as_ref(),
                &"77".repeat(32),
                Some(vec![0x77; 32]),
            ),
        ],
    );
}

struct DigestExpectation<'a> {
    attribute: &'a str,
    field: Option<&'a DatabaseDigest>,
    text: &'a str,
    bytes: Option<Vec<u8>>,
}

const fn expected_digest<'a>(
    attribute: &'a str,
    field: Option<&'a DatabaseDigest>,
    text: &'a str,
    bytes: Option<Vec<u8>>,
) -> DigestExpectation<'a> {
    DigestExpectation {
        attribute,
        field,
        text,
        bytes,
    }
}

fn assert_digests(
    xml: &str,
    context: &str,
    first_ordinal: usize,
    fields: &[DigestExpectation<'_>],
) {
    for (ordinal, expected) in fields.iter().enumerate() {
        let digest = expected.field.expect("present digest declaration");
        assert_eq!(digest.source.value, expected.text);
        assert_eq!(digest.source.source_order, first_ordinal + ordinal);
        assert_eq!(
            digest.source.location,
            fixture_attribute_location(xml, context, expected.attribute)
        );
        assert_eq!(&digest.value, &expected.bytes);
    }
}

#[test]
fn sibling_header_and_zero_source_games_are_valid() {
    let validated = read(
        "<header><version>v</version></header><datafile><game name=\"empty sources\"/></datafile>",
    )
    .expect("sibling envelope and source-free game parse");
    let (document, games) = validated.into_inner();
    assert_eq!(document.envelope, EnvelopeKind::SiblingHeaderDatafile);
    assert_eq!(games.len(), 1);
    assert!(games[0].source_or_release.is_empty());
    assert!(games[0].archives.is_empty());
}

#[test]
fn header_field_ordinals_are_zero_based() {
    let (document, _) = read("<header><version>v</version><author>a</author></header><datafile/>")
        .expect("observed sibling envelope parses")
        .into_inner();
    let fields = document.header.expect("header retained").fields;
    assert_eq!(fields[0].value.source_order, 0);
    assert_eq!(fields[1].value.source_order, 1);
}

#[test]
fn scalar_budget_covers_text_split_by_comments_and_references() {
    let chunk = "a".repeat(512 * 1024 + 1);
    for separator in ["<!--split-->", "<?split?>", "&#32;"] {
        let xml = format!(
            "<datafile><header><author>{chunk}{separator}{chunk}</author></header></datafile>"
        );
        assert!(
            read(&xml).is_err(),
            "split scalar must not bypass its cumulative limit"
        );
    }
}

#[test]
fn scalar_cdata_preserves_text_without_resolving_literal_references() {
    let (document, _) =
        read("<datafile><header><author><![CDATA[a &amp; b]]></author></header></datafile>")
            .expect("CDATA inside a text field is well-formed character content")
            .into_inner();
    assert_eq!(
        document.header.expect("header").fields[0].value.value,
        "a &amp; b"
    );
}

#[test]
fn repeated_file_ids_remain_owned_by_each_source_and_release() {
    let validated = read("<datafile><game name=\"same ids\"><source><file id=\"1\"/></source><release><file id=\"1\"/></release><source><file id=\"1\"/></source></game></datafile>")
        .expect("repeated owner-scoped ids parse");
    let (_, games) = validated.into_inner();
    assert_eq!(games[0].source_or_release.len(), 3);
    assert_eq!(
        games[0]
            .source_or_release
            .iter()
            .map(SourceOrRelease::source_order)
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
}

#[test]
fn duplicate_singletons_wrong_owner_and_second_root_fail() {
    for xml in [
        "<datafile><game name=\"g\"><source><details/><details/></source></game></datafile>",
        "<datafile><game name=\"g\"><release><serials/><serials/></release></game></datafile>",
        "<datafile><game name=\"g\"><source><details><file/></details></source></game></datafile>",
        "<datafile/><datafile/>",
    ] {
        assert!(read(xml).is_err(), "must reject {xml}");
    }
}

#[test]
fn null_recovery_is_decoded_only_and_records_all_six_original_locations() {
    let prefix = "<datafile>\r\n<game name=\"g\"><source><details comment1=\"";
    let suffix = "\"/></source></game></datafile>";
    let xml = format!("{prefix}a\0\0\0\0\0\0b{suffix}");
    let parsed = read_with::<_, Error>(
        xml.as_bytes(),
        NoIntroDatabaseMode::NullRecoveryCompatible,
        |document| Ok((document, Vec::<DatabaseGame>::new())),
        |state, game| {
            state.1.push(game);
            Ok(())
        },
    )
    .expect("decoded NUL recovery parses");
    assert_eq!(parsed.recovery_warnings().count(), 6);
    let prefix_columns = prefix
        .rsplit('\n')
        .next()
        .expect("fixture prefix has a final line")
        .chars()
        .count();
    let first_null_column =
        i64::try_from(prefix_columns).expect("fixture scalar column fits i64") + 2;
    assert!(
        parsed
            .recovery_warnings()
            .enumerate()
            .all(|(index, warning)| warning.location.line == 2
                && warning.location.column
                    == first_null_column
                        + i64::try_from(index).expect("six-warning ordinal fits i64"))
    );
    let (_, games) = parsed.into_inner();
    assert_eq!(games[0].source_or_release.len(), 1);
    let SourceOrRelease::Source(source) = &games[0].source_or_release[0] else {
        panic!("source")
    };
    assert_eq!(
        source
            .details
            .as_ref()
            .expect("recovered source details retained")
            .comment1
            .as_ref()
            .expect("recovered source comment retained")
            .value,
        "a������b"
    );
    assert!(read(&xml).is_err(), "observed-compatible mode rejects NUL");

    let utf16 = utf16le_with_bom(&xml);
    let parsed = read_with::<_, Error>(
        &utf16,
        NoIntroDatabaseMode::NullRecoveryCompatible,
        |document| Ok((document, Vec::<DatabaseGame>::new())),
        |state, game| {
            state.1.push(game);
            Ok(())
        },
    )
    .expect("UTF-16 decoded NUL recovery parses");
    assert_eq!(parsed.recovery_warnings().count(), 6);
    assert_eq!(
        parsed
            .recovery_warnings()
            .map(|warning| warning.location)
            .collect::<Vec<_>>(),
        document_warning_locations(&xml)
    );
}

#[test]
fn detail_opening_ranges_own_only_warnings_inside_multiline_tags() {
    let xml = detail_opening_ranges_xml();
    let parsed = read_with::<_, Error>(
        xml.as_bytes(),
        NoIntroDatabaseMode::NullRecoveryCompatible,
        |document| Ok((document, Vec::<DatabaseGame>::new())),
        |state, game| {
            state.1.push(game);
            Ok(())
        },
    )
    .expect("multiline details tags with recovered NULs parse");
    let warnings = parsed
        .recovery_warnings()
        .map(|warning| warning.location)
        .collect::<Vec<_>>();
    assert_eq!(warnings.len(), 4);

    let ranges = detail_opening_ranges(xml.as_bytes());
    assert_detail_opening_end_locations(xml, ranges);
    assert_eq!(detail_warning_counts(&warnings, ranges), [1, 1, 2]);

    assert_eq!(detail_opening_ranges(&utf16le_with_bom(xml)), ranges);
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(xml.as_bytes())
        .expect("gzip fixture writes");
    let gzip = encoder.finish().expect("gzip fixture finishes");
    assert_eq!(detail_opening_ranges(&gzip), ranges);
}

fn detail_opening_ranges_xml() -> &'static str {
    concat!(
        "<datafile>\r\n",
        "<game name=\"before\0\"/>\r\n",
        "<game name=\"details\">\r\n",
        " <source>\r\n",
        "  <details\r\n",
        "    comment1=\"before&#x26;😀\0after\"\r\n",
        "    id=\"source&#x31;\"\r\n",
        "  />\r\n",
        " </source>\r\n",
        " <release>\r\n",
        "  <details\r\n",
        "    comment=\"release\r\n",
        "      &#x26;😀\0tail\"\r\n",
        "  ></details>\r\n",
        " </release>\r\n",
        "</game>\r\n",
        "<game name=\"after\0\"/>\r\n",
        "</datafile>"
    )
}

fn assert_detail_opening_end_locations(xml: &str, ranges: [(RecordLocation, RecordLocation); 2]) {
    assert_eq!(
        ranges[0].1,
        location_at(
            xml,
            xml.find("  />").expect("source empty tag") + "  />".len()
        )
    );
    assert_eq!(
        ranges[1].1,
        location_at(
            xml,
            xml.find("  >").expect("release opening tag") + "  >".len()
        )
    );
}

fn detail_warning_counts(
    warnings: &[RecordLocation],
    ranges: [(RecordLocation, RecordLocation); 2],
) -> [usize; 3] {
    let inside = |(start, end)| {
        warnings
            .iter()
            .filter(|&&warning| location_in_range(warning, start, end))
            .count()
    };
    let outside = warnings
        .iter()
        .filter(|&&warning| {
            !location_in_range(warning, ranges[0].0, ranges[0].1)
                && !location_in_range(warning, ranges[1].0, ranges[1].1)
        })
        .count();
    [inside(ranges[0]), inside(ranges[1]), outside]
}

fn location_in_range(location: RecordLocation, start: RecordLocation, end: RecordLocation) -> bool {
    (start.line, start.column) <= (location.line, location.column)
        && (location.line, location.column) < (end.line, end.column)
}

fn location_at(xml: &str, byte_offset: usize) -> RecordLocation {
    let mut position = SourcePosition::new();
    for character in xml[..byte_offset].chars() {
        position.advance(character);
    }
    position.location()
}

fn detail_opening_ranges(input: &[u8]) -> [(RecordLocation, RecordLocation); 2] {
    let parsed = read_with::<_, Error>(
        input,
        NoIntroDatabaseMode::NullRecoveryCompatible,
        |document| Ok((document, Vec::<DatabaseGame>::new())),
        |state, game| {
            state.1.push(game);
            Ok(())
        },
    )
    .expect("details with recovered NULs parse");
    let (_, games) = parsed.into_inner();
    let details_game = &games[1];
    let SourceOrRelease::Source(source) = &details_game.source_or_release[0] else {
        panic!("source")
    };
    let SourceOrRelease::Release(release) = &details_game.source_or_release[1] else {
        panic!("release")
    };
    let source_details = source.details.as_ref().expect("source details retained");
    let release_details = release.details.as_ref().expect("release details retained");
    [
        (source_details.location, source_details.opening_end),
        (release_details.location, release_details.opening_end),
    ]
}

#[test]
fn recovery_warnings_retain_exact_utf8_utf16_and_gzip_byte_excerpts() {
    let xml = "<datafile><game name=\"😀\0\"/></datafile>";
    let nul = xml.find('\0').expect("fixture contains NUL");
    let parsed = read_with::<_, Error>(
        xml.as_bytes(),
        NoIntroDatabaseMode::NullRecoveryCompatible,
        |_| Ok(()),
        |(), _| Ok(()),
    )
    .expect("recovery parses");
    let warning = parsed.recovery_warnings().next().expect("NUL warning");
    let excerpt = warning.excerpt.expect("warning retains exact byte excerpt");
    assert_eq!(excerpt.bytes(), b"\0");
    assert_eq!(excerpt.view(), ExcerptView::RetainedOriginalBytes);
    assert_eq!(excerpt.start_byte(), nul);
    assert_eq!(excerpt.problem(), ByteRange::new(0, 1));
    assert_eq!(excerpt.original_problem(), ByteRange::new(nul, nul + 1));

    let utf16 = utf16le_with_bom(xml);
    let encoded_nul = 2 + xml[..nul].encode_utf16().count() * 2;
    let parsed = read_with::<_, Error>(
        &utf16,
        NoIntroDatabaseMode::NullRecoveryCompatible,
        |_| Ok(()),
        |(), _| Ok(()),
    )
    .expect("UTF-16 recovery parses");
    let excerpt = parsed
        .recovery_warnings()
        .next()
        .expect("UTF-16 NUL warning")
        .excerpt
        .expect("UTF-16 warning retains bytes");
    assert_eq!(excerpt.bytes(), &[0, 0]);
    assert_eq!(excerpt.view(), ExcerptView::RetainedOriginalBytes);
    assert_eq!(excerpt.start_byte(), encoded_nul);
    assert_eq!(excerpt.problem(), ByteRange::new(0, 2));
    assert_eq!(
        excerpt.original_problem(),
        ByteRange::new(encoded_nul, encoded_nul + 2)
    );

    let clipped = excerpt
        .clone()
        .clip(ByteRange::new(0, 1).expect("ordered clip range"))
        .expect("partial byte clip retained");
    assert_eq!(clipped.bytes(), &[0]);
    assert_eq!(clipped.start_byte(), encoded_nul);
    assert_eq!(clipped.problem(), None);
    assert_eq!(clipped.source_problem(), excerpt.source_problem());

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(xml.as_bytes())
        .expect("gzip fixture writes");
    let gzip = encoder.finish().expect("gzip fixture finishes");
    let parsed = read_with::<_, Error>(
        &gzip,
        NoIntroDatabaseMode::NullRecoveryCompatible,
        |_| Ok(()),
        |(), _| Ok(()),
    )
    .expect("gzip recovery parses");
    let excerpt = parsed
        .recovery_warnings()
        .next()
        .expect("gzip NUL warning")
        .excerpt
        .expect("gzip warning retains decoded bytes");
    assert_eq!(excerpt.bytes(), b"\0");
    assert_eq!(excerpt.view(), ExcerptView::TransportDecodedXmlBytes);
    assert_eq!(excerpt.start_byte(), nul);
    assert_eq!(excerpt.original_problem(), None);
}

#[test]
fn strict_nul_error_retains_the_exact_encoded_excerpt() {
    let xml = "<datafile><game name=\"😀\0\"/></datafile>";
    let nul = xml.find('\0').expect("fixture contains NUL");
    let Err(error) = read_with::<_, Error>(
        xml.as_bytes(),
        NoIntroDatabaseMode::ObservedCompatible,
        |_| Ok(()),
        |(), _| Ok(()),
    ) else {
        panic!("strict mode rejects NUL")
    };
    let Error::CatalogParse {
        excerpt: Some(excerpt),
        line: Some(_),
        column: Some(_),
        ..
    } = error
    else {
        panic!("strict NUL error carries coordinates and an excerpt")
    };
    assert_eq!(excerpt.bytes(), b"\0");
    assert_eq!(excerpt.start_byte(), nul);
    assert_eq!(excerpt.original_problem(), ByteRange::new(nul, nul + 1));

    let utf16 = utf16le_with_bom(xml);
    let encoded_nul = 2 + xml[..nul].encode_utf16().count() * 2;
    let Err(error) = read_with::<_, Error>(
        &utf16,
        NoIntroDatabaseMode::ObservedCompatible,
        |_| Ok(()),
        |(), _| Ok(()),
    ) else {
        panic!("strict mode rejects UTF-16 NUL")
    };
    let Error::CatalogParse {
        excerpt: Some(excerpt),
        line: Some(line),
        column: Some(column),
        coordinates: Some(crate::diagnostics::CoordinateConvention::XmlUnicodeScalars),
        ..
    } = error
    else {
        panic!("UTF-16 strict NUL error carries Unicode coordinates and an excerpt")
    };
    assert_eq!(excerpt.bytes(), &[0, 0]);
    assert_eq!(excerpt.start_byte(), encoded_nul);
    assert_eq!(
        excerpt.original_problem(),
        ByteRange::new(encoded_nul, encoded_nul + 2)
    );
    assert_eq!(line, 1);
    assert_eq!(
        column,
        i64::try_from(xml[..nul].chars().count()).expect("fixture column fits i64") + 1
    );
}

fn document_warning_locations(xml: &str) -> Vec<crate::logiqx::RecordLocation> {
    let prefix_columns = xml
        .split("\r\n")
        .nth(1)
        .expect("fixture has a second CRLF-delimited line")
        .split('\0')
        .next()
        .expect("fixture has text before its first NUL")
        .chars()
        .count();
    let first_column = i64::try_from(prefix_columns).expect("fixture scalar column fits i64") + 1;
    (0..6)
        .map(|index| crate::logiqx::RecordLocation {
            line: 2,
            column: first_column + index,
        })
        .collect()
}

fn utf16le_with_bom(value: &str) -> Vec<u8> {
    let mut bytes = vec![0xff, 0xfe];
    for unit in value.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    bytes
}

#[test]
fn failure_after_a_consumed_game_never_returns_an_eof_proof() {
    let consumed = std::cell::Cell::new(0);
    let result: Result<ValidatedNoIntroDatabase<'_, ()>> = read_with(
        b"<datafile><game name=\"already consumed\"/></datafile><stray/>",
        MODE,
        |_| Ok(()),
        |(), _| {
            consumed.set(consumed.get() + 1);
            Ok(())
        },
    );
    assert!(result.is_err());
    assert_eq!(consumed.get(), 1);
    assert!(read("<datafile><game name=\"g\"><source><details comment1=\"\u{1}\"/></source></game></datafile>").is_err(),
        "XML-forbidden characters other than NUL are never recovered");
}
