#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use super::read_with;
use crate::{
    Error,
    no_intro_dat_xml::{Document, Game, NoIntroDatMode},
};
use sha2::{Digest, Sha256};

type Parsed = (Document, Vec<Game>);

fn read(xml: &str, mode: NoIntroDatMode) -> Result<Parsed, Error> {
    let validated = read_with::<_, Error>(
        xml.as_bytes(),
        mode,
        |document| Ok((document, Vec::new())),
        |(_, games), game| {
            games.push(game);
            Ok(())
        },
    )?;
    Ok(validated.into_inner())
}

fn minimal_header() -> &'static str {
    "<header><id>+0</id><name>Set</name><description>Description</description><version>1</version><author>Author</author></header>"
}

fn valid_v3() -> String {
    "<datafile><header><id>0</id><name>Set</name><description>Desc</description><version>1</version><author>A</author></header><game name=\"game\"><category>Arcade</category><description>Game</description><rom name=\"rom\" size=\"1\" crc=\"not-a-crc\" md5=\"\" sha1=\"not-a-sha1\" status=\"nonstandard status\"/><release name=\"US\" region=\"USA\"/></game></datafile>".to_owned()
}

#[test]
fn producer_schema_fixtures_match_the_fetched_sources() {
    let v3 = include_bytes!("fixtures/schema_nointro_datfile_v3.xsd");
    let v4 = include_bytes!("fixtures/schema_nointro_datfile_v4.xsd");
    let v3 = v3.strip_suffix(b"\n").unwrap_or(v3);
    let v4 = v4.strip_suffix(b"\n").unwrap_or(v4);

    assert_eq!(
        format!("{:x}", Sha256::digest(v3)),
        "728f5ce84c458221eaba4948afe264e151dc27ff8aab59871d4b09aa3978f59c"
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(v4)),
        "23c37039f792ef38c10e884d7662ab99a483d1df6e56b06dfd2b9d66b5ca660f"
    );
}

#[test]
fn observed_mode_accepts_a_required_header_and_zero_games() {
    let xml = "<datafile><header><id>+0</id><name>Set</name><description>Description</description><version>1</version><author>Author</author></header></datafile>";
    let (document, games) = read(xml, NoIntroDatMode::V4Compatible).unwrap();

    assert_eq!(document.header.name.as_ref().unwrap().value, "Set");
    assert!(games.is_empty());
}

#[test]
fn observed_header_comment_uses_the_producer_comment_element() {
    let xml = "<datafile><header><id>0</id><name>Set</name><description>D</description><version>1</version><comment>one</comment></header></datafile>";
    let (document, _) = read(xml, NoIntroDatMode::V4Compatible).unwrap();
    assert_eq!(document.header.comment.unwrap().value, "one");
}

#[test]
fn observed_header_comment_is_a_known_singleton() {
    let xml = "<datafile><header><id>0</id><name>Set</name><description>D</description><version>1</version><comment>one</comment><comment>two</comment></header></datafile>";
    assert!(read(xml, NoIntroDatMode::V4Compatible).is_err());
}

#[test]
fn observed_mode_keeps_empty_optional_values_and_multiple_sparse_roms() {
    let xml = "<datafile><header><id>7</id><name>  Empty  </name><description></description><version>1</version></header>\
         <game name=\"game\" id=\"\"><description>Game</description>\
         <rom name=\"first\" size=\"\" crc=\"\"/><rom name=\"second\" size=\"4294967296\" sha256=\"\"/>\
         </game></datafile>";
    let (document, games) = read(xml, NoIntroDatMode::V3Compatible).unwrap();

    assert_eq!(document.header.name.as_ref().unwrap().value, "  Empty  ");
    assert_eq!(document.header.description.as_ref().unwrap().value, "");
    assert!(document.header.author.is_none());
    assert_eq!(games.len(), 1);
    assert_eq!(games[0].id.as_ref().unwrap().value, "");
    assert_eq!(games[0].roms.len(), 2);
    assert_eq!(games[0].roms[0].size.as_ref().unwrap().value, "");
    assert!(games[0].roms[0].md5.is_none());
    assert_eq!(games[0].roms[1].size.as_ref().unwrap().value, "4294967296");
    assert_eq!(games[0].roms[1].sha256.as_ref().unwrap().value, "");
}

