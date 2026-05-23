using System.Collections.Concurrent;
using Windows.Devices.Bluetooth;
using Windows.Devices.Bluetooth.Advertisement;
using Windows.Devices.Bluetooth.GenericAttributeProfile;
using Windows.Storage.Streams;
using System.Runtime.InteropServices;
using System.Text;

static string FormatAddress(ulong address) => $"0x{address:x12}";

static string JoinUuids(IReadOnlyList<Guid> uuids) =>
    uuids.Count == 0 ? "" : string.Join(",", uuids.Select(uuid => uuid.ToString()));

var projectUuid = Guid.Parse("53454d53-240a-4b1e-9bb2-510e7d010001");
var statusUuid = Guid.Parse("53454d53-240a-4b1e-9bb2-510e7d010101");
var projectUuidWindows = Guid.Parse("0100017d-0e51-b29b-1e4b-0a24534d4553");
var statusUuidWindows = Guid.Parse("0101017d-0e51-b29b-1e4b-0a24534d4553");
var metadataUuidWindows = Guid.Parse("0102017d-0e51-b29b-1e4b-0a24534d4553");
var fragmentUuidWindows = Guid.Parse("0103017d-0e51-b29b-1e4b-0a24534d4553");
var controlUuidWindows = Guid.Parse("0104017d-0e51-b29b-1e4b-0a24534d4553");

if (args.Length > 0 && string.Equals(args[0], "scan-closed-window", StringComparison.OrdinalIgnoreCase))
{
    var scanSeconds = args.Length > 1 && int.TryParse(args[1], out var parsedScanSeconds)
        ? parsedScanSeconds
        : 30;
    var scanTargetName = args.Length > 2 ? args[2] : "sleep-env-esp32c3";
    return await ScanThenCheckClosedWindowAsync(
        scanSeconds,
        scanTargetName,
        projectUuid,
        projectUuidWindows,
        metadataUuidWindows,
        fragmentUuidWindows,
        controlUuidWindows);
}

if (args.Length > 0 && string.Equals(args[0], "scan-transfer-record", StringComparison.OrdinalIgnoreCase))
{
    var scanSeconds = args.Length > 1 && int.TryParse(args[1], out var parsedScanSeconds)
        ? parsedScanSeconds
        : 30;
    var scanTargetName = args.Length > 2 ? args[2] : "sleep-env-esp32c3";
    var ackMode = args.Length > 3 ? args[3] : "no-ack";
    var fragmentLength = args.Length > 4 && ushort.TryParse(args[4], out var parsedFragmentLength)
        ? parsedFragmentLength
        : (ushort)128;
    return await ScanThenTransferRecordAsync(
        scanSeconds,
        scanTargetName,
        ackMode,
        fragmentLength,
        projectUuid,
        projectUuidWindows,
        statusUuidWindows,
        metadataUuidWindows,
        fragmentUuidWindows,
        controlUuidWindows);
}

if (args.Length > 0 && string.Equals(args[0], "scan-read-status", StringComparison.OrdinalIgnoreCase))
{
    var scanSeconds = args.Length > 1 && int.TryParse(args[1], out var parsedScanSeconds)
        ? parsedScanSeconds
        : 30;
    var scanTargetName = args.Length > 2 ? args[2] : "sleep-env-esp32c3";
    return await ScanThenReadStatusAsync(
        scanSeconds,
        scanTargetName,
        projectUuid,
        projectUuidWindows,
        statusUuidWindows);
}

if (args.Length > 0 && string.Equals(args[0], "scan-watch-status", StringComparison.OrdinalIgnoreCase))
{
    var scanSeconds = args.Length > 1 && int.TryParse(args[1], out var parsedScanSeconds)
        ? parsedScanSeconds
        : 30;
    var scanTargetName = args.Length > 2 ? args[2] : "sleep-env-esp32c3";
    var watchSeconds = args.Length > 3 && int.TryParse(args[3], out var parsedWatchSeconds)
        ? parsedWatchSeconds
        : 60;
    return await ScanThenWatchStatusAsync(
        scanSeconds,
        scanTargetName,
        watchSeconds,
        projectUuid,
        projectUuidWindows,
        statusUuidWindows);
}

if (args.Length > 0 && string.Equals(args[0], "read-status", StringComparison.OrdinalIgnoreCase))
{
    if (args.Length < 2)
    {
        Console.Error.WriteLine("usage: read-status <hex-address>");
        return 64;
    }

    return await ReadStatusFromArgumentAsync(args[1], projectUuidWindows, statusUuidWindows);
}

var seconds = args.Length > 0 && int.TryParse(args[0], out var parsedSeconds) ? parsedSeconds : 30;
var targetName = args.Length > 1 ? args[1] : "sleep-env-esp32c3";

