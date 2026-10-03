use camino::Utf8PathBuf;
use diesel::{
    Connection, QueryableByName, RunQueryDsl, SqliteConnection, sql_query,
    sql_types::{BigInt, Text},
};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest},
    catalog_files::{self, LogiqxFilePayload, OccurrenceId},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey, SnapshotKey},
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const BODY: &str = "<datafile xmlns:v='urn:vendor' v:vendor='é𝄞' debug='no' build=''>\r\n\t<header><name>All fields</name><description>Fixture</description><version>1</version><author>Fixture</author>\r\n\t<clrmamepro v:vendor='é𝄞' forcepacking='zip' forcenodump='obsolete' forcemerging='split' header=''/>\r\n\t<romcenter v:vendor='é𝄞' locksamplemode='no' lockbiosmode='no' lockrommode='no' samplemode='merged' biosmode='split' rommode='split' plugin=''/>\r\n\t</header>\r\n\t<game v:vendor='é𝄞' rebuildto='rebuilt' board='' sampleof='parent' romof='parent' cloneof='parent' isbios='no' sourcefile='' name='all'>\r\n\t<description>All</description>\r\n\t<release v:vendor='é𝄞' default='no' date='' language='' region='US' name='World'/>\r\n\t<biosset v:vendor='é𝄞' default='no' description='' name='base'/>\r\n\t<rom v:vendor='é𝄞' serial=''\r\n\tdate='' status='good' merge='parent.bin' md5='11111111111111111111111111111111' sha1='2222222222222222222222222222222222222222' crc='12345678' size='0001' name='all.bin'/>\r\n\t<disk v:vendor='é𝄞' status='good' merge='parent.chd' md5='11111111111111111111111111111111' sha1='2222222222222222222222222222222222222222' name='all.chd'/>\r\n\t<sample v:vendor='é𝄞' name='sample'/>\r\n\t<archive v:vendor='é𝄞' name='archive'/>\r\n\t<device_ref v:vendor='é𝄞' name='device'/>\r\n\t<device_ref v:vendor='é𝄞' v:name='lookalike' name='device'/>\r\n\t</game></datafile>";

struct Catalog {
    directory: tempfile::TempDir,
    path: Utf8PathBuf,
    database: Database,
}

impl Catalog {
    fn new() -> TestResult<Self> {
        let directory = tempfile::tempdir()?;
        let path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
        let database = Database::open(&path)?;
        Ok(Self {
            directory,
            path,
            database,
        })
    }
    fn import(&self, name: &str, bytes: &[u8]) -> TestResult<SnapshotKey> {
        let report = app::import_catalog(&self.database, &self.request(name, bytes)?)?;
        assert_eq!(report.status, app::CatalogImportStatus::Succeeded);
        Ok(report.snapshot_key.ok_or("missing successful snapshot")?)
    }

    fn request(&self, name: &str, bytes: &[u8]) -> TestResult<CatalogImportRequest> {
        let document_path =
            Utf8PathBuf::try_from(self.directory.path().join(format!("{name}.xml")))?;
        std::fs::write(&document_path, bytes)?;
        Ok(CatalogImportRequest {
            document_path,
            format: CatalogDocumentFormat::Logiqx,
            source_key: PublishingSourceKey::new("logiqx-all-attributes"),
            source_display_name: "Logiqx fields".into(),
            catalog_key: CatalogKey::new("logiqx-all-attributes"),
            catalog_display_name: "Logiqx fields".into(),
            scope: CatalogScope::Complete,
        })
    }
    fn connection(&self) -> TestResult<SqliteConnection> {
        Ok(SqliteConnection::establish(self.path.as_str())?)
    }
}

#[derive(QueryableByName)]
struct Position {
    #[diesel(sql_type=BigInt)]
    field_kind: i64,
    #[diesel(sql_type=BigInt)]
    source_order: i64,
    #[diesel(sql_type=BigInt)]
    source_line: i64,
    #[diesel(sql_type=BigInt)]
    source_column: i64,
}

enum Owner {
    Document,
    Set,
    Occurrence,
}

struct Witness {
    table: &'static str,
    owner: Owner,
    tag: &'static str,
    fields: &'static [(i64, &'static str)],
}

