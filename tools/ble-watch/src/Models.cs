using Windows.Devices.Bluetooth;
using Windows.Devices.Bluetooth.GenericAttributeProfile;

internal sealed record AdvertisementRecord(
    DateTimeOffset Timestamp,
    ulong Address,
    BluetoothAddressType AddressType,
    string LocalName,
    short Rssi,
    string AdvertisementType,
    IReadOnlyList<Guid> ServiceUuids);

internal readonly record struct RecordMetadata(
    byte Version,
    ulong Sequence,
    ushort PayloadLength,
    byte Flags,
    uint Crc32,
    bool CurrentBoot);

internal readonly record struct RecordFragment(
    byte Version,
    ulong Sequence,
    ushort Offset,
    byte[] Payload);

internal readonly record struct TransferRecordResult(
    RecordMetadata Metadata,
    byte[] Payload,
    uint ComputedCrc,
    bool AckRequested,
    int NotificationCount);

internal readonly record struct DrainRecordsResult(
    IReadOnlyList<ulong> Sequences,
    StatusSnapshot? InitialStatus,
    StatusSnapshot? FinalStatus);

internal readonly record struct ProtectedReadResult(
    GattCommunicationStatus Status,
    byte? ProtocolError,
    byte[]? Bytes);

internal readonly record struct StatusConnectionOpenResult(
    StatusConnection? Connection,
    int FailureCode);

internal sealed class StatusConnection(
    BluetoothLEDevice device,
    GattDeviceService service,
    GattCharacteristic status) : IDisposable
{
    public GattCharacteristic Status { get; } = status;

    public void Dispose()
    {
        service.Dispose();
        device.Dispose();
    }
}

internal readonly record struct StatusSnapshot(
    byte Version,
    byte Runtime,
    byte Network,
    byte Upload,
    ushort Pending,
    uint ErrorFlags,
    byte? Pairing,
    byte? BootButton,
    uint? PairingRemainingMs,
    uint? BootPressedMs);
