use crate::board::{
    FLASH_APP_RESERVED_OFFSET, FLASH_APP_RESERVED_SIZE, FLASH_BLE_AUTH_REGION_OFFSET,
    FLASH_BLE_AUTH_REGION_SIZE, FLASH_SECTOR_SIZE_BYTES, FLASH_SPOOL_REGION_OFFSET,
    FLASH_SPOOL_REGION_SIZE, FLASH_SYSTEM_RESERVED_OFFSET, FLASH_SYSTEM_RESERVED_SIZE,
    FLASH_TOTAL_SIZE_BYTES,
};
#[cfg(target_arch = "riscv32")]
use crate::storage::flash_model::{FlashError, FlashStorage};

pub const MAX_PROTECTED_FLASH_RANGES: usize = 4;
#[cfg(target_arch = "riscv32")]
const ROM_FLASH_WORD_CHUNK: usize = 16;
#[cfg(target_arch = "riscv32")]
const FLASH_SMOKE_PATTERN: [u8; 16] = [
    0x53, 0x45, 0x46, 0x54, 0x18, 0x0b, 0x5a, 0xa5, 0x00, 0x3c, 0x00, 0x00, 0x5a, 0xa5, 0xc3, 0x3c,
];

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
    pub ble_auth: FlashRange,
    pub spool: FlashRange,
    pub protected: [Option<FlashRange>; MAX_PROTECTED_FLASH_RANGES],
}