const WITNESSES: [Witness; 10] = [
    Witness {
        table: "logiqx_document_attribute_positions",
        owner: Owner::Document,
        tag: "datafile",
        fields: &[(1, "debug"), (0, "build")],
    },
    Witness {
        table: "logiqx_clrmamepro_attribute_positions",
        owner: Owner::Document,
        tag: "clrmamepro",
        fields: &[
            (3, "forcepacking"),
            (2, "forcenodump"),
            (1, "forcemerging"),
            (0, "header"),
        ],
    },
    Witness {
        table: "logiqx_romcenter_attribute_positions",
        owner: Owner::Document,
        tag: "romcenter",
        fields: &[
            (6, "locksamplemode"),
            (5, "lockbiosmode"),
            (4, "lockrommode"),
            (3, "samplemode"),
            (2, "biosmode"),
            (1, "rommode"),
            (0, "plugin"),
        ],
    },
    Witness {
        table: "logiqx_game_attribute_positions",
        owner: Owner::Set,
        tag: "game",
        fields: &[
            (7, "rebuildto"),
            (6, "board"),
            (5, "sampleof"),
            (4, "romof"),
            (3, "cloneof"),
            (2, "isbios"),
            (1, "sourcefile"),
            (0, "name"),
        ],
    },
    Witness {
        table: "logiqx_release_attribute_positions",
        owner: Owner::Set,
        tag: "release",
        fields: &[
            (4, "default"),
            (3, "date"),
            (2, "language"),
            (1, "region"),
            (0, "name"),
        ],
    },
    Witness {
        table: "logiqx_bios_attribute_positions",
        owner: Owner::Set,
        tag: "biosset",
        fields: &[(2, "default"), (1, "description"), (0, "name")],
    },
    Witness {
        table: "logiqx_rom_attribute_positions",
        owner: Owner::Occurrence,
        tag: "rom",
        fields: &[
            (8, "serial"),
            (7, "date"),
            (6, "status"),
            (5, "merge"),
            (4, "md5"),
            (3, "sha1"),
            (2, "crc"),
            (1, "size"),
            (0, "name"),
        ],
    },
    Witness {
        table: "logiqx_disk_attribute_positions",
        owner: Owner::Occurrence,
        tag: "disk",
        fields: &[
            (4, "status"),
            (3, "merge"),
            (2, "md5"),
            (1, "sha1"),
            (0, "name"),
        ],
    },
    Witness {
        table: "logiqx_sample_attribute_positions",
        owner: Owner::Occurrence,
        tag: "sample",
        fields: &[(0, "name")],
    },
    Witness {
        table: "logiqx_archive_attribute_positions",
        owner: Owner::Set,
        tag: "archive",
        fields: &[(0, "name")],
    },
];

fn position_query(witness: &Witness) -> String {
    let ancestry = match witness.owner {
        Owner::Document => "WHERE positions.snapshot_key=?",
        Owner::Set => {
            "JOIN catalog_sets AS sets USING(set_id) JOIN catalog_set_groups AS groups USING(set_group_id) WHERE groups.snapshot_key=?"
        }
        Owner::Occurrence => {
            "JOIN asset_occurrences AS occurrence USING(occurrence_id) JOIN catalog_sets AS sets ON sets.set_id=occurrence.record_id JOIN catalog_set_groups AS groups USING(set_group_id) WHERE groups.snapshot_key=?"
        }
    };
    format!(
        "SELECT positions.field_kind,positions.source_order,positions.source_line,positions.source_column FROM {} AS positions {ancestry} ORDER BY positions.source_order",
        witness.table
    )
}

// Independent original-text oracle: QName first character, not parser records.
fn qname_location(text: &str, tag_start: usize, field: &str) -> TestResult<(i64, i64)> {
    let tag = &text[tag_start..];
    let marker = format!("{field}=");
    let relative = tag
        .match_indices(&marker)
        .find(|(offset, _)| {
            *offset == 0
                || tag[..*offset]
                    .chars()
                    .next_back()
                    .is_some_and(char::is_whitespace)
        })
        .map(|(offset, _)| offset)
        .ok_or("fixture unqualified attribute missing")?;
    let prefix = &text[..tag_start + relative];
    let line = i64::try_from(prefix.bytes().filter(|byte| *byte == b'\n').count())? + 1;
    let column = i64::try_from(
        prefix
            .rsplit('\n')
            .next()
            .ok_or("fixture line missing")?
            .chars()
            .count(),
    )? + 1;
    Ok((line, column))
}

