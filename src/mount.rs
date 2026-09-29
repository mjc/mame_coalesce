//! A validated, immutable namespace projected from one pinned view manifest.

use std::{
    collections::{BTreeMap, HashMap},
    io,
    path::{Path, PathBuf},
};

use sha2::{Digest as _, Sha256};
use thiserror::Error;
use unicode_normalization::UnicodeNormalization;

use crate::{
    build::view_manifest::{TargetProfile, ViewManifest},
    domain::{ArchiveBackend, EvidenceScope, LogicalEntry, SourceLocation},
    resolution::MatchStrength,
    serving::{ByteLength, ServingIdentity},
};

const ROOT_NODE_ID: u64 = 1;
const MAX_MOUNT_FILE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_MOUNT_PATH_BYTES: usize = 4096;
const MAX_MOUNT_PATH_DEPTH: usize = 128;
const MAX_MOUNT_NAMESPACE_NODES: usize = 250_000;
const MAX_MOUNT_NAMESPACE_BYTES: usize = 32 * 1024 * 1024;
const MANIFEST_ID_DOMAIN: &[u8] = b"mame-coalesce-mount-manifest-v1\0";
pub const MOUNT_SPOOL_QUOTA_BYTES: u64 = 512 * 1024 * 1024;
pub const MOUNT_MAX_OPEN_FILES: usize = 8;
pub const MOUNT_MAX_ARCHIVE_DECODERS: usize = 2;

#[cfg(all(feature = "fuse", target_os = "linux"))]
pub mod linux;

/// Stable identity for one exact serialized manifest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ManifestId([u8; 32]);

impl ManifestId {
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    #[must_use]
    pub fn to_hex(self) -> String {
        hex::encode(self.0)
    }
}

/// Stable namespace identity for the lifetime of a compiled projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MountNodeId(u64);

impl MountNodeId {
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NodeKind {
    Directory,
    File { size: ByteLength, entry: EntryIndex },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EntryIndex {
    group: usize,
    entry: usize,
}

type ArchivePreflightGroup<'a> = Vec<(&'a MountNode, &'a LogicalEntry, u64)>;

/// One directory or read-only file in the pinned namespace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MountNode {
    id: MountNodeId,
    parent: MountNodeId,
    name: String,
    path: String,
    children: Vec<MountNodeId>,
    kind: NodeKind,
}

impl MountNode {
    #[must_use]
    pub const fn id(&self) -> MountNodeId {
        self.id
    }

