use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{File, OpenOptions},
    io::{self, BufWriter, Read, Write},
    path::PathBuf,
};

use std::fs;
const MAX_ARCHIVE_STAGING_BYTES: u64 = 16 * 1024 * 1024 * 1024;

use camino::{Utf8Path, Utf8PathBuf};
use md5::Md5;
use sha1::{Digest, Sha1};
#[cfg(all(unix, test))]
use std::os::unix::fs::DirBuilderExt;
use xxhash_rust::xxh3::Xxh3;
use zip::{ZipWriter, write::SimpleFileOptions};

#[cfg(test)]
use crate::build::validation::checked_destination;
use crate::build::validation::{
    canonicalize_destination, checked_plan_destination, is_safe_relative_path, validate_plan,
};
use crate::domain::{
    ArchiveBackend, ArchiveMemberSelector as PlannedArchiveMemberSelector, ArtifactOutcome,
    ArtifactResult, ArtifactReusePolicy, BuildPlan, LogicalEntry, OutputContainer, OutputGroup,
    PlanOutcome, SourceFingerprint, SourceLocation, ZipCompression,
};

#[cfg(test)]
pub fn write_plan(plan: &BuildPlan, destination: &Utf8Path) -> crate::Result<Vec<Utf8PathBuf>> {
    let mut written = Vec::new();
    for result in write_plan_with_compression(plan, destination, ZipCompression::Deflate)? {
        match result.outcome {
            ArtifactOutcome::Completed | ArtifactOutcome::Reused => written.push(
                Utf8PathBuf::try_from(PathBuf::from(result.path)).map_err(|_| {
                    crate::Error::InvalidPath("artifact path is not UTF-8".to_owned())
                })?,
            ),
            ArtifactOutcome::CompletedWithWarning { .. } => written.push(
                Utf8PathBuf::try_from(PathBuf::from(result.path)).map_err(|_| {
                    crate::Error::InvalidPath("artifact path is not UTF-8".to_owned())
                })?,
            ),
            ArtifactOutcome::Failed { error } => {
                return Err(crate::Error::InvalidPath(error));
            }
            ArtifactOutcome::ReplacedButNotDurable { error } => {
                return Err(crate::Error::ArtifactReplacedNotDurable { error });
            }
            ArtifactOutcome::Unattempted => {}
        }
    }
    Ok(written)
}

#[cfg(test)]
fn write_plan_with_compression(
    plan: &BuildPlan,
    destination: &Utf8Path,
    compression: ZipCompression,
) -> crate::Result<Vec<ArtifactResult>> {
    write_plan_with_container(plan, destination, OutputContainer::Zip, compression)
}

pub fn write_plan_with_container(
    plan: &BuildPlan,
    destination: &Utf8Path,
    container: OutputContainer,
    compression: ZipCompression,
) -> crate::Result<Vec<ArtifactResult>> {
    write_plan_with_container_policy(
        plan,
        destination,
        container,
        compression,
        ArtifactReusePolicy::Replace,
    )
}

pub fn write_plan_with_container_policy(
    plan: &BuildPlan,
    destination: &Utf8Path,
    container: OutputContainer,
    compression: ZipCompression,
    reuse_policy: ArtifactReusePolicy,
) -> crate::Result<Vec<ArtifactResult>> {
    if plan.report.outcome != PlanOutcome::Ready || !plan.has_outputs() {
        return Ok(Vec::new());
    }

    let validated = validate_plan(plan)?;
    let plan = validated.plan();
    write_plan_with_hook(
        plan,
        destination,
        container,
        file_options(compression),
        reuse_policy,
        &mut |_, _| Ok(()),
    )
}

fn write_plan_with_hook(
    plan: &BuildPlan,
    destination: &Utf8Path,
    container: OutputContainer,
    options: SimpleFileOptions,
    reuse_policy: ArtifactReusePolicy,
    hook: &mut impl FnMut(usize, ArtifactPhase) -> io::Result<()>,
) -> crate::Result<Vec<ArtifactResult>> {
    let checked = checked_plan_destination(plan, destination)?;
    let destination = checked.path();
    let source_paths = checked.sources();
    let sources = source_paths
        .iter()
        .map(Utf8PathBuf::as_path)
        .collect::<Vec<_>>();
    let output_paths = output_paths(plan, destination, container)?;
    // Handle-relative output is supported on Linux and macOS. Reject other
    // targets before opening or creating output.
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let _ = (container, options, output_paths, reuse_policy);
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    return Err(
        io::Error::other("secure output writing is supported only on Linux and macOS").into(),
    );

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let output_root = SecureOutputDirectory::open(destination, &sources)?;
        let mut results = Vec::with_capacity(plan.groups.len());
        for (index, (group, path)) in plan.groups.iter().zip(&output_paths).enumerate() {
            let mut artifact = ArtifactContext {
                options,
                reuse_policy,
                spool_parent: destination,
                checked_destination: &checked,
                artifact_index: index,
                hook,
                #[cfg(unix)]
                output_root: &output_root,
            };
            match write_artifact(group, path, container, &mut artifact) {
                Ok(outcome) => results.push(ArtifactResult {
                    path: path.to_string(),
                    outcome,
                }),
                Err(crate::Error::ArtifactReplacedNotDurable { error }) => {
                    results.push(ArtifactResult {
                        path: path.to_string(),
                        outcome: ArtifactOutcome::ReplacedButNotDurable { error },
                    });
                    results.extend(output_paths.iter().skip(index + 1).map(|path| {
                        ArtifactResult {
                            path: path.to_string(),
                            outcome: ArtifactOutcome::Unattempted,
                        }
                    }));
                    break;
                }
                Err(error) => {
                    results.push(ArtifactResult {
                        path: path.to_string(),
                        outcome: ArtifactOutcome::Failed {
                            error: error.to_string(),
                        },
                    });
                    results.extend(output_paths.iter().skip(index + 1).map(|path| {
                        ArtifactResult {
                            path: path.to_string(),
                            outcome: ArtifactOutcome::Unattempted,
                        }
                    }));
                    break;
                }
            }
        }
        Ok(results)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArtifactPhase {
    Read,
    Write,
    Finalize,
    Replace,
    StagedDirectorySync { depth: usize },
    DirectorySync,
}

struct ArtifactContext<'a> {
    options: SimpleFileOptions,
    reuse_policy: ArtifactReusePolicy,
    artifact_index: usize,
    spool_parent: &'a Utf8Path,
    checked_destination: &'a crate::build::validation::CheckedPlanDestination,
    hook: &'a mut dyn FnMut(usize, ArtifactPhase) -> io::Result<()>,
    #[cfg(unix)]
    output_root: &'a SecureOutputDirectory,
}

fn write_artifact(
    group: &OutputGroup,
    path: &Utf8Path,
    container: OutputContainer,
    artifact: &mut ArtifactContext<'_>,
) -> crate::Result<ArtifactOutcome> {
    #[cfg(unix)]
    if artifact.reuse_policy == ArtifactReusePolicy::ReuseVerified {
        artifact.output_root.ensure_disjoint()?;
        (artifact.hook)(artifact.artifact_index, ArtifactPhase::Read)?;
        if try_reuse_artifact(group, container, artifact)? {
            return Ok(ArtifactOutcome::Reused);
        }
    }
    match container {
        OutputContainer::Zip => write_zip_artifact(group, path, artifact),
        OutputContainer::Directory => write_directory_artifact(group, path, artifact),
        OutputContainer::SevenZip => write_seven_zip_artifact(group, path, artifact),
    }
}

#[cfg(unix)]
fn try_reuse_artifact(
    group: &OutputGroup,
    container: OutputContainer,
    artifact: &ArtifactContext<'_>,
) -> crate::Result<bool> {
    let (relative, is_directory) = match container {
        OutputContainer::Zip => (format!("{}.zip", group.path.as_str()), false),
        OutputContainer::Directory => (group.path.as_str().to_owned(), true),
        OutputContainer::SevenZip => (format!("{}.7z", group.path.as_str()), false),
    };
    let Some(existing) = artifact
        .output_root
        .open_existing_artifact(&relative, is_directory)?
    else {
        return Ok(false);
    };
    let Some(snapshot) = existing.snapshot() else {
        return Ok(false);
    };
    let matches = match container {
        OutputContainer::Zip => verify_existing_zip(group, &existing.file)?,
        OutputContainer::Directory => {
            let path = artifact.output_root.path.join(&relative);
            verify_existing_directory(group, &path, &existing, artifact.output_root)?
        }
        OutputContainer::SevenZip => verify_existing_seven_zip(group, &existing.file)?,
    };
    if !matches {
        return Ok(false);
    }
    artifact.output_root.ensure_disjoint()?;
    Ok(existing.is_unchanged_and_still_named(&snapshot))
}

#[cfg(unix)]
fn verify_existing_zip(group: &OutputGroup, file: &File) -> crate::Result<bool> {
    let Some(central_entry_count) = crate::sources::zip_central_entry_count_from_file(file)
        .ok()
        .flatten()
    else {
        return Ok(false);
    };
    let file = file.try_clone()?;
    let Ok(mut archive) = zip::ZipArchive::new(file) else {
        return Ok(false);
    };
    if central_entry_count != archive.len() || archive.len() != group.entries.len() {
        return Ok(false);
    }
    let expected: BTreeMap<_, _> = group
        .entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect();
    let mut seen = BTreeSet::new();
    for index in 0..archive.len() {
        let Ok(mut member) = archive.by_index(index) else {
            return Ok(false);
        };
        if !member.is_file() || !seen.insert(member.name().to_owned()) {
            return Ok(false);
        }
        let Some(entry) = expected.get(member.name()) else {
            return Ok(false);
        };
        let declared_size = member.size();
        if !verify_existing_member(entry, &mut member, Some(declared_size)) {
            return Ok(false);
        }
    }
    Ok(seen.len() == expected.len())
}

#[cfg(unix)]
fn verify_existing_seven_zip(group: &OutputGroup, file: &File) -> crate::Result<bool> {
    let Ok(archive) = r7z::Archive::from_reader(file.try_clone()?) else {
        return Ok(false);
    };
    let entries = archive.entries().collect::<Vec<_>>();
    if entries.len() != group.entries.len() {
        return Ok(false);
    }
    let Ok(listing) = archive.listing(None) else {
        return Ok(false);
    };
    let expected: BTreeMap<_, _> = group
        .entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect();
    let mut seen = BTreeSet::new();
    let mut selected = BTreeMap::new();
    for member in entries {
        if !matches!(
            member.entry_type,
            r7z::EntryType::File | r7z::EntryType::EmptyFile
        ) {
            return Ok(false);
        }
        if !seen.insert(member.name.clone()) {
            return Ok(false);
        }
        let Some(entry) = expected.get(member.name.as_str()) else {
            return Ok(false);
        };
        let Some(declared_size) = listing
            .entries
            .get(member.index)
            .and_then(|listed| listed.size)
        else {
            return Ok(false);
        };
        let maximum = existing_member_maximum(entry);
        if declared_size > maximum {
            return Ok(false);
        }
        selected.insert(member.index, (entry, declared_size, maximum));
    }
    if seen.len() != expected.len() {
        return Ok(false);
    }

    let indices = selected.keys().copied().collect::<Vec<_>>();
    Ok(archive
        .stream_selected_files(&indices, |member, reader| {
            let Some((entry, declared_size, maximum)) = selected.get(&member.index) else {
                return Err(r7z::R7zError::Parse);
            };
            let mut content = ContentWriter::with_limit(io::sink(), *maximum);
            io::copy(reader, &mut content).map_err(r7z::R7zError::Io)?;
            if !verify_existing_member_digest(entry, content.finish(), Some(*declared_size)) {
                return Err(r7z::R7zError::Io(io::Error::other(
                    "existing 7z member content does not match the planned output",
                )));
            }
            Ok(())
        })
        .is_ok())
}

#[cfg(unix)]
fn verify_existing_directory(
    group: &OutputGroup,
    path: &Utf8Path,
    existing: &ExistingArtifact,
    output_root: &SecureOutputDirectory,
) -> crate::Result<bool> {
    let expected: BTreeMap<_, _> = group
        .entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect();
    let mut expected_directories = BTreeSet::new();
    for entry in &group.entries {
        let mut parent = Utf8Path::new(entry.path.as_str()).parent();
        while let Some(directory) = parent.filter(|directory| !directory.as_str().is_empty()) {
            expected_directories.insert(directory.as_str().to_owned());
            parent = directory.parent();
        }
    }
    let mut seen_files = BTreeSet::new();
    let mut seen_directories = BTreeSet::new();
    let Some(root_snapshot) = existing.snapshot() else {
        return Ok(false);
    };
    let mut file_snapshots = BTreeMap::new();
    let mut directory_snapshots = BTreeMap::new();
    for item in walkdir::WalkDir::new(path).follow_links(false) {
        let Ok(item) = item else {
            return Ok(false);
        };
        if item.depth() == 0 {
            continue;
        }
        let Ok(relative_path) = item.path().strip_prefix(path) else {
            return Ok(false);
        };
        let Some(relative) = Utf8Path::from_path(relative_path).map(Utf8Path::as_str) else {
            return Ok(false);
        };
        let file_type = item.file_type();
        if file_type.is_symlink() {
            return Ok(false);
        }
        if file_type.is_dir() {
            if !expected_directories.contains(relative)
                || !seen_directories.insert(relative.to_owned())
            {
                return Ok(false);
            }
            let Some(directory) = open_existing_relative(&existing.file, relative, true)? else {
                return Ok(false);
            };
            let Some(snapshot) = directory.snapshot() else {
                return Ok(false);
            };
            directory_snapshots.insert(relative.to_owned(), snapshot);
            continue;
        }
        if !file_type.is_file() || !seen_files.insert(relative.to_owned()) {
            return Ok(false);
        }
        let Some(entry) = expected.get(relative) else {
            return Ok(false);
        };
        let Some(file) = open_existing_relative(&existing.file, relative, false)? else {
            return Ok(false);
        };
        let Some(snapshot) = file.snapshot() else {
            return Ok(false);
        };
        if !verify_existing_member(entry, &mut &file.file, Some(snapshot.size)) {
            return Ok(false);
        }
        file_snapshots.insert(relative.to_owned(), snapshot);
    }
    output_root.ensure_disjoint()?;
    Ok(seen_files.len() == expected.len()
        && seen_directories == expected_directories
        && existing.is_unchanged_and_still_named(&root_snapshot)
        && directory_snapshots.iter().all(|(relative, snapshot)| {
            open_existing_relative(&existing.file, relative, true)
                .ok()
                .flatten()
                .is_some_and(|directory| directory.is_unchanged_and_still_named(snapshot))
        })
        && file_snapshots.iter().all(|(relative, snapshot)| {
            open_existing_relative(&existing.file, relative, false)
                .ok()
                .flatten()
                .is_some_and(|file| file.is_unchanged_and_still_named(snapshot))
        }))
}

#[cfg(unix)]
fn existing_member_maximum(entry: &LogicalEntry) -> u64 {
    match entry.selection.strength {
        crate::resolution::MatchStrength::Sha1 | crate::resolution::MatchStrength::Md5 => {
            entry.source.observed.size
        }
        crate::resolution::MatchStrength::CrcAndSize => {
            entry.expected.size.or(entry.source.observed.size)
        }
    }
    .unwrap_or(MAX_ARCHIVE_STAGING_BYTES)
}

#[cfg(unix)]
fn verify_existing_member_digest(
    entry: &LogicalEntry,
    actual: ContentDigest,
    declared_size: Option<u64>,
) -> bool {
    let maximum = existing_member_maximum(entry);
    declared_size.is_none_or(|size| size <= maximum)
        && actual.size <= maximum
        && verify_content(entry, actual).is_ok()
}

#[cfg(unix)]
fn verify_existing_member(
    entry: &LogicalEntry,
    reader: &mut impl io::Read,
    declared_size: Option<u64>,
) -> bool {
    let maximum = existing_member_maximum(entry);
    if declared_size.is_some_and(|size| size > maximum) {
        return false;
    }
    let mut content = ContentWriter::with_limit(io::sink(), maximum);
    io::copy(reader, &mut content).is_ok()
        && verify_existing_member_digest(entry, content.finish(), declared_size)
}

fn write_zip_artifact(
    group: &OutputGroup,
    _path: &Utf8Path,
    artifact: &mut ArtifactContext<'_>,
) -> crate::Result<ArtifactOutcome> {
    #[cfg(unix)]
    let (mut staged, file) = artifact
        .output_root
        .stage_file(&format!("{}.zip", group.path.as_str()))?;
    #[cfg(not(unix))]
    let (staged, file) = {
        let parent = _path.parent().ok_or_else(|| {
            crate::Error::InvalidPath(format!("artifact has no parent directory: {_path}"))
        })?;
        fs::create_dir_all(parent)?;
        StagedArtifact::create(_path)?
    };
    let mut zip_writer = ZipWriter::new(BufWriter::new(file));
    zip_writer.set_comment("Generated by mame_coalesce")?;
    write_zip_group(group, &mut zip_writer, artifact)?;
    (artifact.hook)(artifact.artifact_index, ArtifactPhase::Finalize)?;
    let mut output = zip_writer.finish()?;
    output.flush()?;
    output.get_ref().sync_all()?;
    drop(output);
    (artifact.hook)(artifact.artifact_index, ArtifactPhase::Replace)?;
    #[cfg(unix)]
    staged.replace(artifact.output_root, artifact.hook, artifact.artifact_index)?;
    #[cfg(not(unix))]
    staged.replace(_path)?;
    Ok(ArtifactOutcome::Completed)
}

fn write_seven_zip_artifact(
    group: &OutputGroup,
    _path: &Utf8Path,
    artifact: &mut ArtifactContext<'_>,
) -> crate::Result<ArtifactOutcome> {
    #[cfg(unix)]
    let (mut staged, file) = artifact
        .output_root
        .stage_file(&format!("{}.7z", group.path.as_str()))?;
    #[cfg(not(unix))]
    let (staged, file) = {
        let parent = _path.parent().ok_or_else(|| {
            crate::Error::InvalidPath(format!("artifact has no parent directory: {_path}"))
        })?;
        fs::create_dir_all(parent)?;
        StagedArtifact::create(_path)?
    };
    let mut archive_writer = r7z::ArchiveWriter::new(file, r7z::ArchiveOptions::default())?;
    write_seven_zip_group(group, &mut archive_writer, artifact)?;
    (artifact.hook)(artifact.artifact_index, ArtifactPhase::Finalize)?;
    let mut output = archive_writer.finish()?;
    output.flush()?;
    output.sync_all()?;
    drop(output);
    (artifact.hook)(artifact.artifact_index, ArtifactPhase::Replace)?;
    #[cfg(unix)]
    staged.replace(artifact.output_root, artifact.hook, artifact.artifact_index)?;
    #[cfg(not(unix))]
    staged.replace(_path)?;
    Ok(ArtifactOutcome::Completed)
}

fn write_directory_artifact(
    group: &OutputGroup,
    path: &Utf8Path,
    artifact: &mut ArtifactContext<'_>,
) -> crate::Result<ArtifactOutcome> {
    #[cfg(unix)]
    {
        let mut staged = artifact.output_root.stage_directory(group.path.as_str())?;
        write_directory_group(group, &staged.directory, artifact)?;
        (artifact.hook)(artifact.artifact_index, ArtifactPhase::Finalize)?;
        (artifact.hook)(artifact.artifact_index, ArtifactPhase::Replace)?;
        staged.replace(
            artifact.output_root,
            path,
            artifact.hook,
            artifact.artifact_index,
        )
    }
    #[cfg(not(unix))]
    {
        let parent = path.parent().ok_or_else(|| {
            crate::Error::InvalidPath(format!("artifact has no parent directory: {path}"))
        })?;
        fs::create_dir_all(parent)?;
        let staged = StagedDirectory::create(path)?;
        write_directory_group(group, staged.path(), artifact)?;
        (artifact.hook)(artifact.artifact_index, ArtifactPhase::Finalize)?;
        (artifact.hook)(artifact.artifact_index, ArtifactPhase::Replace)?;
        staged.replace(path)
    }
}