var seen = new ConcurrentDictionary<ulong, AdvertisementRecord>();
var targetHits = new ConcurrentBag<AdvertisementRecord>();
using var stopped = new ManualResetEventSlim(false);

var watcher = new BluetoothLEAdvertisementWatcher
{
    ScanningMode = BluetoothLEScanningMode.Active,
};

watcher.Received += (_, eventArgs) =>
{
    var name = string.IsNullOrWhiteSpace(eventArgs.Advertisement.LocalName)
        ? "<unnamed>"
        : eventArgs.Advertisement.LocalName;
    var uuids = eventArgs.Advertisement.ServiceUuids.ToArray();
    var record = new AdvertisementRecord(
        DateTimeOffset.Now,
        eventArgs.BluetoothAddress,
        eventArgs.BluetoothAddressType,
        name,
        eventArgs.RawSignalStrengthInDBm,
        eventArgs.AdvertisementType.ToString(),
        uuids);

    seen[eventArgs.BluetoothAddress] = record;

    if (string.Equals(name, targetName, StringComparison.OrdinalIgnoreCase) ||
        uuids.Contains(projectUuid))
    {
        targetHits.Add(record);
        Console.WriteLine(
            $"TARGET ts={record.Timestamp:o} address={FormatAddress(record.Address)} address_type={record.AddressType} name={record.LocalName} rssi={record.Rssi} type={record.AdvertisementType} uuids={JoinUuids(record.ServiceUuids)}");
    }
};

watcher.Stopped += (_, eventArgs) =>
{
    Console.WriteLine($"STOPPED status={watcher.Status} error={eventArgs.Error}");
    stopped.Set();
};

Console.WriteLine($"START target={targetName} seconds={seconds} initial_status={watcher.Status}");
watcher.Start();
Console.WriteLine($"STARTED status={watcher.Status}");
await Task.Delay(TimeSpan.FromSeconds(seconds));
Console.WriteLine($"STOPPING status={watcher.Status}");
watcher.Stop();
stopped.Wait(TimeSpan.FromSeconds(3));

Console.WriteLine($"FINAL status={watcher.Status} seen={seen.Count} target_hits={targetHits.Count}");
foreach (var record in seen.Values.OrderByDescending(record => record.Rssi).Take(30))
{
    Console.WriteLine(
        $"SEEN address={FormatAddress(record.Address)} address_type={record.AddressType} name={record.LocalName} rssi={record.Rssi} type={record.AdvertisementType} uuids={JoinUuids(record.ServiceUuids)}");
}

return targetHits.IsEmpty ? 2 : 0;

static async Task<int> ReadStatusFromArgumentAsync(string rawAddress, Guid serviceUuid, Guid statusUuid)
{
    var normalized = rawAddress.StartsWith("0x", StringComparison.OrdinalIgnoreCase)
        ? rawAddress[2..]
        : rawAddress;
    var address = Convert.ToUInt64(normalized, 16);

    var addressType = BluetoothAddressType.Random;
    return await ReadStatusAsync(address, addressType, serviceUuid, statusUuid);
}

static async Task<int> ReadStatusAsync(
    ulong address,
    BluetoothAddressType addressType,
    Guid serviceUuid,
    Guid statusUuid)
{
    Console.WriteLine(
        $"CONNECT address={FormatAddress(address)} address_type={addressType} service={serviceUuid} status={statusUuid}");
    using var device = await BluetoothLEDevice.FromBluetoothAddressAsync(address, addressType);
    if (device is null)
    {
        Console.WriteLine("DEVICE_NOT_FOUND");
        return 2;
    }

    Console.WriteLine($"DEVICE name={device.Name} connection_status={device.ConnectionStatus}");

    var servicesResult = await device.GetGattServicesForUuidAsync(serviceUuid, BluetoothCacheMode.Uncached);
    Console.WriteLine($"SERVICES status={servicesResult.Status} protocol_error=0x{servicesResult.ProtocolError:x}");
    if (servicesResult.Status != GattCommunicationStatus.Success)
    {
        return 3;
    }
    if (servicesResult.Services.Count == 0)
    {
        Console.WriteLine("SERVICE_NOT_FOUND");
        await DumpGattAsync(device);
        return 4;
    }

    using var service = servicesResult.Services[0];
    var characteristicsResult =
        await service.GetCharacteristicsForUuidAsync(statusUuid, BluetoothCacheMode.Uncached);
    Console.WriteLine(
        $"CHARACTERISTICS status={characteristicsResult.Status} count={characteristicsResult.Characteristics.Count} protocol_error=0x{characteristicsResult.ProtocolError:x}");
    if (characteristicsResult.Status != GattCommunicationStatus.Success)
    {
        return 5;
    }
    if (characteristicsResult.Characteristics.Count == 0)
    {
        Console.WriteLine("STATUS_CHARACTERISTIC_NOT_FOUND");
        return 6;
    }

    var characteristic = characteristicsResult.Characteristics[0];
    Console.WriteLine($"CHARACTERISTIC props={characteristic.CharacteristicProperties}");

    var readResult = await characteristic.ReadValueAsync(BluetoothCacheMode.Uncached);
    Console.WriteLine($"READ status={readResult.Status} protocol_error=0x{readResult.ProtocolError:x}");
    if (readResult.Status != GattCommunicationStatus.Success)
    {
        return 7;
    }

    var bytes = BufferToBytes(readResult.Value);
    Console.WriteLine($"STATUS_BYTES len={bytes.Length} hex={Convert.ToHexString(bytes)}");
    PrintStatusDecoded(bytes);

    return 0;
}