    #[must_use]
    pub const fn parent(&self) -> MountNodeId {
        self.parent
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Empty only for the root node.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub const fn is_directory(&self) -> bool {
        matches!(self.kind, NodeKind::Directory)
    }

    #[must_use]
    pub const fn size(&self) -> Option<u64> {
        match self.kind {
            NodeKind::Directory => None,
            NodeKind::File { size, .. } => Some(size.get()),
        }
    }
}

/// Why a manifest cannot safely be exposed as a mount namespace.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum MountProjectionError {
    #[error("only the MAME 0.289 target profile can be mounted")]
    UnsupportedProfile,
    #[error("manifest contains unresolved layout diagnostics")]
    UnresolvedLayout,
    #[error("manifest has no mountable files")]
    EmptyView,
    #[error("unsafe or unsupported manifest path: {0}")]
    InvalidPath(String),
    #[error("manifest paths collide on a case-insensitive filesystem: {first} and {second}")]
    PathCollision { first: String, second: String },
    #[error("file and directory paths conflict: {0}")]
    FileDirectoryConflict(String),
    #[error("entry lacks consistent whole-asset size and digest evidence: {0}")]
    IncompleteEvidence(String),
    #[error("entry exceeds the {maximum}-byte mount file limit: {path} ({size} bytes)")]
    FileTooLarge {
        path: String,
        size: u64,
        maximum: u64,
    },
    #[error("manifest serialization failed: {0}")]
    ManifestEncoding(String),
    #[error("manifest namespace exceeded the supported node count")]
    NamespaceTooLarge,
    #[error("manifest namespace exceeded its {maximum}-byte path storage budget")]
    NamespaceStorageExceeded { maximum: usize },
}

#[derive(Debug, Error)]
pub enum MountPreflightError {
    #[error("RAR-backed entries are not supported by the bounded mount profile: {0}")]
    UnsupportedRar(String),
    #[error("archive backend lacks enforceable decoder memory bounds: {0}")]
    UnboundedDecoder(String),
    #[error("current source length differs from the pinned manifest for {path}")]
    SourceLengthChanged { path: String },
    #[error(transparent)]
    Serving(#[from] crate::serving::ServingError),
}

#[derive(Debug, Error)]
pub enum MountStartupError {
    #[error("mount point must be an existing directory: {0}")]
    InvalidMountPoint(PathBuf),
    #[error("mount point must be empty: {0}")]
    MountPointNotEmpty(PathBuf),
    #[error("spool root must be an existing directory: {0}")]
    InvalidSpoolRoot(PathBuf),
    #[error("spool root overlaps the mount namespace")]
    SpoolOverlapsMount,
    #[error("mount point overlaps source tree or backing source {0}")]
    MountOverlapsSource(PathBuf),
    #[error("spool root overlaps source tree {0}")]
    SpoolOverlapsSource(PathBuf),
    #[error("spool volume has {available} bytes available; {required} are required")]
    InsufficientSpoolSpace { available: u64, required: u64 },
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[derive(Clone, Copy)]
struct PendingFile {
    entry: EntryIndex,
    size: ByteLength,
}

struct NamespaceTables {
    nodes: Vec<MountNode>,
    by_id: HashMap<MountNodeId, usize>,
    by_path: BTreeMap<String, MountNodeId>,
}

#[derive(Clone, Copy)]
enum PendingKind {
    Directory,
    File(PendingFile),
}

/// Snapshot-pinned namespace. IDs are assigned from sorted paths, never hashes.
pub struct MountProjection {
    manifest: ViewManifest,
    manifest_id: ManifestId,
    nodes: Vec<MountNode>,
    by_id: HashMap<MountNodeId, usize>,
    by_path: BTreeMap<String, MountNodeId>,
}

impl MountProjection {
    pub fn compile(manifest: ViewManifest) -> Result<Self, MountProjectionError> {
        if manifest.profile() != TargetProfile::Mame0289 {
            return Err(MountProjectionError::UnsupportedProfile);
        }
        if !manifest.layout().diagnostics().is_empty() {
            return Err(MountProjectionError::UnresolvedLayout);
        }

        let encoded = manifest
            .to_json()
            .map_err(|error| MountProjectionError::ManifestEncoding(error.to_string()))?;
        let mut digest = Sha256::new();
        digest.update(MANIFEST_ID_DOMAIN);
        digest.update(encoded);
        let manifest_id = ManifestId(digest.finalize().into());

        let pending = pending_namespace(&manifest)?;
        let namespace = assign_namespace_ids(pending)?;

        Ok(Self {
            manifest,
            manifest_id,
            nodes: namespace.nodes,
            by_id: namespace.by_id,
            by_path: namespace.by_path,
        })
    }

    #[must_use]
    pub const fn manifest_id(&self) -> ManifestId {
        self.manifest_id
    }

    #[must_use]
    pub const fn root_id(&self) -> MountNodeId {
        MountNodeId(ROOT_NODE_ID)
    }

    #[must_use]
    pub fn node(&self, id: MountNodeId) -> Option<&MountNode> {
        self.by_id.get(&id).map(|index| &self.nodes[*index])
    }

    #[must_use]
    pub fn node_at(&self, path: &str) -> Option<&MountNode> {
        self.by_path.get(path).and_then(|id| self.node(*id))
    }

    #[must_use]
    pub fn lookup(&self, parent: MountNodeId, name: &str) -> Option<&MountNode> {
        let parent = self.node(parent)?;
        parent
            .children
            .iter()
            .filter_map(|id| self.node(*id))
            .find(|node| node.name == name)
    }

    pub fn children(&self, parent: MountNodeId) -> impl Iterator<Item = &MountNode> + '_ {
        self.node(parent)
            .into_iter()
            .flat_map(|node| node.children.iter())
            .filter_map(|id| self.node(*id))
    }

