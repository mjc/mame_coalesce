use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Nullable, Text},
};
use mame_coalesce::{
    NoIntroDatMode,
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    catalog_files::{self, OccurrenceId},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey, SnapshotKey},
    file_match_reviews::{
        self, ConflictRef, Disposition, EvidenceDecision, EvidenceRole, FileMatchReview,
        ReviewAction, ReviewedEvidence, SizeField,
    },
};
use std::error::Error;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const SHA1: &str = "0123456789abcdef0123456789abcdef01234567";

#[derive(QueryableByName)]
struct IdRow {
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
}

#[derive(QueryableByName)]
struct SizeRow {
    #[diesel(sql_type = BigInt)]
    occurrence_id: i64,
    #[diesel(sql_type = Text)]
    size_field: String,
    #[diesel(sql_type = BigInt)]
    size: i64,
}

#[derive(QueryableByName)]
struct CountRow {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

struct Fixture {
    directory: tempfile::TempDir,
    database: Database,
    connection: SqliteConnection,
}

impl Fixture {
    fn new() -> TestResult<Self> {
        let directory = tempfile::tempdir()?;
        let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
            .map_err(|_| "non-UTF-8 database path")?;
        let database = Database::open(&database_path)?;
        let connection = SqliteConnection::establish(database_path.as_str())?;
        Ok(Self {
            directory,
            database,
            connection,
        })
    }

    fn import(
        &self,
        key: &str,
        format: CatalogDocumentFormat,
        contents: &str,
    ) -> TestResult<SnapshotKey> {
        let document_path =
            Utf8PathBuf::from_path_buf(self.directory.path().join(format!("{key}.xml")))
                .map_err(|_| "non-UTF-8 input path")?;
        std::fs::write(&document_path, contents)?;
        let report = app::import_catalog(
            &self.database,
            &CatalogImportRequest {
                document_path,
                format,
                source_key: PublishingSourceKey::new(key),
                source_display_name: key.to_owned(),
                catalog_key: CatalogKey::new(key),
                catalog_display_name: key.to_owned(),
                scope: CatalogScope::Complete,
            },
        )?;
        assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
        report.snapshot_key.ok_or_else(|| "snapshot missing".into())
    }

    fn occurrences(
        &mut self,
        snapshot: &SnapshotKey,
    ) -> TestResult<Vec<catalog_files::CatalogFileOccurrence>> {
        let ids = sql_query(
            "SELECT occurrence.occurrence_id FROM asset_occurrences AS occurrence \
             JOIN catalog_sets AS sets ON sets.set_id = occurrence.record_id \
             JOIN catalog_set_groups AS groups USING (set_group_id) \
             WHERE groups.snapshot_key = ? ORDER BY occurrence.occurrence_order",
        )
        .bind::<Text, _>(snapshot.as_str())
        .load::<IdRow>(&mut self.connection)?
        .into_iter()
        .map(|row| OccurrenceId::from_database(row.occurrence_id))
        .collect::<Vec<_>>();
        Ok(catalog_files::occurrences_for_ids(&self.database, &ids)?)
    }
}

const fn software_format() -> CatalogDocumentFormat {
    CatalogDocumentFormat::MameSoftwareListXml
}

const fn dat_format() -> CatalogDocumentFormat {
    CatalogDocumentFormat::NoIntroDat(NoIntroDatMode::V4Compatible)
}

fn software_xml(entries: &str) -> String {
    format!(
        "<softwarelists><softwarelist name=\"size-tests\"><software name=\"sample\">\
         <description>Sample</description><year>2000</year><publisher>Example</publisher>\
         <part name=\"cart\" interface=\"cart\"><dataarea name=\"rom\" size=\"64\">\
         {entries}</dataarea></part></software></softwarelist></softwarelists>"
    )
}

fn software_load_chain() -> String {
    format!(
        "<rom name=\"program.bin\" size=\"4\" sha1=\"{SHA1}\" loadflag=\"load16_word\"/>\
         <rom name=\"program.bin\" size=\"2\" loadflag=\"continue\"/>\
         <rom name=\"program.bin\" size=\"2\" loadflag=\"ignore\"/>\
         <rom name=\"program.bin\" size=\"10\" loadflag=\"reload\"/>"
    )
}

fn dat_xml(size: u64) -> String {
    format!(
        "<datafile><header><id>1</id><name>Size test</name><description>Size test</description>\
         <version>1</version><author>Tests</author></header><game name=\"sample\">\
         <description>Sample</description><rom name=\"program.bin\" size=\"{size}\" \
         sha1=\"{SHA1}\"/></game></datafile>"
    )
}

fn size_assertions(connection: &mut SqliteConnection) -> TestResult<Vec<(String, i64)>> {
    Ok(sql_query(
        "SELECT occurrence_id, size_field, size FROM catalog_file_size_assertions \
         ORDER BY size_field, size",
    )
    .load::<SizeRow>(connection)?
    .into_iter()
    .map(|row| {
        let _ = row.occurrence_id;
        (row.size_field, row.size)
    })
    .collect())
}

fn software_rom_named<'a>(
    files: &'a [catalog_files::CatalogFileOccurrence],
    name: &str,
) -> Option<&'a catalog_files::CatalogFileOccurrence> {
    files.iter().find(|file| {
        matches!(
            file.software_file.as_ref(),
            Some(catalog_files::SoftwareFilePayload::Rom(rom)) if rom.name.as_deref() == Some(name)
        )
    })
}

