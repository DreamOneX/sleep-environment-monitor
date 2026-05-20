#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FlashError {
    OutOfBounds,
    UnalignedErase,
    InvalidEraseRange,
    WriteRequiresErase,
}

pub trait FlashStorage {
    fn len(&self) -> usize;
    fn sector_size(&self) -> usize;
    fn read(&self, offset: usize, out: &mut [u8]) -> Result<(), FlashError>;
    fn write(&mut self, offset: usize, data: &[u8]) -> Result<(), FlashError>;
    fn erase(&mut self, offset: usize, len: usize) -> Result<(), FlashError>;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub struct InMemoryFlash<const N: usize, const SECTOR_SIZE: usize> {
    data: [u8; N],
}

impl<const N: usize, const SECTOR_SIZE: usize> InMemoryFlash<N, SECTOR_SIZE> {
    pub const fn new() -> Self {
        Self { data: [0xff_u8; N] }
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data
    }

    fn checked_range(
        &self,
        offset: usize,
        len: usize,
    ) -> Result<core::ops::Range<usize>, FlashError> {
        let end = offset.checked_add(len).ok_or(FlashError::OutOfBounds)?;
        if end > N {
            return Err(FlashError::OutOfBounds);
        }

        Ok(offset..end)
    }
}

impl<const N: usize, const SECTOR_SIZE: usize> Default for InMemoryFlash<N, SECTOR_SIZE> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize, const SECTOR_SIZE: usize> FlashStorage for InMemoryFlash<N, SECTOR_SIZE> {
    fn len(&self) -> usize {
        N
    }

    fn sector_size(&self) -> usize {
        SECTOR_SIZE
    }

    fn read(&self, offset: usize, out: &mut [u8]) -> Result<(), FlashError> {
        let range = self.checked_range(offset, out.len())?;
        out.copy_from_slice(&self.data[range]);

        Ok(())
    }

    fn write(&mut self, offset: usize, data: &[u8]) -> Result<(), FlashError> {
        let range = self.checked_range(offset, data.len())?;

        for (existing, new) in self.data[range.clone()].iter().zip(data) {
            if (*existing & *new) != *new {
                return Err(FlashError::WriteRequiresErase);
            }
        }

        for (existing, new) in self.data[range].iter_mut().zip(data) {
            *existing &= *new;
        }

        Ok(())
    }

    fn erase(&mut self, offset: usize, len: usize) -> Result<(), FlashError> {
        if SECTOR_SIZE == 0
            || !offset.is_multiple_of(SECTOR_SIZE)
            || !len.is_multiple_of(SECTOR_SIZE)
        {
            return Err(FlashError::UnalignedErase);
        }
        if len == 0 {
            return Err(FlashError::InvalidEraseRange);
        }

        let range = self.checked_range(offset, len)?;
        self.data[range].fill(0xff);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_flash_is_erased() {
        let flash = InMemoryFlash::<16, 4>::new();

        assert!(flash.as_slice().iter().all(|byte| *byte == 0xff));
    }

    #[test]
    fn write_can_only_clear_bits() {
        let mut flash = InMemoryFlash::<16, 4>::new();

        flash.write(0, &[0b1111_0000]).unwrap();
        assert_eq!(
            flash.write(0, &[0b1111_1111]),
            Err(FlashError::WriteRequiresErase)
        );
        flash.write(0, &[0b1100_0000]).unwrap();

        let mut out = [0_u8; 1];
        flash.read(0, &mut out).unwrap();
        assert_eq!(out[0], 0b1100_0000);
    }

    #[test]
    fn erase_resets_sector_to_erased_value() {
        let mut flash = InMemoryFlash::<16, 4>::new();

        flash.write(4, &[0, 1, 2, 3]).unwrap();
        flash.erase(4, 4).unwrap();

        assert_eq!(&flash.as_slice()[4..8], &[0xff; 4]);
    }

    #[test]
    fn unaligned_erase_fails() {
        let mut flash = InMemoryFlash::<16, 4>::new();

        assert_eq!(flash.erase(1, 4), Err(FlashError::UnalignedErase));
        assert_eq!(flash.erase(0, 3), Err(FlashError::UnalignedErase));
    }

    #[test]
    fn out_of_range_access_fails() {
        let mut flash = InMemoryFlash::<16, 4>::new();
        let mut out = [0_u8; 4];

        assert_eq!(flash.read(13, &mut out), Err(FlashError::OutOfBounds));
        assert_eq!(flash.write(13, &[1, 2, 3, 4]), Err(FlashError::OutOfBounds));
        assert_eq!(flash.erase(12, 8), Err(FlashError::OutOfBounds));
    }
}
