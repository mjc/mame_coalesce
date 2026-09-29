//! Shared, content-based source archive access.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom, Write},
    sync::atomic::{AtomicUsize, Ordering},
};

use camino::Utf8Path;

use crate::domain::ArchiveBackend;
use crate::{Error, Result};

const RAR_MAX_MEMBER_SIZE: u64 = 128 * 1024 * 1024;
const RAR_MAX_PREREQUISITE_MEMBER_SIZE: u64 = 512 * 1024 * 1024;
const RAR_MAX_PREREQUISITE_OUTPUT_SIZE: u64 = 1024 * 1024 * 1024;
const RAR_MAX_DICTIONARY_SIZE: u64 = 256 * 1024 * 1024;
const SEVEN_Z_MAX_DICTIONARY_SIZE: u64 = 256 * 1024 * 1024;
const SEVEN_Z_MAX_DECODER_WORKING_SET_SIZE: u64 = 512 * 1024 * 1024;
const MAX_ACTIVE_ARCHIVE_DECODERS: usize = 2;
const SEVEN_Z_MAX_SELECTED_MEMBER_SIZE: u64 = 16 * 1024 * 1024 * 1024;
const SEVEN_Z_MAX_PACKED_FOLDER_SIZE: u64 = 512 * 1024 * 1024;
const SEVEN_Z_MAX_PREREQUISITE_MEMBER_SIZE: u64 = 512 * 1024 * 1024;
const SEVEN_Z_MAX_PREREQUISITE_OUTPUT_SIZE: u64 = 1024 * 1024 * 1024;
pub const MAX_SERVING_ARCHIVE_ENTRIES: usize = 100_000;

static ARCHIVE_DECODER_SLOTS: DecoderSlots = DecoderSlots::new(MAX_ACTIVE_ARCHIVE_DECODERS);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArchiveMemberSelector {
    pub index: usize,
    /// The centrally normalized, safe archive-relative path.
    pub name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    BareFile,
    Archive(ArchiveBackend),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveMember {
    pub selector: ArchiveMemberSelector,
    pub size: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BackendCapabilities {
    pub backend: ArchiveBackend,
    pub selected_reads: &'static str,
    pub limitation: &'static str,
    pub max_member_size: Option<u64>,
    pub max_packed_folder_size: Option<u64>,
    pub max_prerequisite_member_size: Option<u64>,
    pub max_prerequisite_output_size: Option<u64>,
    pub max_concurrent_decoders: Option<usize>,
    pub max_decoder_dictionary_size: Option<u64>,
    pub max_decoder_working_set_size: Option<u64>,
}

/// Operational details of the backends pinned by this project.
pub const BACKEND_CAPABILITIES: [BackendCapabilities; 3] = [
    BackendCapabilities {
        backend: ArchiveBackend::Zip,
        selected_reads: "Selected indices can be opened in caller order.",
        limitation: "The pinned ZIP reader collapses exact duplicate raw filenames, so those archives are rejected; distinct raw names that normalize identically remain selectable by index.",
        max_member_size: None,
        max_packed_folder_size: None,
        max_prerequisite_member_size: None,
        max_prerequisite_output_size: None,
        max_concurrent_decoders: None,
        max_decoder_dictionary_size: None,
        max_decoder_working_set_size: None,
    },
    BackendCapabilities {
        backend: ArchiveBackend::SevenZip,
        selected_reads: "Selected members are streamed by index in archive order; each selected solid folder is decoded once and folders with no selected entries are not opened.",
        limitation: "Serving admits at most two concurrent 7z/RAR decoders. Per decoder, LZMA/LZMA2 dictionaries and PPMd memory are capped at 256 MiB; decoder working state is capped at 512 MiB. Unselected members in selected solid folders are capped at 512 MiB each and 1 GiB aggregate decoded output. Multi-range folders buffer at most 512 MiB of packed data. BCJ2 enforces a 512 MiB combined packed/intermediate/final-buffer/decoder-state budget and a 256 MiB final-output ceiling. Decoder chains are limited to 64 coders; AES buffers are capped at 128 MiB each, and materialized folder output shares a 512 MiB budget with decoder state.",
        max_member_size: Some(SEVEN_Z_MAX_SELECTED_MEMBER_SIZE),
        max_packed_folder_size: Some(SEVEN_Z_MAX_PACKED_FOLDER_SIZE),
        max_prerequisite_member_size: Some(SEVEN_Z_MAX_PREREQUISITE_MEMBER_SIZE),
        max_prerequisite_output_size: Some(SEVEN_Z_MAX_PREREQUISITE_OUTPUT_SIZE),
        max_concurrent_decoders: Some(MAX_ACTIVE_ARCHIVE_DECODERS),
        max_decoder_dictionary_size: Some(SEVEN_Z_MAX_DICTIONARY_SIZE),
        max_decoder_working_set_size: Some(SEVEN_Z_MAX_DECODER_WORKING_SET_SIZE),
    },
    BackendCapabilities {
        backend: ArchiveBackend::Rar,
        selected_reads: "For serving, selected entries stream directly to the bounded spool and solid predecessors are decoded to a bounded sink.",
        limitation: "Serving admits at most two concurrent 7z/RAR decoders. Each RAR decoder has a 256 MiB dictionary ceiling. Selected members are capped at 128 MiB. Solid prerequisites are capped at 512 MiB each and 1 GiB total decoded output.",
        max_member_size: Some(RAR_MAX_MEMBER_SIZE),
        max_packed_folder_size: None,
        max_prerequisite_member_size: Some(RAR_MAX_PREREQUISITE_MEMBER_SIZE),
        max_prerequisite_output_size: Some(RAR_MAX_PREREQUISITE_OUTPUT_SIZE),
        max_concurrent_decoders: Some(MAX_ACTIVE_ARCHIVE_DECODERS),
        max_decoder_dictionary_size: Some(RAR_MAX_DICTIONARY_SIZE),
        max_decoder_working_set_size: None,
    },
];

pub const fn capabilities(backend: ArchiveBackend) -> &'static BackendCapabilities {
    match backend {
        ArchiveBackend::Zip => &BACKEND_CAPABILITIES[0],
        ArchiveBackend::SevenZip => &BACKEND_CAPABILITIES[1],
        ArchiveBackend::Rar => &BACKEND_CAPABILITIES[2],
    }
}

struct DecoderSlots {
    maximum: usize,
    active: AtomicUsize,
}

impl DecoderSlots {
    const fn new(maximum: usize) -> Self {
        Self {
            maximum,
            active: AtomicUsize::new(0),
        }
    }

    fn try_acquire(&self) -> Option<DecoderPermit<'_>> {
        self.active
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |active| {
                active.checked_add(1).filter(|next| *next <= self.maximum)
            })
            .ok()
            .map(|_| DecoderPermit { slots: self })
    }
}

struct DecoderPermit<'a> {
    slots: &'a DecoderSlots,
}

impl Drop for DecoderPermit<'_> {
    fn drop(&mut self) {
        self.slots.active.fetch_sub(1, Ordering::Release);
    }
}

fn reserve_serving_decoder() -> Result<DecoderPermit<'static>> {
    ARCHIVE_DECODER_SLOTS
        .try_acquire()
        .ok_or(Error::ArchiveDecoderLimitExceeded {
            maximum: MAX_ACTIVE_ARCHIVE_DECODERS,
        })
}

pub fn detect(path: &Utf8Path) -> Result<SourceKind> {
    let mut file = File::open(path)?;
    let mut signature = [0_u8; 8];
    let count = file.read(&mut signature)?;
    let signature = &signature[..count];

    if signature.starts_with(b"PK\x03\x04")
        || signature.starts_with(b"PK\x05\x06")
        || signature.starts_with(b"PK\x07\x08")
    {
        return Ok(SourceKind::Archive(ArchiveBackend::Zip));
    }
    if signature.starts_with(b"7z\xBC\xAF\x27\x1C") {
        return Ok(SourceKind::Archive(ArchiveBackend::SevenZip));
    }
    if signature.starts_with(b"Rar!\x1A\x07\x00") || signature.starts_with(b"Rar!\x1A\x07\x01\x00")
    {
        return Ok(SourceKind::Archive(ArchiveBackend::Rar));
    }
    Ok(SourceKind::BareFile)
}

pub fn stream_file(path: &Utf8Path, writer: &mut dyn Write) -> Result<()> {
    io::copy(&mut File::open(path)?, writer)?;
    Ok(())
}

pub fn enumerate(path: &Utf8Path, backend: ArchiveBackend) -> Result<Vec<ArchiveMember>> {
    enumerate_with_limit(path, backend, None)
}

/// Enumerates an archive under the member-count limit used by content serving.
pub fn enumerate_for_serving(
    path: &Utf8Path,
    backend: ArchiveBackend,
) -> Result<Vec<ArchiveMember>> {
    enumerate_with_limit(path, backend, Some(MAX_SERVING_ARCHIVE_ENTRIES))
}

