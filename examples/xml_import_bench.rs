use std::{env, time::Instant};

use camino::Utf8PathBuf;
use mame_coalesce::{
    app::{self, CatalogDocumentFormat, CatalogImportRequest, CatalogImportStatus},
    database::Database,
    domain::{CatalogKey, CatalogScope, PublishingSourceKey},
};
use stats_alloc::{INSTRUMENTED_SYSTEM, Region};

#[global_allocator]
static GLOBAL: &stats_alloc::StatsAlloc<std::alloc::System> = &INSTRUMENTED_SYSTEM;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let format = parse_format(&args.next().ok_or("missing format")?)?;
    let database_path = Utf8PathBuf::from(args.next().ok_or("missing database path")?);
    let document_path = Utf8PathBuf::from(args.next().ok_or("missing document path")?);
    if args.next().is_some() {
        return Err("too many arguments".into());
    }

    let database = Database::open(&database_path)?;
    let request = CatalogImportRequest {
        document_path,
        format,
        source_key: PublishingSourceKey::new("xml-import-benchmark"),
        source_display_name: "XML import benchmark".into(),
        catalog_key: CatalogKey::new(format!("xml-import-{}", format.as_str())),
        catalog_display_name: format!("XML import benchmark ({})", format.as_str()),
        scope: CatalogScope::Complete,
    };

    let allocations = Region::new(&INSTRUMENTED_SYSTEM);
    let started = Instant::now();
    let report = app::import_catalog(&database, &request)?;
    let elapsed = started.elapsed();
    let stats = allocations.change();
    if report.status != CatalogImportStatus::Succeeded {
        return Err(format!("catalog import failed: {report:?}").into());
    }
    println!(
        "format={} elapsed_ns={} allocations={} reallocations={} bytes_allocated={} bytes_reallocated={}",
        format.as_str(),
        elapsed.as_nanos(),
        stats.allocations,
        stats.reallocations,
        stats.bytes_allocated,
        stats.bytes_reallocated,
    );
    Ok(())
}

fn parse_format(value: &str) -> Result<CatalogDocumentFormat, Box<dyn std::error::Error>> {
    match value {
        "logiqx" => Ok(CatalogDocumentFormat::Logiqx),
        "machine" => Ok(CatalogDocumentFormat::MameListXml),
        "software-list" => Ok(CatalogDocumentFormat::MameSoftwareListXml),
        _ => Err(format!("unsupported benchmark format: {value}").into()),
    }
}
