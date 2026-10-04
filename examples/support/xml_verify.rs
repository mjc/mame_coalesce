//! Typed recognized-attribute positions shared by XML corpus verifiers.

use std::fmt::Debug;

use mame_coalesce::{catalog_files::XmlAttributePosition, logiqx::AttributePosition};

use crate::support::catalog_verify::{VerifyResult, equal};

pub fn compare_media_positions<F: PartialEq + Debug>(
    field: &str,
    source: &[AttributePosition<F>],
    native: &[XmlAttributePosition<F>],
) -> VerifyResult {
    equal(&format!("{field}.count"), &source.len(), &native.len())?;
    for (index, (source, native)) in source.iter().zip(native).enumerate() {
        equal(
            &format!("{field}[{index}]"),
            &(
                &source.field,
                i64::try_from(source.source_order)?,
                source.location.line,
                source.location.column,
            ),
            &(
                &native.field,
                native.source_order,
                native.location.line,
                native.location.column,
            ),
        )?;
    }
    Ok(())
}