fn assert_all_fields(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
    text: &str,
) -> TestResult {
    let mut declared = 0;
    for witness in WITNESSES {
        let rows = sql_query(position_query(&witness))
            .bind::<Text, _>(snapshot.as_str())
            .load::<Position>(conn)?;
        assert_eq!(rows.len(), witness.fields.len(), "{}", witness.tag);
        let start = text
            .find(&format!("<{} ", witness.tag))
            .ok_or("fixture owner missing")?;
        let gap = if witness.tag == "datafile" { 2 } else { 1 };
        for (index, (row, (code, field))) in rows.iter().zip(witness.fields).enumerate() {
            let (line, column) = qname_location(text, start, field)?;
            assert_eq!(
                (
                    row.field_kind,
                    row.source_order,
                    row.source_line,
                    row.source_column
                ),
                (*code, i64::try_from(index)? + gap, line, column),
                "{} {field}",
                witness.tag
            );
            if *code != 8 || witness.tag != "rom" {
                declared += 1;
            }
        }
    }
    assert_eq!(
        declared, 44,
        "compatibility serial is not one of the 44 DTD attributes"
    );
    let device = Witness {
        table: "logiqx_device_reference_attribute_positions",
        owner: Owner::Set,
        tag: "device_ref",
        fields: &[(0, "name")],
    };
    let rows = sql_query(position_query(&device))
        .bind::<Text, _>(snapshot.as_str())
        .load::<Position>(conn)?;
    assert_eq!(rows.len(), 2);
    for (index, row) in rows.iter().enumerate() {
        let (start, _) = text
            .match_indices("<device_ref ")
            .nth(index)
            .ok_or("device fixture missing")?;
        let tag = &text[start..];
        let field_start = start
            + tag
                .find(" name=")
                .ok_or("unqualified device name missing")?
            + 1;
        let (line, column) = qname_location(text, field_start, "name")?;
        assert_eq!(
            (
                row.field_kind,
                row.source_order,
                row.source_line,
                row.source_column
            ),
            (0, i64::try_from(index)? + 1, line, column)
        );
    }
    Ok(())
}

#[derive(QueryableByName)]
struct Id {
    #[diesel(sql_type=BigInt)]
    occurrence_id: i64,
}

fn ids(conn: &mut SqliteConnection, snapshot: &SnapshotKey) -> TestResult<Vec<OccurrenceId>> {
    Ok(sql_query("SELECT occurrence_id FROM catalog_set_groups AS groups JOIN catalog_sets AS sets USING(set_group_id) JOIN asset_occurrences AS occurrence ON occurrence.record_id=sets.set_id WHERE groups.snapshot_key=? ORDER BY occurrence_id").bind::<Text,_>(snapshot.as_str()).load::<Id>(conn)?.into_iter().map(|row|OccurrenceId::from_database(row.occurrence_id)).collect())
}

#[test]
fn all_dtd_attributes_and_named_compatibility_keep_qnames_in_utf8_and_utf16() -> TestResult {
    let catalog = Catalog::new()?;
    let mut requested = Vec::new();
    for encoding in ["UTF-8", "UTF-16", "UTF-8-BOM"] {
        let text = if encoding == "UTF-8-BOM" {
            BODY.to_owned()
        } else {
            format!("<?xml version='1.0' encoding='{encoding}'?>\r\n{BODY}")
        };
        let bytes = if encoding == "UTF-8" {
            text.as_bytes().to_vec()
        } else if encoding == "UTF-8-BOM" {
            let mut bytes = vec![0xef, 0xbb, 0xbf];
            bytes.extend(text.as_bytes());
            bytes
        } else {
            let mut bytes = vec![0xff, 0xfe];
            bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
            bytes
        };
        let snapshot = catalog.import(encoding, &bytes)?;
        assert_eq!(
            app::load_snapshot_source(&catalog.database, &snapshot)?,
            bytes
        );
        let mut conn = catalog.connection()?;
        assert_all_fields(&mut conn, &snapshot, &text)?;
        requested.extend(ids(&mut conn, &snapshot)?);
    }
    catalog.import(
        "unrequested",
        b"<datafile><game name='outside'><rom name='outside.bin'/></game></datafile>",
    )?;
    let payloads = catalog_files::occurrences_for_ids(&catalog.database, &requested)?;
    assert_eq!(payloads.len(), 9);
    assert_eq!(
        payloads
            .iter()
            .map(|file| file.occurrence_id)
            .collect::<Vec<_>>(),
        requested
    );
    for file in payloads {
        match file.logiqx_file.ok_or("missing Logiqx payload")? {
            LogiqxFilePayload::Rom(rom) => {
                assert_eq!(rom.size_text.as_deref(), Some("0001"));
                assert_eq!(rom.compatibility_serial.as_deref(), Some(""));
                assert_eq!(rom.attribute_positions.len(), 9);
                assert!(rom.status_was_present);
            }
            LogiqxFilePayload::Disk(disk) => {
                assert_eq!(disk.attribute_positions.len(), 5);
                assert!(disk.status_was_present);
            }
            LogiqxFilePayload::Sample(sample) => assert_eq!(sample.attribute_positions.len(), 1),
        }
    }
    Ok(())
}

