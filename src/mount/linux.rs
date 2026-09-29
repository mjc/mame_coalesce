//! Linux FUSE adapter for the validated MAME 0.289 projection.

use std::{
    collections::HashMap,
    ffi::OsStr,
    fs::File,
    io::Read,
    os::unix::ffi::OsStrExt,
    path::Path,
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
    sync::{Arc, Mutex},
    time::SystemTime,
};

use fuser::{
    Config, Errno, FileAttr, FileHandle, FileType, Filesystem, FopenFlags, Generation, INodeNo,
    KernelConfig, LockOwner, MountOption, OpenAccMode, OpenFlags, ReplyAttr, ReplyData,
    ReplyDirectory, ReplyEmpty, ReplyEntry, ReplyOpen, Request, Session,
};
use signal_hook::{
    consts::signal::{SIGINT, SIGTERM},
    iterator::Signals,
};

use super::{
    MOUNT_MAX_ARCHIVE_DECODERS, MOUNT_MAX_OPEN_FILES, MOUNT_SPOOL_QUOTA_BYTES, MountNode,
    MountNodeId, MountProjection,
};
use crate::{
    build::view_manifest::{MAX_VIEW_MANIFEST_BYTES, ViewManifest},
    serving::{
        ByteLength, ByteOffset, ByteRange, MaterializationBudget, ServingFailureKind,
        VerifiedContentSession,
    },
};

const MAX_READ_BYTES: u32 = 1024 * 1024;
const TTL: std::time::Duration = std::time::Duration::from_secs(1);

/// Validate the pinned manifest and its sources, then mount it read-only.
///
/// This blocks until the mount is unmounted. The spool is deliberately separate
/// from both the source tree and the mounted namespace.
pub fn run_mount(
    manifest_path: &Path,
    mount_point: &Path,
    spool_root: &Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut manifest_file = File::open(manifest_path)?;
    let mut bytes = Vec::new();
    manifest_file
        .by_ref()
        .take(u64::try_from(MAX_VIEW_MANIFEST_BYTES)? + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_VIEW_MANIFEST_BYTES {
        return Err(format!("manifest exceeds the {MAX_VIEW_MANIFEST_BYTES}-byte limit").into());
    }
    let manifest = ViewManifest::from_json(&bytes)?;
    let projection = MountProjection::compile(manifest)?;
    let canonical_spool = projection.validate_spool_root(spool_root, mount_point)?;
    let budget = MaterializationBudget::new_in(
        ByteLength::new(MOUNT_SPOOL_QUOTA_BYTES),
        ByteLength::new(256 * 1024 * 1024),
        canonical_spool,
    );
    projection.preflight_sources(&budget)?;

    let filesystem = PinnedViewFilesystem::new(projection, budget);
    let mut config = Config::default();
    config.n_threads = Some(4);
    config.mount_options = vec![
        MountOption::RO,
        MountOption::DefaultPermissions,
        MountOption::FSName("mame-coalesce".to_owned()),
    ];
    let mut signals = Signals::new([SIGINT, SIGTERM])?;
    let signal_handle = signals.handle();
    let mut session = Session::new(filesystem, mount_point, &config)?;
    let mut unmount = session.unmount_callable();
    let signal_thread = std::thread::spawn(move || {
        if signals.forever().next().is_some()
            && let Err(error) = unmount.unmount()
        {
            log::warn!("failed to unmount after shutdown signal: {error}");
        }
    });
    let result = session.run();
    signal_handle.close();
    if signal_thread.join().is_err() {
        return Err("FUSE shutdown signal thread panicked".into());
    }
    result?;
    Ok(())
}

struct OpenFile {
    node: MountNodeId,
    session: Mutex<VerifiedContentSession>,
    _permit: SlotPermit,
}

#[derive(Clone)]
struct SlotLimit {
    active: Arc<AtomicUsize>,
    maximum: usize,
}

impl SlotLimit {
    fn new(maximum: usize) -> Self {
        Self {
            active: Arc::new(AtomicUsize::new(0)),
            maximum,
        }
    }

    fn acquire(&self) -> Option<SlotPermit> {
        self.active
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |active| {
                (active < self.maximum).then_some(active + 1)
            })
            .ok()
            .map(|_| SlotPermit(Arc::clone(&self.active)))
    }
}

