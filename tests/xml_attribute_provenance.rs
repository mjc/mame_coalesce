use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query, sql_types::BigInt,
};
use mame_coalesce::{
    NoIntroDatMode,
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
    no_intro_db_xml::NoIntroDatabaseMode,
};
use std::io::Write as _;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(Clone, Copy)]
enum Encoding {
    Utf8,
    Utf16Le,
    Utf16Be,
    Gzip,
}

impl Encoding {
    fn encode(self, xml: &str) -> TestResult<Vec<u8>> {
        match self {
            Self::Utf8 => Ok(xml.as_bytes().to_vec()),
            Self::Utf16Le | Self::Utf16Be => {
                let little = matches!(self, Self::Utf16Le);
                let mut bytes = if little {
                    vec![0xff, 0xfe]
                } else {
                    vec![0xfe, 0xff]
                };
                bytes.extend(xml.encode_utf16().flat_map(|unit| {
                    if little {
                        unit.to_le_bytes()
                    } else {
                        unit.to_be_bytes()
                    }
                }));
                Ok(bytes)
            }
            Self::Gzip => {
                let mut encoder =
                    flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
                encoder.write_all(xml.as_bytes())?;
                Ok(encoder.finish()?)
            }
        }
    }
}

struct Imported {
    _directory: tempfile::TempDir,
    connection: SqliteConnection,
}

impl Imported {
    fn new(xml: &str, format: CatalogDocumentFormat, encoding: Encoding) -> TestResult<Self> {
        let directory = tempfile::tempdir()?;
        let db_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
        let input_path = Utf8PathBuf::try_from(directory.path().join(
            if matches!(encoding, Encoding::Gzip) {
                "source.xml.gz"
            } else {
                "source.xml"
            },
        ))?;
        let original = encoding.encode(xml)?;
        std::fs::write(&input_path, &original)?;
        let database = Database::open(&db_path)?;
        let report = app::import_catalog(
            &database,
            &CatalogImportRequest {
                document_path: input_path,
                format,
                source_key: PublishingSourceKey::new("attribute-tokens"),
                source_display_name: "Attribute tokens".into(),
                catalog_key: CatalogKey::new("attribute-tokens"),
                catalog_display_name: "Attribute tokens".into(),
                scope: CatalogScope::Complete,
            },
        )?;
        if report.status != CatalogImportStatus::Succeeded {
            let mut connection = SqliteConnection::establish(db_path.as_str())?;
            let messages = sql_query(
                "SELECT message AS value FROM import_diagnostics ORDER BY diagnostic_key",
            )
            .load::<Diagnostic>(&mut connection)?;
            return Err(format!(
                "attribute fixture import failed: {:?}",
                messages
                    .into_iter()
                    .map(|message| message.value)
                    .collect::<Vec<_>>()
            )
            .into());
        }
        assert_eq!(
            app::load_snapshot_source(&database, &report.snapshot_key.ok_or("missing snapshot")?)?,
            original
        );
        Ok(Self {
            _directory: directory,
            connection: SqliteConnection::establish(db_path.as_str())?,
        })
    }

    fn assert_field(
        &mut self,
        xml: &str,
        table: &str,
        field: i64,
        order: i64,
        token: &str,
    ) -> TestResult {
        let stored = sql_query(format!(
            "SELECT source_order,source_line,source_column FROM {table} WHERE field_kind=?"
        ))
        .bind::<BigInt, _>(field)
        .get_result::<Position>(&mut self.connection)?;
        assert_eq!(stored.order, order, "{table}.{field} lexical ordinal");
        assert_eq!(
            (stored.line, stored.column),
            token_location(xml, token)?,
            "{table}.{field} must locate its attribute QName, not the opening tag"
        );
        Ok(())
    }
}

#[derive(QueryableByName)]
struct Diagnostic {
    #[diesel(sql_type=diesel::sql_types::Text)]
    value: String,
}

#[derive(QueryableByName)]
struct Position {
    #[diesel(sql_type=BigInt, column_name=source_order)]
    order: i64,
    #[diesel(sql_type=BigInt, column_name=source_line)]
    line: i64,
    #[diesel(sql_type=BigInt, column_name=source_column)]
    column: i64,
}

fn token_location(xml: &str, token: &str) -> TestResult<(i64, i64)> {
    let prefix = xml
        .get(..xml.find(token).ok_or("missing fixture token")?)
        .ok_or("invalid token boundary")?;
    let mut characters = prefix.chars().peekable();
    let (mut line, mut column) = (1, 1);
    while let Some(character) = characters.next() {
        match character {
            '\r' => {
                line += 1;
                column = 1;
                if characters.peek() == Some(&'\n') {
                    characters.next();
                }
            }
            '\n' => {
                line += 1;
                column = 1;
            }
            _ => column += 1,
        }
    }
    Ok((line, column))
}

const DAT: &str = "<datafile xmlns:v='urn:vendor'>\r\n<header><id>1</id><name>Pack</name><description/><version>1</version><clrmamepro v:ignored='skip'\r\n\tforcenodump='obsolete' header=''/><romcenter plugin='démo😀'/></header>\r\n<game v:ignored='skip' name='démo😀'\r\n\tid='0007' cloneof='parent' cloneofid='0008'><description/>\r\n<release name='European😀'\tregion='EUR'/>\r\n<rom xmlns:w='urn:vendor' w:ignored='skip' name='démo.bin'\r\n\tsize='0004' crc='ABCDEF01'/></game></datafile>";

