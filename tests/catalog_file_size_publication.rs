use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection,
    connection::SimpleConnection,
    sql_query,
    sql_types::{BigInt, Text},
};
use mame_coalesce::{
    NoIntroDatMode,
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
    logiqx::LogiqxMode,
    no_intro_db_xml::NoIntroDatabaseMode,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const SHA1: &str = "0123456789abcdef0123456789abcdef01234567";
const SHA1_BYTES: [u8; 20] = [
    0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef,
    0x01, 0x23, 0x45, 0x67,
];
const PUBLICATION_DELETE_GUARD: &str = "snapshot_publications_are_immutable_delete";
const OCCURRENCE_UPDATE_GUARD: &str = "asset_occurrences_are_immutable_update";
const LINKED_SIZE_GUARD: &str = "catalog_linked_file_size_publication";

#[derive(Clone, Copy)]
enum Role {
    MameRom,
    LogiqxRom,
    CmpRom,
    NoIntroDatRom,
    NoIntroPcFile,
    NoIntroDatabaseSourceFile,
    NoIntroDatabaseReleaseFile,
    SoftwareRomEntry,
}

impl Role {
    const ALL: [Self; 8] = [
        Self::MameRom,
        Self::LogiqxRom,
        Self::CmpRom,
        Self::NoIntroDatRom,
        Self::NoIntroPcFile,
        Self::NoIntroDatabaseSourceFile,
        Self::NoIntroDatabaseReleaseFile,
        Self::SoftwareRomEntry,
    ];

    const PUBLICLY_QUALIFIED: [Self; 6] = [
        Self::MameRom,
        Self::LogiqxRom,
        Self::CmpRom,
        Self::NoIntroDatRom,
        Self::NoIntroPcFile,
        Self::SoftwareRomEntry,
    ];

    const fn database_file_owner(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::NoIntroDatabaseSourceFile => Some((
                "no_intro_dump_files",
                "no_intro_dump_files_immutable_update",
            )),
            Self::NoIntroDatabaseReleaseFile => Some((
                "no_intro_release_files",
                "no_intro_release_files_immutable_update",
            )),
            _ => None,
        }
    }

    const fn claim_kind(self) -> &'static str {
        match self {
            Self::MameRom => "mame_rom",
            Self::LogiqxRom => "logiqx_rom",
            Self::CmpRom => "cmp_rom",
            Self::NoIntroDatRom => "no_intro_dat_rom",
            Self::NoIntroPcFile => "no_intro_pc_file",
            Self::NoIntroDatabaseSourceFile => "no_intro_database_source_file",
            Self::NoIntroDatabaseReleaseFile => "no_intro_database_release_file",
            Self::SoftwareRomEntry => "software_rom_entry",
        }
    }

    const fn size_field(self) -> &'static str {
        match self {
            Self::MameRom => "mame_rom_size",
            Self::LogiqxRom => "logiqx_rom_size",
            Self::CmpRom => "cmp_rom_size",
            Self::NoIntroDatRom => "no_intro_dat_rom_size",
            Self::NoIntroPcFile => "no_intro_pc_file_size",
            Self::NoIntroDatabaseSourceFile => "no_intro_database_source_file_size",
            Self::NoIntroDatabaseReleaseFile => "no_intro_database_release_file_size",
            Self::SoftwareRomEntry => "software_rom_file_size",
        }
    }

    fn request_parts(self, size: Option<i64>) -> (CatalogDocumentFormat, String, &'static str) {
        let size = size.map_or_else(String::new, |size| format!(" size='{size}'"));
        let cmp_size = size
            .strip_prefix(" size='")
            .and_then(|size| size.strip_suffix('\''))
            .map_or_else(String::new, |size| format!(" SIZE {size}"));
        match self {
            Self::MameRom => (
                CatalogDocumentFormat::MameListXml,
                format!(
                    "<mame mameconfig='10'><machine name='game'><description>Game</description><rom name='game.bin'{size} sha1='{SHA1}'/></machine></mame>"
                ),
                "xml",
            ),
            Self::LogiqxRom => (
                CatalogDocumentFormat::Logiqx(LogiqxMode::ObservedCompatible),
                format!(
                    "<datafile><game name='game'><description>Game</description><rom name='game.bin'{size} sha1='{SHA1}'/></game></datafile>"
                ),
                "xml",
            ),
            Self::CmpRom => (
                CatalogDocumentFormat::ClrMamePro,
                format!(
                    "clrmamepro ( name Catalog description Description version v1 ) GAME ( NAME game DESCRIPTION Game ROM ( NAME game.bin{cmp_size} SHA1 {SHA1} ) )"
                ),
                "dat",
            ),
            Self::NoIntroDatRom => (
                CatalogDocumentFormat::NoIntroDat(NoIntroDatMode::V4Compatible),
                format!(
                    "<datafile><header><id>1</id><name>Catalog</name><description>Catalog</description><version>1</version><author>Tests</author></header><game name='game'><description>Game</description><rom name='game.bin'{size} sha1='{SHA1}'/></game></datafile>"
                ),
                "dat",
            ),
            Self::NoIntroPcFile => (
                CatalogDocumentFormat::NoIntroPcXml,
                format!(
                    "<datafile><game name='game'><description>Game</description><rom name='game.bin'{size} sha1='{SHA1}'/></game></datafile>"
                ),
                "xml",
            ),
            Self::NoIntroDatabaseSourceFile => (
                CatalogDocumentFormat::NoIntroDatabase(NoIntroDatabaseMode::ObservedCompatible),
                format!(
                    "<datafile><game name='game'><source><file id='source.bin' item='rom'{size} sha1='{SHA1}'/></source></game></datafile>"
                ),
                "xml",
            ),
            Self::NoIntroDatabaseReleaseFile => (
                CatalogDocumentFormat::NoIntroDatabase(NoIntroDatabaseMode::ObservedCompatible),
                format!(
                    "<datafile><game name='game'><release><file id='release.bin' item='rom'{size} sha1='{SHA1}'/></release></game></datafile>"
                ),
                "xml",
            ),
            Self::SoftwareRomEntry => (
                CatalogDocumentFormat::MameSoftwareListXml,
                format!(
                    "<softwarelist name='list'><software name='game'><description>Game</description><year>2000</year><publisher>Tests</publisher><part name='cart' interface='test'><dataarea name='rom' size='16'><rom name='game.bin'{size} sha1='{SHA1}'/></dataarea></part></software></softwarelist>"
                ),
                "xml",
            ),
        }
    }
}

