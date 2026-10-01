use diesel::{
    dsl::sql,
    prelude::*,
    sql_types::{Bool, Text},
};

use crate::{
    domain::{
        BareFileCacheStamp, CompleteSourceScan, EvidenceProvenance, EvidenceScope, ObservedContent,
        ScanProvenance, ScanRunKey, SourceFile, SourceFingerprint, SourceLocation,
        SourcePhysicalPath, SourceRoot,
    },
    hashes::Sha1Digest,
    storage::{
        db::Pool,
        models::{NewRomFile, RomFile},
        schema,
    },
};

pub struct SourceRepository<'pool> {
    pool: &'pool Pool,
}

impl<'pool> SourceRepository<'pool> {
    #[must_use]
    pub const fn new(pool: &'pool Pool) -> Self {
        Self { pool }
    }

    pub fn replace_completed_scan(&self, scan: &CompleteSourceScan) -> crate::Result<usize> {
        let rows = scan
            .observations()
            .iter()
            .map(NewRomFile::from_observation)
            .collect::<crate::Result<Vec<_>>>()?;
        crate::storage::db::replace_rom_files_for_source_root(
            self.pool,
            camino::Utf8Path::new(scan.source_root().as_str()),
            &rows,
        )
    }

    pub fn replace_completed_scans(
        &self,
        scans: &[CompleteSourceScan],
    ) -> crate::Result<Vec<usize>> {
        let rows = scans
            .iter()
            .map(|scan| {
                scan.observations()
                    .iter()
                    .map(NewRomFile::from_observation)
                    .collect::<crate::Result<Vec<_>>>()
                    .map(|rows| (scan.source_root().as_str().to_owned(), rows))
            })
            .collect::<crate::Result<Vec<_>>>()?;
        crate::storage::db::replace_rom_files_for_source_roots(self.pool, &rows)
    }

    pub fn load_source_files_for_root(
        &self,
        source_root: &SourceRoot,
    ) -> crate::Result<Vec<SourceFile>> {
        self.load_source_files_for_roots(std::slice::from_ref(source_root))
    }

    pub fn load_source_files_for_roots(
        &self,
        source_roots: &[SourceRoot],
    ) -> crate::Result<Vec<SourceFile>> {
        let mut conn = self.pool.get()?;
        let mut files = Vec::new();
        for source_root in source_roots {
            let root = source_root.as_str();
            let mut escaped_root = String::with_capacity(root.len());
            for character in root.chars() {
                match character {
                    '*' => escaped_root.push_str("[*]"),
                    '?' => escaped_root.push_str("[?]"),
                    '[' => escaped_root.push_str("[[]"),
                    ']' => escaped_root.push_str("[]]"),
                    _ => escaped_root.push(character),
                }
            }
            let pattern = if root == "/" {
                "/*".to_owned()
            } else {
                format!("{escaped_root}/*")
            };
            let within_root =
                schema::rom_files::dsl::scan_root
                    .eq(root)
                    .or(schema::rom_files::dsl::scan_root.is_null().and(
                        schema::rom_files::dsl::path
                            .eq(root)
                            .or(sql::<Bool>("path GLOB ").bind::<Text, _>(pattern)),
                    ));
            files.extend(
                schema::rom_files::dsl::rom_files
                    .filter(within_root)
                    .load::<RomFile>(&mut conn)?
                    .into_iter()
                    .map(source_file_from_model)
                    .collect::<crate::Result<Vec<_>>>()?,
            );
        }
        Ok(files)
    }
}

fn source_file_from_model(rom_file: RomFile) -> crate::Result<SourceFile> {
    let location = source_location_from_model(&rom_file)?;
    let physical_path = rom_file.physical_path.as_ref().map_or_else(
        || SourcePhysicalPath::from_location(&location),
        |path| SourcePhysicalPath::from_storage(path.clone()),
    );
    let scan_run = scan_run_from_model(&rom_file)?;
    let scan_provenance = scan_provenance_from_model(&rom_file)?;
    let sha1 = sha1_digest_from_db(rom_file.sha1, "rom_files.sha1", &rom_file.name)?;
    let xxh3 = digest_from_db::<8>(Some(rom_file.xxhash3), "rom_files.xxhash3", &rom_file.name)?
        .ok_or_else(|| {
            crate::Error::InvalidHash(format!(
                "rom_files.xxhash3 for {} is missing",
                rom_file.name
            ))
        })?;
    let source_size = rom_file
        .observed_size
        .map(|size| {
            u64::try_from(size).map_err(|_| crate::Error::InvalidRomSize(size.unsigned_abs()))
        })
        .transpose()?;
    let fingerprint = digest_from_db::<20>(
        rom_file.source_fingerprint,
        "rom_files.source_fingerprint",
        &rom_file.name,
    )?
    .map(SourceFingerprint::new);
    let bare_file_cache_stamp = digest_from_db::<32>(
        rom_file.bare_file_cache_stamp,
        "rom_files.bare_file_cache_stamp",
        &rom_file.name,
    )?
    .map(BareFileCacheStamp::new);
    if rom_file.scan_root.is_some() != scan_run.is_some()
        || scan_run.is_some() != fingerprint.is_some()
        || fingerprint.is_some() != scan_provenance.is_some()
    {
        return Err(crate::Error::InvalidPath(format!(
            "source {} has incomplete scan provenance",
            rom_file.path
        )));
    }
    if matches!(
        &location,
        SourceLocation::ArchiveMember { .. } | SourceLocation::LegacyUnknown { .. }
    ) && bare_file_cache_stamp.is_some()
    {
        return Err(crate::Error::InvalidPath(format!(
            "archive source {} has a bare-file cache stamp",
            rom_file.path
        )));
    }
    if scan_provenance == Some(ScanProvenance::ReusedStatValidatedV1)
        && bare_file_cache_stamp.is_none()
    {
        return Err(crate::Error::InvalidPath(format!(
            "reused source {} has no bare-file cache stamp",
            rom_file.path
        )));
    }
    let content_provenance = match scan_provenance {
        Some(ScanProvenance::StreamedSha1Xxh3V1) => EvidenceProvenance::Computed,
        Some(ScanProvenance::ReusedStatValidatedV1) => EvidenceProvenance::StatValidatedCache,
        None => EvidenceProvenance::Unknown,
    };
    Ok(SourceFile {
        source_root: SourceRoot::new(rom_file.scan_root.unwrap_or(rom_file.parent_path)),
        location,
        physical_path,
        observed: ObservedContent {
            scope: EvidenceScope::WholeAsset,
            provenance: content_provenance,
            size: source_size,
            crc: None,
            md5: None,
            sha1: Some(sha1),
            xxh3,
        },
        fingerprint,
        scan_run,
        scan_provenance,
        bare_file_cache_stamp,
    })
}