fn dat_rom_named<'a>(
    files: &'a [catalog_files::CatalogFileOccurrence],
    name: &str,
) -> Option<&'a catalog_files::CatalogFileOccurrence> {
    files.iter().find(|file| {
        file.no_intro_dat_rom
            .as_ref()
            .is_some_and(|rom| rom.name == name)
    })
}

fn stage_forged_dat_header(fixture: &mut Fixture, source: &SnapshotKey) -> TestResult {
    sql_query(
        "INSERT INTO catalogs(catalog_key,source_key,display_name) \
         SELECT 'pending-forged-size',catalogs.source_key,'Pending forged size' \
         FROM catalog_snapshots JOIN catalogs USING(catalog_key) WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(source.as_str())
    .execute(&mut fixture.connection)?;
    sql_query(
        "INSERT INTO catalog_snapshots(snapshot_key,catalog_key,document_key,interpretation_key,coverage_id) \
         SELECT 'pending-forged-size','pending-forged-size',document_key,interpretation_key,coverage_id \
         FROM catalog_snapshots WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(source.as_str())
    .execute(&mut fixture.connection)?;
    sql_query(
        "INSERT INTO catalog_set_groups(snapshot_key,kind,list_order) \
         VALUES('pending-forged-size','root',0)",
    )
    .execute(&mut fixture.connection)?;
    sql_query(
        "INSERT INTO no_intro_dat_documents(snapshot_key,schema_location,source_line,source_column) \
         SELECT 'pending-forged-size',schema_location,source_line,source_column \
         FROM no_intro_dat_documents WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(source.as_str())
    .execute(&mut fixture.connection)?;
    copy_forged_dat_header_rows(fixture, source)?;
    Ok(())
}

fn copy_forged_dat_header_rows(fixture: &mut Fixture, source: &SnapshotKey) -> TestResult {
    sql_query(
        "INSERT INTO no_intro_dat_headers(snapshot_key,source_order,source_line,source_column, \
         id_text,name,description,version_text,date,author,homepage,url,trademarks,piracy,subset,comment) \
         SELECT 'pending-forged-size',source_order,source_line,source_column,id_text,name,description, \
         version_text,date,author,homepage,url,trademarks,piracy,subset,comment \
         FROM no_intro_dat_headers WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(source.as_str())
    .execute(&mut fixture.connection)?;
    sql_query(
        "INSERT INTO no_intro_dat_header_field_positions(snapshot_key,field_kind,source_order,source_line,source_column) \
         SELECT 'pending-forged-size',field_kind,source_order,source_line,source_column \
         FROM no_intro_dat_header_field_positions WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(source.as_str())
    .execute(&mut fixture.connection)?;
    sql_query(
        "INSERT INTO no_intro_dat_parse_counts(snapshot_key,game_count,rom_count,category_count,identifier_count,release_count, \
         clrmamepro_option_count,romcenter_option_count,header_field_count,clrmamepro_field_count,romcenter_field_count,game_field_count,rom_field_count) \
         SELECT 'pending-forged-size',game_count,rom_count,category_count,identifier_count,release_count, \
         clrmamepro_option_count,romcenter_option_count,header_field_count,clrmamepro_field_count, \
         romcenter_field_count,game_field_count,rom_field_count FROM no_intro_dat_parse_counts \
         WHERE snapshot_key = ?",
    )
    .bind::<Text, _>(source.as_str())
    .execute(&mut fixture.connection)?;
    Ok(())
}

fn stage_forged_dat_game(fixture: &mut Fixture, source: &SnapshotKey) -> TestResult<i64> {
    let set_id = sql_query(
        "INSERT INTO catalog_sets(set_group_id,source_element_kind,list_order,set_name,source_line,source_column) \
         SELECT (SELECT set_group_id FROM catalog_set_groups WHERE snapshot_key='pending-forged-size'), \
         sets.source_element_kind,sets.list_order,sets.set_name,sets.source_line,sets.source_column \
         FROM catalog_sets AS sets JOIN catalog_set_groups AS groups USING(set_group_id) \
         WHERE groups.snapshot_key = ? LIMIT 1 RETURNING set_id AS count",
    )
    .bind::<Text, _>(source.as_str())
    .get_result::<CountRow>(&mut fixture.connection)?
    .count;
    sql_query(
        "INSERT INTO no_intro_dat_games(set_id,source_order,id_text,description_text) \
         SELECT ?,source_order,id_text,description_text FROM no_intro_dat_games WHERE set_id = \
         (SELECT sets.set_id FROM catalog_sets AS sets JOIN catalog_set_groups AS groups USING(set_group_id) \
          WHERE groups.snapshot_key = ? LIMIT 1)",
    )
    .bind::<BigInt, _>(set_id)
    .bind::<Text, _>(source.as_str())
    .execute(&mut fixture.connection)?;
    sql_query(
        "INSERT INTO no_intro_dat_game_field_positions(set_id,field_kind,source_order,source_line,source_column) \
         SELECT ?,field_kind,source_order,source_line,source_column FROM no_intro_dat_game_field_positions \
         WHERE set_id = (SELECT sets.set_id FROM catalog_sets AS sets \
           JOIN catalog_set_groups AS groups USING(set_group_id) WHERE groups.snapshot_key = ? LIMIT 1)",
    )
    .bind::<BigInt, _>(set_id)
    .bind::<Text, _>(source.as_str())
    .execute(&mut fixture.connection)?;
    Ok(set_id)
}

fn stage_forged_dat_rom(
    fixture: &mut Fixture,
    set_id: i64,
    source_occurrence: OccurrenceId,
    content_id: mame_coalesce::domain::CatalogContentId,
    size: &str,
) -> TestResult {
    let occurrence_id = sql_query(
        "INSERT INTO asset_occurrences(record_id,occurrence_order,claim_kind,content_uuid) \
         VALUES(?,0,'no_intro_dat_rom',?) RETURNING occurrence_id AS count",
    )
    .bind::<BigInt, _>(set_id)
    .bind::<diesel::sql_types::Binary, _>(content_id.as_bytes().as_slice())
    .get_result::<CountRow>(&mut fixture.connection)?
    .count;
    sql_query(
        "INSERT INTO no_intro_dat_rom_claims(occurrence_id,name,size_text,status_text,serial_text,header_text, \
         date_text,mia_text,evidence_scope,evidence_provenance,source_order,source_line,source_column) \
         SELECT ?,name,?,status_text,serial_text,header_text,date_text,mia_text,evidence_scope, \
         evidence_provenance,source_order,source_line,source_column FROM no_intro_dat_rom_claims \
         WHERE occurrence_id = ?",
    )
    .bind::<BigInt, _>(occurrence_id)
    .bind::<Text, _>(size)
    .bind::<BigInt, _>(source_occurrence.database_value())
    .execute(&mut fixture.connection)?;
    let digest_id = sql_query(
        "SELECT digest_id AS count FROM digest_values WHERE algorithm = 'sha1' AND digest = ?",
    )
    .bind::<diesel::sql_types::Binary, _>(hex::decode(SHA1)?)
    .get_result::<CountRow>(&mut fixture.connection)?
    .count;
    copy_forged_dat_rom_evidence(fixture, occurrence_id, source_occurrence, digest_id)?;
    Ok(())
}

fn copy_forged_dat_rom_evidence(
    fixture: &mut Fixture,
    occurrence_id: i64,
    source_occurrence: OccurrenceId,
    digest_id: i64,
) -> TestResult {
    sql_query(
        "INSERT INTO occurrence_digest_assertions(occurrence_id,digest_id,scope,provenance) \
         SELECT ?,?,scope,provenance FROM occurrence_digest_assertions WHERE occurrence_id = ?",
    )
    .bind::<BigInt, _>(occurrence_id)
    .bind::<BigInt, _>(digest_id)
    .bind::<BigInt, _>(source_occurrence.database_value())
    .execute(&mut fixture.connection)?;
    sql_query(
        "INSERT INTO no_intro_dat_rom_digest_fields(occurrence_id,field_kind,digest_id,invalid_text) \
         SELECT ?,field_kind,?,invalid_text FROM no_intro_dat_rom_digest_fields \
         WHERE occurrence_id = ?",
    )
    .bind::<BigInt, _>(occurrence_id)
    .bind::<BigInt, _>(digest_id)
    .bind::<BigInt, _>(source_occurrence.database_value())
    .execute(&mut fixture.connection)?;
    sql_query(
        "INSERT INTO no_intro_dat_rom_field_positions(occurrence_id,field_kind,source_order,source_line,source_column) \
         SELECT ?,field_kind,source_order,source_line,source_column FROM no_intro_dat_rom_field_positions \
         WHERE occurrence_id = ?",
    )
    .bind::<BigInt, _>(occurrence_id)
    .bind::<BigInt, _>(source_occurrence.database_value())
    .execute(&mut fixture.connection)?;
    Ok(())
}

fn try_publish_forged_dat(fixture: &mut Fixture) -> diesel::QueryResult<usize> {
    sql_query(
        "INSERT INTO snapshot_publications(catalog_key,document_key,interpretation_key,snapshot_key) \
         SELECT catalog_key,document_key,interpretation_key,snapshot_key \
         FROM catalog_snapshots WHERE snapshot_key='pending-forged-size'",
    )
    .execute(&mut fixture.connection)
}

fn assert_load_operations_share_declaration(
    files: &[catalog_files::CatalogFileOccurrence],
    declaration_id: OccurrenceId,
) {
    let actual = files
        .iter()
        .filter_map(|file| match file.software_file.as_ref()? {
            catalog_files::SoftwareFilePayload::Rom(rom)
                if rom.name.as_deref() == Some("program.bin") =>
            {
                Some((rom.operation, rom.declaration_occurrence_id))
            }
            catalog_files::SoftwareFilePayload::Rom(_)
            | catalog_files::SoftwareFilePayload::Disk(_) => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        [
            (
                catalog_files::SoftwareFileOperation::Load,
                Some(declaration_id)
            ),
            (
                catalog_files::SoftwareFileOperation::Continue,
                Some(declaration_id)
            ),
            (
                catalog_files::SoftwareFileOperation::Ignore,
                Some(declaration_id)
            ),
            (
                catalog_files::SoftwareFileOperation::Reload,
                Some(declaration_id)
            ),
        ],
        "every loading operation must retain its exact declaration edge"
    );
}

#[test]
fn software_load_segments_derive_whole_file_size_and_share_identity_with_matching_dat() -> TestResult
{
    let mut fixture = Fixture::new()?;
    let software_snapshot = fixture.import(
        "software-size-eight",
        software_format(),
        &software_xml(&software_load_chain()),
    )?;
    let dat_snapshot = fixture.import("dat-size-eight", dat_format(), &dat_xml(8))?;

    let software = fixture.occurrences(&software_snapshot)?;
    let dat = fixture.occurrences(&dat_snapshot)?;
    let software_rom =
        software_rom_named(&software, "program.bin").ok_or("software ROM occurrence missing")?;
    let dat_rom = dat_rom_named(&dat, "program.bin").ok_or("DAT ROM occurrence missing")?;

    assert_eq!(software_rom.content_id, dat_rom.content_id);
    assert_load_operations_share_declaration(&software, software_rom.occurrence_id);
    let content_id = software_rom.content_id.ok_or("shared UUID missing")?;
    let shared_files = catalog_files::occurrences_for_content(
        &fixture.database,
        content_id,
        catalog_files::ContentOccurrenceLimit::new(10)?,
        None,
    )?;
    assert_eq!(shared_files.occurrences.len(), 2);
    assert_eq!(
        size_assertions(&mut fixture.connection)?,
        vec![
            ("no_intro_dat_rom_size".to_owned(), 8),
            ("software_rom_file_size".to_owned(), 8),
        ]
    );

    let rom_entries = sql_query(
        "SELECT size_text, load_instruction FROM software_rom_entries \
         JOIN asset_occurrences USING (occurrence_id) \
         JOIN catalog_sets ON catalog_sets.set_id = asset_occurrences.record_id \
         JOIN catalog_set_groups USING (set_group_id) WHERE snapshot_key = ? \
         ORDER BY occurrence_order",
    )
    .bind::<Text, _>(software_snapshot.as_str())
    .load::<RawSoftwareRom>(&mut fixture.connection)?;
    assert_eq!(
        rom_entries
            .iter()
            .map(|entry| (entry.size_text.as_str(), entry.load_instruction.as_deref()))
            .collect::<Vec<_>>(),
        vec![
            ("4", Some("load16_word")),
            ("2", Some("continue")),
            ("2", Some("ignore")),
            ("10", Some("reload")),
        ],
        "derived whole-file size must not replace source segment literals"
    );
    Ok(())
}

#[derive(QueryableByName)]
struct RawSoftwareRom {
    #[diesel(sql_type = Text)]
    size_text: String,
    #[diesel(sql_type = Nullable<Text>)]
    load_instruction: Option<String>,
}

#[test]
fn differing_cross_format_sizes_conflict_in_either_import_order() -> TestResult {
    for software_first in [true, false] {
        let mut fixture = Fixture::new()?;
        let software_snapshot;
        let dat_snapshot;
        if software_first {
            software_snapshot = fixture.import(
                "software-first-size-eight",
                software_format(),
                &software_xml(&software_load_chain()),
            )?;
            dat_snapshot = fixture.import("dat-later-size-four", dat_format(), &dat_xml(4))?;
        } else {
            dat_snapshot = fixture.import("dat-first-size-four", dat_format(), &dat_xml(4))?;
            software_snapshot = fixture.import(
                "software-later-size-eight",
                software_format(),
                &software_xml(&software_load_chain()),
            )?;
        }

        let software = fixture.occurrences(&software_snapshot)?;
        let dat = fixture.occurrences(&dat_snapshot)?;
        let software_rom = software_rom_named(&software, "program.bin")
            .ok_or("software ROM occurrence missing")?;
        let dat_rom = dat_rom_named(&dat, "program.bin").ok_or("DAT ROM occurrence missing")?;
        assert_ne!(software_rom.content_id, dat_rom.content_id);
        let incoming = if software_first {
            dat_rom
        } else {
            software_rom
        };
        let conflicts = sql_query(
            "SELECT COUNT(*) AS count FROM occurrence_content_conflicts \
             WHERE occurrence_id = ?",
        )
        .bind::<BigInt, _>(incoming.occurrence_id.database_value())
        .get_result::<CountRow>(&mut fixture.connection)?;
        assert_eq!(conflicts.count, 1);
        assert_eq!(
            sql_query(
                "SELECT COUNT(*) AS count FROM occurrence_content_conflict_sizes AS evidence \
                 JOIN software_file_declarations AS declaration \
                   ON declaration.occurrence_id = evidence.evidence_occurrence_id \
                 WHERE evidence.occurrence_id = ? AND evidence.size_field = 'software_rom_file_size' \
                   AND evidence.role = ?",
            )
            .bind::<BigInt, _>(incoming.occurrence_id.database_value())
            .bind::<Text, _>(if software_first { "candidate" } else { "incoming" })
            .get_result::<CountRow>(&mut fixture.connection)?
            .count,
            1,
            "software whole-file size remains source-attributed on the candidate side"
        );
    }
    Ok(())
}

#[test]
fn software_lists_with_same_first_segment_but_different_derived_sizes_conflict() -> TestResult {
    let mut fixture = Fixture::new()?;
    let first_snapshot = fixture.import(
        "software-size-seven-candidate",
        software_format(),
        &software_xml(&format!(
            "<rom name=\"program.bin\" size=\"4\" sha1=\"{SHA1}\" \
                 loadflag=\"load16_word\"/><rom name=\"program.bin\" size=\"3\" \
                 loadflag=\"continue\"/>"
        )),
    )?;
    let second_snapshot = fixture.import(
        "software-size-eight-incoming",
        software_format(),
        &software_xml(&software_load_chain()),
    )?;
    let first = fixture.occurrences(&first_snapshot)?;
    let second = fixture.occurrences(&second_snapshot)?;
    let first_rom =
        software_rom_named(&first, "program.bin").ok_or("first software ROM missing")?;
    let second_rom =
        software_rom_named(&second, "program.bin").ok_or("second software ROM missing")?;
    assert_ne!(first_rom.content_id, second_rom.content_id);
    assert_eq!(
        size_assertions(&mut fixture.connection)?,
        vec![
            ("software_rom_file_size".to_owned(), 7),
            ("software_rom_file_size".to_owned(), 8),
        ]
    );
    assert_eq!(
        sql_query(
            "SELECT size_text AS value FROM software_rom_entries \
             JOIN asset_occurrences USING (occurrence_id) \
             JOIN catalog_sets ON catalog_sets.set_id = asset_occurrences.record_id \
             JOIN catalog_set_groups USING (set_group_id) WHERE snapshot_key = ? \
             ORDER BY occurrence_order",
        )
        .bind::<Text, _>(first_snapshot.as_str())
        .load::<TextRow>(&mut fixture.connection)?
        .into_iter()
        .map(|row| row.value)
        .collect::<Vec<_>>(),
        vec!["4", "3"]
    );
    Ok(())
}

#[test]
fn same_area_declarations_with_same_digest_and_different_sizes_conflict_after_import() -> TestResult
{
    let mut fixture = Fixture::new()?;
    let snapshot = fixture.import(
        "software-same-area-size-conflict",
        software_format(),
        &software_xml(&format!(
            "<rom name=\"program.bin\" size=\"4\" sha1=\"{SHA1}\" \
             loadflag=\"load16_word\"/><rom name=\"program.bin\" size=\"5\" \
             sha1=\"{SHA1}\" loadflag=\"load16_word\"/>"
        )),
    )?;
    let files = fixture.occurrences(&snapshot)?;
    let mut declarations = files
        .iter()
        .filter_map(|file| match file.software_file.as_ref()? {
            catalog_files::SoftwareFilePayload::Rom(rom)
                if rom.name.as_deref() == Some("program.bin") =>
            {
                Some((rom.source_order, file))
            }
            catalog_files::SoftwareFilePayload::Rom(_)
            | catalog_files::SoftwareFilePayload::Disk(_) => None,
        })
        .collect::<Vec<_>>();
    declarations.sort_by_key(|(source_order, _)| *source_order);
    assert_eq!(declarations.len(), 2);
    assert!(declarations[0].1.content_id.is_some());
    assert!(declarations[1].1.content_id.is_none());
    assert_eq!(
        sql_query(
            "SELECT COUNT(*) AS count FROM occurrence_content_conflicts \
             WHERE occurrence_id = ?",
        )
        .bind::<BigInt, _>(declarations[1].1.occurrence_id.database_value())
        .get_result::<CountRow>(&mut fixture.connection)?
        .count,
        1,
        "the whole-area import records the conflict against the earlier UUID"
    );
    assert_eq!(
        size_assertions(&mut fixture.connection)?,
        vec![
            ("software_rom_file_size".to_owned(), 4),
            ("software_rom_file_size".to_owned(), 5),
        ]
    );
    Ok(())
}

#[test]
fn malformed_software_size_does_not_block_a_valid_hash_match() -> TestResult {
    let mut fixture = Fixture::new()?;
    let software_snapshot = fixture.import(
        "software-malformed-size",
        software_format(),
        &software_xml(&format!(
            "<rom name=\"program.bin\" size=\"invalid\" sha1=\"{SHA1}\" \
             loadflag=\"load16_word\"/>"
        )),
    )?;
    let dat_snapshot = fixture.import("dat-valid-size", dat_format(), &dat_xml(4))?;
    let software = fixture.occurrences(&software_snapshot)?;
    let dat = fixture.occurrences(&dat_snapshot)?;
    let software_rom =
        software_rom_named(&software, "program.bin").ok_or("software ROM occurrence missing")?;
    let dat_rom = dat_rom_named(&dat, "program.bin").ok_or("DAT ROM occurrence missing")?;
    assert_eq!(software_rom.content_id, dat_rom.content_id);
    assert!(software_rom.content_id.is_some());
    assert_eq!(
        catalog_files::occurrences_for_content(
            &fixture.database,
            software_rom.content_id.ok_or("software UUID missing")?,
            catalog_files::ContentOccurrenceLimit::new(10)?,
            None,
        )?
        .occurrences
        .len(),
        2
    );
    assert_eq!(
        sql_query(
            "SELECT COUNT(*) AS count FROM catalog_file_size_assertions \
             WHERE size_field = 'software_rom_file_size'",
        )
        .get_result::<CountRow>(&mut fixture.connection)?
        .count,
        0
    );
    Ok(())
}

#[test]
fn zero_size_rules_preserve_only_nonempty_base_and_zero_ignore() -> TestResult {
    let mut fixture = Fixture::new()?;
    let entries = concat!(
        "<rom name=\"zero-base.bin\" size=\"0\" loadflag=\"load16_word\"/>",
        "<rom name=\"zero-continue.bin\" size=\"4\" loadflag=\"load16_word\"/>",
        "<rom name=\"zero-continue.bin\" size=\"0\" loadflag=\"continue\"/>",
        "<rom name=\"zero-ignore.bin\" size=\"4\" loadflag=\"load16_word\"/>",
        "<rom name=\"zero-ignore.bin\" size=\"0\" loadflag=\"ignore\"/>",
        "<rom name=\"standalone.bin\" size=\"5\" loadflag=\"load16_word\"/>"
    );
    let snapshot = fixture.import(
        "software-zero-size-rules",
        software_format(),
        &software_xml(entries),
    )?;
    let files = fixture.occurrences(&snapshot)?;
    assert_eq!(
        size_assertions(&mut fixture.connection)?,
        vec![
            ("software_rom_file_size".to_owned(), 4),
            ("software_rom_file_size".to_owned(), 5),
        ]
    );
    for (name, expected) in [
        ("zero-base.bin", None),
        ("zero-continue.bin", None),
        ("zero-ignore.bin", Some(4)),
        ("standalone.bin", Some(5)),
    ] {
        let occurrence =
            software_rom_named(&files, name).ok_or("software ROM occurrence missing")?;
        let size = sql_query(
            "SELECT (SELECT size FROM catalog_file_size_assertions \
             WHERE occurrence_id = ? AND size_field = 'software_rom_file_size') AS size",
        )
        .bind::<BigInt, _>(occurrence.occurrence_id.database_value())
        .get_result::<OptionalSizeRow>(&mut fixture.connection)?
        .size;
        assert_eq!(size, expected, "size projection for {name}");
    }
    Ok(())
}

#[derive(QueryableByName)]
struct OptionalSizeRow {
    #[diesel(sql_type = Nullable<BigInt>)]
    size: Option<i64>,
}

#[derive(QueryableByName)]
struct TextRow {
    #[diesel(sql_type = Text)]
    value: String,
}

#[test]
fn size_conflict_review_keeps_software_declaration_and_load_chain_reachable() -> TestResult {
    let mut fixture = Fixture::new()?;
    let software_snapshot = fixture.import(
        "software-review-candidate",
        software_format(),
        &software_xml(&software_load_chain()),
    )?;
    let dat_snapshot = fixture.import("dat-review-incoming", dat_format(), &dat_xml(4))?;
    let software = fixture.occurrences(&software_snapshot)?;
    let dat = fixture.occurrences(&dat_snapshot)?;
    let declaration =
        software_rom_named(&software, "program.bin").ok_or("software load declaration missing")?;
    let incoming = dat_rom_named(&dat, "program.bin").ok_or("DAT incoming occurrence missing")?;
    assert!(matches!(
        declaration.software_file.as_ref(),
        Some(catalog_files::SoftwareFilePayload::Rom(rom))
            if rom.operation == catalog_files::SoftwareFileOperation::Load
    ));
    let candidate = declaration.content_id.ok_or("candidate UUID missing")?;
    let conflict = ConflictRef {
        incoming: incoming.occurrence_id,
        candidate,
    };
    let decision_id = file_match_reviews::record_review(
        &fixture.database,
        &FileMatchReview {
            rationale: "Retain the imported size disagreement for separate review".into(),
            conflicts: vec![conflict],
            evidence: vec![EvidenceDecision {
                conflict,
                evidence: ReviewedEvidence::Size {
                    occurrence: declaration.occurrence_id,
                    field: SizeField::SoftwareRomFile,
                    role: EvidenceRole::Candidate,
                },
                disposition: Disposition::Reject,
            }],
            action: ReviewAction::KeepSeparate,
        },
    )?;
    assert!(decision_id.as_i64() > 0);
    assert_eq!(
        file_match_reviews::resolve_file_id(&fixture.database, candidate)?,
        candidate
    );
    assert_eq!(
        sql_query(
            "SELECT COUNT(*) AS count FROM accepted_file_size_assertions \
             WHERE occurrence_id = ? AND size_field = 'software_rom_file_size'",
        )
        .bind::<BigInt, _>(declaration.occurrence_id.database_value())
        .get_result::<CountRow>(&mut fixture.connection)?
        .count,
        0,
        "rejecting the size review excludes only the accepted projection"
    );
    assert_eq!(
        sql_query(
            "SELECT COUNT(*) AS count FROM catalog_file_size_assertions \
             WHERE occurrence_id = ? AND size_field = 'software_rom_file_size' AND size = 8",
        )
        .bind::<BigInt, _>(declaration.occurrence_id.database_value())
        .get_result::<CountRow>(&mut fixture.connection)?
        .count,
        1,
        "review rejection preserves the raw source-derived assertion"
    );

    let reachable_chain = sql_query(
        "SELECT COUNT(*) AS count FROM file_match_size_decisions AS review \
         JOIN occurrence_content_conflict_sizes AS conflict_size \
           ON conflict_size.occurrence_id = review.occurrence_id \
          AND conflict_size.candidate_content_uuid = review.candidate_content_uuid \
          AND conflict_size.evidence_occurrence_id = review.evidence_occurrence_id \
          AND conflict_size.size_field = review.size_field AND conflict_size.role = review.role \
         JOIN software_file_declarations AS declaration \
           ON declaration.occurrence_id = review.evidence_occurrence_id \
         JOIN software_file_uses AS file_use \
           ON file_use.declaration_occurrence_id = declaration.occurrence_id \
         JOIN software_rom_entries AS rom ON rom.occurrence_id = file_use.occurrence_id \
         WHERE review.occurrence_id = ? AND review.size_field = 'software_rom_file_size' \
           AND review.evidence_occurrence_id = ? AND review.role = 'candidate'",
    )
    .bind::<BigInt, _>(incoming.occurrence_id.database_value())
    .bind::<BigInt, _>(declaration.occurrence_id.database_value())
    .get_result::<CountRow>(&mut fixture.connection)?;
    assert_eq!(reachable_chain.count, 4);
    Ok(())
}

fn assert_unprovable_raw_rom_facts(
    occurrences: &[catalog_files::CatalogFileOccurrence],
) -> TestResult {
    let roms = occurrences
        .iter()
        .filter_map(|file| match file.software_file.as_ref()? {
            catalog_files::SoftwareFilePayload::Rom(rom) => Some(rom),
            catalog_files::SoftwareFilePayload::Disk(_) => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(roms.len(), 13);
    assert_eq!(
        roms.iter()
            .map(|rom| (rom.name.as_deref(), rom.size_text.as_deref(), rom.size))
            .collect::<Vec<_>>(),
        vec![
            (Some("malformed.bin"), Some("not-a-number"), None),
            (
                Some("overflow.bin"),
                Some("9223372036854775807"),
                Some(i64::MAX)
            ),
            (Some("overflow.bin"), Some("1"), Some(1)),
            (None, Some("not-a-number"), None),
            (Some("after-fill.bin"), Some("2"), Some(2)),
            (Some("reload-bad.bin"), Some("4"), Some(4)),
            (Some("reload-bad.bin"), Some("invalid-reload"), None),
            (Some("invalid-successor.bin"), Some("4"), Some(4)),
            (
                Some("invalid-successor.bin"),
                Some("invalid-continue"),
                None
            ),
            (Some("mismatched-link.bin"), Some("4"), Some(4)),
            (Some("mismatched-link.bin"), Some("2"), Some(2)),
            (Some("dense-gap.bin"), Some("4"), Some(4)),
            (Some("dense-gap.bin"), Some("2"), Some(2)),
        ],
        "raw numeric facts remain queryable even when the complete size is unknown"
    );
    let after_fill = roms
        .iter()
        .find(|rom| rom.name.as_deref() == Some("after-fill.bin"))
        .ok_or("post-fill continuation missing")?;
    assert_eq!(
        after_fill.operation,
        catalog_files::SoftwareFileOperation::Continue
    );
    assert!(after_fill.declaration_occurrence_id.is_none());
    Ok(())
}

fn size_for_occurrence(fixture: &mut Fixture, occurrence: OccurrenceId) -> TestResult<Option<i64>> {
    Ok(sql_query(
        "SELECT (SELECT size FROM catalog_file_size_assertions \
         WHERE occurrence_id = ? AND size_field = 'software_rom_file_size') AS size",
    )
    .bind::<BigInt, _>(occurrence.database_value())
    .get_result::<OptionalSizeRow>(&mut fixture.connection)?
    .size)
}

fn clear_bad_successor_edges(
    fixture: &mut Fixture,
    occurrences: &[catalog_files::CatalogFileOccurrence],
) -> TestResult<()> {
    sql_query("DROP TRIGGER software_file_uses_immutable_update")
        .execute(&mut fixture.connection)?;
    sql_query("DROP TRIGGER software_rom_entries_immutable_update")
        .execute(&mut fixture.connection)?;
    let mismatch = software_rom_operation_named(
        occurrences,
        "mismatched-link.bin",
        catalog_files::SoftwareFileOperation::Continue,
    )?;
    let rows = sql_query(
        "UPDATE software_file_uses SET declaration_occurrence_id = NULL \
         WHERE occurrence_id = ? AND declaration_occurrence_id IS NOT NULL",
    )
    .bind::<BigInt, _>(mismatch.occurrence_id.database_value())
    .execute(&mut fixture.connection)?;
    assert_eq!(rows, 1, "mismatched ownership edge was changed");

    let dense_gap = software_rom_operation_named(
        occurrences,
        "dense-gap.bin",
        catalog_files::SoftwareFileOperation::Continue,
    )?;
    let rows = sql_query(
        "UPDATE software_rom_entries SET component_order = 13 \
         WHERE occurrence_id = ? AND component_order <> 13",
    )
    .bind::<BigInt, _>(dense_gap.occurrence_id.database_value())
    .execute(&mut fixture.connection)?;
    assert_eq!(rows, 1, "component order was changed to create a gap");
    Ok(())
}

fn software_rom_operation_named<'a>(
    occurrences: &'a [catalog_files::CatalogFileOccurrence],
    name: &str,
    operation: catalog_files::SoftwareFileOperation,
) -> TestResult<&'a catalog_files::CatalogFileOccurrence> {
    occurrences
        .iter()
        .find(|file| {
            matches!(
                file.software_file.as_ref(),
                Some(catalog_files::SoftwareFilePayload::Rom(rom))
                    if rom.name.as_deref() == Some(name) && rom.operation == operation
            )
        })
        .ok_or_else(|| format!("software ROM {name} with {operation:?} operation missing").into())
}

#[test]
fn malformed_overflow_and_continue_after_fill_keep_source_facts_without_known_size() -> TestResult {
    let mut fixture = Fixture::new()?;
    let entries = format!(
        "<rom name=\"malformed.bin\" size=\"not-a-number\" sha1=\"{SHA1}\" \
             loadflag=\"load16_word\"/>\
         <rom name=\"overflow.bin\" size=\"9223372036854775807\" \
             sha1=\"{SHA1}\" loadflag=\"load16_word\"/>\
         <rom name=\"overflow.bin\" size=\"1\" loadflag=\"continue\"/>\
         <rom size=\"not-a-number\" loadflag=\"fill\"/>\
         <rom name=\"after-fill.bin\" size=\"2\" sha1=\"{SHA1}\" \
             loadflag=\"continue\"/>\
         <rom name=\"reload-bad.bin\" size=\"4\" loadflag=\"load16_word\"/>\
         <rom name=\"reload-bad.bin\" size=\"invalid-reload\" loadflag=\"reload\"/>\
         <rom name=\"invalid-successor.bin\" size=\"4\" loadflag=\"load16_word\"/>\
         <rom name=\"invalid-successor.bin\" size=\"invalid-continue\" loadflag=\"continue\"/>\
         <rom name=\"mismatched-link.bin\" size=\"4\" loadflag=\"load16_word\"/>\
         <rom name=\"mismatched-link.bin\" size=\"2\" loadflag=\"continue\"/>\
         <rom name=\"dense-gap.bin\" size=\"4\" loadflag=\"load16_word\"/>\
         <future-component/>\
         <rom name=\"dense-gap.bin\" size=\"2\" loadflag=\"continue\"/>"
    );
    let snapshot = fixture.import(
        "software-unprovable-sizes",
        software_format(),
        &software_xml(&entries),
    )?;
    let occurrences = fixture.occurrences(&snapshot)?;
    assert_unprovable_raw_rom_facts(&occurrences)?;

    let reload =
        software_rom_named(&occurrences, "reload-bad.bin").ok_or("reload base ROM missing")?;
    let invalid = software_rom_named(&occurrences, "invalid-successor.bin")
        .ok_or("invalid successor base missing")?;
    let mismatch = software_rom_named(&occurrences, "mismatched-link.bin")
        .ok_or("mismatched-link base missing")?;
    let gap = software_rom_named(&occurrences, "dense-gap.bin").ok_or("dense-gap base missing")?;
    assert_eq!(
        sql_query(
            "SELECT COUNT(*) AS count FROM catalog_file_size_assertions \
                   WHERE size_field = 'software_rom_file_size'"
        )
        .get_result::<CountRow>(&mut fixture.connection)?
        .count,
        3,
        "the valid reload and intact load chains initially have sizes"
    );
    assert_eq!(
        size_for_occurrence(&mut fixture, reload.occurrence_id)?,
        Some(4)
    );
    clear_bad_successor_edges(&mut fixture, &occurrences)?;
    for base in [invalid, mismatch, gap] {
        assert_eq!(size_for_occurrence(&mut fixture, base.occurrence_id)?, None);
    }
    Ok(())
}

#[test]
fn declaration_table_has_no_null_only_size_placeholder() -> TestResult {
    let mut fixture = Fixture::new()?;
    let columns =
        sql_query("SELECT name AS value FROM pragma_table_info('software_file_declarations')")
            .load::<TextRow>(&mut fixture.connection)?
            .into_iter()
            .map(|row| row.value)
            .collect::<Vec<_>>();
    assert!(!columns.iter().any(|name| name == "declared_size"));
    Ok(())
}

#[test]
fn dat_publication_rejects_a_forged_link_to_software_with_a_different_size() -> TestResult {
    for size in ["4", "8"] {
        check_dat_publication_against_software_only_size(size)?;
    }
    Ok(())
}

fn check_dat_publication_against_software_only_size(size: &str) -> TestResult {
    let mut fixture = Fixture::new()?;
    let software_snapshot = fixture.import(
        "software-publication-size-eight",
        software_format(),
        &software_xml(&software_load_chain()),
    )?;
    let template = dat_xml(8).replace(SHA1, "fedcba9876543210fedcba9876543210fedcba98");
    let dat_snapshot = fixture.import("dat-publication-template", dat_format(), &template)?;
    let software = fixture.occurrences(&software_snapshot)?;
    let dat = fixture.occurrences(&dat_snapshot)?;
    let software_rom =
        software_rom_named(&software, "program.bin").ok_or("software candidate missing")?;
    let dat_rom = dat_rom_named(&dat, "program.bin").ok_or("DAT template missing")?;
    let content_id = software_rom.content_id.ok_or("software UUID missing")?;
    assert_ne!(dat_rom.content_id, Some(content_id));
    let candidate_sizes = sql_query(
        "SELECT sizes.occurrence_id, sizes.size_field, sizes.size \
         FROM source_file_size_assertions AS sizes \
         JOIN asset_occurrences AS occurrence USING (occurrence_id) \
         WHERE occurrence.content_uuid = ?",
    )
    .bind::<diesel::sql_types::Binary, _>(content_id.as_bytes().as_slice())
    .load::<SizeRow>(&mut fixture.connection)?;
    assert_eq!(candidate_sizes.len(), 1);
    assert_eq!(candidate_sizes[0].size_field, "software_rom_file_size");
    assert_eq!(candidate_sizes[0].size, 8);

    stage_forged_dat_header(&mut fixture, &dat_snapshot)?;
    let set_id = stage_forged_dat_game(&mut fixture, &dat_snapshot)?;
    stage_forged_dat_rom(
        &mut fixture,
        set_id,
        dat_rom.occurrence_id,
        content_id,
        size,
    )?;
    let publication = try_publish_forged_dat(&mut fixture);
    if size == "8" {
        assert_eq!(publication?, 1, "equal software length must publish");
        return Ok(());
    }
    let error = publication
        .err()
        .ok_or("No-Intro DAT publication must reject a contradictory software source length")?;
    assert!(
        error.to_string().contains("source evidence"),
        "unexpected publication error: {error}"
    );
    Ok(())
}