static async Task<int> ScanThenWatchStatusAsync(
    int scanSeconds,
    string targetName,
    int watchSeconds,
    Guid advertisementUuid,
    Guid serviceUuid,
    Guid statusUuid)
{
    var found = await ScanForTargetAsync(scanSeconds, targetName, advertisementUuid, serviceUuid);
    if (found is null)
    {
        return 2;
    }

    await Task.Delay(TimeSpan.FromMilliseconds(300));
    return await WatchStatusAsync(found.Address, found.AddressType, watchSeconds, serviceUuid, statusUuid);
}

static async Task<int> WatchStatusAsync(
    ulong address,
    BluetoothAddressType addressType,
    int watchSeconds,
    Guid serviceUuid,
    Guid statusUuid)
{
    Console.WriteLine(
        $"WATCH_CONNECT address={FormatAddress(address)} address_type={addressType} seconds={watchSeconds}");
    using var device = await BluetoothLEDevice.FromBluetoothAddressAsync(address, addressType);
    if (device is null)
    {
        Console.WriteLine("DEVICE_NOT_FOUND");
        return 2;
    }

    var servicesResult = await device.GetGattServicesForUuidAsync(serviceUuid, BluetoothCacheMode.Uncached);
    Console.WriteLine($"SERVICES status={servicesResult.Status} protocol_error=0x{servicesResult.ProtocolError:x}");
    if (servicesResult.Status != GattCommunicationStatus.Success || servicesResult.Services.Count == 0)
    {
        return 3;
    }

    using var service = servicesResult.Services[0];
    var characteristicsResult =
        await service.GetCharacteristicsForUuidAsync(statusUuid, BluetoothCacheMode.Uncached);
    Console.WriteLine(
        $"CHARACTERISTICS status={characteristicsResult.Status} count={characteristicsResult.Characteristics.Count} protocol_error=0x{characteristicsResult.ProtocolError:x}");
    if (characteristicsResult.Status != GattCommunicationStatus.Success ||
        characteristicsResult.Characteristics.Count == 0)
    {
        return 4;
    }

    var characteristic = characteristicsResult.Characteristics[0];
    var deadline = DateTimeOffset.UtcNow + TimeSpan.FromSeconds(watchSeconds);
    var index = 0;
    while (DateTimeOffset.UtcNow < deadline)
    {
        index++;
        var readResult = await characteristic.ReadValueAsync(BluetoothCacheMode.Uncached);
        Console.WriteLine(
            $"WATCH_READ index={index} status={readResult.Status} protocol_error=0x{readResult.ProtocolError:x}");
        if (readResult.Status != GattCommunicationStatus.Success)
        {
            return 5;
        }

        var bytes = BufferToBytes(readResult.Value);
        Console.WriteLine($"WATCH_STATUS_BYTES index={index} len={bytes.Length} hex={Convert.ToHexString(bytes)}");
        PrintStatusDecoded(bytes);
        await Task.Delay(TimeSpan.FromSeconds(1));
    }

    return 0;
}

static async Task DumpGattAsync(BluetoothLEDevice device)
{
    var allServices = await device.GetGattServicesAsync(BluetoothCacheMode.Uncached);
    Console.WriteLine(
        $"ALL_SERVICES status={allServices.Status} count={allServices.Services.Count} protocol_error=0x{allServices.ProtocolError:x}");
    if (allServices.Status != GattCommunicationStatus.Success)
    {
        return;
    }

    foreach (var service in allServices.Services)
    {
        using (service)
        {
            Console.WriteLine($"SERVICE uuid={service.Uuid}");
            var characteristics = await service.GetCharacteristicsAsync(BluetoothCacheMode.Uncached);
            Console.WriteLine(
                $"  CHARS status={characteristics.Status} count={characteristics.Characteristics.Count} protocol_error=0x{characteristics.ProtocolError:x}");
            if (characteristics.Status != GattCommunicationStatus.Success)
            {
                continue;
            }

            foreach (var characteristic in characteristics.Characteristics)
            {
                Console.WriteLine(
                    $"  CHAR uuid={characteristic.Uuid} props={characteristic.CharacteristicProperties}");
            }
        }
    }
}