struct SlotPermit(Arc<AtomicUsize>);

impl Drop for SlotPermit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

struct PinnedViewFilesystem {
    projection: MountProjection,
    budget: MaterializationBudget,
    handles: Mutex<HashMap<FileHandle, Arc<OpenFile>>>,
    open_slots: SlotLimit,
    decoder_slots: SlotLimit,
    next_handle: AtomicU64,
    max_read_bytes: AtomicUsize,
}

impl PinnedViewFilesystem {
    fn new(projection: MountProjection, budget: MaterializationBudget) -> Self {
        Self {
            projection,
            budget,
            handles: Mutex::new(HashMap::new()),
            open_slots: SlotLimit::new(MOUNT_MAX_OPEN_FILES),
            decoder_slots: SlotLimit::new(MOUNT_MAX_ARCHIVE_DECODERS),
            next_handle: AtomicU64::new(1),
            max_read_bytes: AtomicUsize::new(MAX_READ_BYTES as usize),
        }
    }

    fn reserve_handle(&self) -> Result<SlotPermit, Errno> {
        self.open_slots.acquire().ok_or(Errno::EMFILE)
    }

    fn reserve_decoder(&self) -> Result<SlotPermit, Errno> {
        self.decoder_slots.acquire().ok_or(Errno::EAGAIN)
    }

    fn attr(&self, node: &MountNode) -> FileAttr {
        let directory = node.is_directory();
        let size = node.size().unwrap_or(0);
        let nlink = if directory {
            u32::try_from(
                2 + self
                    .projection
                    .children(node.id())
                    .filter(|child| child.is_directory())
                    .count(),
            )
            .unwrap_or(u32::MAX)
        } else {
            1
        };
        FileAttr {
            ino: INodeNo(node.id().get()),
            size,
            blocks: size.div_ceil(512),
            atime: SystemTime::UNIX_EPOCH,
            mtime: SystemTime::UNIX_EPOCH,
            ctime: SystemTime::UNIX_EPOCH,
            crtime: SystemTime::UNIX_EPOCH,
            kind: if directory {
                FileType::Directory
            } else {
                FileType::RegularFile
            },
            perm: if directory { 0o555 } else { 0o444 },
            nlink,
            uid: 0,
            gid: 0,
            rdev: 0,
            blksize: 4096,
            flags: 0,
        }
    }

    fn serving_errno(error: &crate::serving::ServingError) -> Errno {
        match error.failure_kind() {
            ServingFailureKind::NotFound => Errno::ENOENT,
            ServingFailureKind::Stale => Errno::ESTALE,
            ServingFailureKind::Capacity => Errno::ENOSPC,
            ServingFailureKind::Unsupported => Errno::ENOTSUP,
            ServingFailureKind::Corrupt | ServingFailureKind::Io => Errno::EIO,
        }
    }

    fn next_handle(&self) -> Result<FileHandle, Errno> {
        self.next_handle
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |handle| {
                handle.checked_add(1)
            })
            .map(FileHandle)
            .map_err(|_| Errno::EMFILE)
    }
}

impl Filesystem for PinnedViewFilesystem {
    fn init(&mut self, _req: &Request, config: &mut KernelConfig) -> std::io::Result<()> {
        let max_read_bytes = match config.set_max_readahead(MAX_READ_BYTES) {
            Ok(_) => MAX_READ_BYTES,
            Err(maximum) => maximum,
        };
        self.max_read_bytes
            .store(max_read_bytes as usize, Ordering::Release);
        Ok(())
    }

    fn lookup(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
        let Some(parent) = self.projection.node(MountNodeId::from_raw(parent.0)) else {
            reply.error(Errno::ENOENT);
            return;
        };
        if !parent.is_directory() {
            reply.error(Errno::ENOTDIR);
            return;
        }
        let Ok(name) = std::str::from_utf8(name.as_bytes()) else {
            reply.error(Errno::ENOENT);
            return;
        };
        match self.projection.lookup(parent.id(), name) {
            Some(node) => reply.entry(&TTL, &self.attr(node), Generation(0)),
            None => reply.error(Errno::ENOENT),
        }
    }

