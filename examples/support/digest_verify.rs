//! Compare normalized source assertions independently of their raw spelling.

use mame_coalesce::catalog_files::{DigestAlgorithm, DigestProvenance, OccurrenceDigest};

use crate::support::catalog_verify::{VerifyResult, equal};

pub fn compare_declared_digests<'a>(
    field: &str,
    expected: impl IntoIterator<Item = (DigestAlgorithm, &'a [u8], &'a str)>,
    native: &[OccurrenceDigest],
) -> VerifyResult {
    let mut expected = expected.into_iter().collect::<Vec<_>>();
    let mut actual = native
        .iter()
        .filter(|digest| digest.provenance == DigestProvenance::SourceDeclared)
        .map(|digest| {
            (
                digest.algorithm,
                digest.value.as_slice(),
                digest.scope.as_str(),
            )
        })
        .collect::<Vec<_>>();
    expected.sort_unstable();
    actual.sort_unstable();
    equal(field, &expected, &actual)
}