#[test]
fn observed_mode_preserves_declaration_order_and_element_locations() {
    let xml = format!(
        "<datafile>\n{}\n<game name=\"game\">\n<description>Game</description>\n\
         <rom name=\"rom\" size=\"1\"/>\n</game>\n</datafile>",
        minimal_header()
    );
    let (document, games) = read(&xml, NoIntroDatMode::V4Compatible).unwrap();

    let header_name = document.header.name.unwrap();
    let header_description = document.header.description.unwrap();
    assert!(header_name.source_order < header_description.source_order);
    assert_eq!(header_name.location.line, 2);
    assert_eq!(games[0].location.line, 3);
    assert!(games[0].roms[0].location.line > games[0].location.line);
}

#[test]
fn strict_v3_accepts_xsd_integer_lexicals_and_unconstrained_hash_strings() {
    let xml = valid_v3()
        .replace("<id>0</id>", "<id> \t-0\n </id>")
        .replace("size=\"1\"", "size=\" +4294967295 \"");
    let (document, games) = read(&xml, NoIntroDatMode::V3Strict).unwrap();

    assert_eq!(document.header.id.unwrap().value, " \t-0\n ");
    assert_eq!(
        games[0].roms[0].size.as_ref().unwrap().value,
        " +4294967295 "
    );
    assert_eq!(games[0].roms[0].crc.as_ref().unwrap().value, "not-a-crc");
    assert_eq!(games[0].roms[0].md5.as_ref().unwrap().value, "");
    assert_eq!(
        games[0].roms[0].status.as_ref().unwrap().value,
        "nonstandard status"
    );

    let leading_zeroes = valid_v3()
        .replace(
            "<id>0</id>",
            "<id>0000000000000000000000000000000000001</id>",
        )
        .replace(
            "size=\"1\"",
            "size=\"0000000000000000000000000000000000001\"",
        );
    assert!(read(&leading_zeroes, NoIntroDatMode::V3Strict).is_ok());
}

#[test]
fn escaped_reserved_xml_namespace_is_accepted_in_every_mode() {
    let xml = valid_v3().replace(
        "<datafile>",
        "<datafile xmlns:xml=\"http://www.w3.org/XML/1998/name&#x73;pace\">",
    );
    for mode in [
        NoIntroDatMode::V3Strict,
        NoIntroDatMode::V3Compatible,
        NoIntroDatMode::V4Strict,
        NoIntroDatMode::V4Compatible,
    ] {
        assert!(
            read(&xml, mode).is_ok(),
            "rejected normalized xml binding in {mode:?}"
        );
        let wrong = xml.replace("name&#x73;pace", "wrong&#x73;pace");
        assert!(
            read(&wrong, mode).is_err(),
            "accepted incorrect xml binding in {mode:?}"
        );
    }
}

#[test]
fn strict_v4_accepts_its_optional_header_extensions() {
    let xml = valid_v3().replace(
        "<author>A</author>",
        "<author>A</author><trademarks>T</trademarks><piracy>P</piracy>",
    );
    let (document, _) = read(&xml, NoIntroDatMode::V4Strict).unwrap();

    assert_eq!(document.header.trademarks.unwrap().value, "T");
    assert_eq!(document.header.piracy.unwrap().value, "P");
}

#[test]
fn strict_v3_rejects_v4_header_extensions() {
    let xml = valid_v3().replace(
        "<author>A</author>",
        "<author>A</author><trademarks>T</trademarks>",
    );
    assert!(read(&xml, NoIntroDatMode::V3Strict).is_err());
}

#[test]
fn strict_schema_location_is_a_hint_and_must_not_contradict_the_mode() {
    let xml = valid_v3().replace(
        "<datafile>",
        "<datafile xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:schemaLocation=\"urn:vendor https://datomatic.no-intro.org/stuff/schema_nointro_datfile_v3.xsd\">",
    );
    let (document, _) = read(&xml, NoIntroDatMode::V3Strict).unwrap();
    assert!(document.schema_location.is_some());

    let contradictory = xml.replace("datfile_v3.xsd", "datfile_v4.xsd");
    assert!(read(&contradictory, NoIntroDatMode::V3Strict).is_err());

    let incomplete = xml.replace("<author>A</author>", "");
    assert!(read(&incomplete, NoIntroDatMode::V3Strict).is_err());

    let no_namespace = valid_v3().replace(
        "<datafile>",
        "<datafile xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:noNamespaceSchemaLocation=\"https://datomatic.no-intro.org/stuff/schema_nointro_datfile_v3.xsd\">",
    );
    assert!(read(&no_namespace, NoIntroDatMode::V3Strict).is_ok());
    let contradictory = no_namespace.replace("datfile_v3.xsd", "datfile_v4.xsd");
    assert!(read(&contradictory, NoIntroDatMode::V3Strict).is_err());

    let uri_with_space = valid_v3().replace(
        "<datafile>",
        "<datafile xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:noNamespaceSchemaLocation=\"https://example.test/vendor docs/schema_nointro_datfile_v3.xsd\">",
    );
    assert!(read(&uri_with_space, NoIntroDatMode::V3Strict).is_ok());

    for location in ["", "https://example.test/a b"] {
        let xml = valid_v3().replace(
            "<datafile>",
            &format!(
                "<datafile xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:noNamespaceSchemaLocation=\"{location}\">"
            ),
        );
        assert!(
            read(&xml, NoIntroDatMode::V3Strict).is_ok(),
            "rejected {location:?}"
        );
    }
}

