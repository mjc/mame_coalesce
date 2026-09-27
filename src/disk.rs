use crate::hashes::Sha1Digest;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ParentDiskName(String);

impl ParentDiskName {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DiskName(String);

impl DiskName {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DiskIdentitySha1(Sha1Digest);

impl DiskIdentitySha1 {
    #[must_use]
    pub const fn new(digest: Sha1Digest) -> Self {
        Self(digest)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &Sha1Digest {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContainerSha1(Sha1Digest);

impl ContainerSha1 {
    #[must_use]
    pub const fn new(digest: Sha1Digest) -> Self {
        Self(digest)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiskDigestScope {
    /// The SHA-1 returned by MAME's CHD header API, whose content varies by CHD version.
    ChdHeaderSha1,
    Unknown,
}

impl DiskDigestScope {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ChdHeaderSha1 => "chd_header_sha1",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiskRequirement {
    name: DiskName,
    expected_sha1: Option<DiskIdentitySha1>,
    digest_scope: DiskDigestScope,
    parent: Option<ParentDiskName>,
}

impl DiskRequirement {
    #[must_use]
    pub const fn new(
        name: DiskName,
        expected_sha1: Option<DiskIdentitySha1>,
        digest_scope: DiskDigestScope,
    ) -> Self {
        Self {
            name,
            expected_sha1,
            digest_scope,
            parent: None,
        }
    }

    #[must_use]
    pub const fn name(&self) -> &DiskName {
        &self.name
    }

    #[must_use]
    pub const fn expected_sha1(&self) -> Option<DiskIdentitySha1> {
        self.expected_sha1
    }

    #[must_use]
    pub const fn digest_scope(&self) -> DiskDigestScope {
        self.digest_scope
    }

    #[must_use]
    pub fn with_parent(mut self, parent: ParentDiskName) -> Self {
        self.parent = Some(parent);
        self
    }

    #[must_use]
    pub const fn parent(&self) -> Option<&ParentDiskName> {
        self.parent.as_ref()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiskObservation {
    Missing,
    ContainerPresent {
        byte_sha1: Option<ContainerSha1>,
    },
    UnsupportedContainer,
    /// A CHD identity established by verification, not merely read from its header.
    LogicalIdentity(DiskIdentitySha1),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiskVerificationState {
    Missing,
    UnknownDigestScope,
    IdentityNotDeclared,
    UnverifiedContainer,
    UnsupportedContainer,
    VerifiedLogicalIdentity,
    LogicalIdentityMismatch,
}

#[must_use]
pub fn audit_disk(
    requirement: &DiskRequirement,
    observation: DiskObservation,
) -> DiskVerificationState {
    match observation {
        DiskObservation::Missing => DiskVerificationState::Missing,
        DiskObservation::UnsupportedContainer => DiskVerificationState::UnsupportedContainer,
        DiskObservation::ContainerPresent { .. } => match requirement.digest_scope {
            DiskDigestScope::ChdHeaderSha1 if requirement.expected_sha1.is_some() => {
                DiskVerificationState::UnverifiedContainer
            }
            DiskDigestScope::ChdHeaderSha1 => DiskVerificationState::IdentityNotDeclared,
            DiskDigestScope::Unknown => DiskVerificationState::UnknownDigestScope,
        },
        DiskObservation::LogicalIdentity(actual) => {
            if !matches!(requirement.digest_scope, DiskDigestScope::ChdHeaderSha1) {
                return DiskVerificationState::UnknownDigestScope;
            }
            match requirement.expected_sha1 {
                Some(expected) if expected == actual => {
                    DiskVerificationState::VerifiedLogicalIdentity
                }
                Some(_) => DiskVerificationState::LogicalIdentityMismatch,
                None => DiskVerificationState::IdentityNotDeclared,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn requirement() -> DiskRequirement {
        DiskRequirement::new(
            DiskName::new("demo_disk"),
            Some(DiskIdentitySha1::new([7; 20])),
            DiskDigestScope::ChdHeaderSha1,
        )
    }

    #[test]
    fn missing_disk_is_not_reported_as_a_hash_failure() {
        assert_eq!(
            audit_disk(&requirement(), DiskObservation::Missing),
            DiskVerificationState::Missing
        );
    }

    #[test]
    fn unknown_digest_scope_is_explicit() {
        let requirement = DiskRequirement::new(
            DiskName::new("demo_disk"),
            Some(DiskIdentitySha1::new([7; 20])),
            DiskDigestScope::Unknown,
        );
        assert_eq!(
            audit_disk(
                &requirement,
                DiskObservation::ContainerPresent {
                    byte_sha1: Some(ContainerSha1::new([7; 20])),
                }
            ),
            DiskVerificationState::UnknownDigestScope
        );
    }

    #[test]
    fn container_digest_cannot_verify_logical_disk_identity() {
        assert_eq!(
            audit_disk(
                &requirement(),
                DiskObservation::ContainerPresent {
                    byte_sha1: Some(ContainerSha1::new([7; 20])),
                }
            ),
            DiskVerificationState::UnverifiedContainer
        );
    }

    #[test]
    fn unsupported_container_is_distinct_from_an_unverified_one() {
        assert_eq!(
            audit_disk(&requirement(), DiskObservation::UnsupportedContainer),
            DiskVerificationState::UnsupportedContainer
        );
    }

    #[test]
    fn matching_logical_identity_is_verified() {
        assert_eq!(
            audit_disk(
                &requirement(),
                DiskObservation::LogicalIdentity(DiskIdentitySha1::new([7; 20]))
            ),
            DiskVerificationState::VerifiedLogicalIdentity
        );
    }

    #[test]
    fn mismatching_logical_identity_is_reported() {
        assert_eq!(
            audit_disk(
                &requirement(),
                DiskObservation::LogicalIdentity(DiskIdentitySha1::new([8; 20]))
            ),
            DiskVerificationState::LogicalIdentityMismatch
        );
    }

    #[test]
    fn parent_disk_reference_is_typed_and_preserved() {
        let requirement = requirement().with_parent(ParentDiskName::new("parent_disk"));
        assert_eq!(
            requirement.parent().map(ParentDiskName::as_str),
            Some("parent_disk")
        );
    }

    #[test]
    fn absent_source_digest_is_not_misreported_as_unknown_scope() {
        let requirement = DiskRequirement::new(
            DiskName::new("demo_disk"),
            None,
            DiskDigestScope::ChdHeaderSha1,
        );
        assert_eq!(
            audit_disk(
                &requirement,
                DiskObservation::ContainerPresent { byte_sha1: None }
            ),
            DiskVerificationState::IdentityNotDeclared
        );
    }
}