#[derive(QueryableByName)]
struct TextValue {
    #[diesel(sql_type = Text)]
    value: String,
}

#[derive(QueryableByName)]
struct BinaryValue {
    #[diesel(sql_type = diesel::sql_types::Binary)]
    value: Vec<u8>,
}

#[derive(QueryableByName)]
struct CountValue {
    #[diesel(sql_type = BigInt)]
    value: i64,
}

#[derive(QueryableByName)]
struct DatabaseFileSizeRow {
    #[diesel(sql_type = Text)]
    evidence_scope: String,
    #[diesel(sql_type = Text)]
    source_size: String,
    #[diesel(sql_type = BigInt)]
    size: i64,
}

struct ImportedSnapshot {
    status: CatalogImportStatus,
    snapshot_key: Option<String>,
}

#[derive(Clone)]
struct GuardSql {
    name: &'static str,
    sql: String,
}

#[derive(Clone, Copy)]
enum PublicationExpectation {
    RejectContradiction,
    AcceptConsistent,
}

struct Fixture {
    _directory: tempfile::TempDir,
    database: Database,
    database_path: Utf8PathBuf,
}

impl Fixture {
    fn new() -> TestResult<Self> {
        let directory = tempfile::tempdir()?;
        let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
            .map_err(|_| "database path is not UTF-8")?;
        let database = Database::open(&database_path)?;
        Ok(Self {
            _directory: directory,
            database,
            database_path,
        })
    }

    fn import(
        &self,
        role: Role,
        label: &str,
        size: Option<i64>,
        second_size: Option<i64>,
    ) -> TestResult<ImportedSnapshot> {
        self.import_with_sha1(role, label, size, second_size, SHA1)
    }

    fn import_with_sha1(
        &self,
        role: Role,
        label: &str,
        size: Option<i64>,
        second_size: Option<i64>,
        sha1: &str,
    ) -> TestResult<ImportedSnapshot> {
        let (format, mut contents, extension) = role.request_parts(size);
        if let Some(second_size) = second_size {
            let second = role.second_entry(second_size);
            let marker = match role {
                Role::MameRom => "</machine>",
                Role::LogiqxRom | Role::NoIntroDatRom | Role::NoIntroPcFile => "</game>",
                Role::CmpRom => " ) )",
                Role::NoIntroDatabaseSourceFile => "</source>",
                Role::NoIntroDatabaseReleaseFile => "</release>",
                Role::SoftwareRomEntry => "</dataarea>",
            };
            contents = contents.replacen(marker, &format!("{second}{marker}"), 1);
        }
        contents = contents.replace(SHA1, sha1);
        let mut connection = SqliteConnection::establish(self.database_path.as_str())?;
        let publications_before = sql_query("SELECT COUNT(*) AS value FROM snapshot_publications")
            .get_result::<CountValue>(&mut connection)?
            .value;
        let path = self
            .database_path
            .parent()
            .ok_or("database has no parent directory")?
            .join(format!("{label}.{extension}"));
        std::fs::write(&path, contents)?;
        let report = app::import_catalog(
            &self.database,
            &CatalogImportRequest {
                document_path: path,
                format,
                source_key: PublishingSourceKey::new(format!("size-publication-{label}")),
                source_display_name: format!("Size publication {label}"),
                catalog_key: CatalogKey::new(format!("size-publication-{label}")),
                catalog_display_name: format!("Size publication {label}"),
                scope: CatalogScope::Complete,
            },
        )?;
        if report.status == CatalogImportStatus::Failed {
            assert!(
                report.snapshot_key.is_none(),
                "failed import exposed a snapshot"
            );
            let message = sql_query(
                "SELECT message AS value FROM import_diagnostics WHERE run_key=? ORDER BY diagnostic_key LIMIT 1",
            )
            .bind::<Text, _>(report.run_key.to_string())
            .get_result::<TextValue>(&mut connection)?
            .value;
            assert!(
                message.contains(
                    "catalog UUID has contradictory source whole-file lengths (source evidence)"
                ),
                "{label} failed for an unrelated reason: {message}"
            );
            let publications_after =
                sql_query("SELECT COUNT(*) AS value FROM snapshot_publications")
                    .get_result::<CountValue>(&mut connection)?
                    .value;
            assert_eq!(
                publications_after, publications_before,
                "rejected publication must leave the fixture reusable"
            );
        }
        Ok(ImportedSnapshot {
            status: report.status,
            snapshot_key: report.snapshot_key.map(|key| key.to_string()),
        })
    }

    fn import_unknown(&self, role: Role, label: &str) -> TestResult<ImportedSnapshot> {
        self.import(role, label, None, None)
    }

    fn connection(&self, foreign_keys: bool) -> TestResult<SqliteConnection> {
        let mut connection = SqliteConnection::establish(self.database_path.as_str())?;
        sql_query(if foreign_keys {
            "PRAGMA foreign_keys=ON"
        } else {
            "PRAGMA foreign_keys=OFF"
        })
        .execute(&mut connection)?;
        sql_query("PRAGMA recursive_triggers=OFF").execute(&mut connection)?;
        Ok(connection)
    }
}

#[derive(QueryableByName)]
struct OccurrenceRow {
    #[diesel(sql_type = BigInt)]
    id: i64,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Binary>)]
    uuid: Option<Vec<u8>>,
}

impl Role {
    fn second_entry(self, size: i64) -> String {
        match self {
            Self::MameRom
            | Self::LogiqxRom
            | Self::NoIntroDatRom
            | Self::NoIntroPcFile
            | Self::SoftwareRomEntry => {
                format!("<rom name='second.bin' size='{size}' sha1='{SHA1}'/>")
            }
            Self::CmpRom => format!(" ROM ( NAME second.bin SIZE {size} SHA1 {SHA1} )"),
            Self::NoIntroDatabaseSourceFile | Self::NoIntroDatabaseReleaseFile => {
                format!("<file id='second.bin' item='rom' size='{size}' sha1='{SHA1}'/>")
            }
        }
    }
}

