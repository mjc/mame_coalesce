use camino::Utf8PathBuf;
use diesel::{
    QueryableByName, SqliteConnection,
    prelude::*,
    sql_query,
    sql_types::{BigInt, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};

#[derive(QueryableByName)]
struct IdentityCounts {
    #[diesel(sql_type = BigInt)]
    occurrences: i64,
    #[diesel(sql_type = BigInt)]
    linked: i64,
    #[diesel(sql_type = BigInt)]
    identities: i64,
}

#[derive(QueryableByName)]
struct LoadSteps {
    #[diesel(sql_type = Text)]
    operations: String,
    #[diesel(sql_type = Text)]
    instructions: String,
    #[diesel(sql_type = Text)]
    segment_sizes: String,
}

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

fn import_xml(
    xml: &str,
) -> Result<(tempfile::TempDir, SqliteConnection), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let database = Database::open(&path)?;
    let document_path = Utf8PathBuf::from_path_buf(directory.path().join("software.xml"))
        .map_err(|_| "non-UTF-8 document path")?;
    std::fs::write(&document_path, xml)?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::MameSoftwareListXml,
            source_key: PublishingSourceKey::new("native-software"),
            source_display_name: "Native software".to_owned(),
            catalog_key: CatalogKey::new("native-software"),
            catalog_display_name: "Native software".to_owned(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
    Ok((directory, SqliteConnection::establish(path.as_str())?))
}

#[test]
fn software_load_file_owners_do_not_cross_area_boundaries() -> Result<(), Box<dyn std::error::Error>>
{
    let (_directory, mut conn) = import_xml(
        r#"<softwarelist name="list"><software name="game">
      <description>Game</description><year>2000</year><publisher>Example</publisher>
      <part name="cart" interface="cart">
        <dataarea name="first" size="8"><rom name="file.bin" size="4"/><rom size="4" loadflag="continue"/>
          <rom size="1" loadflag="fill" value="ff"/></dataarea>
        <dataarea name="second" size="4"><rom size="4" loadflag="continue"/></dataarea>
      </part></software></softwarelist>"#,
    )?;
    let foreign_owners = sql_query(
        "SELECT COUNT(*) AS value FROM software_file_uses AS use_row \
         JOIN software_rom_entries AS entry ON entry.occurrence_id = use_row.occurrence_id \
         JOIN software_rom_entries AS owner ON owner.occurrence_id = use_row.declaration_occurrence_id \
         WHERE entry.area_id <> owner.area_id OR use_row.operation = 'fill'",
    ).get_result::<CountRow>(&mut conn)?;
    assert_eq!(foreign_owners.value, 0);
    let same_area = sql_query("SELECT COUNT(*) AS value FROM software_file_uses WHERE operation = 'continue' AND declaration_occurrence_id IS NOT NULL")
        .get_result::<CountRow>(&mut conn)?;
    assert_eq!(same_area.value, 1);
    Ok(())
}

#[test]
fn software_report_digests_follow_the_native_declared_scope()
-> Result<(), Box<dyn std::error::Error>> {
    let (_directory, mut conn) = import_xml(
        r#"<softwarelist name="list"><software name="game">
      <description>Game</description><year>2000</year><publisher>Example</publisher>
      <part name="cart" interface="cart"><dataarea name="rom" size="4">
        <rom name="file.bin" size="4" sha1="0123456789abcdef0123456789abcdef01234567"/>
        <rom size="1" loadflag="fill" value="ff"/>
      </dataarea></part></software></softwarelist>"#,
    )?;
    diesel::connection::SimpleConnection::batch_execute(
        &mut conn,
        "INSERT INTO digest_values(algorithm,digest) VALUES ('sha1',zeroblob(20)); \
         INSERT INTO occurrence_digest_assertions(occurrence_id,digest_id,scope,provenance) \
         SELECT occurrence_id,(SELECT digest_id FROM digest_values WHERE digest = zeroblob(20)), 'track','computed' \
         FROM software_rom_entries WHERE name = 'file.bin'",
    )?;
    let digest = sql_query("SELECT lower(hex(sha1)) AS value FROM software_components WHERE component_name = 'file.bin'")
        .get_result::<TextValue>(&mut conn)?;
    assert_eq!(digest.value, "0123456789abcdef0123456789abcdef01234567");
    let scope = sql_query(
        "SELECT evidence_scope AS value FROM software_components WHERE component_name IS NULL",
    )
    .get_result::<TextValue>(&mut conn)?;
    assert_eq!(scope.value, "unknown");
    let file_scope = sql_query(
        "SELECT evidence_scope AS value FROM software_components WHERE component_name = 'file.bin'",
    )
    .get_result::<TextValue>(&mut conn)?;
    assert_eq!(file_scope.value, "whole_asset");
    Ok(())
}

#[derive(QueryableByName)]
struct TextValue {
    #[diesel(sql_type = Text)]
    value: String,
}

#[test]
fn software_segment_sizes_do_not_split_whole_file_identity_across_lists()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "non-UTF-8 database path")?;
    let database = Database::open(&database_path)?;
    let document_path = Utf8PathBuf::from_path_buf(directory.path().join("software.xml"))
        .map_err(|_| "non-UTF-8 document path")?;
    let sha1 = "0123456789abcdef0123456789abcdef01234567";
    std::fs::write(
        &document_path,
        format!(
            r#"<softwarelists>
              <softwarelist name="segmented"><software name="game">
                <description>Segmented</description><year>2000</year><publisher>Example</publisher>
                <part name="cart" interface="cart"><dataarea name="rom" size="8">
                  <rom name="shared.bin" size="4" sha1="{sha1}" loadflag="load16_word"/>
                  <rom name="shared.bin" size="4" sha1="{sha1}" loadflag="continue"/>
                </dataarea></part>
              </software></softwarelist>
              <softwarelist name="standalone"><software name="game">
                <description>Standalone</description><year>2000</year><publisher>Example</publisher>
                <part name="cart" interface="cart"><dataarea name="rom" size="8">
                  <rom name="shared.bin" size="8" sha1="{sha1}"/>
                </dataarea></part>
              </software></softwarelist>
            </softwarelists>"#
        ),
    )?;
    let request = CatalogImportRequest {
        document_path,
        format: CatalogDocumentFormat::MameSoftwareListXml,
        source_key: PublishingSourceKey::new("software-native-test-source"),
        source_display_name: "Software native model test".to_owned(),
        catalog_key: CatalogKey::new("software-native-test-catalog"),
        catalog_display_name: "Software native model test".to_owned(),
        scope: CatalogScope::Unknown,
    };
    let imported = app::import_catalog(&database, &request)?;
    assert_eq!(imported.status, app::CatalogImportStatus::Succeeded);

    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    let identities = sql_query(
        "SELECT COUNT(*) AS occurrences, COUNT(content_uuid) AS linked, \
         COUNT(DISTINCT content_uuid) AS identities \
         FROM asset_occurrences WHERE claim_kind = 'software_rom_entry'",
    )
    .get_result::<IdentityCounts>(&mut connection)?;
    assert_eq!(identities.occurrences, 2);
    assert_eq!(identities.linked, 2);
    assert_eq!(identities.identities, 1);

    let steps = sql_query(
        "SELECT GROUP_CONCAT(operation, ',') AS operations, \
         GROUP_CONCAT(instruction, ',') AS instructions, \
         GROUP_CONCAT(segment_size, ',') AS segment_sizes FROM ( \
           SELECT file_use.operation, COALESCE(rom.load_instruction, 'default') AS instruction, \
             rom.size AS segment_size \
           FROM asset_occurrences AS occurrence \
           JOIN software_rom_entries AS rom USING (occurrence_id, record_id) \
           JOIN software_file_uses AS file_use USING (occurrence_id, record_id) \
           JOIN records USING (record_id) \
           JOIN record_namespaces USING (namespace_id) \
           ORDER BY record_namespaces.source_order, occurrence.occurrence_order \
         )",
    )
    .get_result::<LoadSteps>(&mut connection)?;
    assert_eq!(steps.operations, "load,continue,load");
    assert_eq!(steps.instructions, "load16_word,continue,default");
    assert_eq!(steps.segment_sizes, "4,4,8");

    let sized_declarations = sql_query(
        "SELECT COUNT(*) AS value FROM software_file_declarations WHERE declared_size IS NOT NULL",
    )
    .get_result::<CountRow>(&mut connection)?;
    assert_eq!(sized_declarations.value, 0);
    Ok(())
}