#[test]
fn positioned_native_results_survive_backup_and_do_not_read_originals() -> TestResult {
    let catalog = Catalog::new()?;
    let before = catalog.import("before", BODY.as_bytes())?;
    let after = catalog.import(
        "after",
        BODY.replace("debug='no' build=''", "build='' debug='no'")
            .as_bytes(),
    )?;
    let mut conn = catalog.connection()?;
    let requested = ids(&mut conn, &after)?;
    let files = catalog_files::occurrences_for_ids(&catalog.database, &requested)?;
    let diff = app::diff_catalog_snapshots(&catalog.database, &before, &after)?;
    let explanations = app::explain_relationships(&catalog.database)?;
    assert_eq!(
        explanations.len(),
        14,
        "seven literal relationship owners per edition"
    );
    assert_relationship_qnames(&explanations)?;
    assert!(diff.document_metadata_changed);
    let backup = Utf8PathBuf::try_from(catalog.directory.path().join("backup.sqlite"))?;
    let restored_path = Utf8PathBuf::try_from(catalog.directory.path().join("restored.sqlite"))?;
    mame_coalesce::create_backup(&catalog.path, &backup)?;
    mame_coalesce::restore_backup(
        &backup,
        &restored_path,
        mame_coalesce::RestorePolicy::CreateNew,
    )?;
    assert!(mame_coalesce::check_integrity(&restored_path)?.is_clean());
    let restored = Database::open(&restored_path)?;
    assert_eq!(
        catalog_files::occurrences_for_ids(&restored, &requested)?,
        files
    );
    assert_eq!(
        app::diff_catalog_snapshots(&restored, &before, &after)?,
        diff
    );
    assert_eq!(app::explain_relationships(&restored)?, explanations);
    let originals = Utf8PathBuf::from(format!("{restored_path}.documents"));
    let unavailable = Utf8PathBuf::try_from(
        catalog
            .directory
            .path()
            .join("temporarily-unavailable-originals"),
    )?;
    std::fs::rename(&originals, &unavailable)?;
    assert!(app::load_snapshot_source(&restored, &after).is_err());
    assert_eq!(
        catalog_files::occurrences_for_ids(&restored, &requested)?,
        files
    );
    assert_eq!(
        app::diff_catalog_snapshots(&restored, &before, &after)?,
        diff
    );
    assert_eq!(app::explain_relationships(&restored)?, explanations);
    std::fs::rename(&unavailable, &originals)?;
    Ok(())
}