fn trigger_sql(connection: &mut SqliteConnection, name: &'static str) -> TestResult<GuardSql> {
    Ok(GuardSql {
        name,
        sql: sql_query("SELECT sql AS value FROM sqlite_schema WHERE type='trigger' AND name=?")
            .bind::<Text, _>(name)
            .get_result::<TextValue>(connection)?
            .value,
    })
}

fn assert_source_digest(
    connection: &mut SqliteConnection,
    occurrence_id: i64,
    digest_bytes: &[u8],
) -> TestResult {
    let count = sql_query(
        "SELECT COUNT(*) AS value FROM occurrence_digest_assertions AS assertion \
         JOIN digest_values AS digest USING(digest_id) \
         WHERE assertion.occurrence_id=? AND digest.algorithm='sha1' AND digest.digest=?",
    )
    .bind::<BigInt, _>(occurrence_id)
    .bind::<diesel::sql_types::Binary, _>(digest_bytes)
    .get_result::<CountValue>(connection)?
    .value;
    assert_eq!(
        count, 1,
        "the copied occurrence must retain the fixture SHA1"
    );
    Ok(())
}

fn issued_uuid(
    connection: &mut SqliteConnection,
    snapshot_key: &str,
    role: Role,
) -> TestResult<Vec<u8>> {
    let occurrence = sql_query(
        "SELECT content_uuid AS value FROM asset_occurrences \
         JOIN catalog_sets ON set_id=record_id \
         JOIN catalog_set_groups USING(set_group_id) \
         WHERE snapshot_key=? AND claim_kind=?",
    )
    .bind::<Text, _>(snapshot_key)
    .bind::<Text, _>(role.claim_kind())
    .get_result::<BinaryValue>(connection)?
    .value;
    let issued = sql_query("SELECT COUNT(*) AS value FROM catalog_contents WHERE content_uuid=?")
        .bind::<diesel::sql_types::Binary, _>(&occurrence)
        .get_result::<CountValue>(connection)?
        .value;
    assert_eq!(
        issued, 1,
        "target UUID must have been issued by import evidence"
    );
    Ok(occurrence)
}

fn assert_issued_consistent_uuid(connection: &mut SqliteConnection, uuid: &[u8]) -> TestResult {
    let issued = sql_query("SELECT COUNT(*) AS value FROM catalog_contents WHERE content_uuid=?")
        .bind::<diesel::sql_types::Binary, _>(uuid)
        .get_result::<CountValue>(connection)?
        .value;
    assert_eq!(issued, 1, "positive control UUID must be issued");
    let inconsistent = sql_query(
        "SELECT inconsistent AS value FROM canonical_file_size_consistency \
         WHERE content_uuid=?",
    )
    .bind::<diesel::sql_types::Binary, _>(uuid)
    .get_result::<CountValue>(connection)?
    .value;
    assert_eq!(
        inconsistent, 0,
        "positive control component must be consistent"
    );
    Ok(())
}

fn snapshot_occurrences(
    connection: &mut SqliteConnection,
    snapshot_key: &str,
    role: Role,
) -> TestResult<Vec<OccurrenceRow>> {
    Ok(sql_query(
        "SELECT occurrence_id AS id, content_uuid AS uuid FROM asset_occurrences \
         JOIN catalog_sets ON set_id=record_id \
         JOIN catalog_set_groups USING(set_group_id) \
         WHERE snapshot_key=? AND claim_kind=? ORDER BY occurrence_order",
    )
    .bind::<Text, _>(snapshot_key)
    .bind::<Text, _>(role.claim_kind())
    .load(connection)?)
}

fn publish(connection: &mut SqliteConnection, snapshot_key: &str) -> diesel::QueryResult<usize> {
    sql_query(
        "INSERT INTO snapshot_publications(catalog_key,document_key,interpretation_key,snapshot_key) \
         SELECT catalog_key,document_key,interpretation_key,snapshot_key \
         FROM catalog_snapshots WHERE snapshot_key=?",
    )
    .bind::<Text, _>(snapshot_key)
    .execute(connection)
}

fn source_size(
    connection: &mut SqliteConnection,
    occurrence_id: i64,
    role: Role,
) -> TestResult<i64> {
    let row = sql_query(
        "SELECT size AS value FROM source_file_size_assertions \
         WHERE occurrence_id=? AND size_field=?",
    )
    .bind::<BigInt, _>(occurrence_id)
    .bind::<Text, _>(role.size_field())
    .get_result::<CountValue>(connection)
    .map_err(|error| {
        format!(
            "{} occurrence {occurrence_id} lacks a source-qualified size assertion: {error}",
            role.claim_kind()
        )
    })?;
    Ok(row.value)
}

fn reopen_snapshot_for_fixture(
    connection: &mut SqliteConnection,
    snapshot_key: &str,
    savepoint: &str,
    guards: &[GuardSql],
) -> TestResult {
    connection.batch_execute(&format!("SAVEPOINT {savepoint}"))?;
    for guard in guards {
        connection.batch_execute(&format!("DROP TRIGGER {}", guard.name))?;
    }
    let deleted = sql_query("DELETE FROM snapshot_publications WHERE snapshot_key=?")
        .bind::<Text, _>(snapshot_key)
        .execute(connection)?;
    assert_eq!(deleted, 1, "remove only the selected publication marker");
    Ok(())
}

fn restore_fixture_guards(connection: &mut SqliteConnection, guards: &[GuardSql]) -> TestResult {
    for guard in guards {
        connection.batch_execute(&guard.sql)?;
    }
    Ok(())
}

fn link_fixture_occurrences(
    connection: &mut SqliteConnection,
    occurrence_ids: &[i64],
    target_uuid: &[u8],
) -> TestResult {
    for occurrence_id in occurrence_ids {
        let updated =
            sql_query("UPDATE asset_occurrences SET content_uuid=? WHERE occurrence_id=?")
                .bind::<diesel::sql_types::Binary, _>(target_uuid)
                .bind::<BigInt, _>(occurrence_id)
                .execute(connection)?;
        assert_eq!(updated, 1, "update only a selected occurrence UUID");
    }
    Ok(())
}