static async Task<int> CheckClosedWindowAsync(
    ulong address,
    BluetoothAddressType addressType,
    Guid serviceUuid,
    Guid metadataUuid,
    Guid fragmentUuid,
    Guid controlUuid)
{
    Console.WriteLine($"CLOSED_WINDOW_CONNECT address={FormatAddress(address)} address_type={addressType}");
    using var device = await BluetoothLEDevice.FromBluetoothAddressAsync(address, addressType);
    if (device is null)
    {
        Console.WriteLine("DEVICE_NOT_FOUND");
        return 2;
    }

    var servicesResult = await device.GetGattServicesForUuidAsync(serviceUuid, BluetoothCacheMode.Uncached);
    Console.WriteLine($"SERVICES status={servicesResult.Status} protocol_error=0x{servicesResult.ProtocolError:x}");
    if (servicesResult.Status != GattCommunicationStatus.Success || servicesResult.Services.Count == 0)
    {
        return 3;
    }

    using var service = servicesResult.Services[0];
    var metadata = await GetCharacteristicAsync(service, metadataUuid, "metadata");
    var fragment = await GetCharacteristicAsync(service, fragmentUuid, "fragment");
    var control = await GetCharacteristicAsync(service, controlUuid, "control");
    if (metadata is null || fragment is null || control is null)
    {
        return 4;
    }

    var metadataRead = await metadata.ReadValueAsync(BluetoothCacheMode.Uncached);
    Console.WriteLine(
        $"CLOSED_METADATA_READ status={metadataRead.Status} protocol_error=0x{metadataRead.ProtocolError:x}");
    var fragmentRead = await fragment.ReadValueAsync(BluetoothCacheMode.Uncached);
    Console.WriteLine(
        $"CLOSED_FRAGMENT_READ status={fragmentRead.Status} protocol_error=0x{fragmentRead.ProtocolError:x}");

    var controlFrame = new byte[14];
    controlFrame[0] = 1;
    controlFrame[1] = 1;
    var writer = new DataWriter();
    writer.WriteBytes(controlFrame);
    GattCommunicationStatus? controlWrite = null;
    int? controlExceptionHResult = null;
    try
    {
        controlWrite = await control.WriteValueAsync(writer.DetachBuffer(), GattWriteOption.WriteWithResponse);
        Console.WriteLine($"CLOSED_CONTROL_WRITE status={controlWrite} frame={Convert.ToHexString(controlFrame)}");
    }
    catch (COMException error)
    {
        controlExceptionHResult = error.HResult;
        Console.WriteLine(
            $"CLOSED_CONTROL_WRITE exception=0x{error.HResult:x8} frame={Convert.ToHexString(controlFrame)}");
    }

    var metadataRejected = metadataRead.Status == GattCommunicationStatus.ProtocolError &&
        metadataRead.ProtocolError == 0x08;
    var fragmentRejected = fragmentRead.Status == GattCommunicationStatus.ProtocolError &&
        fragmentRead.ProtocolError == 0x08;
    var controlRejected = controlWrite == GattCommunicationStatus.ProtocolError ||
        controlExceptionHResult == unchecked((int)0x80650008);

    Console.WriteLine(
        $"CLOSED_WINDOW_RESULT metadata_rejected={metadataRejected} fragment_rejected={fragmentRejected} control_rejected={controlRejected}");

    return metadataRejected && fragmentRejected && controlRejected ? 0 : 8;
}

static async Task<GattCharacteristic?> GetCharacteristicAsync(
    GattDeviceService service,
    Guid uuid,
    string name)
{
    var result = await service.GetCharacteristicsForUuidAsync(uuid, BluetoothCacheMode.Uncached);
    Console.WriteLine(
        $"CHAR_LOOKUP name={name} uuid={uuid} status={result.Status} count={result.Characteristics.Count} protocol_error=0x{result.ProtocolError:x}");
    return result.Status == GattCommunicationStatus.Success && result.Characteristics.Count > 0
        ? result.Characteristics[0]
        : null;
}

static async Task<int> ScanThenCheckClosedWindowAsync(
    int seconds,
    string targetName,
    Guid advertisementUuid,
    Guid serviceUuid,
    Guid metadataUuid,
    Guid fragmentUuid,
    Guid controlUuid)
{
    var found = await ScanForTargetAsync(seconds, targetName, advertisementUuid, serviceUuid);
    if (found is null)
    {
        return 2;
    }

    await Task.Delay(TimeSpan.FromMilliseconds(300));
    return await CheckClosedWindowAsync(
        found.Address,
        found.AddressType,
        serviceUuid,
        metadataUuid,
        fragmentUuid,
        controlUuid);
}