    #[must_use]
    pub fn entry(&self, node: MountNodeId) -> Option<&LogicalEntry> {
        let node = self.node(node)?;
        let NodeKind::File { entry, .. } = node.kind else {
            return None;
        };
        self.manifest
            .layout()
            .groups()
            .get(entry.group)?
            .entries()
            .get(entry.entry)
    }

    #[must_use]
    pub fn serving_identity(&self, node: MountNodeId) -> Option<ServingIdentity> {
        let entry = self.entry(node)?;
        let digest = entry.source.observed.sha1?;
        Some(ServingIdentity::Content(
            crate::domain::ContentIdentity::new(
                crate::domain::ContentDigestAlgorithm::Sha1,
                hex::encode(digest),
            )
            .ok()?,
        ))
    }

    pub fn files(&self) -> impl Iterator<Item = &MountNode> {
        self.nodes.iter().filter(|node| !node.is_directory())
    }

    /// Re-resolve every pinned source before mounting; archive selectors are re-enumerated by the
    /// same serving backend used later for opens.
    pub fn preflight_sources(
        &self,
        budget: &crate::serving::MaterializationBudget,
    ) -> Result<(), MountPreflightError> {
        let mut archive_groups: BTreeMap<(String, ArchiveBackend), ArchivePreflightGroup<'_>> =
            BTreeMap::new();
        for node in self.files() {
            let size = node
                .size()
                .ok_or_else(|| MountPreflightError::SourceLengthChanged {
                    path: node.path.clone(),
                })?;
            let Some(entry) = self.entry(node.id) else {
                return Err(MountPreflightError::SourceLengthChanged {
                    path: node.path.clone(),
                });
            };
            if matches!(
                &entry.source.location,
                SourceLocation::ArchiveMember {
                    backend: ArchiveBackend::Rar,
                    ..
                }
            ) {
                return Err(MountPreflightError::UnsupportedRar(node.path.clone()));
            }
            let capabilities = budget.capabilities(entry)?;
            if capabilities.maximum_per_session_bytes.get() < size {
                return Err(crate::serving::ServingError::PerSessionLimitExceeded {
                    requested: ByteLength::new(size),
                    maximum: capabilities.maximum_per_session_bytes,
                }
                .into());
            }
            if matches!(
                &entry.source.location,
                SourceLocation::ArchiveMember {
                    backend: ArchiveBackend::SevenZip,
                    ..
                }
            ) && (capabilities.maximum_decoder_dictionary_bytes.is_none()
                || capabilities.maximum_decoder_working_set_bytes.is_none())
            {
                return Err(MountPreflightError::UnboundedDecoder(node.path.clone()));
            }
            match &entry.source.location {
                SourceLocation::ArchiveMember { backend, .. } => {
                    archive_groups
                        .entry((entry.source.physical_path.as_path().to_string(), *backend))
                        .or_default()
                        .push((node, entry, size));
                }
                SourceLocation::BareFile { .. } | SourceLocation::LegacyUnknown { .. } => {
                    let current_size =
                        crate::serving::VerifiedContentSession::preflight_source(entry)?;
                    if current_size.get() != size {
                        return Err(MountPreflightError::SourceLengthChanged {
                            path: node.path.clone(),
                        });
                    }
                }
            }
        }
        for (_, group) in archive_groups {
            let entries: Vec<_> = group.iter().map(|(_, entry, _)| *entry).collect();
            let current_sizes =
                crate::serving::VerifiedContentSession::preflight_archive_sources(&entries)?;
            for ((node, _, expected_size), current_size) in group.into_iter().zip(current_sizes) {
                if current_size.get() != expected_size {
                    return Err(MountPreflightError::SourceLengthChanged {
                        path: node.path.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Canonicalize and validate the spool volume before any filesystem is mounted.
    pub fn validate_spool_root(
        &self,
        spool_root: &Path,
        mount_point: &Path,
    ) -> Result<PathBuf, MountStartupError> {
        let mount_point = std::fs::canonicalize(mount_point)?;
        if !mount_point.is_dir() {
            return Err(MountStartupError::InvalidMountPoint(mount_point));
        }
        let spool_root = std::fs::canonicalize(spool_root)?;
        if !spool_root.is_dir() {
            return Err(MountStartupError::InvalidSpoolRoot(spool_root));
        }
        if paths_overlap(&spool_root, &mount_point) {
            return Err(MountStartupError::SpoolOverlapsMount);
        }
        for node in self.files() {
            let Some(entry) = self.entry(node.id) else {
                continue;
            };
            let source_root = std::fs::canonicalize(entry.source.source_root.as_str())?;
            if paths_overlap(&mount_point, &source_root) {
                return Err(MountStartupError::MountOverlapsSource(source_root));
            }
            if paths_overlap(&spool_root, &source_root) {
                return Err(MountStartupError::SpoolOverlapsSource(source_root));
            }
            let physical_source = std::fs::canonicalize(entry.source.physical_path.as_str())?;
            if paths_overlap(&mount_point, &physical_source) {
                return Err(MountStartupError::MountOverlapsSource(physical_source));
            }
            if paths_overlap(&spool_root, &physical_source) {
                return Err(MountStartupError::SpoolOverlapsSource(physical_source));
            }
        }
        let mut mount_entries = std::fs::read_dir(&mount_point)?;
        if mount_entries.next().transpose()?.is_some() {
            return Err(MountStartupError::MountPointNotEmpty(mount_point));
        }
        let available = fs4::available_space(&spool_root)?;
        if available < MOUNT_SPOOL_QUOTA_BYTES {
            return Err(MountStartupError::InsufficientSpoolSpace {
                available,
                required: MOUNT_SPOOL_QUOTA_BYTES,
            });
        }
        Ok(spool_root)
    }
}

fn pending_namespace(
    manifest: &ViewManifest,
) -> Result<BTreeMap<String, PendingKind>, MountProjectionError> {
    let mut pending = BTreeMap::new();
    let mut normalized = BTreeMap::new();
    let mut namespace_bytes = 0usize;
    let mut file_count = 0usize;
    for (group_index, group) in manifest.layout().groups().iter().enumerate() {
        for (entry_index, entry) in group.entries().iter().enumerate() {
            let path = format!("{}/{}", group.path().as_str(), entry.path.as_str());
            validate_mount_path(&path)?;
            validate_evidence(entry, &path)?;
            let size = entry
                .source
                .observed
                .size
                .ok_or_else(|| MountProjectionError::IncompleteEvidence(path.clone()))?;
            if size > MAX_MOUNT_FILE_BYTES {
                return Err(MountProjectionError::FileTooLarge {
                    path,
                    size,
                    maximum: MAX_MOUNT_FILE_BYTES,
                });
            }
            insert_directories(&path, &mut pending, &mut normalized, &mut namespace_bytes)?;
            insert_path(
                &path,
                PendingKind::File(PendingFile {
                    entry: EntryIndex {
                        group: group_index,
                        entry: entry_index,
                    },
                    size: ByteLength::new(size),
                }),
                &mut pending,
                &mut normalized,
                &mut namespace_bytes,
            )?;
            file_count += 1;
        }
    }
    if file_count == 0 {
        return Err(MountProjectionError::EmptyView);
    }
    Ok(pending)
}

fn assign_namespace_ids(
    pending: BTreeMap<String, PendingKind>,
) -> Result<NamespaceTables, MountProjectionError> {
    let root_id = MountNodeId(ROOT_NODE_ID);
    let mut nodes = vec![MountNode {
        id: root_id,
        parent: root_id,
        name: String::new(),
        path: String::new(),
        children: Vec::new(),
        kind: NodeKind::Directory,
    }];
    let mut by_id = HashMap::from([(root_id, 0)]);
    let mut by_path = BTreeMap::new();
    let mut next_id = ROOT_NODE_ID + 1;
    for (path, pending_kind) in pending {
        let (parent_path, name) = path
            .rsplit_once('/')
            .map_or(("", path.as_str()), |(parent, name)| (parent, name));
        let parent = if parent_path.is_empty() {
            root_id
        } else {
            *by_path
                .get(parent_path)
                .ok_or_else(|| MountProjectionError::FileDirectoryConflict(path.clone()))?
        };
        let id = MountNodeId(next_id);
        next_id = next_id
            .checked_add(1)
            .ok_or(MountProjectionError::NamespaceTooLarge)?;
        let kind = match pending_kind {
            PendingKind::Directory => NodeKind::Directory,
            PendingKind::File(file) => NodeKind::File {
                size: file.size,
                entry: file.entry,
            },
        };
        let node_index = nodes.len();
        nodes.push(MountNode {
            id,
            parent,
            name: name.to_owned(),
            path: path.clone(),
            children: Vec::new(),
            kind,
        });
        let parent_index = by_id[&parent];
        nodes[parent_index].children.push(id);
        by_id.insert(id, node_index);
        by_path.insert(path, id);
    }
    Ok(NamespaceTables {
        nodes,
        by_id,
        by_path,
    })
}

fn paths_overlap(left: &Path, right: &Path) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

fn validate_evidence(entry: &LogicalEntry, path: &str) -> Result<(), MountProjectionError> {
    let observed = &entry.source.observed;
    let expected = &entry.expected;
    if observed.scope != EvidenceScope::WholeAsset
        || expected.scope != EvidenceScope::WholeAsset
        || observed.sha1.is_none()
        || observed.size.is_none()
        || expected.size.is_none()
        || observed.size != expected.size
        || expected
            .sha1
            .is_some_and(|digest| observed.sha1 != Some(digest))
        || expected
            .md5
            .is_some_and(|digest| observed.md5 != Some(digest))
        || expected
            .crc
            .is_some_and(|digest| observed.crc != Some(digest))
    {
        return Err(MountProjectionError::IncompleteEvidence(path.to_owned()));
    }

    let selected_digest_present = match entry.selection.strength {
        MatchStrength::Sha1 => expected.sha1.is_some() && observed.sha1.is_some(),
        MatchStrength::Md5 => expected.md5.is_some() && observed.md5.is_some(),
        MatchStrength::CrcAndSize => expected.crc.is_some() && observed.crc.is_some(),
    };
    if !selected_digest_present {
        return Err(MountProjectionError::IncompleteEvidence(path.to_owned()));
    }
    Ok(())
}

fn validate_mount_path(path: &str) -> Result<(), MountProjectionError> {
    if path.is_empty()
        || path.len() > MAX_MOUNT_PATH_BYTES
        || path.split('/').count() > MAX_MOUNT_PATH_DEPTH
        || path.starts_with('/')
        || path.contains('\\')
        || path.contains(':')
    {
        return Err(
            if path.len() > MAX_MOUNT_PATH_BYTES || path.split('/').count() > MAX_MOUNT_PATH_DEPTH {
                MountProjectionError::NamespaceTooLarge
            } else {
                MountProjectionError::InvalidPath(path.to_owned())
            },
        );
    }
    if path.split('/').any(|component| component.len() > 255) {
        return Err(MountProjectionError::NamespaceTooLarge);
    }
    for component in path.split('/') {
        let folded = component.to_ascii_lowercase();
        let device_stem = folded.split('.').next().unwrap_or_default();
        let reserved_device = matches!(
            device_stem,
            "con" | "prn" | "aux" | "nul" | "conin$" | "conout$"
        ) || ["com", "lpt"].into_iter().any(|prefix| {
            device_stem.strip_prefix(prefix).is_some_and(|number| {
                matches!(number, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            })
        });
        if component.is_empty()
            || matches!(component, "." | "..")
            || component.ends_with(['.', ' '])
            || component
                .chars()
                .any(|character| matches!(character, '<' | '>' | '"' | '|' | '?' | '*'))
            || component.chars().any(char::is_control)
            || reserved_device
        {
            return Err(MountProjectionError::InvalidPath(path.to_owned()));
        }
    }
    Ok(())
}

fn insert_directories(
    path: &str,
    pending: &mut BTreeMap<String, PendingKind>,
    normalized: &mut BTreeMap<String, String>,
    namespace_bytes: &mut usize,
) -> Result<(), MountProjectionError> {
    for (end, _) in path.match_indices('/') {
        let directory = &path[..end];
        insert_path(
            directory,
            PendingKind::Directory,
            pending,
            normalized,
            namespace_bytes,
        )?;
    }
    Ok(())
}

fn insert_path(
    path: &str,
    kind: PendingKind,
    pending: &mut BTreeMap<String, PendingKind>,
    normalized: &mut BTreeMap<String, String>,
    namespace_bytes: &mut usize,
) -> Result<(), MountProjectionError> {
    let normalized_path = path.nfkc().flat_map(char::to_lowercase).collect::<String>();
    let normalized_path_bytes = normalized_path.len();
    if let Some(existing) = normalized.get(&normalized_path) {
        if existing != path {
            return Err(MountProjectionError::PathCollision {
                first: existing.clone(),
                second: path.to_owned(),
            });
        }
    } else {
        normalized.insert(normalized_path, path.to_owned());
    }

    match (pending.get(path), kind) {
        (Some(PendingKind::File(_)), PendingKind::Directory)
        | (Some(PendingKind::Directory), PendingKind::File(_)) => {
            return Err(MountProjectionError::FileDirectoryConflict(path.to_owned()));
        }
        (Some(PendingKind::File(_)), PendingKind::File(_)) => {
            return Err(MountProjectionError::PathCollision {
                first: path.to_owned(),
                second: path.to_owned(),
            });
        }
        (Some(PendingKind::Directory), PendingKind::Directory) => {}
        (None, _) => {
            let next_count = pending
                .len()
                .checked_add(1)
                .ok_or(MountProjectionError::NamespaceTooLarge)?;
            if next_count > MAX_MOUNT_NAMESPACE_NODES {
                return Err(MountProjectionError::NamespaceTooLarge);
            }
            // Account conservatively for the strings retained in both indexes and nodes.
            let retained_bytes = path
                .len()
                .checked_mul(6)
                .and_then(|bytes| bytes.checked_add(normalized_path_bytes))
                .ok_or(MountProjectionError::NamespaceTooLarge)?;
            let next_bytes = namespace_bytes
                .checked_add(retained_bytes)
                .ok_or(MountProjectionError::NamespaceTooLarge)?;
            if next_bytes > MAX_MOUNT_NAMESPACE_BYTES {
                return Err(MountProjectionError::NamespaceStorageExceeded {
                    maximum: MAX_MOUNT_NAMESPACE_BYTES,
                });
            }
            *namespace_bytes = next_bytes;
            pending.insert(path.to_owned(), kind);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{MountProjectionError, validate_mount_path};

    #[test]
    fn namespace_paths_reject_portably_invalid_components() {
        for path in [
            "set/with<angle.rom",
            "set/question?.rom",
            "set/NUL.txt",
            "set/trailing.",
        ] {
            assert!(matches!(
                validate_mount_path(path),
                Err(MountProjectionError::InvalidPath(_))
            ));
        }
        assert!(validate_mount_path("set/ordinary-name.rom").is_ok());
    }

    #[test]
    fn namespace_paths_bound_length_depth_and_component_size() {
        let deeply_nested = std::iter::repeat_n("a", super::MAX_MOUNT_PATH_DEPTH + 1)
            .collect::<Vec<_>>()
            .join("/");
        assert!(matches!(
            validate_mount_path(&deeply_nested),
            Err(MountProjectionError::NamespaceTooLarge)
        ));

        let long_path = std::iter::repeat_n("a".repeat(200), 22)
            .collect::<Vec<_>>()
            .join("/");
        assert!(matches!(
            validate_mount_path(&long_path),
            Err(MountProjectionError::NamespaceTooLarge)
        ));

        let oversized_component = format!("{}.rom", "a".repeat(256));
        assert!(matches!(
            validate_mount_path(&oversized_component),
            Err(MountProjectionError::NamespaceTooLarge)
        ));
    }

    #[test]
    fn namespace_projection_rejects_paths_over_its_storage_budget() {
        let mut pending = std::collections::BTreeMap::new();
        let mut normalized = std::collections::BTreeMap::new();
        let mut namespace_bytes = super::MAX_MOUNT_NAMESPACE_BYTES - 1;
        assert!(matches!(
            super::insert_path(
                "game",
                super::PendingKind::Directory,
                &mut pending,
                &mut normalized,
                &mut namespace_bytes,
            ),
            Err(MountProjectionError::NamespaceStorageExceeded { .. })
        ));
    }
}