fn publish_and_restore_fixture(
    connection: &mut SqliteConnection,
    snapshot_key: &str,
    role: Role,
    originals: &[Option<Vec<u8>>],
    savepoint: &str,
    expectation: PublicationExpectation,
) -> TestResult {
    let result = publish(connection, snapshot_key);
    match expectation {
        PublicationExpectation::RejectContradiction => {
            let error = match result {
                Ok(inserted) => {
                    return Err(format!(
                        "a contradictory linked component unexpectedly published ({inserted} rows)"
                    )
                    .into());
                }
                Err(error) => error,
            };
            assert!(
                error.to_string().contains(
                    "catalog UUID has contradictory source whole-file lengths (source evidence)"
                ),
                "unexpected publication rejection: {error}"
            );
        }
        PublicationExpectation::AcceptConsistent => {
            assert_eq!(result?, 1, "consistent fixture must publish one snapshot");
        }
    }
    connection.batch_execute(&format!(
        "ROLLBACK TO SAVEPOINT {savepoint}; RELEASE SAVEPOINT {savepoint}"
    ))?;
    let restored = snapshot_occurrences(connection, snapshot_key, role)?;
    assert_eq!(
        restored
            .iter()
            .map(|occurrence| occurrence.uuid.clone())
            .collect::<Vec<_>>(),
        originals,
        "savepoint restores source UUIDs"
    );
    let publications =
        sql_query("SELECT COUNT(*) AS value FROM snapshot_publications WHERE snapshot_key=?")
            .bind::<Text, _>(snapshot_key)
            .get_result::<CountValue>(connection)?
            .value;
    assert_eq!(publications, 1, "savepoint restores original publication");
    Ok(())
}

fn stage_forged_component_publication(
    connection: &mut SqliteConnection,
    candidate_snapshot: &str,
    candidate_role: Role,
    root_occurrence_id: i64,
    target_uuid: &[u8],
    expected_candidate_sizes: &[i64],
) -> TestResult {
    if candidate_role.database_file_owner().is_some() {
        assert_eq!(expected_candidate_sizes.len(), 1);
        return stage_database_whole_file_publication(
            connection,
            candidate_snapshot,
            candidate_role,
            target_uuid,
            expected_candidate_sizes[0],
            PublicationExpectation::RejectContradiction,
        );
    }
    let delete_guard = trigger_sql(connection, PUBLICATION_DELETE_GUARD)?;
    let update_guard = trigger_sql(connection, OCCURRENCE_UPDATE_GUARD)?;
    let occurrences = snapshot_occurrences(connection, candidate_snapshot, candidate_role)?;
    assert_eq!(
        occurrences.len(),
        expected_candidate_sizes.len(),
        "candidate source fixture occurrence count"
    );
    assert_eq!(
        source_size(connection, root_occurrence_id, Role::SoftwareRomEntry)?,
        4
    );
    for (occurrence, size) in occurrences.iter().zip(expected_candidate_sizes) {
        assert_eq!(
            source_size(connection, occurrence.id, candidate_role)?,
            *size
        );
        assert_source_digest(connection, occurrence.id, &SHA1_BYTES)?;
    }
    let original_uuids = occurrences
        .iter()
        .map(|occurrence| occurrence.uuid.clone())
        .collect::<Vec<_>>();

    let guards = [delete_guard, update_guard];
    reopen_snapshot_for_fixture(
        connection,
        candidate_snapshot,
        "linked_size_publication_case",
        &guards,
    )?;
    // This fixture-only rewrite is not a production content_uuid update.
    let occurrence_ids = occurrences
        .iter()
        .map(|occurrence| occurrence.id)
        .collect::<Vec<_>>();
    link_fixture_occurrences(connection, &occurrence_ids, target_uuid)?;
    restore_fixture_guards(connection, &guards)?;

    let linked = snapshot_occurrences(connection, candidate_snapshot, candidate_role)?;
    assert!(
        linked
            .iter()
            .all(|occurrence| occurrence.uuid.as_deref() == Some(target_uuid))
    );
    assert_source_digest(connection, root_occurrence_id, &SHA1_BYTES)?;
    assert_eq!(
        source_size(connection, root_occurrence_id, Role::SoftwareRomEntry)?,
        4
    );
    for (occurrence, size) in linked.iter().zip(expected_candidate_sizes) {
        assert_eq!(occurrence.uuid.as_deref(), Some(target_uuid));
        assert_eq!(
            source_size(connection, occurrence.id, candidate_role)?,
            *size
        );
    }

    publish_and_restore_fixture(
        connection,
        candidate_snapshot,
        candidate_role,
        &original_uuids,
        "linked_size_publication_case",
        PublicationExpectation::RejectContradiction,
    )?;
    Ok(())
}

fn stage_database_whole_file_publication(
    connection: &mut SqliteConnection,
    snapshot_key: &str,
    role: Role,
    target_uuid: &[u8],
    candidate_size: i64,
    expectation: PublicationExpectation,
) -> TestResult {
    let (owner_table, owner_update_name) = role
        .database_file_owner()
        .ok_or("expected a No-Intro database file role")?;
    let delete_guard = trigger_sql(connection, PUBLICATION_DELETE_GUARD)?;
    let occurrence_guard = trigger_sql(connection, OCCURRENCE_UPDATE_GUARD)?;
    let owner_guard = trigger_sql(connection, owner_update_name)?;
    let digest_guard = trigger_sql(connection, "occurrence_digest_immutable_update")?;
    let occurrences = snapshot_occurrences(connection, snapshot_key, role)?;
    assert_eq!(
        occurrences.len(),
        1,
        "{} native occurrence",
        role.claim_kind()
    );
    assert_database_public_unknown(connection, role, occurrences[0].id, candidate_size)?;
    let originals = occurrences
        .iter()
        .map(|occurrence| occurrence.uuid.clone())
        .collect::<Vec<_>>();
    let guards = [delete_guard, occurrence_guard, owner_guard, digest_guard];
    qualify_database_fixture_owner(
        connection,
        snapshot_key,
        owner_table,
        occurrences[0].id,
        target_uuid,
        &guards,
    )?;
    assert_database_fixture_qualified(
        connection,
        role,
        occurrences[0].id,
        target_uuid,
        candidate_size,
        expectation,
    )?;
    publish_and_restore_fixture(
        connection,
        snapshot_key,
        role,
        &originals,
        "qualify_database_native_fixture",
        expectation,
    )?;
    Ok(())
}

