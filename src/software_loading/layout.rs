use std::convert::TryFrom;

use thiserror::Error;

/// A supported mapping from file bytes to destination region bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ByteLayout {
    Linear,
    ByteLane16,
    WordSwap16,
    ByteLane32,
    WordLane32,
    WordSwap32,
}

impl ByteLayout {
    /// Map a catalog load instruction to the byte layout implemented here.
    #[must_use]
    pub const fn from_load(
        instruction: Option<crate::catalog_files::SoftwareLoadInstruction>,
    ) -> Option<Self> {
        use crate::catalog_files::SoftwareLoadInstruction as Load;

        match instruction {
            None
            | Some(
                Load::Load16Word | Load::Load32Dword | Load::Load64Word | Load::Load64WordSwap,
            ) => Some(Self::Linear),
            Some(Load::Load16Byte) => Some(Self::ByteLane16),
            Some(Load::Load16WordSwap) => Some(Self::WordSwap16),
            Some(Load::Load32Byte) => Some(Self::ByteLane32),
            Some(Load::Load32Word) => Some(Self::WordLane32),
            Some(Load::Load32WordSwap) => Some(Self::WordSwap32),
            Some(Load::Reload | Load::Fill | Load::Continue | Load::ReloadPlain | Load::Ignore) => {
                None
            }
        }
    }

    /// Validate and describe where `length` bytes fit in a region.
    pub fn placement(
        self,
        offset: u64,
        length: u64,
        region_length: u64,
    ) -> Result<BytePlacement, LayoutError> {
        if length == 0 {
            return Err(LayoutError::ZeroLength);
        }

        let (group_size, skip, reversed) = self.geometry();
        let additional_group = u64::from(!length.is_multiple_of(group_size));
        let groups = (length / group_size)
            .checked_add(additional_group)
            .ok_or(LayoutError::ArithmeticOverflow)?;
        let grouped_extent = groups
            .checked_mul(group_size)
            .ok_or(LayoutError::ArithmeticOverflow)?;
        let skipped_extent = (groups - 1)
            .checked_mul(skip)
            .ok_or(LayoutError::ArithmeticOverflow)?;
        let extent = grouped_extent
            .checked_add(skipped_extent)
            .ok_or(LayoutError::ArithmeticOverflow)?;
        let end = offset
            .checked_add(extent)
            .ok_or(LayoutError::ArithmeticOverflow)?;

        if end > region_length {
            return Err(LayoutError::OutOfBounds { end, region_length });
        }

        Ok(BytePlacement {
            offset,
            length,
            region_length,
            end,
            group_size,
            skip,
            reversed,
        })
    }

    const fn geometry(self) -> (u64, u64, bool) {
        match self {
            Self::Linear => (1, 0, false),
            Self::ByteLane16 => (1, 1, false),
            Self::WordSwap16 => (2, 0, true),
            Self::ByteLane32 => (1, 3, false),
            Self::WordLane32 => (2, 2, false),
            Self::WordSwap32 => (2, 2, true),
        }
    }
}

/// A checked, allocation-free placement of file bytes in a destination region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BytePlacement {
    offset: u64,
    length: u64,
    region_length: u64,
    end: u64,
    group_size: u64,
    skip: u64,
    reversed: bool,
}

impl BytePlacement {
    /// Return the exclusive end of the padded destination extent.
    #[must_use]
    pub const fn end(self) -> u64 {
        self.end
    }

    /// Return whether the final group contains fewer source bytes than lanes.
    #[must_use]
    pub const fn has_partial_group(self) -> bool {
        !self.length.is_multiple_of(self.group_size)
    }

    /// Return the destination offset for one source byte.
    #[must_use]
    pub fn destination(self, source_index: u64) -> Option<u64> {
        if source_index >= self.length {
            return None;
        }

        let group = source_index / self.group_size;
        let lane = source_index % self.group_size;
        let destination_lane = if self.reversed {
            self.group_size.checked_sub(lane.checked_add(1)?)?
        } else {
            lane
        };
        let stride = self.group_size.checked_add(self.skip)?;
        let group_offset = group.checked_mul(stride)?;
        let relative = group_offset.checked_add(destination_lane)?;
        let destination = self.offset.checked_add(relative)?;

        (destination < self.end).then_some(destination)
    }