fn scan_run_from_model(rom_file: &RomFile) -> crate::Result<Option<ScanRunKey>> {
    rom_file
        .scan_run
        .as_deref()
        .map(|key| {
            ScanRunKey::from_storage_key(key).ok_or_else(|| {
                crate::Error::InvalidPath(format!(
                    "source {} has an invalid scan run key",
                    rom_file.path
                ))
            })
        })
        .transpose()
}

fn scan_provenance_from_model(rom_file: &RomFile) -> crate::Result<Option<ScanProvenance>> {
    let provenance = rom_file
        .scan_provenance
        .as_deref()
        .map(|value| {
            ScanProvenance::from_storage_key(value).ok_or_else(|| {
                crate::Error::InvalidPath(format!(
                    "source {} has unknown scan provenance",
                    rom_file.path
                ))
            })
        })
        .transpose()?;
    if rom_file.cache_reused {
        if provenance != Some(ScanProvenance::StreamedSha1Xxh3V1) {
            return Err(crate::Error::InvalidPath(format!(
                "source {} has inconsistent reused-scan metadata",
                rom_file.path
            )));
        }
        Ok(Some(ScanProvenance::ReusedStatValidatedV1))
    } else {
        Ok(provenance)
    }
}

fn source_location_from_model(rom_file: &RomFile) -> crate::Result<SourceLocation> {
    if !rom_file.in_archive {
        if rom_file.archive_backend.is_some() || rom_file.archive_member_index.is_some() {
            return Err(crate::Error::InvalidPath(format!(
                "bare source {} has archive member identity",
                rom_file.path
            )));
        }
        return Ok(SourceLocation::BareFile {
            path: rom_file.path.clone(),
        });
    }
    match (&rom_file.archive_backend, rom_file.archive_member_index) {
        (Some(backend), Some(index)) => {
            let backend =
                crate::domain::ArchiveBackend::from_storage_key(backend).ok_or_else(|| {
                    crate::Error::InvalidPath(format!(
                        "archive source {} has unknown backend {backend:?}",
                        rom_file.path
                    ))
                })?;
            let index = u64::try_from(index).map_err(|_| {
                crate::Error::InvalidPath(format!(
                    "archive source {} has negative member index {index}",
                    rom_file.path
                ))
            })?;
            Ok(SourceLocation::ArchiveMember {
                path: rom_file.path.clone(),
                backend,
                selector: crate::domain::ArchiveMemberSelector::IndexAndName {
                    index,
                    name: rom_file.name.clone(),
                },
            })
        }
        (None, None) => Ok(SourceLocation::LegacyUnknown {
            path: rom_file.path.clone(),
            member_name: Some(rom_file.name.clone()),
        }),
        _ => Err(crate::Error::InvalidPath(format!(
            "archive source {} has incomplete member identity",
            rom_file.path
        ))),
    }
}

fn sha1_digest_from_db(bytes: Vec<u8>, column: &str, label: &str) -> crate::Result<Sha1Digest> {
    digest_from_db::<20>(Some(bytes), column, label)?
        .ok_or_else(|| crate::Error::InvalidHash(format!("{column} for {label} is missing")))
}

fn digest_from_db<const N: usize>(
    bytes: Option<Vec<u8>>,
    column: &str,
    label: &str,
) -> crate::Result<Option<[u8; N]>> {
    bytes.map_or(Ok(None), |bytes| {
        let len = bytes.len();
        bytes.try_into().map(Some).map_err(|_| {
            crate::Error::InvalidHash(format!(
                "{column} for {label} has length {len}; expected {N} bytes"
            ))
        })
    })
}

#[cfg(test)]
mod tests;