fn assert_database_public_unknown(
    connection: &mut SqliteConnection,
    role: Role,
    occurrence_id: i64,
    candidate_size: i64,
) -> TestResult {
    let (table, _) = role
        .database_file_owner()
        .ok_or("expected a No-Intro database file role")?;
    let owner = sql_query(format!(
        "SELECT evidence_scope,source_size,size FROM {table} WHERE occurrence_id=?"
    ))
    .bind::<BigInt, _>(occurrence_id)
    .get_result::<DatabaseFileSizeRow>(connection)?;
    assert_eq!(owner.evidence_scope, "unknown");
    assert_eq!(owner.source_size, candidate_size.to_string());
    assert_eq!(owner.size, candidate_size);
    let qualified = sql_query(
        "SELECT COUNT(*) AS value FROM source_file_size_assertions \
         WHERE occurrence_id=? AND size_field=?",
    )
    .bind::<BigInt, _>(occurrence_id)
    .bind::<Text, _>(role.size_field())
    .get_result::<CountValue>(connection)?
    .value;
    assert_eq!(qualified, 0, "public DB size is not whole-file qualified");
    assert_source_digest(connection, occurrence_id, &SHA1_BYTES)?;
    let unknown_scope = sql_query(
        "SELECT COUNT(*) AS value FROM occurrence_digest_assertions \
         WHERE occurrence_id=? AND provenance='source_declared' AND scope='unknown' \
         AND digest_id IN (SELECT digest_id FROM digest_values WHERE algorithm='sha1')",
    )
    .bind::<BigInt, _>(occurrence_id)
    .get_result::<CountValue>(connection)?
    .value;
    assert_eq!(unknown_scope, 1, "native SHA1 starts at unknown scope");
    Ok(())
}

fn qualify_database_fixture_owner(
    connection: &mut SqliteConnection,
    snapshot_key: &str,
    owner_table: &str,
    occurrence_id: i64,
    target_uuid: &[u8],
    guards: &[GuardSql],
) -> TestResult {
    // The parser intentionally retains unknown scope; this adversarial fixture
    // promotes only the native owner and its matching SHA1 assertion.
    reopen_snapshot_for_fixture(
        connection,
        snapshot_key,
        "qualify_database_native_fixture",
        guards,
    )?;
    let qualified = sql_query(format!(
        "UPDATE {owner_table} SET evidence_scope='whole_file' WHERE occurrence_id=?"
    ))
    .bind::<BigInt, _>(occurrence_id)
    .execute(connection)?;
    assert_eq!(qualified, 1, "qualify only the selected native owner");
    let digest_scope = sql_query(
        "UPDATE occurrence_digest_assertions SET scope='whole_file' \
         WHERE occurrence_id=? AND provenance='source_declared' AND scope='unknown' \
         AND digest_id IN (SELECT digest_id FROM digest_values WHERE algorithm='sha1')",
    )
    .bind::<BigInt, _>(occurrence_id)
    .execute(connection)?;
    assert_eq!(
        digest_scope, 1,
        "retain exact source SHA1 at whole-file scope"
    );
    link_fixture_occurrences(connection, &[occurrence_id], target_uuid)?;
    restore_fixture_guards(connection, guards)?;
    Ok(())
}

fn assert_database_fixture_qualified(
    connection: &mut SqliteConnection,
    role: Role,
    occurrence_id: i64,
    target_uuid: &[u8],
    candidate_size: i64,
    expectation: PublicationExpectation,
) -> TestResult {
    let linked_uuid =
        sql_query("SELECT content_uuid AS value FROM asset_occurrences WHERE occurrence_id=?")
            .bind::<BigInt, _>(occurrence_id)
            .get_result::<BinaryValue>(connection)?
            .value;
    assert_eq!(
        linked_uuid, target_uuid,
        "native DB occurrence shares target UUID"
    );
    assert_eq!(
        source_size(connection, occurrence_id, role)?,
        candidate_size
    );
    assert_source_digest(connection, occurrence_id, &SHA1_BYTES)?;
    assert_eq!(issued_uuid_for_content(connection, target_uuid)?, 1);
    let inconsistent = sql_query(
        "SELECT inconsistent AS value FROM canonical_file_size_consistency \
         WHERE content_uuid=?",
    )
    .bind::<diesel::sql_types::Binary, _>(target_uuid)
    .get_result::<CountValue>(connection)?
    .value;
    let expected = match expectation {
        PublicationExpectation::RejectContradiction => 1,
        PublicationExpectation::AcceptConsistent => 0,
    };
    assert_eq!(inconsistent, expected);
    Ok(())
}

fn issued_uuid_for_content(connection: &mut SqliteConnection, uuid: &[u8]) -> TestResult<i64> {
    Ok(
        sql_query("SELECT COUNT(*) AS value FROM catalog_contents WHERE content_uuid=?")
            .bind::<diesel::sql_types::Binary, _>(uuid)
            .get_result::<CountValue>(connection)?
            .value,
    )
}

fn seed_inconsistent_component(
    connection: &mut SqliteConnection,
    candidate_snapshot: &str,
    target_uuid: &[u8],
) -> TestResult {
    let delete_guard = trigger_sql(connection, PUBLICATION_DELETE_GUARD)?;
    let update_guard = trigger_sql(connection, OCCURRENCE_UPDATE_GUARD)?;
    let size_guard = trigger_sql(connection, LINKED_SIZE_GUARD)?;
    let occurrences = snapshot_occurrences(connection, candidate_snapshot, Role::NoIntroDatRom)?;
    assert_eq!(occurrences.len(), 2);
    assert_eq!(
        source_size(connection, occurrences[0].id, Role::NoIntroDatRom)?,
        5
    );
    assert_eq!(
        source_size(connection, occurrences[1].id, Role::NoIntroDatRom)?,
        6
    );

    connection.batch_execute("SAVEPOINT seed_inconsistent_component")?;
    connection.batch_execute(&format!(
        "DROP TRIGGER {}; DROP TRIGGER {}; DROP TRIGGER {}",
        delete_guard.name, update_guard.name, size_guard.name
    ))?;
    let deleted = sql_query("DELETE FROM snapshot_publications WHERE snapshot_key=?")
        .bind::<Text, _>(candidate_snapshot)
        .execute(connection)?;
    assert_eq!(deleted, 1);
    for occurrence in &occurrences {
        let updated =
            sql_query("UPDATE asset_occurrences SET content_uuid=? WHERE occurrence_id=?")
                .bind::<diesel::sql_types::Binary, _>(target_uuid)
                .bind::<BigInt, _>(occurrence.id)
                .execute(connection)?;
        assert_eq!(updated, 1);
    }
    connection.batch_execute(&delete_guard.sql)?;
    connection.batch_execute(&update_guard.sql)?;
    publish(connection, candidate_snapshot)?;
    connection.batch_execute(&size_guard.sql)?;
    connection.batch_execute("RELEASE SAVEPOINT seed_inconsistent_component")?;
    Ok(())
}