#[test]
fn strict_mode_rejects_out_of_range_and_malformed_integer_values() {
    let overflow = valid_v3().replace("size=\"1\"", "size=\"4294967296\"");
    assert!(read(&overflow, NoIntroDatMode::V3Strict).is_err());

    let malformed = valid_v3().replace("<id>0</id>", "<id>1 2</id>");
    assert!(read(&malformed, NoIntroDatMode::V3Strict).is_err());

    let negative_unsigned = valid_v3().replace("size=\"1\"", "size=\"-1\"");
    assert!(read(&negative_unsigned, NoIntroDatMode::V3Strict).is_err());
    let negative_zero = valid_v3().replace("size=\"1\"", "size=\"-0\"");
    assert!(read(&negative_zero, NoIntroDatMode::V3Strict).is_ok());
}

#[test]
fn strict_mode_rejects_schema_order_unknown_fields_and_namespace_changes() {
    let reordered = valid_v3().replace(
        "<description>Desc</description><version>1</version>",
        "<version>1</version><description>Desc</description>",
    );
    assert!(read(&reordered, NoIntroDatMode::V3Strict).is_err());

    let unknown_attribute = valid_v3().replace("<game name=", "<game vendor:flag=\"x\" name=");
    let unknown_attribute =
        unknown_attribute.replace("<datafile>", "<datafile xmlns:vendor=\"urn:vendor\">");
    assert!(read(&unknown_attribute, NoIntroDatMode::V3Strict).is_err());

    let default_namespace = valid_v3().replace("<datafile>", "<datafile xmlns=\"urn:vendor\">");
    assert!(read(&default_namespace, NoIntroDatMode::V3Strict).is_err());

    let namespaced_field = valid_v3()
        .replace("<datafile>", "<datafile xmlns:v=\"urn:vendor\">")
        .replace(
            "<category>Arcade</category>",
            "<v:category>Arcade</v:category>",
        );
    assert!(read(&namespaced_field, NoIntroDatMode::V3Strict).is_err());
    assert!(read(&namespaced_field, NoIntroDatMode::V3Compatible).is_ok());
}

#[test]
fn strict_mode_handles_xsi_nil_and_narrowed_xsi_type() {
    let valid_xsi = valid_v3()
        .replace(
            "<datafile>",
            "<datafile xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\">",
        )
        .replace("<name>Set</name>", "<name xsi:type=\"xs:token\">Set</name>");
    assert!(read(&valid_xsi, NoIntroDatMode::V3Strict).is_ok());

    let nilled_scalar = valid_v3()
        .replace(
            "<datafile>",
            "<datafile xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">",
        )
        .replace("<name>Set</name>", "<name xsi:nil=\"true\"/>");
    assert!(read(&nilled_scalar, NoIntroDatMode::V3Strict).is_err());

    let incompatible_type = valid_v3()
        .replace(
            "<datafile>",
            "<datafile xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\">",
        )
        .replace("<name>Set</name>", "<name xsi:type=\"xs:int\">1</name>");
    assert!(read(&incompatible_type, NoIntroDatMode::V3Strict).is_err());

    let invalid_nil = valid_v3()
        .replace(
            "<datafile>",
            "<datafile xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">",
        )
        .replace("<name>Set</name>", "<name xsi:nil=\"perhaps\">Set</name>");
    assert!(read(&invalid_nil, NoIntroDatMode::V3Strict).is_err());

    for value in ["false", "0"] {
        let nil_attribute = valid_v3().replace(
            "<datafile>",
            &format!("<datafile xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:nil=\"{value}\">"),
        );
        assert!(read(&nil_attribute, NoIntroDatMode::V3Strict).is_err());
    }

    let compatible_nilled_scalar = valid_v3()
        .replace(
            "<datafile>",
            "<datafile xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">",
        )
        .replace("<name>Set</name>", "<name xsi:nil=\"true\"/>");
    assert!(read(&compatible_nilled_scalar, NoIntroDatMode::V3Compatible).is_err());

    let narrowed_out_of_range = valid_v3()
        .replace(
            "<datafile>",
            "<datafile xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\">",
        )
        .replace("<id>0</id>", "<id xsi:type=\"xs:short\">32768</id>");
    assert!(read(&narrowed_out_of_range, NoIntroDatMode::V3Strict).is_err());

    let unknown_xsi_attribute = valid_v3()
        .replace(
            "<datafile>",
            "<datafile xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">",
        )
        .replace("<name>Set</name>", "<name xsi:vendor=\"x\">Set</name>");
    assert!(read(&unknown_xsi_attribute, NoIntroDatMode::V3Strict).is_err());
}