fn enumerate_with_limit(
    path: &Utf8Path,
    backend: ArchiveBackend,
    maximum_entries: Option<usize>,
) -> Result<Vec<ArchiveMember>> {
    match detect(path)? {
        SourceKind::Archive(detected) if detected == backend => {}
        SourceKind::Archive(detected) => {
            return Err(Error::InvalidPath(format!(
                "archive backend mismatch for {path}: expected {backend:?}, detected {detected:?}"
            )));
        }
        SourceKind::BareFile => {
            return Err(Error::InvalidPath(format!(
                "source is not a recognized archive: {path}"
            )));
        }
    }
    let result = match backend {
        ArchiveBackend::Zip => enumerate_zip(path, maximum_entries),
        ArchiveBackend::SevenZip => enumerate_7z(path, maximum_entries),
        ArchiveBackend::Rar => enumerate_rar(path, maximum_entries),
    };
    result.map_err(|error| enumeration_error(error, backend, path))
}

fn enumeration_error(error: Error, backend: ArchiveBackend, path: &Utf8Path) -> Error {
    match error {
        Error::ArchiveDecoderLimitExceeded { .. } => error,
        error => Error::InvalidPath(format!(
            "failed to enumerate {backend:?} source {path}: {error}"
        )),
    }
}

pub fn stream_archive<F>(
    path: &Utf8Path,
    backend: ArchiveBackend,
    selected: Option<&[ArchiveMemberSelector]>,
    mut callback: F,
) -> Result<Vec<ArchiveMember>>
where
    F: FnMut(&ArchiveMember, &mut dyn Read) -> Result<()>,
{
    let capabilities = capabilities(backend);
    log::debug!(
        "streaming {backend:?} source {path}: {}; limitation: {}",
        capabilities.selected_reads,
        capabilities.limitation
    );
    let inventory = enumerate(path, backend)?;
    let selected_members = resolve_selection(&inventory, selected)?;
    let result = match backend {
        ArchiveBackend::Zip => stream_zip(path, &selected_members, &mut callback),
        ArchiveBackend::SevenZip => stream_7z(path, &selected_members, &mut callback),
        ArchiveBackend::Rar => stream_rar(path, &selected_members, &mut callback),
    };
    result.map_err(|error| {
        Error::InvalidPath(format!(
            "failed to stream {backend:?} source {path}: {error}"
        ))
    })?;
    Ok(inventory)
}

/// Streams one selected member directly to a bounded writer.
///
/// Unlike [`stream_archive`], this avoids a reader-side extraction spool for
/// backends whose reader API must materialize a member before returning it.
pub fn stream_enumerated_archive_member_to_writer(
    path: &Utf8Path,
    backend: ArchiveBackend,
    member: &ArchiveMember,
    writer: &mut dyn Write,
) -> Result<()> {
    match backend {
        ArchiveBackend::Zip => stream_zip(path, std::slice::from_ref(member), &mut |_, reader| {
            io::copy(reader, writer)?;
            Ok(())
        })?,
        ArchiveBackend::SevenZip => {
            stream_7z_for_serving(path, std::slice::from_ref(member), &mut |_, reader| {
                io::copy(reader, writer)?;
                Ok(())
            })?;
        }
        ArchiveBackend::Rar => stream_rar_to_writer(path, member, writer)?,
    }
    Ok(())
}

fn stream_rar_to_writer(
    path: &Utf8Path,
    member: &ArchiveMember,
    writer: &mut dyn Write,
) -> Result<()> {
    let maximum = capabilities(ArchiveBackend::Rar)
        .max_member_size
        .ok_or_else(|| Error::InvalidPath("RAR member limit is not configured".to_owned()))?;
    if member.size > maximum {
        return Err(Error::InvalidPath(format!(
            "RAR member {} exceeds the {} MiB extraction limit",
            member.selector.name,
            maximum / (1024 * 1024)
        )));
    }
    let _decoder_permit = reserve_serving_decoder()?;
    let mut archive = unrar_rs::RarArchive::open(File::open(path)?).map_err(|error| {
        Error::InvalidPath(format!("failed to open RAR source {path}: {error}"))
    })?;
    archive.set_limits(unrar_rs::limits::Limits {
        max_unpacked_size: RAR_MAX_PREREQUISITE_MEMBER_SIZE,
        max_dict_size: RAR_MAX_DICTIONARY_SIZE,
        ..unrar_rs::limits::Limits::default()
    });
    let selected_member_is_solid = archive.is_solid()
        || archive
            .by_index(member.selector.index)
            .map_err(|error| {
                Error::InvalidPath(format!("failed to inspect selected RAR member: {error}"))
            })?
            .info()
            .compression
            .solid;
    if selected_member_is_solid {
        let mut decoded_prerequisites = 0_u64;
        for index in 0..member.selector.index {
            let entry = archive.by_index(index).map_err(|error| {
                Error::InvalidPath(format!("failed to select RAR prerequisite: {error}"))
            })?;
            if entry
                .size()
                .is_some_and(|size| size > RAR_MAX_PREREQUISITE_MEMBER_SIZE)
            {
                return Err(Error::InvalidPath(format!(
                    "RAR solid prerequisite exceeds the {} MiB per-member decode limit",
                    RAR_MAX_PREREQUISITE_MEMBER_SIZE / (1024 * 1024)
                )));
            }
            let remaining = RAR_MAX_PREREQUISITE_OUTPUT_SIZE - decoded_prerequisites;
            let mut discard = io::sink();
            let mut bounded = BoundedMemberWriter::new(&mut discard, remaining);
            let written = entry.copy_to(&mut bounded).map_err(|error| {
                Error::InvalidPath(format!("failed to decode RAR prerequisite: {error}"))
            })?;
            if written != bounded.written {
                return Err(Error::InvalidPath(
                    "RAR prerequisite byte count changed while decoding".to_owned(),
                ));
            }
            decoded_prerequisites =
                decoded_prerequisites.checked_add(written).ok_or_else(|| {
                    Error::InvalidPath("RAR prerequisite byte count overflowed".to_owned())
                })?;
        }
    }
    let entry = archive
        .by_index(member.selector.index)
        .map_err(|error| Error::InvalidPath(format!("failed to select RAR member: {error}")))?;
    if entry.is_dir()
        || normalize_member_name(entry.name())? != member.selector.name
        || entry.size() != Some(member.size)
    {
        return Err(Error::InvalidPath(format!(
            "RAR member changed after enumeration at index {}: {}",
            member.selector.index, member.selector.name
        )));
    }
    let mut bounded = BoundedMemberWriter::new(writer, member.size);
    let written = entry
        .copy_to(&mut bounded)
        .map_err(|error| Error::InvalidPath(format!("failed to decode RAR member: {error}")))?;
    if written != member.size || bounded.written != member.size {
        return Err(Error::InvalidPath(format!(
            "RAR member size changed while reading: {}",
            member.selector.name
        )));
    }
    Ok(())
}

struct BoundedMemberWriter<'a> {
    writer: &'a mut dyn Write,
    limit: u64,
    written: u64,
}

impl<'a> BoundedMemberWriter<'a> {
    const fn new(writer: &'a mut dyn Write, limit: u64) -> Self {
        Self {
            writer,
            limit,
            written: 0,
        }
    }
}

impl Write for BoundedMemberWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let remaining = self.limit - self.written;
        if remaining == 0 && !buffer.is_empty() {
            return Err(io::Error::other("RAR decoded output exceeded its limit"));
        }
        let allowed = usize::try_from(remaining)
            .unwrap_or(usize::MAX)
            .min(buffer.len());
        let written = self.writer.write(&buffer[..allowed])?;
        self.written = self
            .written
            .checked_add(u64::try_from(written).map_err(io::Error::other)?)
            .ok_or_else(|| io::Error::other("RAR decoded output size overflowed"))?;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