fn assert_unknown_link_rejected(
    connection: &mut SqliteConnection,
    candidate_snapshot: &str,
    target_uuid: &[u8],
) -> TestResult {
    let delete_guard = trigger_sql(connection, PUBLICATION_DELETE_GUARD)?;
    let occurrences = snapshot_occurrences(connection, candidate_snapshot, Role::NoIntroPcFile)?;
    assert_eq!(occurrences.len(), 1);
    assert_eq!(
        occurrences[0].uuid.as_deref(),
        Some(target_uuid),
        "public import should retain the root identity for matching native SHA1"
    );
    let source_size_count = sql_query(
        "SELECT COUNT(*) AS value FROM source_file_size_assertions \
         WHERE occurrence_id=? AND size_field=?",
    )
    .bind::<BigInt, _>(occurrences[0].id)
    .bind::<Text, _>(Role::NoIntroPcFile.size_field())
    .get_result::<CountValue>(connection)?
    .value;
    assert_eq!(
        source_size_count, 0,
        "incoming occurrence must be size-unknown"
    );
    assert_source_digest(connection, occurrences[0].id, &SHA1_BYTES)?;

    connection.batch_execute("SAVEPOINT link_unknown_candidate")?;
    connection.batch_execute(&format!("DROP TRIGGER {}", delete_guard.name))?;
    let deleted = sql_query("DELETE FROM snapshot_publications WHERE snapshot_key=?")
        .bind::<Text, _>(candidate_snapshot)
        .execute(connection)?;
    assert_eq!(deleted, 1);
    connection.batch_execute(&delete_guard.sql)?;

    let error = publish(connection, candidate_snapshot)
        .err()
        .ok_or("unknown-size link to an inconsistent component must not publish")?;
    assert!(
        error
            .to_string()
            .contains("catalog UUID has contradictory source whole-file lengths (source evidence)"),
        "unexpected publication rejection: {error}"
    );
    connection.batch_execute(
        "ROLLBACK TO SAVEPOINT link_unknown_candidate; \
         RELEASE SAVEPOINT link_unknown_candidate",
    )?;
    Ok(())
}

#[test]
fn native_import_fixtures_keep_equal_qualified_size_facts() -> TestResult {
    let fixture = Fixture::new()?;
    let mut connection = fixture.connection(true)?;
    let mut shared_uuid = None;
    for role in Role::PUBLICLY_QUALIFIED {
        let imported = fixture.import(role, role.claim_kind(), Some(4), None)?;
        assert_eq!(imported.status, CatalogImportStatus::Succeeded);
        let snapshot = imported
            .snapshot_key
            .ok_or("published import has no snapshot")?;
        let occurrences = snapshot_occurrences(&mut connection, &snapshot, role)?;
        assert_eq!(
            occurrences.len(),
            1,
            "{} fixture must contain one occurrence",
            role.claim_kind()
        );
        assert_eq!(source_size(&mut connection, occurrences[0].id, role)?, 4);
        let uuid = occurrences[0]
            .uuid
            .as_deref()
            .ok_or_else(|| format!("{} equal-size occurrence is unlinked", role.claim_kind()))?;
        if let Some(expected) = shared_uuid.as_deref() {
            assert_eq!(
                uuid,
                expected,
                "{} must share the component UUID",
                role.claim_kind()
            );
        } else {
            shared_uuid = Some(uuid.to_vec());
        }
        assert_issued_consistent_uuid(&mut connection, uuid)?;
    }
    Ok(())
}

#[test]
fn no_intro_database_public_scope_remains_unknown_and_unlinked() -> TestResult {
    for role in [
        Role::NoIntroDatabaseSourceFile,
        Role::NoIntroDatabaseReleaseFile,
    ] {
        let fixture = Fixture::new()?;
        let imported = fixture.import(role, role.claim_kind(), Some(4), None)?;
        assert_eq!(imported.status, CatalogImportStatus::Succeeded);
        let snapshot = imported
            .snapshot_key
            .ok_or("published No-Intro database snapshot missing")?;
        let mut connection = fixture.connection(true)?;
        let occurrences = snapshot_occurrences(&mut connection, &snapshot, role)?;
        assert_eq!(
            occurrences.len(),
            1,
            "{} occurrence count",
            role.claim_kind()
        );
        assert!(
            occurrences[0].uuid.is_none(),
            "unknown-scope DB occurrence must stay unlinked"
        );
        let (table, _) = role
            .database_file_owner()
            .ok_or("No-Intro database role expected")?;
        let owner = sql_query(format!(
            "SELECT evidence_scope,source_size,size FROM {table} WHERE occurrence_id=?"
        ))
        .bind::<BigInt, _>(occurrences[0].id)
        .get_result::<DatabaseFileSizeRow>(&mut connection)?;
        assert_eq!(owner.evidence_scope, "unknown");
        assert_eq!(owner.source_size, "4");
        assert_eq!(owner.size, 4);
        let unknown_sha1 = sql_query(
            "SELECT COUNT(*) AS value FROM occurrence_digest_assertions AS assertion \
             JOIN digest_values AS digest USING(digest_id) \
             WHERE assertion.occurrence_id=? AND assertion.provenance='source_declared' \
             AND assertion.scope='unknown' AND digest.algorithm='sha1'",
        )
        .bind::<BigInt, _>(occurrences[0].id)
        .get_result::<CountValue>(&mut connection)?
        .value;
        assert_eq!(unknown_sha1, 1, "native SHA1 remains unknown-scoped");
        let qualified_sizes = sql_query(
            "SELECT COUNT(*) AS value FROM source_file_size_assertions \
             WHERE occurrence_id=? AND size_field=?",
        )
        .bind::<BigInt, _>(occurrences[0].id)
        .bind::<Text, _>(role.size_field())
        .get_result::<CountValue>(&mut connection)?
        .value;
        assert_eq!(
            qualified_sizes, 0,
            "raw size must not become whole-file evidence"
        );
    }
    Ok(())
}