#[test]
fn strict_id_and_idref_types_enforce_document_wide_constraints() {
    let with_xsi = valid_v3().replace(
        "<datafile>",
        "<datafile xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\">",
    );
    let forward_reference = with_xsi
        .replace(
            "<name>Set</name>",
            "<name xsi:type=\"xs:IDREF\">record1</name>",
        )
        .replace(
            "<description>Desc</description>",
            "<description xsi:type=\"xs:ID\">record1</description>",
        );
    assert!(read(&forward_reference, NoIntroDatMode::V3Strict).is_ok());

    let unresolved_idref = with_xsi.replace(
        "<name>Set</name>",
        "<name xsi:type=\"xs:IDREF\">missing</name>",
    );
    assert!(read(&unresolved_idref, NoIntroDatMode::V3Strict).is_err());

    let duplicate_ids = with_xsi
        .replace("<description>Game</description>", "<description xsi:type=\"xs:ID\">duplicate</description>")
        .replace(
            "</datafile>",
            "<game name=\"second\"><description xsi:type=\"xs:ID\">duplicate</description><rom name=\"second-rom\" size=\"1\" crc=\"c\" md5=\"m\" sha1=\"s\"/></game></datafile>",
        );
    assert!(read(&duplicate_ids, NoIntroDatMode::V3Strict).is_err());
}

#[test]
fn observed_ordinals_keep_child_and_attribute_domains_independent() {
    let xml = "<datafile><header xmlns:v=\"urn:v\"><id>0</id><name>Set</name><description>D</description><version>1</version><author>A</author></header><v:extension xmlns:v=\"urn:v\"/><!----><game xmlns:v=\"urn:v\" name=\"game\" v:flag=\"x\" id=\"id\"><v:extension/><category>C</category><description>G</description><rom name=\"r\" size=\"1\"/></game></datafile>";
    let (document, games) = read(xml, NoIntroDatMode::V4Compatible).unwrap();

    assert_eq!(document.header.name.as_ref().unwrap().source_order, 1);
    assert_eq!(games[0].list_order, 0);
    assert_eq!(games[0].source_order, 2);
    assert_eq!(games[0].name.source_order, 1);
    assert_eq!(games[0].id.as_ref().unwrap().source_order, 3);
    assert_eq!(games[0].categories[0].source_order, 1);
    assert_eq!(games[0].description.as_ref().unwrap().source_order, 2);
    assert_eq!(games[0].roms[0].source_order, 3);
}

#[test]
fn observed_mode_rejects_duplicate_known_singletons() {
    let xml = "<datafile><header><id>1</id><name>A</name><name>B</name><description>D</description><version>1</version></header></datafile>";
    assert!(read(xml, NoIntroDatMode::V4Compatible).is_err());
}

#[test]
fn observed_mode_rejects_duplicate_expanded_attribute_names() {
    let xml = "<datafile><header><id>0</id><name>Set</name><description>D</description><version>1</version></header><game xmlns:a=\"urn:vendor\" xmlns:b=\"urn:vendor\" name=\"g\" a:flag=\"x\" b:flag=\"y\"><description>D</description><rom name=\"r\"/></game></datafile>";
    assert!(read(xml, NoIntroDatMode::V4Compatible).is_err());
}

