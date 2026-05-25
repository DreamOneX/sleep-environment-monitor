#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
pub struct BleAuthFlashWorkspace {
    records: [BleAuthRecord; crate::config::ble::AUTH_RECORD_CAPACITY],
    status: BleAuthRecordStatus,
    record_count: usize,
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
impl BleAuthFlashWorkspace {
    const fn new() -> Self {
        Self {
            records: [BleAuthRecord::EMPTY; crate::config::ble::AUTH_RECORD_CAPACITY],
            status: BleAuthRecordStatus::Missing,
            record_count: 0,
        }
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
pub type BleAuthWorkspace = Mutex<CriticalSectionRawMutex, BleAuthFlashWorkspace>;

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
const BLE_AUTH_WORKSPACE_LEN: usize =
    AUTH_HEADER_LEN + AUTH_RECORD_LEN * crate::config::ble::AUTH_RECORD_CAPACITY;

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
static BLE_AUTH_WORKSPACE: StaticCell<BleAuthWorkspace> = StaticCell::new();

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
pub fn ble_auth_workspace() -> &'static BleAuthWorkspace {
    BLE_AUTH_WORKSPACE.init_with(|| Mutex::new(BleAuthFlashWorkspace::new()))
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
async fn clear_saved_ble_auth_records(auth_workspace: &'static BleAuthWorkspace) -> bool {
    let mut flash = match RomBleAuthFlash::new() {
        Ok(flash) => flash,
        Err(error) => {
            warn!("ble auth flash init failed before clear error={:?}", error);
            return false;
        }
    };
    let offset = flash.absolute_offset();
    let len = flash.len();
    match clear_auth_records(&mut flash) {
        Ok(()) => {
            let mut workspace = auth_workspace.lock().await;
            workspace.records = [BleAuthRecord::EMPTY; crate::config::ble::AUTH_RECORD_CAPACITY];
            workspace.status = BleAuthRecordStatus::Missing;
            workspace.record_count = 0;
            info!(
                "ble auth records cleared offset=0x{:08x} len={=usize}",
                offset, len
            );
            true
        }
        Err(error) => {
            warn!(
                "ble auth records clear failed error={:?} offset=0x{:08x} len={=usize}",
                error, offset, len
            );
            false
        }
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
fn should_auto_open_pairing_window_on_boot() -> bool {
    let flash = match RomBleAuthFlash::new() {
        Ok(flash) => flash,
        Err(error) => {
            warn!("ble auth flash init failed error={:?}", error);
            return false;
        }
    };
    let mut records = [BleAuthRecord::EMPTY; crate::config::ble::AUTH_RECORD_CAPACITY];
    let mut scratch = [0_u8; BLE_AUTH_WORKSPACE_LEN];
    let load = match load_auth_records(
        &flash,
        crate::config::ble::AUTH_RECORDS_VERSION,
        crate::config::ble::AUTH_RECORDS_CHECKSUM,
        &mut records,
        &mut scratch,
    ) {
        Ok(load) => load,
        Err(error) => {
            warn!("ble auth records load failed error={:?}", error);
            return false;
        }
    };
    let should_open = should_auto_open_pairing_window(
        crate::config::ble::AUTO_PAIR_ON_AUTH_RECORD_RESET,
        load.status,
    );
    info!(
        "ble auth records status={:?} count={=usize} auto_pair={=bool} offset=0x{:08x} len={=usize}",
        load.status,
        load.record_count,
        should_open,
        flash.absolute_offset(),
        flash.len()
    );
    should_open
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
async fn restore_ble_auth_records(
    stack: &Stack<'_, BleController, DefaultPacketPool>,
    auth_workspace: &'static BleAuthWorkspace,
) {
    let flash = match RomBleAuthFlash::new() {
        Ok(flash) => flash,
        Err(error) => {
            warn!("ble auth flash init failed error={:?}", error);
            return;
        }
    };
    let mut records = [BleAuthRecord::EMPTY; crate::config::ble::AUTH_RECORD_CAPACITY];
    let mut scratch = [0_u8; BLE_AUTH_WORKSPACE_LEN];
    let load = match load_auth_records(
        &flash,
        crate::config::ble::AUTH_RECORDS_VERSION,
        crate::config::ble::AUTH_RECORDS_CHECKSUM,
        &mut records,
        &mut scratch,
    ) {
        Ok(load) => load,
        Err(error) => {
            warn!("ble auth records load failed error={:?}", error);
            let mut workspace = auth_workspace.lock().await;
            workspace.status = BleAuthRecordStatus::Missing;
            workspace.record_count = 0;
            return;
        }
    };

    let mut restored_count = 0_usize;
    for record in records[..load.record_count].iter().copied() {
        match stack.add_bond_information(bond_information_from_auth_record(record)) {
            Ok(()) => restored_count += 1,
            Err(error) => warn!("ble auth bond restore failed error={:?}", error),
        }
    }
    {
        let mut workspace = auth_workspace.lock().await;
        workspace.records = records;
        workspace.status = load.status;
        workspace.record_count = load.record_count;
    }

    info!(
        "ble auth records restored status={:?} loaded={=usize} restored={=usize} offset=0x{:08x} len={=usize}",
        load.status,
        load.record_count,
        restored_count,
        flash.absolute_offset(),
        flash.len()
    );
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
async fn persist_ble_bond_information(
    connection: &GattConnection<'_, '_, DefaultPacketPool>,
    auth_workspace: &'static BleAuthWorkspace,
    bond: &BondInformation,
) {
    let mut flash = match RomBleAuthFlash::new() {
        Ok(flash) => flash,
        Err(error) => {
            warn!(
                "ble auth flash init failed before bond store error={:?}",
                error
            );
            return;
        }
    };
    let record = auth_record_from_bond(connection.raw().peer_addr_kind(), bond);
    let (mut records, mut store_count) = {
        let workspace = auth_workspace.lock().await;
        let record_count = if matches!(workspace.status, BleAuthRecordStatus::Valid { .. }) {
            workspace.record_count
        } else {
            0
        };
        (workspace.records, record_count)
    };

    let (next_store_count, upsert_result) = upsert_auth_record(&mut records, store_count, record);
    store_count = next_store_count;
    match upsert_result {
        BleAuthRecordUpsert::Updated { index } => {
            info!("ble auth record updated index={=usize}", index);
        }
        BleAuthRecordUpsert::Appended { index } => {
            info!("ble auth record appended index={=usize}", index);
        }
        BleAuthRecordUpsert::ReplacedOldest { index } => {
            warn!(
                "ble auth record capacity full; replacing oldest bond record index={=usize}",
                index
            );
        }
        BleAuthRecordUpsert::NoCapacity => {
            warn!("ble auth record capacity is zero; bond record not stored");
            return;
        }
    }

    let mut scratch = [0_u8; BLE_AUTH_WORKSPACE_LEN];
    match store_auth_records(
        &mut flash,
        crate::config::ble::AUTH_RECORDS_VERSION,
        crate::config::ble::AUTH_RECORDS_CHECKSUM,
        &records[..store_count],
        &mut scratch,
    ) {
        Ok(()) => {
            let mut workspace = auth_workspace.lock().await;
            workspace.records = records;
            workspace.record_count = store_count;
            workspace.status = BleAuthRecordStatus::Valid {
                records_version: crate::config::ble::AUTH_RECORDS_VERSION,
                record_count: store_count as u16,
            };
            info!(
                "ble auth bond stored count={=usize} offset=0x{:08x} len={=usize}",
                store_count,
                flash.absolute_offset(),
                flash.len()
            );
        }
        Err(error) => warn!("ble auth bond store failed error={:?}", error),
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
fn bond_information_from_auth_record(record: BleAuthRecord) -> BondInformation {
    BondInformation::new(
        Identity {
            bd_addr: BdAddr::new(record.identity_address),
            irk: record
                .identity_resolving_key
                .map(IdentityResolvingKey::from_le_bytes),
        },
        LongTermKey::from_le_bytes(record.long_term_key),
        security_level_from_auth_record(record.security_level),
        record.bonded,
    )
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
fn auth_record_from_bond(address_kind: AddrKind, bond: &BondInformation) -> BleAuthRecord {
    BleAuthRecord {
        address_kind: auth_address_kind_from_addr_kind(address_kind),
        identity_address: bond.identity.bd_addr.into_inner(),
        long_term_key: bond.ltk.to_le_bytes(),
        identity_resolving_key: bond.identity.irk.map(IdentityResolvingKey::to_le_bytes),
        security_level: auth_security_level_from_connection(bond.security_level),
        bonded: bond.is_bonded,
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
fn auth_address_kind_from_addr_kind(address_kind: AddrKind) -> BleAuthAddressKind {
    if address_kind == AddrKind::RANDOM {
        BleAuthAddressKind::Random
    } else {
        BleAuthAddressKind::Public
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
const fn security_level_from_auth_record(level: BleAuthSecurityLevel) -> SecurityLevel {
    match level {
        BleAuthSecurityLevel::NoEncryption => SecurityLevel::NoEncryption,
        BleAuthSecurityLevel::Encrypted => SecurityLevel::Encrypted,
        BleAuthSecurityLevel::EncryptedAuthenticated => SecurityLevel::EncryptedAuthenticated,
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
const fn auth_security_level_from_connection(level: SecurityLevel) -> BleAuthSecurityLevel {
    match level {
        SecurityLevel::NoEncryption => BleAuthSecurityLevel::NoEncryption,
        SecurityLevel::Encrypted => BleAuthSecurityLevel::Encrypted,
        SecurityLevel::EncryptedAuthenticated => BleAuthSecurityLevel::EncryptedAuthenticated,
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
async fn ensure_ble_authorized(
    connection: &GattConnection<'_, '_, DefaultPacketPool>,
    auth_workspace: &'static BleAuthWorkspace,
) -> Result<(), AttErrorCode> {
    let pairing_open = BLE_PAIRING_STATE.lock().await.is_open();
    if pairing_open {
        return Ok(());
    }

    let has_saved_auth = {
        let workspace = auth_workspace.lock().await;
        matches!(workspace.status, BleAuthRecordStatus::Valid { .. }) && workspace.record_count > 0
    };
    if matches!(
        connection.raw().security_level(),
        Ok(SecurityLevel::Encrypted | SecurityLevel::EncryptedAuthenticated)
    ) && saved_auth_matches_connection(connection, auth_workspace).await
    {
        Ok(())
    } else if has_saved_auth {
        Err(AttErrorCode::INSUFFICIENT_ENCRYPTION)
    } else {
        Err(AttErrorCode::INSUFFICIENT_AUTHORISATION)
    }
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
async fn saved_auth_matches_connection(
    connection: &GattConnection<'_, '_, DefaultPacketPool>,
    auth_workspace: &'static BleAuthWorkspace,
) -> bool {
    let peer_identity = connection.raw().peer_identity();
    let workspace = auth_workspace.lock().await;
    if !matches!(workspace.status, BleAuthRecordStatus::Valid { .. }) {
        return false;
    }

    workspace.records[..workspace.record_count]
        .iter()
        .copied()
        .any(|record| identity_from_auth_record(record).match_identity(&peer_identity))
}

#[cfg(all(target_arch = "riscv32", feature = "ble-upload"))]
fn identity_from_auth_record(record: BleAuthRecord) -> Identity {
    Identity {
        bd_addr: BdAddr::new(record.identity_address),
        irk: record
            .identity_resolving_key
            .map(IdentityResolvingKey::from_le_bytes),
    }
}
