//! Verified, snapshot-bound byte-range reads from source locations.

use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::PathBuf,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServingCapabilities {
    /// Native backend access mode. Opening a verified session still hashes and
    /// snapshots the entire object before it returns any range.
    pub access_pattern: AccessPattern,
    pub maximum_materialized_bytes: ByteLength,
    pub maximum_per_session_bytes: ByteLength,
    pub maximum_member_bytes: Option<ByteLength>,
    pub maximum_archive_entries: Option<usize>,
    pub maximum_prerequisite_member_bytes: Option<ByteLength>,
    pub maximum_prerequisite_output_bytes: Option<ByteLength>,
    pub maximum_concurrent_archive_decoders: Option<usize>,
    pub maximum_decoder_dictionary_bytes: Option<ByteLength>,
    pub maximum_decoder_working_set_bytes: Option<ByteLength>,
    pub maximum_open_sessions: usize,
    pub open_snapshots_entire_object: bool,
    /// Volume root used for this budget's private per-session spools.
    pub spool_root: PathBuf,
}

/// A shared ceiling for all simultaneously open materialized readers.
#[derive(Clone, Debug)]
pub struct MaterializationBudget {
    inner: Arc<Mutex<BudgetState>>,
    maximum_per_session: ByteLength,
    spool_root: PathBuf,
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
        Self::new_in(maximum, maximum, std::env::temp_dir())
    }

    /// Create a shared budget whose private session directories live on a caller-selected volume.
    #[must_use]
    pub fn new_in(
        maximum: ByteLength,
        maximum_per_session: ByteLength,
        spool_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            inner: Arc::new(Mutex::new(BudgetState {
                maximum,
                reserved: ByteLength::new(0),
                active_sessions: 0,
            })),
            maximum_per_session,
            spool_root: spool_root.into(),
        }
    }

    #[must_use]
    pub fn spool_root(&self) -> &std::path::Path {
        &self.spool_root
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
            maximum_per_session_bytes: self.maximum_per_session,
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
            spool_root: self.spool_root.clone(),
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
        if length > budget.maximum_per_session {
            return Err(ServingError::PerSessionLimitExceeded {
                requested: length,
                maximum: budget.maximum_per_session,
            });
        }
        lease.resize(length)?;
        let (temporary_directory, file, actual) =
            pinned_source.materialize(entry, &identity, budget.spool_root())?;
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

    /// Resolve a pinned source and return its current exact length without exposing bytes.
    /// Archive sources are enumerated with the serving backend and selected ordinal/name.
    pub fn preflight_source(entry: &LogicalEntry) -> Result<ByteLength, ServingError> {
        Ok(PinnedSource::resolve(entry)?.length)
    }

    /// Resolve several members of one archive from a single bounded inventory.
    pub fn preflight_archive_sources(
        entries: &[&LogicalEntry],
    ) -> Result<Vec<ByteLength>, ServingError> {
        let Some(first) = entries.first() else {
            return Ok(Vec::new());
        };
        let inventory = ArchiveInventory::open(first)?;
        entries
            .iter()
            .map(|entry| {
                PinnedSource::resolve_with_inventory(entry, Some(&inventory))
                    .map(|source| source.length)
            })
            .collect()
    }

    /// Stream and verify one planned source against its cached observation and selected catalog evidence.
    pub fn verify_logical_entry_fresh(entry: &LogicalEntry) -> Result<(), ServingError> {
        PinnedSource::resolve(entry)?.verify(entry)
    }

    /// Verify several selected members of one archive with a shared inventory and fingerprint pass.
    pub fn verify_archive_entries_fresh(
        entries: &[&LogicalEntry],
    ) -> Result<Vec<Result<(), ServingError>>, ServingError> {
        verify_archive_entries_with_fingerprint(entries, file_fingerprint)
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
    archive_member: Option<(ArchiveBackend, crate::sources::ArchiveMember)>,
}

struct SevenZipVerificationTarget {
    entry_index: usize,
    source_index: usize,
}

struct SevenZipVerificationMember {
    member: crate::sources::ArchiveMember,
    targets: Vec<SevenZipVerificationTarget>,
}

struct SevenZipVerificationBatch {
    results: Vec<Option<Result<(), ServingError>>>,
    members: BTreeMap<usize, SevenZipVerificationMember>,
    sources: Vec<PinnedSource>,
}

struct ArchiveInventory {
    physical_path: SourcePhysicalPath,
    backend: ArchiveBackend,
    members: BTreeMap<crate::sources::ArchiveMemberSelector, crate::sources::ArchiveMember>,
}

impl ArchiveInventory {
    fn open(entry: &LogicalEntry) -> Result<Self, ServingError> {
        let SourceLocation::ArchiveMember { path, backend, .. } = &entry.source.location else {
            return Err(ServingError::UnaddressableLocation {
                path: location_path(&entry.source.location).to_owned(),
            });
        };
        let physical_path = SourcePhysicalPath::capture(Utf8Path::new(path))?;
        if physical_path != entry.source.physical_path {
            return Err(ServingError::SourceLocationChanged { path: path.clone() });
        }
        let members = crate::sources::enumerate_for_serving(physical_path.as_path(), *backend)?
            .into_iter()
            .map(|member| (member.selector.clone(), member))
            .collect();
        Ok(Self {
            physical_path,
            backend: *backend,
            members,
        })
    }
}

impl PinnedSource {
    fn resolve(entry: &LogicalEntry) -> Result<Self, ServingError> {
        Self::resolve_with_inventory(entry, None)
    }

    fn resolve_with_inventory(
        entry: &LogicalEntry,
        archive_inventory: Option<&ArchiveInventory>,
    ) -> Result<Self, ServingError> {
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
                let missing_member = || ServingError::MemberUnavailable {
                    path: path.to_string(),
                    index: selector_index,
                    name: name.clone(),
                };
                let member = match archive_inventory {
                    Some(inventory)
                        if inventory.backend == *backend
                            && inventory.physical_path == physical_path =>
                    {
                        inventory
                            .members
                            .get(&selected)
                            .cloned()
                            .ok_or_else(missing_member)?
                    }
                    Some(_) => {
                        return Err(ServingError::SourceLocationChanged {
                            path: path.to_string(),
                        });
                    }
                    None => {
                        let inventory = crate::sources::enumerate_for_serving(path, *backend)?;
                        inventory
                            .into_iter()
                            .find(|member| member.selector == selected)
                            .ok_or_else(missing_member)?
                    }
                };
                (member.size, Some((*backend, member)))
            }
            SourceLocation::LegacyUnknown { .. } => {
                return Err(ServingError::UnaddressableLocation {
                    path: original_path.to_owned(),
                });
            }
        };
        Self::resolved(entry, physical_path, length, archive_member)
    }

    fn resolved(
        entry: &LogicalEntry,
        physical_path: SourcePhysicalPath,
        length: u64,
        archive_member: Option<(ArchiveBackend, crate::sources::ArchiveMember)>,
    ) -> Result<Self, ServingError> {
        if entry
            .source
            .observed
            .size
            .is_some_and(|observed| observed != length)
        {
            return Err(ServingError::SourceChanged {
                path: physical_path.as_path().to_string(),
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
        spool_root: &std::path::Path,
    ) -> Result<(PrivateTempDir, File, ContentDigests), ServingError> {
        let temporary_directory = PrivateTempDir::create_in(spool_root, "mame-coalesce-serving-")?;
        let staged_path = temporary_directory.path().join(MATERIALIZED_FILE_NAME);
        let output = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(staged_path)?;
        let (file, actual) = self.stream(entry, Some(identity), output)?;
        Ok((temporary_directory, file, actual))
    }

    fn verify(&self, entry: &LogicalEntry) -> Result<(), ServingError> {
        let (_, actual) = self.stream(entry, None, io::sink())?;
        let path = self.physical_path.as_path();
        verify_observation(entry, actual, path)?;
        verify_selected_evidence(entry, actual, path)
    }

    fn stream<W: Write>(
        &self,
        entry: &LogicalEntry,
        identity: Option<&ServingIdentity>,
        output: W,
    ) -> Result<(W, ContentDigests), ServingError> {
        self.stream_with_archive_fingerprint(entry, identity, output, true)
    }

    fn stream_with_archive_fingerprint<W: Write>(
        &self,
        entry: &LogicalEntry,
        identity: Option<&ServingIdentity>,
        output: W,
        verify_archive_fingerprint: bool,
    ) -> Result<(W, ContentDigests), ServingError> {
        let path = self.physical_path.as_path();
        let original_path = location_path(&entry.source.location);
        let mut materializer = ContentMaterializer::new(
            output,
            self.length,
            DigestRequirements::for_entry(entry, identity),
        );
        if let Some((backend, member)) = &self.archive_member {
            let before = if verify_archive_fingerprint {
                let fingerprint = file_fingerprint(path)?;
                if entry
                    .source
                    .fingerprint
                    .is_some_and(|expected| expected != fingerprint)
                {
                    return Err(ServingError::SourceChanged {
                        path: path.to_string(),
                        reason: "archive fingerprint differs from its observation".to_owned(),
                    });
                }
                Some(fingerprint)
            } else {
                None
            };
            crate::sources::stream_enumerated_archive_member_to_writer(
                path,
                *backend,
                member,
                &mut materializer,
            )?;
            if let Some(before) = before {
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
        let (output, actual) = materializer.finish()?;
        if actual.size != self.length {
            return Err(ServingError::SourceChanged {
                path: path.to_string(),
                reason: "materialized byte length differs from the pinned location".to_owned(),
            });
        }
        Ok((output, actual))
    }
}

fn verify_archive_entries_with_fingerprint(
    entries: &[&LogicalEntry],
    fingerprint: impl Fn(&Utf8Path) -> crate::Result<SourceFingerprint>,
) -> Result<Vec<Result<(), ServingError>>, ServingError> {
    let Some(first) = entries.first() else {
        return Ok(Vec::new());
    };
    let SourceLocation::ArchiveMember {
        path: first_path,
        backend,
        ..
    } = &first.source.location
    else {
        return Err(ServingError::UnaddressableLocation {
            path: location_path(&first.source.location).to_owned(),
        });
    };
    if entries.iter().any(|entry| {
        !matches!(
            &entry.source.location,
            SourceLocation::ArchiveMember { path, backend: candidate, .. }
                if path == first_path && candidate == backend
        )
    }) {
        return Err(ServingError::SourceLocationChanged {
            path: first_path.clone(),
        });
    }

    let inventory = ArchiveInventory::open(first)?;
    let sources: Vec<_> = entries
        .iter()
        .map(|entry| PinnedSource::resolve_with_inventory(entry, Some(&inventory)))
        .collect();
    let path = inventory.physical_path.as_path();
    let before = fingerprint(path)?;
    let mut results = if *backend == ArchiveBackend::SevenZip {
        verify_7z_archive_entries(entries, sources, before, path)
    } else {
        entries
            .iter()
            .zip(sources)
            .map(|(entry, source)| {
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
                source.and_then(|source| {
                    let (_, actual) =
                        source.stream_with_archive_fingerprint(entry, None, io::sink(), false)?;
                    verify_observation(entry, actual, path)?;
                    verify_selected_evidence(entry, actual, path)
                })
            })
            .collect()
    };
    let after = fingerprint(path)?;
    if after != before {
        let changed = || ServingError::SourceChanged {
            path: path.to_string(),
            reason: "archive changed while selected members were verified".to_owned(),
        };
        for result in &mut results {
            if result.is_ok() {
                *result = Err(changed());
            }
        }
    }
    for (entry, result) in entries.iter().zip(&mut results) {
        if !result.is_ok() {
            continue;
        }
        let original_path = location_path(&entry.source.location);
        match SourcePhysicalPath::capture(Utf8Path::new(original_path)) {
            Ok(current) if current == entry.source.physical_path => {}
            Ok(_) => {
                *result = Err(ServingError::SourceLocationChanged {
                    path: original_path.to_owned(),
                });
            }
            Err(error) => *result = Err(ServingError::Io(error)),
        }
    }
    Ok(results)
}

fn verify_7z_archive_entries(
    entries: &[&LogicalEntry],
    sources: Vec<Result<PinnedSource, ServingError>>,
    fingerprint: SourceFingerprint,
    path: &Utf8Path,
) -> Vec<Result<(), ServingError>> {
    let mut batch = prepare_7z_verification(entries, sources, fingerprint, path);
    let members: Vec<_> = batch
        .members
        .values()
        .map(|member| member.member.clone())
        .collect();
    let stream_result =
        crate::sources::stream_enumerated_7z_members(path, &members, |member, reader| {
            verify_7z_member(
                member,
                reader,
                &batch.members,
                entries,
                &batch.sources,
                &mut batch.results,
                path,
            )
        });
    if let Err(error) = stream_result {
        invalidate_7z_verification_results(&mut batch.results, &error.to_string());
    }
    batch
        .results
        .into_iter()
        .map(|result| {
            result.unwrap_or_else(|| {
                Err(ServingError::Source(crate::Error::InvalidPath(
                    "selected 7z member was not returned by the decoder".to_owned(),
                )))
            })
        })
        .collect()
}

fn prepare_7z_verification(
    entries: &[&LogicalEntry],
    sources: Vec<Result<PinnedSource, ServingError>>,
    fingerprint: SourceFingerprint,
    path: &Utf8Path,
) -> SevenZipVerificationBatch {
    let mut results: Vec<Option<Result<(), ServingError>>> = std::iter::repeat_with(|| None)
        .take(entries.len())
        .collect();
    let mut members = BTreeMap::new();
    let mut pinned_sources = Vec::new();
    for (entry_index, (entry, source)) in entries.iter().zip(sources).enumerate() {
        if entry
            .source
            .fingerprint
            .is_some_and(|expected| expected != fingerprint)
        {
            results[entry_index] = Some(Err(ServingError::SourceChanged {
                path: path.to_string(),
                reason: "archive fingerprint differs from its observation".to_owned(),
            }));
            continue;
        }
        let source = match source {
            Ok(source) => source,
            Err(error) => {
                results[entry_index] = Some(Err(error));
                continue;
            }
        };
        let Some((ArchiveBackend::SevenZip, member)) = &source.archive_member else {
            results[entry_index] = Some(Err(ServingError::SourceChanged {
                path: path.to_string(),
                reason: "selected 7z member is unavailable after enumeration".to_owned(),
            }));
            continue;
        };
        let member = member.clone();
        let member_index = member.selector.index;
        let source_index = pinned_sources.len();
        pinned_sources.push(source);
        members
            .entry(member_index)
            .or_insert_with(|| SevenZipVerificationMember {
                member,
                targets: Vec::new(),
            })
            .targets
            .push(SevenZipVerificationTarget {
                entry_index,
                source_index,
            });
    }
    SevenZipVerificationBatch {
        results,
        members,
        sources: pinned_sources,
    }
}

fn verify_7z_member(
    member: &crate::sources::ArchiveMember,
    reader: &mut dyn Read,
    members: &BTreeMap<usize, SevenZipVerificationMember>,
    entries: &[&LogicalEntry],
    pinned_sources: &[PinnedSource],
    results: &mut [Option<Result<(), ServingError>>],
    path: &Utf8Path,
) -> crate::Result<()> {
    let Some(selected_member) = members.get(&member.selector.index) else {
        return Err(crate::Error::InvalidPath(
            "7z decoder returned an unselected member".to_owned(),
        ));
    };
    let Some(first_target) = selected_member.targets.first() else {
        return Err(crate::Error::InvalidPath(
            "selected 7z member has no verification targets".to_owned(),
        ));
    };
    let first_source = &pinned_sources[first_target.source_index];
    let mut requirements = DigestRequirements::for_entry(entries[first_target.entry_index], None);
    for target in selected_member.targets.iter().skip(1) {
        let additional = DigestRequirements::for_entry(entries[target.entry_index], None);
        requirements.md5 |= additional.md5;
        requirements.crc32 |= additional.crc32;
        requirements.sha256 |= additional.sha256;
    }
    let mut materializer = ContentMaterializer::new(io::sink(), first_source.length, requirements);
    let actual = (|| {
        io::copy(reader, &mut materializer)?;
        materializer.finish().map(|(_, actual)| actual)
    })();
    for target in &selected_member.targets {
        let verification = match &actual {
            Ok(actual) if pinned_sources[target.source_index].length == first_source.length => {
                verify_observation(entries[target.entry_index], *actual, path).and_then(|()| {
                    verify_selected_evidence(entries[target.entry_index], *actual, path)
                })
            }
            Ok(_) => Err(ServingError::SourceChanged {
                path: path.to_string(),
                reason: "selected member size changed after enumeration".to_owned(),
            }),
            Err(error) => Err(ServingError::Source(crate::Error::InvalidPath(
                error.to_string(),
            ))),
        };
        results[target.entry_index] = Some(verification);
    }
    Ok(())
}

fn invalidate_7z_verification_results(
    results: &mut [Option<Result<(), ServingError>>],
    reason: &str,
) {
    for result in results {
        if result.as_ref().is_none_or(Result::is_ok) {
            *result = Some(Err(ServingError::Source(crate::Error::InvalidPath(
                reason.to_owned(),
            ))));
        }
    }
}

impl Drop for VerifiedContentSession {
    fn drop(&mut self) {
        self.file.take();
    }
}

struct ContentMaterializer<W: Write> {
    output: W,
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
    fn for_entry(entry: &LogicalEntry, identity: Option<&ServingIdentity>) -> Self {
        let identity_algorithm = identity.map(|identity| match identity {
            ServingIdentity::Content(identity) => identity.algorithm(),
            ServingIdentity::Representation(_) => ContentDigestAlgorithm::Sha256,
        });
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

impl<W: Write> ContentMaterializer<W> {
    fn new(output: W, maximum: ByteLength, requirements: DigestRequirements) -> Self {
        Self {
            output,
            maximum,
            size: 0,
            sha1: Sha1::new(),
            md5: requirements.md5.then(Md5::new),
            crc32: requirements.crc32.then(crc32fast::Hasher::new),
            xxh3: Xxh3::new(),
            sha256: requirements.sha256.then(Sha256::new),
        }
    }

    fn finish(mut self) -> Result<(W, ContentDigests), ServingError> {
        self.output.flush()?;
        let evidence = ContentDigests {
            size: ByteLength::new(self.size),
            sha1: self.sha1.finalize().into(),
            md5: self.md5.map(|digest| digest.finalize().into()),
            crc32: self.crc32.map(crc32fast::Hasher::finalize),
            xxh3: self.xxh3.digest().to_be_bytes(),
            sha256: self.sha256.map(|digest| digest.finalize().into()),
        };
        Ok((self.output, evidence))
    }
}

impl<W: Write> Write for ContentMaterializer<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = u64::try_from(bytes.len()).map_err(io::Error::other)?;
        let Some(size) = self.size.checked_add(count) else {
            return Err(io::Error::other("materialized content length overflow"));
        };
        if size > self.maximum.get() {
            return Err(io::Error::other("materialization budget exceeded"));
        }
        self.output.write_all(bytes)?;
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
        self.output.flush()
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
    #[error("session needs {requested:?}, above its {maximum:?} per-session limit")]
    PerSessionLimitExceeded {
        requested: ByteLength,
        maximum: ByteLength,
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

/// Stable failure categories for platform adapters; display text is never inspected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServingFailureKind {
    NotFound,
    Stale,
    Capacity,
    Unsupported,
    Corrupt,
    Io,
}

impl ServingError {
    #[must_use]
    pub fn failure_kind(&self) -> ServingFailureKind {
        match self {
            Self::Io(error) => classify_io(error),
            Self::Source(error) => classify_source(error),
            Self::BudgetExceeded { .. }
            | Self::PerSessionLimitExceeded { .. }
            | Self::SessionLimitExceeded { .. } => ServingFailureKind::Capacity,
            Self::SourceChanged { .. }
            | Self::SourceKindChanged { .. }
            | Self::SourceLocationChanged { .. }
            | Self::MemberUnavailable { .. }
            | Self::EvidenceMismatch { .. }
            | Self::IdentityMismatch { .. } => ServingFailureKind::Stale,
            Self::UnaddressableLocation { .. } | Self::InvalidMemberIndex(_) => {
                ServingFailureKind::Unsupported
            }
            Self::RangeOverflow
            | Self::RangeTooLarge(_)
            | Self::BufferTooSmall { .. }
            | Self::BudgetPoisoned
            | Self::SessionClosed => ServingFailureKind::Io,
        }
    }
}

fn classify_io(error: &io::Error) -> ServingFailureKind {
    match error.kind() {
        io::ErrorKind::NotFound => ServingFailureKind::NotFound,
        io::ErrorKind::StorageFull | io::ErrorKind::QuotaExceeded => ServingFailureKind::Capacity,
        _ => ServingFailureKind::Io,
    }
}

fn classify_source(error: &crate::Error) -> ServingFailureKind {
    match error {
        crate::Error::Io(error) if error.kind() == io::ErrorKind::NotFound => {
            ServingFailureKind::Stale
        }
        crate::Error::Io(error) => classify_io(error),
        crate::Error::ArchiveDecoderLimitExceeded { .. } => ServingFailureKind::Capacity,
        crate::Error::SourceChanged { .. } => ServingFailureKind::Stale,
        crate::Error::InvalidPath(_)
        | crate::Error::Zip(_)
        | crate::Error::Archive(_)
        | crate::Error::Rar(_) => ServingFailureKind::Corrupt,
        _ => ServingFailureKind::Io,
    }
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

    fn assert_verified_ranges(
        session: &mut VerifiedContentSession,
        identity: &ServingIdentity,
        contents: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let range = ByteRange::new(ByteOffset::new(2), ByteLength::new(5))?;
        let mut output = [0; 5];
        let response = session.read_range(range, &mut output)?;
        assert_eq!(&output[..], &contents[2..7]);
        assert_eq!(&response.identity, identity);
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
        for offset in [
            contents.len() - 1,
            0,
            contents.len() / 2,
            1,
            contents.len() - 1,
        ] {
            let length = (contents.len() - offset).min(4);
            let mut ranged = vec![0; length];
            let response = session.read_range(
                ByteRange::new(
                    ByteOffset::new(u64::try_from(offset)?),
                    ByteLength::new(u64::try_from(length)?),
                )?,
                &mut ranged,
            )?;
            assert_eq!(ranged, contents[offset..offset + length]);
            assert_eq!(response.returned, ByteLength::new(u64::try_from(length)?));
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
            assert_eq!(
                VerifiedContentSession::preflight_source(entry)?,
                ByteLength::new(u64::try_from(contents.len())?)
            );
            VerifiedContentSession::verify_logical_entry_fresh(entry)?;
            assert_decoder_capabilities(&budget, entry)?;
            let identity = content_identity(contents)?;
            let mut session = VerifiedContentSession::open(entry, identity.clone(), &budget)?;
            assert_verified_ranges(&mut session, &identity, contents)?;
        }
        Ok(())
    }

    #[test]
    fn archive_preflight_reuses_one_inventory_for_multiple_members()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let archive = path(&temp)?.join("multi-member.zip");
        let first = b"first member";
        let second = b"second member";
        write_zip(&archive, &[("first.rom", first), ("second.rom", second)])?;
        let first_entry = logical_entry(
            &archive,
            first,
            Some(ArchiveBackend::Zip),
            Some((0, "first.rom")),
        )?;
        let second_entry = logical_entry(
            &archive,
            second,
            Some(ArchiveBackend::Zip),
            Some((1, "second.rom")),
        )?;

        assert_eq!(
            VerifiedContentSession::preflight_archive_sources(&[&first_entry, &second_entry])?,
            [
                ByteLength::new(u64::try_from(first.len())?),
                ByteLength::new(u64::try_from(second.len())?),
            ]
        );
        let fingerprint_calls = std::cell::Cell::new(0);
        let verified =
            verify_archive_entries_with_fingerprint(&[&first_entry, &second_entry], |path| {
                fingerprint_calls.set(fingerprint_calls.get() + 1);
                file_fingerprint(path)
            })?;
        assert!(verified.into_iter().all(|result| result.is_ok()));
        assert_eq!(fingerprint_calls.get(), 2);
        Ok(())
    }

    #[test]
    fn selected_solid_7z_members_are_verified_in_one_stream_pass()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let archive = path(&temp)?.join("multi-member.7z");
        let first = b"first selected member";
        let second = b"second selected member";
        fs::write(
            &archive,
            r7z::ArchiveBuilder::new()
                .add_file("first.rom", first)
                .add_file("second.rom", second)
                .build()?,
        )?;
        let first_entry = logical_entry(
            &archive,
            first,
            Some(ArchiveBackend::SevenZip),
            Some((0, "first.rom")),
        )?;
        let second_entry = logical_entry(
            &archive,
            second,
            Some(ArchiveBackend::SevenZip),
            Some((1, "second.rom")),
        )?;

        let verified = verify_archive_entries_with_fingerprint(
            &[&first_entry, &second_entry],
            file_fingerprint,
        )?;
        assert!(verified.into_iter().all(|result| result.is_ok()));
        Ok(())
    }

    #[test]
    fn late_7z_integrity_error_invalidates_provisional_member_successes() {
        let mut results = vec![Some(Ok(())), None];
        invalidate_7z_verification_results(&mut results, "archive CRC mismatch");
        assert!(
            results
                .iter()
                .all(|result| result.as_ref().is_some_and(Result::is_err))
        );
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
        assert!(matches!(
            VerifiedContentSession::verify_logical_entry_fresh(&entry),
            Err(ServingError::SourceChanged { .. })
        ));
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
    fn same_length_stale_sources_fail_before_serving_any_bytes()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let source = path(&temp)?.join("same-length.rom");
        let pinned = b"before";
        fs::write(&source, pinned)?;
        let entry = logical_entry(&source, pinned, None, None)?;
        let budget = MaterializationBudget::new(ByteLength::new(64));

        fs::write(&source, b"later!")?;
        assert_eq!(
            VerifiedContentSession::preflight_source(&entry)?,
            ByteLength::new(u64::try_from(pinned.len())?)
        );
        assert!(matches!(
            VerifiedContentSession::open(&entry, content_identity(pinned)?, &budget),
            Err(error) if error.failure_kind() == ServingFailureKind::Stale
        ));
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

    #[test]
    fn spool_creation_failure_releases_quota_for_a_later_open()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let root = path(&temp)?;
        let source = root.join("game.rom");
        let spool_root = root.join("spool-root");
        let contents = b"retry bytes";
        fs::write(&source, contents)?;
        fs::write(&spool_root, b"not a directory")?;
        let entry = logical_entry(&source, contents, None, None)?;
        let budget = MaterializationBudget::new_in(
            ByteLength::new(u64::try_from(contents.len())?),
            ByteLength::new(u64::try_from(contents.len())?),
            spool_root.clone(),
        );

        assert!(matches!(
            VerifiedContentSession::open(&entry, content_identity(contents)?, &budget),
            Err(ServingError::Io(_))
        ));
        fs::remove_file(&spool_root)?;
        fs::create_dir(&spool_root)?;
        let session = VerifiedContentSession::open(&entry, content_identity(contents)?, &budget)?;
        assert!(session.spool_directory().starts_with(&spool_root));
        drop(session);
        assert_eq!(fs::read_dir(&spool_root)?.count(), 0);
        Ok(())
    }

    #[test]
    fn sessions_use_the_configured_spool_root_and_enforce_the_per_file_limit()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let root = path(&temp)?;
        let spool_root = root.join("spool");
        let source = root.join("game.rom");
        fs::create_dir(&spool_root)?;
        fs::write(&source, b"five!")?;
        let entry = logical_entry(&source, b"five!", None, None)?;
        let budget = MaterializationBudget::new_in(
            ByteLength::new(512),
            ByteLength::new(4),
            spool_root.clone(),
        );

        assert!(matches!(
            VerifiedContentSession::open(&entry, content_identity(b"five!")?, &budget),
            Err(ServingError::PerSessionLimitExceeded {
                requested: ByteLength(5),
                maximum: ByteLength(4),
            })
        ));
        assert_eq!(fs::read_dir(&spool_root)?.count(), 0);

        let allowed = MaterializationBudget::new_in(
            ByteLength::new(512),
            ByteLength::new(5),
            spool_root.clone(),
        );
        let session = VerifiedContentSession::open(&entry, content_identity(b"five!")?, &allowed)?;
        let directory = session.spool_directory();
        assert!(directory.starts_with(&spool_root));
        assert_eq!(allowed.spool_root(), spool_root);
        assert_eq!(allowed.capabilities(&entry)?.spool_root, spool_root);
        drop(session);
        assert_eq!(fs::read_dir(&spool_root)?.count(), 0);
        Ok(())
    }

    #[test]
    fn serving_failures_have_stable_typed_categories() {
        assert_eq!(
            ServingError::Io(io::Error::new(io::ErrorKind::NotFound, "untrusted text"))
                .failure_kind(),
            ServingFailureKind::NotFound
        );
        assert_eq!(
            ServingError::Source(crate::Error::Io(io::Error::new(
                io::ErrorKind::NotFound,
                "pinned source disappeared",
            )))
            .failure_kind(),
            ServingFailureKind::Stale
        );
        assert_eq!(
            ServingError::PerSessionLimitExceeded {
                requested: ByteLength::new(2),
                maximum: ByteLength::new(1),
            }
            .failure_kind(),
            ServingFailureKind::Capacity
        );
        assert_eq!(
            ServingError::Source(crate::Error::ArchiveDecoderLimitExceeded { maximum: 2 })
                .failure_kind(),
            ServingFailureKind::Capacity
        );
        assert_eq!(
            ServingError::SourceChanged {
                path: "rom.bin".to_owned(),
                reason: "untrusted text".to_owned(),
            }
            .failure_kind(),
            ServingFailureKind::Stale
        );
        assert_eq!(
            ServingError::UnaddressableLocation {
                path: "rom.bin".to_owned(),
            }
            .failure_kind(),
            ServingFailureKind::Unsupported
        );
    }
}