#[test]
fn escaped_vendor_namespace_uri_is_accepted_in_both_modes() {
    let strict = valid_v3().replace(
        "<datafile>",
        "<datafile xmlns:v=\"urn:a&amp;b\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema&#x2D;instance\" xmlns:xs=\"http://www.w3.org/2001/XML&#x53;chema\" xsi:schemaLocation=\"urn:a&amp;b https://vendor.test/schema.xsd\">",
    ).replace("<name>Set</name>", "<name xsi:type=\"xs:token\">Set</name>");
    assert!(read(&strict, NoIntroDatMode::V3Strict).is_ok());

    let compatible = valid_v3()
        .replace("<datafile>", "<datafile xmlns:v=\"urn:a&amp;b\">")
        .replace(
            "<game name=\"game\">",
            "<game xmlns:v=\"urn:a&amp;b\" name=\"game\"><v:extension/>",
        );
    assert!(read(&compatible, NoIntroDatMode::V3Compatible).is_ok());
}

#[test]
fn strict_container_whitespace_is_xml_whitespace_only() {
    let xml_whitespace = valid_v3().replace("<datafile>", "<datafile> \t\r\n");
    assert!(read(&xml_whitespace, NoIntroDatMode::V3Strict).is_ok());

    let unicode_whitespace = valid_v3().replace("<datafile>", "<datafile>\u{00a0}");
    assert!(read(&unicode_whitespace, NoIntroDatMode::V3Strict).is_err());
}

#[test]
fn strict_mode_enforces_required_fields_and_rom_cardinality() {
    let cases = [
        valid_v3().replace("<id>0</id>", ""),
        valid_v3().replace("<author>A</author>", ""),
        valid_v3().replace("<description>Game</description>", ""),
        valid_v3().replace("<rom name=\"rom\" size=\"1\" crc=\"not-a-crc\" md5=\"\" sha1=\"not-a-sha1\" status=\"nonstandard status\"/>", ""),
        valid_v3().replace(" size=\"1\"", ""),
        valid_v3().replace(
            "<rom name=\"rom\" size=\"1\" crc=\"not-a-crc\" md5=\"\" sha1=\"not-a-sha1\" status=\"nonstandard status\"/>",
            "<rom name=\"rom\" size=\"1\" crc=\"not-a-crc\" md5=\"\" sha1=\"not-a-sha1\" status=\"nonstandard status\"/><rom name=\"second\" size=\"0\" crc=\"c\" md5=\"m\" sha1=\"s\"/>",
        ),
    ];
    for xml in cases {
        assert!(
            read(&xml, NoIntroDatMode::V3Strict).is_err(),
            "accepted {xml}"
        );
    }
}

#[test]
fn document_framing_rejects_doctypes_and_multiple_roots() {
    let doctype = valid_v3().replace(
        "<datafile>",
        "<!DOCTYPE datafile [<!ENTITY e \"expanded\">]><datafile>",
    );
    assert!(read(&doctype, NoIntroDatMode::V4Compatible).is_err());

    let second_root = format!("{}<other/>", valid_v3());
    assert!(read(&second_root, NoIntroDatMode::V4Compatible).is_err());

    for cdata in ["", " \t\r\n"] {
        let document = valid_v3();
        let before_root = format!("<![CDATA[{cdata}]]>{document}");
        let after_root = format!("{document}<![CDATA[{cdata}]]>");
        assert!(read(&before_root, NoIntroDatMode::V4Compatible).is_err());
        assert!(read(&after_root, NoIntroDatMode::V4Compatible).is_err());
    }
}

#[test]
fn strict_element_only_records_reject_even_whitespace_text() {
    let cases = [
        valid_v3().replace(
            "<rom name=\"rom\" size=\"1\" crc=\"not-a-crc\" md5=\"\" sha1=\"not-a-sha1\" status=\"nonstandard status\"/>",
            "<rom name=\"rom\" size=\"1\" crc=\"not-a-crc\" md5=\"\" sha1=\"not-a-sha1\" status=\"nonstandard status\"> </rom>",
        ),
        valid_v3().replace(
            "<rom name=\"rom\" size=\"1\" crc=\"not-a-crc\" md5=\"\" sha1=\"not-a-sha1\" status=\"nonstandard status\"/>",
            "<rom name=\"rom\" size=\"1\" crc=\"not-a-crc\" md5=\"\" sha1=\"not-a-sha1\" status=\"nonstandard status\"><![CDATA[ ]]></rom>",
        ),
        valid_v3().replace("<release name=\"US\" region=\"USA\"/>", "<release name=\"US\" region=\"USA\"> </release>"),
        valid_v3().replace(
            "</author></header>",
            "</author><clrmamepro> </clrmamepro></header>",
        ),
        valid_v3().replace(
            "</author></header>",
            "</author><romcenter> </romcenter></header>",
        ),
    ];
    for xml in cases {
        assert!(
            read(&xml, NoIntroDatMode::V3Strict).is_err(),
            "accepted {xml}"
        );
    }

    let comments_and_pi = valid_v3().replace(
        "<release name=\"US\" region=\"USA\"/>",
        "<release name=\"US\" region=\"USA\"><!--valid--><?producer ok?></release>",
    );
    assert!(read(&comments_and_pi, NoIntroDatMode::V3Strict).is_ok());
}