fn assert_relationship_qnames(
    explanations: &[mame_coalesce::domain::RelationshipExplanation],
) -> TestResult {
    let mut expected = Vec::new();
    for (tag, field, wire) in [
        ("game", "cloneof", "cloneof"),
        ("game", "romof", "romof"),
        ("game", "sampleof", "sampleof"),
        ("rom", "merge", "merge"),
        ("disk", "merge", "merge"),
    ] {
        let start = BODY
            .find(&format!("<{tag} "))
            .ok_or("relationship fixture missing")?;
        let (line, column) = qname_location(BODY, start, wire)?;
        expected.extend([
            (field.to_owned(), line, column),
            (field.to_owned(), line, column),
        ]);
    }
    for (start, _) in BODY.match_indices("<device_ref ") {
        let name = start + BODY[start..].find(" name=").ok_or("device name missing")? + 1;
        let (line, column) = qname_location(BODY, name, "name")?;
        expected.extend([
            ("device_ref".to_owned(), line, column),
            ("device_ref".to_owned(), line, column),
        ]);
    }
    let mut actual = explanations
        .iter()
        .map(|explanation| {
            let field = explanation
                .source_field
                .clone()
                .ok_or("source field missing")?;
            let location = explanation
                .source_location
                .ok_or("source QName location missing")?;
            Ok((field, location.line, location.column))
        })
        .collect::<TestResult<Vec<_>>>()?;
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
    Ok(())
}

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type=BigInt)]
    value: i64,
}

fn native_counts(conn: &mut SqliteConnection) -> TestResult<Vec<(&'static str, i64)>> {
    let mut result = Vec::new();
    for table in [
        "catalog_snapshots",
        "snapshot_publications",
        "catalog_set_groups",
        "catalog_sets",
        "asset_occurrences",
        "catalog_relationships",
        "reported_catalog_relationships",
        "digest_values",
        "catalog_contents",
        "logiqx_document_facts",
        "logiqx_clrmamepro_options",
        "logiqx_romcenter_options",
        "logiqx_games",
        "logiqx_releases",
        "logiqx_bios_sets",
        "logiqx_archive_references",
        "logiqx_set_links",
        "logiqx_device_references",
        "logiqx_file_merges",
        "logiqx_rom_claims",
        "logiqx_disk_claims",
        "logiqx_sample_claims",
    ] {
        let row = sql_query(format!("SELECT count(*) AS value FROM {table}"))
            .get_result::<Count>(conn)?;
        result.push((table, row.value));
    }
    for witness in WITNESSES {
        let row = sql_query(format!("SELECT count(*) AS value FROM {}", witness.table))
            .get_result::<Count>(conn)?;
        result.push((witness.table, row.value));
    }
    let row =
        sql_query("SELECT count(*) AS value FROM logiqx_device_reference_attribute_positions")
            .get_result::<Count>(conn)?;
    result.push(("logiqx_device_reference_attribute_positions", row.value));
    Ok(result)
}

#[test]
fn positioned_sql_failure_rolls_back_native_owners_relationships_and_publication() -> TestResult {
    use diesel::connection::SimpleConnection;
    let catalog = Catalog::new()?;
    let seed = catalog.import("seed", b"<datafile><game name='seed'/></datafile>")?;
    let mut conn = catalog.connection()?;
    let before = native_counts(&mut conn)?;
    // The failure can fire only after the actual ROM name position exists.
    conn.batch_execute("CREATE TRIGGER injected_after_name_position BEFORE INSERT ON logiqx_rom_attribute_positions WHEN NEW.field_kind=1 AND EXISTS(SELECT 1 FROM logiqx_rom_attribute_positions WHERE occurrence_id=NEW.occurrence_id AND field_kind=0) BEGIN SELECT RAISE(ABORT,'injected after native QName position'); END;")?;
    let source = "<datafile build='failure'><game name='failure' cloneof='seed'><rom name='failure.bin' size='2' crc='12345678'/></game></datafile>";
    let error = app::import_catalog(
        &catalog.database,
        &catalog.request("failure", source.as_bytes())?,
    )
    .err()
    .ok_or("fault injection did not fail")?;
    assert!(
        error
            .to_string()
            .contains("injected after native QName position"),
        "{error}"
    );
    assert_eq!(native_counts(&mut conn)?, before);
    assert_eq!(
        app::load_snapshot_source(&catalog.database, &seed)?,
        b"<datafile><game name='seed'/></datafile>"
    );
    conn.batch_execute("DROP TRIGGER injected_after_name_position;")?;
    catalog.import("retry", source.as_bytes())?;
    assert_eq!(
        sql_query("SELECT count(*) AS value FROM snapshot_publications")
            .get_result::<Count>(&mut conn)?
            .value,
        2
    );
    Ok(())
}

