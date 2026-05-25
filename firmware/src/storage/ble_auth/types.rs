pub const AUTH_HEADER_MAGIC: u32 = 0x5345_4241;
pub const AUTH_HEADER_FORMAT_VERSION: u16 = 2;
pub const AUTH_HEADER_LEN: usize = 32;
pub const AUTH_RECORD_MAGIC: u32 = 0x5345_4252;
pub const AUTH_RECORD_FORMAT_VERSION: u16 = 1;
pub const AUTH_RECORD_LEN: usize = 64;
pub const AUTH_RECORD_FLASH_ALIGNMENT: usize = 4;
pub const AUTH_RECORD_FLAG_BONDED: u8 = 0x01;
pub const AUTH_RECORD_FLAG_IRK_PRESENT: u8 = 0x02;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub enum BleAuthRecordStatus {
    Missing,
    BadMagic,
    UnsupportedFormat {
        stored: u16,
        expected: u16,
    },
    BadHeaderLength {
        stored: u16,
        expected: u16,
    },
    ChecksumMismatch {
        stored: u32,
        computed: u32,
    },
    RecordsVersionMismatch {
        stored: u32,
        expected: u32,
    },
    CompatibilityChecksumMismatch {
        stored: u32,
        expected: u32,
    },
    NoRecords,
    RecordCountTooLarge {
        stored: u16,
        capacity: u16,
    },
    RecordRegionTooLarge,
    RecordSetChecksumMismatch {
        stored: u32,
        computed: u32,
    },
    BadRecordMagic {
        index: u16,
    },
    UnsupportedRecordFormat {
        index: u16,
        stored: u16,
        expected: u16,
    },
    BadRecordLength {
        index: u16,
        stored: u16,
        expected: u16,
    },
    BadRecordAddressKind {
        index: u16,
        stored: u8,
    },
    BadRecordSecurityLevel {
        index: u16,
        stored: u8,
    },
    RecordCrcMismatch {
        index: u16,
        stored: u32,
        computed: u32,
    },
    Valid {
        records_version: u32,
        record_count: u16,
    },
}

impl BleAuthRecordStatus {
    pub const fn requires_pairing_window(self) -> bool {
        !matches!(self, Self::Valid { .. })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub enum BleAuthStorageError {
    BufferTooSmall,
    TooManyRecords,
    RecordRegionTooLarge,
    UnalignedFlash,
    Flash(FlashError),
}

impl From<FlashError> for BleAuthStorageError {
    fn from(error: FlashError) -> Self {
        Self::Flash(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub enum BleAuthHeaderError {
    BufferTooSmall,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub enum BleAuthAddressKind {
    Public,
    Random,
}

impl BleAuthAddressKind {
    const fn code(self) -> u8 {
        match self {
            Self::Public => 0,
            Self::Random => 1,
        }
    }

    const fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::Public),
            1 => Some(Self::Random),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub enum BleAuthSecurityLevel {
    NoEncryption,
    Encrypted,
    EncryptedAuthenticated,
}

impl BleAuthSecurityLevel {
    const fn code(self) -> u8 {
        match self {
            Self::NoEncryption => 0,
            Self::Encrypted => 1,
            Self::EncryptedAuthenticated => 2,
        }
    }

    const fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::NoEncryption),
            1 => Some(Self::Encrypted),
            2 => Some(Self::EncryptedAuthenticated),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BleAuthRecord {
    pub address_kind: BleAuthAddressKind,
    pub identity_address: [u8; 6],
    pub long_term_key: [u8; 16],
    pub identity_resolving_key: Option<[u8; 16]>,
    pub security_level: BleAuthSecurityLevel,
    pub bonded: bool,
}

impl BleAuthRecord {
    pub const EMPTY: Self = Self {
        address_kind: BleAuthAddressKind::Public,
        identity_address: [0_u8; 6],
        long_term_key: [0_u8; 16],
        identity_resolving_key: None,
        security_level: BleAuthSecurityLevel::NoEncryption,
        bonded: false,
    };
}

impl Default for BleAuthRecord {
    fn default() -> Self {
        Self::EMPTY
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BleAuthRecordUpsert {
    Updated { index: usize },
    Appended { index: usize },
    ReplacedOldest { index: usize },
    NoCapacity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BleAuthLoadResult {
    pub status: BleAuthRecordStatus,
    pub record_count: usize,
}