#[test]
fn strict_mode_accepts_nested_schema_hints_and_valid_string_derived_types() {
    let cases = [
        (
            "<name>Set</name>",
            "<name xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:schemaLocation=\"urn:vendor https://example.test/vendor.xsd\">Set</name>",
        ),
        (
            "<name>Set</name>",
            "<name xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xsi:type=\"xs:Name\">a:b</name>",
        ),
        (
            "<name>Set</name>",
            "<name xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xsi:type=\"xs:Name\">éclair</name>",
        ),
        (
            "<name>Set</name>",
            "<name xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xsi:type=\"xs:NCName\">a-b</name>",
        ),
        (
            "<name>Set</name>",
            "<name xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xsi:type=\"xs:NMTOKEN\">1:a</name>",
        ),
        (
            "<name>Set</name>",
            "<name xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xsi:type=\"xs:language\">en-US</name>",
        ),
        (
            "<name>Set</name>",
            "<name xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xsi:type=\"xs:ID\">record1</name>",
        ),
        (
            "<name>Set</name>",
            "<name xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xsi:type=\"xs:normalizedString\">a&#x9;b</name>",
        ),
    ];
    for (needle, replacement) in cases {
        let xml = valid_v3().replace(needle, replacement);
        assert!(
            read(&xml, NoIntroDatMode::V3Strict).is_ok(),
            "rejected {replacement}"
        );
    }

    let invalid_language = valid_v3()
        .replace(
            "<datafile>",
            "<datafile xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\">",
        )
        .replace("<name>Set</name>", "<name xsi:type=\"xs:language\">en_US</name>");
    assert!(read(&invalid_language, NoIntroDatMode::V3Strict).is_err());

    for (type_name, value) in [
        ("Name", "1bad"),
        ("NCName", "a:b"),
        ("NMTOKEN", "two words"),
        ("ID", "1bad"),
        ("IDREF", "a:b"),
        ("ENTITY", "unparsedEntity"),
        ("NMTOKENS", "one two"),
    ] {
        let invalid_value = valid_v3()
            .replace(
                "<datafile>",
                "<datafile xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\">",
            )
            .replace(
                "<name>Set</name>",
                &format!("<name xsi:type=\"xs:{type_name}\">{value}</name>"),
            );
        assert!(read(&invalid_value, NoIntroDatMode::V3Strict).is_err());
    }

    let nested_no_namespace_hint = valid_v3()
        .replace(
            "<name>Set</name>",
            "<name xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:noNamespaceSchemaLocation=\"https://example.test/vendor.xsd\">Set</name>",
        );
    assert!(read(&nested_no_namespace_hint, NoIntroDatMode::V3Strict).is_ok());

    let nested_contradictory_hint =
        nested_no_namespace_hint.replace("vendor.xsd", "schema_nointro_datfile_v4.xsd");
    assert!(read(&nested_contradictory_hint, NoIntroDatMode::V3Strict).is_err());
}

#[test]
fn cdata_uses_xml_line_end_normalization() {
    let xml = valid_v3().replace("<name>Set</name>", "<name><![CDATA[a\r\nb\rc]]></name>");
    let (document, _) = read(&xml, NoIntroDatMode::V4Compatible).unwrap();
    assert_eq!(document.header.name.unwrap().value, "a\nb\nc");

    let token_whitespace = valid_v3().replace(
        "<name>Set</name>",
        "<name xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xsi:type=\"xs:token\"><![CDATA[  a\r\n  b  ]]></name>",
    );
    let (document, _) = read(&token_whitespace, NoIntroDatMode::V3Strict).unwrap();
    assert_eq!(document.header.name.unwrap().value, "  a\n  b  ");
}

#[test]
fn observed_mode_still_rejects_malformed_xml() {
    let xml = "<datafile><header><id>+0</id><name>Set</name><description>Description</description><version>1</version><author>Author</author></header></datafile";
    assert!(read(xml, NoIntroDatMode::V4Compatible).is_err());
}