#[test]
fn malformed_late_eof_keeps_previous_native_publication_unchanged() -> TestResult {
    let catalog = Catalog::new()?;
    let seed = catalog.import("seed", b"<datafile><game name='seed'/></datafile>")?;
    let mut conn = catalog.connection()?;
    let before = native_counts(&mut conn)?;
    let truncated = BODY
        .strip_suffix("</datafile>")
        .ok_or("fixture closing tag missing")?;
    let report = app::import_catalog(
        &catalog.database,
        &catalog.request("truncated", truncated.as_bytes())?,
    )?;
    assert_eq!(report.status, app::CatalogImportStatus::Failed);
    assert!(report.snapshot_key.is_none());
    assert_eq!(native_counts(&mut conn)?, before);
    assert_eq!(
        app::load_snapshot_source(&catalog.database, &seed)?,
        b"<datafile><game name='seed'/></datafile>"
    );
    Ok(())
}

#[derive(QueryableByName)]
struct DefaultField {
    #[diesel(sql_type=Text)]
    family: String,
    #[diesel(sql_type=Text)]
    effective: String,
    #[diesel(sql_type=BigInt)]
    declared: i64,
    #[diesel(sql_type=BigInt)]
    positioned: i64,
}

fn default_fields(
    conn: &mut SqliteConnection,
    snapshot: &SnapshotKey,
) -> TestResult<Vec<DefaultField>> {
    let query="WITH requested(snapshot_key) AS (VALUES(?)), sets AS (SELECT set_id FROM catalog_set_groups JOIN catalog_sets USING(set_group_id) JOIN requested USING(snapshot_key))
        SELECT 'document' AS family,debug AS effective,debug_was_present AS declared,(SELECT count(*) FROM logiqx_document_attribute_positions AS p WHERE p.snapshot_key=d.snapshot_key AND field_kind=1) AS positioned FROM logiqx_document_facts AS d JOIN requested USING(snapshot_key)
        UNION ALL SELECT 'clrmamepro',forcemerging,forcemerging_was_present,(SELECT count(*) FROM logiqx_clrmamepro_attribute_positions AS p WHERE p.snapshot_key=d.snapshot_key AND field_kind=1) FROM logiqx_clrmamepro_options AS d JOIN requested USING(snapshot_key)
        UNION ALL SELECT 'romcenter',rommode,rommode_was_present,(SELECT count(*) FROM logiqx_romcenter_attribute_positions AS p WHERE p.snapshot_key=d.snapshot_key AND field_kind=1) FROM logiqx_romcenter_options AS d JOIN requested USING(snapshot_key)
        UNION ALL SELECT 'game',is_bios,is_bios_was_present,(SELECT count(*) FROM logiqx_game_attribute_positions AS p WHERE p.set_id=d.set_id AND field_kind=2) FROM logiqx_games AS d JOIN sets USING(set_id)
        UNION ALL SELECT 'release',\"default\",default_was_present,(SELECT count(*) FROM logiqx_release_attribute_positions AS p WHERE p.set_id=d.set_id AND p.release_order=d.release_order AND field_kind=4) FROM logiqx_releases AS d JOIN sets USING(set_id)
        UNION ALL SELECT 'bios',is_default,default_was_present,(SELECT count(*) FROM logiqx_bios_attribute_positions AS p WHERE p.set_id=d.set_id AND p.bios_order=d.bios_order AND field_kind=2) FROM logiqx_bios_sets AS d JOIN sets USING(set_id)
        UNION ALL SELECT 'rom',dump_status,status_was_present,(SELECT count(*) FROM logiqx_rom_attribute_positions AS p WHERE p.occurrence_id=d.occurrence_id AND field_kind=6) FROM logiqx_rom_claims AS d JOIN asset_occurrences AS o USING(occurrence_id) JOIN sets ON sets.set_id=o.record_id
        UNION ALL SELECT 'disk',dump_status,status_was_present,(SELECT count(*) FROM logiqx_disk_attribute_positions AS p WHERE p.occurrence_id=d.occurrence_id AND field_kind=4) FROM logiqx_disk_claims AS d JOIN asset_occurrences AS o USING(occurrence_id) JOIN sets ON sets.set_id=o.record_id ORDER BY family";
    Ok(sql_query(query)
        .bind::<Text, _>(snapshot.as_str())
        .load::<DefaultField>(conn)?)
}

