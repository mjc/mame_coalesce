//! Source-declared first-run lengths, independent of destination geometry.

use crate::catalog_files::{SoftwareFileOperation, SoftwareLoadInstruction};

/// Derive the first verification run, never the largest reload/progress run.
/// An invalid or overflowing continuation makes the length unknown, not a prefix.
pub fn file_verification_length(
    entries: impl IntoIterator<Item = (Option<SoftwareLoadInstruction>, Option<u64>)>,
) -> Option<u64> {
    let mut entries = entries.into_iter();
    let (instruction, size) = entries.next()?;
    if SoftwareFileOperation::from_instruction(instruction) != SoftwareFileOperation::Load {
        return None;
    }
    let mut length = size.filter(|size| *size > 0)?;
    for (instruction, size) in entries {
        match SoftwareFileOperation::from_instruction(instruction) {
            SoftwareFileOperation::Continue => {
                length = length.checked_add(size.filter(|size| *size > 0)?)?;
            }
            SoftwareFileOperation::Ignore => length = length.checked_add(size?)?,
            SoftwareFileOperation::Load
            | SoftwareFileOperation::Reload
            | SoftwareFileOperation::ReloadPlain
            | SoftwareFileOperation::Fill => break,
        }
    }
    Some(length)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verification_counts_continue_and_ignore_but_not_reload() {
        let entries = [
            (None, Some(4)),
            (Some(SoftwareLoadInstruction::Continue), Some(2)),
            (Some(SoftwareLoadInstruction::Ignore), Some(2)),
            (Some(SoftwareLoadInstruction::Reload), Some(10)),
        ];
        assert_eq!(file_verification_length(entries), Some(8));
    }
}
