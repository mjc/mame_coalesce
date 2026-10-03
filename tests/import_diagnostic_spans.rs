use camino::Utf8PathBuf;
use diesel::{
    prelude::*,
    sql_query,
    sql_types::{BigInt, Binary, Nullable, Text},
};
use flate2::{Compression, write::GzEncoder};
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};
use std::io::Write;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(QueryableByName)]
struct Diagnostic {
    #[diesel(sql_type = Binary)]
    source_excerpt: Vec<u8>,
    #[diesel(sql_type = Text)]
    excerpt_view: String,
    #[diesel(sql_type = BigInt)]
    excerpt_start_byte: i64,
    #[diesel(sql_type = BigInt)]
    problem_start_byte: i64,
    #[diesel(sql_type = BigInt)]
    problem_end_byte: i64,
    #[diesel(sql_type = Nullable<BigInt>)]
    original_problem_start_byte: Option<i64>,
    #[diesel(sql_type = Nullable<BigInt>)]
    original_problem_end_byte: Option<i64>,
}

fn failed_excerpt(bytes: &[u8]) -> TestResult<Diagnostic> {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::from_path_buf(directory.path().join("catalog.sqlite"))
        .map_err(|_| "UTF-8 database path")?;
    let database = Database::open(&database_path)?;
    let path = directory.path().join("catalog.xml");
    std::fs::write(&path, bytes)?;
    let request = CatalogImportRequest {
        document_path: Utf8PathBuf::from_path_buf(path).map_err(|_| "UTF-8 path")?,
        format: CatalogDocumentFormat::Logiqx(
            mame_coalesce::logiqx::LogiqxMode::ObservedCompatible,
        ),
        source_key: PublishingSourceKey::new("diagnostic-source"),
        source_display_name: "Diagnostic source".into(),
        catalog_key: CatalogKey::new("diagnostic-catalog"),
        catalog_display_name: "Diagnostic catalog".into(),
        scope: CatalogScope::Complete,
    };
    let report = app::import_catalog(&database, &request)?;
    assert_eq!(report.status, CatalogImportStatus::Failed);
    assert!(report.snapshot_key.is_none());
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    let diagnostic = sql_query("SELECT d.source_excerpt, d.excerpt_view, d.excerpt_start_byte, d.problem_start_byte, d.problem_end_byte, d.original_problem_start_byte, d.original_problem_end_byte FROM import_diagnostics d JOIN import_runs r ON r.run_key = d.run_key AND r.document_key = d.document_key JOIN documents doc ON doc.document_key = d.document_key WHERE d.run_key = ? AND doc.retention_status = 'retained'")
        .bind::<Text, _>(report.run_key.to_string())
        .get_result::<Diagnostic>(&mut connection)?;
    let snapshots: i64 = sql_query("SELECT COUNT(*) AS count FROM catalog_snapshots")
        .get_result::<Count>(&mut connection)?
        .count;
    assert_eq!(
        snapshots, 0,
        "failed facts must roll back, diagnostic and document survive"
    );
    Ok(diagnostic)
}

#[derive(QueryableByName)]
struct Count {
    #[diesel(sql_type = BigInt)]
    count: i64,
}

#[derive(QueryableByName)]
struct Coordinates {
    #[diesel(sql_type = Nullable<Text>)]
    coordinate_view: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    column_convention: Option<String>,
    #[diesel(sql_type = Nullable<Binary>)]
    source_excerpt: Option<Vec<u8>>,
}

#[test]
fn forbidden_unicode_is_highlighted_inside_the_persisted_raw_excerpt() -> TestResult {
    let prefix = "<datafile>\n<!-- é😀";
    let xml = format!("{prefix}\u{fffe} --></datafile>");
    let diagnostic = failed_excerpt(xml.as_bytes())?;
    assert_eq!(diagnostic.source_excerpt, "\u{fffe}".as_bytes());
    assert_eq!(diagnostic.excerpt_view, "retained_original_bytes");
    assert_eq!(diagnostic.excerpt_start_byte, i64::try_from(prefix.len())?);
    assert_eq!(
        (diagnostic.problem_start_byte, diagnostic.problem_end_byte),
        (0, 3)
    );
    assert_eq!(
        diagnostic.original_problem_start_byte,
        Some(i64::try_from(prefix.len())?)
    );
    assert_eq!(
        diagnostic.original_problem_end_byte,
        Some(i64::try_from(prefix.len() + 3)?)
    );
    Ok(())
}

