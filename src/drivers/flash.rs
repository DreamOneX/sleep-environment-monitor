use crate::board::{
    FLASH_APP_RESERVED_OFFSET, FLASH_APP_RESERVED_SIZE, FLASH_SECTOR_SIZE_BYTES,
    FLASH_SPOOL_REGION_OFFSET, FLASH_SPOOL_REGION_SIZE, FLASH_SYSTEM_RESERVED_OFFSET,
    FLASH_SYSTEM_RESERVED_SIZE, FLASH_TOTAL_SIZE_BYTES,
};

pub const MAX_PROTECTED_FLASH_RANGES: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlashRange {
    pub offset: u32,
    pub size: u32,
}

impl FlashRange {
    pub const fn new(offset: u32, size: u32) -> Self {
        Self { offset, size }
    }

    pub const fn end(self) -> Option<u32> {
        self.offset.checked_add(self.size)
    }

    pub const fn is_empty(self) -> bool {
        self.size == 0
    }

    pub const fn overlaps(self, other: Self) -> bool {
        let Some(self_end) = self.end() else {
            return true;
        };
        let Some(other_end) = other.end() else {
            return true;
        };

        self.offset < other_end && other.offset < self_end
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FlashRegionLayout {
    pub flash_size: u32,
    pub sector_size: u32,
    pub spool: FlashRange,
    pub protected: [Option<FlashRange>; MAX_PROTECTED_FLASH_RANGES],
}

impl FlashRegionLayout {
    pub const fn default_spool() -> Self {
        Self {
            flash_size: FLASH_TOTAL_SIZE_BYTES,
            sector_size: FLASH_SECTOR_SIZE_BYTES,
            spool: FlashRange::new(FLASH_SPOOL_REGION_OFFSET, FLASH_SPOOL_REGION_SIZE),
            protected: [
                Some(FlashRange::new(
                    FLASH_SYSTEM_RESERVED_OFFSET,
                    FLASH_SYSTEM_RESERVED_SIZE,
                )),
                Some(FlashRange::new(
                    FLASH_APP_RESERVED_OFFSET,
                    FLASH_APP_RESERVED_SIZE,
                )),
                None,
                None,
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlashRegionError {
    ZeroFlash,
    ZeroSector,
    ZeroSpool,
    OutOfBounds,
    Unaligned,
    ProtectedOverlap,
}

pub fn validate_flash_region_layout(layout: &FlashRegionLayout) -> Result<(), FlashRegionError> {
    if layout.flash_size == 0 {
        return Err(FlashRegionError::ZeroFlash);
    }
    if layout.sector_size == 0 {
        return Err(FlashRegionError::ZeroSector);
    }
    if layout.spool.is_empty() {
        return Err(FlashRegionError::ZeroSpool);
    }
    if !is_aligned(layout.spool.offset, layout.sector_size)
        || !is_aligned(layout.spool.size, layout.sector_size)
    {
        return Err(FlashRegionError::Unaligned);
    }

    let Some(spool_end) = layout.spool.end() else {
        return Err(FlashRegionError::OutOfBounds);
    };
    if spool_end > layout.flash_size {
        return Err(FlashRegionError::OutOfBounds);
    }

    for protected in layout.protected.into_iter().flatten() {
        let Some(protected_end) = protected.end() else {
            return Err(FlashRegionError::OutOfBounds);
        };
        if protected_end > layout.flash_size {
            return Err(FlashRegionError::OutOfBounds);
        }
        if layout.spool.overlaps(protected) {
            return Err(FlashRegionError::ProtectedOverlap);
        }
    }

    Ok(())
}

pub fn validate_default_flash_spool_region() -> Result<(), FlashRegionError> {
    validate_flash_region_layout(&FlashRegionLayout::default_spool())
}

const fn is_aligned(value: u32, alignment: u32) -> bool {
    value.is_multiple_of(alignment)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_spool_region_is_valid() {
        assert_eq!(validate_default_flash_spool_region(), Ok(()));
    }

    #[test]
    fn flash_range_overlap_detects_intersections() {
        let first = FlashRange::new(0x1000, 0x1000);
        let second = FlashRange::new(0x1800, 0x1000);
        let adjacent = FlashRange::new(0x2000, 0x1000);

        assert!(first.overlaps(second));
        assert!(!first.overlaps(adjacent));
    }

    #[test]
    fn zero_sized_spool_region_is_rejected() {
        let mut layout = FlashRegionLayout::default_spool();
        layout.spool.size = 0;

        assert_eq!(
            validate_flash_region_layout(&layout),
            Err(FlashRegionError::ZeroSpool)
        );
    }

    #[test]
    fn out_of_bounds_spool_region_is_rejected() {
        let mut layout = FlashRegionLayout::default_spool();
        layout.spool.offset = FLASH_TOTAL_SIZE_BYTES - FLASH_SECTOR_SIZE_BYTES;
        layout.spool.size = FLASH_SECTOR_SIZE_BYTES * 2;

        assert_eq!(
            validate_flash_region_layout(&layout),
            Err(FlashRegionError::OutOfBounds)
        );
    }

    #[test]
    fn sector_unaligned_spool_region_is_rejected() {
        let mut layout = FlashRegionLayout::default_spool();
        layout.spool.offset += 1;

        assert_eq!(
            validate_flash_region_layout(&layout),
            Err(FlashRegionError::Unaligned)
        );

        let mut layout = FlashRegionLayout::default_spool();
        layout.spool.size -= 1;

        assert_eq!(
            validate_flash_region_layout(&layout),
            Err(FlashRegionError::Unaligned)
        );
    }

    #[test]
    fn app_overlap_configuration_is_rejected() {
        let mut layout = FlashRegionLayout::default_spool();
        layout.spool = FlashRange::new(FLASH_APP_RESERVED_OFFSET, FLASH_SECTOR_SIZE_BYTES);

        assert_eq!(
            validate_flash_region_layout(&layout),
            Err(FlashRegionError::ProtectedOverlap)
        );
    }

    #[test]
    fn protected_range_outside_flash_is_rejected() {
        let mut layout = FlashRegionLayout::default_spool();
        layout.protected[2] = Some(FlashRange::new(
            FLASH_TOTAL_SIZE_BYTES,
            FLASH_SECTOR_SIZE_BYTES,
        ));

        assert_eq!(
            validate_flash_region_layout(&layout),
            Err(FlashRegionError::OutOfBounds)
        );
    }
}