#[test]
fn omitted_defaults_have_no_positions_but_explicit_defaults_and_empty_text_do() -> TestResult {
    let catalog = Catalog::new()?;
    let omitted=catalog.import("omitted",b"<datafile><header><name>Defaults</name><clrmamepro/><romcenter/></header><game name='same'><release name='World' region='US'/><biosset name='base' description=''/><rom name='same.bin'/><disk name='same.chd'/></game></datafile>")?;
    let explicit=catalog.import("explicit",b"<datafile debug='no' build=''><header><name>Defaults</name><clrmamepro forcemerging='split' header=''/><romcenter rommode='split' plugin=''/></header><game name='same' isbios='no' board=''><release name='World' region='US' default='no'/><biosset name='base' description='' default='no'/><rom name='same.bin' status='good' date=''/><disk name='same.chd' status='good'/></game></datafile>")?;
    let mut conn = catalog.connection()?;
    let before = default_fields(&mut conn, &omitted)?;
    let after = default_fields(&mut conn, &explicit)?;
    assert_eq!(before.len(), 8);
    assert_eq!(after.len(), 8);
    for (before, after) in before.iter().zip(&after) {
        assert_eq!(
            (&before.family, &before.effective),
            (&after.family, &after.effective)
        );
        assert_eq!(
            (before.declared, before.positioned),
            (0, 0),
            "{}",
            before.family
        );
        assert_eq!(
            (after.declared, after.positioned),
            (1, 1),
            "{}",
            after.family
        );
    }
    let explicit_roms =
        catalog_files::occurrences_for_ids(&catalog.database, &ids(&mut conn, &explicit)?)?;
    let rom = explicit_roms
        .iter()
        .find_map(|file| match file.logiqx_file.as_ref() {
            Some(LogiqxFilePayload::Rom(rom)) => Some(rom),
            _ => None,
        })
        .ok_or("explicit ROM missing")?;
    assert_eq!(rom.date.as_deref(), Some(""));
    assert!(rom.size_text.is_none());
    assert!(rom.crc_text.is_none());
    assert_eq!(
        rom.attribute_positions
            .iter()
            .map(|position| position.field.as_str())
            .collect::<Vec<_>>(),
        ["name", "status", "date"]
    );
    Ok(())
}

#[derive(QueryableByName)]
struct SqlText {
    #[diesel(sql_type=Text)]
    value: String,
}

#[test]
fn standalone_integrity_detects_missing_value_owner_with_schema_guards_restored() -> TestResult {
    use diesel::connection::SimpleConnection;
    let catalog = Catalog::new()?;
    let snapshot = catalog.import(
        "seed",
        b"<datafile><game name='seed' sourcefile='seed.zip'><rom name='seed.bin'/></game></datafile>",
    )?;
    let occurrence_ids = ids(&mut catalog.connection()?, &snapshot)?;
    drop(catalog.database);
    assert!(mame_coalesce::check_integrity(&catalog.path)?.is_clean());
    let mut conn = SqliteConnection::establish(catalog.path.as_str())?;
    conn.batch_execute("PRAGMA foreign_keys=OFF; PRAGMA recursive_triggers=OFF;")?;
    let guard = sql_query(
        "SELECT sql AS value FROM sqlite_schema WHERE name='logiqx_games_native_immutable_delete'",
    )
    .get_result::<SqlText>(&mut conn)?
    .value;
    conn.batch_execute(
        "DROP TRIGGER logiqx_games_native_immutable_delete; DELETE FROM logiqx_games;",
    )?;
    conn.batch_execute(&guard)?;
    drop(conn);
    let report = mame_coalesce::check_integrity(&catalog.path)?;
    assert!(
        report
            .durable_issues
            .iter()
            .any(|issue| issue.starts_with("Logiqx attributes:")
                && issue.contains("extraneous_or_misplaced_position")),
        "{report:?}"
    );
    assert!(
        !report
            .durable_issues
            .iter()
            .any(|issue| issue.contains("schema validation")),
        "schema mismatch must not mask the semantic witness: {report:?}"
    );
    let database = Database::open(&catalog.path)?;
    assert!(
        catalog_files::occurrences_for_ids(&database, &occurrence_ids).is_err(),
        "bounded payload reads must reject a missing native game"
    );
    Ok(())
}
