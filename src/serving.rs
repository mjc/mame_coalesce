//! Verified, snapshot-bound byte-range reads from source locations.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    sync::{Arc, Mutex},
};

#[cfg(test)]
use std::path::Path;

use camino::Utf8Path;
use md5::{Digest as _, Md5};
use sha1::Sha1;
use sha2::Sha256;
use xxhash_rust::xxh3::Xxh3;

use crate::{
    domain::media::VerifiedRepresentationIdentity,
    domain::{
        ArchiveBackend, ContentDigestAlgorithm, ContentIdentity, LogicalEntry, SourceFingerprint,
        SourceLocation, SourcePhysicalPath,
    },
    hashes::{Sha1Digest, Xxh3Digest},
    private_temp::PrivateTempDir,
    resolution::MatchStrength,
};

const MATERIALIZED_FILE_NAME: &str = "verified-content";
const MAX_ACTIVE_SESSIONS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ByteOffset(u64);

impl ByteOffset {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ByteLength(u64);

impl ByteLength {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ByteRange {
    offset: ByteOffset,
    length: ByteLength,
}

impl ByteRange {
    pub fn new(offset: ByteOffset, length: ByteLength) -> Result<Self, ServingError> {
        offset
            .get()
            .checked_add(length.get())
            .ok_or(ServingError::RangeOverflow)?;
        Ok(Self { offset, length })
    }

    #[must_use]
    pub const fn offset(self) -> ByteOffset {
        self.offset
    }

