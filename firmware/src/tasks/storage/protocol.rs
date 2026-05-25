#[cfg(target_arch = "riscv32")]
#[derive(Clone, Copy)]
#[cfg_attr(target_arch = "riscv32", derive(defmt::Format))]
pub enum StorageClient {
    Wifi,
    Ble,
}

#[cfg(target_arch = "riscv32")]
#[derive(Clone, Copy)]
pub enum StorageCommand {
    Append(Measurement),
    Peek(StorageClient),
    Ack {
        client: StorageClient,
        sequence: u64,
    },
}

#[cfg(target_arch = "riscv32")]
#[derive(Clone, Copy)]
#[allow(
    clippy::large_enum_variant,
    reason = "StorageResponse crosses an Embassy Signal with a fixed payload buffer; boxing would require heap allocation on the target."
)]
pub enum StorageResponse {
    Peeked(Option<StoredPayload>),
    Acked(bool),
    Error(StorageError),
}
