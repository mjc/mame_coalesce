pub const ARCHIVE_FIELDS: &[&str] = &[
    "additional",
    "aftermarket",
    "alt",
    "bios",
    "categories",
    "complete",
    "dat",
    "datter_note",
    "description",
    "devstatus",
    "gameid1",
    "gameid2",
    "langchecked",
    "languages",
    "licensed",
    "listed",
    "mergename",
    "name",
    "name_alt",
    "number",
    "physical",
    "region",
    "regparent",
    "showlang",
    "special1",
    "special2",
    "sticky_note",
    "version1",
    "version2",
];

pub const DUMP_DETAILS_FIELDS: &[&str] = &[
    "comment1",
    "comment2",
    "d_date",
    "d_date_info",
    "dumper",
    "id",
    "link1",
    "link2",
    "link3",
    "media_title",
    "nodump",
    "origin",
    "originalformat",
    "project",
    "r_date",
    "r_date_info",
    "region",
    "rominfo",
    "section",
    "tool",
];

pub const DUMP_DETAILS_FIELDS_B: &[&str] = &[
    "comment1",
    "comment2",
    "d_date_info",
    "dumper",
    "link1",
    "link2",
    "link3",
    "media_title",
    "nodump",
    "origin",
    "originalformat",
    "project",
    "r_date",
    "r_date_info",
    "region",
    "rominfo",
    "section",
    "tool",
];

pub const DUMP_SERIAL_FIELDS: &[&str] = &[
    "box_barcode",
    "box_serial",
    "chip_serial",
    "digital_serial1",
    "digital_serial2",
    "lockout_serial",
    "media_serial1",
    "media_serial2",
    "media_serial3",
    "mediastamp",
    "pcb_serial",
    "romchip_serial1",
    "romchip_serial2",
    "savechip_serial",
];

pub const DUMP_SERIAL_FIELDS_B: &[&str] = &[
    "box_barcode",
    "chip_serial",
    "digital_serial1",
    "digital_serial2",
    "lockout_serial",
    "media_serial1",
    "media_serial2",
    "media_serial3",
    "mediastamp",
    "pcb_serial",
    "romchip_serial1",
    "romchip_serial2",
];

pub const DUMP_FILE_FIELDS: &[&str] = &[
    "bad",
    "date",
    "extension",
    "filter",
    "forcename",
    "forcescenename",
    "format",
    "header",
    "item",
    "mia",
    "note",
    "origin_size",
    "serial",
    "unique",
    "update_type",
    "version",
];

pub const RELEASE_DETAILS_FIELDS: &[&str] = &[
    "archivename",
    "category",
    "comment",
    "date",
    "dirname",
    "group",
    "id",
    "nfo_size",
    "nfoname",
    "nfosize",
    "origin",
    "originalformat",
    "region",
    "rominfo",
    "tool",
];

pub const RELEASE_SERIAL_FIELDS: &[&str] = &[
    "box_barcode",
    "box_serial",
    "media_serial1",
    "mediastamp",
    "pcb_serial",
    "romchip_serial1",
];

pub const RELEASE_FILE_FIELDS: &[&str] = &[
    "bad",
    "extension",
    "forcename",
    "forcescenename",
    "format",
    "header",
    "id",
    "item",
    "note",
    "serial",
    "update_type",
    "version",
];

pub const HASHES_A: [&str; 5] = [
    "AaBbCcDd",
    "11111111111111111111111111111111",
    "2222222222222222222222222222222222222222",
    "3333333333333333333333333333333333333333333333333333333333333333",
    "4444444444444444444444444444444444444444444444444444444444444444",
];

pub const HASHES_B: [&str; 4] = [
    "8899Aabb",
    "99999999999999999999999999999999",
    "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
];

pub const HASHES_RELEASE: [&str; 3] = [
    "55555555555555555555555555555555",
    "6666666666666666666666666666666666666666",
    "7777777777777777777777777777777777777777777777777777777777777777",
];

fn attributes(prefix: &str, fields: &[&str]) -> String {
    let mut result = String::new();
    for field in fields {
        result.push(' ');
        result.push_str(field);
        result.push_str("='");
        result.push_str(prefix);
        result.push(':');
        result.push_str(field);
        result.push('\'');
    }
    result
}

pub fn all_fields_xml() -> String {
    let archive = attributes("archive-a", ARCHIVE_FIELDS).replace(
        " additional='archive-a:additional'",
        " additional='archive-a:additional' adult=''",
    );
    let dump_details = attributes("dump-details-a", DUMP_DETAILS_FIELDS);
    let dump_serials = attributes("dump-serials-a", DUMP_SERIAL_FIELDS);
    let dump_file = attributes("dump-file-a", DUMP_FILE_FIELDS);
    let release_details = attributes("release-details-a", RELEASE_DETAILS_FIELDS);
    let release_serials = attributes("release-serials-a", RELEASE_SERIAL_FIELDS);
    let release_file = attributes("release-file-a", RELEASE_FILE_FIELDS);
    let other_dump_details = attributes("dump-details-b", DUMP_DETAILS_FIELDS)
        .replace(" id='dump-details-b:id'", " id=''")
        .replace(" d_date='dump-details-b:d_date'", "");
    let other_dump_serials = attributes("dump-serials-b", DUMP_SERIAL_FIELDS)
        .replace(" box_serial='dump-serials-b:box_serial'", " box_serial=''")
        .replace(" savechip_serial='dump-serials-b:savechip_serial'", "");
    let other_dump_file = attributes("dump-file-b", DUMP_FILE_FIELDS);
    format!(
        "<datafile>\n<header><author>header-author-a</author><piracy></piracy><author>header-author-b</author><url>header-url</url></header>\n\
         <game name='same-publisher-name'>\n\
         <archive{archive} clone='P' mergeof='opaque-merge-declaration'/>\n\
         <source>\n<details{dump_details}/>\n<serials{dump_serials}/>\n\
         <file{dump_file} id='same-publisher-id' size='0003' crc32='{}' md5='{}' sha1='{}' sha256='{}' origin_sha256='{}'/>\n</source>\n\
         <archive name='second-archive' clone='ambiguous-parent-number'/>\n\
         <release>\n<details{release_details} nfo_crc32='CcDdEeFf' nfocrc='not-a-hash'/>\n\
         <serials{release_serials}/>\n<file{release_file} size='0004' crc32='EeFf0011' md5='{}' sha1='{}' sha256='{}'/>\n</release>\n\
         <release>\n<details comment=''/>\n<serials media_serial1=''/>\n</release>\n\
         </game>\n\
         <game name='same-publisher-name'>\n<archive number='same-publisher-id'/>\n\
         <source>\n<details{other_dump_details}/>\n<serials{other_dump_serials}/>\n\
         <file{other_dump_file} id='same-publisher-id' size='0005' crc32='{}' md5='{}' sha1='{}' sha256='{}' origin_sha256='not-a-digest'/>\n\
         </source>\n</game>\n</datafile>",
        HASHES_A[0],
        HASHES_A[1],
        HASHES_A[2],
        HASHES_A[3],
        HASHES_A[4],
        HASHES_RELEASE[0],
        HASHES_RELEASE[1],
        HASHES_RELEASE[2],
        HASHES_B[0],
        HASHES_B[1],
        HASHES_B[2],
        HASHES_B[3],
    )
}