    fn getattr(&self, _req: &Request, ino: INodeNo, _fh: Option<FileHandle>, reply: ReplyAttr) {
        match self.projection.node(MountNodeId::from_raw(ino.0)) {
            Some(node) => reply.attr(&TTL, &self.attr(node)),
            None => reply.error(Errno::ENOENT),
        }
    }

    fn open(&self, _req: &Request, ino: INodeNo, flags: OpenFlags, reply: ReplyOpen) {
        if flags.acc_mode() != OpenAccMode::O_RDONLY {
            reply.error(Errno::EROFS);
            return;
        }
        let id = MountNodeId::from_raw(ino.0);
        let Some(node) = self.projection.node(id) else {
            reply.error(Errno::ENOENT);
            return;
        };
        if node.is_directory() {
            reply.error(Errno::EISDIR);
            return;
        }
        let Some(entry) = self.projection.entry(id) else {
            reply.error(Errno::EIO);
            return;
        };
        let Ok(handle_permit) = self.reserve_handle() else {
            reply.error(Errno::EMFILE);
            return;
        };
        let Some(identity) = self.projection.serving_identity(id) else {
            reply.error(Errno::EIO);
            return;
        };
        let archive = matches!(
            &entry.source.location,
            crate::domain::SourceLocation::ArchiveMember { .. }
        );
        let decoder_permit = if archive {
            match self.reserve_decoder() {
                Ok(permit) => Some(permit),
                Err(error) => {
                    reply.error(error);
                    return;
                }
            }
        } else {
            None
        };
        let session = match VerifiedContentSession::open(entry, identity, &self.budget) {
            Ok(session) => session,
            Err(error) => {
                reply.error(Self::serving_errno(&error));
                return;
            }
        };
        drop(decoder_permit);
        let handle = match self.next_handle() {
            Ok(handle) => handle,
            Err(error) => {
                reply.error(error);
                return;
            }
        };
        let file = Arc::new(OpenFile {
            node: id,
            session: Mutex::new(session),
            _permit: handle_permit,
        });
        let Ok(mut handles) = self.handles.lock() else {
            reply.error(Errno::EIO);
            return;
        };
        handles.insert(handle, file);
        reply.opened(handle, FopenFlags::empty());
    }

    fn read(
        &self,
        _req: &Request,
        ino: INodeNo,
        fh: FileHandle,
        offset: u64,
        size: u32,
        _flags: OpenFlags,
        _lock_owner: Option<LockOwner>,
        reply: ReplyData,
    ) {
        if size as usize > self.max_read_bytes.load(Ordering::Acquire) {
            reply.error(Errno::EINVAL);
            return;
        }
        let Ok(handles) = self.handles.lock() else {
            reply.error(Errno::EIO);
            return;
        };
        let Some(file) = handles
            .get(&fh)
            .filter(|file| file.node.get() == ino.0)
            .cloned()
        else {
            reply.error(Errno::EBADF);
            return;
        };
        drop(handles);

        let Ok(range) = ByteRange::new(ByteOffset::new(offset), ByteLength::new(u64::from(size)))
        else {
            reply.error(Errno::EINVAL);
            return;
        };
        let Ok(mut session) = file.session.lock() else {
            reply.error(Errno::EIO);
            return;
        };
        let Ok(size) = usize::try_from(size) else {
            reply.error(Errno::EINVAL);
            return;
        };
        let mut data = vec![0; size];
        match session.read_range(range, &mut data) {
            Ok(response) => {
                let Ok(returned) = usize::try_from(response.returned.get()) else {
                    reply.error(Errno::EIO);
                    return;
                };
                data.truncate(returned);
                reply.data(&data);
            }
            Err(error) => reply.error(Self::serving_errno(&error)),
        }
    }