#[test]
fn no_intro_database_whole_file_sql_fixtures_publish_equal_lengths() -> TestResult {
    for foreign_keys in [false, true] {
        for role in [
            Role::NoIntroDatabaseSourceFile,
            Role::NoIntroDatabaseReleaseFile,
        ] {
            let fixture = Fixture::new()?;
            let root = fixture.import(
                Role::SoftwareRomEntry,
                "database-whole-file-root",
                Some(4),
                None,
            )?;
            assert_eq!(root.status, CatalogImportStatus::Succeeded);
            let root_snapshot = root.snapshot_key.ok_or("database fixture root missing")?;
            let candidate = fixture.import(role, role.claim_kind(), Some(4), None)?;
            assert_eq!(candidate.status, CatalogImportStatus::Succeeded);
            let candidate_snapshot = candidate
                .snapshot_key
                .ok_or("database native candidate snapshot missing")?;
            let mut connection = fixture.connection(foreign_keys)?;
            let target_uuid = issued_uuid(&mut connection, &root_snapshot, Role::SoftwareRomEntry)?;
            stage_database_whole_file_publication(
                &mut connection,
                &candidate_snapshot,
                role,
                &target_uuid,
                4,
                PublicationExpectation::AcceptConsistent,
            )?;
        }
    }
    Ok(())
}

#[test]
fn normal_public_conflict_import_remains_successful_but_unlinked() -> TestResult {
    let fixture = Fixture::new()?;
    let root = fixture.import(Role::SoftwareRomEntry, "root-software-four", Some(4), None)?;
    assert_eq!(root.status, CatalogImportStatus::Succeeded);
    let root_snapshot = root.snapshot_key.ok_or("published root has no snapshot")?;
    let candidate = fixture.import(Role::LogiqxRom, "public-logiqx-five", Some(5), None)?;
    assert_eq!(candidate.status, CatalogImportStatus::Succeeded);
    let snapshot = candidate
        .snapshot_key
        .ok_or("published candidate has no snapshot")?;
    let mut connection = fixture.connection(true)?;
    let root_occurrences =
        snapshot_occurrences(&mut connection, &root_snapshot, Role::SoftwareRomEntry)?;
    assert_eq!(root_occurrences.len(), 1);
    let target_uuid = issued_uuid(&mut connection, &root_snapshot, Role::SoftwareRomEntry)?;
    assert_eq!(
        root_occurrences[0].uuid.as_deref(),
        Some(target_uuid.as_slice())
    );
    assert_source_digest(&mut connection, root_occurrences[0].id, &SHA1_BYTES)?;
    let incoming = snapshot_occurrences(&mut connection, &snapshot, Role::LogiqxRom)?;
    assert_eq!(incoming.len(), 1);
    assert!(
        incoming[0].uuid.is_none(),
        "automatic resolver must leave this conflict unlinked"
    );
    assert_source_digest(&mut connection, incoming[0].id, &SHA1_BYTES)?;
    assert_eq!(
        source_size(&mut connection, incoming[0].id, Role::LogiqxRom)?,
        5
    );
    Ok(())
}

#[test]
fn two_same_snapshot_native_occurrences_are_checked_after_direct_linking() -> TestResult {
    let fixture = Fixture::new()?;
    let root = fixture.import(Role::SoftwareRomEntry, "same-snapshot-root", Some(4), None)?;
    assert_eq!(root.status, CatalogImportStatus::Succeeded);
    let root_snapshot = root.snapshot_key.ok_or("published root has no snapshot")?;
    let candidate = fixture.import(
        Role::NoIntroDatRom,
        "same-snapshot-two-dat-roms",
        Some(4),
        Some(5),
    )?;
    assert_eq!(candidate.status, CatalogImportStatus::Succeeded);
    let candidate_snapshot = candidate
        .snapshot_key
        .ok_or("published candidate has no snapshot")?;
    let mut connection = fixture.connection(false)?;
    let root_occurrences =
        snapshot_occurrences(&mut connection, &root_snapshot, Role::SoftwareRomEntry)?;
    assert_eq!(root_occurrences.len(), 1);
    let target_uuid = issued_uuid(&mut connection, &root_snapshot, Role::SoftwareRomEntry)?;
    assert_issued_consistent_uuid(&mut connection, &target_uuid)?;
    let dat_occurrences =
        snapshot_occurrences(&mut connection, &candidate_snapshot, Role::NoIntroDatRom)?;
    assert_eq!(
        dat_occurrences.len(),
        2,
        "same-snapshot DAT occurrence count"
    );
    assert_eq!(
        dat_occurrences[0].uuid.as_deref(),
        Some(target_uuid.as_slice())
    );
    assert!(dat_occurrences[1].uuid.is_none());
    assert_eq!(
        source_size(&mut connection, dat_occurrences[0].id, Role::NoIntroDatRom)?,
        4
    );
    assert_eq!(
        source_size(&mut connection, dat_occurrences[1].id, Role::NoIntroDatRom)?,
        5
    );
    assert_source_digest(&mut connection, dat_occurrences[0].id, &SHA1_BYTES)?;
    assert_source_digest(&mut connection, dat_occurrences[1].id, &SHA1_BYTES)?;
    stage_forged_component_publication(
        &mut connection,
        &candidate_snapshot,
        Role::NoIntroDatRom,
        root_occurrences[0].id,
        &target_uuid,
        &[4, 5],
    )?;
    Ok(())
}