static async Task<int> ScanThenReadStatusAsync(
    int seconds,
    string targetName,
    Guid advertisementUuid,
    Guid serviceUuid,
    Guid statusUuid)
{
    var found = await ScanForTargetAsync(seconds, targetName, advertisementUuid, serviceUuid);
    if (found is null)
    {
        return 2;
    }

    await Task.Delay(TimeSpan.FromMilliseconds(300));
    return await ReadStatusAsync(found.Address, found.AddressType, serviceUuid, statusUuid);
}

static async Task<int> ScanThenTransferRecordAsync(
    int seconds,
    string targetName,
    string ackMode,
    ushort fragmentLength,
    Guid advertisementUuid,
    Guid serviceUuid,
    Guid statusUuid,
    Guid metadataUuid,
    Guid fragmentUuid,
    Guid controlUuid)
{
    var found = await ScanForTargetAsync(seconds, targetName, advertisementUuid, serviceUuid);
    if (found is null)
    {
        return 2;
    }

    await Task.Delay(TimeSpan.FromMilliseconds(300));
    return await TransferRecordAsync(
        found.Address,
        found.AddressType,
        ackMode,
        fragmentLength,
        serviceUuid,
        statusUuid,
        metadataUuid,
        fragmentUuid,
        controlUuid);
}