    #[must_use]
    pub const fn length(self) -> ByteLength {
        self.length
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServingIdentity {
    Content(ContentIdentity),
    Representation(VerifiedRepresentationIdentity),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerificationStatus {
    WholeObjectVerified,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ByteRangeResponse {
    pub identity: ServingIdentity,
    pub verification: VerificationStatus,
    pub range: ByteRange,
    pub returned: ByteLength,
    pub total_length: ByteLength,
    pub eof: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessPattern {
    Unavailable,
    CheapRandomAccess,
    SequentialStreaming,
    BoundedMaterialization,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServingCapabilities {
    /// Native backend access mode. Opening a verified session still hashes and
    /// snapshots the entire object before it returns any range.
    pub access_pattern: AccessPattern,
    pub maximum_materialized_bytes: ByteLength,
    pub maximum_member_bytes: Option<ByteLength>,
    pub maximum_archive_entries: Option<usize>,
    pub maximum_prerequisite_member_bytes: Option<ByteLength>,
    pub maximum_prerequisite_output_bytes: Option<ByteLength>,
    pub maximum_concurrent_archive_decoders: Option<usize>,
    pub maximum_decoder_dictionary_bytes: Option<ByteLength>,
    pub maximum_decoder_working_set_bytes: Option<ByteLength>,
    pub maximum_open_sessions: usize,
    pub open_snapshots_entire_object: bool,
}

/// A shared ceiling for all simultaneously open materialized readers.
#[derive(Clone, Debug)]
pub struct MaterializationBudget {
    inner: Arc<Mutex<BudgetState>>,
}

#[derive(Debug)]
struct BudgetState {
    maximum: ByteLength,
    reserved: ByteLength,
    active_sessions: usize,
}

impl MaterializationBudget {
    #[must_use]
    pub fn new(maximum: ByteLength) -> Self {
        Self {
            inner: Arc::new(Mutex::new(BudgetState {
                maximum,
                reserved: ByteLength::new(0),
                active_sessions: 0,
            })),
        }
    }

    pub fn capabilities(&self, source: &LogicalEntry) -> Result<ServingCapabilities, ServingError> {
        let maximum_materialized_bytes = self
            .inner
            .lock()
            .map_err(|_| ServingError::BudgetPoisoned)?
            .maximum;
        let maximum_archive_entries = matches!(
            &source.source.location,
            SourceLocation::ArchiveMember { .. }
        )
        .then_some(crate::sources::MAX_SERVING_ARCHIVE_ENTRIES);
        let (
            access_pattern,
            backend_limit,
            prerequisite_member_limit,
            prerequisite_output_limit,
            decoder_limit,
            dictionary_limit,
            decoder_working_set_limit,
        ) = match &source.source.location {
            SourceLocation::BareFile { .. } => (
                AccessPattern::CheapRandomAccess,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
            SourceLocation::ArchiveMember { backend, .. } => match backend {
                ArchiveBackend::Zip => (
                    AccessPattern::SequentialStreaming,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                ),
                ArchiveBackend::SevenZip | ArchiveBackend::Rar => {
                    let backend_capabilities = crate::sources::capabilities(*backend);
                    let limit = backend_capabilities.max_member_size.map(ByteLength::new);
                    let prerequisite_member_limit = backend_capabilities
                        .max_prerequisite_member_size
                        .map(ByteLength::new);
                    let prerequisite_output_limit = backend_capabilities
                        .max_prerequisite_output_size
                        .map(ByteLength::new);
                    let dictionary_limit = backend_capabilities
                        .max_decoder_dictionary_size
                        .map(ByteLength::new);
                    let decoder_working_set_limit = backend_capabilities
                        .max_decoder_working_set_size
                        .map(ByteLength::new);
                    (
                        AccessPattern::BoundedMaterialization,
                        limit,
                        prerequisite_member_limit,
                        prerequisite_output_limit,
                        backend_capabilities.max_concurrent_decoders,
                        dictionary_limit,
                        decoder_working_set_limit,
                    )
                }
            },
            SourceLocation::LegacyUnknown { .. } => (
                AccessPattern::Unavailable,
                None,
                None,
                None,
                None,
                None,
                None,
            ),
        };
        Ok(ServingCapabilities {
            access_pattern,
            maximum_materialized_bytes,
            maximum_member_bytes: Some(backend_limit.map_or(maximum_materialized_bytes, |limit| {
                ByteLength::new(limit.get().min(maximum_materialized_bytes.get()))
            })),
            maximum_archive_entries,
            maximum_prerequisite_member_bytes: prerequisite_member_limit,
            maximum_prerequisite_output_bytes: prerequisite_output_limit,
            maximum_concurrent_archive_decoders: decoder_limit,
            maximum_decoder_dictionary_bytes: dictionary_limit,
            maximum_decoder_working_set_bytes: decoder_working_set_limit,
            maximum_open_sessions: MAX_ACTIVE_SESSIONS,
            open_snapshots_entire_object: true,
        })
    }

    fn reserve(&self, length: ByteLength) -> Result<BudgetLease, ServingError> {
        let mut state = self
            .inner
            .lock()
            .map_err(|_| ServingError::BudgetPoisoned)?;
        if state.active_sessions >= MAX_ACTIVE_SESSIONS {
            return Err(ServingError::SessionLimitExceeded {
                maximum: MAX_ACTIVE_SESSIONS,
            });
        }
        let Some(reserved) = state.reserved.get().checked_add(length.get()) else {
            return Err(ServingError::BudgetExceeded {
                requested: length,
                available: ByteLength::new(0),
            });
        };
        if reserved > state.maximum.get() {
            return Err(ServingError::BudgetExceeded {
                requested: length,
                available: ByteLength::new(state.maximum.get() - state.reserved.get()),
            });
        }
        state.reserved = ByteLength::new(reserved);
        state.active_sessions += 1;
        drop(state);
        Ok(BudgetLease {
            inner: Arc::clone(&self.inner),
            length,
        })
    }
}

struct BudgetLease {
    inner: Arc<Mutex<BudgetState>>,
    length: ByteLength,
}

impl BudgetLease {
    fn resize(&mut self, length: ByteLength) -> Result<(), ServingError> {
        let additional = length.get().checked_sub(self.length.get()).ok_or_else(|| {
            ServingError::BudgetExceeded {
                requested: length,
                available: ByteLength::new(0),
            }
        })?;
        let mut state = self
            .inner
            .lock()
            .map_err(|_| ServingError::BudgetPoisoned)?;
        let Some(reserved) = state.reserved.get().checked_add(additional) else {
            return Err(ServingError::BudgetExceeded {
                requested: ByteLength::new(additional),
                available: ByteLength::new(0),
            });
        };
        if reserved > state.maximum.get() {
            return Err(ServingError::BudgetExceeded {
                requested: ByteLength::new(additional),
                available: ByteLength::new(state.maximum.get() - state.reserved.get()),
            });
        }
        state.reserved = ByteLength::new(reserved);
        self.length = length;
        drop(state);
        Ok(())
    }
}

impl Drop for BudgetLease {
    fn drop(&mut self) {
        let mut state = match self.inner.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.reserved = ByteLength::new(state.reserved.get() - self.length.get());
        state.active_sessions -= 1;
    }
}

/// A read session exists only after its complete backing object has been verified.
pub struct VerifiedContentSession {
    file: Option<File>,
    _temporary_directory: PrivateTempDir,
    _budget: BudgetLease,
    identity: ServingIdentity,
    total_length: ByteLength,
}

impl VerifiedContentSession {
    pub fn open(
        entry: &LogicalEntry,
        identity: ServingIdentity,
        budget: &MaterializationBudget,
    ) -> Result<Self, ServingError> {
        let mut lease = budget.reserve(ByteLength::new(0))?;
        let pinned_source = PinnedSource::resolve(entry)?;
        let path = pinned_source.physical_path.as_path();
        let length = pinned_source.length;
        lease.resize(length)?;
        let (temporary_directory, file, actual) = pinned_source.materialize(entry, &identity)?;
        verify_observation(entry, actual, path)?;
        verify_selected_evidence(entry, actual, path)?;
        verify_identity(&identity, actual, path)?;

        Ok(Self {
            file: Some(file),
            _temporary_directory: temporary_directory,
            _budget: lease,
            identity,
            total_length: length,
        })
    }

    #[must_use]
    pub const fn total_length(&self) -> ByteLength {
        self.total_length
    }

    #[cfg(test)]
    #[allow(clippy::used_underscore_binding)]
    fn spool_directory(&self) -> &Path {
        self._temporary_directory.path()
    }

    pub fn read_range(
        &mut self,
        range: ByteRange,
        output: &mut [u8],
    ) -> Result<ByteRangeResponse, ServingError> {
        let available = self.total_length.get().saturating_sub(range.offset.get());
        let returned = range.length.get().min(available);
        let returned_usize = usize::try_from(returned)
            .map_err(|_| ServingError::RangeTooLarge(ByteLength::new(returned)))?;
        if output.len() < returned_usize {
            return Err(ServingError::BufferTooSmall {
                requested: ByteLength::new(returned),
                available: ByteLength::new(u64::try_from(output.len()).unwrap_or(u64::MAX)),
            });
        }
        let file = self.file.as_mut().ok_or(ServingError::SessionClosed)?;
        if returned_usize > 0 {
            file.seek(SeekFrom::Start(range.offset.get()))?;
            file.read_exact(&mut output[..returned_usize])?;
        }
        Ok(ByteRangeResponse {
            identity: self.identity.clone(),
            verification: VerificationStatus::WholeObjectVerified,
            range,
            returned: ByteLength::new(returned),
            total_length: self.total_length,
            eof: range.offset.get().saturating_add(returned) >= self.total_length.get(),
        })
    }
}

struct PinnedSource {
    physical_path: SourcePhysicalPath,
    length: ByteLength,
    archive_member: Option<(ArchiveBackend, crate::sources::ArchiveMemberSelector)>,
}

impl PinnedSource {
    fn resolve(entry: &LogicalEntry) -> Result<Self, ServingError> {
        let location = &entry.source.location;
        let original_path = location_path(location);
        let physical_path = SourcePhysicalPath::capture(Utf8Path::new(original_path))?;
        if physical_path != entry.source.physical_path {
            return Err(ServingError::SourceLocationChanged {
                path: original_path.to_owned(),
            });
        }
        let path = physical_path.as_path();
        let (length, archive_member) = match location {
            SourceLocation::BareFile { .. } => {
                if crate::sources::detect(path)? != crate::sources::SourceKind::BareFile {
                    return Err(ServingError::SourceKindChanged {
                        path: path.to_string(),
                    });
                }
                (fs::metadata(path)?.len(), None)
            }
            SourceLocation::ArchiveMember {
                backend, selector, ..
            } => {
                let crate::domain::ArchiveMemberSelector::IndexAndName { index, name } = selector;
                let selector_index = *index;
                let index = usize::try_from(selector_index)
                    .map_err(|_| ServingError::InvalidMemberIndex(selector_index))?;
                let selected = crate::sources::ArchiveMemberSelector {
                    index,
                    name: name.clone(),
                };
                let inventory = crate::sources::enumerate_for_serving(path, *backend)?;
                let member = inventory
                    .iter()
                    .find(|member| member.selector == selected)
                    .ok_or_else(|| ServingError::MemberUnavailable {
                        path: path.to_string(),
                        index: selector_index,
                        name: name.clone(),
                    })?;
                (member.size, Some((*backend, selected)))
            }
            SourceLocation::LegacyUnknown { .. } => {
                return Err(ServingError::UnaddressableLocation {
                    path: original_path.to_owned(),
                });
            }
        };
        if entry
            .source
            .observed
            .size
            .is_some_and(|observed| observed != length)
        {
            return Err(ServingError::SourceChanged {
                path: path.to_string(),
                reason: "selected object length differs from its observation".to_owned(),
            });
        }
        Ok(Self {
            physical_path,
            length: ByteLength::new(length),
            archive_member,
        })
    }

    fn materialize(
        &self,
        entry: &LogicalEntry,
        identity: &ServingIdentity,
    ) -> Result<(PrivateTempDir, File, ContentDigests), ServingError> {
        let path = self.physical_path.as_path();
        let original_path = location_path(&entry.source.location);
        let temporary_directory = PrivateTempDir::create("mame-coalesce-serving-")?;
        let staged_path = temporary_directory.path().join(MATERIALIZED_FILE_NAME);
        let output = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(staged_path)?;
        let mut materializer = ContentMaterializer::new(
            output,
            self.length,
            DigestRequirements::for_entry(entry, identity),
        );

        if let Some((backend, selector)) = &self.archive_member {
            let before = file_fingerprint(path)?;
            if entry
                .source
                .fingerprint
                .is_some_and(|expected| expected != before)
            {
                return Err(ServingError::SourceChanged {
                    path: path.to_string(),
                    reason: "archive fingerprint differs from its observation".to_owned(),
                });
            }
            crate::sources::stream_archive_member_to_writer(
                path,
                *backend,
                selector,
                &mut materializer,
            )?;
            let after = file_fingerprint(path)?;
            if after != before
                || entry
                    .source
                    .fingerprint
                    .is_some_and(|expected| expected != after)
            {
                return Err(ServingError::SourceChanged {
                    path: path.to_string(),
                    reason: "archive changed while its member was read".to_owned(),
                });
            }
        } else {
            let mut input = File::open(path)?;
            io::copy(&mut input, &mut materializer)?;
        }

        let current_path = SourcePhysicalPath::capture(Utf8Path::new(original_path))?;
        if current_path != self.physical_path {
            return Err(ServingError::SourceLocationChanged {
                path: original_path.to_owned(),
            });
        }
        let (file, actual) = materializer.finish()?;
        if actual.size != self.length {
            return Err(ServingError::SourceChanged {
                path: path.to_string(),
                reason: "materialized byte length differs from the pinned location".to_owned(),
            });
        }
        Ok((temporary_directory, file, actual))
    }
}

impl Drop for VerifiedContentSession {
    fn drop(&mut self) {
        self.file.take();
    }
}

struct ContentMaterializer {
    file: File,
    maximum: ByteLength,
    size: u64,
    sha1: Sha1,
    md5: Option<Md5>,
    crc32: Option<crc32fast::Hasher>,
    xxh3: Xxh3,
    sha256: Option<Sha256>,
}

#[derive(Clone, Copy)]
struct DigestRequirements {
    md5: bool,
    crc32: bool,
    sha256: bool,
}

impl DigestRequirements {
    fn for_entry(entry: &LogicalEntry, identity: &ServingIdentity) -> Self {
        let identity_algorithm = match identity {
            ServingIdentity::Content(identity) => Some(identity.algorithm()),
            ServingIdentity::Representation(_) => Some(ContentDigestAlgorithm::Sha256),
        };
        Self {
            md5: entry.source.observed.md5.is_some()
                || entry.expected.md5.is_some()
                || matches!(entry.selection.strength, MatchStrength::Md5)
                || identity_algorithm == Some(ContentDigestAlgorithm::Md5),
            crc32: entry.source.observed.crc.is_some()
                || entry.expected.crc.is_some()
                || matches!(entry.selection.strength, MatchStrength::CrcAndSize)
                || identity_algorithm == Some(ContentDigestAlgorithm::Crc32),
            sha256: identity_algorithm == Some(ContentDigestAlgorithm::Sha256),
        }
    }
}

impl ContentMaterializer {
    fn new(file: File, maximum: ByteLength, requirements: DigestRequirements) -> Self {
        Self {
            file,
            maximum,
            size: 0,
            sha1: Sha1::new(),
            md5: requirements.md5.then(Md5::new),
            crc32: requirements.crc32.then(crc32fast::Hasher::new),
            xxh3: Xxh3::new(),
            sha256: requirements.sha256.then(Sha256::new),
        }
    }

    fn finish(mut self) -> Result<(File, ContentDigests), ServingError> {
        self.file.flush()?;
        let evidence = ContentDigests {
            size: ByteLength::new(self.size),
            sha1: self.sha1.finalize().into(),
            md5: self.md5.map(|digest| digest.finalize().into()),
            crc32: self.crc32.map(crc32fast::Hasher::finalize),
            xxh3: self.xxh3.digest().to_be_bytes(),
            sha256: self.sha256.map(|digest| digest.finalize().into()),
        };
        Ok((self.file, evidence))
    }
}

impl Write for ContentMaterializer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = u64::try_from(bytes.len()).map_err(io::Error::other)?;
        let Some(size) = self.size.checked_add(count) else {
            return Err(io::Error::other("materialized content length overflow"));
        };
        if size > self.maximum.get() {
            return Err(io::Error::other("materialization budget exceeded"));
        }
        self.file.write_all(bytes)?;
        self.sha1.update(bytes);
        if let Some(md5) = &mut self.md5 {
            md5.update(bytes);
        }
        if let Some(crc32) = &mut self.crc32 {
            crc32.update(bytes);
        }
        self.xxh3.update(bytes);
        if let Some(sha256) = &mut self.sha256 {
            sha256.update(bytes);
        }
        self.size = size;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

#[derive(Clone, Copy)]
struct ContentDigests {
    size: ByteLength,
    sha1: Sha1Digest,
    md5: Option<[u8; 16]>,
    crc32: Option<u32>,
    xxh3: Xxh3Digest,
    sha256: Option<[u8; 32]>,
}

fn verify_observation(
    entry: &LogicalEntry,
    actual: ContentDigests,
    path: &Utf8Path,
) -> Result<(), ServingError> {
    let observed = &entry.source.observed;
    if observed.scope != crate::domain::EvidenceScope::WholeAsset
        || observed.size.is_some_and(|size| size != actual.size.get())
        || observed.sha1.is_some_and(|sha1| sha1 != actual.sha1)
        || observed.md5.is_some_and(|md5| Some(md5.0) != actual.md5)
        || observed
            .crc
            .is_some_and(|crc| actual.crc32.map(u32::to_be_bytes) != Some(crc.0))
        || observed.xxh3 != actual.xxh3
        || (matches!(entry.source.location, SourceLocation::BareFile { .. })
            && entry
                .source
                .fingerprint
                .is_some_and(|fingerprint| fingerprint != SourceFingerprint::new(actual.sha1)))
    {
        return Err(ServingError::SourceChanged {
            path: path.to_string(),
            reason: "materialized bytes differ from observed evidence".to_owned(),
        });
    }
    Ok(())
}

fn verify_selected_evidence(
    entry: &LogicalEntry,
    actual: ContentDigests,
    path: &Utf8Path,
) -> Result<(), ServingError> {
    let expected = &entry.expected;
    let strength_matches = match entry.selection.strength {
        MatchStrength::Sha1 => expected.sha1.is_some_and(|sha1| sha1 == actual.sha1),
        MatchStrength::Md5 => expected.md5.is_some_and(|md5| Some(md5.0) == actual.md5),
        MatchStrength::CrcAndSize => {
            expected
                .crc
                .is_some_and(|crc| actual.crc32.map(u32::to_be_bytes) == Some(crc.0))
                && expected.size.is_some_and(|size| size == actual.size.get())
        }
    };
    let contradicts_expected = expected.size.is_some_and(|size| size != actual.size.get())
        || expected.sha1.is_some_and(|sha1| sha1 != actual.sha1)
        || expected.md5.is_some_and(|md5| Some(md5.0) != actual.md5)
        || expected
            .crc
            .is_some_and(|crc| actual.crc32.map(u32::to_be_bytes) != Some(crc.0));
    if expected.scope != crate::domain::EvidenceScope::WholeAsset
        || !strength_matches
        || contradicts_expected
    {
        return Err(ServingError::EvidenceMismatch {
            path: path.to_string(),
        });
    }
    Ok(())
}

fn verify_identity(
    identity: &ServingIdentity,
    actual: ContentDigests,
    path: &Utf8Path,
) -> Result<(), ServingError> {
    let matches = match identity {
        ServingIdentity::Content(expected) => {
            let digest = match expected.algorithm() {
                ContentDigestAlgorithm::Crc32 => actual.crc32.map(|crc| format!("{crc:08x}")),
                ContentDigestAlgorithm::Md5 => actual.md5.map(hex::encode),
                ContentDigestAlgorithm::Sha1 => Some(hex::encode(actual.sha1)),
                ContentDigestAlgorithm::Sha256 => actual.sha256.map(hex::encode),
            };
            digest.as_deref() == Some(expected.digest())
        }
        ServingIdentity::Representation(expected) => actual
            .sha256
            .is_some_and(|sha256| expected.digest().as_bytes() == &sha256),
    };
    if matches {
        Ok(())
    } else {
        Err(ServingError::IdentityMismatch {
            path: path.to_string(),
        })
    }
}

fn location_path(location: &SourceLocation) -> &str {
    match location {
        SourceLocation::BareFile { path }
        | SourceLocation::ArchiveMember { path, .. }
        | SourceLocation::LegacyUnknown { path, .. } => path,
    }
}

fn file_fingerprint(path: &Utf8Path) -> crate::Result<SourceFingerprint> {
    let mut digest = Sha1::new();
    crate::sources::stream_file(path, &mut digest)?;
    Ok(SourceFingerprint::new(digest.finalize().into()))
}

#[derive(Debug, thiserror::Error)]
pub enum ServingError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Source(#[from] crate::Error),
    #[error("byte range offset plus length overflows u64")]
    RangeOverflow,
    #[error("requested range length cannot fit in memory: {0:?}")]
    RangeTooLarge(ByteLength),
    #[error("range buffer holds {available:?}, fewer than requested {requested:?}")]
    BufferTooSmall {
        requested: ByteLength,
        available: ByteLength,
    },
    #[error("materialization budget is poisoned")]
    BudgetPoisoned,
    #[error("materialization budget has {available:?} available; requested {requested:?}")]
    BudgetExceeded {
        requested: ByteLength,
        available: ByteLength,
    },
    #[error("maximum number of open content sessions reached ({maximum})")]
    SessionLimitExceeded { maximum: usize },
    #[error("archive member index does not fit this platform: {0}")]
    InvalidMemberIndex(u64),
    #[error("source location is not addressable: {path}")]
    UnaddressableLocation { path: String },
    #[error("source kind changed at {path}")]
    SourceKindChanged { path: String },
    #[error("source location changed while opening: {path}")]
    SourceLocationChanged { path: String },
    #[error("selected source object is unavailable at {path} (member {index}, {name:?})")]
    MemberUnavailable {
        path: String,
        index: u64,
        name: String,
    },
    #[error("source changed at {path}: {reason}")]
    SourceChanged { path: String, reason: String },
    #[error("source bytes do not satisfy the selected evidence: {path}")]
    EvidenceMismatch { path: String },
    #[error("source bytes do not match the pinned serving identity: {path}")]
    IdentityMismatch { path: String },
    #[error("read session is closed")]
    SessionClosed,
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::{self, Write},
    };

    use camino::{Utf8Path, Utf8PathBuf};
    use zip::{ZipWriter, write::SimpleFileOptions};

    use super::*;
    use crate::{
        domain::{
            CatalogKey, EvidenceProvenance, EvidenceScope, ExpectedEvidence, LogicalPath,
            MatchingPolicy, ObservedContent, RequirementKey, SelectionProvenance, SetKey,
            SourceFile, SourceRoot,
        },
        hashes::{sha1_bytes, xxhash3_bytes},
    };

    fn write_zip(path: &Utf8Path, entries: &[(&str, &[u8])]) -> io::Result<()> {
        let mut zip = ZipWriter::new(File::create(path)?);
        for (name, contents) in entries {
            zip.start_file(*name, SimpleFileOptions::default())?;
            zip.write_all(contents)?;
        }
        zip.finish()?;
        Ok(())
    }

    fn logical_entry(
        path: &Utf8Path,
        contents: &[u8],
        backend: Option<ArchiveBackend>,
        selector: Option<(u64, &str)>,
    ) -> Result<LogicalEntry, Box<dyn std::error::Error>> {
        let location = match (backend, selector) {
            (Some(backend), Some((index, name))) => SourceLocation::ArchiveMember {
                path: path.to_string(),
                backend,
                selector: crate::domain::ArchiveMemberSelector::IndexAndName {
                    index,
                    name: name.to_owned(),
                },
            },
            (None, None) => SourceLocation::BareFile {
                path: path.to_string(),
            },
            _ => return Err(io::Error::other("backend and selector must be paired").into()),
        };
        let file_bytes = fs::read(path)?;
        let source = SourceFile {
            source_root: SourceRoot::new(
                path.parent()
                    .ok_or_else(|| io::Error::other("source has no parent"))?
                    .to_string(),
            ),
            physical_path: SourcePhysicalPath::capture(path)?,
            location,
            observed: ObservedContent {
                scope: EvidenceScope::WholeAsset,
                provenance: EvidenceProvenance::Computed,
                size: Some(u64::try_from(contents.len())?),
                crc: None,
                md5: None,
                sha1: Some(sha1_bytes(contents)),
                xxh3: xxhash3_bytes(contents),
            },
            fingerprint: Some(SourceFingerprint::new(sha1_bytes(&file_bytes))),
            scan_run: None,
            scan_provenance: None,
            bare_file_cache_stamp: None,
        };
        let expected = ExpectedEvidence {
            scope: EvidenceScope::WholeAsset,
            provenance: EvidenceProvenance::SourceDeclared,
            size: Some(u64::try_from(contents.len())?),
            sha1: Some(sha1_bytes(contents)),
            ..ExpectedEvidence::default()
        };
        Ok(LogicalEntry {
            path: LogicalPath::new("game.rom"),
            requirement: RequirementKey::new(
                SetKey::new(CatalogKey::new("test"), "nes"),
                "game.rom",
            ),
            source,
            expected,
            selection: SelectionProvenance {
                policy: MatchingPolicy::Sha1Compatibility,
                strength: MatchStrength::Sha1,
                assessments: Vec::new(),
                omitted_assessments: 0,
            },
        })
    }

    fn content_identity(contents: &[u8]) -> Result<ServingIdentity, crate::Error> {
        Ok(ServingIdentity::Content(ContentIdentity::new(
            ContentDigestAlgorithm::Sha1,
            hex::encode(sha1_bytes(contents)),
        )?))
    }

    fn path(temp: &tempfile::TempDir) -> Result<Utf8PathBuf, io::Error> {
        Utf8PathBuf::try_from(temp.path().to_path_buf())
            .map_err(|_| io::Error::other("temporary path is not UTF-8"))
    }

    fn assert_decoder_capabilities(
        budget: &MaterializationBudget,
        entry: &LogicalEntry,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let capabilities = budget.capabilities(entry)?;
        match &entry.source.location {
            SourceLocation::ArchiveMember {
                backend: ArchiveBackend::Rar,
                ..
            } => {
                assert_eq!(capabilities.maximum_concurrent_archive_decoders, Some(2));
                assert_eq!(
                    capabilities.maximum_decoder_dictionary_bytes,
                    Some(ByteLength::new(256 * 1024 * 1024))
                );
                assert_eq!(capabilities.maximum_decoder_working_set_bytes, None);
            }
            SourceLocation::ArchiveMember {
                backend: ArchiveBackend::SevenZip,
                ..
            } => {
                assert_eq!(capabilities.maximum_concurrent_archive_decoders, Some(2));
                assert_eq!(
                    capabilities.maximum_decoder_dictionary_bytes,
                    Some(ByteLength::new(256 * 1024 * 1024))
                );
                assert_eq!(
                    capabilities.maximum_decoder_working_set_bytes,
                    Some(ByteLength::new(512 * 1024 * 1024))
                );
            }
            _ => {
                assert_eq!(capabilities.maximum_concurrent_archive_decoders, None);
                assert_eq!(capabilities.maximum_decoder_dictionary_bytes, None);
                assert_eq!(capabilities.maximum_decoder_working_set_bytes, None);
            }
        }
        Ok(())
    }

    #[test]
    fn bare_zip_7z_and_rar_sources_serve_verified_ranges() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp = tempfile::tempdir()?;
        let root = path(&temp)?;
        let bytes = b"verified member bytes";
        let budget = MaterializationBudget::new(ByteLength::new(1024));

        let bare = root.join("bare.rom");
        fs::write(&bare, bytes)?;
        let bare_entry = logical_entry(&bare, bytes, None, None)?;
        let zip = root.join("renamed.data");
        write_zip(&zip, &[("game.rom", bytes)])?;
        let zip_entry = logical_entry(
            &zip,
            bytes,
            Some(ArchiveBackend::Zip),
            Some((0, "game.rom")),
        )?;
        let seven_zip = root.join("source.7z");
        fs::write(
            &seven_zip,
            r7z::ArchiveBuilder::new()
                .add_file("game.rom", bytes)
                .build()?,
        )?;
        let seven_zip_entry = logical_entry(
            &seven_zip,
            bytes,
            Some(ArchiveBackend::SevenZip),
            Some((0, "game.rom")),
        )?;
        let rar = root.join("source.rar");
        fs::write(
            &rar,
            hex::decode(
                "526172211a0700cf907300000d000000000000000f0c7420802700150000000b0000000345f37dc6a48a07471d330700a481000056455253494f4e0c008fec8a45cc23c848088362fe5fdd5c5388f072c43d7b00400700",
            )?,
        )?;
        let rar_bytes = b"unrar-0.4.0";
        let rar_entry = logical_entry(
            &rar,
            rar_bytes,
            Some(ArchiveBackend::Rar),
            Some((0, "VERSION")),
        )?;

        for (entry, contents, expected_pattern) in [
            (
                &bare_entry,
                bytes.as_slice(),
                AccessPattern::CheapRandomAccess,
            ),
            (
                &zip_entry,
                bytes.as_slice(),
                AccessPattern::SequentialStreaming,
            ),
            (
                &seven_zip_entry,
                bytes.as_slice(),
                AccessPattern::BoundedMaterialization,
            ),
            (
                &rar_entry,
                rar_bytes.as_slice(),
                AccessPattern::BoundedMaterialization,
            ),
        ] {
            let capabilities = budget.capabilities(entry)?;
            assert_eq!(capabilities.access_pattern, expected_pattern);
            assert_decoder_capabilities(&budget, entry)?;
            let identity = content_identity(contents)?;
            let mut session = VerifiedContentSession::open(entry, identity.clone(), &budget)?;
            let range = ByteRange::new(ByteOffset::new(2), ByteLength::new(5))?;
            let mut output = [0; 5];
            let response = session.read_range(range, &mut output)?;
            assert_eq!(&output[..], &contents[2..7]);
            assert_eq!(response.identity, identity);
            assert_eq!(
                response.verification,
                VerificationStatus::WholeObjectVerified
            );
            assert_eq!(
                response.total_length,
                ByteLength::new(u64::try_from(contents.len())?)
            );
            assert_eq!(response.returned, ByteLength::new(5));
            assert!(!response.eof);
        }
        Ok(())
    }

    #[test]
    fn ordinal_selector_distinguishes_duplicate_normalized_names()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let archive = path(&temp)?.join("duplicates.zip");
        write_zip(
            &archive,
            &[("same.rom", b"first"), ("./same.rom", b"second")],
        )?;
        let entry = logical_entry(
            &archive,
            b"second",
            Some(ArchiveBackend::Zip),
            Some((1, "same.rom")),
        )?;
        let budget = MaterializationBudget::new(ByteLength::new(32));
        let mut session =
            VerifiedContentSession::open(&entry, content_identity(b"second")?, &budget)?;
        let mut output = [0; 6];
        session.read_range(
            ByteRange::new(ByteOffset::new(0), ByteLength::new(6))?,
            &mut output,
        )?;
        assert_eq!(&output, b"second");
        Ok(())
    }

    #[test]
    fn stale_sources_are_rejected_and_open_sessions_keep_the_verified_snapshot()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let source = path(&temp)?.join("game.rom");
        fs::write(&source, b"verified snapshot")?;
        let entry = logical_entry(&source, b"verified snapshot", None, None)?;
        let budget = MaterializationBudget::new(ByteLength::new(64));
        let mut session =
            VerifiedContentSession::open(&entry, content_identity(b"verified snapshot")?, &budget)?;
        fs::write(&source, b"changed after opening")?;
        let mut bytes = [0; 17];
        let response = session.read_range(
            ByteRange::new(ByteOffset::new(0), ByteLength::new(17))?,
            &mut bytes,
        )?;
        assert_eq!(&bytes, b"verified snapshot");
        assert_eq!(
            response.verification,
            VerificationStatus::WholeObjectVerified
        );
        assert!(
            VerifiedContentSession::open(&entry, content_identity(b"verified snapshot")?, &budget,)
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn budget_is_shared_released_on_drop_and_eof_is_explicit()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let source = path(&temp)?.join("game.rom");
        fs::write(&source, b"ten bytes!")?;
        let entry = logical_entry(&source, b"ten bytes!", None, None)?;
        let budget = MaterializationBudget::new(ByteLength::new(10));
        let directory = {
            let mut session =
                VerifiedContentSession::open(&entry, content_identity(b"ten bytes!")?, &budget)?;
            let directory = session.spool_directory().to_path_buf();
            assert!(
                VerifiedContentSession::open(&entry, content_identity(b"ten bytes!")?, &budget,)
                    .is_err()
            );
            let mut output = [0; 4];
            let response = session.read_range(
                ByteRange::new(ByteOffset::new(8), ByteLength::new(4))?,
                &mut output,
            )?;
            assert_eq!(&output[..2], b"s!");
            assert_eq!(response.returned, ByteLength::new(2));
            assert!(response.eof);
            let mut last_byte = [0; 1];
            let final_byte = session.read_range(
                ByteRange::new(ByteOffset::new(9), ByteLength::new(1))?,
                &mut last_byte,
            )?;
            assert_eq!(&last_byte, b"!");
            assert!(final_byte.eof);
            let mut whole_object = [0; 10];
            let complete = session.read_range(
                ByteRange::new(ByteOffset::new(0), ByteLength::new(10))?,
                &mut whole_object,
            )?;
            assert_eq!(&whole_object, b"ten bytes!");
            assert!(complete.eof);
            directory
        };
        assert!(!directory.exists());
        let _next =
            VerifiedContentSession::open(&entry, content_identity(b"ten bytes!")?, &budget)?;
        Ok(())
    }

    #[test]
    fn zero_byte_sessions_are_limited_and_release_their_slots()
    -> Result<(), Box<dyn std::error::Error>> {
        let budget = MaterializationBudget::new(ByteLength::new(0));
        let temp = tempfile::tempdir()?;
        let source = path(&temp)?.join("game.rom");
        fs::write(&source, b"content")?;
        let entry = logical_entry(&source, b"content", None, None)?;
        fs::remove_file(&source)?;
        let mut sessions = Vec::new();
        for _ in 0..MAX_ACTIVE_SESSIONS {
            sessions.push(budget.reserve(ByteLength::new(0))?);
        }
        assert!(matches!(
            VerifiedContentSession::open(&entry, content_identity(b"content")?, &budget),
            Err(ServingError::SessionLimitExceeded { .. })
        ));
        assert!(matches!(
            budget.reserve(ByteLength::new(0)),
            Err(ServingError::SessionLimitExceeded {
                maximum: MAX_ACTIVE_SESSIONS
            })
        ));
        drop(sessions);
        assert!(budget.reserve(ByteLength::new(0)).is_ok());
        Ok(())
    }

    #[test]
    fn range_overflow_and_undersized_buffers_are_rejected() -> Result<(), Box<dyn std::error::Error>>
    {
        assert!(matches!(
            ByteRange::new(ByteOffset::new(u64::MAX), ByteLength::new(1)),
            Err(ServingError::RangeOverflow)
        ));
        let temp = tempfile::tempdir()?;
        let source = path(&temp)?.join("game.rom");
        fs::write(&source, b"range")?;
        let entry = logical_entry(&source, b"range", None, None)?;
        let budget = MaterializationBudget::new(ByteLength::new(8));
        let mut session =
            VerifiedContentSession::open(&entry, content_identity(b"range")?, &budget)?;
        assert!(matches!(
            session.read_range(
                ByteRange::new(ByteOffset::new(0), ByteLength::new(5))?,
                &mut [0; 4]
            ),
            Err(ServingError::BufferTooSmall { .. })
        ));
        let empty = session.read_range(
            ByteRange::new(ByteOffset::new(u64::MAX), ByteLength::new(0))?,
            &mut [],
        )?;
        assert_eq!(empty.returned, ByteLength::new(0));
        assert!(empty.eof);
        Ok(())
    }

    #[test]
    fn pinned_content_identity_must_match_materialized_bytes()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let source = path(&temp)?.join("game.rom");
        fs::write(&source, b"actual bytes")?;
        let entry = logical_entry(&source, b"actual bytes", None, None)?;
        let wrong_identity = ServingIdentity::Content(ContentIdentity::new(
            ContentDigestAlgorithm::Sha1,
            "0000000000000000000000000000000000000000",
        )?);
        let budget = MaterializationBudget::new(ByteLength::new(64));

        assert!(matches!(
            VerifiedContentSession::open(&entry, wrong_identity, &budget),
            Err(ServingError::IdentityMismatch { .. })
        ));
        Ok(())
    }
}