    /// Apply this placement after validating all lengths and destinations.
    pub fn apply(&self, bytes: &[u8], region: &mut [u8]) -> Result<(), LayoutError> {
        let actual_input_length =
            u64::try_from(bytes.len()).map_err(|_| LayoutError::HostIndexOverflow)?;
        if actual_input_length != self.length {
            return Err(LayoutError::InputLengthMismatch {
                expected: self.length,
                actual: actual_input_length,
            });
        }

        let actual_region_length =
            u64::try_from(region.len()).map_err(|_| LayoutError::HostIndexOverflow)?;
        if self.end > actual_region_length || self.end > self.region_length {
            return Err(LayoutError::RegionTooShort {
                required: self.end,
                actual: actual_region_length,
            });
        }

        // Validate every host index and destination before changing any bytes.
        for source_index in 0..self.length {
            usize::try_from(source_index).map_err(|_| LayoutError::HostIndexOverflow)?;
            let destination = self
                .destination(source_index)
                .ok_or(LayoutError::ArithmeticOverflow)?;
            let destination =
                usize::try_from(destination).map_err(|_| LayoutError::HostIndexOverflow)?;
            if region.get(destination).is_none() {
                return Err(LayoutError::RegionTooShort {
                    required: self.end,
                    actual: actual_region_length,
                });
            }
        }

        for (source_index, source) in bytes.iter().enumerate() {
            let source_index =
                u64::try_from(source_index).map_err(|_| LayoutError::HostIndexOverflow)?;
            let destination = self
                .destination(source_index)
                .ok_or(LayoutError::ArithmeticOverflow)?;
            let destination =
                usize::try_from(destination).map_err(|_| LayoutError::HostIndexOverflow)?;
            let target = region
                .get_mut(destination)
                .ok_or(LayoutError::RegionTooShort {
                    required: self.end,
                    actual: actual_region_length,
                })?;
            *target = *source;
        }

        Ok(())
    }
}