#[test]
fn unknown_native_occurrence_cannot_hide_an_inconsistent_linked_component() -> TestResult {
    for foreign_keys in [false, true] {
        let fixture = Fixture::new()?;
        let root = fixture.import(
            Role::SoftwareRomEntry,
            "unknown-component-root",
            Some(4),
            None,
        )?;
        assert_eq!(root.status, CatalogImportStatus::Succeeded);
        let root_snapshot = root.snapshot_key.ok_or("software root snapshot missing")?;
        let mut connection = fixture.connection(foreign_keys)?;
        let target_uuid = issued_uuid(&mut connection, &root_snapshot, Role::SoftwareRomEntry)?;

        let unknown = fixture.import_unknown(Role::NoIntroPcFile, "unknown-linked-native")?;
        assert_eq!(unknown.status, CatalogImportStatus::Succeeded);
        let unknown_snapshot = unknown
            .snapshot_key
            .ok_or("unknown-size source snapshot missing")?;
        let unknown_occurrence =
            snapshot_occurrences(&mut connection, &unknown_snapshot, Role::NoIntroPcFile)?;
        assert_eq!(unknown_occurrence.len(), 1);
        assert_eq!(
            unknown_occurrence[0].uuid.as_deref(),
            Some(target_uuid.as_slice())
        );

        let inconsistent = fixture.import(
            Role::NoIntroDatRom,
            "unknown-component-inconsistent-dat",
            Some(5),
            Some(6),
        )?;
        assert_eq!(inconsistent.status, CatalogImportStatus::Succeeded);
        let inconsistent_snapshot = inconsistent
            .snapshot_key
            .ok_or("inconsistent source snapshot missing")?;
        seed_inconsistent_component(&mut connection, &inconsistent_snapshot, &target_uuid)?;
        assert_unknown_link_rejected(&mut connection, &unknown_snapshot, &target_uuid)?;
    }
    Ok(())
}

#[test]
fn direct_native_candidates_reject_conflicting_sizes_for_all_eight_roles() -> TestResult {
    for foreign_keys in [false, true] {
        let fixture = Fixture::new()?;
        let root = fixture.import(Role::SoftwareRomEntry, "root-software-four", Some(4), None)?;
        assert_eq!(root.status, CatalogImportStatus::Succeeded);
        let root_snapshot = root.snapshot_key.ok_or("software root snapshot missing")?;
        let mut connection = fixture.connection(foreign_keys)?;
        let root_occurrences =
            snapshot_occurrences(&mut connection, &root_snapshot, Role::SoftwareRomEntry)?;
        assert_eq!(root_occurrences.len(), 1);
        let target_uuid = issued_uuid(&mut connection, &root_snapshot, Role::SoftwareRomEntry)?;
        assert_eq!(
            root_occurrences[0].uuid.as_deref(),
            Some(target_uuid.as_slice())
        );
        assert_eq!(
            source_size(
                &mut connection,
                root_occurrences[0].id,
                Role::SoftwareRomEntry
            )?,
            4
        );
        assert_source_digest(&mut connection, root_occurrences[0].id, &SHA1_BYTES)?;

        for role in Role::ALL {
            let label = format!("{}-candidate-{foreign_keys}", role.claim_kind());
            let imported = fixture.import(role, &label, Some(5), None)?;
            assert_eq!(
                imported.status,
                CatalogImportStatus::Succeeded,
                "public fixture import should remain valid while automatic identity is unlinked"
            );
            let candidate_snapshot = imported.snapshot_key.ok_or("candidate snapshot missing")?;
            stage_forged_component_publication(
                &mut connection,
                &candidate_snapshot,
                role,
                root_occurrences[0].id,
                &target_uuid,
                &[5],
            )?;
        }
    }
    Ok(())
}

#[test]
fn unknown_and_unknown_only_lengths_remain_publishable() -> TestResult {
    let consistent = Fixture::new()?;
    let known = consistent.import(Role::MameRom, "known-four", Some(4), None)?;
    assert_eq!(known.status, CatalogImportStatus::Succeeded);
    let known_snapshot = known.snapshot_key.ok_or("known snapshot missing")?;
    let unknown = consistent.import_unknown(Role::NoIntroPcFile, "unknown-against-four")?;
    assert_eq!(unknown.status, CatalogImportStatus::Succeeded);
    let unknown_snapshot = unknown.snapshot_key.ok_or("unknown snapshot missing")?;
    let mut connection = consistent.connection(true)?;
    let known_occurrences = snapshot_occurrences(&mut connection, &known_snapshot, Role::MameRom)?;
    let unknown_occurrences =
        snapshot_occurrences(&mut connection, &unknown_snapshot, Role::NoIntroPcFile)?;
    assert_eq!(known_occurrences.len(), 1);
    assert_eq!(unknown_occurrences.len(), 1);
    let known_uuid = issued_uuid(&mut connection, &known_snapshot, Role::MameRom)?;
    assert_eq!(
        known_occurrences[0].uuid.as_deref(),
        Some(known_uuid.as_slice())
    );
    assert_eq!(
        unknown_occurrences[0].uuid.as_deref(),
        Some(known_uuid.as_slice())
    );
    assert_issued_consistent_uuid(&mut connection, &known_uuid)?;

    let unknown_only = Fixture::new()?;
    let first = unknown_only.import_unknown(Role::MameRom, "unknown-first")?;
    assert_eq!(first.status, CatalogImportStatus::Succeeded);
    let first_snapshot = first.snapshot_key.ok_or("first unknown snapshot missing")?;
    let second = unknown_only.import_unknown(Role::SoftwareRomEntry, "unknown-second")?;
    assert_eq!(second.status, CatalogImportStatus::Succeeded);
    let second_snapshot = second
        .snapshot_key
        .ok_or("second unknown snapshot missing")?;
    let mut connection = unknown_only.connection(true)?;
    let first_occurrences = snapshot_occurrences(&mut connection, &first_snapshot, Role::MameRom)?;
    let second_occurrences =
        snapshot_occurrences(&mut connection, &second_snapshot, Role::SoftwareRomEntry)?;
    assert_eq!(first_occurrences.len(), 1);
    assert_eq!(second_occurrences.len(), 1);
    let first_uuid = issued_uuid(&mut connection, &first_snapshot, Role::MameRom)?;
    assert_eq!(
        first_occurrences[0].uuid.as_deref(),
        Some(first_uuid.as_slice())
    );
    assert_eq!(
        second_occurrences[0].uuid.as_deref(),
        Some(first_uuid.as_slice())
    );
    assert_issued_consistent_uuid(&mut connection, &first_uuid)?;
    Ok(())
}