#[test]
fn utf16_highlights_encoded_bytes_after_a_surrogate_pair() -> TestResult {
    let prefix = "<datafile>\n<!-- é😀";
    let xml = format!("{prefix}\0 --></datafile>");
    for little_endian in [true, false] {
        let mut bytes = if little_endian {
            vec![0xff, 0xfe]
        } else {
            vec![0xfe, 0xff]
        };
        for unit in xml.encode_utf16() {
            bytes.extend(if little_endian {
                unit.to_le_bytes()
            } else {
                unit.to_be_bytes()
            });
        }
        let diagnostic = failed_excerpt(&bytes)?;
        let offset = i64::try_from(2 + prefix.encode_utf16().count() * 2)?;
        assert_eq!(diagnostic.source_excerpt, [0, 0]);
        assert_eq!(diagnostic.excerpt_view, "retained_original_bytes");
        assert_eq!(diagnostic.excerpt_start_byte, offset);
        assert_eq!(
            (diagnostic.problem_start_byte, diagnostic.problem_end_byte),
            (0, 2)
        );
        assert_eq!(diagnostic.original_problem_start_byte, Some(offset));
        assert_eq!(diagnostic.original_problem_end_byte, Some(offset + 2));
    }
    Ok(())
}

#[test]
fn gzip_excerpt_offsets_are_xml_offsets_not_compressed_file_offsets() -> TestResult {
    let prefix = "<datafile>\n<!-- é😀";
    let xml = format!("{prefix}\0 --></datafile>");
    for utf16 in [false, true] {
        let bytes = if utf16 {
            let mut bytes = vec![0xff, 0xfe];
            bytes.extend(xml.encode_utf16().flat_map(u16::to_le_bytes));
            bytes
        } else {
            xml.as_bytes().to_vec()
        };
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&bytes)?;
        let diagnostic = failed_excerpt(&encoder.finish()?)?;
        let offset = if utf16 {
            2 + prefix.encode_utf16().count() * 2
        } else {
            prefix.len()
        };
        assert_eq!(
            diagnostic.source_excerpt,
            if utf16 { vec![0, 0] } else { vec![0] }
        );
        assert_eq!(diagnostic.excerpt_view, "transport_decoded_xml_bytes");
        assert_eq!(diagnostic.excerpt_start_byte, i64::try_from(offset)?);
        assert_eq!(diagnostic.problem_start_byte, 0);
        assert_eq!(diagnostic.problem_end_byte, if utf16 { 2 } else { 1 });
        assert_eq!(diagnostic.original_problem_start_byte, None);
        assert_eq!(diagnostic.original_problem_end_byte, None);
    }
    Ok(())
}

#[test]
fn malformed_encoding_retains_the_actual_invalid_bytes() -> TestResult {
    let mut utf8 = b"<datafile><!-- ".to_vec();
    let offset = utf8.len();
    utf8.extend([0xff]);
    utf8.extend(b" --></datafile>");
    let diagnostic = failed_excerpt(&utf8)?;
    assert_eq!(diagnostic.source_excerpt, [0xff]);
    assert_eq!(diagnostic.excerpt_start_byte, i64::try_from(offset)?);
    assert_eq!(
        (diagnostic.problem_start_byte, diagnostic.problem_end_byte),
        (0, 1)
    );
    let mut utf16 = vec![0xff, 0xfe];
    utf16.extend(
        "<datafile><!-- 😀"
            .encode_utf16()
            .flat_map(u16::to_le_bytes),
    );
    let offset = utf16.len();
    utf16.extend([0x00, 0xd8, 0x41, 0x00]);
    let diagnostic = failed_excerpt(&utf16)?;
    assert_eq!(diagnostic.source_excerpt, [0x00, 0xd8]);
    assert_eq!(diagnostic.excerpt_start_byte, i64::try_from(offset)?);
    assert_eq!(
        (diagnostic.problem_start_byte, diagnostic.problem_end_byte),
        (0, 2)
    );
    Ok(())
}

#[test]
fn xml_coordinates_keep_their_convention_without_byte_evidence() -> TestResult {
    let directory = tempfile::tempdir()?;
    let database_path = Utf8PathBuf::try_from(directory.path().join("catalog.sqlite"))?;
    let database = Database::open(&database_path)?;
    let path = Utf8PathBuf::try_from(directory.path().join("catalog.xml"))?;
    std::fs::write(&path, b"<softwarelist/>")?;
    let report = app::import_catalog(
        &database,
        &CatalogImportRequest {
            document_path: path,
            format: CatalogDocumentFormat::MameSoftwareListXml,
            source_key: PublishingSourceKey::new("coordinate-source"),
            source_display_name: "Source".into(),
            catalog_key: CatalogKey::new("coordinate-catalog"),
            catalog_display_name: "Catalog".into(),
            scope: CatalogScope::Complete,
        },
    )?;
    assert_eq!(report.status, CatalogImportStatus::Failed);
    let mut connection = SqliteConnection::establish(database_path.as_str())?;
    let coordinates = sql_query("SELECT coordinate_view, column_convention, source_excerpt FROM import_diagnostics WHERE run_key = ?").bind::<Text, _>(report.run_key.to_string()).get_result::<Coordinates>(&mut connection)?;
    assert_eq!(coordinates.source_excerpt, None);
    assert_eq!(
        coordinates.coordinate_view.as_deref(),
        Some("transport_decoded_xml_text")
    );
    assert_eq!(
        coordinates.column_convention.as_deref(),
        Some("unicode_scalar_1based")
    );
    Ok(())
}
