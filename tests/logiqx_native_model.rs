#![allow(clippy::expect_used)]

use mame_coalesce::logiqx::DataFile;

#[test]
fn native_import_accepts_an_absent_header_without_inventing_a_title()
-> Result<(), Box<dyn std::error::Error>> {
    use diesel::{
        Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query, sql_types::BigInt,
    };
    use mame_coalesce::{
        app::{self, CatalogDocumentFormat, CatalogImportRequest},
        database::Database,
        domain::{CatalogKey, CatalogScope, PublishingSourceKey},
    };
    #[derive(QueryableByName)]
    struct Count {
        #[diesel(sql_type = BigInt)]
        value: i64,
    }
    let directory = tempfile::tempdir()?;
    let database_path = camino::Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let database = Database::open(&database_path)?;
    let document_path = camino::Utf8PathBuf::from_path_buf(directory.path().join("catalog.dat"))
        .map_err(|_| "non-UTF-8 document path")?;
    std::fs::write(
        &document_path,
        "<datafile><game name='headerless'><rom name='file.bin' size='4'/></game></datafile>",
    )?;
    let request = CatalogImportRequest {
        document_path: document_path.clone(),
        format: CatalogDocumentFormat::Logiqx(
            mame_coalesce::logiqx::LogiqxMode::ObservedCompatible,
        ),
        source_key: PublishingSourceKey::new("headerless"),
        source_display_name: "Headerless source".to_owned(),
        catalog_key: CatalogKey::new("headerless"),
        catalog_display_name: "User supplied catalog name".to_owned(),
        scope: CatalogScope::Complete,
    };
    let report = app::import_catalog(&database, &request)?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    let mut conn = SqliteConnection::establish(database_path.as_str())?;
    let count =
        sql_query("SELECT COUNT(*) AS value FROM logiqx_document_facts WHERE header_name IS NULL")
            .get_result::<Count>(&mut conn)?;
    assert_eq!(count.value, 1);
    std::fs::write(
        &document_path,
        "<datafile><game name='headerless'><rom name='file.bin' size='8'/></game></datafile>",
    )?;
    let second = app::import_catalog(&database, &request)?;
    let diff = app::diff_catalog_snapshots(
        &database,
        &report.snapshot_key.ok_or("first snapshot missing")?,
        &second.snapshot_key.ok_or("second snapshot missing")?,
    )?;
    assert_eq!(diff.records.len(), 1);
    assert!(
        diff.records[0]
            .requirement_changes
            .iter()
            .any(|change| change.size_changed)
    );
    Ok(())
}

#[test]
fn parses_optional_header_options_and_repeated_dtd_native_game_families() {
    let xml = br#"<?xml version="1.0"?>
<!DOCTYPE datafile PUBLIC "-//Logiqx//DTD ROM Management Datafile//EN" "http://www.logiqx.com/Dats/datafile.dtd">
<datafile build="test-build">
  <header>
    <name>Native</name><description>Native model</description><version>1</version><author>Test</author>
    <clrmamepro forcemerging="full"/>
    <romcenter rommode="unmerged" lockrommode="yes"/>
  </header>
  <game name="native">
    <comment>first</comment><comment>second</comment>
    <description>Native game</description><year>1999</year><manufacturer>Example</manufacturer>
    <release name="US" region="USA"/>
    <release name="EU" region="Europe" language="English" date="1999-01-02" default="yes"/>
    <biosset name="base" description="Base BIOS"/>
    <biosset name="alt" description="Alternate BIOS" default="yes"/>
    <rom name="native.bin" size="4" status="verified" serial="A-123" date="1999-01-02"/>
    <disk name="native.chd"/>
    <sample name="native-a"/><sample name="native-b"/>
    <archive name="native.zip"/>
  </game>
</datafile>"#;
    let data = DataFile::from_reader(xml.as_slice()).expect("valid DTD 1.5 datafile");

    assert_eq!(
        data.header_opt().map(mame_coalesce::logiqx::Header::name),
        Some("Native")
    );
    assert_eq!(data.debug_effective(), "no");
    let clrmamepro = data.clrmamepro_options_opt().expect("clrmamepro options");
    assert_eq!(clrmamepro.forcemerging(), "full");
    assert_eq!(clrmamepro.forcenodump(), "obsolete");
    assert!(!clrmamepro.forcenodump_was_explicit());
    let romcenter = data.romcenter_options_opt().expect("romcenter options");
    assert_eq!(romcenter.rommode(), "unmerged");
    assert_eq!(romcenter.samplemode(), "merged");

    let game = &data.games()[0];
    assert_eq!(
        game.comments()
            .iter()
            .map(mame_coalesce::logiqx::NativeComment::text)
            .collect::<Vec<_>>(),
        ["first", "second"]
    );
    assert_eq!(game.releases().len(), 2);
    assert_eq!(game.releases()[0].default(), "no");
    assert!(!game.releases()[0].default_was_explicit());
    assert_eq!(game.releases()[1].language(), Some("English"));
    assert_eq!(game.bios_sets().len(), 2);
    assert_eq!(game.bios_sets()[0].default(), "no");
    assert_eq!(game.roms()[0].effective_status(), "verified");
    assert!(game.roms()[0].status_was_explicit());
    assert_eq!(game.roms()[0].serial(), Some("A-123"));
    assert_eq!(game.roms()[0].date(), Some("1999-01-02"));
    assert_eq!(game.disks().len(), 1);
    assert_eq!(game.disks()[0].effective_status(), "good");
    assert!(!game.disks()[0].status_was_explicit());
    assert_eq!(game.samples().len(), 2);
    assert_eq!(game.archives()[0].name(), "native.zip");
}

#[test]
fn parses_datafile_without_optional_header() {
    let xml =
        br#"<datafile><game name="minimal"><description>Minimal</description></game></datafile>"#;
    let data = DataFile::from_reader(xml.as_slice()).expect("header is optional in DTD 1.5");

    assert!(data.header_opt().is_none());
    assert_eq!(data.games()[0].name(), "minimal");
}

#[test]
fn rejects_values_outside_the_pinned_dtd_enums() {
    let xml = br#"<datafile><game name="bad"><description>Bad</description><rom name="bad.bin" size="1" status="unknown"/></game></datafile>"#;
    assert!(DataFile::from_reader(xml.as_slice()).is_err());
}
