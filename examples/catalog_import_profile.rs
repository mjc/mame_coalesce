use std::{env, path::Path};

use camino::Utf8PathBuf;
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let format = parse_format(&args.next().ok_or("missing format")?)?;
    let database_path = Utf8PathBuf::from(args.next().ok_or("missing database path")?);
    let document_paths = args.map(Utf8PathBuf::from).collect::<Vec<_>>();
    if document_paths.is_empty() {
        return Err("provide at least one catalog document".into());
    }

    let database = Database::open(&database_path)?;
    let source_key = PublishingSourceKey::new(format!("profile-{}", format.as_str()));
    let mut succeeded = 0usize;
    let mut failed = 0usize;
    for document_path in document_paths {
        let name = Path::new(document_path.as_str())
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| format!("document path has no UTF-8 file stem: {document_path}"))?
            .to_owned();
        let request = CatalogImportRequest {
            document_path,
            format,
            source_key: source_key.clone(),
            source_display_name: format!("Profiling corpus ({})", format.as_str()),
            catalog_key: CatalogKey::new(format!("profile-{}-{name}", format.as_str())),
            catalog_display_name: format!("Profiling import: {name}"),
            scope: CatalogScope::Complete,
        };
        let report = app::import_catalog(&database, &request)?;
        match report.status {
            CatalogImportStatus::Succeeded => succeeded += 1,
            CatalogImportStatus::Failed => failed += 1,
        }
    }

    println!(
        "format={} succeeded={} failed={}",
        format.as_str(),
        succeeded,
        failed
    );
    Ok(())
}

fn parse_format(value: &str) -> Result<CatalogDocumentFormat, Box<dyn std::error::Error>> {
    match value {
        "logiqx" => Ok(CatalogDocumentFormat::Logiqx(
            mame_coalesce::logiqx::LogiqxMode::ObservedCompatible,
        )),
        "logiqx-dtd15" => Ok(CatalogDocumentFormat::Logiqx(
            mame_coalesce::logiqx::LogiqxMode::StrictDtd15,
        )),
        "machine" => Ok(CatalogDocumentFormat::MameListXml),
        "software-list" => Ok(CatalogDocumentFormat::MameSoftwareListXml),
        "clrmamepro" => Ok(CatalogDocumentFormat::ClrMamePro),
        "no-intro-pc-xml" => Ok(CatalogDocumentFormat::NoIntroPcXml),
        "no-intro-dat-v3-strict" => Ok(CatalogDocumentFormat::NoIntroDat(
            mame_coalesce::NoIntroDatMode::V3Strict,
        )),
        "no-intro-dat-v3-compatible" => Ok(CatalogDocumentFormat::NoIntroDat(
            mame_coalesce::NoIntroDatMode::V3Compatible,
        )),
        "no-intro-dat-v4-strict" => Ok(CatalogDocumentFormat::NoIntroDat(
            mame_coalesce::NoIntroDatMode::V4Strict,
        )),
        "no-intro-dat-v4-compatible" => Ok(CatalogDocumentFormat::NoIntroDat(
            mame_coalesce::NoIntroDatMode::V4Compatible,
        )),
        "no-intro-database-xml-compatible" => Ok(CatalogDocumentFormat::NoIntroDatabase(
            mame_coalesce::no_intro_db_xml::NoIntroDatabaseMode::ObservedCompatible,
        )),
        "no-intro-database-xml-nul-compatible" => Ok(CatalogDocumentFormat::NoIntroDatabase(
            mame_coalesce::no_intro_db_xml::NoIntroDatabaseMode::NullRecoveryCompatible,
        )),
        _ => Err(format!("unsupported catalog format: {value}").into()),
    }
}