static async Task<int> TransferRecordAsync(
    ulong address,
    BluetoothAddressType addressType,
    string ackMode,
    ushort fragmentLength,
    Guid serviceUuid,
    Guid statusUuid,
    Guid metadataUuid,
    Guid fragmentUuid,
    Guid controlUuid)
{
    var shouldAck = string.Equals(ackMode, "ack", StringComparison.OrdinalIgnoreCase);
    if (!shouldAck && !string.Equals(ackMode, "no-ack", StringComparison.OrdinalIgnoreCase))
    {
        Console.Error.WriteLine("usage: scan-transfer-record [scan-seconds] [name] [no-ack|ack] [fragment-len]");
        return 64;
    }
    if (fragmentLength == 0)
    {
        Console.Error.WriteLine("fragment length must be non-zero");
        return 64;
    }

    Console.WriteLine(
        $"TRANSFER_CONNECT address={FormatAddress(address)} address_type={addressType} ack_mode={(shouldAck ? "ack" : "no-ack")} fragment_len={fragmentLength}");
    using var device = await BluetoothLEDevice.FromBluetoothAddressAsync(address, addressType);
    if (device is null)
    {
        Console.WriteLine("DEVICE_NOT_FOUND");
        return 2;
    }

    Console.WriteLine($"DEVICE name={device.Name} connection_status={device.ConnectionStatus}");
    var servicesResult = await device.GetGattServicesForUuidAsync(serviceUuid, BluetoothCacheMode.Uncached);
    Console.WriteLine($"SERVICES status={servicesResult.Status} protocol_error=0x{servicesResult.ProtocolError:x}");
    if (servicesResult.Status != GattCommunicationStatus.Success || servicesResult.Services.Count == 0)
    {
        return 3;
    }

    using var service = servicesResult.Services[0];
    var status = await GetCharacteristicAsync(service, statusUuid, "status");
    var metadata = await GetCharacteristicAsync(service, metadataUuid, "metadata");
    var fragment = await GetCharacteristicAsync(service, fragmentUuid, "fragment");
    var control = await GetCharacteristicAsync(service, controlUuid, "control");
    if (status is null || metadata is null || fragment is null || control is null)
    {
        return 4;
    }

    Console.WriteLine($"STATUS props={status.CharacteristicProperties}");
    Console.WriteLine($"METADATA props={metadata.CharacteristicProperties}");
    Console.WriteLine($"FRAGMENT props={fragment.CharacteristicProperties}");
    Console.WriteLine($"CONTROL props={control.CharacteristicProperties}");

    if (!await WaitForPairingOpenAsync(status, TimeSpan.FromSeconds(90)))
    {
        return 5;
    }

    var requestMetadata = EncodeControlFrame(1, 0, 0, 0);
    if (!await WaitForAuthorizedMetadataRequestAsync(control, requestMetadata, TimeSpan.FromSeconds(15)))
    {
        return 5;
    }

    var metadataRead = await metadata.ReadValueAsync(BluetoothCacheMode.Uncached);
    Console.WriteLine($"METADATA_READ status={metadataRead.Status} protocol_error=0x{metadataRead.ProtocolError:x}");
    if (metadataRead.Status != GattCommunicationStatus.Success)
    {
        return 6;
    }

    var metadataBytes = BufferToBytes(metadataRead.Value);
    Console.WriteLine($"METADATA_BYTES len={metadataBytes.Length} hex={Convert.ToHexString(metadataBytes)}");
    if (!TryDecodeMetadata(metadataBytes, out var recordMetadata))
    {
        return 7;
    }

    Console.WriteLine(
        $"METADATA_DECODED version={recordMetadata.Version} sequence={recordMetadata.Sequence} payload_len={recordMetadata.PayloadLength} flags=0x{recordMetadata.Flags:x2} crc32=0x{recordMetadata.Crc32:x8} current_boot={recordMetadata.CurrentBoot}");

    var payload = new byte[recordMetadata.PayloadLength];
    var offset = 0;
    while (offset < payload.Length)
    {
        var requested = (ushort)Math.Min(fragmentLength, payload.Length - offset);
        var requestFragment = EncodeControlFrame(2, recordMetadata.Sequence, (ushort)offset, requested);
        if (!await WriteControlAsync(control, "REQUEST_FRAGMENT", requestFragment))
        {
            return 8;
        }

        var fragmentRead = await fragment.ReadValueAsync(BluetoothCacheMode.Uncached);
        Console.WriteLine(
            $"FRAGMENT_READ status={fragmentRead.Status} protocol_error=0x{fragmentRead.ProtocolError:x} requested_offset={offset} requested_len={requested}");
        if (fragmentRead.Status != GattCommunicationStatus.Success)
        {
            return 9;
        }

        var fragmentBytes = BufferToBytes(fragmentRead.Value);
        if (!TryDecodeFragment(fragmentBytes, out var recordFragment))
        {
            return 10;
        }

        Console.WriteLine(
            $"FRAGMENT_DECODED version={recordFragment.Version} sequence={recordFragment.Sequence} offset={recordFragment.Offset} payload_len={recordFragment.Payload.Length} first_payload_hex={Convert.ToHexString(recordFragment.Payload.Take(16).ToArray())}");

        if (recordFragment.Sequence != recordMetadata.Sequence ||
            recordFragment.Offset != offset ||
            recordFragment.Payload.Length == 0 ||
            recordFragment.Payload.Length > requested ||
            offset + recordFragment.Payload.Length > payload.Length)
        {
            Console.WriteLine("FRAGMENT_VALIDATION_FAILED");
            return 11;
        }

        Array.Copy(recordFragment.Payload, 0, payload, offset, recordFragment.Payload.Length);
        offset += recordFragment.Payload.Length;
    }

    var computedCrc = Crc32(payload);
    var crcMatches = computedCrc == recordMetadata.Crc32;
    Console.WriteLine(
        $"PAYLOAD_RESULT len={payload.Length} crc32=0x{computedCrc:x8} crc_matches={crcMatches} utf8_preview={Utf8Preview(payload)}");
    if (!crcMatches)
    {
        return 12;
    }

    var completeFrame = EncodeControlFrame(3, recordMetadata.Sequence, 0, 0);
    if (!await WriteControlAsync(control, "COMPLETE_RECORD", completeFrame))
    {
        return 13;
    }

    if (shouldAck)
    {
        var ackFrame = EncodeControlFrame(4, recordMetadata.Sequence, 0, 0);
        if (!await WriteControlAsync(control, "ACK_RECORD", ackFrame))
        {
            return 14;
        }
        Console.WriteLine($"ACK_RESULT requested=True sequence={recordMetadata.Sequence}");
    }
    else
    {
        Console.WriteLine($"ACK_RESULT requested=False sequence={recordMetadata.Sequence}");
    }

    Console.WriteLine(
        $"TRANSFER_RESULT success=True sequence={recordMetadata.Sequence} payload_len={payload.Length} crc32=0x{computedCrc:x8} ack_requested={shouldAck}");
    return 0;
}

static async Task<bool> WaitForPairingOpenAsync(
    GattCharacteristic status,
    TimeSpan timeout)
{
    var deadline = DateTimeOffset.UtcNow + timeout;
    var attempt = 0;
    Console.WriteLine($"PAIRING_WAIT seconds={timeout.TotalSeconds:0}");

    while (DateTimeOffset.UtcNow < deadline)
    {
        attempt++;
        var read = await status.ReadValueAsync(BluetoothCacheMode.Uncached);
        Console.WriteLine($"PAIRING_WAIT_READ attempt={attempt} status={read.Status} protocol_error=0x{read.ProtocolError:x}");
        if (read.Status == GattCommunicationStatus.Success)
        {
            var bytes = BufferToBytes(read.Value);
            Console.WriteLine($"PAIRING_WAIT_STATUS_BYTES len={bytes.Length} hex={Convert.ToHexString(bytes)}");
            PrintStatusDecoded(bytes);
            if (bytes.Length >= 16 && bytes[10] == 1)
            {
                Console.WriteLine($"PAIRING_OPEN attempt={attempt}");
                return true;
            }
        }

        await Task.Delay(TimeSpan.FromMilliseconds(500));
    }

    Console.WriteLine($"PAIRING_TIMEOUT attempts={attempt}");
    return false;
}