#[test]
fn dat_native_attribute_positions_identify_each_token_across_encodings() -> TestResult {
    for encoding in [
        Encoding::Utf8,
        Encoding::Utf16Le,
        Encoding::Utf16Be,
        Encoding::Gzip,
    ] {
        let mut imported = Imported::new(
            DAT,
            CatalogDocumentFormat::NoIntroDat(NoIntroDatMode::V4Compatible),
            encoding,
        )?;
        for (table, field, order, token) in [
            (
                "no_intro_dat_clrmamepro_field_positions",
                0,
                1,
                "forcenodump='",
            ),
            ("no_intro_dat_clrmamepro_field_positions", 1, 2, "header=''"),
            ("no_intro_dat_romcenter_field_positions", 0, 0, "plugin='"),
            ("no_intro_dat_game_field_positions", 0, 1, "name='démo😀'"),
            ("no_intro_dat_game_field_positions", 1, 2, "id='0007'"),
            ("no_intro_dat_game_field_positions", 2, 3, "cloneof='"),
            ("no_intro_dat_game_field_positions", 3, 4, "cloneofid='"),
            ("no_intro_dat_rom_field_positions", 0, 2, "name='démo.bin'"),
            ("no_intro_dat_rom_field_positions", 1, 3, "size='0004'"),
            ("no_intro_dat_rom_field_positions", 2, 4, "crc='"),
        ] {
            imported.assert_field(DAT, table, field, order, token)?;
        }
        for (order, field) in [(0, "name"), (1, "region")] {
            let row = sql_query(format!("SELECT {field}_source_order AS source_order,{field}_source_line AS source_line,{field}_source_column AS source_column FROM no_intro_dat_releases"))
                .get_result::<Position>(&mut imported.connection)?;
            assert_eq!(row.order, order);
            assert_eq!(
                (row.line, row.column),
                token_location(
                    DAT,
                    &format!(
                        "{field}='{}",
                        if field == "name" { "European" } else { "EUR" }
                    )
                )?
            );
        }
    }
    Ok(())
}

const EXPORT: &str = "<datafile>\r\n<game\r\n\tname='Export😀'><archive number='0007' name='démo😀'\tclone='0008' region='JPN'/>\r\n<source><details comment1='idéé😀'\r\n\tid='dump-7'/><serials media_serial1='S-1'/>\r\n<file id='file-9' item='rom'\r\n\tsize='0004'/></source>\r\n<release><details comment='rel😀'\tid='rel-11'/><serials box_serial='B-1'/>\r\n<file id='relfile-12'\r\n\tsize='0008'/></release></game></datafile>";

#[test]
fn imports_identify_the_attribute_token_coordinate_rules() -> TestResult {
    for (xml, format, rules) in [
        (
            DAT,
            CatalogDocumentFormat::NoIntroDat(NoIntroDatMode::V4Compatible),
            "no-intro-dat-observed-compat-v2",
        ),
        (
            EXPORT,
            CatalogDocumentFormat::NoIntroDatabase(NoIntroDatabaseMode::ObservedCompatible),
            "no-intro-database-observed-compat-v2",
        ),
        (
            EXPORT,
            CatalogDocumentFormat::NoIntroDatabase(NoIntroDatabaseMode::NullRecoveryCompatible),
            "no-intro-database-nul-recovery-v2",
        ),
    ] {
        let mut imported = Imported::new(xml, format, Encoding::Utf8)?;
        let stored = sql_query("SELECT rules_version AS value FROM parser_interpretations")
            .get_result::<Diagnostic>(&mut imported.connection)?;
        assert_eq!(stored.value, rules);
    }
    Ok(())
}

#[test]
fn database_export_native_attribute_positions_identify_each_actual_owner_token() -> TestResult {
    for encoding in [
        Encoding::Utf8,
        Encoding::Utf16Le,
        Encoding::Utf16Be,
        Encoding::Gzip,
    ] {
        let mut imported = Imported::new(
            EXPORT,
            CatalogDocumentFormat::NoIntroDatabase(NoIntroDatabaseMode::ObservedCompatible),
            encoding,
        )?;
        for (table, field, order, token) in [
            ("no_intro_archive_field_positions", 20, 0, "number='"),
            ("no_intro_archive_field_positions", 18, 1, "name='démo😀'"),
            ("no_intro_archive_field_positions", 30, 2, "clone='"),
            ("no_intro_archive_field_positions", 22, 3, "region='JPN'"),
            ("no_intro_dump_details_field_positions", 5, 1, "id='dump-7'"),
            (
                "no_intro_dump_serials_field_positions",
                6,
                0,
                "media_serial1='",
            ),
            ("no_intro_dump_file_field_positions", 9, 0, "id='file-9'"),
            ("no_intro_dump_file_field_positions", 19, 2, "size='0004'"),
            (
                "no_intro_release_details_field_positions",
                6,
                1,
                "id='rel-11'",
            ),
            (
                "no_intro_release_serials_field_positions",
                1,
                0,
                "box_serial='",
            ),
            (
                "no_intro_release_file_field_positions",
                7,
                0,
                "id='relfile-12'",
            ),
            (
                "no_intro_release_file_field_positions",
                14,
                1,
                "size='0008'",
            ),
        ] {
            imported.assert_field(EXPORT, table, field, order, token)?;
        }
        let game = sql_query("SELECT name_source_order AS source_order,name_source_line AS source_line,name_source_column AS source_column FROM no_intro_database_games")
            .get_result::<Position>(&mut imported.connection)?;
        assert_eq!(game.order, 0);
        assert_eq!(
            (game.line, game.column),
            token_location(EXPORT, "name='Export😀'")?
        );
    }
    Ok(())
}
