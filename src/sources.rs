//! Shared, content-based source archive access.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom, Write},
};

use camino::Utf8Path;

use crate::domain::ArchiveBackend;
use crate::{Error, Result};

const RAR_MAX_MEMBER_SIZE: u64 = 128 * 1024 * 1024;

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
}

/// Operational details of the backends pinned by this project.
pub const BACKEND_CAPABILITIES: [BackendCapabilities; 3] = [
    BackendCapabilities {
        backend: ArchiveBackend::Zip,
        selected_reads: "Selected indices can be opened in caller order.",
        limitation: "The pinned ZIP reader collapses exact duplicate raw filenames, so those archives are rejected; distinct raw names that normalize identically remain selectable by index.",
        max_member_size: None,
    },
    BackendCapabilities {
        backend: ArchiveBackend::SevenZip,
        selected_reads: "A single sequential archive pass emits selected entries in archive order.",
        limitation: "The pinned r7z decoder buffers packed ranges in memory for some multi-range/solid extraction paths; this boundary does not guarantee constant-memory compressed-input use.",
        max_member_size: None,
    },
    BackendCapabilities {
        backend: ArchiveBackend::Rar,
        selected_reads: "Selected entries are extracted to a unique temporary directory, then read as a stream.",
        limitation: "Declared members larger than 128 MiB are rejected before extraction and actual size is checked afterward. The pinned unrar extract_to API has no streaming byte ceiling, so understated metadata can temporarily exceed the limit on disk.",
        max_member_size: Some(RAR_MAX_MEMBER_SIZE),
    },
];

pub const fn capabilities(backend: ArchiveBackend) -> &'static BackendCapabilities {
    match backend {
        ArchiveBackend::Zip => &BACKEND_CAPABILITIES[0],
        ArchiveBackend::SevenZip => &BACKEND_CAPABILITIES[1],
        ArchiveBackend::Rar => &BACKEND_CAPABILITIES[2],
    }
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
        ArchiveBackend::Zip => enumerate_zip(path),
        ArchiveBackend::SevenZip => enumerate_7z(path),
        ArchiveBackend::Rar => enumerate_rar(path),
    };
    result.map_err(|error| {
        Error::InvalidPath(format!(
            "failed to enumerate {backend:?} source {path}: {error}"
        ))
    })
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

fn enumerate_zip(path: &Utf8Path) -> Result<Vec<ArchiveMember>> {
    let central_entry_count = zip_central_entry_count(path)?.ok_or_else(|| {
        Error::InvalidPath(format!(
            "cannot validate ZIP central directory count: {path}"
        ))
    })?;
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
    const EOCD_SIGNATURE: &[u8; 4] = b"PK\x05\x06";
    const MAX_EOCD_TAIL: usize = 22 + u16::MAX as usize;

    let mut file = File::open(path)?;
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
    let requires_zip64 = entries == u16::MAX || size32 == u32::MAX || offset32 == u32::MAX;
    let (directory_end, directory_size) = if requires_zip64 {
        if eocd < 20 || &tail[eocd - 20..eocd - 16] != ZIP64_LOCATOR_SIGNATURE {
            return Ok((None, None));
        }
        let zip64_offset = u64::from_le_bytes(
            tail[eocd - 12..eocd - 4]
                .try_into()
                .map_err(|_| Error::InvalidPath("invalid ZIP64 locator".to_owned()))?,
        );
        file.seek(SeekFrom::Start(zip64_offset))?;
        let mut zip64_eocd = [0_u8; 56];
        file.read_exact(&mut zip64_eocd)?;
        if &zip64_eocd[..4] != ZIP64_EOCD_SIGNATURE {
            return Ok((None, None));
        }
        let size = u64::from_le_bytes(
            zip64_eocd[40..48]
                .try_into()
                .map_err(|_| Error::InvalidPath("invalid ZIP64 end record".to_owned()))?,
        );
        (zip64_offset, size)
    } else {
        (eocd_offset, u64::from(size32))
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

fn enumerate_7z(path: &Utf8Path) -> Result<Vec<ArchiveMember>> {
    let archive = r7z::Archive::open(path.as_std_path())?;
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

fn enumerate_rar(path: &Utf8Path) -> Result<Vec<ArchiveMember>> {
    let mut archive = unrar::Archive::new(path.as_std_path()).open_for_processing()?;
    let mut members = Vec::new();
    let mut index = 0;
    while let Some(header) = archive.read_header()? {
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
        let mut reader: &mut dyn Read = &mut entry;
        callback(member, &mut reader)?;
        io::copy(&mut reader, &mut io::sink())?;
    }
    Ok(())
}

fn stream_7z<F>(path: &Utf8Path, members: &[ArchiveMember], callback: &mut F) -> Result<()>
where
    F: FnMut(&ArchiveMember, &mut dyn Read) -> Result<()>,
{
    let archive = r7z::Archive::open(path.as_std_path())?;
    let selected = members
        .iter()
        .map(|member| (member.selector.index, member))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut callback_error = None;
    let result = archive.stream_files(|entry, reader| {
        let Some(member) = selected.get(&entry.index) else {
            return Ok(());
        };
        let name = normalize_member_name(&entry.name)
            .map_err(|_| r7z::R7zError::UnsafePath(entry.name.clone()))?;
        if member.selector.name != name {
            return Err(r7z::R7zError::UnsafePath(entry.name.clone()));
        }
        if let Err(error) = callback(member, reader) {
            callback_error = Some(error);
            return Err(r7z::R7zError::Io(io::Error::other(
                "source callback failed",
            )));
        }
        io::copy(reader, &mut io::sink()).map_err(r7z::R7zError::Io)?;
        Ok(())
    });
    if let Some(error) = callback_error {
        return Err(error);
    }
    result?;
    Ok(())
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
        } else {
            archive = header.skip()?;
        }
        index += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::{self, Write};

    use camino::Utf8Path;
    use zip::{ZipWriter, write::SimpleFileOptions};

    use super::{
        ArchiveBackend, ArchiveMember, ArchiveMemberSelector, RAR_MAX_MEMBER_SIZE, SourceKind,
        capabilities, detect, enumerate, stream_archive, stream_file, stream_rar,
        zip_central_entry_count,
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
    fn rar_limit_is_checked_before_extraction() -> Result<(), Box<dyn std::error::Error>> {
        assert!(
            capabilities(ArchiveBackend::Rar)
                .limitation
                .contains("understated metadata")
        );
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