static async Task<bool> WaitForAuthorizedMetadataRequestAsync(
    GattCharacteristic control,
    byte[] requestMetadata,
    TimeSpan timeout)
{
    var deadline = DateTimeOffset.UtcNow + timeout;
    var attempt = 0;
    Console.WriteLine($"AUTH_WAIT seconds={timeout.TotalSeconds:0}");

    while (DateTimeOffset.UtcNow < deadline)
    {
        attempt++;
        if (await WriteControlAsync(control, $"REQUEST_METADATA_ATTEMPT_{attempt}", requestMetadata))
        {
            Console.WriteLine($"AUTH_GRANTED attempt={attempt}");
            return true;
        }

        await Task.Delay(TimeSpan.FromMilliseconds(500));
    }

    Console.WriteLine($"AUTH_TIMEOUT attempts={attempt}");
    return false;
}

static async Task<bool> WriteControlAsync(GattCharacteristic control, string label, byte[] frame)
{
    var writer = new DataWriter();
    writer.WriteBytes(frame);
    try
    {
        var status = await control.WriteValueAsync(writer.DetachBuffer(), GattWriteOption.WriteWithResponse);
        Console.WriteLine($"{label}_WRITE status={status} frame={Convert.ToHexString(frame)}");
        return status == GattCommunicationStatus.Success;
    }
    catch (COMException error)
    {
        Console.WriteLine($"{label}_WRITE exception=0x{error.HResult:x8} frame={Convert.ToHexString(frame)}");
        return false;
    }
}

static byte[] EncodeControlFrame(byte opcode, ulong sequence, ushort offset, ushort length)
{
    var frame = new byte[14];
    frame[0] = 1;
    frame[1] = opcode;
    BitConverter.GetBytes(sequence).CopyTo(frame, 2);
    BitConverter.GetBytes(offset).CopyTo(frame, 10);
    BitConverter.GetBytes(length).CopyTo(frame, 12);
    return frame;
}

static bool TryDecodeMetadata(byte[] bytes, out RecordMetadata metadata)
{
    metadata = default;
    if (bytes.Length < 17 || bytes[0] != 1)
    {
        Console.WriteLine("METADATA_DECODE_FAILED");
        return false;
    }

    metadata = new RecordMetadata(
        bytes[0],
        BitConverter.ToUInt64(bytes, 1),
        BitConverter.ToUInt16(bytes, 9),
        bytes[11],
        BitConverter.ToUInt32(bytes, 12),
        bytes[16] != 0);
    return true;
}

static bool TryDecodeFragment(byte[] bytes, out RecordFragment fragment)
{
    fragment = default;
    if (bytes.Length < 13 || bytes[0] != 1)
    {
        Console.WriteLine("FRAGMENT_DECODE_FAILED");
        return false;
    }

    var payloadLen = BitConverter.ToUInt16(bytes, 11);
    if (bytes.Length < 13 + payloadLen)
    {
        Console.WriteLine($"FRAGMENT_LENGTH_FAILED raw_len={bytes.Length} declared_payload_len={payloadLen}");
        return false;
    }

    var payload = bytes.Skip(13).Take(payloadLen).ToArray();
    fragment = new RecordFragment(
        bytes[0],
        BitConverter.ToUInt64(bytes, 1),
        BitConverter.ToUInt16(bytes, 9),
        payload);
    return true;
}

static uint Crc32(byte[] data)
{
    var crc = 0xffff_ffffu;
    foreach (var value in data)
    {
        crc ^= value;
        for (var bit = 0; bit < 8; bit++)
        {
            var mask = 0u - (crc & 1u);
            crc = (crc >> 1) ^ (0xedb8_8320u & mask);
        }
    }

    return ~crc;
}

static string Utf8Preview(byte[] payload)
{
    var previewBytes = payload.Take(120).ToArray();
    var text = Encoding.UTF8.GetString(previewBytes)
        .Replace("\r", "\\r", StringComparison.Ordinal)
        .Replace("\n", "\\n", StringComparison.Ordinal);
    return payload.Length > previewBytes.Length ? text + "..." : text;
}

