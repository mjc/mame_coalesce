use diesel::{Associations, Insertable, Queryable};

use crate::{
    domain::{ArchiveMemberSelector, ScanProvenance, SourceLocation, SourceObservation},
    hashes::{Sha1Digest, Xxh3Digest},
    storage::schema::rom_files,
};

#[derive(Queryable, Associations, PartialEq, Eq, Debug, Hash)]
#[diesel(table_name = rom_files)]
#[diesel(belongs_to(crate::storage::models::Rom))]
pub struct RomFile {
    pub id: i32,
    pub parent_path: String,
    pub parent_game_name: Option<String>,
    pub path: String,
    pub name: String,
    pub crc: Option<Vec<u8>>,
    pub sha1: Vec<u8>,
    pub md5: Option<Vec<u8>>,
    pub xxhash3: Vec<u8>,
    pub in_archive: bool,
    pub archive_backend: Option<String>,
    pub archive_member_index: Option<i64>,
    pub scan_root: Option<String>,
    pub scan_run: Option<String>,
    pub observed_size: Option<i64>,
    pub source_fingerprint: Option<Vec<u8>>,
    pub scan_provenance: Option<String>,
    pub bare_file_cache_stamp: Option<Vec<u8>>,
    pub cache_reused: bool,
    pub rom_id: Option<i32>,
}

#[derive(Clone, Insertable, Debug)]
#[diesel(table_name = rom_files)]
pub struct New {
    pub parent_path: String,
    pub path: String,
    pub name: String,
    pub crc: Option<[u8; 4]>,
    pub sha1: Sha1Digest,
    pub md5: Option<[u8; 16]>,
    pub xxhash3: Xxh3Digest,
    pub in_archive: bool,
    pub archive_backend: Option<String>,
    pub archive_member_index: Option<i64>,
    pub scan_root: Option<String>,
    pub scan_run: Option<String>,
    pub observed_size: Option<i64>,
    pub source_fingerprint: Option<Vec<u8>>,
    pub scan_provenance: Option<String>,
    pub bare_file_cache_stamp: Option<Vec<u8>>,
    pub cache_reused: bool,
    pub rom_id: Option<i32>,
}

impl New {
    pub fn from_observation(observation: &SourceObservation) -> crate::Result<Self> {
        let path = camino::Utf8Path::new(observation.location.path());
        let parent_path = path.parent().ok_or_else(|| {
            crate::Error::InvalidPath(format!("source path has no parent: {path}"))
        })?;
        let name = observation
            .location
            .member_name()
            .or_else(|| path.file_name())
            .ok_or_else(|| {
                crate::Error::InvalidPath(format!("source location has no name: {path}"))
            })?;
        let sha1 = observation.observed.sha1.ok_or_else(|| {
            crate::Error::InvalidHash(format!("source observation has no SHA1: {name}"))
        })?;
        let size = observation
            .observed
            .size
            .ok_or(crate::Error::InvalidRomSize(0))?;
        let observed_size = i64::try_from(size).map_err(|_| crate::Error::InvalidRomSize(size))?;
        let (in_archive, archive_backend, archive_member_index) = match &observation.location {
            SourceLocation::BareFile { .. } => (false, None, None),
            SourceLocation::ArchiveMember {
                backend,
                selector: ArchiveMemberSelector::IndexAndName { index, .. },
                ..
            } => (
                true,
                Some(backend.storage_key().to_owned()),
                Some(i64::try_from(*index).map_err(|_| {
                    crate::Error::InvalidPath(format!("archive member index is too large: {index}"))
                })?),
            ),
            SourceLocation::LegacyUnknown { .. } => {
                return Err(crate::Error::InvalidPath(
                    "new source observation has legacy-unknown location".to_owned(),
                ));
            }
        };
        if in_archive && observation.bare_file_cache_stamp.is_some() {
            return Err(crate::Error::InvalidPath(
                "archive observation cannot carry a bare-file cache stamp".to_owned(),
            ));
        }

        Ok(Self {
            parent_path: parent_path.as_str().to_owned(),
            path: path.as_str().to_owned(),
            name: name.to_owned(),
            crc: observation.observed.crc.map(|digest| digest.0),
            sha1,
            md5: observation.observed.md5.map(|digest| digest.0),
            xxhash3: observation.observed.xxh3,
            in_archive,
            archive_backend,
            archive_member_index,
            scan_root: Some(observation.source_root.as_str().to_owned()),
            scan_run: Some(observation.scan_run.to_storage_key()),
            observed_size: Some(observed_size),
            source_fingerprint: Some(observation.fingerprint.digest().to_vec()),
            scan_provenance: Some(ScanProvenance::StreamedSha1Xxh3V1.storage_key().to_owned()),
            bare_file_cache_stamp: observation
                .bare_file_cache_stamp
                .map(|stamp| stamp.as_bytes().to_vec()),
            cache_reused: observation.scan_provenance == ScanProvenance::ReusedStatValidatedV1,
            rom_id: None,
        })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }
}