    fn release(
        &self,
        _req: &Request,
        ino: INodeNo,
        fh: FileHandle,
        _flags: OpenFlags,
        _lock_owner: Option<LockOwner>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        let Ok(mut handles) = self.handles.lock() else {
            reply.error(Errno::EIO);
            return;
        };
        match handles.get(&fh) {
            Some(file) if file.node.get() == ino.0 => {
                handles.remove(&fh);
                reply.ok();
            }
            _ => reply.error(Errno::EBADF),
        }
    }

    fn opendir(&self, _req: &Request, ino: INodeNo, flags: OpenFlags, reply: ReplyOpen) {
        if flags.acc_mode() != OpenAccMode::O_RDONLY {
            reply.error(Errno::EROFS);
            return;
        }
        match self.projection.node(MountNodeId::from_raw(ino.0)) {
            Some(node) if node.is_directory() => {
                reply.opened(FileHandle(ino.0), FopenFlags::empty());
            }
            Some(_) => reply.error(Errno::ENOTDIR),
            None => reply.error(Errno::ENOENT),
        }
    }

    fn readdir(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        offset: u64,
        mut reply: ReplyDirectory,
    ) {
        let Some(directory) = self.projection.node(MountNodeId::from_raw(ino.0)) else {
            reply.error(Errno::ENOENT);
            return;
        };
        if !directory.is_directory() {
            reply.error(Errno::ENOTDIR);
            return;
        }
        let mut entries = Vec::with_capacity(directory.children.len().saturating_add(2));
        entries.push((ino, FileType::Directory, OsStr::new(".")));
        let parent = self
            .projection
            .node(directory.parent())
            .map_or(ino, |node| INodeNo(node.id().get()));
        entries.push((parent, FileType::Directory, OsStr::new("..")));
        for child in self.projection.children(directory.id()) {
            entries.push((
                INodeNo(child.id().get()),
                if child.is_directory() {
                    FileType::Directory
                } else {
                    FileType::RegularFile
                },
                OsStr::new(child.name()),
            ));
        }
        let Ok(start) = usize::try_from(offset) else {
            reply.error(Errno::EINVAL);
            return;
        };
        for (index, (child_ino, kind, name)) in entries.into_iter().enumerate().skip(start) {
            let Ok(next_offset) = u64::try_from(index.saturating_add(1)) else {
                break;
            };
            if reply.add(child_ino, next_offset, kind, name) {
                break;
            }
        }
        reply.ok();
    }
}