fn output_paths(
    plan: &BuildPlan,
    destination: &Utf8Path,
    container: OutputContainer,
) -> crate::Result<Vec<Utf8PathBuf>> {
    let extension = match container {
        OutputContainer::Zip => ".zip",
        OutputContainer::Directory => "",
        OutputContainer::SevenZip => ".7z",
    };
    let kind = match container {
        OutputContainer::Zip => "ZIP artifact",
        OutputContainer::Directory => "directory artifact",
        OutputContainer::SevenZip => "7z artifact",
    };
    let relative_paths = plan
        .groups
        .iter()
        .map(|group| format!("{}{extension}", group.path.as_str()))
        .collect::<Vec<_>>();
    let mut folded_paths = BTreeSet::new();
    for path in &relative_paths {
        if !is_safe_relative_path(path) {
            return Err(crate::Error::InvalidPath(format!(
                "unsafe {kind} path: {path}"
            )));
        }
        if !folded_paths.insert(path.to_ascii_lowercase()) {
            return Err(crate::Error::InvalidPath(format!(
                "duplicate {kind} path: {path}"
            )));
        }
    }
    for path in &relative_paths {
        for slash in path.match_indices('/').map(|(index, _)| index) {
            if folded_paths.contains(&path[..slash].to_ascii_lowercase()) {
                return Err(crate::Error::InvalidPath(format!(
                    "{kind} file/directory conflict: {path}"
                )));
            }
        }
    }
    Ok(relative_paths
        .into_iter()
        .map(|path| destination.join(path))
        .collect())
}

fn file_options(compression: ZipCompression) -> SimpleFileOptions {
    let method = match compression {
        ZipCompression::Deflate => zip::CompressionMethod::Deflated,
        ZipCompression::Store => zip::CompressionMethod::Stored,
    };
    SimpleFileOptions::default().compression_method(method)
}

#[cfg(test)]
fn validate_output_file_name(name: &str) -> crate::Result<()> {
    if is_safe_relative_path(name) {
        Ok(())
    } else {
        Err(crate::Error::InvalidPath(format!(
            "unsafe output zip file name: {name}"
        )))
    }
}

#[cfg(test)]
fn validate_zip_entry_name(name: &str) -> crate::Result<()> {
    if is_safe_relative_path(name) {
        Ok(())
    } else {
        Err(crate::Error::InvalidPath(format!(
            "unsafe zip entry name: {name}"
        )))
    }
}

#[cfg(unix)]
struct SecureOutputDirectory {
    directory: File,
    path: Utf8PathBuf,
    source_directories: Vec<File>,
}

#[cfg(unix)]
impl SecureOutputDirectory {
    fn open(destination: &Utf8Path, sources: &[&Utf8Path]) -> crate::Result<Self> {
        let destination = canonicalize_destination(destination)?;
        let source_directories = sources
            .iter()
            .map(|source| {
                let source = source.canonicalize_utf8()?;
                open_directory(&source, false)
            })
            .collect::<crate::Result<Vec<_>>>()?;
        let directory = open_directory(&destination, true)?;
        let output = Self {
            directory,
            path: destination,
            source_directories,
        };
        output.ensure_disjoint()?;
        Ok(output)
    }

    fn ensure_disjoint(&self) -> crate::Result<()> {
        self.ensure_directory_disjoint(&self.directory)
    }

    fn ensure_directory_disjoint(&self, directory: &File) -> crate::Result<()> {
        for source in &self.source_directories {
            if directory_is_ancestor(source, directory)?
                || directory_is_ancestor(directory, source)?
            {
                return Err(crate::Error::InvalidPath(
                    "source/destination overlap is not allowed".to_owned(),
                ));
            }
        }
        Ok(())
    }

    fn open_existing_artifact(
        &self,
        relative: &str,
        directory: bool,
    ) -> crate::Result<Option<ExistingArtifact>> {
        self.ensure_disjoint()?;
        open_existing_relative(&self.directory, relative, directory)
    }

    fn stage_file(&self, relative: &str) -> crate::Result<(StagedZip, File)> {
        use rustix::{
            fs::{self, Mode, OFlags},
            io::Errno,
        };
        use std::os::fd::AsFd;

        self.ensure_disjoint()?;
        let components = relative.split('/').collect::<Vec<_>>();
        let Some(file_name) = components.last().copied() else {
            return Err(crate::Error::InvalidPath(format!(
                "invalid output path: {relative}"
            )));
        };
        let mut parent = self.directory.try_clone()?;
        for component in &components[..components.len() - 1] {
            let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW;
            let next = match fs::openat(parent.as_fd(), *component, flags, Mode::empty()) {
                Ok(next) => next,
                Err(error) if error == Errno::NOENT => {
                    match fs::mkdirat(parent.as_fd(), *component, Mode::from_raw_mode(0o755)) {
                        Ok(()) => parent.sync_all()?,
                        Err(error) if error == Errno::EXIST => {}
                        Err(error) => return Err(std::io::Error::from(error).into()),
                    }
                    fs::openat(parent.as_fd(), *component, flags, Mode::empty())
                        .map_err(std::io::Error::from)?
                }
                Err(error) => return Err(std::io::Error::from(error).into()),
            };
            parent = File::from(next);
        }
        for _ in 0..10 {
            let temporary_name = format!(".{file_name}.{}.tmp", uuid::Uuid::new_v4());
            match fs::openat(
                parent.as_fd(),
                temporary_name.as_str(),
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                Mode::from_raw_mode(0o600),
            ) {
                Ok(file) => {
                    let staged = StagedZip {
                        parent,
                        temporary_name,
                        file_name: (*file_name).to_owned(),
                        state: StagedZipState::Staged,
                    };
                    return Ok((staged, File::from(file)));
                }
                Err(error) if error == Errno::EXIST => {}
                Err(error) => return Err(std::io::Error::from(error).into()),
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "could not allocate a staged ZIP beside its destination",
        )
        .into())
    }

    fn create_spool_directory(&self) -> crate::Result<SecureSpoolDirectory> {
        use rustix::{
            fs::{self, Mode, OFlags},
            io::Errno,
        };
        use std::os::fd::AsFd;

        self.ensure_disjoint()?;
        let parent = self.directory.try_clone()?;
        for _ in 0..10 {
            let name = format!("mame-coalesce-build-{}", uuid::Uuid::new_v4());
            match fs::mkdirat(parent.as_fd(), name.as_str(), Mode::from_raw_mode(0o700)) {
                Ok(()) => {
                    let created_stat = fs::statat(
                        parent.as_fd(),
                        name.as_str(),
                        rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
                    )
                    .map_err(std::io::Error::from)?;
                    let effective_uid = rustix::process::geteuid().as_raw();
                    if !private_directory_owned_by(&created_stat, effective_uid) {
                        cleanup_spool_name_if_same(&parent, &name, &created_stat);
                        return Err(crate::Error::InvalidPath(
                            "archive staging directory was replaced or is not private".to_owned(),
                        ));
                    }
                    let directory = match fs::openat(
                        parent.as_fd(),
                        name.as_str(),
                        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                        Mode::empty(),
                    ) {
                        Ok(directory) => directory,
                        Err(error) => {
                            cleanup_spool_name_if_same(&parent, &name, &created_stat);
                            return Err(std::io::Error::from(error).into());
                        }
                    };
                    let opened_stat = fs::fstat(&directory).map_err(std::io::Error::from)?;
                    let named_stat = fs::statat(
                        parent.as_fd(),
                        name.as_str(),
                        rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
                    )
                    .map_err(std::io::Error::from)?;
                    let same_directory = same_directory(&opened_stat, &created_stat)
                        && same_directory(&opened_stat, &named_stat);
                    let owned_private_directory =
                        private_directory_owned_by(&opened_stat, effective_uid)
                            && private_directory_owned_by(&named_stat, effective_uid);
                    if !same_directory || !owned_private_directory {
                        drop(directory);
                        cleanup_spool_name_if_same(&parent, &name, &created_stat);
                        return Err(crate::Error::InvalidPath(
                            "archive staging directory was replaced or is not private".to_owned(),
                        ));
                    }
                    let source_directories = match self
                        .source_directories
                        .iter()
                        .map(File::try_clone)
                        .collect::<std::io::Result<Vec<_>>>()
                    {
                        Ok(sources) => sources,
                        Err(error) => {
                            cleanup_spool_name_if_same(&parent, &name, &created_stat);
                            return Err(error.into());
                        }
                    };
                    return Ok(SecureSpoolDirectory {
                        parent,
                        name,
                        directory: File::from(directory),
                        source_directories,
                    });
                }
                Err(error) if error == Errno::EXIST => {}
                Err(error) => return Err(std::io::Error::from(error).into()),
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "could not allocate a private archive staging directory",
        )
        .into())
    }

    fn stage_directory(&self, relative: &str) -> crate::Result<StagedDirectory> {
        use rustix::{
            fs::{self, Mode, OFlags},
            io::Errno,
        };
        use std::os::fd::AsFd;

        self.ensure_disjoint()?;
        let (parent, directory_name, parent_path) = self.open_parent(relative, true)?;
        for _ in 0..10 {
            let staging_name = format!(".{directory_name}.stage-{}", uuid::Uuid::new_v4());
            match fs::mkdirat(
                parent.as_fd(),
                staging_name.as_str(),
                Mode::from_raw_mode(0o700),
            ) {
                Ok(()) => {
                    let directory = fs::openat(
                        parent.as_fd(),
                        staging_name.as_str(),
                        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                        Mode::empty(),
                    )
                    .map_err(|error| {
                        let _ = fs::unlinkat(
                            parent.as_fd(),
                            staging_name.as_str(),
                            rustix::fs::AtFlags::REMOVEDIR,
                        );
                        std::io::Error::from(error)
                    })?;
                    let directory = File::from(directory);
                    if let Err(error) = self.ensure_directory_disjoint(&directory) {
                        let _ = remove_directory_at(&parent, &staging_name, &parent_path);
                        return Err(error);
                    }
                    return Ok(StagedDirectory {
                        parent,
                        parent_path,
                        staging_name,
                        directory_name: directory_name.to_owned(),
                        directory,
                        state: StagedDirectoryState::Staged,
                    });
                }
                Err(error) if error == Errno::EXIST => {}
                Err(error) => return Err(std::io::Error::from(error).into()),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate a staged directory beside its destination",
        )
        .into())
    }

    fn open_parent<'a>(
        &self,
        relative: &'a str,
        create: bool,
    ) -> crate::Result<(File, &'a str, Utf8PathBuf)> {
        use rustix::{
            fs::{self, Mode, OFlags},
            io::Errno,
        };
        use std::os::fd::AsFd;

        let components = relative.split('/').collect::<Vec<_>>();
        let Some(name) = components.last().copied() else {
            return Err(crate::Error::InvalidPath(format!(
                "invalid output path: {relative}"
            )));
        };
        let mut parent = self.directory.try_clone()?;
        for component in &components[..components.len() - 1] {
            let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW;
            let next = match fs::openat(parent.as_fd(), *component, flags, Mode::empty()) {
                Ok(next) => next,
                Err(error) if create && error == Errno::NOENT => {
                    match fs::mkdirat(parent.as_fd(), *component, Mode::from_raw_mode(0o755)) {
                        Ok(()) => parent.sync_all()?,
                        Err(error) if error == Errno::EXIST => {}
                        Err(error) => return Err(std::io::Error::from(error).into()),
                    }
                    fs::openat(parent.as_fd(), *component, flags, Mode::empty())
                        .map_err(std::io::Error::from)?
                }
                Err(error) => return Err(std::io::Error::from(error).into()),
            };
            parent = File::from(next);
        }
        let parent_path = self.path.join(components[..components.len() - 1].join("/"));
        Ok((parent, name, parent_path))
    }

    fn create_directory_file(
        &self,
        directory: &File,
        relative: &str,
        staged_directories: &mut BTreeMap<Utf8PathBuf, File>,
    ) -> crate::Result<File> {
        use rustix::{
            fs::{self, Mode, OFlags},
            io::Errno,
        };
        use std::os::fd::AsFd;

        self.ensure_directory_disjoint(directory)?;
        let components = relative.split('/').collect::<Vec<_>>();
        let Some(file_name) = components.last().copied() else {
            return Err(crate::Error::InvalidPath(format!(
                "invalid directory entry: {relative}"
            )));
        };
        let mut parent = directory.try_clone()?;
        let mut parent_relative = Utf8PathBuf::new();
        for component in &components[..components.len() - 1] {
            let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW;
            let next = match fs::openat(parent.as_fd(), *component, flags, Mode::empty()) {
                Ok(next) => next,
                Err(error) if error == Errno::NOENT => {
                    match fs::mkdirat(parent.as_fd(), *component, Mode::from_raw_mode(0o755)) {
                        Ok(()) => parent.sync_all()?,
                        Err(error) if error == Errno::EXIST => {}
                        Err(error) => return Err(std::io::Error::from(error).into()),
                    }
                    fs::openat(parent.as_fd(), *component, flags, Mode::empty())
                        .map_err(std::io::Error::from)?
                }
                Err(error) => return Err(std::io::Error::from(error).into()),
            };
            parent = File::from(next);
            parent_relative.push(component);
            staged_directories
                .entry(parent_relative.clone())
                .or_insert(parent.try_clone()?);
        }
        let file = fs::openat(
            parent.as_fd(),
            file_name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::from_raw_mode(0o600),
        )
        .map_err(std::io::Error::from)?;
        Ok(File::from(file))
    }
}