fn resolve_selection(
    inventory: &[ArchiveMember],
    selected: Option<&[ArchiveMemberSelector]>,
) -> Result<Vec<ArchiveMember>> {
    let Some(selected) = selected else {
        return Ok(inventory.to_vec());
    };
    let by_selector = inventory
        .iter()
        .map(|member| {
            (
                (member.selector.index, member.selector.name.as_str()),
                member,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut resolved = Vec::with_capacity(selected.len());
    let mut seen = BTreeSet::new();
    for selector in selected {
        let normalized = normalize_member_name(&selector.name)?;
        let key = (selector.index, normalized.as_str());
        let member = by_selector.get(&key).ok_or_else(|| {
            Error::InvalidPath(format!(
                "archive entry not found for selector index {} name {:?}",
                selector.index, selector.name
            ))
        })?;
        if seen.contains(&(selector.index, normalized.clone())) {
            return Err(Error::InvalidPath(format!(
                "archive selector is duplicated: index {} name {:?}",
                selector.index, selector.name
            )));
        }
        let member = (*member).clone();
        seen.insert((selector.index, normalized));
        resolved.push(member);
    }
    Ok(resolved)
}

pub fn normalize_member_name(name: &str) -> Result<String> {
    if name.is_empty() || name.contains(['\\', '\0']) || name.starts_with('/') {
        return Err(unsafe_member_name(name));
    }
    let mut components = Vec::new();
    for component in name.split('/') {
        match component {
            "" | "." => {}
            ".." => return Err(unsafe_member_name(name)),
            value if components.is_empty() && value.contains(':') => {
                return Err(unsafe_member_name(name));
            }
            value => components.push(value),
        }
    }
    if components.is_empty() {
        return Err(unsafe_member_name(name));
    }
    Ok(components.join("/"))
}

fn unsafe_member_name(name: &str) -> Error {
    Error::InvalidPath(format!("unsafe archive member name: {name:?}"))
}

fn archive_member(index: usize, name: &str, size: u64) -> Result<ArchiveMember> {
    Ok(ArchiveMember {
        selector: ArchiveMemberSelector {
            index,
            name: normalize_member_name(name)?,
        },
        size,
    })
}

fn enumerate_zip(path: &Utf8Path, maximum_entries: Option<usize>) -> Result<Vec<ArchiveMember>> {
    let central_entry_count = zip_central_entry_count(path)?.ok_or_else(|| {
        Error::InvalidPath(format!(
            "cannot validate ZIP central directory count: {path}"
        ))
    })?;
    if maximum_entries.is_some_and(|maximum| central_entry_count > maximum) {
        return Err(Error::InvalidPath(format!(
            "archive exceeds the {MAX_SERVING_ARCHIVE_ENTRIES}-entry serving limit"
        )));
    }
    let mut archive = zip::ZipArchive::new(File::open(path)?)?;
    if central_entry_count != archive.len() {
        return Err(Error::InvalidPath(format!(
            "ZIP archive {path} contains duplicate raw filenames that the pinned reader cannot address unambiguously"
        )));
    }
    let mut members = Vec::new();
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        if entry.is_dir() {
            continue;
        }
        members.push(archive_member(index, entry.name(), entry.size())?);
    }
    Ok(members)
}

fn zip_central_entry_count(path: &Utf8Path) -> Result<Option<usize>> {
    let file = File::open(path)?;
    zip_central_entry_count_from_file(&file)
}

pub fn zip_central_entry_count_from_file(file: &File) -> Result<Option<usize>> {
    const EOCD_SIGNATURE: &[u8; 4] = b"PK\x05\x06";
    const MAX_EOCD_TAIL: usize = 22 + u16::MAX as usize;

    let mut file = file.try_clone()?;
    let file_len = file.metadata()?.len();
    let tail_len = usize::try_from(file_len.min(MAX_EOCD_TAIL as u64))
        .map_err(|_| Error::InvalidPath("ZIP trailer is too large".to_owned()))?;
    let tail_offset = i64::try_from(tail_len)
        .map_err(|_| Error::InvalidPath("ZIP trailer offset is too large".to_owned()))?;
    file.seek(SeekFrom::End(-tail_offset))?;
    let mut tail = vec![0; tail_len];
    file.read_exact(&mut tail)?;
    let Some(eocd) = tail
        .windows(4)
        .enumerate()
        .rev()
        .find_map(|(index, signature)| {
            if signature != EOCD_SIGNATURE || index + 22 > tail.len() {
                return None;
            }
            let comment_len = usize::from(u16::from_le_bytes([tail[index + 20], tail[index + 21]]));
            (index + 22 + comment_len == tail.len()).then_some(index)
        })
    else {
        return Ok(None);
    };
    let (directory_end, directory_size) =
        zip_central_directory_location(&mut file, &tail, eocd, file_len)?;
    let Some((directory_end, directory_size)) = directory_end.zip(directory_size) else {
        return Ok(None);
    };
    let directory_start = directory_end
        .checked_sub(directory_size)
        .ok_or_else(|| Error::InvalidPath("ZIP central directory offset underflowed".to_owned()))?;
    count_zip_central_entries(&mut file, directory_start, directory_size).map(Some)
}

fn zip_central_directory_location(
    file: &mut File,
    tail: &[u8],
    eocd: usize,
    file_len: u64,
) -> Result<(Option<u64>, Option<u64>)> {
    const ZIP64_LOCATOR_SIGNATURE: &[u8; 4] = b"PK\x06\x07";
    const ZIP64_EOCD_SIGNATURE: &[u8; 4] = b"PK\x06\x06";

    let eocd_offset = file_len
        .checked_sub(u64::try_from(tail.len()).map_err(|_| {
            Error::InvalidPath("ZIP trailer offset exceeds platform limits".to_owned())
        })?)
        .and_then(|offset| offset.checked_add(u64::try_from(eocd).ok()?))
        .ok_or_else(|| Error::InvalidPath("ZIP end record offset overflowed".to_owned()))?;
    let entries_on_disk = u16::from_le_bytes([tail[eocd + 8], tail[eocd + 9]]);
    let entries = u16::from_le_bytes([tail[eocd + 10], tail[eocd + 11]]);
    let size32 = u32::from_le_bytes(
        tail[eocd + 12..eocd + 16]
            .try_into()
            .map_err(|_| Error::InvalidPath("invalid ZIP end record".to_owned()))?,
    );
    let offset32 = u32::from_le_bytes(
        tail[eocd + 16..eocd + 20]
            .try_into()
            .map_err(|_| Error::InvalidPath("invalid ZIP end record".to_owned()))?,
    );
    let requires_zip64 = entries_on_disk == u16::MAX
        || entries == u16::MAX
        || size32 == u32::MAX
        || offset32 == u32::MAX;
    let locator_offset = eocd_offset.checked_sub(20);
    let zip64_location =
        if let Some(locator_offset) = locator_offset {
            file.seek(SeekFrom::Start(locator_offset))?;
            let mut locator = [0_u8; 20];
            file.read_exact(&mut locator)?;
            if &locator[..4] == ZIP64_LOCATOR_SIGNATURE {
                let zip64_offset = u64::from_le_bytes(
                    locator[8..16]
                        .try_into()
                        .map_err(|_| Error::InvalidPath("invalid ZIP64 locator".to_owned()))?,
                );
                let mut zip64_eocd = [0_u8; 56];
                if zip64_offset
                    .checked_add(56)
                    .is_some_and(|end| end <= file_len)
                {
                    file.seek(SeekFrom::Start(zip64_offset))?;
                    file.read_exact(&mut zip64_eocd)?;
                } else {
                    return Ok((None, None));
                }
                if &zip64_eocd[..4] != ZIP64_EOCD_SIGNATURE {
                    return Ok((None, None));
                }
                let record_size =
                    u64::from_le_bytes(zip64_eocd[4..12].try_into().map_err(|_| {
                        Error::InvalidPath("invalid ZIP64 end record size".to_owned())
                    })?);
                let record_end = zip64_offset
                    .checked_add(12)
                    .and_then(|start| start.checked_add(record_size));
                if record_size < 44 || record_end != Some(locator_offset) {
                    return Ok((None, None));
                }
                let zip64_entries_on_disk = u64::from_le_bytes(
                    zip64_eocd[24..32]
                        .try_into()
                        .map_err(|_| Error::InvalidPath("invalid ZIP64 entry count".to_owned()))?,
                );
                let zip64_entries = u64::from_le_bytes(
                    zip64_eocd[32..40]
                        .try_into()
                        .map_err(|_| Error::InvalidPath("invalid ZIP64 entry count".to_owned()))?,
                );
                let size =
                    u64::from_le_bytes(zip64_eocd[40..48].try_into().map_err(|_| {
                        Error::InvalidPath("invalid ZIP64 directory size".to_owned())
                    })?);
                let offset = u64::from_le_bytes(zip64_eocd[48..56].try_into().map_err(|_| {
                    Error::InvalidPath("invalid ZIP64 directory offset".to_owned())
                })?);
                if (entries_on_disk != u16::MAX
                    && u64::from(entries_on_disk) != zip64_entries_on_disk)
                    || (entries != u16::MAX && u64::from(entries) != zip64_entries)
                    || (size32 != u32::MAX && u64::from(size32) != size)
                    || (offset32 != u32::MAX && u64::from(offset32) != offset)
                {
                    return Ok((None, None));
                }
                Some((zip64_offset, size))
            } else {
                None
            }
        } else {
            None
        };
    let (directory_end, directory_size) = match zip64_location {
        Some(location) => location,
        None if requires_zip64 => return Ok((None, None)),
        None => (eocd_offset, u64::from(size32)),
    };
    Ok((Some(directory_end), Some(directory_size)))
}

fn count_zip_central_entries(file: &mut File, start: u64, size: u64) -> Result<usize> {
    const CENTRAL_FILE_HEADER_SIGNATURE: &[u8; 4] = b"PK\x01\x02";

    let directory_size = size;
    file.seek(SeekFrom::Start(start))?;

    let mut consumed = 0_u64;
    let mut count = 0_usize;
    while directory_size.saturating_sub(consumed) >= 4 {
        let mut signature = [0_u8; 4];
        file.read_exact(&mut signature)?;
        if &signature != CENTRAL_FILE_HEADER_SIGNATURE {
            break;
        }
        if directory_size.saturating_sub(consumed) < 46 {
            return Err(Error::InvalidPath(
                "truncated ZIP central directory".to_owned(),
            ));
        }
        let mut header = [0_u8; 42];
        file.read_exact(&mut header)?;
        let name_len = u64::from(u16::from_le_bytes([header[24], header[25]]));
        let extra_len = u64::from(u16::from_le_bytes([header[26], header[27]]));
        let comment_len = u64::from(u16::from_le_bytes([header[28], header[29]]));
        let record_len = 46_u64
            .checked_add(name_len)
            .and_then(|length| length.checked_add(extra_len))
            .and_then(|length| length.checked_add(comment_len))
            .ok_or_else(|| Error::InvalidPath("ZIP central record length overflowed".to_owned()))?;
        consumed = consumed.checked_add(record_len).ok_or_else(|| {
            Error::InvalidPath("ZIP central directory length overflowed".to_owned())
        })?;
        if consumed > directory_size {
            return Err(Error::InvalidPath(
                "truncated ZIP central directory".to_owned(),
            ));
        }
        file.seek(SeekFrom::Current(
            i64::try_from(name_len + extra_len + comment_len)
                .map_err(|_| Error::InvalidPath("ZIP central record is too large".to_owned()))?,
        ))?;
        count = count
            .checked_add(1)
            .ok_or_else(|| Error::InvalidPath("ZIP entry count overflowed".to_owned()))?;
    }
    Ok(count)
}

fn enumerate_7z(path: &Utf8Path, maximum_entries: Option<usize>) -> Result<Vec<ArchiveMember>> {
    let _decoder_permit = maximum_entries
        .map(|_| reserve_serving_decoder())
        .transpose()?;
    let archive = match maximum_entries {
        Some(_) => open_7z_for_serving(path)?,
        None => r7z::Archive::open(path.as_std_path())?,
    };
    if maximum_entries.is_some_and(|maximum| archive.num_files() > maximum) {
        return Err(Error::InvalidPath(format!(
            "archive exceeds the {MAX_SERVING_ARCHIVE_ENTRIES}-entry serving limit"
        )));
    }
    let entries = archive.entries().collect::<Vec<_>>();
    let listing = archive.listing(None)?;
    entries
        .into_iter()
        .filter(r7z::ArchiveEntryInfo::is_file)
        .map(|entry| {
            let size = listing
                .entries
                .get(entry.index)
                .and_then(|listed| listed.size)
                .ok_or_else(|| Error::InvalidPath("7z member size is unavailable".to_owned()))?;
            archive_member(entry.index, &entry.name, size)
        })
        .collect()
}

fn enumerate_rar(path: &Utf8Path, maximum_entries: Option<usize>) -> Result<Vec<ArchiveMember>> {
    let mut archive = unrar::Archive::new(path.as_std_path()).open_for_processing()?;
    let mut members = Vec::new();
    let mut index = 0;
    while let Some(header) = archive.read_header()? {
        if maximum_entries.is_some_and(|maximum| index >= maximum) {
            return Err(Error::InvalidPath(format!(
                "archive exceeds the {MAX_SERVING_ARCHIVE_ENTRIES}-entry serving limit"
            )));
        }
        if header.entry().is_file() {
            let name = header.entry().filename.to_str().ok_or_else(|| {
                Error::InvalidPath("RAR member name is not valid UTF-8".to_owned())
            })?;
            members.push(archive_member(index, name, header.entry().unpacked_size)?);
        }
        archive = header.skip()?;
        index += 1;
    }
    Ok(members)
}

fn stream_zip<F>(path: &Utf8Path, members: &[ArchiveMember], callback: &mut F) -> Result<()>
where
    F: FnMut(&ArchiveMember, &mut dyn Read) -> Result<()>,
{
    let mut archive = zip::ZipArchive::new(File::open(path)?)?;
    for member in members {
        let mut entry = archive.by_index(member.selector.index)?;
        let actual_name = normalize_member_name(entry.name())?;
        if !entry.is_file() || actual_name != member.selector.name || entry.size() != member.size {
            return Err(Error::InvalidPath(format!(
                "ZIP member changed after enumeration at index {}: {}",
                member.selector.index, member.selector.name
            )));
        }
        let mut reader: &mut dyn Read = &mut entry;
        callback(member, &mut reader)?;
        io::copy(&mut reader, &mut io::sink())?;
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SevenZipReadMode {
    Existing,
    Serving,
}

fn stream_7z<F>(path: &Utf8Path, members: &[ArchiveMember], callback: &mut F) -> Result<()>
where
    F: FnMut(&ArchiveMember, &mut dyn Read) -> Result<()>,
{
    stream_7z_with_mode(path, members, callback, SevenZipReadMode::Existing)
}

fn stream_7z_for_serving<F>(
    path: &Utf8Path,
    members: &[ArchiveMember],
    callback: &mut F,
) -> Result<()>
where
    F: FnMut(&ArchiveMember, &mut dyn Read) -> Result<()>,
{
    stream_7z_with_mode(path, members, callback, SevenZipReadMode::Serving)
}

fn stream_7z_with_mode<F>(
    path: &Utf8Path,
    members: &[ArchiveMember],
    callback: &mut F,
    mode: SevenZipReadMode,
) -> Result<()>
where
    F: FnMut(&ArchiveMember, &mut dyn Read) -> Result<()>,
{
    for member in members {
        if member.size > SEVEN_Z_MAX_SELECTED_MEMBER_SIZE {
            return Err(Error::InvalidPath(format!(
                "7z member {} exceeds the 16 GiB selected-member limit",
                member.selector.name
            )));
        }
    }
    if members.is_empty() {
        return Ok(());
    }
    let _decoder_permit = (mode == SevenZipReadMode::Serving)
        .then(reserve_serving_decoder)
        .transpose()?;
    let archive = match mode {
        SevenZipReadMode::Existing => r7z::Archive::open(path.as_std_path())?,
        SevenZipReadMode::Serving => open_7z_for_serving(path)?,
    };
    let listing = archive.listing(None)?;
    let selected_indices = members
        .iter()
        .map(|member| member.selector.index)
        .collect::<Vec<_>>();
    if mode == SevenZipReadMode::Serving {
        preflight_7z_prerequisite_output(&listing.entries, &selected_indices)?;
        preflight_7z_packed_folders(&archive, &listing, &selected_indices)?;
    }
    let selected = members
        .iter()
        .map(|member| (member.selector.index, member))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut callback_error = None;
    let mut visited = BTreeSet::new();
    let result = archive.stream_selected_files(&selected_indices, |entry, reader| {
        let Some(member) = selected.get(&entry.index) else {
            return Err(r7z::R7zError::Parse);
        };
        let name = normalize_member_name(&entry.name)
            .map_err(|_| r7z::R7zError::UnsafePath(entry.name.clone()))?;
        if member.selector.name != name {
            return Err(r7z::R7zError::UnsafePath(entry.name.clone()));
        }
        let mut bounded = BoundedMemberReader::new(reader, member.size);
        if let Err(error) = callback(member, &mut bounded) {
            callback_error = Some(error);
            return Err(r7z::R7zError::Io(io::Error::other(
                "source callback failed",
            )));
        }
        io::copy(&mut bounded, &mut io::sink()).map_err(r7z::R7zError::Io)?;
        if bounded.read != member.size {
            return Err(r7z::R7zError::Io(io::Error::other(
                "7z member size changed while reading",
            )));
        }
        visited.insert(member.selector.index);
        Ok(())
    });
    if let Some(error) = callback_error {
        return Err(error);
    }
    result?;
    if visited.len() != members.len() {
        return Err(Error::InvalidPath(
            "a selected 7z member disappeared after enumeration".to_owned(),
        ));
    }
    Ok(())
}

fn open_7z_for_serving(path: &Utf8Path) -> Result<r7z::Archive> {
    Ok(r7z::Archive::open_with_options(
        path.as_std_path(),
        r7z::ArchiveOpenOptions {
            storage_mode: r7z::ArchiveStorageMode::Seek,
            ..r7z::ArchiveOpenOptions::default()
        },
    )?)
}

fn preflight_7z_packed_folders(
    archive: &r7z::Archive,
    listing: &r7z::ArchiveListing,
    selected_indices: &[usize],
) -> Result<()> {
    let blocks = selected_indices
        .iter()
        .filter_map(|&index| listing.entries.get(index).and_then(|entry| entry.block))
        .collect::<BTreeSet<_>>();
    let mut packed_sizes_by_block = BTreeMap::new();
    for entry in &listing.entries {
        let (Some(block), Some(size)) = (entry.block, entry.packed_size) else {
            continue;
        };
        if blocks.contains(&block) {
            packed_sizes_by_block.entry(block).or_insert(size);
        }
    }

    let unpack_info = archive
        .streams_info()
        .and_then(|streams| streams.unpack_info.as_ref());
    for block in blocks {
        let unpack_info = unpack_info.ok_or_else(|| {
            Error::InvalidPath("7z packed folder metadata is unavailable".to_owned())
        })?;
        let folder = unpack_info.parse_folder(block)?;
        let input_streams = folder.coders.iter().try_fold(0_u64, |total, coder| {
            total
                .checked_add(coder.num_in_streams)
                .ok_or_else(|| Error::InvalidPath("7z packed stream count overflowed".to_owned()))
        })?;
        let bound_streams = u64::try_from(folder.bind_pairs.len())
            .map_err(|_| Error::InvalidPath("7z bound stream count overflowed".to_owned()))?;
        let packed_streams = input_streams
            .checked_sub(bound_streams)
            .ok_or_else(|| Error::InvalidPath("7z packed stream count underflowed".to_owned()))?;
        if packed_streams <= 1 {
            continue;
        }
        let packed_size = packed_sizes_by_block.get(&block).copied().ok_or_else(|| {
            Error::InvalidPath(format!(
                "7z packed size is unavailable for folder {block}; refusing unbounded decode"
            ))
        })?;
        enforce_7z_packed_folder_limit(block, packed_streams, packed_size)?;
    }
    Ok(())
}

fn preflight_7z_prerequisite_output(
    entries: &[r7z::ArchiveListingEntry],
    selected_indices: &[usize],
) -> Result<()> {
    let selected = selected_indices.iter().copied().collect::<BTreeSet<_>>();
    let selected_blocks = selected_indices
        .iter()
        .filter_map(|&index| entries.get(index).and_then(|entry| entry.block))
        .collect::<BTreeSet<_>>();
    let mut total = 0_u64;
    for entry in entries {
        if !entry
            .block
            .is_some_and(|block| selected_blocks.contains(&block))
            || selected.contains(&entry.index)
        {
            continue;
        }
        let size = entry.size.ok_or_else(|| {
            Error::InvalidPath("7z solid prerequisite size is unavailable".to_owned())
        })?;
        if size > SEVEN_Z_MAX_PREREQUISITE_MEMBER_SIZE {
            return Err(Error::InvalidPath(format!(
                "7z solid prerequisite exceeds the {} MiB per-member decode limit",
                SEVEN_Z_MAX_PREREQUISITE_MEMBER_SIZE / (1024 * 1024)
            )));
        }
        total = total.checked_add(size).ok_or_else(|| {
            Error::InvalidPath("7z solid prerequisite byte count overflowed".to_owned())
        })?;
        if total > SEVEN_Z_MAX_PREREQUISITE_OUTPUT_SIZE {
            return Err(Error::InvalidPath(format!(
                "7z solid prerequisites exceed the {} MiB aggregate decode limit",
                SEVEN_Z_MAX_PREREQUISITE_OUTPUT_SIZE / (1024 * 1024)
            )));
        }
    }
    Ok(())
}

fn enforce_7z_packed_folder_limit(
    block: usize,
    packed_streams: u64,
    packed_size: u64,
) -> Result<()> {
    if packed_streams > 1 && packed_size > SEVEN_Z_MAX_PACKED_FOLDER_SIZE {
        return Err(Error::InvalidPath(format!(
            "7z packed folder {block} requires {packed_size} bytes; limit is {SEVEN_Z_MAX_PACKED_FOLDER_SIZE}"
        )));
    }
    Ok(())
}

struct BoundedMemberReader<'a> {
    reader: &'a mut dyn Read,
    read: u64,
    limit: u64,
}

impl<'a> BoundedMemberReader<'a> {
    const fn new(reader: &'a mut dyn Read, limit: u64) -> Self {
        Self {
            reader,
            read: 0,
            limit,
        }
    }
}

impl Read for BoundedMemberReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let remaining = self.limit.saturating_sub(self.read);
        if remaining == 0 {
            let mut extra = [0; 1];
            return match self.reader.read(&mut extra)? {
                0 => Ok(0),
                _ => Err(io::Error::other("7z member exceeded declared size")),
            };
        }
        let allowed = usize::try_from(remaining).unwrap_or(usize::MAX);
        let read_len = buffer.len().min(allowed);
        let read = self.reader.read(&mut buffer[..read_len])?;
        self.read = self
            .read
            .checked_add(u64::try_from(read).map_err(io::Error::other)?)
            .ok_or_else(|| io::Error::other("7z member byte count overflowed"))?;
        Ok(read)
    }
}

fn stream_rar<F>(path: &Utf8Path, members: &[ArchiveMember], callback: &mut F) -> Result<()>
where
    F: FnMut(&ArchiveMember, &mut dyn Read) -> Result<()>,
{
    let max_member_size = capabilities(ArchiveBackend::Rar)
        .max_member_size
        .ok_or_else(|| Error::InvalidPath("RAR member limit is not configured".to_owned()))?;
    for member in members {
        if member.size > max_member_size {
            return Err(Error::InvalidPath(format!(
                "RAR member {} exceeds the {} MiB extraction limit",
                member.selector.name,
                max_member_size / (1024 * 1024)
            )));
        }
    }
    let selected_members = members
        .iter()
        .map(|member| (member.selector.index, member))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut visited = BTreeSet::new();
    let mut archive = unrar::Archive::new(path.as_std_path()).open_for_processing()?;
    let mut index = 0;
    while let Some(header) = archive.read_header()? {
        if let Some(member) = selected_members
            .get(&index)
            .filter(|_| header.entry().is_file())
        {
            let member = *member;
            let name = header.entry().filename.to_str().ok_or_else(|| {
                Error::InvalidPath("RAR member name is not valid UTF-8".to_owned())
            })?;
            if normalize_member_name(name)? != member.selector.name {
                return Err(Error::InvalidPath("RAR selector name changed".to_owned()));
            }
            if header.entry().unpacked_size != member.size
                || header.entry().unpacked_size > max_member_size
            {
                return Err(Error::InvalidPath(format!(
                    "RAR member changed after enumeration or exceeds the extraction limit: {}",
                    member.selector.name
                )));
            }
            let temp_dir = crate::private_temp::PrivateTempDir::create("mame-coalesce-rar-")?;
            let output_path = temp_dir.path().join("member.data");
            archive = header.extract_to(&output_path)?;
            let actual_size = fs::metadata(&output_path)?.len();
            if actual_size > max_member_size {
                return Err(Error::InvalidPath(format!(
                    "RAR member {} exceeds the {} MiB extraction limit",
                    member.selector.name,
                    max_member_size / (1024 * 1024)
                )));
            }
            let mut reader: &mut dyn Read = &mut File::open(output_path)?;
            callback(member, &mut reader)?;
            io::copy(&mut reader, &mut io::sink())?;
            visited.insert(index);
        } else {
            archive = header.skip()?;
        }
        index += 1;
    }
    if visited.len() != members.len() {
        return Err(Error::InvalidPath(
            "a selected RAR member disappeared after enumeration".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};

    use crate::Error;
    use camino::Utf8Path;
    use zip::{ZipWriter, write::SimpleFileOptions};

    use super::{
        ArchiveBackend, ArchiveMember, ArchiveMemberSelector, MAX_ACTIVE_ARCHIVE_DECODERS,
        RAR_MAX_DICTIONARY_SIZE, RAR_MAX_MEMBER_SIZE, RAR_MAX_PREREQUISITE_MEMBER_SIZE,
        RAR_MAX_PREREQUISITE_OUTPUT_SIZE, SEVEN_Z_MAX_DECODER_WORKING_SET_SIZE,
        SEVEN_Z_MAX_DICTIONARY_SIZE, SEVEN_Z_MAX_PACKED_FOLDER_SIZE,
        SEVEN_Z_MAX_PREREQUISITE_MEMBER_SIZE, SEVEN_Z_MAX_PREREQUISITE_OUTPUT_SIZE, SourceKind,
        capabilities, detect, enforce_7z_packed_folder_limit, enumerate, enumerate_with_limit,
        enumeration_error, preflight_7z_prerequisite_output, stream_7z, stream_archive,
        stream_file, stream_rar, stream_zip, zip_central_entry_count,
    };

    #[cfg(unix)]
    #[test]
    fn rar_extraction_directory_is_private() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;

        let directory = crate::private_temp::PrivateTempDir::create("mame-coalesce-test-")?;
        let mode = std::fs::metadata(directory.path())?.permissions().mode();
        assert_eq!(mode & 0o077, 0);
        Ok(())
    }

    fn write_zip(path: &Utf8Path, entries: &[(&str, &[u8])]) -> io::Result<()> {
        let mut zip = ZipWriter::new(std::fs::File::create(path)?);
        for (name, data) in entries {
            zip.start_file(*name, SimpleFileOptions::default())?;
            zip.write_all(data)?;
        }
        zip.finish()?;
        Ok(())
    }

    #[test]
    fn serving_enumeration_rejects_an_archive_over_its_entry_limit()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let archive = Utf8Path::from_path(temp.path())
            .ok_or_else(|| io::Error::other("temporary path is not UTF-8"))?
            .join("one-entry.zip");
        write_zip(&archive, &[("game.rom", b"content")])?;

        let Err(error) = enumerate_with_limit(&archive, ArchiveBackend::Zip, Some(0)) else {
            return Err("the serving entry limit was not checked".into());
        };
        assert!(error.to_string().contains("entry serving limit"));

        let seven_zip = Utf8Path::from_path(temp.path())
            .ok_or_else(|| io::Error::other("temporary path is not UTF-8"))?
            .join("one-entry.7z");
        std::fs::write(
            &seven_zip,
            r7z::ArchiveBuilder::new()
                .add_file("game.rom", b"content")
                .build()?,
        )?;
        let Err(error) = enumerate_with_limit(&seven_zip, ArchiveBackend::SevenZip, Some(0)) else {
            return Err("the 7z serving entry limit was not checked".into());
        };
        assert!(error.to_string().contains("entry serving limit"));
        Ok(())
    }

    #[test]
    fn serving_enumeration_preserves_decoder_capacity_classification() {
        let error = enumeration_error(
            Error::ArchiveDecoderLimitExceeded {
                maximum: MAX_ACTIVE_ARCHIVE_DECODERS,
            },
            ArchiveBackend::SevenZip,
            Utf8Path::new("game.7z"),
        );
        assert!(matches!(error, Error::ArchiveDecoderLimitExceeded { .. }));
        assert_eq!(
            crate::serving::ServingError::Source(error).failure_kind(),
            crate::serving::ServingFailureKind::Capacity
        );
    }

    #[test]
    fn rar_decoded_output_is_bounded_even_when_the_writer_is_not() {
        let mut output = Vec::new();
        {
            let mut writer = super::BoundedMemberWriter::new(&mut output, 3);
            assert!(writer.write_all(b"four").is_err());
            assert_eq!(writer.written, 3);
        }
        assert_eq!(output, b"fou");
        assert_eq!(
            capabilities(ArchiveBackend::Rar).max_prerequisite_member_size,
            Some(RAR_MAX_PREREQUISITE_MEMBER_SIZE)
        );
        assert_eq!(
            capabilities(ArchiveBackend::Rar).max_prerequisite_output_size,
            Some(RAR_MAX_PREREQUISITE_OUTPUT_SIZE)
        );
    }

    #[test]
    fn archive_decoder_slots_bound_decoder_memory() -> Result<(), Box<dyn std::error::Error>> {
        let slots = super::DecoderSlots::new(2);
        let first = slots
            .try_acquire()
            .ok_or_else(|| io::Error::other("first archive decoder slot was unavailable"))?;
        let _second = slots
            .try_acquire()
            .ok_or_else(|| io::Error::other("second archive decoder slot was unavailable"))?;
        assert!(slots.try_acquire().is_none());
        drop(first);
        assert!(slots.try_acquire().is_some());
        let rar = capabilities(ArchiveBackend::Rar);
        assert_eq!(
            rar.max_concurrent_decoders,
            Some(MAX_ACTIVE_ARCHIVE_DECODERS)
        );
        assert_eq!(
            rar.max_decoder_dictionary_size,
            Some(RAR_MAX_DICTIONARY_SIZE)
        );
        let seven_zip = capabilities(ArchiveBackend::SevenZip);
        assert_eq!(
            seven_zip.max_concurrent_decoders,
            Some(MAX_ACTIVE_ARCHIVE_DECODERS)
        );
        assert_eq!(
            seven_zip.max_decoder_dictionary_size,
            Some(SEVEN_Z_MAX_DICTIONARY_SIZE)
        );
        assert_eq!(
            seven_zip.max_decoder_working_set_size,
            Some(SEVEN_Z_MAX_DECODER_WORKING_SET_SIZE)
        );
        Ok(())
    }

    #[test]
    fn seven_zip_solid_prerequisites_have_per_member_and_aggregate_limits() {
        let listing = |index, block, size| r7z::ArchiveListingEntry {
            index,
            path: format!("member-{index}"),
            kind: r7z::ListingEntryKind::File,
            size: Some(size),
            packed_size: Some(1),
            modified: None,
            attributes: None,
            crc: None,
            encrypted: false,
            methods: Vec::new(),
            block: Some(block),
        };
        let too_large_member = [
            listing(0, 0, SEVEN_Z_MAX_PREREQUISITE_MEMBER_SIZE + 1),
            listing(1, 0, 1),
        ];
        assert!(preflight_7z_prerequisite_output(&too_large_member, &[1]).is_err());

        let too_large_total = [
            listing(0, 0, 400 * 1024 * 1024),
            listing(1, 0, 1),
            listing(2, 0, 400 * 1024 * 1024),
            listing(3, 0, 225 * 1024 * 1024),
        ];
        assert!(preflight_7z_prerequisite_output(&too_large_total, &[1]).is_err());

        let other_folder_is_irrelevant = [
            listing(0, 0, SEVEN_Z_MAX_PREREQUISITE_MEMBER_SIZE + 1),
            listing(1, 1, 1),
        ];
        assert!(preflight_7z_prerequisite_output(&other_folder_is_irrelevant, &[1]).is_ok());
        assert_eq!(
            capabilities(ArchiveBackend::SevenZip).max_prerequisite_output_size,
            Some(SEVEN_Z_MAX_PREREQUISITE_OUTPUT_SIZE)
        );
    }

    fn zip64_with_max_comment(bytes: &[u8], saturated_eocd_fields: bool) -> io::Result<Vec<u8>> {
        let eocd = bytes
            .windows(4)
            .rposition(|window| window == b"PK\x05\x06")
            .ok_or_else(|| io::Error::other("ZIP end record was not written"))?;
        let entries = u16::from_le_bytes([bytes[eocd + 10], bytes[eocd + 11]]);
        let directory_size = u32::from_le_bytes(
            bytes[eocd + 12..eocd + 16]
                .try_into()
                .map_err(|_| io::Error::other("invalid ZIP end record"))?,
        );
        let directory_offset = u32::from_le_bytes(
            bytes[eocd + 16..eocd + 20]
                .try_into()
                .map_err(|_| io::Error::other("invalid ZIP end record"))?,
        );

        let mut result = bytes[..eocd].to_vec();
        let zip64_offset = u64::try_from(result.len())
            .map_err(|_| io::Error::other("ZIP64 end record offset overflowed"))?;
        result.extend_from_slice(b"PK\x06\x06");
        result.extend_from_slice(&44_u64.to_le_bytes());
        result.extend_from_slice(&45_u16.to_le_bytes());
        result.extend_from_slice(&45_u16.to_le_bytes());
        result.extend_from_slice(&0_u32.to_le_bytes());
        result.extend_from_slice(&0_u32.to_le_bytes());
        result.extend_from_slice(&u64::from(entries).to_le_bytes());
        result.extend_from_slice(&u64::from(entries).to_le_bytes());
        result.extend_from_slice(&u64::from(directory_size).to_le_bytes());
        result.extend_from_slice(&u64::from(directory_offset).to_le_bytes());

        result.extend_from_slice(b"PK\x06\x07");
        result.extend_from_slice(&0_u32.to_le_bytes());
        result.extend_from_slice(&zip64_offset.to_le_bytes());
        result.extend_from_slice(&1_u32.to_le_bytes());

        let mut standard_eocd = bytes[eocd..eocd + 22].to_vec();
        if saturated_eocd_fields {
            standard_eocd[8..12].fill(u8::MAX);
            standard_eocd[12..20].fill(u8::MAX);
        }
        standard_eocd[20..22].copy_from_slice(&u16::MAX.to_le_bytes());
        result.extend_from_slice(&standard_eocd);
        result.resize(result.len() + usize::from(u16::MAX), 0xA5);
        Ok(result)
    }

    #[test]
    fn seven_zip_packed_folder_preflight_limits_the_decoded_folders() {
        assert!(enforce_7z_packed_folder_limit(0, 2, 4).is_ok());
        assert!(enforce_7z_packed_folder_limit(0, 2, SEVEN_Z_MAX_PACKED_FOLDER_SIZE).is_ok());
        assert!(enforce_7z_packed_folder_limit(1, 2, SEVEN_Z_MAX_PACKED_FOLDER_SIZE + 1).is_err());
        assert!(enforce_7z_packed_folder_limit(1, 1, SEVEN_Z_MAX_PACKED_FOLDER_SIZE + 1).is_ok());
    }

    #[test]
    fn zip64_locator_is_read_before_maximum_length_comment()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let root = Utf8Path::from_path(directory.path())
            .ok_or_else(|| io::Error::other("temporary path is not UTF-8"))?;
        let path = root.join("zip64-max-comment.zip");
        let ordinary = root.join("ordinary.zip");
        write_zip(&ordinary, &[("one.rom", b"rom")])?;
        std::fs::write(
            &path,
            zip64_with_max_comment(&std::fs::read(ordinary)?, true)?,
        )?;

        assert_eq!(zip_central_entry_count(&path)?, Some(1));
        let members = enumerate(&path, ArchiveBackend::Zip)?;
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].selector.name, "one.rom");
        Ok(())
    }

    #[test]
    fn zip64_locator_is_used_when_ordinary_end_fields_fit() -> Result<(), Box<dyn std::error::Error>>
    {
        let directory = tempfile::tempdir()?;
        let root = Utf8Path::from_path(directory.path())
            .ok_or_else(|| io::Error::other("temporary path is not UTF-8"))?;
        let path = root.join("zip64-unsaturated.zip");
        let ordinary = root.join("ordinary.zip");
        write_zip(&ordinary, &[("one.rom", b"rom")])?;
        std::fs::write(
            &path,
            zip64_with_max_comment(&std::fs::read(ordinary)?, false)?,
        )?;

        assert_eq!(zip_central_entry_count(&path)?, Some(1));
        let members = enumerate(&path, ArchiveBackend::Zip)?;
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].selector.name, "one.rom");
        Ok(())
    }

    #[test]
    fn rejects_inconsistent_optional_zip64_directory_metadata()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let root = Utf8Path::from_path(directory.path())
            .ok_or_else(|| io::Error::other("temporary path is not UTF-8"))?;
        let ordinary = root.join("duplicates.zip");
        let malformed = root.join("malformed-zip64.zip");
        write_zip(&ordinary, &[("first.rom", b"one"), ("other.rom", b"two")])?;
        let mut bytes = std::fs::read(ordinary)?;
        let mut offset = 0;
        while let Some(relative) = bytes[offset..]
            .windows(b"other.rom".len())
            .position(|window| window == b"other.rom")
        {
            offset += relative;
            bytes[offset..offset + b"other.rom".len()].copy_from_slice(b"first.rom");
            offset += b"other.rom".len();
        }
        let eocd = bytes
            .windows(4)
            .rposition(|window| window == b"PK\x05\x06")
            .ok_or_else(|| io::Error::other("ZIP end record was not written"))?;
        let directory_size = u32::from_le_bytes(
            bytes[eocd + 12..eocd + 16]
                .try_into()
                .map_err(|_| io::Error::other("invalid ZIP end record"))?,
        );
        let first_central = u32::from_le_bytes(
            bytes[eocd + 16..eocd + 20]
                .try_into()
                .map_err(|_| io::Error::other("invalid ZIP end record"))?,
        ) as usize;
        let first_name_len = usize::from(u16::from_le_bytes(
            bytes[first_central + 28..first_central + 30]
                .try_into()
                .map_err(|_| io::Error::other("invalid central directory entry"))?,
        ));
        let first_extra_len = usize::from(u16::from_le_bytes(
            bytes[first_central + 30..first_central + 32]
                .try_into()
                .map_err(|_| io::Error::other("invalid central directory entry"))?,
        ));
        let first_comment_len = usize::from(u16::from_le_bytes(
            bytes[first_central + 32..first_central + 34]
                .try_into()
                .map_err(|_| io::Error::other("invalid central directory entry"))?,
        ));
        let first_entry_len = 46 + first_name_len + first_extra_len + first_comment_len;
        let last_entry_len = u64::from(directory_size)
            .checked_sub(u64::try_from(first_entry_len)?)
            .ok_or_else(|| io::Error::other("invalid central directory size"))?;
        let mut bytes = zip64_with_max_comment(&bytes, false)?;
        let zip64_eocd = bytes
            .windows(4)
            .rposition(|window| window == b"PK\x06\x06")
            .ok_or_else(|| io::Error::other("ZIP64 end record was not written"))?;
        bytes[zip64_eocd + 40..zip64_eocd + 48].copy_from_slice(&last_entry_len.to_le_bytes());
        std::fs::write(&malformed, bytes)?;

        assert!(enumerate(&malformed, ArchiveBackend::Zip).is_err());
        Ok(())
    }

    #[test]
    fn detects_bare_and_renamed_archives_by_magic() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let root = Utf8Path::from_path(directory.path())
            .ok_or_else(|| io::Error::other("temporary path is not UTF-8"))?;
        let bare = root.join("looks-like.zip");
        std::fs::write(&bare, b"ordinary file")?;
        assert_eq!(detect(&bare)?, SourceKind::BareFile);

        let zip_path = root.join("renamed.data");
        write_zip(&zip_path, &[("one.rom", b"rom")])?;
        assert_eq!(detect(&zip_path)?, SourceKind::Archive(ArchiveBackend::Zip));
        let mut streamed = Vec::new();
        stream_file(&zip_path, &mut streamed)?;
        assert_eq!(streamed, std::fs::read(&zip_path)?);

        let rar_path = root.join("renamed.rar-data");
        std::fs::write(
            &rar_path,
            hex::decode(
                "526172211a0700cf907300000d000000000000000f0c7420802700150000000b0000000345f37dc6a48a07471d330700a481000056455253494f4e0c008fec8a45cc23c848088362fe5fdd5c5388f072c43d7b00400700",
            )?,
        )?;
        assert_eq!(detect(&rar_path)?, SourceKind::Archive(ArchiveBackend::Rar));

        let seven_zip_path = root.join("renamed.7z-data");
        std::fs::write(
            &seven_zip_path,
            r7z::ArchiveBuilder::new()
                .add_file("one.rom", b"rom")
                .build()?,
        )?;
        assert_eq!(
            detect(&seven_zip_path)?,
            SourceKind::Archive(ArchiveBackend::SevenZip)
        );
        Ok(())
    }

    #[test]
    fn seven_zip_and_rar_enumeration_select_and_stream_the_same_member()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let root = Utf8Path::from_path(directory.path())
            .ok_or_else(|| io::Error::other("temporary path is not UTF-8"))?;

        let seven_zip = root.join("renamed.archive");
        std::fs::write(
            &seven_zip,
            r7z::ArchiveBuilder::new()
                .add_file("nested/game.rom", b"seven-zip bytes")
                .build()?,
        )?;
        let seven_zip_members = enumerate(&seven_zip, ArchiveBackend::SevenZip)?;
        let mut seven_zip_bytes = Vec::new();
        stream_archive(
            &seven_zip,
            ArchiveBackend::SevenZip,
            Some(&[seven_zip_members[0].selector.clone()]),
            |member, reader| {
                assert_eq!(member, &seven_zip_members[0]);
                io::copy(reader, &mut seven_zip_bytes)?;
                Ok(())
            },
        )?;
        assert_eq!(seven_zip_bytes, b"seven-zip bytes");

        let rar = root.join("renamed.rar-data");
        std::fs::write(
            &rar,
            hex::decode(
                "526172211a0700cf907300000d000000000000000f0c7420802700150000000b0000000345f37dc6a48a07471d330700a481000056455253494f4e0c008fec8a45cc23c848088362fe5fdd5c5388f072c43d7b00400700",
            )?,
        )?;
        let rar_members = enumerate(&rar, ArchiveBackend::Rar)?;
        let mut rar_bytes = Vec::new();
        stream_archive(
            &rar,
            ArchiveBackend::Rar,
            Some(&[rar_members[0].selector.clone()]),
            |member, reader| {
                assert_eq!(member, &rar_members[0]);
                io::copy(reader, &mut rar_bytes)?;
                Ok(())
            },
        )?;
        assert_eq!(rar_bytes, b"unrar-0.4.0");
        Ok(())
    }

    #[test]
    fn seven_zip_selected_subset_extracts_only_the_requested_indices()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let root = Utf8Path::from_path(directory.path())
            .ok_or_else(|| io::Error::other("temporary path is not UTF-8"))?;
        let path = root.join("selected.7z");
        std::fs::write(
            &path,
            r7z::ArchiveBuilder::new()
                .add_file("first.rom", b"first")
                .add_file("unselected.rom", b"unselected")
                .add_file("last.rom", b"last")
                .build()?,
        )?;
        let inventory = enumerate(&path, ArchiveBackend::SevenZip)?;
        let selected = [inventory[0].selector.clone(), inventory[2].selector.clone()];
        let mut observed = Vec::new();
        stream_archive(
            &path,
            ArchiveBackend::SevenZip,
            Some(&selected),
            |member, reader| {
                let mut bytes = Vec::new();
                io::copy(reader, &mut bytes)?;
                observed.push((member.selector.index, bytes));
                Ok(())
            },
        )?;
        assert_eq!(observed, [(0, b"first".to_vec()), (2, b"last".to_vec())]);
        Ok(())
    }

    #[test]
    fn selected_seven_zip_reads_ignore_corrupt_later_folders()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let root = Utf8Path::from_path(directory.path())
            .ok_or_else(|| io::Error::other("temporary path is not UTF-8"))?;
        let path = root.join("selected.7z");
        let options = r7z::ArchiveOptions {
            codec: r7z::Codec::Copy,
            compression: r7z::CompressionOptions {
                solid: r7z::SolidMode::NonSolid,
                ..Default::default()
            },
            ..Default::default()
        };
        std::fs::write(
            &path,
            r7z::ArchiveBuilder::new()
                .options(options)
                .add_file("first.rom", b"first-selected")
                .add_file("second.rom", b"second-selected")
                .add_file("later.rom", b"corrupt-later-folder")
                .build()?,
        )?;
        let inventory = enumerate(&path, ArchiveBackend::SevenZip)?;
        let mut bytes = std::fs::read(&path)?;
        let later_offset = bytes
            .windows(b"corrupt-later-folder".len())
            .position(|window| window == b"corrupt-later-folder")
            .ok_or_else(|| io::Error::other("stored later member bytes were not found"))?;
        bytes[later_offset] ^= 0xFF;
        std::fs::write(&path, bytes)?;

        let selected = [inventory[0].selector.clone(), inventory[1].selector.clone()];
        let mut observed = Vec::new();
        stream_archive(
            &path,
            ArchiveBackend::SevenZip,
            Some(&selected),
            |member, reader| {
                let mut data = Vec::new();
                io::copy(reader, &mut data)?;
                observed.push((member.selector.index, data));
                Ok(())
            },
        )?;
        assert_eq!(
            observed,
            [
                (0, b"first-selected".to_vec()),
                (1, b"second-selected".to_vec())
            ]
        );
        Ok(())
    }

    #[test]
    fn selected_seven_zip_out_of_range_index_is_an_error() -> Result<(), Box<dyn std::error::Error>>
    {
        let directory = tempfile::tempdir()?;
        let root = Utf8Path::from_path(directory.path())
            .ok_or_else(|| io::Error::other("temporary path is not UTF-8"))?;
        let path = root.join("selected.7z");
        std::fs::write(
            &path,
            r7z::ArchiveBuilder::new()
                .add_file("present.rom", b"present")
                .build()?,
        )?;
        let mut member = enumerate(&path, ArchiveBackend::SevenZip)?[0].clone();
        member.selector.index = 9;
        let Err(error) = stream_7z(&path, &[member], &mut |_, _| Ok(())) else {
            return Err("a selected index that was not visited must fail".into());
        };
        assert!(
            error
                .to_string()
                .contains("selected entry index out of bounds")
        );
        Ok(())
    }

    #[test]
    fn rar_limit_is_checked_before_extraction() -> Result<(), Box<dyn std::error::Error>> {
        let rar_capabilities = capabilities(ArchiveBackend::Rar);
        assert_eq!(rar_capabilities.max_member_size, Some(RAR_MAX_MEMBER_SIZE));
        assert_eq!(
            rar_capabilities.max_prerequisite_member_size,
            Some(RAR_MAX_PREREQUISITE_MEMBER_SIZE)
        );
        assert_eq!(
            rar_capabilities.max_prerequisite_output_size,
            Some(RAR_MAX_PREREQUISITE_OUTPUT_SIZE)
        );
        assert!(rar_capabilities.limitation.contains("Solid prerequisites"));
        let directory = tempfile::tempdir()?;
        let root = Utf8Path::from_path(directory.path())
            .ok_or_else(|| io::Error::other("temporary path is not UTF-8"))?;
        let rar = root.join("version.rar");
        std::fs::write(
            &rar,
            hex::decode(
                "526172211a0700cf907300000d000000000000000f0c7420802700150000000b0000000345f37dc6a48a07471d330700a481000056455253494f4e0c008fec8a45cc23c848088362fe5fdd5c5388f072c43d7b00400700",
            )?,
        )?;
        let member = ArchiveMember {
            selector: ArchiveMemberSelector {
                index: 0,
                name: "VERSION".to_owned(),
            },
            size: RAR_MAX_MEMBER_SIZE + 1,
        };
        let Err(error) = stream_rar(&rar, &[member], &mut |_, _| Ok(())) else {
            return Err("oversized metadata was not rejected before extraction".into());
        };
        assert!(error.to_string().contains("128 MiB extraction limit"));
        let mut stale = enumerate(&rar, ArchiveBackend::Rar)?[0].clone();
        stale.size += 1;
        let Err(error) = stream_rar(&rar, &[stale], &mut |_, _| Ok(())) else {
            return Err("changed RAR member size was not rejected before extraction".into());
        };
        assert!(error.to_string().contains("changed after enumeration"));
        let mut missing = enumerate(&rar, ArchiveBackend::Rar)?[0].clone();
        missing.selector.index = 9;
        let Err(error) = stream_rar(&rar, &[missing], &mut |_, _| Ok(())) else {
            return Err("a selected RAR index that was not visited must fail".into());
        };
        assert!(error.to_string().contains("disappeared after enumeration"));
        Ok(())
    }

    #[test]
    fn zip_member_changed_after_enumeration_is_not_read() -> Result<(), Box<dyn std::error::Error>>
    {
        let directory = tempfile::tempdir()?;
        let root = Utf8Path::from_path(directory.path())
            .ok_or_else(|| io::Error::other("temporary path is not UTF-8"))?;
        let path = root.join("source.zip");
        write_zip(&path, &[("expected.rom", b"rom")])?;
        let inventory = enumerate(&path, ArchiveBackend::Zip)?;
        write_zip(&path, &[("replacement.rom", b"rom")])?;

        let Err(error) = stream_zip(&path, &inventory, &mut |_, _| {
            Err(crate::Error::InvalidPath(
                "stale member reached callback".to_owned(),
            ))
        }) else {
            return Err("changed ZIP member was not rejected".into());
        };
        assert!(error.to_string().contains("changed after enumeration"));
        Ok(())
    }

    #[test]
    fn selectors_and_enumeration_agree_for_normalized_duplicate_names()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let root = Utf8Path::from_path(directory.path())
            .ok_or_else(|| io::Error::other("temporary path is not UTF-8"))?;
        let path = root.join("duplicates.zip");
        write_zip(&path, &[("same.rom", b"first"), ("./same.rom", b"second")])?;

        let members = enumerate(&path, ArchiveBackend::Zip)?;
        assert_eq!(members.len(), 2);
        assert_eq!(members[0].selector.name, members[1].selector.name);
        assert_ne!(members[0].selector.index, members[1].selector.index);
        let selected = [members[1].selector.clone(), members[0].selector.clone()];
        let mut read = Vec::new();
        let streamed = stream_archive(
            &path,
            ArchiveBackend::Zip,
            Some(&selected),
            |member, reader| {
                assert!(members.contains(member));
                let mut bytes = Vec::new();
                io::copy(reader, &mut bytes)?;
                read.push((member.selector.index, bytes));
                Ok(())
            },
        )?;
        assert_eq!(streamed, members);
        assert_eq!(read, [(1, b"second".to_vec()), (0, b"first".to_vec())]);
        Ok(())
    }

    #[test]
    fn rejects_exact_duplicate_zip_filenames_the_backend_cannot_address()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let root = Utf8Path::from_path(directory.path())
            .ok_or_else(|| io::Error::other("temporary path is not UTF-8"))?;
        let unique = root.join("unique.zip");
        let duplicate = root.join("duplicate.zip");
        write_zip(&unique, &[("same.rom", b"first"), ("diff.rom", b"second")])?;
        let mut bytes = std::fs::read(&unique)?;
        let mut offset = 0;
        while let Some(relative) = bytes[offset..]
            .windows(b"diff.rom".len())
            .position(|window| window == b"diff.rom")
        {
            offset += relative;
            bytes[offset..offset + b"diff.rom".len()].copy_from_slice(b"same.rom");
            offset += b"diff.rom".len();
        }

        let real_eocd = bytes
            .windows(4)
            .rposition(|window| window == b"PK\x05\x06")
            .ok_or_else(|| io::Error::other("ZIP end record was not written"))?;
        let directory_size = u32::from_le_bytes(
            bytes[real_eocd + 12..real_eocd + 16]
                .try_into()
                .map_err(|_| io::Error::other("invalid ZIP end record"))?,
        );
        let mut false_eocd = [0_u8; 22];
        false_eocd[..4].copy_from_slice(b"PK\x05\x06");
        false_eocd[10..12].copy_from_slice(&1_u16.to_le_bytes());
        false_eocd[12..16].copy_from_slice(&(directory_size + 22).to_le_bytes());
        bytes[real_eocd + 20..real_eocd + 22].copy_from_slice(&22_u16.to_le_bytes());
        bytes.extend_from_slice(&false_eocd);
        std::fs::write(&duplicate, bytes)?;

        assert_eq!(zip_central_entry_count(&duplicate)?, Some(2));
        let Err(error) = enumerate(&duplicate, ArchiveBackend::Zip) else {
            return Err("exact duplicate raw names were silently dropped".into());
        };
        assert!(error.to_string().contains("duplicate raw filenames"));
        Ok(())
    }

    #[test]
    fn rejects_unsafe_names_and_missing_selectors() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let root = Utf8Path::from_path(directory.path())
            .ok_or_else(|| io::Error::other("temporary path is not UTF-8"))?;
        let unsafe_path = root.join("unsafe.zip");
        write_zip(&unsafe_path, &[("../escape.rom", b"unsafe")])?;
        assert!(enumerate(&unsafe_path, ArchiveBackend::Zip).is_err());

        let path = root.join("safe.zip");
        write_zip(&path, &[("safe.rom", b"safe")])?;
        let missing = [ArchiveMemberSelector {
            index: 9,
            name: "safe.rom".to_owned(),
        }];
        assert!(stream_archive(&path, ArchiveBackend::Zip, Some(&missing), |_, _| Ok(())).is_err());
        Ok(())
    }
}