impl FlashRegionLayout {
    pub const fn default_spool() -> Self {
        Self {
            flash_size: FLASH_TOTAL_SIZE_BYTES,
            sector_size: FLASH_SECTOR_SIZE_BYTES,
            ble_auth: FlashRange::new(FLASH_BLE_AUTH_REGION_OFFSET, FLASH_BLE_AUTH_REGION_SIZE),
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
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub enum FlashRegionError {
    ZeroFlash,
    ZeroSector,
    ZeroBleAuth,
    ZeroSpool,
    OutOfBounds,
    Unaligned,
    ProtectedOverlap,
    BleAuthSpoolOverlap,
}

#[cfg(target_arch = "riscv32")]
#[derive(Clone, Copy, Debug, Eq, PartialEq, defmt::Format)]
pub enum RomFlashError {
    Region(FlashRegionError),
    OutOfBounds,
    Unaligned,
    InvalidEraseRange,
    ReadFailed(i32),
    WriteFailed(i32),
    EraseFailed(i32),
}

#[cfg(target_arch = "riscv32")]
#[derive(Clone, Copy, Debug, Eq, PartialEq, defmt::Format)]
pub enum FlashSmokeError {
    Flash(RomFlashError),
    EraseVerifyFailed,
    ReadbackMismatch,
    CleanupEraseFailed(RomFlashError),
}

#[cfg(target_arch = "riscv32")]
impl From<RomFlashError> for FlashSmokeError {
    fn from(error: RomFlashError) -> Self {
        Self::Flash(error)
    }
}

pub fn validate_flash_region_layout(layout: &FlashRegionLayout) -> Result<(), FlashRegionError> {
    if layout.flash_size == 0 {
        return Err(FlashRegionError::ZeroFlash);
    }
    if layout.sector_size == 0 {
        return Err(FlashRegionError::ZeroSector);
    }
    validate_flash_data_range(layout.ble_auth, layout, FlashRegionError::ZeroBleAuth)?;
    validate_flash_data_range(layout.spool, layout, FlashRegionError::ZeroSpool)?;
    if layout.ble_auth.overlaps(layout.spool) {
        return Err(FlashRegionError::BleAuthSpoolOverlap);
    }

    for protected in layout.protected.into_iter().flatten() {
        let Some(protected_end) = protected.end() else {
            return Err(FlashRegionError::OutOfBounds);
        };
        if protected_end > layout.flash_size {
            return Err(FlashRegionError::OutOfBounds);
        }
        if layout.ble_auth.overlaps(protected) || layout.spool.overlaps(protected) {
            return Err(FlashRegionError::ProtectedOverlap);
        }
    }

    Ok(())
}

pub fn validate_default_flash_spool_region() -> Result<(), FlashRegionError> {
    validate_flash_region_layout(&FlashRegionLayout::default_spool())
}

fn validate_flash_data_range(
    range: FlashRange,
    layout: &FlashRegionLayout,
    zero_error: FlashRegionError,
) -> Result<(), FlashRegionError> {
    if range.is_empty() {
        return Err(zero_error);
    }
    if !is_aligned(range.offset, layout.sector_size) || !is_aligned(range.size, layout.sector_size)
    {
        return Err(FlashRegionError::Unaligned);
    }

    let Some(end) = range.end() else {
        return Err(FlashRegionError::OutOfBounds);
    };
    if end > layout.flash_size {
        return Err(FlashRegionError::OutOfBounds);
    }

    Ok(())
}

const fn is_aligned(value: u32, alignment: u32) -> bool {
    value.is_multiple_of(alignment)
}

#[cfg(target_arch = "riscv32")]
pub struct RomSpoolFlash {
    range: FlashRange,
    sector_size: u32,
}

#[cfg(target_arch = "riscv32")]
pub struct RomBleAuthFlash {
    range: FlashRange,
}

#[cfg(target_arch = "riscv32")]
impl RomSpoolFlash {
    pub fn new() -> Result<Self, RomFlashError> {
        let layout = FlashRegionLayout::default_spool();
        validate_flash_region_layout(&layout).map_err(RomFlashError::Region)?;

        Ok(Self {
            range: layout.spool,
            sector_size: layout.sector_size,
        })
    }

    pub const fn absolute_offset(&self) -> u32 {
        self.range.offset
    }

    fn read_region(&self, offset: usize, out: &mut [u8]) -> Result<(), RomFlashError> {
        rom_flash_read_range(self.range, offset, out)
    }

    fn write_region(&mut self, offset: usize, data: &[u8]) -> Result<(), RomFlashError> {
        if !offset.is_multiple_of(4) || !data.len().is_multiple_of(4) {
            return Err(RomFlashError::Unaligned);
        }

        let mut absolute = check_flash_subrange(self.range, offset, data.len())?;
        let mut remaining = data;
        let mut words = [0_u32; ROM_FLASH_WORD_CHUNK];

        while !remaining.is_empty() {
            let chunk_len = remaining.len().min(words.len() * 4);
            let word_count = chunk_len / 4;
            let byte_len = word_count * 4;

            for (word, source) in words[..word_count]
                .iter_mut()
                .zip(remaining[..byte_len].chunks_exact(4))
            {
                *word = u32::from_le_bytes([source[0], source[1], source[2], source[3]]);
            }

            let result = unsafe {
                esp_hal::rom::spiflash::esp_rom_spiflash_write(
                    absolute,
                    words.as_ptr(),
                    byte_len as u32,
                )
            };
            if result != esp_hal::rom::spiflash::ESP_ROM_SPIFLASH_RESULT_OK {
                return Err(RomFlashError::WriteFailed(result));
            }

            words[..word_count].fill(0);
            absolute += byte_len as u32;
            remaining = &remaining[byte_len..];
        }

        Ok(())
    }

    fn erase_region(&mut self, offset: usize, len: usize) -> Result<(), RomFlashError> {
        if len == 0 {
            return Err(RomFlashError::InvalidEraseRange);
        }
        if !offset.is_multiple_of(self.sector_size as usize)
            || !len.is_multiple_of(self.sector_size as usize)
        {
            return Err(RomFlashError::Unaligned);
        }

        let absolute = check_flash_subrange(self.range, offset, len)?;
        let first_sector = absolute / self.sector_size;
        let sectors = len as u32 / self.sector_size;

        for sector in first_sector..first_sector + sectors {
            let result = unsafe { esp_hal::rom::spiflash::esp_rom_spiflash_erase_sector(sector) };
            if result != esp_hal::rom::spiflash::ESP_ROM_SPIFLASH_RESULT_OK {
                return Err(RomFlashError::EraseFailed(result));
            }
        }

        Ok(())
    }
}

#[cfg(target_arch = "riscv32")]
impl RomBleAuthFlash {
    pub fn new() -> Result<Self, RomFlashError> {
        let layout = FlashRegionLayout::default_spool();
        validate_flash_region_layout(&layout).map_err(RomFlashError::Region)?;

        Ok(Self {
            range: layout.ble_auth,
        })
    }

    pub const fn absolute_offset(&self) -> u32 {
        self.range.offset
    }

    pub const fn len(&self) -> usize {
        self.range.size as usize
    }

    pub const fn is_empty(&self) -> bool {
        self.range.is_empty()
    }

    pub fn read(&self, offset: usize, out: &mut [u8]) -> Result<(), RomFlashError> {
        rom_flash_read_range(self.range, offset, out)
    }
}

#[cfg(target_arch = "riscv32")]
fn rom_flash_read_range(
    range: FlashRange,
    offset: usize,
    out: &mut [u8],
) -> Result<(), RomFlashError> {
    if !offset.is_multiple_of(4) || !out.len().is_multiple_of(4) {
        return Err(RomFlashError::Unaligned);
    }

    let mut absolute = check_flash_subrange(range, offset, out.len())?;
    let mut remaining = out;
    let mut words = [0_u32; ROM_FLASH_WORD_CHUNK];

    while !remaining.is_empty() {
        let chunk_len = remaining.len().min(words.len() * 4);
        let word_count = chunk_len / 4;
        let byte_len = word_count * 4;

        let result = unsafe {
            esp_hal::rom::spiflash::esp_rom_spiflash_read(
                absolute,
                words.as_mut_ptr().cast_const(),
                byte_len as u32,
            )
        };
        if result != esp_hal::rom::spiflash::ESP_ROM_SPIFLASH_RESULT_OK {
            return Err(RomFlashError::ReadFailed(result));
        }

        for (destination, word) in remaining[..byte_len]
            .chunks_exact_mut(4)
            .zip(words[..word_count].iter())
        {
            destination.copy_from_slice(&word.to_le_bytes());
        }

        absolute += byte_len as u32;
        remaining = &mut remaining[byte_len..];
    }

    Ok(())
}

#[cfg(target_arch = "riscv32")]
fn check_flash_subrange(
    range: FlashRange,
    offset: usize,
    len: usize,
) -> Result<u32, RomFlashError> {
    let offset = u32::try_from(offset).map_err(|_| RomFlashError::OutOfBounds)?;
    let len = u32::try_from(len).map_err(|_| RomFlashError::OutOfBounds)?;
    let end = offset.checked_add(len).ok_or(RomFlashError::OutOfBounds)?;
    if end > range.size {
        return Err(RomFlashError::OutOfBounds);
    }

    range
        .offset
        .checked_add(offset)
        .ok_or(RomFlashError::OutOfBounds)
}

#[cfg(target_arch = "riscv32")]
impl FlashStorage for RomSpoolFlash {
    fn len(&self) -> usize {
        self.range.size as usize
    }

    fn sector_size(&self) -> usize {
        self.sector_size as usize
    }

    fn read(&self, offset: usize, out: &mut [u8]) -> Result<(), FlashError> {
        self.read_region(offset, out).map_err(rom_to_flash_error)
    }

    fn write(&mut self, offset: usize, data: &[u8]) -> Result<(), FlashError> {
        self.write_region(offset, data).map_err(rom_to_flash_error)
    }

    fn erase(&mut self, offset: usize, len: usize) -> Result<(), FlashError> {
        self.erase_region(offset, len).map_err(rom_to_flash_error)
    }
}

#[cfg(target_arch = "riscv32")]
pub fn run_flash_smoke_test() -> Result<u32, FlashSmokeError> {
    let mut flash = RomSpoolFlash::new()?;

    flash.erase_region(0, flash.sector_size as usize)?;

    let mut erased = [0_u8; FLASH_SMOKE_PATTERN.len()];
    flash.read_region(0, &mut erased)?;
    if erased.iter().any(|byte| *byte != 0xff) {
        return Err(FlashSmokeError::EraseVerifyFailed);
    }

    flash.write_region(0, &FLASH_SMOKE_PATTERN)?;

    let mut readback = [0_u8; FLASH_SMOKE_PATTERN.len()];
    flash.read_region(0, &mut readback)?;
    if readback != FLASH_SMOKE_PATTERN {
        return Err(FlashSmokeError::ReadbackMismatch);
    }

    flash
        .erase_region(0, flash.sector_size as usize)
        .map_err(FlashSmokeError::CleanupEraseFailed)?;

    Ok(flash.absolute_offset())
}

#[cfg(target_arch = "riscv32")]
fn rom_to_flash_error(error: RomFlashError) -> FlashError {
    match error {
        RomFlashError::OutOfBounds | RomFlashError::Region(_) => FlashError::OutOfBounds,
        RomFlashError::Unaligned => FlashError::UnalignedErase,
        RomFlashError::InvalidEraseRange => FlashError::InvalidEraseRange,
        RomFlashError::ReadFailed(_)
        | RomFlashError::WriteFailed(_)
        | RomFlashError::EraseFailed(_) => FlashError::WriteRequiresErase,
    }
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
    fn zero_sized_ble_auth_region_is_rejected() {
        let mut layout = FlashRegionLayout::default_spool();
        layout.ble_auth.size = 0;

        assert_eq!(
            validate_flash_region_layout(&layout),
            Err(FlashRegionError::ZeroBleAuth)
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
    fn out_of_bounds_ble_auth_region_is_rejected() {
        let mut layout = FlashRegionLayout::default_spool();
        layout.ble_auth.offset = FLASH_TOTAL_SIZE_BYTES - FLASH_SECTOR_SIZE_BYTES;
        layout.ble_auth.size = FLASH_SECTOR_SIZE_BYTES * 2;

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
    fn sector_unaligned_ble_auth_region_is_rejected() {
        let mut layout = FlashRegionLayout::default_spool();
        layout.ble_auth.offset += 1;

        assert_eq!(
            validate_flash_region_layout(&layout),
            Err(FlashRegionError::Unaligned)
        );

        let mut layout = FlashRegionLayout::default_spool();
        layout.ble_auth.size -= 1;

        assert_eq!(
            validate_flash_region_layout(&layout),
            Err(FlashRegionError::Unaligned)
        );
    }

    #[test]
    fn ble_auth_spool_overlap_configuration_is_rejected() {
        let mut layout = FlashRegionLayout::default_spool();
        layout.ble_auth = FlashRange::new(FLASH_SPOOL_REGION_OFFSET, FLASH_SECTOR_SIZE_BYTES);

        assert_eq!(
            validate_flash_region_layout(&layout),
            Err(FlashRegionError::BleAuthSpoolOverlap)
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

    #[test]
    fn ble_auth_region_is_adjacent_to_spool_region() {
        let layout = FlashRegionLayout::default_spool();

        assert_eq!(layout.ble_auth.end(), Some(layout.spool.offset));
        assert!(!layout.ble_auth.overlaps(layout.spool));
        assert_eq!(layout.ble_auth.size, FLASH_SECTOR_SIZE_BYTES);
    }
}