#[cfg(unix)]
struct ExistingArtifact {
    file: File,
    parent: File,
    name: String,
    ancestors: Vec<DirectoryBinding>,
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ArtifactSnapshot {
    device: u64,
    inode: u64,
    file_type: u32,
    size: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

#[cfg(unix)]
struct DirectoryBinding {
    directory: File,
    parent: File,
    name: String,
    snapshot: ArtifactSnapshot,
}

#[cfg(unix)]
impl DirectoryBinding {
    fn is_unchanged_and_still_named(&self) -> bool {
        is_snapshot_still_named(&self.directory, &self.parent, &self.name, &self.snapshot)
    }
}

#[cfg(unix)]
impl ExistingArtifact {
    fn snapshot(&self) -> Option<ArtifactSnapshot> {
        snapshot_handle(&self.file)
    }

    fn is_unchanged_and_still_named(&self, snapshot: &ArtifactSnapshot) -> bool {
        is_snapshot_still_named(&self.file, &self.parent, &self.name, snapshot)
            && self
                .ancestors
                .iter()
                .all(DirectoryBinding::is_unchanged_and_still_named)
    }
}

#[cfg(unix)]
fn snapshot_handle(file: &File) -> Option<ArtifactSnapshot> {
    use std::os::unix::fs::MetadataExt;

    let metadata = file.metadata().ok()?;
    Some(ArtifactSnapshot {
        device: metadata.dev(),
        inode: metadata.ino(),
        file_type: metadata.mode() & 0o170_000,
        size: metadata.size(),
        modified_seconds: metadata.mtime(),
        modified_nanoseconds: metadata.mtime_nsec(),
        changed_seconds: metadata.ctime(),
        changed_nanoseconds: metadata.ctime_nsec(),
    })
}

#[cfg(unix)]
fn is_snapshot_still_named(
    file: &File,
    parent: &File,
    name: &str,
    snapshot: &ArtifactSnapshot,
) -> bool {
    use rustix::fs::{self, AtFlags};
    use std::os::fd::AsFd;

    let Ok(named) = fs::statat(parent.as_fd(), name, AtFlags::SYMLINK_NOFOLLOW) else {
        return false;
    };
    snapshot_handle(file).is_some_and(|current| {
        let named_identity_matches =
            current.device == named.st_dev && current.inode == named.st_ino;
        let named_type_matches = current.file_type == (named.st_mode & 0o170_000);
        current == *snapshot && named_identity_matches && named_type_matches
    })
}

#[cfg(unix)]
fn open_existing_relative(
    root: &File,
    relative: &str,
    directory: bool,
) -> crate::Result<Option<ExistingArtifact>> {
    use rustix::{
        fs::{self, Mode, OFlags},
        io::Errno,
    };
    use std::os::fd::AsFd;

    if !is_safe_relative_path(relative) {
        return Ok(None);
    }
    let components = relative.split('/').collect::<Vec<_>>();
    let Some(name) = components.last().copied() else {
        return Ok(None);
    };
    let mut parent = root.try_clone()?;
    let mut ancestors = Vec::new();
    for component in &components[..components.len() - 1] {
        let flags = OFlags::RDONLY
            | OFlags::DIRECTORY
            | OFlags::CLOEXEC
            | OFlags::NOFOLLOW
            | OFlags::NONBLOCK;
        match fs::openat(parent.as_fd(), *component, flags, Mode::empty()) {
            Ok(next) => {
                let next = File::from(next);
                let Some(snapshot) = snapshot_handle(&next) else {
                    return Ok(None);
                };
                ancestors.push(DirectoryBinding {
                    directory: next.try_clone()?,
                    parent: parent.try_clone()?,
                    name: (*component).to_owned(),
                    snapshot,
                });
                parent = next;
            }
            Err(error)
                if error == Errno::NOENT
                    || error == Errno::NOTDIR
                    || error == Errno::LOOP
                    || error == Errno::ACCESS
                    || error == Errno::PERM =>
            {
                return Ok(None);
            }
            Err(error) => return Err(std::io::Error::from(error).into()),
        }
    }
    let expected_kind = if directory { 0o040_000 } else { 0o100_000 };
    let named_before = match fs::statat(parent.as_fd(), name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
    {
        Ok(metadata) if metadata.st_mode & 0o170_000 == expected_kind => metadata,
        Ok(_) => return Ok(None),
        Err(error)
            if error == Errno::NOENT
                || error == Errno::NOTDIR
                || error == Errno::LOOP
                || error == Errno::ACCESS
                || error == Errno::PERM =>
        {
            return Ok(None);
        }
        Err(error) => return Err(std::io::Error::from(error).into()),
    };
    let mut flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK;
    flags |= OFlags::NOCTTY;
    if directory {
        flags |= OFlags::DIRECTORY;
    }
    let file = match fs::openat(parent.as_fd(), name, flags, Mode::empty()) {
        Ok(file) => File::from(file),
        Err(error)
            if error == Errno::NOENT
                || error == Errno::NOTDIR
                || error == Errno::LOOP
                || error == Errno::ACCESS
                || error == Errno::PERM =>
        {
            return Ok(None);
        }
        Err(error) => return Err(std::io::Error::from(error).into()),
    };
    let metadata = fs::fstat(file.as_fd()).map_err(std::io::Error::from)?;
    let file_kind = metadata.st_mode & 0o170_000;
    if file_kind != expected_kind
        || metadata.st_dev != named_before.st_dev
        || metadata.st_ino != named_before.st_ino
    {
        return Ok(None);
    }
    Ok(Some(ExistingArtifact {
        file,
        parent,
        name: name.to_owned(),
        ancestors,
    }))
}

#[cfg(unix)]
const fn same_directory(left: &rustix::fs::Stat, right: &rustix::fs::Stat) -> bool {
    left.st_dev == right.st_dev && left.st_ino == right.st_ino
}

#[cfg(unix)]
const fn private_directory_owned_by(stat: &rustix::fs::Stat, uid: u32) -> bool {
    stat.st_uid == uid
        && stat.st_mode & 0o170_000 == 0o040_000
        && stat.st_mode.trailing_zeros() >= 6
}

#[cfg(unix)]
fn cleanup_spool_name_if_same(parent: &File, name: &str, expected: &rustix::fs::Stat) {
    use rustix::fs::{self, AtFlags};
    use std::os::fd::AsFd;

    let Ok(current) = fs::statat(parent.as_fd(), name, AtFlags::SYMLINK_NOFOLLOW) else {
        return;
    };
    if same_directory(&current, expected)
        && private_directory_owned_by(&current, rustix::process::geteuid().as_raw())
    {
        let _ = fs::unlinkat(parent.as_fd(), name, AtFlags::REMOVEDIR);
    }
}

#[cfg(unix)]
fn open_directory(path: &Utf8Path, create: bool) -> crate::Result<File> {
    use rustix::{
        fs::{self, Mode, OFlags},
        io::Errno,
    };
    use std::os::fd::AsFd;

    let root = fs::open(
        "/",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(std::io::Error::from)?;
    let mut directory = File::from(root);
    let components = path.as_std_path().components().collect::<Vec<_>>();
    for component in &components {
        let name = match component {
            std::path::Component::RootDir | std::path::Component::CurDir => continue,
            std::path::Component::ParentDir => {
                return Err(crate::Error::InvalidPath(
                    "directory path was not canonicalized".to_owned(),
                ));
            }
            std::path::Component::Normal(name) => name.to_str().ok_or_else(|| {
                crate::Error::InvalidPath("destination path is not UTF-8".to_owned())
            })?,
            std::path::Component::Prefix(_) => {
                return Err(crate::Error::InvalidPath(
                    "destination path prefixes are unsupported on Unix".to_owned(),
                ));
            }
        };
        let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW;
        let next = match fs::openat(directory.as_fd(), name, flags, Mode::empty()) {
            Ok(next) => next,
            Err(error) if create && error == Errno::NOENT => {
                match fs::mkdirat(directory.as_fd(), name, Mode::from_raw_mode(0o755)) {
                    Ok(()) => directory.sync_all()?,
                    Err(error) if error == Errno::EXIST => {}
                    Err(error) => return Err(std::io::Error::from(error).into()),
                }
                fs::openat(directory.as_fd(), name, flags, Mode::empty())
                    .map_err(std::io::Error::from)?
            }
            Err(error) => return Err(std::io::Error::from(error).into()),
        };
        directory = File::from(next);
    }
    Ok(directory)
}

#[cfg(unix)]
fn directory_is_ancestor(ancestor: &File, descendant: &File) -> crate::Result<bool> {
    use rustix::fs::{self, Mode, OFlags};
    use std::os::fd::AsFd;

    let ancestor_stat = fs::fstat(ancestor.as_fd()).map_err(std::io::Error::from)?;
    let mut current = descendant.try_clone()?;
    loop {
        let current_stat = fs::fstat(current.as_fd()).map_err(std::io::Error::from)?;
        if current_stat.st_dev == ancestor_stat.st_dev
            && current_stat.st_ino == ancestor_stat.st_ino
        {
            return Ok(true);
        }
        let parent = fs::openat(
            current.as_fd(),
            "..",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(std::io::Error::from)?;
        let parent_stat = fs::fstat(&parent).map_err(std::io::Error::from)?;
        if parent_stat.st_dev == current_stat.st_dev && parent_stat.st_ino == current_stat.st_ino {
            return Ok(false);
        }
        current = File::from(parent);
    }
}

#[cfg(unix)]
struct StagedZip {
    parent: File,
    temporary_name: String,
    file_name: String,
    state: StagedZipState,
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StagedZipState {
    Staged,
    Replaced,
    Durable,
}

#[cfg(unix)]
struct SecureSpoolDirectory {
    parent: File,
    name: String,
    directory: File,
    source_directories: Vec<File>,
}

#[cfg(unix)]
impl SecureSpoolDirectory {
    fn create_file(&self, name: &str) -> crate::Result<File> {
        use rustix::fs::{self, Mode, OFlags};
        use std::os::fd::AsFd;

        self.ensure_disjoint()?;
        let file = fs::openat(
            self.directory.as_fd(),
            name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::from_raw_mode(0o600),
        )
        .map_err(std::io::Error::from)?;
        Ok(File::from(file))
    }

    fn open_file(&self, name: &str) -> crate::Result<File> {
        use rustix::fs::{self, OFlags};
        use std::os::fd::AsFd;

        self.ensure_disjoint()?;
        let file = fs::openat(
            self.directory.as_fd(),
            name,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            rustix::fs::Mode::empty(),
        )
        .map_err(std::io::Error::from)?;
        Ok(File::from(file))
    }

    fn ensure_disjoint(&self) -> crate::Result<()> {
        for source in &self.source_directories {
            if directory_is_ancestor(source, &self.directory)?
                || directory_is_ancestor(&self.directory, source)?
            {
                return Err(crate::Error::InvalidPath(
                    "source/archive staging overlap is not allowed".to_owned(),
                ));
            }
        }
        Ok(())
    }

    fn is_still_named(&self) -> bool {
        use rustix::fs::{self, AtFlags};
        use std::os::fd::AsFd;

        let Ok(opened) = fs::fstat(self.directory.as_fd()) else {
            return false;
        };
        let Ok(named) = fs::statat(
            self.parent.as_fd(),
            self.name.as_str(),
            AtFlags::SYMLINK_NOFOLLOW,
        ) else {
            return false;
        };
        opened.st_dev == named.st_dev && opened.st_ino == named.st_ino
    }
}

#[cfg(unix)]
impl StagedZip {
    fn replace(
        &mut self,
        output_root: &SecureOutputDirectory,
        hook: &mut dyn FnMut(usize, ArtifactPhase) -> io::Result<()>,
        artifact_index: usize,
    ) -> crate::Result<()> {
        use rustix::fs;
        use std::os::fd::AsFd;

        output_root.ensure_directory_disjoint(&self.parent)?;
        fs::renameat(
            self.parent.as_fd(),
            self.temporary_name.as_str(),
            self.parent.as_fd(),
            self.file_name.as_str(),
        )
        .map_err(std::io::Error::from)?;
        self.state = StagedZipState::Replaced;
        (hook)(artifact_index, ArtifactPhase::DirectorySync).map_err(|error| {
            crate::Error::ArtifactReplacedNotDurable {
                error: error.to_string(),
            }
        })?;
        self.parent
            .sync_all()
            .map_err(|error| crate::Error::ArtifactReplacedNotDurable {
                error: error.to_string(),
            })?;
        self.state = StagedZipState::Durable;
        Ok(())
    }
}

#[cfg(unix)]
impl Drop for StagedZip {
    fn drop(&mut self) {
        if self.state == StagedZipState::Staged {
            use rustix::{fs, fs::AtFlags};
            use std::os::fd::AsFd;

            let _ = fs::unlinkat(
                self.parent.as_fd(),
                self.temporary_name.as_str(),
                AtFlags::empty(),
            );
        }
    }
}

fn write_zip_group(
    group: &OutputGroup,
    zip_writer: &mut ZipWriter<BufWriter<File>>,
    artifact: &mut ArtifactContext<'_>,
) -> crate::Result<()> {
    #[cfg(unix)]
    artifact.output_root.ensure_disjoint()?;
    let session = SourceSession::open(&group.entries, artifact.checked_destination)?;
    let staged = stage_archive_sources(&session, artifact)?;
    for (entry, source) in group.entries.iter().zip(session.resolved) {
        #[cfg(unix)]
        artifact.output_root.ensure_disjoint()?;
        (artifact.hook)(artifact.artifact_index, ArtifactPhase::Write)?;
        zip_writer.start_file(entry.path.as_str(), artifact.options)?;
        write_entry_content(entry, source, &staged, zip_writer, artifact)?;
    }
    Ok(())
}

fn write_seven_zip_group(
    group: &OutputGroup,
    archive_writer: &mut r7z::ArchiveWriter<File>,
    artifact: &mut ArtifactContext<'_>,
) -> crate::Result<()> {
    #[cfg(unix)]
    artifact.output_root.ensure_disjoint()?;
    let session = SourceSession::open(&group.entries, artifact.checked_destination)?;
    let staged = stage_archive_sources(&session, artifact)?;
    for (entry, source) in group.entries.iter().zip(session.resolved) {
        #[cfg(unix)]
        artifact.output_root.ensure_disjoint()?;
        (artifact.hook)(artifact.artifact_index, ArtifactPhase::Write)?;
        let file = match source {
            ResolvedSource::Bare { path } => {
                (artifact.hook)(artifact.artifact_index, ArtifactPhase::Read)?;
                File::open(path)?
            }
            ResolvedSource::Archive {
                path,
                backend,
                selector,
            } => {
                let staged_path = staged
                    .members
                    .get(&(path.clone(), backend, selector))
                    .ok_or_else(|| {
                        crate::Error::InvalidPath(format!(
                            "selected archive member was not staged: {path}"
                        ))
                    })?;
                let spool = staged.spool.as_ref().ok_or_else(|| {
                    crate::Error::InvalidPath("archive staging was not initialized".to_owned())
                })?;
                spool.ensure_disjoint()?;
                spool.open_file(staged_path)?
            }
        };
        let mut verified = VerifiedContentReader::new(file, entry);
        archive_writer.append(entry.path.as_str(), &mut verified)?;
    }
    #[cfg(unix)]
    artifact.output_root.ensure_disjoint()?;
    Ok(())
}

#[cfg(unix)]
fn write_directory_group(
    group: &OutputGroup,
    directory: &File,
    artifact: &mut ArtifactContext<'_>,
) -> crate::Result<()> {
    let session = SourceSession::open(&group.entries, artifact.checked_destination)?;
    let staged = stage_archive_sources(&session, artifact)?;
    let mut staged_directories = BTreeMap::from([(Utf8PathBuf::new(), directory.try_clone()?)]);
    for (entry, source) in group.entries.iter().zip(session.resolved) {
        artifact.output_root.ensure_directory_disjoint(directory)?;
        (artifact.hook)(artifact.artifact_index, ArtifactPhase::Write)?;
        let mut output = artifact.output_root.create_directory_file(
            directory,
            entry.path.as_str(),
            &mut staged_directories,
        )?;
        write_entry_content(entry, source, &staged, &mut output, artifact)?;
        output.flush()?;
        output.sync_all()?;
    }
    let mut staged_directories = staged_directories.into_iter().collect::<Vec<_>>();
    staged_directories.sort_by_key(|(path, _)| std::cmp::Reverse(relative_path_depth(path)));
    for (path, directory) in staged_directories {
        (artifact.hook)(
            artifact.artifact_index,
            ArtifactPhase::StagedDirectorySync {
                depth: relative_path_depth(&path),
            },
        )?;
        directory.sync_all()?;
    }
    Ok(())
}

#[cfg(unix)]
fn relative_path_depth(path: &Utf8Path) -> usize {
    path.as_str()
        .split('/')
        .filter(|component| !component.is_empty())
        .count()
}

#[cfg(not(unix))]
fn write_directory_group(
    group: &OutputGroup,
    directory: &Utf8Path,
    artifact: &mut ArtifactContext<'_>,
) -> crate::Result<()> {
    let session = SourceSession::open(&group.entries, artifact.checked_destination)?;
    let staged = stage_archive_sources(&session, artifact)?;
    for (entry, source) in group.entries.iter().zip(session.resolved) {
        (artifact.hook)(artifact.artifact_index, ArtifactPhase::Write)?;
        let path = directory.join(entry.path.as_str());
        let parent = path.parent().ok_or_else(|| {
            crate::Error::InvalidPath(format!("directory entry has no parent: {path}"))
        })?;
        fs::create_dir_all(parent)?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        write_entry_content(entry, source, &staged, &mut output, artifact)?;
        output.flush()?;
        output.sync_all()?;
    }
    Ok(())
}

fn write_entry_content(
    entry: &LogicalEntry,
    source: ResolvedSource,
    staged: &StagedSources,
    output: &mut dyn Write,
    artifact: &mut ArtifactContext<'_>,
) -> crate::Result<()> {
    match source {
        ResolvedSource::Bare { path } => {
            (artifact.hook)(artifact.artifact_index, ArtifactPhase::Read)?;
            let mut source = File::open(&path)?;
            let mut content = ContentWriter::new(output);
            io::copy(&mut source, &mut content)?;
            verify_content(entry, content.finish())?;
        }
        ResolvedSource::Archive {
            path,
            backend,
            selector,
        } => {
            let staged_path = staged
                .members
                .get(&(path.clone(), backend, selector))
                .ok_or_else(|| {
                    crate::Error::InvalidPath(format!(
                        "selected archive member was not staged: {path}"
                    ))
                })?;
            let spool = staged.spool.as_ref().ok_or_else(|| {
                crate::Error::InvalidPath("archive staging was not initialized".to_owned())
            })?;
            spool.ensure_disjoint()?;
            io::copy(&mut spool.open_file(staged_path)?, output)?;
        }
    }
    Ok(())
}

type StagedMemberKey = (
    String,
    ArchiveBackend,
    crate::sources::ArchiveMemberSelector,
);
type StagedMembers = BTreeMap<StagedMemberKey, PathBuf>;

struct StagedSources {
    spool: Option<ArchiveSpool>,
    members: StagedMembers,
}

fn stage_archive_sources(
    session: &SourceSession<'_>,
    artifact: &mut ArtifactContext<'_>,
) -> crate::Result<StagedSources> {
    if session.archives.is_empty() {
        return Ok(StagedSources {
            spool: None,
            members: BTreeMap::new(),
        });
    }
    #[cfg(unix)]
    let spool = ArchiveSpool::create_secure(artifact.spool_parent, artifact.output_root)?;
    #[cfg(not(unix))]
    let spool = ArchiveSpool::create(artifact.spool_parent)?;
    let mut staged = BTreeMap::new();
    for ((path, backend), selectors) in &session.archives {
        spool.ensure_disjoint()?;
        let source_path = Utf8Path::new(path);
        let expected_fingerprints = session
            .archive_entries
            .iter()
            .filter(|((archive_path, archive_backend, _), _)| {
                archive_path == path && archive_backend == backend
            })
            .flat_map(|(_, entries)| entries.iter().map(|entry| entry.source.fingerprint))
            .collect::<BTreeSet<_>>();
        if expected_fingerprints.len() > 1 {
            return Err(crate::Error::InvalidPath(format!(
                "inconsistent archive fingerprints in build plan: {path}"
            )));
        }
        let expected_fingerprint = expected_fingerprints.iter().next().copied().flatten();
        let before = source_fingerprint(source_path)?;
        if expected_fingerprint.is_some_and(|expected| expected != before) {
            return Err(source_changed(
                path,
                "archive fingerprint differs from the plan",
            ));
        }
        let selectors = selectors.iter().cloned().collect::<Vec<_>>();
        crate::sources::stream_archive(
            source_path,
            *backend,
            Some(&selectors),
            |member, reader| {
                (artifact.hook)(artifact.artifact_index, ArtifactPhase::Read)?;
                let staged_path = spool.next_path()?;
                let remaining = spool.remaining_bytes();
                if member.size > remaining {
                    return Err(crate::Error::InvalidPath(format!(
                        "selected archive members exceed the {} GiB staging limit",
                        MAX_ARCHIVE_STAGING_BYTES / (1024 * 1024 * 1024)
                    )));
                }
                spool.ensure_disjoint()?;
                let file = spool.create_file(&staged_path)?;
                let mut bounded = ArchiveSpoolWriter::new(file, remaining);
                let mut content = ContentWriter::new(&mut bounded);
                io::copy(reader, &mut content)?;
                spool.ensure_disjoint()?;
                let digest = content.finish();
                if digest.size != member.size {
                    return Err(source_changed(
                        path,
                        "decoded archive member size differs from its inventory",
                    ));
                }
                let written = bounded.written_bytes();
                for entry in session
                    .archive_entries
                    .get(&(path.clone(), *backend, member.selector.clone()))
                    .ok_or_else(|| {
                        crate::Error::InvalidPath(format!(
                            "selected archive member was not in the build plan: {path}"
                        ))
                    })?
                {
                    verify_content(entry, digest)?;
                }
                drop(bounded);
                spool.record_bytes(written)?;
                staged.insert(
                    (path.clone(), *backend, member.selector.clone()),
                    staged_path,
                );
                Ok(())
            },
        )?;
        let after = source_fingerprint(source_path)?;
        if after != before || expected_fingerprint.is_some_and(|expected| expected != after) {
            return Err(source_changed(path, "archive changed while being read"));
        }
    }
    Ok(StagedSources {
        spool: Some(spool),
        members: staged,
    })
}

/// A per-artifact view of plan entries with each archive and selected member grouped once.
struct SourceSession<'entry> {
    resolved: Vec<ResolvedSource>,
    archives: BTreeMap<(String, ArchiveBackend), BTreeSet<crate::sources::ArchiveMemberSelector>>,
    archive_entries: BTreeMap<
        (
            String,
            ArchiveBackend,
            crate::sources::ArchiveMemberSelector,
        ),
        Vec<&'entry LogicalEntry>,
    >,
}

impl<'entry> SourceSession<'entry> {
    fn open(
        entries: &'entry [LogicalEntry],
        checked_destination: &crate::build::validation::CheckedPlanDestination,
    ) -> crate::Result<Self> {
        let resolved = entries
            .iter()
            .map(|entry| resolve_source(entry, checked_destination))
            .collect::<crate::Result<Vec<_>>>()?;
        let mut archives = BTreeMap::new();
        let mut archive_entries = BTreeMap::<
            (
                String,
                ArchiveBackend,
                crate::sources::ArchiveMemberSelector,
            ),
            Vec<&LogicalEntry>,
        >::new();
        for (entry, source) in entries.iter().zip(&resolved) {
            if let ResolvedSource::Archive {
                path,
                backend,
                selector,
            } = source
            {
                archives
                    .entry((path.clone(), *backend))
                    .or_insert_with(BTreeSet::new)
                    .insert(selector.clone());
                archive_entries
                    .entry((path.clone(), *backend, selector.clone()))
                    .or_default()
                    .push(entry);
            }
        }
        Ok(Self {
            resolved,
            archives,
            archive_entries,
        })
    }
}

fn source_fingerprint(path: &Utf8Path) -> crate::Result<SourceFingerprint> {
    let mut hasher = Sha1::new();
    io::copy(&mut File::open(path)?, &mut hasher)?;
    Ok(SourceFingerprint::new(hasher.finalize().into()))
}

fn source_changed(path: &str, reason: &str) -> crate::Error {
    crate::Error::SourceChanged {
        path: path.to_owned(),
        reason: reason.to_owned(),
    }
}

#[derive(Clone, Copy)]
struct ContentDigest {
    size: u64,
    sha1: [u8; 20],
    md5: [u8; 16],
    crc: [u8; 4],
    xxh3: [u8; 8],
}

struct ContentHasher {
    size: u64,
    sha1: Sha1,
    md5: Md5,
    crc: crc32fast::Hasher,
    xxh3: Xxh3,
}

impl ContentHasher {
    fn new() -> Self {
        Self {
            size: 0,
            sha1: Sha1::new(),
            md5: Md5::new(),
            crc: crc32fast::Hasher::new(),
            xxh3: Xxh3::new(),
        }
    }

    fn update(&mut self, bytes: &[u8]) -> io::Result<()> {
        let count = u64::try_from(bytes.len()).map_err(io::Error::other)?;
        self.size = self
            .size
            .checked_add(count)
            .ok_or_else(|| io::Error::other("source byte count overflowed"))?;
        self.sha1.update(bytes);
        self.md5.update(bytes);
        self.crc.update(bytes);
        self.xxh3.update(bytes);
        Ok(())
    }

    fn finish(self) -> ContentDigest {
        ContentDigest {
            size: self.size,
            sha1: self.sha1.finalize().into(),
            md5: self.md5.finalize().into(),
            crc: self.crc.finalize().to_be_bytes(),
            xxh3: self.xxh3.digest().to_be_bytes(),
        }
    }
}

struct ContentWriter<W> {
    inner: W,
    hasher: ContentHasher,
    maximum: Option<u64>,
}

impl<W> ContentWriter<W> {
    fn new(inner: W) -> Self {
        Self::with_maximum(inner, None)
    }

    fn with_limit(inner: W, maximum: u64) -> Self {
        Self::with_maximum(inner, Some(maximum))
    }

    fn with_maximum(inner: W, maximum: Option<u64>) -> Self {
        Self {
            inner,
            hasher: ContentHasher::new(),
            maximum,
        }
    }

    fn finish(self) -> ContentDigest {
        self.hasher.finish()
    }
}

impl<W: Write> Write for ContentWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let bounded = if let Some(maximum) = self.maximum {
            let remaining = maximum.saturating_sub(self.hasher.size);
            let allowed = usize::try_from(remaining)
                .unwrap_or(usize::MAX)
                .min(bytes.len());
            if allowed == 0 && !bytes.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "existing artifact member exceeds its planned size bound",
                ));
            }
            &bytes[..allowed]
        } else {
            bytes
        };
        let written = self.inner.write(bounded)?;
        let Some(written_bytes) = bytes.get(..written) else {
            return Err(io::Error::other("writer reported an invalid byte count"));
        };
        self.hasher.update(written_bytes)?;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

struct VerifiedContentReader<'entry, R> {
    inner: R,
    entry: &'entry LogicalEntry,
    hasher: Option<ContentHasher>,
}

impl<'entry, R> VerifiedContentReader<'entry, R> {
    fn new(inner: R, entry: &'entry LogicalEntry) -> Self {
        Self {
            inner,
            entry,
            hasher: Some(ContentHasher::new()),
        }
    }
}

impl<R: Read> Read for VerifiedContentReader<'_, R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() || self.hasher.is_none() {
            return Ok(0);
        }
        let read = self.inner.read(buffer)?;
        if read == 0 {
            if let Some(hasher) = self.hasher.take() {
                verify_content(self.entry, hasher.finish())
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            }
        } else if let Some(hasher) = self.hasher.as_mut() {
            hasher.update(&buffer[..read])?;
        }
        Ok(read)
    }
}