/// Errors returned while validating or applying a byte placement.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum LayoutError {
    #[error("byte placement length must be nonzero")]
    ZeroLength,
    #[error("byte placement arithmetic overflowed")]
    ArithmeticOverflow,
    #[error("byte placement ends at {end}, beyond region length {region_length}")]
    OutOfBounds { end: u64, region_length: u64 },
    #[error("input length is {actual}, expected {expected}")]
    InputLengthMismatch { expected: u64, actual: u64 },
    #[error("destination region length is {actual}, requires at least {required}")]
    RegionTooShort { required: u64, actual: u64 },
    #[error("byte placement cannot be represented by host indices")]
    HostIndexOverflow,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::{ByteLayout, LayoutError};
    use crate::catalog_files::SoftwareLoadInstruction as Load;

    #[test]
    fn maps_every_software_load_instruction() {
        let mappings = [
            (Load::Load16Byte, Some(ByteLayout::ByteLane16)),
            (Load::Load16Word, Some(ByteLayout::Linear)),
            (Load::Load16WordSwap, Some(ByteLayout::WordSwap16)),
            (Load::Load32Byte, Some(ByteLayout::ByteLane32)),
            (Load::Load32Word, Some(ByteLayout::WordLane32)),
            (Load::Load32WordSwap, Some(ByteLayout::WordSwap32)),
            (Load::Load32Dword, Some(ByteLayout::Linear)),
            (Load::Load64Word, Some(ByteLayout::Linear)),
            (Load::Load64WordSwap, Some(ByteLayout::Linear)),
            (Load::Reload, None),
            (Load::Fill, None),
            (Load::Continue, None),
            (Load::ReloadPlain, None),
            (Load::Ignore, None),
        ];

        for (instruction, expected) in mappings {
            assert_eq!(ByteLayout::from_load(Some(instruction)), expected);
        }
        assert_eq!(ByteLayout::from_load(None), Some(ByteLayout::Linear));
    }

    #[test]
    fn maps_lanes_and_swapped_words() {
        let cases = [
            (ByteLayout::ByteLane16, vec![0, 2, 4], 5),
            (ByteLayout::ByteLane32, vec![0, 4, 8], 9),
            (ByteLayout::WordLane32, vec![0, 1, 4], 6),
            (ByteLayout::WordSwap32, vec![1, 0, 5], 6),
            (ByteLayout::WordSwap16, vec![1, 0, 3], 4),
        ];

        for (layout, destinations, end) in cases {
            let placement = layout.placement(0, 3, 16).expect("valid placement");
            let actual: Vec<_> = (0..3)
                .map(|source_index| placement.destination(source_index).expect("mapped byte"))
                .collect();
            assert_eq!(actual, destinations);
            assert_eq!(placement.end(), end);
        }
    }

    #[test]
    fn applies_lane_and_word_mappings() {
        let cases = [
            (
                ByteLayout::Linear,
                [0x10, 0x20, 0x30, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa],
            ),
            (
                ByteLayout::ByteLane16,
                [0x10, 0xaa, 0x20, 0xaa, 0x30, 0xaa, 0xaa, 0xaa, 0xaa],
            ),
            (
                ByteLayout::WordSwap16,
                [0x20, 0x10, 0xaa, 0x30, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa],
            ),
            (
                ByteLayout::ByteLane32,
                [0x10, 0xaa, 0xaa, 0xaa, 0x20, 0xaa, 0xaa, 0xaa, 0x30],
            ),
            (
                ByteLayout::WordLane32,
                [0x10, 0x20, 0xaa, 0xaa, 0x30, 0xaa, 0xaa, 0xaa, 0xaa],
            ),
            (
                ByteLayout::WordSwap32,
                [0x20, 0x10, 0xaa, 0xaa, 0xaa, 0x30, 0xaa, 0xaa, 0xaa],
            ),
        ];

        for (layout, expected) in cases {
            let placement = layout.placement(0, 3, 9).expect("valid placement");
            let mut region = [0xaa; 9];
            placement
                .apply(&[0x10, 0x20, 0x30], &mut region)
                .expect("checked region");
            assert_eq!(region, expected);
        }
    }

    #[test]
    fn reversed_partial_group_uses_high_lane_and_preserves_padding() {
        let placement = ByteLayout::WordSwap16
            .placement(0, 3, 4)
            .expect("padded placement");
        let mut region = [0xaa; 4];
        placement
            .apply(&[0x10, 0x20, 0x30], &mut region)
            .expect("checked region");

        assert_eq!(region, [0x20, 0x10, 0xaa, 0x30]);
        assert!(placement.has_partial_group());
        assert_eq!(placement.destination(3), None);
    }

    #[test]
    fn validates_padded_extent_and_overflow() {
        assert_eq!(
            ByteLayout::ByteLane16.placement(2, 3, 6),
            Err(LayoutError::OutOfBounds {
                end: 7,
                region_length: 6,
            })
        );
        assert_eq!(
            ByteLayout::Linear.placement(u64::MAX, 1, u64::MAX),
            Err(LayoutError::ArithmeticOverflow)
        );
        assert_eq!(
            ByteLayout::ByteLane32.placement(0, u64::MAX, u64::MAX),
            Err(LayoutError::ArithmeticOverflow)
        );
        assert_eq!(
            ByteLayout::Linear.placement(0, 0, 10),
            Err(LayoutError::ZeroLength)
        );
    }

    #[test]
    fn validation_errors_leave_region_unchanged() {
        let placement = ByteLayout::WordSwap16
            .placement(1, 3, 5)
            .expect("valid placement");
        let mut region = [0xaa; 4];
        assert!(matches!(
            placement.apply(&[1, 2], &mut region),
            Err(LayoutError::InputLengthMismatch { .. })
        ));
        assert_eq!(region, [0xaa; 4]);

        assert!(matches!(
            placement.apply(&[1, 2, 3], &mut region),
            Err(LayoutError::RegionTooShort { .. })
        ));
        assert_eq!(region, [0xaa; 4]);
    }
}