static async Task<AdvertisementRecord?> ScanForTargetAsync(
    int seconds,
    string targetName,
    Guid advertisementUuid,
    Guid serviceUuid)
{
    var found = new TaskCompletionSource<AdvertisementRecord>(
        TaskCreationOptions.RunContinuationsAsynchronously);
    using var stopped = new ManualResetEventSlim(false);
    var watcher = new BluetoothLEAdvertisementWatcher
    {
        ScanningMode = BluetoothLEScanningMode.Active,
    };

    watcher.Received += (_, eventArgs) =>
    {
        var name = string.IsNullOrWhiteSpace(eventArgs.Advertisement.LocalName)
            ? "<unnamed>"
            : eventArgs.Advertisement.LocalName;
        var uuids = eventArgs.Advertisement.ServiceUuids.ToArray();
        if (!string.Equals(name, targetName, StringComparison.OrdinalIgnoreCase) &&
            !uuids.Contains(advertisementUuid) &&
            !uuids.Contains(serviceUuid))
        {
            return;
        }

        var record = new AdvertisementRecord(
            DateTimeOffset.Now,
            eventArgs.BluetoothAddress,
            eventArgs.BluetoothAddressType,
            name,
            eventArgs.RawSignalStrengthInDBm,
            eventArgs.AdvertisementType.ToString(),
            uuids);
        Console.WriteLine(
            $"FOUND ts={record.Timestamp:o} address={FormatAddress(record.Address)} address_type={record.AddressType} name={record.LocalName} rssi={record.Rssi} type={record.AdvertisementType} uuids={JoinUuids(record.ServiceUuids)}");
        found.TrySetResult(record);
    };

    watcher.Stopped += (_, eventArgs) =>
    {
        Console.WriteLine($"SCAN_STOPPED status={watcher.Status} error={eventArgs.Error}");
        stopped.Set();
    };

    Console.WriteLine($"SCAN_START target={targetName} seconds={seconds} initial_status={watcher.Status}");
    watcher.Start();
    var completed = await Task.WhenAny(found.Task, Task.Delay(TimeSpan.FromSeconds(seconds)));
    Console.WriteLine($"SCAN_STOPPING status={watcher.Status}");
    watcher.Stop();
    stopped.Wait(TimeSpan.FromSeconds(3));

    if (completed != found.Task)
    {
        Console.WriteLine("TARGET_NOT_FOUND");
        return null;
    }

    return await found.Task;
}

static byte[] BufferToBytes(IBuffer buffer)
{
    using var reader = DataReader.FromBuffer(buffer);
    var bytes = new byte[buffer.Length];
    reader.ReadBytes(bytes);
    return bytes;
}

static string DecodeRuntime(byte value) => value switch
{
    0 => "Disabled",
    1 => "ControllerReady",
    2 => "HostPending",
    3 => "Advertising",
    4 => "Connected",
    5 => "Error",
    _ => $"Unknown({value})",
};

static string DecodeNetwork(byte value) => value switch
{
    0 => "Disconnected",
    1 => "Connecting",
    2 => "Connected",
    3 => "IpReady",
    _ => $"Unknown({value})",
};

static string DecodeUpload(byte value) => value switch
{
    0 => "Idle",
    1 => "Success",
    2 => "Failed",
    3 => "DiscoveryFailed",
    4 => "TimeFailed",
    5 => "TransportFailed",
    6 => "HttpFailed",
    _ => $"Unknown({value})",
};

static void PrintStatusDecoded(byte[] bytes)
{
    if (bytes.Length < 10)
    {
        return;
    }

    var pending = BitConverter.ToUInt16(bytes, 4);
    var errors = BitConverter.ToUInt32(bytes, 6);
    Console.WriteLine(
        $"STATUS_DECODED version={bytes[0]} runtime={DecodeRuntime(bytes[1])} network={DecodeNetwork(bytes[2])} upload={DecodeUpload(bytes[3])} pending={pending} error_flags=0x{errors:x8}");

    if (bytes.Length < 16)
    {
        return;
    }

    var pairingRemainingMs = BitConverter.ToUInt32(bytes, 12);
    if (bytes.Length >= 20)
    {
        var bootPressedMs = BitConverter.ToUInt32(bytes, 16);
        Console.WriteLine(
            $"STATUS_PAIRING pairing={DecodePairing(bytes[10])} boot_button={DecodeBootButton(bytes[11])} remaining_ms={pairingRemainingMs} pressed_ms={bootPressedMs}");
    }
    else
    {
        Console.WriteLine(
            $"STATUS_PAIRING pairing={DecodePairing(bytes[10])} boot_button={DecodeBootButton(bytes[11])} remaining_ms={pairingRemainingMs}");
    }
}

static string DecodePairing(byte value) => value switch
{
    0 => "Closed",
    1 => "Open",
    _ => $"Unknown({value})",
};

static string DecodeBootButton(byte value) => value switch
{
    0 => "Released",
    1 => "Pressed",
    _ => $"Unknown({value})",
};

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