fn verify_content(entry: &LogicalEntry, actual: ContentDigest) -> crate::Result<()> {
    let observed = &entry.source.observed;
    if observed.scope == crate::domain::EvidenceScope::WholeAsset
        && (observed.size.is_some_and(|size| size != actual.size)
            || observed.sha1.is_some_and(|sha1| sha1 != actual.sha1)
            || observed.md5.is_some_and(|md5| md5.0 != actual.md5)
            || observed.crc.is_some_and(|crc| crc.0 != actual.crc)
            || observed.xxh3 != actual.xxh3)
    {
        return Err(source_changed(
            entry.source.location.path(),
            "streamed content differs from observed evidence",
        ));
    }

    let expected = &entry.expected;
    let matches = match entry.selection.strength {
        crate::resolution::MatchStrength::Sha1 => {
            expected.sha1.is_some_and(|sha1| sha1 == actual.sha1)
        }
        crate::resolution::MatchStrength::Md5 => {
            expected.md5.is_some_and(|md5| md5.0 == actual.md5)
        }
        crate::resolution::MatchStrength::CrcAndSize => {
            expected.crc.is_some_and(|crc| crc.0 == actual.crc)
                && expected.size.is_some_and(|size| size == actual.size)
        }
    };
    if !matches {
        return Err(source_changed(
            entry.source.location.path(),
            "streamed content differs from selected catalog evidence",
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
struct StagedArtifact {
    path: Utf8PathBuf,
}

#[cfg(not(unix))]
impl StagedArtifact {
    fn create(destination: &Utf8Path) -> crate::Result<(Self, File)> {
        let parent = destination.parent().ok_or_else(|| {
            crate::Error::InvalidPath(format!("artifact has no parent directory: {destination}"))
        })?;
        let name = destination.file_name().ok_or_else(|| {
            crate::Error::InvalidPath(format!("artifact has no file name: {destination}"))
        })?;
        for _ in 0..10 {
            let path = parent.join(format!(".{name}.{}.tmp", uuid::Uuid::new_v4()));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => return Ok((Self { path }, file)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate a staged artifact beside its destination",
        )
        .into())
    }

    fn replace(self, destination: &Utf8Path) -> crate::Result<()> {
        let _ = destination;
        Err(io::Error::other("atomic artifact replacement is supported only on Unix").into())
    }
}

#[cfg(not(unix))]
impl Drop for StagedArtifact {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(unix)]
struct StagedDirectory {
    parent: File,
    parent_path: Utf8PathBuf,
    staging_name: String,
    directory_name: String,
    directory: File,
    state: StagedDirectoryState,
}

#[cfg(unix)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum StagedDirectoryState {
    Staged,
    Replaced,
}

#[cfg(unix)]
impl StagedDirectory {
    fn replace(
        &mut self,
        output_root: &SecureOutputDirectory,
        destination: &Utf8Path,
        hook: &mut dyn FnMut(usize, ArtifactPhase) -> io::Result<()>,
        artifact_index: usize,
    ) -> crate::Result<ArtifactOutcome> {
        use rustix::fs::{self, AtFlags};
        use std::os::fd::AsFd;

        output_root.ensure_directory_disjoint(&self.directory)?;
        let existing = match fs::statat(
            self.parent.as_fd(),
            self.directory_name.as_str(),
            AtFlags::SYMLINK_NOFOLLOW,
        ) {
            Ok(metadata) => Some(metadata),
            Err(error) if error == rustix::io::Errno::NOENT => None,
            Err(error) => return Err(std::io::Error::from(error).into()),
        };
        if let Some(metadata) = existing
            && !rustix::fs::FileType::from_raw_mode(metadata.st_mode).is_dir()
        {
            return Err(crate::Error::InvalidPath(format!(
                "directory output destination exists but is not a real directory: {destination}"
            )));
        }

        let backup = if existing.is_some() {
            Some(self.backup_existing()?)
        } else {
            None
        };

        if let Err(install_error) = self.install() {
            return self.rollback(backup.as_ref(), destination, install_error);
        }
        self.state = StagedDirectoryState::Replaced;

        (hook)(artifact_index, ArtifactPhase::DirectorySync).map_err(|error| {
            directory_durability_error(destination, backup.as_ref(), error.to_string())
        })?;
        self.parent.sync_all().map_err(|error| {
            directory_durability_error(destination, backup.as_ref(), error.to_string())
        })?;

        if let Some(backup) = backup
            && let Err(error) = remove_directory_at(&self.parent, &backup.name, &self.parent_path)
        {
            let backup_path = backup.display_path(destination);
            return Ok(ArtifactOutcome::CompletedWithWarning {
                warning: format!(
                    "new directory installed, but old output backup remains at {backup_path} (cleanup failed: {error})"
                ),
            });
        }
        Ok(ArtifactOutcome::Completed)
    }

    fn backup_existing(&self) -> crate::Result<DirectoryBackup> {
        use rustix::fs::{self, Mode, OFlags};
        use std::os::fd::AsFd;

        for _ in 0..10 {
            let name = format!(".{}.backup-{}", self.directory_name, uuid::Uuid::new_v4());
            match fs::mkdirat(
                self.parent.as_fd(),
                name.as_str(),
                Mode::from_raw_mode(0o700),
            ) {
                Ok(()) => {
                    let directory = fs::openat(
                        self.parent.as_fd(),
                        name.as_str(),
                        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                        Mode::empty(),
                    )
                    .map_err(std::io::Error::from)?;
                    let directory = File::from(directory);
                    if let Err(error) = fs::renameat(
                        self.parent.as_fd(),
                        self.directory_name.as_str(),
                        &directory,
                        "previous-output",
                    ) {
                        let _ = fs::unlinkat(
                            self.parent.as_fd(),
                            name.as_str(),
                            rustix::fs::AtFlags::REMOVEDIR,
                        );
                        return Err(std::io::Error::from(error).into());
                    }
                    return Ok(DirectoryBackup { name, directory });
                }
                Err(error) if error == rustix::io::Errno::EXIST => {}
                Err(error) => return Err(std::io::Error::from(error).into()),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate a private directory backup",
        )
        .into())
    }

    fn install(&self) -> io::Result<()> {
        use rustix::fs::{self, RenameFlags};
        use std::os::fd::AsFd;

        fs::renameat_with(
            self.parent.as_fd(),
            self.staging_name.as_str(),
            self.parent.as_fd(),
            self.directory_name.as_str(),
            RenameFlags::NOREPLACE,
        )
        .map_err(std::io::Error::from)
    }

    fn rollback(
        &self,
        backup: Option<&DirectoryBackup>,
        destination: &Utf8Path,
        install_error: io::Error,
    ) -> crate::Result<ArtifactOutcome> {
        use rustix::fs::{self, RenameFlags};
        use std::os::fd::AsFd;

        let Some(backup) = backup else {
            return Err(install_error.into());
        };
        match fs::renameat_with(
            &backup.directory,
            "previous-output",
            self.parent.as_fd(),
            self.directory_name.as_str(),
            RenameFlags::NOREPLACE,
        ) {
            Ok(()) => {
                let _ = fs::unlinkat(
                    self.parent.as_fd(),
                    backup.name.as_str(),
                    rustix::fs::AtFlags::REMOVEDIR,
                );
                Err(install_error.into())
            }
            Err(rollback_error) => Err(crate::Error::InvalidPath(format!(
                "directory install failed ({install_error}); rollback failed ({rollback_error}); previous output preserved at {}",
                backup.display_path(destination)
            ))),
        }
    }
}

#[cfg(unix)]
struct DirectoryBackup {
    name: String,
    directory: File,
}

#[cfg(unix)]
impl DirectoryBackup {
    fn display_path(&self, destination: &Utf8Path) -> Utf8PathBuf {
        destination
            .parent()
            .unwrap_or_else(|| Utf8Path::new("."))
            .join(&self.name)
            .join("previous-output")
    }
}

#[cfg(unix)]
fn directory_durability_error(
    destination: &Utf8Path,
    backup: Option<&DirectoryBackup>,
    error: String,
) -> crate::Error {
    let error = match backup {
        Some(backup) => format!(
            "{error}; previous output preserved at {}",
            backup.display_path(destination)
        ),
        None => error,
    };
    crate::Error::ArtifactReplacedNotDurable { error }
}

#[cfg(unix)]
impl Drop for StagedDirectory {
    fn drop(&mut self) {
        if self.state == StagedDirectoryState::Staged {
            let _ = remove_directory_at(&self.parent, &self.staging_name, &self.parent_path);
        }
    }
}

#[cfg(unix)]
fn remove_directory_at(parent: &File, name: &str, parent_path: &Utf8Path) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        // macOS does not support traversing into a directory through /dev/fd/N.
        // The path was canonicalized and every component opened without following
        // symlinks before staging; concurrent local filesystem changes are outside
        // the writer's threat model.
        let _ = parent;
        fs::remove_dir_all(parent_path.join(name))
    }
    #[cfg(not(target_os = "macos"))]
    {
        use std::os::fd::AsRawFd;

        let _ = parent_path;

        let path = PathBuf::from(format!("/dev/fd/{}/{name}", parent.as_raw_fd()));
        fs::remove_dir_all(path)
    }
}

#[cfg(not(unix))]
struct StagedDirectory {
    path: Utf8PathBuf,
}

#[cfg(not(unix))]
impl StagedDirectory {
    fn create(destination: &Utf8Path) -> crate::Result<Self> {
        let parent = destination.parent().ok_or_else(|| {
            crate::Error::InvalidPath(format!("artifact has no parent directory: {destination}"))
        })?;
        let name = destination.file_name().ok_or_else(|| {
            crate::Error::InvalidPath(format!("artifact has no file name: {destination}"))
        })?;
        let path = create_private_directory(parent, &format!(".{name}.stage"))?;
        Ok(Self { path })
    }

    fn path(&self) -> &Utf8Path {
        &self.path
    }

    fn replace(self, destination: &Utf8Path) -> crate::Result<ArtifactOutcome> {
        replace_staged_directory(&self.path, destination, &mut |from, to| {
            fs::rename(from, to)
        })
    }
}

#[cfg(any(not(unix), test))]
fn replace_staged_directory(
    staged: &Utf8Path,
    destination: &Utf8Path,
    rename: &mut impl FnMut(&Utf8Path, &Utf8Path) -> io::Result<()>,
) -> crate::Result<ArtifactOutcome> {
    let metadata = match fs::symlink_metadata(destination) {
        Ok(metadata) => Some(metadata),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    let Some(metadata) = metadata else {
        rename(staged, destination)?;
        return Ok(ArtifactOutcome::Completed);
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(crate::Error::InvalidPath(format!(
            "directory output destination exists but is not a real directory: {destination}"
        )));
    }

    let parent = destination.parent().ok_or_else(|| {
        crate::Error::InvalidPath(format!("artifact has no parent directory: {destination}"))
    })?;
    let name = destination.file_name().ok_or_else(|| {
        crate::Error::InvalidPath(format!("artifact has no file name: {destination}"))
    })?;
    let backup_root = create_private_directory(parent, &format!(".{name}.backup"))?;
    let backup = backup_root.join("previous-output");
    if let Err(error) = rename(destination, &backup) {
        let _ = fs::remove_dir(&backup_root);
        return Err(error.into());
    }
    if let Err(install_error) = rename(staged, destination) {
        return match rename(&backup, destination) {
            Ok(()) => {
                let _ = fs::remove_dir(&backup_root);
                Err(install_error.into())
            }
            Err(rollback_error) => Err(crate::Error::InvalidPath(format!(
                "directory install failed ({install_error}); rollback failed ({rollback_error}); previous output preserved at {backup}"
            ))),
        };
    }
    if let Err(error) = fs::remove_dir_all(&backup_root) {
        return Ok(ArtifactOutcome::CompletedWithWarning {
            warning: format!(
                "new directory installed, but old output backup remains at {backup} (cleanup failed: {error})"
            ),
        });
    }
    Ok(ArtifactOutcome::Completed)
}

#[cfg(not(unix))]
impl Drop for StagedDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(any(not(unix), test))]
fn create_private_directory(parent: &Utf8Path, prefix: &str) -> crate::Result<Utf8PathBuf> {
    for _ in 0..10 {
        let path = parent.join(format!("{prefix}-{}", uuid::Uuid::new_v4()));
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        builder.mode(0o700);
        match builder.create(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a private directory beside its destination",
    )
    .into())
}

#[derive(Clone, Debug)]
enum ResolvedSource {
    Bare {
        path: String,
    },
    Archive {
        path: String,
        backend: ArchiveBackend,
        selector: crate::sources::ArchiveMemberSelector,
    },
}

fn resolve_source(
    entry: &LogicalEntry,
    checked_destination: &crate::build::validation::CheckedPlanDestination,
) -> crate::Result<ResolvedSource> {
    match &entry.source.location {
        SourceLocation::BareFile { path } => Ok(ResolvedSource::Bare {
            path: checked_destination.source_path(path)?.to_string(),
        }),
        SourceLocation::ArchiveMember {
            path,
            backend,
            selector: PlannedArchiveMemberSelector::IndexAndName { index, name },
        } => {
            let index = usize::try_from(*index).map_err(|_| {
                crate::Error::InvalidPath(format!("archive member index is too large: {index}"))
            })?;
            let name = crate::sources::normalize_member_name(name)?;
            Ok(ResolvedSource::Archive {
                path: checked_destination.source_path(path)?.to_string(),
                backend: *backend,
                selector: crate::sources::ArchiveMemberSelector { index, name },
            })
        }
        SourceLocation::LegacyUnknown { path, member_name } => {
            let path = checked_destination.source_path(path)?;
            let name = member_name.as_deref().ok_or_else(|| {
                crate::Error::InvalidPath(format!(
                    "archive source has no entry name: {}",
                    entry.source.display_name()
                ))
            })?;
            let backend = match crate::sources::detect(Utf8Path::new(path))? {
                crate::sources::SourceKind::Archive(backend) => backend,
                crate::sources::SourceKind::BareFile => {
                    return Err(crate::Error::InvalidPath(format!(
                        "legacy archive source is not an archive: {path}"
                    )));
                }
            };
            let name = crate::sources::normalize_member_name(name)?;
            let matches = crate::sources::enumerate(Utf8Path::new(path), backend)?
                .into_iter()
                .filter(|member| member.selector.name == name)
                .collect::<Vec<_>>();
            let [member] = matches.as_slice() else {
                return Err(crate::Error::InvalidPath(format!(
                    "legacy archive member name is {} in {path}: {name}",
                    if matches.is_empty() {
                        "missing"
                    } else {
                        "ambiguous"
                    }
                )));
            };
            Ok(ResolvedSource::Archive {
                path: path.to_string(),
                backend,
                selector: member.selector.clone(),
            })
        }
    }
}

struct ArchiveSpool {
    _directory: Option<crate::private_temp::PrivateTempDir>,
    #[cfg(unix)]
    secure_directory: Option<SecureSpoolDirectory>,
    root: PathBuf,
    next: std::cell::Cell<u64>,
    staged_bytes: std::cell::Cell<u64>,
}

impl ArchiveSpool {
    #[cfg(any(not(unix), test))]
    fn create(parent: &Utf8Path) -> crate::Result<Self> {
        let directory = crate::private_temp::PrivateTempDir::create_in(
            parent.as_std_path(),
            "mame-coalesce-build-",
        )?;
        let root = directory.path().to_path_buf();
        Ok(Self {
            _directory: Some(directory),
            #[cfg(unix)]
            secure_directory: None,
            root,
            next: std::cell::Cell::new(0),
            staged_bytes: std::cell::Cell::new(0),
        })
    }

    #[cfg(unix)]
    fn create_secure(
        parent: &Utf8Path,
        output_root: &SecureOutputDirectory,
    ) -> crate::Result<Self> {
        let secure_directory = output_root.create_spool_directory()?;
        Ok(Self {
            _directory: None,
            root: parent.join(&secure_directory.name).into_std_path_buf(),
            secure_directory: Some(secure_directory),
            next: std::cell::Cell::new(0),
            staged_bytes: std::cell::Cell::new(0),
        })
    }

    fn next_path(&self) -> crate::Result<PathBuf> {
        let index = self.next.get();
        self.next.set(index.checked_add(1).ok_or_else(|| {
            crate::Error::InvalidPath("archive staging file count overflowed".to_owned())
        })?);
        Ok(self.root.join(format!("{index}.member")))
    }

    fn create_file(&self, path: &std::path::Path) -> crate::Result<File> {
        #[cfg(unix)]
        if let Some(directory) = &self.secure_directory {
            let name = path
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                .ok_or_else(|| crate::Error::InvalidPath("invalid archive staging name".into()))?;
            return directory.create_file(name);
        }
        Ok(OpenOptions::new().write(true).create_new(true).open(path)?)
    }

    fn open_file(&self, path: &std::path::Path) -> crate::Result<File> {
        #[cfg(unix)]
        if let Some(directory) = &self.secure_directory {
            let name = path
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                .ok_or_else(|| crate::Error::InvalidPath("invalid archive staging name".into()))?;
            return directory.open_file(name);
        }
        Ok(File::open(path)?)
    }

    fn ensure_disjoint(&self) -> crate::Result<()> {
        #[cfg(unix)]
        if let Some(directory) = &self.secure_directory {
            return directory.ensure_disjoint();
        }
        Ok(())
    }

    const fn remaining_bytes(&self) -> u64 {
        MAX_ARCHIVE_STAGING_BYTES - self.staged_bytes.get()
    }

    fn record_bytes(&self, count: u64) -> crate::Result<()> {
        let total = self.staged_bytes.get().checked_add(count).ok_or_else(|| {
            crate::Error::InvalidPath("archive staging byte count overflowed".to_owned())
        })?;
        if total > MAX_ARCHIVE_STAGING_BYTES {
            return Err(crate::Error::InvalidPath(format!(
                "selected archive members exceed the {} GiB staging limit",
                MAX_ARCHIVE_STAGING_BYTES / (1024 * 1024 * 1024)
            )));
        }
        self.staged_bytes.set(total);
        Ok(())
    }
}

#[cfg(unix)]
impl Drop for ArchiveSpool {
    fn drop(&mut self) {
        use rustix::fs::{self, AtFlags};
        use std::os::fd::AsFd;

        let Some(directory) = &self.secure_directory else {
            return;
        };
        for index in 0..self.next.get() {
            let name = format!("{index}.member");
            let _ = fs::unlinkat(directory.directory.as_fd(), name.as_str(), AtFlags::empty());
        }
        if directory.is_still_named() {
            let _ = fs::unlinkat(
                directory.parent.as_fd(),
                directory.name.as_str(),
                AtFlags::REMOVEDIR,
            );
        }
    }
}

struct ArchiveSpoolWriter {
    file: File,
    remaining: u64,
    written: u64,
}

impl ArchiveSpoolWriter {
    const fn new(file: File, remaining: u64) -> Self {
        Self {
            file,
            remaining,
            written: 0,
        }
    }

    const fn written_bytes(&self) -> u64 {
        self.written
    }
}

impl Write for ArchiveSpoolWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            return Err(io::Error::other("archive staging limit exceeded"));
        }
        let allowed = usize::try_from(self.remaining).unwrap_or(usize::MAX);
        let Some(chunk) = buffer.get(..buffer.len().min(allowed)) else {
            return Err(io::Error::other("archive staging chunk length was invalid"));
        };
        let written = self.file.write(chunk)?;
        let written_u64 = u64::try_from(written)
            .map_err(|_| io::Error::other("archive staging byte count overflowed"))?;
        self.remaining -= written_u64;
        self.written = self
            .written
            .checked_add(written_u64)
            .ok_or_else(|| io::Error::other("archive staging byte count overflowed"))?;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use std::{
        fs,
        io::{self, Read, Write},
    };

    use proptest::prelude::*;

    use crate::domain::{
        ArchiveMemberSelector, ArtifactReusePolicy, BuildReport, CatalogKey, ExpectedEvidence,
        LogicalEntry, LogicalPath, MatchingPolicy, OutputGroup, RequirementKey,
        SelectionProvenance, SetKey, SourceFile,
    };

    #[derive(Clone, Copy)]
    enum SourceKind {
        Zip,
        Archive,
        Rar,
    }

    use super::*;

    fn utf8_path(path: &std::path::Path) -> Result<&Utf8Path, io::Error> {
        Utf8Path::from_path(path).ok_or_else(|| io::Error::other("path is not UTF-8"))
    }

    fn source_fixture_path(
        temp_dir: &tempfile::TempDir,
        name: &str,
    ) -> Result<Utf8PathBuf, Box<dyn std::error::Error>> {
        let source_root = utf8_path(temp_dir.path())?.join("sources");
        fs::create_dir_all(&source_root)?;
        Ok(source_root.join(name))
    }

    #[test]
    fn directory_output_reuses_unsafe_name_and_group_collision_validation()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let source_root = root.join("sources");
        fs::create_dir(&source_root)?;
        let source_path = source_root.join("source.rom");
        fs::write(&source_path, b"rom")?;
        let entry = logical_entry("game.rom", source_file(&source_path));
        let destination = root.join("output");

        let unsafe_plan = BuildPlan {
            groups: vec![OutputGroup {
                path: LogicalPath::new("../escape"),
                entries: vec![entry.clone()],
            }],
            report: BuildReport::default(),
        };
        let unsafe_result = write_plan_with_container(
            &unsafe_plan,
            &destination,
            OutputContainer::Directory,
            ZipCompression::Deflate,
        );
        let Err(error) = unsafe_result else {
            return Err(io::Error::other("unsafe directory group paths must be rejected").into());
        };
        assert!(
            error
                .to_string()
                .contains("unsafe logical output group path")
        );
        assert!(!destination.exists());

        let colliding_plan = BuildPlan {
            groups: vec![
                OutputGroup {
                    path: LogicalPath::new("safe"),
                    entries: vec![entry.clone()],
                },
                OutputGroup {
                    path: LogicalPath::new("safe/nested"),
                    entries: vec![entry],
                },
            ],
            report: BuildReport::default(),
        };
        let collision_result = write_plan_with_container(
            &colliding_plan,
            &destination,
            OutputContainer::Directory,
            ZipCompression::Deflate,
        );
        let Err(error) = collision_result else {
            return Err(io::Error::other("overlapping logical groups must be rejected").into());
        };
        assert!(
            error
                .to_string()
                .contains("directory artifact file/directory conflict"),
            "{error}"
        );
        assert!(!destination.exists());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn directory_output_rejects_symlinked_group_parent_without_touching_target()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let source_root = root.join("sources");
        fs::create_dir(&source_root)?;
        let source_path = source_root.join("source.rom");
        fs::write(&source_path, b"rom")?;

        let destination = root.join("output");
        let output_parent = destination.join("nested");
        fs::create_dir(&destination)?;
        let outside = root.join("outside");
        let outside_group = outside.join("set");
        fs::create_dir_all(&outside_group)?;
        let protected = outside_group.join("existing.rom");
        fs::write(&protected, b"must remain untouched")?;
        symlink(&outside, &output_parent)?;

        let plan = BuildPlan {
            groups: vec![OutputGroup {
                path: LogicalPath::new("nested/set"),
                entries: vec![logical_entry("game.rom", source_file(&source_path))],
            }],
            report: BuildReport::default(),
        };

        let results = write_plan_with_container(
            &plan,
            &destination,
            OutputContainer::Directory,
            ZipCompression::Deflate,
        )?;

        assert!(matches!(
            results.first().map(|result| &result.outcome),
            Some(ArtifactOutcome::Failed { .. })
        ));
        assert_eq!(fs::read(&protected)?, b"must remain untouched");
        assert!(!outside_group.join("game.rom").exists());
        assert_eq!(fs::read_link(output_parent)?, outside);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn directory_output_replaces_existing_group_through_pinned_parent()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let source_root = root.join("sources");
        fs::create_dir(&source_root)?;
        let source_path = source_root.join("source.rom");
        fs::write(&source_path, b"new bytes")?;

        let destination = root.join("output");
        let existing_group = destination.join("nested/set");
        fs::create_dir_all(&existing_group)?;
        fs::write(existing_group.join("old.rom"), b"old bytes")?;

        let plan = BuildPlan {
            groups: vec![OutputGroup {
                path: LogicalPath::new("nested/set"),
                entries: vec![logical_entry("new.rom", source_file(&source_path))],
            }],
            report: BuildReport::default(),
        };

        let results = write_plan_with_container(
            &plan,
            &destination,
            OutputContainer::Directory,
            ZipCompression::Deflate,
        )?;

        assert_eq!(results[0].outcome, ArtifactOutcome::Completed);
        assert_eq!(fs::read(existing_group.join("new.rom"))?, b"new bytes");
        assert!(!existing_group.join("old.rom").exists());
        assert_eq!(fs::read_dir(destination.join("nested"))?.count(), 1);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn staged_directory_sync_failure_preserves_existing_output()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let source_root = root.join("sources");
        fs::create_dir(&source_root)?;
        let source_path = source_root.join("source.rom");
        fs::write(&source_path, b"new bytes")?;

        let destination = root.join("output");
        let existing_group = destination.join("nested/set");
        fs::create_dir_all(&existing_group)?;
        fs::write(existing_group.join("old.rom"), b"old bytes")?;
        let plan = BuildPlan {
            groups: vec![OutputGroup {
                path: LogicalPath::new("nested/set"),
                entries: vec![
                    logical_entry("subdir/nested/new.rom", source_file(&source_path)),
                    logical_entry("other.rom", source_file(&source_path)),
                ],
            }],
            report: BuildReport::default(),
        };
        let mut sync_depths = Vec::new();
        let mut hook = |_, phase| match phase {
            ArtifactPhase::StagedDirectorySync { depth } => {
                sync_depths.push(depth);
                if depth == 0 {
                    Err(io::Error::other("injected staged directory sync failure"))
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
        };

        let results = write_plan_with_hook(
            &plan,
            &destination,
            OutputContainer::Directory,
            file_options(ZipCompression::Deflate),
            ArtifactReusePolicy::Replace,
            &mut hook,
        )?;

        assert!(matches!(
            results.first().map(|result| &result.outcome),
            Some(ArtifactOutcome::Failed { error })
                if error.contains("injected staged directory sync failure")
        ));
        assert_eq!(sync_depths, vec![2, 1, 0]);
        assert_eq!(fs::read(existing_group.join("old.rom"))?, b"old bytes");
        assert!(!existing_group.join("subdir/nested/new.rom").exists());
        assert_eq!(fs::read_dir(destination.join("nested"))?.count(), 1);
        Ok(())
    }

    #[test]
    fn directory_replace_rolls_back_existing_nonempty_output_on_install_failure()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let destination = root.join("output");
        let staged = root.join(".output.stage");
        fs::create_dir(&destination)?;
        fs::write(destination.join("old.rom"), b"old")?;
        fs::create_dir(&staged)?;
        fs::write(staged.join("new.rom"), b"new")?;
        let mut calls = 0;

        let result = replace_staged_directory(&staged, &destination, &mut |from, to| {
            calls += 1;
            if calls == 2 {
                Err(io::Error::other("injected install failure"))
            } else {
                fs::rename(from, to)
            }
        });

        assert!(result.is_err());
        assert_eq!(fs::read(destination.join("old.rom"))?, b"old");
        assert!(!destination.join("new.rom").exists());
        assert!(staged.join("new.rom").exists());
        Ok(())
    }

    #[test]
    fn directory_replace_replaces_existing_nonempty_output_and_removes_backup()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let destination = root.join("output");
        let staged = root.join(".output.stage");
        fs::create_dir(&destination)?;
        fs::write(destination.join("old.rom"), b"old")?;
        fs::create_dir_all(staged.join("nested"))?;
        fs::write(staged.join("nested/new.rom"), b"new")?;

        let result =
            replace_staged_directory(&staged, &destination, &mut |from, to| fs::rename(from, to))?;

        assert_eq!(result, ArtifactOutcome::Completed);
        assert!(!destination.join("old.rom").exists());
        assert_eq!(fs::read(destination.join("nested/new.rom"))?, b"new");
        assert_eq!(fs::read_dir(root)?.count(), 1);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn directory_replace_refuses_to_follow_destination_symlinks()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let previous = root.join("previous");
        let destination = root.join("output");
        let staged = root.join(".output.stage");
        fs::create_dir(&previous)?;
        fs::write(previous.join("old.rom"), b"old")?;
        symlink(&previous, &destination)?;
        fs::create_dir(&staged)?;

        let result =
            replace_staged_directory(&staged, &destination, &mut |from, to| fs::rename(from, to));
        let Err(error) = result else {
            return Err(io::Error::other("destination symlinks must fail safely").into());
        };

        assert!(error.to_string().contains("not a real directory"));
        assert!(destination.symlink_metadata()?.file_type().is_symlink());
        assert_eq!(fs::read(previous.join("old.rom"))?, b"old");
        Ok(())
    }

    #[test]
    fn directory_replace_preserves_named_backup_when_rollback_fails()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let destination = root.join("output");
        let staged = root.join(".output.stage");
        fs::create_dir(&destination)?;
        fs::write(destination.join("old.rom"), b"old")?;
        fs::create_dir(&staged)?;
        fs::write(staged.join("new.rom"), b"new")?;
        let mut calls = 0;

        let result = replace_staged_directory(&staged, &destination, &mut |from, to| {
            calls += 1;
            if calls >= 2 {
                Err(io::Error::other("injected rename failure"))
            } else {
                fs::rename(from, to)
            }
        });
        let Err(error) = result else {
            return Err(io::Error::other("install and rollback are injected to fail").into());
        };

        assert!(error.to_string().contains("previous output preserved at"));
        assert!(!destination.exists());
        let backup_root = fs::read_dir(root)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with(".output.backup-"))
            })
            .ok_or_else(|| io::Error::other("recoverable backup directory missing"))?;
        assert_eq!(
            fs::read(backup_root.join("previous-output/old.rom"))?,
            b"old"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn archive_spool_directory_is_private() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;

        let parent = tempfile::tempdir()?;
        let parent_path = Utf8Path::from_path(parent.path())
            .ok_or_else(|| io::Error::other("temporary path is not UTF-8"))?;
        let spool = ArchiveSpool::create(parent_path)?;
        assert_eq!(spool.root.parent(), Some(parent.path()));
        let mode = std::fs::metadata(&spool.root)?.permissions().mode();
        assert_eq!(mode & 0o077, 0);
        Ok(())
    }

    #[allow(clippy::expect_used)]
    fn source_file(path: &Utf8Path) -> SourceFile {
        let bytes = std::fs::read(path).ok();
        let sha1 = bytes.as_deref().map(crate::hashes::sha1_bytes);
        let xxh3 = bytes
            .as_deref()
            .map_or([0; 8], crate::hashes::xxhash3_bytes);
        let fingerprint = sha1.map(SourceFingerprint::new);
        let location = SourceLocation::BareFile {
            path: path.as_str().to_owned(),
        };
        SourceFile {
            source_root: crate::domain::SourceRoot::new(
                path.parent()
                    .expect("test source file has a parent")
                    .as_str(),
            ),
            physical_path: crate::domain::SourcePhysicalPath::from_location(&location),
            location,
            observed: crate::domain::ObservedContent {
                scope: crate::domain::EvidenceScope::WholeAsset,
                provenance: crate::domain::EvidenceProvenance::Computed,
                size: None,
                crc: None,
                md5: None,
                sha1,
                xxh3,
            },
            fingerprint,
            scan_run: None,
            scan_provenance: None,
            bare_file_cache_stamp: None,
        }
    }

    fn logical_entry(path: &str, source: SourceFile) -> LogicalEntry {
        let expected = ExpectedEvidence {
            scope: crate::domain::EvidenceScope::WholeAsset,
            sha1: source.observed.sha1,
            ..ExpectedEvidence::default()
        };
        LogicalEntry {
            path: LogicalPath::new(path),
            source,
            requirement: RequirementKey::new(
                SetKey::new(CatalogKey::new("writer-test"), path),
                path,
            ),
            expected,
            selection: SelectionProvenance {
                policy: MatchingPolicy::Sha1Compatibility,
                strength: crate::resolution::MatchStrength::Sha1,
                assessments: Vec::new(),
                omitted_assessments: 0,
            },
        }
    }

    #[allow(clippy::expect_used)]
    fn archive_source_file(
        path: &Utf8Path,
        entry_name: Option<&str>,
        kind: SourceKind,
    ) -> SourceFile {
        let observed_bytes = entry_name.and_then(|name| archive_member_bytes(path, name, kind));
        let sha1 = observed_bytes.as_deref().map(crate::hashes::sha1_bytes);
        let xxh3 = observed_bytes
            .as_deref()
            .map_or([0; 8], crate::hashes::xxhash3_bytes);
        let fingerprint = std::fs::read(path)
            .ok()
            .map(|bytes| SourceFingerprint::new(crate::hashes::sha1_bytes(&bytes)));
        let location = entry_name.map_or_else(
            || SourceLocation::LegacyUnknown {
                path: path.as_str().to_owned(),
                member_name: None,
            },
            |name| {
                let backend = match kind {
                    SourceKind::Zip => ArchiveBackend::Zip,
                    SourceKind::Archive => ArchiveBackend::SevenZip,
                    SourceKind::Rar => ArchiveBackend::Rar,
                };
                let normalized = crate::sources::normalize_member_name(name).ok();
                let index = crate::sources::enumerate(path, backend)
                    .ok()
                    .and_then(|members| {
                        members.into_iter().find(|member| {
                            normalized.as_deref() == Some(member.selector.name.as_str())
                        })
                    })
                    .map_or(0, |member| member.selector.index as u64);
                SourceLocation::ArchiveMember {
                    path: path.as_str().to_owned(),
                    backend,
                    selector: ArchiveMemberSelector::IndexAndName {
                        index,
                        name: name.to_owned(),
                    },
                }
            },
        );
        let physical_path = crate::domain::SourcePhysicalPath::from_location(&location);
        SourceFile {
            source_root: crate::domain::SourceRoot::new(
                path.parent()
                    .expect("test archive source has a parent")
                    .as_str(),
            ),
            location,
            physical_path,
            observed: crate::domain::ObservedContent {
                scope: crate::domain::EvidenceScope::WholeAsset,
                provenance: crate::domain::EvidenceProvenance::Computed,
                size: None,
                crc: None,
                md5: None,
                sha1,
                xxh3,
            },
            fingerprint,
            scan_run: None,
            scan_provenance: None,
            bare_file_cache_stamp: None,
        }
    }

    fn archive_member_bytes(path: &Utf8Path, name: &str, kind: SourceKind) -> Option<Vec<u8>> {
        let backend = match kind {
            SourceKind::Zip => ArchiveBackend::Zip,
            SourceKind::Archive => ArchiveBackend::SevenZip,
            SourceKind::Rar => ArchiveBackend::Rar,
        };
        let normalized = crate::sources::normalize_member_name(name).ok()?;
        let member = crate::sources::enumerate(path, backend)
            .ok()?
            .into_iter()
            .find(|member| member.selector.name == normalized)?;
        let mut contents = Vec::new();
        crate::sources::stream_archive(path, backend, Some(&[member.selector]), |_, reader| {
            reader.read_to_end(&mut contents)?;
            Ok(())
        })
        .ok()?;
        Some(contents)
    }

    fn error_message(result: crate::Result<Vec<Utf8PathBuf>>) -> Result<String, &'static str> {
        match result {
            Ok(_) => Err("expected write plan to fail"),
            Err(error) => Ok(error.to_string()),
        }
    }

    fn write_source_zip(
        path: &Utf8Path,
        entries: &[(&str, &[u8])],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let file = File::create(path)?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        for (name, contents) in entries {
            zip.start_file(*name, options)?;
            zip.write_all(contents)?;
        }
        zip.finish()?;
        Ok(())
    }

    fn replace_zip_name(
        path: &Utf8Path,
        old: &str,
        new: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if old.len() != new.len() {
            return Err("ZIP test names must have equal byte lengths".into());
        }
        let mut bytes = fs::read(path)?;
        let mut cursor = 0;
        while let Some(relative) = bytes[cursor..]
            .windows(old.len())
            .position(|window| window == old.as_bytes())
        {
            cursor += relative;
            bytes[cursor..cursor + old.len()].copy_from_slice(new.as_bytes());
            cursor += old.len();
        }
        fs::write(path, bytes)?;
        Ok(())
    }

    fn mark_zip_entry_as_symlink(
        path: &Utf8Path,
        name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut bytes = fs::read(path)?;
        let header = bytes
            .windows(4)
            .position(|window| window == b"PK\x01\x02")
            .ok_or_else(|| io::Error::other("ZIP central entry was not written"))?;
        let name_length = usize::from(u16::from_le_bytes(
            bytes[header + 28..header + 30]
                .try_into()
                .map_err(|_| io::Error::other("invalid ZIP central entry"))?,
        ));
        if &bytes[header + 46..header + 46 + name_length] != name.as_bytes() {
            return Err("ZIP central entry name did not match the fixture".into());
        }
        bytes[header + 5] = 3;
        bytes[header + 38..header + 42].copy_from_slice(&(0o120_777_u32 << 16).to_le_bytes());
        fs::write(path, bytes)?;
        Ok(())
    }

    fn write_version_rar(path: &Utf8Path) -> Result<(), Box<dyn std::error::Error>> {
        let archive = hex::decode(
            "526172211a0700cf907300000d000000000000000f0c7420802700150000000b0000000345f37dc6a48a07471d330700a481000056455253494f4e0c008fec8a45cc23c848088362fe5fdd5c5388f072c43d7b00400700",
        )?;
        std::fs::write(path, archive)?;
        Ok(())
    }

    fn single_zip_entry_plan(
        archive_path: &Utf8Path,
        entry_name: &str,
        output_name: &str,
        kind: SourceKind,
    ) -> BuildPlan {
        BuildPlan {
            groups: vec![OutputGroup {
                path: LogicalPath::new("safe"),
                entries: vec![logical_entry(
                    output_name,
                    archive_source_file(archive_path, Some(entry_name), kind),
                )],
            }],
            report: BuildReport::default(),
        }
    }

    prop_compose! {
        fn safe_component()(name in prop_oneof![
            "[A-Za-z0-9]",
            "[A-Za-z0-9][A-Za-z0-9._-]{0,14}[A-Za-z0-9_-]",
        ]) -> String {
            name
        }
    }

    prop_compose! {
        fn unsafe_component()(
            name in prop_oneof![
                Just(String::new()),
                Just(".".to_owned()),
                Just("..".to_owned()),
                "[A-Za-z0-9]{1,8}\\\\[A-Za-z0-9]{1,8}",
                "[A-Za-z0-9]{1,8}\\x00[A-Za-z0-9]{1,8}",
                "[A-Za-z]:[A-Za-z0-9]{1,8}",
            ]
        ) -> String {
            name
        }
    }

    proptest! {
        #[test]
        fn generated_safe_path_components_are_accepted(component in safe_component()) {
            let nested = format!("nested/{component}");
            prop_assert!(validate_output_file_name(&component).is_ok());
            prop_assert!(validate_zip_entry_name(&component).is_ok());
            prop_assert!(validate_zip_entry_name(&nested).is_ok());
        }

        #[test]
        fn generated_unsafe_path_components_are_rejected(component in unsafe_component()) {
            prop_assert!(validate_output_file_name(&component).is_err());
            prop_assert!(validate_zip_entry_name(&component).is_err());
        }
    }

    #[test]
    fn output_zip_file_name_validation_rejects_unsafe_components() {
        for name in [
            "",
            ".",
            "..",
            "nested\\file.zip",
            "bad\0.zip",
            "C:bad.zip",
            "game:stream",
            "CON",
            "con.txt",
            "CONIN$",
            "CONOUT$",
            "PRN",
            "AUX",
            "NUL",
            "COM1",
            "LPT9",
            "trailing.",
            "trailing ",
        ] {
            assert!(
                validate_output_file_name(name).is_err(),
                "expected {name:?} to be rejected"
            );
        }
        assert!(validate_output_file_name("nested/file.zip").is_ok());
    }

    #[test]
    fn zip_entry_name_validation_rejects_unsafe_components() {
        for name in [
            "",
            ".",
            "..",
            "/absolute.rom",
            "nested/",
            "nested//game.rom",
            "nested/./game.rom",
            "nested/../game.rom",
            "nested\\game.rom",
            "bad\0.rom",
            "C:bad.rom",
        ] {
            assert!(
                validate_zip_entry_name(name).is_err(),
                "expected {name:?} to be rejected"
            );
        }
    }

    #[test]
    fn write_plan_does_not_create_destination_when_nothing_will_be_written()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let destination = utf8_path(temp_dir.path())?.join("output");

        for plan in [
            BuildPlan {
                groups: vec![OutputGroup {
                    path: LogicalPath::new("safe"),
                    entries: Vec::new(),
                }],
                report: BuildReport {
                    outcome: PlanOutcome::Blocked(crate::domain::PlanBlockReason::MissingContent),
                    ..BuildReport::default()
                },
            },
            BuildPlan {
                groups: Vec::new(),
                report: BuildReport::default(),
            },
        ] {
            assert!(write_plan(&plan, &destination)?.is_empty());
            assert!(!destination.exists());
        }

        Ok(())
    }

    #[test]
    fn failed_source_read_preserves_the_previous_artifact() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp_dir = tempfile::tempdir()?;
        let destination = utf8_path(temp_dir.path())?.join("output");
        std::fs::create_dir_all(&destination)?;
        let artifact = destination.join("safe.zip");
        write_source_zip(&artifact, &[("previous.rom", b"valid old artifact")])?;
        let previous = std::fs::read(&artifact)?;
        let missing_source = destination.join("missing.rom");
        let plan = BuildPlan {
            groups: vec![OutputGroup {
                path: LogicalPath::new("safe"),
                entries: vec![logical_entry("game.rom", source_file(&missing_source))],
            }],
            report: BuildReport::default(),
        };

        let result = write_plan(&plan, &destination);

        assert!(result.is_err());
        assert_eq!(std::fs::read(&artifact)?, previous);
        Ok(())
    }

    #[test]
    fn injected_artifact_failures_preserve_old_output_and_remove_staging_files()
    -> Result<(), Box<dyn std::error::Error>> {
        for failed_phase in [
            ArtifactPhase::Read,
            ArtifactPhase::Write,
            ArtifactPhase::Finalize,
            ArtifactPhase::Replace,
        ] {
            let temp_dir = tempfile::tempdir()?;
            let root = utf8_path(temp_dir.path())?;
            let source_path = source_fixture_path(&temp_dir, "source.rom")?;
            std::fs::write(&source_path, b"new artifact")?;
            let destination = root.join("output");
            std::fs::create_dir_all(&destination)?;
            let artifact = destination.join("safe.zip");
            write_source_zip(&artifact, &[("previous.rom", b"old valid artifact")])?;
            let previous = std::fs::read(&artifact)?;
            let plan = BuildPlan {
                groups: vec![OutputGroup {
                    path: LogicalPath::new("safe"),
                    entries: vec![logical_entry("game.rom", source_file(&source_path))],
                }],
                report: BuildReport::default(),
            };
            let mut hook = |index, phase| {
                if index == 0 && phase == failed_phase {
                    Err(io::Error::other("injected artifact failure"))
                } else {
                    Ok(())
                }
            };

            let results = write_plan_with_hook(
                &plan,
                &destination,
                OutputContainer::Zip,
                file_options(ZipCompression::Deflate),
                ArtifactReusePolicy::Replace,
                &mut hook,
            )?;

            let result = results
                .first()
                .ok_or_else(|| io::Error::other("expected one artifact result"))?;
            assert!(matches!(&result.outcome, ArtifactOutcome::Failed { .. }));
            assert_eq!(std::fs::read(&artifact)?, previous);
            assert_eq!(std::fs::read_dir(&destination)?.count(), 1);
        }
        Ok(())
    }

    #[test]
    fn directory_sync_failure_reports_visible_replacement_as_uncertain()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let source_path = source_fixture_path(&temp_dir, "source.rom")?;
        std::fs::write(&source_path, b"new artifact")?;
        let destination = root.join("output");
        std::fs::create_dir_all(&destination)?;
        let artifact = destination.join("safe.zip");
        write_source_zip(&artifact, &[("previous.rom", b"old valid artifact")])?;
        let plan = BuildPlan {
            groups: vec![OutputGroup {
                path: LogicalPath::new("safe"),
                entries: vec![logical_entry("game.rom", source_file(&source_path))],
            }],
            report: BuildReport::default(),
        };
        let mut hook = |_, phase| {
            if phase == ArtifactPhase::DirectorySync {
                Err(io::Error::other("injected directory sync failure"))
            } else {
                Ok(())
            }
        };

        let results = write_plan_with_hook(
            &plan,
            &destination,
            OutputContainer::Zip,
            file_options(ZipCompression::Deflate),
            ArtifactReusePolicy::Replace,
            &mut hook,
        )?;

        assert!(matches!(
            results.first().map(|result| &result.outcome),
            Some(ArtifactOutcome::ReplacedButNotDurable { error })
                if error.contains("injected directory sync failure")
        ));
        assert_eq!(
            archive_member_bytes(&artifact, "game.rom", SourceKind::Zip).as_deref(),
            Some(b"new artifact" as &[u8])
        );
        assert_eq!(std::fs::read_dir(&destination)?.count(), 1);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn directory_sync_failure_reports_retained_backup_path()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let source_path = source_fixture_path(&temp_dir, "source.rom")?;
        std::fs::write(&source_path, b"new output")?;
        let destination = root.join("output");
        let existing = destination.join("nested/set");
        std::fs::create_dir_all(&existing)?;
        std::fs::write(existing.join("old.rom"), b"previous output")?;
        let plan = BuildPlan {
            groups: vec![OutputGroup {
                path: LogicalPath::new("nested/set"),
                entries: vec![logical_entry("new.rom", source_file(&source_path))],
            }],
            report: BuildReport::default(),
        };
        let mut hook = |_, phase| {
            if phase == ArtifactPhase::DirectorySync {
                Err(io::Error::other("injected directory sync failure"))
            } else {
                Ok(())
            }
        };

        let results = write_plan_with_hook(
            &plan,
            &destination,
            OutputContainer::Directory,
            file_options(ZipCompression::Deflate),
            ArtifactReusePolicy::Replace,
            &mut hook,
        )?;

        let Some(ArtifactOutcome::ReplacedButNotDurable { error }) =
            results.first().map(|result| &result.outcome)
        else {
            return Err("expected a nondurable replacement result".into());
        };
        let entries = std::fs::read_dir(destination.join("nested"))?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()?;
        let backup = entries
            .iter()
            .find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(".set.backup-"))
            })
            .ok_or("expected a retained prior-output backup")?;
        let backup_path = backup.join("previous-output");
        assert!(error.contains("injected directory sync failure"));
        assert!(error.contains(backup_path.to_string_lossy().as_ref()));
        assert_eq!(
            std::fs::read(backup_path.join("old.rom"))?,
            b"previous output"
        );
        assert_eq!(std::fs::read(existing.join("new.rom"))?, b"new output");
        Ok(())
    }

    #[test]
    fn operating_system_replacement_failure_preserves_destination_directory()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let source_path = source_fixture_path(&temp_dir, "source.rom")?;
        std::fs::write(&source_path, b"new content")?;
        let destination = root.join("output");
        let artifact = destination.join("safe.zip");
        std::fs::create_dir_all(&artifact)?;
        let marker = artifact.join("preserve.txt");
        std::fs::write(&marker, b"existing directory")?;
        let plan = BuildPlan {
            groups: vec![OutputGroup {
                path: LogicalPath::new("safe"),
                entries: vec![logical_entry("game.rom", source_file(&source_path))],
            }],
            report: BuildReport::default(),
        };

        let results = write_plan_with_compression(&plan, &destination, ZipCompression::Deflate)?;
        let result = results
            .first()
            .ok_or_else(|| io::Error::other("expected one artifact result"))?;

        assert!(matches!(&result.outcome, ArtifactOutcome::Failed { .. }));
        assert_eq!(std::fs::read(&marker)?, b"existing directory");
        assert_eq!(std::fs::read_dir(&artifact)?.count(), 1);
        assert_eq!(std::fs::read_dir(&destination)?.count(), 1);
        Ok(())
    }

    #[test]
    fn later_artifact_failure_reports_completed_failed_and_unattempted()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let first_source = source_fixture_path(&temp_dir, "first.rom")?;
        let third_source = source_fixture_path(&temp_dir, "third.rom")?;
        std::fs::write(&first_source, b"first")?;
        std::fs::write(&third_source, b"third")?;
        let destination = root.join("output");
        std::fs::create_dir_all(&destination)?;
        let second_artifact = destination.join("second.zip");
        let third_artifact = destination.join("third.zip");
        write_source_zip(&second_artifact, &[("old.rom", b"keep second")])?;
        write_source_zip(&third_artifact, &[("old.rom", b"keep third")])?;
        let previous_second = std::fs::read(&second_artifact)?;
        let previous_third = std::fs::read(&third_artifact)?;
        let missing_source = source_fixture_path(&temp_dir, "missing.rom")?;
        let plan = BuildPlan {
            groups: vec![
                OutputGroup {
                    path: LogicalPath::new("first"),
                    entries: vec![logical_entry("first.rom", source_file(&first_source))],
                },
                OutputGroup {
                    path: LogicalPath::new("second"),
                    entries: vec![logical_entry("second.rom", source_file(&missing_source))],
                },
                OutputGroup {
                    path: LogicalPath::new("third"),
                    entries: vec![logical_entry("third.rom", source_file(&third_source))],
                },
            ],
            report: BuildReport::default(),
        };

        let results = write_plan_with_compression(&plan, &destination, ZipCompression::Deflate)?;

        assert_eq!(results.len(), 3);
        let mut outcomes = results.iter().map(|result| &result.outcome);
        assert_eq!(outcomes.next(), Some(&ArtifactOutcome::Completed));
        assert!(matches!(
            outcomes.next(),
            Some(ArtifactOutcome::Failed { .. })
        ));
        assert_eq!(outcomes.next(), Some(&ArtifactOutcome::Unattempted));
        assert!(outcomes.next().is_none());
        assert!(destination.join("first.zip").is_file());
        assert_eq!(std::fs::read(&second_artifact)?, previous_second);
        assert_eq!(std::fs::read(&third_artifact)?, previous_third);
        assert_eq!(std::fs::read_dir(&destination)?.count(), 3);
        let expected_third = third_artifact.canonicalize_utf8()?;
        assert_eq!(
            results.last().map(|result| result.path.as_str()),
            Some(expected_third.as_str())
        );
        Ok(())
    }

    #[test]
    fn changed_bare_source_is_rejected_without_replacing_old_output()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let source_path = source_fixture_path(&temp_dir, "source.rom")?;
        std::fs::write(&source_path, b"planned content")?;
        let destination = root.join("output");
        std::fs::create_dir_all(&destination)?;
        let artifact = destination.join("safe.zip");
        write_source_zip(&artifact, &[("old.rom", b"preserve me")])?;
        let previous = std::fs::read(&artifact)?;
        let plan = BuildPlan {
            groups: vec![OutputGroup {
                path: LogicalPath::new("safe"),
                entries: vec![logical_entry("game.rom", source_file(&source_path))],
            }],
            report: BuildReport::default(),
        };
        std::fs::write(&source_path, b"changed after planning")?;

        let results = write_plan_with_compression(&plan, &destination, ZipCompression::Deflate)?;
        let result = results
            .first()
            .ok_or_else(|| io::Error::other("expected one artifact result"))?;

        assert!(matches!(
            &result.outcome,
            ArtifactOutcome::Failed { error } if error.contains("source changed since planning")
        ));
        assert_eq!(std::fs::read(&artifact)?, previous);
        assert_eq!(std::fs::read_dir(&destination)?.count(), 1);
        Ok(())
    }

    #[test]
    fn changed_archive_member_is_rejected_without_replacing_old_output()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let archive_path = source_fixture_path(&temp_dir, "source.zip")?;
        write_source_zip(&archive_path, &[("game.rom", b"planned content")])?;
        let destination = root.join("output");
        std::fs::create_dir_all(&destination)?;
        let artifact = destination.join("safe.zip");
        write_source_zip(&artifact, &[("old.rom", b"preserve me")])?;
        let previous = std::fs::read(&artifact)?;
        let plan = single_zip_entry_plan(&archive_path, "game.rom", "game.rom", SourceKind::Zip);
        write_source_zip(&archive_path, &[("game.rom", b"changed after planning")])?;

        let results = write_plan_with_compression(&plan, &destination, ZipCompression::Deflate)?;
        let result = results
            .first()
            .ok_or_else(|| io::Error::other("expected one artifact result"))?;

        assert!(matches!(
            &result.outcome,
            ArtifactOutcome::Failed { error } if error.contains("source changed since planning")
        ));
        assert_eq!(std::fs::read(&artifact)?, previous);
        assert_eq!(std::fs::read_dir(&destination)?.count(), 1);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn successful_artifact_replacement_preserves_sources_and_leaves_no_staging_file()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let source_path = source_fixture_path(&temp_dir, "source.rom")?;
        std::fs::write(&source_path, b"new content")?;
        let original_source = std::fs::read(&source_path)?;
        let destination = root.join("output");
        std::fs::create_dir_all(&destination)?;
        let artifact = destination.join("safe.zip");
        write_source_zip(&artifact, &[("old.rom", b"old content")])?;
        let plan = BuildPlan {
            groups: vec![OutputGroup {
                path: LogicalPath::new("safe"),
                entries: vec![logical_entry("game.rom", source_file(&source_path))],
            }],
            report: BuildReport::default(),
        };

        let results = write_plan_with_compression(&plan, &destination, ZipCompression::Deflate)?;

        let result = results
            .first()
            .ok_or_else(|| io::Error::other("expected one artifact result"))?;
        assert_eq!(&result.outcome, &ArtifactOutcome::Completed);
        assert_eq!(std::fs::read(&source_path)?, original_source);
        assert_eq!(std::fs::read_dir(&destination)?.count(), 1);
        let mut zip = zip::ZipArchive::new(File::open(&artifact)?)?;
        let mut entry = zip.by_name("game.rom")?;
        let mut contents = Vec::new();
        entry.read_to_end(&mut contents)?;
        assert_eq!(contents, b"new content");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn verified_zip_artifact_is_reused_without_reading_changed_sources()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let source_path = source_fixture_path(&temp_dir, "source.rom")?;
        fs::write(&source_path, b"planned content")?;
        let plan = single_bare_file_plan(&source_path);
        let destination = root.join("output");
        let artifact = destination.join("safe.zip");
        write_plan_with_container(
            &plan,
            &destination,
            OutputContainer::Zip,
            ZipCompression::Deflate,
        )?;
        fs::write(&source_path, b"changed after planning")?;

        let results = write_plan_with_container_policy(
            &plan,
            &destination,
            OutputContainer::Zip,
            ZipCompression::Deflate,
            ArtifactReusePolicy::ReuseVerified,
        )?;

        assert_eq!(results[0].outcome, ArtifactOutcome::Reused);
        let mut archive = zip::ZipArchive::new(File::open(artifact)?)?;
        let mut member = archive.by_name("game.rom")?;
        let mut content = Vec::new();
        member.read_to_end(&mut content)?;
        assert_eq!(content, b"planned content");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn verified_seven_zip_artifact_is_reused_without_reading_changed_sources()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let source_path = source_fixture_path(&temp_dir, "source.rom")?;
        fs::write(&source_path, b"planned content")?;
        let plan = single_bare_file_plan(&source_path);
        let destination = root.join("output");
        let artifact = destination.join("safe.7z");
        write_plan_with_container(
            &plan,
            &destination,
            OutputContainer::SevenZip,
            ZipCompression::Deflate,
        )?;
        fs::write(&source_path, b"changed after planning")?;

        let results = write_plan_with_container_policy(
            &plan,
            &destination,
            OutputContainer::SevenZip,
            ZipCompression::Deflate,
            ArtifactReusePolicy::ReuseVerified,
        )?;

        assert_eq!(results[0].outcome, ArtifactOutcome::Reused);
        let archive = r7z::Archive::open(artifact.as_std_path())?;
        let entries = archive.entries().collect::<Vec<_>>();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "game.rom");
        let mut content = Vec::new();
        archive.stream_selected_files(&[entries[0].index], |_, reader| {
            reader.read_to_end(&mut content)?;
            Ok(())
        })?;
        assert_eq!(content, b"planned content");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn mismatched_existing_seven_zip_is_replaced() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let source_path = source_fixture_path(&temp_dir, "source.rom")?;
        fs::write(&source_path, b"planned content")?;
        let plan = single_bare_file_plan(&source_path);
        let destination = root.join("output");
        fs::create_dir_all(&destination)?;
        let artifact = destination.join("safe.7z");
        let existing = r7z::ArchiveBuilder::new()
            .add_file("game.rom", b"stale content")
            .build()?;
        fs::write(&artifact, existing)?;

        let results = write_plan_with_container_policy(
            &plan,
            &destination,
            OutputContainer::SevenZip,
            ZipCompression::Deflate,
            ArtifactReusePolicy::ReuseVerified,
        )?;

        assert_eq!(results[0].outcome, ArtifactOutcome::Completed);
        let archive = r7z::Archive::open(artifact.as_std_path())?;
        let entries = archive.entries().collect::<Vec<_>>();
        assert_eq!(entries.len(), 1);
        let mut content = Vec::new();
        archive.stream_selected_files(&[entries[0].index], |_, reader| {
            reader.read_to_end(&mut content)?;
            Ok(())
        })?;
        assert_eq!(content, b"planned content");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn symlink_only_seven_zip_is_not_reused() -> Result<(), Box<dyn std::error::Error>> {
        let existing = r7z::ArchiveBuilder::new()
            .add_symlink("game.rom", "planned content", r7z::EntryMeta::default())
            .build()?;

        assert_unplanned_seven_zip_is_replaced(existing)
    }

    #[cfg(unix)]
    #[test]
    fn seven_zip_with_extra_anti_item_is_not_reused() -> Result<(), Box<dyn std::error::Error>> {
        let existing = r7z::ArchiveBuilder::new()
            .add_file("game.rom", b"planned content")
            .add_anti_item("deleted.rom", r7z::EntryMeta::default())
            .build()?;

        assert_unplanned_seven_zip_is_replaced(existing)
    }

    #[cfg(unix)]
    #[test]
    fn solid_seven_zip_with_multiple_members_is_reused_in_one_stream_pass()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let first_source = source_fixture_path(&temp_dir, "first.rom")?;
        let second_source = source_fixture_path(&temp_dir, "second.rom")?;
        fs::write(&first_source, b"planned first content")?;
        fs::write(&second_source, b"planned second content")?;
        let mut plan = single_bare_file_plan(&first_source);
        plan.groups[0]
            .entries
            .push(logical_entry("other.rom", source_file(&second_source)));
        plan.report.matched_roms = 2;
        let destination = root.join("output");
        write_plan_with_container(
            &plan,
            &destination,
            OutputContainer::SevenZip,
            ZipCompression::Deflate,
        )?;
        fs::write(&first_source, b"changed first content")?;
        fs::write(&second_source, b"changed second content")?;

        let results = write_plan_with_container_policy(
            &plan,
            &destination,
            OutputContainer::SevenZip,
            ZipCompression::Deflate,
            ArtifactReusePolicy::ReuseVerified,
        )?;

        assert_eq!(results[0].outcome, ArtifactOutcome::Reused);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn invalid_existing_zip_is_replaced_instead_of_reused() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let source_path = source_fixture_path(&temp_dir, "source.rom")?;
        fs::write(&source_path, b"planned content")?;
        let plan = single_bare_file_plan(&source_path);
        let destination = root.join("output");
        fs::create_dir_all(&destination)?;
        let artifact = destination.join("safe.zip");
        fs::write(&artifact, b"not a zip")?;

        let results = write_plan_with_container_policy(
            &plan,
            &destination,
            OutputContainer::Zip,
            ZipCompression::Deflate,
            ArtifactReusePolicy::ReuseVerified,
        )?;

        assert_eq!(results[0].outcome, ArtifactOutcome::Completed);
        let mut archive = zip::ZipArchive::new(File::open(artifact)?)?;
        let mut member = archive.by_name("game.rom")?;
        let mut content = Vec::new();
        member.read_to_end(&mut content)?;
        assert_eq!(content, b"planned content");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn duplicate_raw_zip_names_are_not_reused() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let source_path = source_fixture_path(&temp_dir, "source.rom")?;
        fs::write(&source_path, b"planned content")?;
        let plan = single_bare_file_plan(&source_path);
        let destination = root.join("output");
        fs::create_dir_all(&destination)?;
        let artifact = destination.join("safe.zip");
        write_source_zip(
            &artifact,
            &[
                ("game.rom", b"wrong content"),
                ("item.rom", b"planned content"),
            ],
        )?;
        replace_zip_name(&artifact, "item.rom", "game.rom")?;

        let results = write_plan_with_container_policy(
            &plan,
            &destination,
            OutputContainer::Zip,
            ZipCompression::Deflate,
            ArtifactReusePolicy::ReuseVerified,
        )?;

        assert_eq!(results[0].outcome, ArtifactOutcome::Completed);
        let mut archive = zip::ZipArchive::new(File::open(artifact)?)?;
        let mut member = archive.by_name("game.rom")?;
        let mut content = Vec::new();
        member.read_to_end(&mut content)?;
        assert_eq!(content, b"planned content");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn zip_symlink_member_is_not_reused() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let source_path = source_fixture_path(&temp_dir, "source.rom")?;
        fs::write(&source_path, b"planned content")?;
        let plan = single_bare_file_plan(&source_path);
        let destination = root.join("output");
        let artifact = destination.join("safe.zip");
        write_plan_with_container(
            &plan,
            &destination,
            OutputContainer::Zip,
            ZipCompression::Deflate,
        )?;
        mark_zip_entry_as_symlink(&artifact, "game.rom")?;

        let results = write_plan_with_container_policy(
            &plan,
            &destination,
            OutputContainer::Zip,
            ZipCompression::Deflate,
            ArtifactReusePolicy::ReuseVerified,
        )?;

        assert_eq!(results[0].outcome, ArtifactOutcome::Completed);
        let mut archive = zip::ZipArchive::new(File::open(artifact)?)?;
        assert!(archive.by_name("game.rom")?.is_file());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn fifo_destination_does_not_block_verified_reuse() -> Result<(), Box<dyn std::error::Error>> {
        use std::{os::fd::AsFd, sync::mpsc, time::Duration};

        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let source_path = source_fixture_path(&temp_dir, "source.rom")?;
        fs::write(&source_path, b"planned content")?;
        let plan = single_bare_file_plan(&source_path);
        let destination = root.join("output");
        fs::create_dir_all(&destination)?;
        let directory = File::open(&destination)?;
        rustix::fs::mkfifoat(
            directory.as_fd(),
            "safe.zip",
            rustix::fs::Mode::from_raw_mode(0o600),
        )?;
        let (sender, receiver) = mpsc::channel();
        let worker_destination = destination.clone();
        std::thread::spawn(move || {
            let _ = sender.send(write_plan_with_container_policy(
                &plan,
                &worker_destination,
                OutputContainer::Zip,
                ZipCompression::Deflate,
                ArtifactReusePolicy::ReuseVerified,
            ));
        });

        let results = receiver.recv_timeout(Duration::from_secs(2))??;

        assert_eq!(results[0].outcome, ArtifactOutcome::Completed);
        assert!(destination.join("safe.zip").is_file());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_zip_falls_back_to_replacement() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let source_path = source_fixture_path(&temp_dir, "source.rom")?;
        fs::write(&source_path, b"planned content")?;
        let plan = single_bare_file_plan(&source_path);
        let destination = root.join("output");
        let artifact = destination.join("safe.zip");
        write_plan_with_container(
            &plan,
            &destination,
            OutputContainer::Zip,
            ZipCompression::Deflate,
        )?;
        fs::set_permissions(&artifact, fs::Permissions::from_mode(0o0))?;

        let results = write_plan_with_container_policy(
            &plan,
            &destination,
            OutputContainer::Zip,
            ZipCompression::Deflate,
            ArtifactReusePolicy::ReuseVerified,
        );
        fs::set_permissions(&artifact, fs::Permissions::from_mode(0o600))?;
        let results = results?;

        assert_eq!(results[0].outcome, ArtifactOutcome::Completed);
        let mut archive = zip::ZipArchive::new(File::open(artifact)?)?;
        assert!(archive.by_name("game.rom")?.is_file());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn verified_sha1_size_bound_uses_observed_size_not_conflicting_catalog_size()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let source_path = source_fixture_path(&temp_dir, "source.rom")?;
        fs::write(&source_path, b"abc")?;
        let mut plan = single_bare_file_plan(&source_path);
        plan.groups[0].entries[0].source.observed.size = Some(3);
        plan.groups[0].entries[0].expected.size = Some(2);
        let destination = root.join("output");
        let artifact = destination.join("safe.zip");
        write_plan_with_container(
            &plan,
            &destination,
            OutputContainer::Zip,
            ZipCompression::Deflate,
        )?;

        let results = write_plan_with_container_policy(
            &plan,
            &destination,
            OutputContainer::Zip,
            ZipCompression::Deflate,
            ArtifactReusePolicy::ReuseVerified,
        )?;

        assert_eq!(results[0].outcome, ArtifactOutcome::Reused);
        assert!(artifact.is_file());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn artifact_snapshot_detects_same_length_file_replacement()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let artifact_path = root.join("artifact.zip");
        fs::write(&artifact_path, b"original")?;
        let root_file = File::open(root)?;
        let artifact = open_existing_relative(&root_file, "artifact.zip", false)?
            .ok_or_else(|| io::Error::other("existing output artifact was not opened"))?;
        let snapshot = artifact
            .snapshot()
            .ok_or_else(|| io::Error::other("artifact metadata was not captured"))?;

        std::thread::sleep(std::time::Duration::from_millis(10));
        fs::write(&artifact_path, b"modified")?;

        assert!(
            !artifact.is_unchanged_and_still_named(&snapshot),
            "snapshot before={snapshot:?}, after={:?}",
            artifact.snapshot()
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn artifact_snapshot_detects_directory_entry_changes() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let directory_path = root.join("artifact");
        fs::create_dir(&directory_path)?;
        let root_file = File::open(root)?;
        let directory = open_existing_relative(&root_file, "artifact", true)?
            .ok_or_else(|| io::Error::other("existing output directory was not opened"))?;
        let snapshot = directory
            .snapshot()
            .ok_or_else(|| io::Error::other("directory metadata was not captured"))?;

        fs::write(directory_path.join("extra.rom"), b"extra")?;

        assert!(!directory.is_unchanged_and_still_named(&snapshot));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn artifact_snapshot_detects_a_renamed_nested_parent() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let parent_path = root.join("parent");
        fs::create_dir(&parent_path)?;
        let artifact_path = parent_path.join("artifact.zip");
        fs::write(&artifact_path, b"contents")?;
        let root_file = File::open(root)?;
        let artifact = open_existing_relative(&root_file, "parent/artifact.zip", false)?
            .ok_or_else(|| io::Error::other("existing output artifact was not opened"))?;
        let snapshot = artifact
            .snapshot()
            .ok_or_else(|| io::Error::other("artifact metadata was not captured"))?;

        fs::rename(&parent_path, root.join("renamed-parent"))?;

        assert!(!artifact.is_unchanged_and_still_named(&snapshot));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn verified_directory_artifact_is_reused_without_reading_changed_sources()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let source_path = source_fixture_path(&temp_dir, "source.rom")?;
        fs::write(&source_path, b"planned content")?;
        let plan = single_bare_file_plan(&source_path);
        let destination = root.join("output");
        let artifact = destination.join("safe");
        write_plan_with_container(
            &plan,
            &destination,
            OutputContainer::Directory,
            ZipCompression::Deflate,
        )?;
        fs::write(&source_path, b"changed after planning")?;

        let results = write_plan_with_container_policy(
            &plan,
            &destination,
            OutputContainer::Directory,
            ZipCompression::Deflate,
            ArtifactReusePolicy::ReuseVerified,
        )?;

        assert_eq!(results[0].outcome, ArtifactOutcome::Reused);
        assert_eq!(fs::read(artifact.join("game.rom"))?, b"planned content");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn directory_with_unplanned_content_is_replaced_instead_of_reused()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let source_path = source_fixture_path(&temp_dir, "source.rom")?;
        fs::write(&source_path, b"planned content")?;
        let plan = single_bare_file_plan(&source_path);
        let destination = root.join("output");
        let artifact = destination.join("safe");
        fs::create_dir_all(&artifact)?;
        fs::write(artifact.join("game.rom"), b"planned content")?;
        fs::write(artifact.join("unexpected.rom"), b"unplanned")?;

        let results = write_plan_with_container_policy(
            &plan,
            &destination,
            OutputContainer::Directory,
            ZipCompression::Deflate,
            ArtifactReusePolicy::ReuseVerified,
        )?;

        assert_eq!(results[0].outcome, ArtifactOutcome::Completed);
        assert_eq!(fs::read(artifact.join("game.rom"))?, b"planned content");
        assert!(!artifact.join("unexpected.rom").exists());
        Ok(())
    }

    fn single_bare_file_plan(source_path: &Utf8Path) -> BuildPlan {
        BuildPlan {
            groups: vec![OutputGroup {
                path: LogicalPath::new("safe"),
                entries: vec![logical_entry("game.rom", source_file(source_path))],
            }],
            report: BuildReport {
                matched_roms: 1,
                outcome: PlanOutcome::Ready,
                ..BuildReport::default()
            },
        }
    }

    #[cfg(unix)]
    fn assert_unplanned_seven_zip_is_replaced(
        existing_archive: Vec<u8>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let source_path = source_fixture_path(&temp_dir, "source.rom")?;
        fs::write(&source_path, b"planned content")?;
        let plan = single_bare_file_plan(&source_path);
        let destination = root.join("output");
        fs::create_dir_all(&destination)?;
        let artifact = destination.join("safe.7z");
        fs::write(&artifact, existing_archive)?;

        let results = write_plan_with_container_policy(
            &plan,
            &destination,
            OutputContainer::SevenZip,
            ZipCompression::Deflate,
            ArtifactReusePolicy::ReuseVerified,
        )?;

        assert_eq!(results[0].outcome, ArtifactOutcome::Completed);
        let archive = r7z::Archive::open(artifact.as_std_path())?;
        let entries = archive.entries().collect::<Vec<_>>();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "game.rom");
        assert_eq!(entries[0].entry_type, r7z::EntryType::File);
        let mut content = Vec::new();
        archive.stream_selected_files(&[entries[0].index], |_, reader| {
            reader.read_to_end(&mut content)?;
            Ok(())
        })?;
        assert_eq!(content, b"planned content");
        Ok(())
    }

    #[test]
    fn archive_replacement_during_read_is_detected_before_commit()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let archive_path = source_fixture_path(&temp_dir, "source.zip")?;
        write_source_zip(&archive_path, &[("game.rom", b"planned content")])?;
        let replacement = source_fixture_path(&temp_dir, "replacement.zip")?;
        write_source_zip(&replacement, &[("game.rom", b"changed during read")])?;
        let destination = root.join("output");
        std::fs::create_dir_all(&destination)?;
        let artifact = destination.join("safe.zip");
        write_source_zip(&artifact, &[("old.rom", b"preserve me")])?;
        let previous = std::fs::read(&artifact)?;
        let plan = single_zip_entry_plan(&archive_path, "game.rom", "game.rom", SourceKind::Zip);
        let mut replaced = false;
        let mut hook = |index, phase| {
            if index == 0 && phase == ArtifactPhase::Read && !replaced {
                fs::rename(&replacement, &archive_path)?;
                replaced = true;
            }
            Ok(())
        };

        let results = write_plan_with_hook(
            &plan,
            &destination,
            OutputContainer::Zip,
            file_options(ZipCompression::Deflate),
            ArtifactReusePolicy::Replace,
            &mut hook,
        )?;
        let result = results
            .first()
            .ok_or_else(|| io::Error::other("expected one artifact result"))?;

        assert!(replaced);
        assert!(matches!(&result.outcome, ArtifactOutcome::Failed { .. }));
        assert_eq!(std::fs::read(&artifact)?, previous);
        assert_eq!(std::fs::read_dir(&destination)?.count(), 1);
        Ok(())
    }

    #[test]
    fn streamed_verification_covers_sha1_md5_and_crc_size_selections()
    -> Result<(), Box<dyn std::error::Error>> {
        let bytes = b"selected evidence";
        let mut writer = ContentWriter::new(io::sink());
        writer.write_all(bytes)?;
        let digest = writer.finish();
        let temp_dir = tempfile::tempdir()?;
        let source_path = utf8_path(temp_dir.path())?.join("source.rom");
        std::fs::write(&source_path, bytes)?;
        let source = source_file(&source_path);
        let mut entry = logical_entry("game.rom", source);

        entry.expected.sha1 = Some(crate::hashes::sha1_bytes(bytes));
        entry.selection.strength = crate::resolution::MatchStrength::Sha1;
        verify_content(&entry, digest)?;
        entry.expected.sha1 = Some(crate::hashes::sha1_bytes(b"different content"));
        assert!(verify_content(&entry, digest).is_err());

        entry.expected.md5 = Some(crate::domain::Md5Digest(digest.md5));
        entry.selection.strength = crate::resolution::MatchStrength::Md5;
        verify_content(&entry, digest)?;
        entry.expected.md5 = Some(crate::domain::Md5Digest([0; 16]));
        assert!(verify_content(&entry, digest).is_err());

        entry.expected.crc = Some(crate::domain::Crc32Digest(digest.crc));
        entry.expected.size = Some(digest.size);
        entry.selection.strength = crate::resolution::MatchStrength::CrcAndSize;
        verify_content(&entry, digest)?;

        entry.expected.crc = Some(crate::domain::Crc32Digest([0; 4]));
        assert!(verify_content(&entry, digest).is_err());
        entry.expected.crc = Some(crate::domain::Crc32Digest(digest.crc));
        entry.expected.size = Some(digest.size + 1);
        assert!(verify_content(&entry, digest).is_err());
        Ok(())
    }

    #[test]
    fn write_plan_rejects_unsafe_logical_output_group_path()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let destination = utf8_path(temp_dir.path())?.join("output");
        let plan = BuildPlan {
            groups: vec![OutputGroup {
                path: LogicalPath::new("../escape"),
                entries: Vec::new(),
            }],
            report: BuildReport::default(),
        };

        let message = error_message(write_plan(&plan, &destination))?;

        assert!(message.contains("unsafe logical output group path"));
        assert!(!destination.exists());
        Ok(())
    }

    #[test]
    fn declared_source_root_cannot_hide_an_output_zip_source()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let destination = root.join("output");
        fs::create_dir(&destination)?;
        let source_path = destination.join("set.zip");
        fs::write(&source_path, b"preserve this source")?;
        let declared_root = root.join("declared-source");
        fs::create_dir(&declared_root)?;
        let mut source = source_file(&source_path);
        source.source_root = crate::domain::SourceRoot::new(declared_root.as_str());
        let plan = BuildPlan {
            groups: vec![OutputGroup {
                path: LogicalPath::new("set"),
                entries: vec![logical_entry("game.rom", source)],
            }],
            report: BuildReport::default(),
        };

        let message = error_message(write_plan(&plan, &destination))?;

        assert!(message.contains("outside its declared source root"));
        assert_eq!(fs::read(&source_path)?, b"preserve this source");
        Ok(())
    }

    #[test]
    fn declared_source_root_diagnostic_escapes_control_characters()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let outside = root.join("outside\nroot");
        let declared_root = root.join("declared-source");
        fs::create_dir(&outside)?;
        fs::create_dir(&declared_root)?;
        let source_path = outside.join("game.rom");
        let mut source = source_file(&source_path);
        source.source_root = crate::domain::SourceRoot::new(declared_root.as_str());
        let plan = BuildPlan {
            groups: vec![OutputGroup {
                path: LogicalPath::new("set"),
                entries: vec![logical_entry("game.rom", source)],
            }],
            report: BuildReport::default(),
        };

        let message = match checked_plan_destination(&plan, &root.join("output")) {
            Ok(_) => return Err("expected source outside its declared root to fail".into()),
            Err(error) => error.to_string(),
        };

        assert!(message.contains("\\nroot"));
        assert!(!message.contains('\n'));
        Ok(())
    }

    #[test]
    fn write_plan_rejects_unsafe_zip_entry_name() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let source_path = source_fixture_path(&temp_dir, "source.rom")?;
        std::fs::write(&source_path, b"rom")?;
        let destination = utf8_path(temp_dir.path())?.join("output");
        let plan = BuildPlan {
            groups: vec![OutputGroup {
                path: LogicalPath::new("safe"),
                entries: vec![logical_entry("../evil.rom", source_file(&source_path))],
            }],
            report: BuildReport::default(),
        };

        let message = error_message(write_plan(&plan, &destination))?;

        assert!(message.contains("unsafe logical entry path"));
        assert!(!destination.exists());
        Ok(())
    }

    #[test]
    fn write_plan_rejects_duplicate_logical_output_group_path()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let destination = utf8_path(temp_dir.path())?.join("output");
        let plan = BuildPlan {
            groups: vec![
                OutputGroup {
                    path: LogicalPath::new("safe"),
                    entries: Vec::new(),
                },
                OutputGroup {
                    path: LogicalPath::new("safe"),
                    entries: Vec::new(),
                },
            ],
            report: BuildReport::default(),
        };

        let message = error_message(write_plan(&plan, &destination))?;

        assert!(message.contains("duplicate logical output group path"));
        assert!(!destination.exists());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn output_path_symlink_cannot_redirect_or_truncate_source_files()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let source = root.join("source");
        let destination = root.join("output");
        fs::create_dir(&source)?;
        fs::create_dir(&destination)?;
        let protected = source.join("set.zip");
        fs::write(&protected, b"must remain untouched")?;
        symlink(&source, destination.join("nested"))?;

        let plan = BuildPlan {
            groups: vec![OutputGroup {
                path: LogicalPath::new("nested/set"),
                entries: Vec::new(),
            }],
            report: BuildReport::default(),
        };
        let result = write_plan_with_compression(&plan, &destination, ZipCompression::Deflate);

        assert!(matches!(
            result?.first().map(|result| &result.outcome),
            Some(ArtifactOutcome::Failed { .. })
        ));
        assert_eq!(fs::read(protected)?, b"must remain untouched");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn writable_ancestor_allows_output() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let parent = root.join("shared");
        let destination = parent.join("output");
        fs::create_dir(&parent)?;
        fs::create_dir(&destination)?;
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o777))?;
        let plan = BuildPlan {
            groups: vec![OutputGroup {
                path: LogicalPath::new("nested/set"),
                entries: Vec::new(),
            }],
            report: BuildReport::default(),
        };

        let result = write_plan(&plan, &destination)?;

        assert_eq!(
            result,
            [destination.join("nested/set.zip").canonicalize_utf8()?]
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn writable_nested_directory_allows_staged_write() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = tempfile::tempdir()?;
        let destination = utf8_path(temp_dir.path())?.join("output");
        let nested = destination.join("nested");
        fs::create_dir(&destination)?;
        fs::create_dir(&nested)?;
        fs::set_permissions(&nested, fs::Permissions::from_mode(0o777))?;
        let plan = BuildPlan {
            groups: vec![OutputGroup {
                path: LogicalPath::new("nested/set"),
                entries: Vec::new(),
            }],
            report: BuildReport::default(),
        };

        let result = write_plan(&plan, &destination)?;

        assert_eq!(result, [nested.join("set.zip").canonicalize_utf8()?]);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn destination_dotdot_is_resolved_before_creating_directories()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let source = root.join("source");
        fs::create_dir(&source)?;
        let destination = source.join("new/../../output");
        let sources = [source.as_path()];
        let checked = checked_destination(&sources, &destination)?;
        let _output = SecureOutputDirectory::open(&checked, &sources)?;

        assert!(!source.join("new").exists());
        assert!(root.join("output").is_dir());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn source_symlink_retarget_after_boundary_check_keeps_resolved_target()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let source_root = root.join("sources");
        let destination = root.join("output");
        fs::create_dir(&source_root)?;
        fs::create_dir(&destination)?;
        let original = root.join("original.rom");
        let replacement = root.join("replacement.rom");
        let link = source_root.join("game.rom");
        fs::write(&original, b"planned source")?;
        fs::write(&replacement, b"retargeted source")?;
        symlink(&original, &link)?;
        let entry = logical_entry("game.rom", source_file(&link));
        let plan = BuildPlan {
            groups: vec![OutputGroup {
                path: LogicalPath::new("set"),
                entries: vec![entry.clone()],
            }],
            report: BuildReport::default(),
        };

        let checked = checked_plan_destination(&plan, &destination)?;
        fs::remove_file(&link)?;
        symlink(&replacement, &link)?;

        let ResolvedSource::Bare { path } = resolve_source(&entry, &checked)? else {
            return Err("bare source resolved as an archive".into());
        };
        assert_eq!(path, original.canonicalize_utf8()?.as_str());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn disjoint_symlinked_destination_writes_through_its_resolved_path()
    -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let actual = root.join("actual-output");
        let alias = root.join("output-alias");
        fs::create_dir(&actual)?;
        symlink(&actual, &alias)?;
        let plan = BuildPlan {
            groups: vec![OutputGroup {
                path: LogicalPath::new("safe"),
                entries: Vec::new(),
            }],
            report: BuildReport::default(),
        };

        write_plan(&plan, &alias)?;

        assert!(actual.join("safe.zip").is_file());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn output_handle_rechecks_overlap_after_source_directory_moves()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let root = utf8_path(temp_dir.path())?;
        let source = root.join("source");
        let destination = root.join("output");
        fs::create_dir(&source)?;
        fs::create_dir(&destination)?;
        let sources = [source.as_path()];
        let destination = checked_destination(&sources, &destination)?;
        let output = SecureOutputDirectory::open(&destination, &sources)?;
        let (mut staged, file) = output.stage_file("safe.zip")?;
        drop(file);
        fs::rename(&source, destination.join("moved-source"))?;
        let mut hook = |_, _| Ok(());

        assert!(staged.replace(&output, &mut hook, 0).is_err());
        Ok(())
    }

    #[test]
    fn write_plan_rejects_case_folded_output_names() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let destination = utf8_path(temp_dir.path())?.join("output");
        let plan = BuildPlan {
            groups: vec![
                OutputGroup {
                    path: LogicalPath::new("game"),
                    entries: Vec::new(),
                },
                OutputGroup {
                    path: LogicalPath::new("GAME"),
                    entries: Vec::new(),
                },
            ],
            report: BuildReport::default(),
        };

        let message = error_message(write_plan(&plan, &destination))?;

        assert!(message.contains("duplicate logical output group path"));
        assert!(!destination.exists());
        Ok(())
    }

    #[test]
    fn write_plan_rejects_non_ascii_output_names() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let destination = utf8_path(temp_dir.path())?.join("output");
        let plan = BuildPlan {
            groups: vec![OutputGroup {
                path: LogicalPath::new("café"),
                entries: Vec::new(),
            }],
            report: BuildReport::default(),
        };

        let message = error_message(write_plan(&plan, &destination))?;

        assert!(message.contains("unsafe logical output group path"));
        assert!(!destination.exists());
        Ok(())
    }

    #[test]
    fn write_plan_rejects_duplicate_logical_entry_path() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let source_path = source_fixture_path(&temp_dir, "source.rom")?;
        std::fs::write(&source_path, b"rom")?;
        let destination = utf8_path(temp_dir.path())?.join("output");
        let plan = BuildPlan {
            groups: vec![OutputGroup {
                path: LogicalPath::new("safe"),
                entries: vec![
                    logical_entry("same.rom", source_file(&source_path)),
                    logical_entry("same.rom", source_file(&source_path)),
                ],
            }],
            report: BuildReport::default(),
        };

        let message = error_message(write_plan(&plan, &destination))?;

        assert!(message.contains("duplicate logical entry path"));
        assert!(!destination.exists());
        Ok(())
    }

    #[test]
    fn write_plan_allows_nested_zip_entry_name() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let source_path = source_fixture_path(&temp_dir, "source.rom")?;
        std::fs::write(&source_path, b"rom")?;
        let destination = utf8_path(temp_dir.path())?.join("output");
        let plan = BuildPlan {
            groups: vec![OutputGroup {
                path: LogicalPath::new("safe"),
                entries: vec![logical_entry("nested/game.rom", source_file(&source_path))],
            }],
            report: BuildReport::default(),
        };

        let mut written_paths = write_plan(&plan, &destination)?;
        assert_eq!(written_paths.len(), 1);
        let zip_path = written_paths.pop().ok_or("expected written zip")?;
        let mut zip = zip::ZipArchive::new(File::open(zip_path)?)?;
        let mut entry = zip.by_name("nested/game.rom")?;
        let mut contents = Vec::new();
        entry.read_to_end(&mut contents)?;

        assert_eq!(contents, b"rom");
        Ok(())
    }

    #[test]
    fn nested_logical_groups_write_under_nested_destination_directories()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let source_path = source_fixture_path(&temp_dir, "source.rom")?;
        std::fs::write(&source_path, b"rom")?;
        let destination = utf8_path(temp_dir.path())?.join("output");
        let plan = BuildPlan {
            groups: vec![OutputGroup {
                path: LogicalPath::new("nested/set"),
                entries: vec![logical_entry("game.rom", source_file(&source_path))],
            }],
            report: BuildReport::default(),
        };

        let written = write_plan(&plan, &destination)?;

        let expected_path = destination.join("nested/set.zip").canonicalize_utf8()?;
        assert_eq!(written, vec![expected_path.clone()]);
        assert!(expected_path.is_file());
        Ok(())
    }

    #[test]
    fn logical_group_file_directory_conflict_is_rejected() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp_dir = tempfile::tempdir()?;
        let destination = utf8_path(temp_dir.path())?.join("output");
        let plan = BuildPlan {
            groups: vec![
                OutputGroup {
                    path: LogicalPath::new("a"),
                    entries: Vec::new(),
                },
                OutputGroup {
                    path: LogicalPath::new("a.zip/child"),
                    entries: Vec::new(),
                },
            ],
            report: BuildReport::default(),
        };

        let message = error_message(write_plan(&plan, &destination))?;
        assert!(
            message.contains("group file/directory path conflict"),
            "{message}"
        );
        assert!(!destination.exists());
        Ok(())
    }

    #[test]
    fn zip_source_entry_writes_expected_content() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let archive_path = source_fixture_path(&temp_dir, "source.archive-data")?;
        write_source_zip(
            &archive_path,
            &[
                ("first.rom", b"first"),
                ("nested/target.rom", b"target"),
                ("last.rom", b"last"),
            ],
        )?;
        let original_archive = std::fs::read(&archive_path)?;
        let destination = utf8_path(temp_dir.path())?.join("output");
        let plan = single_zip_entry_plan(
            &archive_path,
            "nested/target.rom",
            "copied.rom",
            SourceKind::Zip,
        );

        let written_paths = write_plan(&plan, &destination)?;
        let zip_path = written_paths
            .first()
            .ok_or_else(|| io::Error::other("expected written zip"))?;
        let mut zip = zip::ZipArchive::new(File::open(zip_path)?)?;
        assert_eq!(zip.len(), 1);
        let mut entry = zip.by_name("copied.rom")?;
        let mut contents = Vec::new();
        entry.read_to_end(&mut contents)?;

        assert_eq!(contents, b"target");
        assert_eq!(std::fs::read(&archive_path)?, original_archive);
        Ok(())
    }

    #[test]
    fn archive_batch_preserves_logical_output_order_and_magic_detects_renamed_zip()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let archive_path = source_fixture_path(&temp_dir, "source.payload")?;
        write_source_zip(
            &archive_path,
            &[("first.rom", b"first"), ("second.rom", b"second")],
        )?;
        let destination = utf8_path(temp_dir.path())?.join("output");
        let plan = BuildPlan {
            groups: vec![OutputGroup {
                path: LogicalPath::new("ordered"),
                entries: vec![
                    logical_entry(
                        "logical-first.rom",
                        archive_source_file(&archive_path, Some("second.rom"), SourceKind::Zip),
                    ),
                    logical_entry(
                        "logical-second.rom",
                        archive_source_file(&archive_path, Some("first.rom"), SourceKind::Zip),
                    ),
                ],
            }],
            report: BuildReport::default(),
        };

        let written = write_plan(&plan, &destination)?;
        let output_path = written.first().ok_or("expected output ZIP")?;
        let mut output = zip::ZipArchive::new(File::open(output_path)?)?;
        let mut first = Vec::new();
        output.by_index(0)?.read_to_end(&mut first)?;
        let mut second = Vec::new();
        output.by_index(1)?.read_to_end(&mut second)?;
        assert_eq!(first, b"second");
        assert_eq!(second, b"first");
        Ok(())
    }

    #[test]
    fn zip_source_entry_uses_enclosed_name_fallback() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let archive_path = source_fixture_path(&temp_dir, "source.zip")?;
        write_source_zip(&archive_path, &[("./nested/target.rom", b"target")])?;
        let destination = utf8_path(temp_dir.path())?.join("output");
        let plan = single_zip_entry_plan(
            &archive_path,
            "nested/target.rom",
            "copied.rom",
            SourceKind::Zip,
        );

        let written_paths = write_plan(&plan, &destination)?;
        let zip_path = written_paths
            .first()
            .ok_or_else(|| io::Error::other("expected written zip"))?;
        let mut zip = zip::ZipArchive::new(File::open(zip_path)?)?;
        let mut entry = zip.by_name("copied.rom")?;
        let mut contents = Vec::new();
        entry.read_to_end(&mut contents)?;

        assert_eq!(contents, b"target");
        Ok(())
    }

    #[test]
    fn archive_source_without_entry_name_errors_clearly() -> Result<(), Box<dyn std::error::Error>>
    {
        let temp_dir = tempfile::tempdir()?;
        let archive_path = source_fixture_path(&temp_dir, "source.zip")?;
        std::fs::write(&archive_path, b"not inspected before entry-name validation")?;
        let destination = utf8_path(temp_dir.path())?.join("output");
        let plan = BuildPlan {
            groups: vec![OutputGroup {
                path: LogicalPath::new("safe"),
                entries: vec![logical_entry(
                    "game.rom",
                    archive_source_file(&archive_path, None, SourceKind::Archive),
                )],
            }],
            report: BuildReport::default(),
        };

        let message = error_message(write_plan(&plan, &destination))?;

        assert!(message.contains("archive source has no entry name"));
        Ok(())
    }

    #[test]
    fn write_plan_with_store_compression_writes_stored_entries()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let source_path = source_fixture_path(&temp_dir, "source.rom")?;
        std::fs::write(&source_path, b"rom")?;
        let destination = utf8_path(temp_dir.path())?.join("output");
        let plan = BuildPlan {
            groups: vec![OutputGroup {
                path: LogicalPath::new("safe"),
                entries: vec![logical_entry("game.rom", source_file(&source_path))],
            }],
            report: BuildReport::default(),
        };

        let results = write_plan_with_compression(&plan, &destination, ZipCompression::Store)?;
        assert_eq!(results.len(), 1);
        let result = results
            .first()
            .ok_or_else(|| io::Error::other("expected one artifact result"))?;
        assert_eq!(&result.outcome, &ArtifactOutcome::Completed);
        let mut zip = zip::ZipArchive::new(File::open(destination.join("safe.zip"))?)?;
        let entry = zip.by_name("game.rom")?;

        assert_eq!(entry.compression(), zip::CompressionMethod::Stored);
        Ok(())
    }

    #[test]
    fn missing_zip_entry_errors_clearly() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let archive_path = source_fixture_path(&temp_dir, "source.zip")?;
        write_source_zip(&archive_path, &[("present.rom", b"rom")])?;
        let destination = utf8_path(temp_dir.path())?.join("output");
        let plan = single_zip_entry_plan(&archive_path, "missing.rom", "game.rom", SourceKind::Zip);

        let message = error_message(write_plan(&plan, &destination))?;

        assert!(message.contains("archive entry not found"));
        assert!(message.contains("missing.rom"));
        Ok(())
    }

    #[test]
    fn corrupt_zip_source_entry_errors() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let archive_path = source_fixture_path(&temp_dir, "source.zip")?;
        std::fs::write(&archive_path, b"not a zip")?;
        let destination = utf8_path(temp_dir.path())?.join("output");
        let plan = single_zip_entry_plan(&archive_path, "game.rom", "game.rom", SourceKind::Zip);

        let message = error_message(write_plan(&plan, &destination))?;

        assert!(!message.is_empty());
        Ok(())
    }

    #[test]
    fn archive_source_entries_write_7z_content() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let archive_path = source_fixture_path(&temp_dir, "source.7z")?;
        let archive_data = r7z::ArchiveBuilder::new()
            .add_file("nested/game.rom", b"rom")
            .add_file("nested/second.rom", b"second")
            .build()?;
        std::fs::write(&archive_path, archive_data)?;
        let destination = utf8_path(temp_dir.path())?.join("output");
        let plan = BuildPlan {
            groups: vec![OutputGroup {
                path: LogicalPath::new("safe"),
                entries: vec![
                    logical_entry(
                        "game.rom",
                        archive_source_file(
                            &archive_path,
                            Some("nested/game.rom"),
                            SourceKind::Archive,
                        ),
                    ),
                    logical_entry(
                        "second.rom",
                        archive_source_file(
                            &archive_path,
                            Some("nested/second.rom"),
                            SourceKind::Archive,
                        ),
                    ),
                ],
            }],
            report: BuildReport::default(),
        };

        let written_paths = write_plan(&plan, &destination)?;
        let zip_path = written_paths
            .first()
            .ok_or_else(|| io::Error::other("expected written zip"))?;
        let mut zip = zip::ZipArchive::new(File::open(zip_path)?)?;
        let mut entry = zip.by_name("game.rom")?;
        let mut contents = Vec::new();
        entry.read_to_end(&mut contents)?;
        assert_eq!(contents, b"rom");
        drop(entry);
        let mut second = zip.by_name("second.rom")?;
        contents.clear();
        second.read_to_end(&mut contents)?;
        assert_eq!(contents, b"second");
        Ok(())
    }

    #[test]
    fn archive_source_entry_writes_rar_content() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let archive_path = source_fixture_path(&temp_dir, "source.rar")?;
        write_version_rar(&archive_path)?;
        let destination = utf8_path(temp_dir.path())?.join("output");
        let plan = single_zip_entry_plan(&archive_path, "VERSION", "game.rom", SourceKind::Rar);

        let written_paths = write_plan(&plan, &destination)?;
        let zip_path = written_paths
            .first()
            .ok_or_else(|| io::Error::other("expected written zip"))?;
        let mut zip = zip::ZipArchive::new(File::open(zip_path)?)?;
        let mut entry = zip.by_name("game.rom")?;
        let mut contents = Vec::new();
        entry.read_to_end(&mut contents)?;

        assert_eq!(contents, b"unrar-0.4.0");
        Ok(())
    }

    #[test]
    fn missing_rar_archive_entry_errors_clearly() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let archive_path = source_fixture_path(&temp_dir, "source.rar")?;
        write_version_rar(&archive_path)?;
        let destination = utf8_path(temp_dir.path())?.join("output");
        let plan = single_zip_entry_plan(&archive_path, "missing.rom", "game.rom", SourceKind::Rar);

        let message = error_message(write_plan(&plan, &destination))?;

        assert!(message.contains("archive entry not found"));
        assert!(message.contains("missing.rom"));
        Ok(())
    }

    #[test]
    fn corrupt_rar_archive_entry_errors() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let archive_path = source_fixture_path(&temp_dir, "source.rar")?;
        std::fs::write(&archive_path, b"Rar!\x1A\x07\x00not a valid rar")?;
        let destination = utf8_path(temp_dir.path())?.join("output");
        let plan = single_zip_entry_plan(&archive_path, "game.rom", "game.rom", SourceKind::Rar);

        let message = error_message(write_plan(&plan, &destination))?;

        assert!(!message.is_empty());
        Ok(())
    }

    #[test]
    fn shared_member_name_validation_rejects_unsafe_components()
    -> Result<(), Box<dyn std::error::Error>> {
        for entry_name in [
            "",
            ".",
            "..",
            "../game.rom",
            "nested\\game.rom",
            "nul\0.rom",
        ] {
            let Err(error) = crate::sources::normalize_member_name(entry_name) else {
                return Err(
                    format!("expected unsafe archive entry to fail: {entry_name:?}").into(),
                );
            };
            assert!(error.to_string().contains("unsafe archive member name"));
        }
        Ok(())
    }

    #[test]
    fn archive_spool_writer_enforces_actual_bytes_not_declared_size()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let spool_path = directory.path().join("bounded.member");
        let file = File::create(&spool_path)?;
        let mut writer = ArchiveSpoolWriter::new(file, 3);
        let mut input = std::io::Cursor::new(b"four bytes".as_slice());
        let Err(error) = std::io::copy(&mut input, &mut writer) else {
            return Err("staging byte limit was not enforced".into());
        };
        assert!(error.to_string().contains("staging limit exceeded"));
        drop(writer);
        assert_eq!(std::fs::read(spool_path)?, b"fou");
        Ok(())
    }

    #[test]
    fn missing_archive_entry_errors_clearly() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let archive_path = source_fixture_path(&temp_dir, "source.7z")?;
        let archive_data = r7z::ArchiveBuilder::new()
            .add_file("present.rom", b"rom")
            .build()?;
        std::fs::write(&archive_path, archive_data)?;
        let destination = utf8_path(temp_dir.path())?.join("output");
        let plan = single_zip_entry_plan(
            &archive_path,
            "missing.rom",
            "game.rom",
            SourceKind::Archive,
        );

        let message = error_message(write_plan(&plan, &destination))?;

        assert!(message.contains("archive entry not found"));
        assert!(message.contains("missing.rom"));
        Ok(())
    }
}

#[cfg(all(test, not(any(target_os = "linux", target_os = "macos"))))]
mod unsupported_output_tests {
    use super::*;

    #[test]
    fn unsupported_output_does_not_create_or_truncate_targets()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let plan = BuildPlan {
            groups: vec![OutputGroup {
                path: LogicalPath::new("nested/set"),
                entries: Vec::new(),
            }],
            report: BuildReport::default(),
        };
        let absent_destination = Utf8Path::from_path(temp_dir.path())
            .ok_or_else(|| io::Error::other("temporary path is not UTF-8"))?
            .join("not-created");
        assert!(
            write_plan_with_compression(&plan, &absent_destination, ZipCompression::Deflate)
                .is_err()
        );
        assert!(!absent_destination.exists());

        let destination = absent_destination.with_file_name("output");
        let nested = destination.join("nested");
        fs::create_dir_all(&nested)?;
        let protected = nested.join("set.zip");
        fs::write(&protected, b"must remain untouched")?;

        assert!(write_plan_with_compression(&plan, &destination, ZipCompression::Deflate).is_err());
        assert_eq!(fs::read(protected)?, b"must remain untouched");
        assert!(destination.is_dir());
        Ok(())
    }
}