impl MountNodeId {
    const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::{Read, Seek, SeekFrom},
    };

    use camino::Utf8Path;

    use super::*;
    use crate::{
        build::{
            mame_layout::{
                AssetRequirementIdentity, MameSetLayoutPolicy, ResolvedMachineAsset,
                ResolvedMachineSet,
            },
            view_manifest::{TargetViewRequest, ViewManifest, plan_view},
        },
        domain::{
            CatalogKey, CatalogScope, DocumentKey, EvidenceProvenance, EvidenceScope,
            ExpectedEvidence, LogicalEntry, LogicalPath, MatchingPolicy, ObservedContent,
            ParserInterpretationKey, RequirementKey, SelectionProvenance, SetKey, SetName,
            SnapshotKey, SourceFile, SourceLocation, SourcePhysicalPath, SourceRoot,
        },
        hashes::{sha1_bytes, xxhash3_bytes},
        machine_dependencies::{MachineDependencyCatalog, MachineSet},
        resolution::MatchStrength,
    };

    #[test]
    fn handle_and_archive_decoder_limits_are_enforced_and_released()
    -> Result<(), Box<dyn std::error::Error>> {
        let handle_slots = SlotLimit::new(MOUNT_MAX_OPEN_FILES);
        let mut handles = Vec::new();
        for _ in 0..MOUNT_MAX_OPEN_FILES {
            handles.push(
                handle_slots
                    .acquire()
                    .ok_or_else(|| std::io::Error::other("handle slot was not available"))?,
            );
        }
        assert!(handle_slots.acquire().is_none());
        drop(handles.pop());
        assert!(handle_slots.acquire().is_some());

        let decoder_slots = SlotLimit::new(MOUNT_MAX_ARCHIVE_DECODERS);
        let first = decoder_slots
            .acquire()
            .ok_or_else(|| std::io::Error::other("first decoder slot was not available"))?;
        let _second = decoder_slots
            .acquire()
            .ok_or_else(|| std::io::Error::other("second decoder slot was not available"))?;
        assert!(decoder_slots.acquire().is_none());
        drop(first);
        assert!(decoder_slots.acquire().is_some());
        Ok(())
    }

    #[test]
    fn directory_link_counts_include_immediate_subdirectories()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let root = Utf8Path::from_path(directory.path())
            .ok_or_else(|| std::io::Error::other("temporary path is not UTF-8"))?;
        let source_root = root.join("roms");
        fs::create_dir(&source_root)?;
        let source_path = source_root.join("game.rom");
        let bytes = b"verified mounted ROM bytes";
        fs::write(&source_path, bytes)?;
        let manifest = manifest_for_source(&source_path, &source_root, bytes)?;
        let projection = MountProjection::compile(manifest)?;
        let root_node = projection
            .node(MountNodeId::from_raw(1))
            .ok_or_else(|| std::io::Error::other("projected root is missing"))?
            .id();
        let game_node = projection
            .node_at("game")
            .ok_or_else(|| std::io::Error::other("projected game directory is missing"))?
            .id();
        let filesystem = PinnedViewFilesystem::new(
            projection,
            MaterializationBudget::new(ByteLength::new(1024)),
        );
        let root_node = filesystem
            .projection
            .node(root_node)
            .ok_or_else(|| std::io::Error::other("projected root is missing"))?;
        let game_node = filesystem
            .projection
            .node(game_node)
            .ok_or_else(|| std::io::Error::other("projected game directory is missing"))?;

        assert_eq!(filesystem.attr(root_node).nlink, 3);
        assert_eq!(filesystem.attr(game_node).nlink, 2);
        Ok(())
    }

    #[test]
    #[ignore = "requires /dev/fuse and permission to mount a FUSE filesystem"]
    fn mounted_file_reads_match_verified_materialization_and_support_offsets()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let root = Utf8Path::from_path(directory.path())
            .ok_or_else(|| std::io::Error::other("temporary path is not UTF-8"))?;
        let source_root = root.join("roms");
        let spool_root = root.join("spool");
        let mount_point = root.join("mount");
        fs::create_dir_all(&source_root)?;
        fs::create_dir(&spool_root)?;
        fs::create_dir(&mount_point)?;
        let source_path = source_root.join("game.rom");
        let expected = b"verified mounted ROM bytes";
        fs::write(&source_path, expected)?;

        let manifest = manifest_for_source(&source_path, &source_root, expected)?;
        let projection = MountProjection::compile(manifest)?;
        let spool_root = projection
            .validate_spool_root(spool_root.as_std_path(), mount_point.as_std_path())
            .map_err(|error| std::io::Error::other(format!("spool validation failed: {error}")))?;
        let budget = MaterializationBudget::new_in(
            ByteLength::new(MOUNT_SPOOL_QUOTA_BYTES),
            ByteLength::new(256 * 1024 * 1024),
            spool_root,
        );
        projection
            .preflight_sources(&budget)
            .map_err(|error| std::io::Error::other(format!("source preflight failed: {error}")))?;

        let file_node = projection
            .node_at("game/game.rom")
            .ok_or_else(|| std::io::Error::other("projected file is missing"))?;
        let entry = projection
            .entry(file_node.id())
            .ok_or_else(|| std::io::Error::other("projected entry is missing"))?;
        let identity = projection
            .serving_identity(file_node.id())
            .ok_or_else(|| std::io::Error::other("serving identity is missing"))?;
        let mut reference = VerifiedContentSession::open(entry, identity, &budget)?;
        let mut reference_bytes = vec![0; expected.len()];
        reference.read_range(
            ByteRange::new(
                ByteOffset::new(0),
                ByteLength::new(u64::try_from(expected.len())?),
            )?,
            &mut reference_bytes,
        )?;

        let mut config = Config::default();
        config.n_threads = Some(4);
        config.mount_options = vec![MountOption::RO, MountOption::DefaultPermissions];
        let background = fuser::spawn_mount(
            PinnedViewFilesystem::new(projection, budget),
            &mount_point,
            &config,
        )
        .map_err(|error| std::io::Error::other(format!("FUSE mount failed: {error}")))?;
        let mounted_path = mount_point.join("game/game.rom");
        let mounted_bytes = fs::read(&mounted_path)
            .map_err(|error| std::io::Error::other(format!("mounted path read failed: {error}")))?;
        assert_eq!(mounted_bytes, reference_bytes);
        let mut mounted_file = fs::File::open(&mounted_path)?;
        mounted_file.seek(SeekFrom::Start(9))?;
        let mut suffix = Vec::new();
        mounted_file.read_to_end(&mut suffix)?;
        assert_eq!(suffix, reference_bytes[9..]);

        fs::write(&source_path, b"badified mounted ROM bytes")?;
        mounted_file.seek(SeekFrom::Start(0))?;
        let mut still_pinned = Vec::new();
        mounted_file.read_to_end(&mut still_pinned)?;
        assert_eq!(still_pinned, reference_bytes);
        assert!(fs::read(&mounted_path).is_err());
        assert!(
            fs::OpenOptions::new()
                .write(true)
                .open(&mounted_path)
                .is_err()
        );
        drop(mounted_file);
        background.umount_and_join()?;
        Ok(())
    }

    fn manifest_for_source(
        source_path: &Utf8Path,
        source_root: &Utf8Path,
        bytes: &[u8],
    ) -> Result<ViewManifest, Box<dyn std::error::Error>> {
        let location = SourceLocation::BareFile {
            path: source_path.to_string(),
        };
        let sha1 = sha1_bytes(bytes);
        let entry = LogicalEntry {
            path: LogicalPath::new("game.rom"),
            source: SourceFile {
                source_root: SourceRoot::new(source_root.as_str()),
                physical_path: SourcePhysicalPath::from_location(&location),
                location,
                observed: ObservedContent {
                    scope: EvidenceScope::WholeAsset,
                    provenance: EvidenceProvenance::Computed,
                    size: Some(u64::try_from(bytes.len())?),
                    crc: None,
                    md5: None,
                    sha1: Some(sha1),
                    xxh3: xxhash3_bytes(bytes),
                },
                fingerprint: None,
                scan_run: None,
                scan_provenance: None,
                bare_file_cache_stamp: None,
            },
            requirement: RequirementKey::new(
                SetKey::new(CatalogKey::new("mount-test"), "game"),
                "game.rom",
            ),
            expected: ExpectedEvidence {
                scope: EvidenceScope::WholeAsset,
                provenance: EvidenceProvenance::SourceDeclared,
                size: Some(u64::try_from(bytes.len())?),
                sha1: Some(sha1),
                ..ExpectedEvidence::default()
            },
            selection: SelectionProvenance {
                policy: MatchingPolicy::Sha1Compatibility,
                strength: MatchStrength::Sha1,
                assessments: Vec::new(),
                omitted_assessments: 0,
            },
        };
        let catalog = CatalogKey::new("mount-test");
        let document = DocumentKey::from_bytes(b"mount-test-snapshot");
        let interpretation =
            ParserInterpretationKey::for_format("mame-machine-xml", &CatalogScope::Complete);
        let snapshot = SnapshotKey::new(&catalog, &document, &interpretation);
        let root = SetName::new("game");
        let mut set = ResolvedMachineSet::new(snapshot.clone(), catalog, root.clone());
        set.entries.push(ResolvedMachineAsset::new(
            AssetRequirementIdentity::new(snapshot.clone(), root.clone(), "game.rom", 0),
            entry,
        ));
        let closure =
            MachineDependencyCatalog::new(snapshot.clone(), vec![MachineSet::new(root.as_str())])
                .resolve(&root);
        let closures = [closure];
        let sets = [set];
        Ok(plan_view(TargetViewRequest::mame_0289(
            &snapshot,
            std::slice::from_ref(&root),
            MameSetLayoutPolicy::NonMerged,
            &closures,
            &sets,
        )))
    }
}
