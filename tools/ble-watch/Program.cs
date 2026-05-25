using System.Collections.Concurrent;
using Windows.Devices.Bluetooth;
using Windows.Devices.Bluetooth.Advertisement;
using Windows.Devices.Enumeration;
using Windows.Devices.Bluetooth.GenericAttributeProfile;
using Windows.Foundation;
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

if (args.Length > 0 && string.Equals(args[0], "scan-read-metadata-now", StringComparison.OrdinalIgnoreCase))
{
    var scanSeconds = args.Length > 1 && int.TryParse(args[1], out var parsedScanSeconds)
        ? parsedScanSeconds
        : 30;
    var scanTargetName = args.Length > 2 ? args[2] : "sleep-env-esp32c3";
    var expectation = args.Length > 3 ? args[3] : "expect-success";
    var pairMode = args.Length > 4 ? args[4] : "auto-pair";
    return await ScanThenReadMetadataNowAsync(
        scanSeconds,
        scanTargetName,
        expectation,
        pairMode,
        projectUuid,
        projectUuidWindows,
        statusUuidWindows,
        metadataUuidWindows,
        controlUuidWindows);
}

if (args.Length > 0 && string.Equals(args[0], "scan-unpair", StringComparison.OrdinalIgnoreCase))
{
    var scanSeconds = args.Length > 1 && int.TryParse(args[1], out var parsedScanSeconds)
        ? parsedScanSeconds
        : 30;
    var scanTargetName = args.Length > 2 ? args[2] : "sleep-env-esp32c3";
    return await ScanThenUnpairAsync(
        scanSeconds,
        scanTargetName,
        projectUuid,
        projectUuidWindows);
}

if (args.Length > 0 && string.Equals(args[0], "scan-unpair-then-pair-metadata", StringComparison.OrdinalIgnoreCase))
{
    var scanSeconds = args.Length > 1 && int.TryParse(args[1], out var parsedScanSeconds)
        ? parsedScanSeconds
        : 30;
    var scanTargetName = args.Length > 2 ? args[2] : "sleep-env-esp32c3";
    var waitWindowSeconds = args.Length > 3 && int.TryParse(args[3], out var parsedWaitWindowSeconds)
        ? parsedWaitWindowSeconds
        : 90;
    return await ScanThenUnpairThenPairMetadataAsync(
        scanSeconds,
        scanTargetName,
        waitWindowSeconds,
        projectUuid,
        projectUuidWindows,
        statusUuidWindows,
        metadataUuidWindows,
        controlUuidWindows);
}

if (args.Length > 0 && string.Equals(args[0], "scan-watch-clear-gesture", StringComparison.OrdinalIgnoreCase))
{
    var scanSeconds = args.Length > 1 && int.TryParse(args[1], out var parsedScanSeconds)
        ? parsedScanSeconds
        : 30;
    var scanTargetName = args.Length > 2 ? args[2] : "sleep-env-esp32c3";
    var watchSeconds = args.Length > 3 && int.TryParse(args[3], out var parsedWatchSeconds)
        ? parsedWatchSeconds
        : 180;
    var holdMillis = args.Length > 4 && uint.TryParse(args[4], out var parsedHoldMillis)
        ? parsedHoldMillis
        : 8_000u;
    return await ScanThenWatchClearGestureAsync(
        scanSeconds,
        scanTargetName,
        watchSeconds,
        holdMillis,
        projectUuid,
        projectUuidWindows,
        statusUuidWindows);
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

if (args.Length > 0 && string.Equals(args[0], "scan-transfer-record-now", StringComparison.OrdinalIgnoreCase))
{
    var scanSeconds = args.Length > 1 && int.TryParse(args[1], out var parsedScanSeconds)
        ? parsedScanSeconds
        : 30;
    var scanTargetName = args.Length > 2 ? args[2] : "sleep-env-esp32c3";
    var ackMode = args.Length > 3 ? args[3] : "no-ack";
    var fragmentLength = args.Length > 4 && ushort.TryParse(args[4], out var parsedFragmentLength)
        ? parsedFragmentLength
        : (ushort)128;
    var pairMode = args.Length > 5 ? args[5] : "auto-pair";
    return await ScanThenTransferRecordNowAsync(
        scanSeconds,
        scanTargetName,
        ackMode,
        fragmentLength,
        pairMode,
        projectUuid,
        projectUuidWindows,
        statusUuidWindows,
        metadataUuidWindows,
        fragmentUuidWindows,
        controlUuidWindows);
}

if (args.Length > 0 && string.Equals(args[0], "scan-ack-then-peek-next", StringComparison.OrdinalIgnoreCase))
{
    var scanSeconds = args.Length > 1 && int.TryParse(args[1], out var parsedScanSeconds)
        ? parsedScanSeconds
        : 30;
    var scanTargetName = args.Length > 2 ? args[2] : "sleep-env-esp32c3";
    var fragmentLength = args.Length > 3 && ushort.TryParse(args[3], out var parsedFragmentLength)
        ? parsedFragmentLength
        : (ushort)128;
    return await ScanThenAckThenPeekNextAsync(
        scanSeconds,
        scanTargetName,
        fragmentLength,
        projectUuid,
        projectUuidWindows,
        statusUuidWindows,
        metadataUuidWindows,
        fragmentUuidWindows,
        controlUuidWindows);
}

if (args.Length > 0 && string.Equals(args[0], "scan-transfer-record-notify", StringComparison.OrdinalIgnoreCase))
{
    var scanSeconds = args.Length > 1 && int.TryParse(args[1], out var parsedScanSeconds)
        ? parsedScanSeconds
        : 30;
    var scanTargetName = args.Length > 2 ? args[2] : "sleep-env-esp32c3";
    var ackMode = args.Length > 3 ? args[3] : "no-ack";
    var fragmentLength = args.Length > 4 && ushort.TryParse(args[4], out var parsedFragmentLength)
        ? parsedFragmentLength
        : (ushort)128;
    return await ScanThenTransferRecordWithNotificationsAsync(
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

if (args.Length > 0 && string.Equals(args[0], "scan-disconnect-preserves-record", StringComparison.OrdinalIgnoreCase))
{
    var scanSeconds = args.Length > 1 && int.TryParse(args[1], out var parsedScanSeconds)
        ? parsedScanSeconds
        : 30;
    var scanTargetName = args.Length > 2 ? args[2] : "sleep-env-esp32c3";
    var fragmentLength = args.Length > 3 && ushort.TryParse(args[3], out var parsedFragmentLength)
        ? parsedFragmentLength
        : (ushort)128;
    return await ScanThenCheckDisconnectPreservesRecordAsync(
        scanSeconds,
        scanTargetName,
        fragmentLength,
        projectUuid,
        projectUuidWindows,
        statusUuidWindows,
        metadataUuidWindows,
        fragmentUuidWindows,
        controlUuidWindows);
}

if (args.Length > 0 && string.Equals(args[0], "scan-drain-then-disconnect-preserves-record", StringComparison.OrdinalIgnoreCase))
{
    var scanSeconds = args.Length > 1 && int.TryParse(args[1], out var parsedScanSeconds)
        ? parsedScanSeconds
        : 30;
    var scanTargetName = args.Length > 2 ? args[2] : "sleep-env-esp32c3";
    var fragmentLength = args.Length > 3 && ushort.TryParse(args[3], out var parsedFragmentLength)
        ? parsedFragmentLength
        : (ushort)128;
    var targetPending = args.Length > 4 && int.TryParse(args[4], out var parsedTargetPending)
        ? parsedTargetPending
        : 25;
    var maxDrain = args.Length > 5 && int.TryParse(args[5], out var parsedMaxDrain)
        ? parsedMaxDrain
        : 40;
    var statusEvery = args.Length > 6 && int.TryParse(args[6], out var parsedStatusEvery)
        ? parsedStatusEvery
        : 8;
    return await ScanThenDrainThenCheckDisconnectPreservesRecordAsync(
        scanSeconds,
        scanTargetName,
        fragmentLength,
        targetPending,
        maxDrain,
        statusEvery,
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
    const int maxConnectionAttempts = 3;

    for (var attempt = 1; attempt <= maxConnectionAttempts; attempt++)
    {
        var opened = await OpenStatusConnectionAsync(
            address,
            addressType,
            serviceUuid,
            statusUuid,
            "READ_STATUS",
            attempt,
            printPairing: true,
            dumpGattWhenServiceMissing: true);
        if (opened.Connection is null)
        {
            if (attempt < maxConnectionAttempts)
            {
                Console.WriteLine(
                    $"READ_STATUS_RECONNECT reason=open_failed failure_code={opened.FailureCode} next_attempt={attempt + 1}");
                await Task.Delay(TimeSpan.FromMilliseconds(500 * attempt));
                continue;
            }

            return opened.FailureCode;
        }

        using var connection = opened.Connection;
        Console.WriteLine($"CHARACTERISTIC props={connection.Status.CharacteristicProperties}");

        var readResult = await ReadStatusValueWithRetryAsync(connection.Status, "READ");
        if (readResult.Status != GattCommunicationStatus.Success)
        {
            if (attempt < maxConnectionAttempts)
            {
                Console.WriteLine(
                    $"READ_STATUS_RECONNECT reason=read_failed status={readResult.Status} next_attempt={attempt + 1}");
                await Task.Delay(TimeSpan.FromMilliseconds(500 * attempt));
                continue;
            }

            return 7;
        }

        var bytes = BufferToBytes(readResult.Value);
        Console.WriteLine($"STATUS_BYTES len={bytes.Length} hex={Convert.ToHexString(bytes)}");
        PrintStatusDecoded(bytes);

        return 0;
    }

    return 7;
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

static async Task<int> ScanThenUnpairAsync(
    int scanSeconds,
    string targetName,
    Guid advertisementUuid,
    Guid serviceUuid)
{
    var found = await ScanForTargetAsync(scanSeconds, targetName, advertisementUuid, serviceUuid);
    if (found is null)
    {
        return 2;
    }

    await Task.Delay(TimeSpan.FromMilliseconds(300));
    return await UnpairAsync(found.Address, found.AddressType);
}

static async Task<int> UnpairAsync(
    ulong address,
    BluetoothAddressType addressType)
{
    const string label = "UNPAIR";
    Console.WriteLine($"{label}_CONNECT address={FormatAddress(address)} address_type={addressType}");
    using var device = await BluetoothLEDevice.FromBluetoothAddressAsync(address, addressType);
    if (device is null)
    {
        Console.WriteLine($"{label}_DEVICE_NOT_FOUND");
        return 2;
    }

    return await TryUnpairDeviceAsync(device, label) ? 0 : 3;
}

static async Task<bool> TryUnpairDeviceAsync(BluetoothLEDevice device, string label)
{
    var pairing = device.DeviceInformation.Pairing;
    PrintPairingState(device, label);
    if (!pairing.IsPaired)
    {
        Console.WriteLine($"{label}_RESULT success=True already_unpaired=True");
        return true;
    }

    try
    {
        var result = await pairing.UnpairAsync();
        Console.WriteLine($"{label}_RESULT status={result.Status}");
        PrintPairingState(device, $"{label}_AFTER");
        return result.Status is DeviceUnpairingResultStatus.Unpaired
            or DeviceUnpairingResultStatus.AlreadyUnpaired;
    }
    catch (Exception error)
    {
        Console.WriteLine($"{label}_ERROR type={error.GetType().Name} message={error.Message}");
        return false;
    }
}

static async Task<int> ScanThenUnpairThenPairMetadataAsync(
    int seconds,
    string targetName,
    int waitWindowSeconds,
    Guid advertisementUuid,
    Guid serviceUuid,
    Guid statusUuid,
    Guid metadataUuid,
    Guid controlUuid)
{
    if (waitWindowSeconds <= 0)
    {
        Console.Error.WriteLine(
            "usage: scan-unpair-then-pair-metadata [scan-seconds] [name] [wait-window-seconds]");
        return 64;
    }

    var found = await ScanForTargetAsync(seconds, targetName, advertisementUuid, serviceUuid);
    if (found is null)
    {
        return 2;
    }

    await Task.Delay(TimeSpan.FromMilliseconds(300));
    return await UnpairThenPairMetadataAsync(
        found.Address,
        found.AddressType,
        waitWindowSeconds,
        serviceUuid,
        statusUuid,
        metadataUuid,
        controlUuid);
}

static async Task<int> UnpairThenPairMetadataAsync(
    ulong address,
    BluetoothAddressType addressType,
    int waitWindowSeconds,
    Guid serviceUuid,
    Guid statusUuid,
    Guid metadataUuid,
    Guid controlUuid)
{
    const string label = "UNPAIR_PAIR_METADATA";
    Console.WriteLine(
        $"{label}_CONNECT address={FormatAddress(address)} address_type={addressType} wait_window_seconds={waitWindowSeconds}");

    using (var unpairDevice = await BluetoothLEDevice.FromBluetoothAddressAsync(address, addressType))
    {
        if (unpairDevice is null)
        {
            Console.WriteLine($"{label}_DEVICE_NOT_FOUND");
            return 2;
        }

        if (!await TryUnpairDeviceAsync(unpairDevice, label))
        {
            Console.WriteLine($"{label}_RESULT success=False phase=unpair");
            return 3;
        }
    }

    Console.WriteLine(
        $"{label}_RTT_REQUIREMENT expected_firmware_log=\"ble auth record updated/appended/capacity full\" note=\"central-side success alone does not validate auth-record replacement\"");

    await Task.Delay(TimeSpan.FromMilliseconds(500));
    {
        using var windowDevice = await BluetoothLEDevice.FromBluetoothAddressAsync(address, addressType);
        if (windowDevice is null)
        {
            Console.WriteLine($"{label}_DEVICE_NOT_FOUND");
            return 2;
        }

        var prePairServices = await GetGattServicesForUuidWithRetryAsync(
            windowDevice,
            serviceUuid,
            $"{label}_PRE_PAIR");
        if (prePairServices.Status != GattCommunicationStatus.Success || prePairServices.Services.Count == 0)
        {
            Console.WriteLine($"{label}_RESULT success=False phase=pre_pair_services");
            return 4;
        }

        using var prePairService = prePairServices.Services[0];
        var prePairStatus = await GetCharacteristicAsync(prePairService, statusUuid, "status");
        if (prePairStatus is null)
        {
            Console.WriteLine($"{label}_RESULT success=False phase=pre_pair_status");
            return 5;
        }

        _ = await ReadStatusSnapshotAsync(prePairStatus, $"{label}_BEFORE_WINDOW");
        if (!await WaitForPairingOpenAsync(prePairStatus, TimeSpan.FromSeconds(waitWindowSeconds)))
        {
            Console.WriteLine($"{label}_RESULT success=False phase=pairing_window");
            return 6;
        }
    }

    await Task.Delay(TimeSpan.FromMilliseconds(500));
    using var device = await BluetoothLEDevice.FromBluetoothAddressAsync(address, addressType);
    if (device is null)
    {
        Console.WriteLine($"{label}_DEVICE_NOT_FOUND");
        return 2;
    }

    if (!await EnsurePairedAsync(device, label))
    {
        Console.WriteLine($"{label}_RESULT success=False phase=pair");
        return 7;
    }

    await Task.Delay(TimeSpan.FromMilliseconds(500));
    var servicesResult = await GetGattServicesForUuidWithRetryAsync(device, serviceUuid, $"{label}_AFTER_PAIR");
    if (servicesResult.Status != GattCommunicationStatus.Success || servicesResult.Services.Count == 0)
    {
        Console.WriteLine($"{label}_RESULT success=False phase=after_pair_services");
        return 8;
    }

    using var service = servicesResult.Services[0];
    var status = await GetCharacteristicAsync(service, statusUuid, "status");
    var metadata = await GetCharacteristicAsync(service, metadataUuid, "metadata");
    var control = await GetCharacteristicAsync(service, controlUuid, "control");
    if (status is null || metadata is null || control is null)
    {
        Console.WriteLine($"{label}_RESULT success=False phase=after_pair_characteristics");
        return 9;
    }

    _ = await ReadStatusSnapshotAsync(status, $"{label}_AFTER_PAIR");
    var recordMetadata = await RequestAndReadMetadataAsync(metadata, control, label);
    if (recordMetadata is null)
    {
        Console.WriteLine($"{label}_RESULT success=False phase=metadata");
        return 10;
    }

    Console.WriteLine(
        $"{label}_SUMMARY success=True sequence={recordMetadata.Value.Sequence} payload_len={recordMetadata.Value.PayloadLength} rtt_required=True");
    return 0;
}

static async Task<int> ScanThenWatchClearGestureAsync(
    int scanSeconds,
    string targetName,
    int watchSeconds,
    uint holdMillis,
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
    return await WatchClearGestureAsync(
        found.Address,
        found.AddressType,
        watchSeconds,
        holdMillis,
        serviceUuid,
        statusUuid);
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
    var deadline = DateTimeOffset.UtcNow + TimeSpan.FromSeconds(watchSeconds);
    var index = 0;
    var connectionAttempt = 1;
    var opened = await OpenStatusConnectionAsync(
        address,
        addressType,
        serviceUuid,
        statusUuid,
        "WATCH",
        connectionAttempt,
        printPairing: false,
        dumpGattWhenServiceMissing: false);
    if (opened.Connection is null)
    {
        return opened.FailureCode;
    }

    var connection = opened.Connection;
    try
    {
        while (DateTimeOffset.UtcNow < deadline)
        {
            index++;
            var maybeSnapshot = await ReadStatusSnapshotRecoverableAsync(connection.Status, $"WATCH_{index}");
            if (maybeSnapshot is null)
            {
                connection.Dispose();
                connectionAttempt++;
                Console.WriteLine($"WATCH_RECONNECT reason=status_read_failed next_attempt={connectionAttempt}");
                var reopened = await OpenStatusConnectionAsync(
                    address,
                    addressType,
                    serviceUuid,
                    statusUuid,
                    "WATCH",
                    connectionAttempt,
                    printPairing: false,
                    dumpGattWhenServiceMissing: false);
                if (reopened.Connection is null)
                {
                    return 5;
                }
                connection = reopened.Connection;
                await Task.Delay(TimeSpan.FromMilliseconds(250));
                continue;
            }

            await Task.Delay(TimeSpan.FromSeconds(1));
        }
    }
    finally
    {
        connection.Dispose();
    }

    return 0;
}

static async Task<int> WatchClearGestureAsync(
    ulong address,
    BluetoothAddressType addressType,
    int watchSeconds,
    uint holdMillis,
    Guid serviceUuid,
    Guid statusUuid)
{
    Console.WriteLine(
        $"CLEAR_GESTURE_CONNECT address={FormatAddress(address)} address_type={addressType} seconds={watchSeconds} hold_ms={holdMillis}");
    Console.WriteLine(
        "CLEAR_GESTURE_OPERATOR action=release_boot_io9_until_released_then_hold_9_to_10_seconds_then_release");

    var observedReleased = false;
    var observedPressedAfterRelease = false;
    var observedHoldThreshold = false;
    var observedRefreshedWindow = false;
    var observedReleasedAfterHold = false;
    var reportedClearEffectObserved = false;
    int? releasedIndex = null;
    int? pressedAfterReleaseIndex = null;
    int? holdThresholdIndex = null;
    int? refreshedWindowIndex = null;
    int? releasedAfterHoldIndex = null;
    uint? holdThresholdPressedMs = null;
    uint? refreshedWindowRemainingMs = null;
    const uint refreshedWindowMinMillis = 55_000;
    StatusSnapshot? latest = null;
    var deadline = DateTimeOffset.UtcNow + TimeSpan.FromSeconds(watchSeconds);
    var index = 0;
    var connectionAttempt = 1;
    var opened = await OpenStatusConnectionAsync(
        address,
        addressType,
        serviceUuid,
        statusUuid,
        "CLEAR_GESTURE",
        connectionAttempt,
        printPairing: false,
        dumpGattWhenServiceMissing: false);
    if (opened.Connection is null)
    {
        return opened.FailureCode;
    }

    var connection = opened.Connection;
    try
    {
        while (DateTimeOffset.UtcNow < deadline)
        {
            index++;
            var maybeSnapshot = await ReadStatusSnapshotRecoverableAsync(connection.Status, $"CLEAR_GESTURE_{index}");
            if (maybeSnapshot is null)
            {
                connection.Dispose();
                connectionAttempt++;
                Console.WriteLine(
                    $"CLEAR_GESTURE_RECONNECT reason=status_read_failed next_attempt={connectionAttempt}");
                var reopened = await OpenStatusConnectionAsync(
                    address,
                    addressType,
                    serviceUuid,
                    statusUuid,
                    "CLEAR_GESTURE",
                    connectionAttempt,
                    printPairing: false,
                    dumpGattWhenServiceMissing: false);
                if (reopened.Connection is null)
                {
                    return 5;
                }
                connection = reopened.Connection;
                await Task.Delay(TimeSpan.FromMilliseconds(250));
                continue;
            }
            var snapshot = maybeSnapshot.Value;
            latest = snapshot;
            var bootReleased = snapshot.BootButton == 0;
            var bootPressed = snapshot.BootButton == 1;
            var pressedMs = snapshot.BootPressedMs ?? 0;
            var pairingOpen = snapshot.Pairing == 1;
            var pairingRemainingMs = snapshot.PairingRemainingMs ?? 0;

            if (bootReleased)
            {
                if (!observedReleased)
                {
                    Console.WriteLine($"CLEAR_GESTURE_RELEASED index={index}");
                    releasedIndex = index;
                }
                observedReleased = true;
                if (observedHoldThreshold)
                {
                    if (!observedReleasedAfterHold)
                    {
                        releasedAfterHoldIndex = index;
                    }
                    observedReleasedAfterHold = true;
                }
            }
            if (observedReleased && bootPressed)
            {
                if (!observedPressedAfterRelease)
                {
                    Console.WriteLine($"CLEAR_GESTURE_PRESSED_AFTER_RELEASE index={index}");
                    pressedAfterReleaseIndex = index;
                }
                observedPressedAfterRelease = true;
            }
            if (observedPressedAfterRelease && pressedMs >= holdMillis)
            {
                if (!observedHoldThreshold)
                {
                    Console.WriteLine(
                        $"CLEAR_GESTURE_HOLD_THRESHOLD index={index} pressed_ms={pressedMs}");
                    holdThresholdIndex = index;
                    holdThresholdPressedMs = pressedMs;
                }
                observedHoldThreshold = true;
            }
            if (observedHoldThreshold && pairingOpen && pairingRemainingMs >= refreshedWindowMinMillis)
            {
                if (!observedRefreshedWindow)
                {
                    Console.WriteLine(
                        $"CLEAR_GESTURE_WINDOW_REFRESHED index={index} remaining_ms={pairingRemainingMs} min_ms={refreshedWindowMinMillis}");
                    refreshedWindowIndex = index;
                    refreshedWindowRemainingMs = pairingRemainingMs;
                }
                observedRefreshedWindow = true;
            }

            if (observedHoldThreshold && observedRefreshedWindow && !reportedClearEffectObserved)
            {
                Console.WriteLine(
                    $"CLEAR_GESTURE_CLEAR_EFFECT_OBSERVED hold_index={holdThresholdIndex} hold_pressed_ms={holdThresholdPressedMs} refreshed_index={refreshedWindowIndex} refreshed_remaining_ms={refreshedWindowRemainingMs} waiting_for_release={!observedReleasedAfterHold}");
                reportedClearEffectObserved = true;
            }

            if (observedHoldThreshold && observedRefreshedWindow && observedReleasedAfterHold)
            {
                Console.WriteLine(
                    $"CLEAR_GESTURE_RESULT success=True released_before_press={observedReleased} released_index={releasedIndex} pressed_after_release={observedPressedAfterRelease} pressed_index={pressedAfterReleaseIndex} hold_threshold={observedHoldThreshold} hold_index={holdThresholdIndex} hold_pressed_ms={holdThresholdPressedMs} refreshed_window={observedRefreshedWindow} refreshed_index={refreshedWindowIndex} refreshed_remaining_ms={refreshedWindowRemainingMs} released_after_hold={observedReleasedAfterHold} release_after_hold_index={releasedAfterHoldIndex}");
                return 0;
            }

            await Task.Delay(TimeSpan.FromSeconds(1));
        }
    }
    finally
    {
        connection.Dispose();
    }

    if (observedHoldThreshold && observedRefreshedWindow && !observedReleasedAfterHold)
    {
        Console.WriteLine(
            $"CLEAR_GESTURE_RELEASE_DIAGNOSTIC_MISSING released_before_press={observedReleased} released_index={releasedIndex} pressed_after_release={observedPressedAfterRelease} pressed_index={pressedAfterReleaseIndex} hold_index={holdThresholdIndex} hold_pressed_ms={holdThresholdPressedMs} refreshed_index={refreshedWindowIndex} refreshed_remaining_ms={refreshedWindowRemainingMs} latest_pairing={DecodeNullablePairing(latest?.Pairing)} latest_boot={DecodeNullableBootButton(latest?.BootButton)} latest_pressed_ms={latest?.BootPressedMs} latest_remaining_ms={latest?.PairingRemainingMs}");
    }
    Console.WriteLine(
        $"CLEAR_GESTURE_RESULT success=False released_before_press={observedReleased} released_index={releasedIndex} pressed_after_release={observedPressedAfterRelease} pressed_index={pressedAfterReleaseIndex} hold_threshold={observedHoldThreshold} hold_index={holdThresholdIndex} hold_pressed_ms={holdThresholdPressedMs} refreshed_window={observedRefreshedWindow} refreshed_index={refreshedWindowIndex} refreshed_remaining_ms={refreshedWindowRemainingMs} released_after_hold={observedReleasedAfterHold} release_after_hold_index={releasedAfterHoldIndex} latest_pairing={DecodeNullablePairing(latest?.Pairing)} latest_boot={DecodeNullableBootButton(latest?.BootButton)} latest_pressed_ms={latest?.BootPressedMs} latest_remaining_ms={latest?.PairingRemainingMs}");
    return 6;
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
    PrintPairingState(device, "CLOSED_WINDOW");

    var servicesResult = await GetGattServicesForUuidWithRetryAsync(device, serviceUuid, "");
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

    var metadataRejected = IsProtectedReadStatusRejected(metadataRead.Status, metadataRead.ProtocolError);
    var fragmentRejected = IsProtectedReadStatusRejected(fragmentRead.Status, fragmentRead.ProtocolError);
    var controlRejected = IsProtectedWriteRejected(controlWrite, controlExceptionHResult);

    Console.WriteLine(
        $"CLOSED_WINDOW_RESULT metadata_rejected={metadataRejected} fragment_rejected={fragmentRejected} control_rejected={controlRejected}");

    return metadataRejected && fragmentRejected && controlRejected ? 0 : 8;
}

static async Task<int> ScanThenReadMetadataNowAsync(
    int seconds,
    string targetName,
    string expectation,
    string pairMode,
    Guid advertisementUuid,
    Guid serviceUuid,
    Guid statusUuid,
    Guid metadataUuid,
    Guid controlUuid)
{
    var expectSuccess = string.Equals(expectation, "expect-success", StringComparison.OrdinalIgnoreCase);
    var expectReject = string.Equals(expectation, "expect-reject", StringComparison.OrdinalIgnoreCase);
    var allowPair = string.Equals(pairMode, "auto-pair", StringComparison.OrdinalIgnoreCase);
    var skipPair = string.Equals(pairMode, "no-pair", StringComparison.OrdinalIgnoreCase);
    if (!expectSuccess && !expectReject)
    {
        Console.Error.WriteLine(
            "usage: scan-read-metadata-now [scan-seconds] [name] [expect-success|expect-reject] [auto-pair|no-pair]");
        return 64;
    }
    if (!allowPair && !skipPair)
    {
        Console.Error.WriteLine(
            "usage: scan-read-metadata-now [scan-seconds] [name] [expect-success|expect-reject] [auto-pair|no-pair]");
        return 64;
    }

    var found = await ScanForTargetAsync(seconds, targetName, advertisementUuid, serviceUuid);
    if (found is null)
    {
        return 2;
    }

    await Task.Delay(TimeSpan.FromMilliseconds(300));
    return await ReadMetadataNowAsync(
        found.Address,
        found.AddressType,
        expectSuccess,
        allowPair,
        serviceUuid,
        statusUuid,
        metadataUuid,
        controlUuid);
}

static async Task<int> ReadMetadataNowAsync(
    ulong address,
    BluetoothAddressType addressType,
    bool expectSuccess,
    bool allowPair,
    Guid serviceUuid,
    Guid statusUuid,
    Guid metadataUuid,
    Guid controlUuid)
{
    const string label = "METADATA_NOW";
    Console.WriteLine(
        $"{label}_CONNECT address={FormatAddress(address)} address_type={addressType} expect_success={expectSuccess} allow_pair={allowPair}");
    using var device = await BluetoothLEDevice.FromBluetoothAddressAsync(address, addressType);
    if (device is null)
    {
        Console.WriteLine($"{label}_DEVICE_NOT_FOUND");
        return 2;
    }
    PrintPairingState(device, label);
    if (expectSuccess && allowPair && !await EnsurePairedAsync(device, label))
    {
        Console.WriteLine(
            $"{label}_RESULT success=False metadata_success=False rejected=False phase=pair");
        return 5;
    }

    var servicesResult = await GetGattServicesForUuidWithRetryAsync(device, serviceUuid, label);
    if (servicesResult.Status != GattCommunicationStatus.Success || servicesResult.Services.Count == 0)
    {
        return 3;
    }

    using var service = servicesResult.Services[0];
    var status = await GetCharacteristicAsync(service, statusUuid, "status");
    var metadata = await GetCharacteristicAsync(service, metadataUuid, "metadata");
    var control = await GetCharacteristicAsync(service, controlUuid, "control");
    if (status is null || metadata is null || control is null)
    {
        return 4;
    }

    _ = await ReadStatusSnapshotAsync(status, $"{label}_INITIAL");

    var requestMetadata = EncodeControlFrame(1, 0, 0, 0);
    var writeSucceeded = await WriteControlAsync(control, $"{label}_REQUEST_METADATA", requestMetadata);
    if (!writeSucceeded)
    {
        Console.WriteLine(
            $"{label}_RESULT success={!expectSuccess} metadata_success=False rejected=True phase=control_write");
        return expectSuccess ? 5 : 0;
    }

    var metadataRead = await ReadProtectedCharacteristicAsync(metadata, $"{label}_METADATA");
    var recordMetadata = default(RecordMetadata);
    var metadataSucceeded = metadataRead.Status == GattCommunicationStatus.Success &&
        metadataRead.Bytes is { } metadataBytes &&
        TryDecodeMetadata(metadataBytes, out recordMetadata);
    var rejected = !metadataSucceeded && IsProtectedReadResultRejected(metadataRead);

    if (metadataSucceeded)
    {
        Console.WriteLine(
            $"{label}_METADATA_DECODED version={recordMetadata.Version} sequence={recordMetadata.Sequence} payload_len={recordMetadata.PayloadLength} flags=0x{recordMetadata.Flags:x2} crc32=0x{recordMetadata.Crc32:x8} current_boot={recordMetadata.CurrentBoot}");
    }

    var accepted = expectSuccess ? metadataSucceeded : rejected;
    Console.WriteLine(
        $"{label}_RESULT success={accepted} metadata_success={metadataSucceeded} rejected={rejected} expect_success={expectSuccess}");
    return accepted ? 0 : 6;
}

static async Task<GattCharacteristic?> GetCharacteristicAsync(
    GattDeviceService service,
    Guid uuid,
    string name)
{
    var result = await GetCharacteristicsForUuidWithRetryAsync(
        service,
        uuid,
        $"CHAR_LOOKUP name={name} uuid={uuid}");
    return result.Status == GattCommunicationStatus.Success && result.Characteristics.Count > 0
        ? result.Characteristics[0]
        : null;
}

static async Task<GattCharacteristicsResult> GetCharacteristicsForUuidWithRetryAsync(
    GattDeviceService service,
    Guid uuid,
    string label)
{
    const int maxAttempts = 5;
    GattCharacteristicsResult? latest = null;

    for (var attempt = 1; ; attempt++)
    {
        var uncached = await service.GetCharacteristicsForUuidAsync(
            uuid,
            BluetoothCacheMode.Uncached);
        latest = uncached;
        Console.WriteLine(
            $"{label} status={uncached.Status} count={uncached.Characteristics.Count} protocol_error=0x{uncached.ProtocolError:x} attempt={attempt} cache=Uncached");

        if (uncached.Status == GattCommunicationStatus.Success && uncached.Characteristics.Count > 0)
        {
            return uncached;
        }

        var cached = await service.GetCharacteristicsForUuidAsync(
            uuid,
            BluetoothCacheMode.Cached);
        latest = cached;
        Console.WriteLine(
            $"{label} status={cached.Status} count={cached.Characteristics.Count} protocol_error=0x{cached.ProtocolError:x} attempt={attempt} cache=Cached");

        if (cached.Status == GattCommunicationStatus.Success && cached.Characteristics.Count > 0)
        {
            return cached;
        }

        if (attempt >= maxAttempts)
        {
            return latest;
        }

        await Task.Delay(TimeSpan.FromMilliseconds(250 * attempt));
    }
}

static async Task<GattDeviceServicesResult> GetGattServicesForUuidWithRetryAsync(
    BluetoothLEDevice device,
    Guid serviceUuid,
    string label)
{
    const int maxAttempts = 5;
    var outputLabel = string.IsNullOrEmpty(label) ? "SERVICES" : $"{label}_SERVICES";
    GattDeviceServicesResult? latest = null;

    for (var attempt = 1; ; attempt++)
    {
        var uncached = await device.GetGattServicesForUuidAsync(
            serviceUuid,
            BluetoothCacheMode.Uncached);
        latest = uncached;
        Console.WriteLine(
            $"{outputLabel} status={uncached.Status} protocol_error=0x{uncached.ProtocolError:x} attempt={attempt} cache=Uncached");

        if (uncached.Status == GattCommunicationStatus.Success && uncached.Services.Count > 0)
        {
            return uncached;
        }

        var cached = await device.GetGattServicesForUuidAsync(
            serviceUuid,
            BluetoothCacheMode.Cached);
        latest = cached;
        Console.WriteLine(
            $"{outputLabel} status={cached.Status} protocol_error=0x{cached.ProtocolError:x} attempt={attempt} cache=Cached");

        if (cached.Status == GattCommunicationStatus.Success && cached.Services.Count > 0)
        {
            return cached;
        }

        if (attempt >= maxAttempts)
        {
            return latest;
        }

        await Task.Delay(TimeSpan.FromMilliseconds(250 * attempt));
    }
}

static async Task<StatusConnectionOpenResult> OpenStatusConnectionAsync(
    ulong address,
    BluetoothAddressType addressType,
    Guid serviceUuid,
    Guid statusUuid,
    string label,
    int attempt,
    bool printPairing,
    bool dumpGattWhenServiceMissing)
{
    var outputLabel = string.IsNullOrEmpty(label) ? "STATUS" : label;
    Console.WriteLine(
        $"{outputLabel}_CONNECT attempt={attempt} address={FormatAddress(address)} address_type={addressType} service={serviceUuid} status={statusUuid}");

    var device = await BluetoothLEDevice.FromBluetoothAddressAsync(address, addressType);
    if (device is null)
    {
        Console.WriteLine($"{outputLabel}_DEVICE_NOT_FOUND attempt={attempt}");
        return new StatusConnectionOpenResult(null, 2);
    }

    Console.WriteLine(
        $"{outputLabel}_DEVICE name={device.Name} connection_status={device.ConnectionStatus} attempt={attempt}");
    if (printPairing)
    {
        PrintPairingState(device, outputLabel);
    }

    var servicesResult = await GetGattServicesForUuidWithRetryAsync(device, serviceUuid, outputLabel);
    if (servicesResult.Status != GattCommunicationStatus.Success)
    {
        device.Dispose();
        return new StatusConnectionOpenResult(null, 3);
    }
    if (servicesResult.Services.Count == 0)
    {
        Console.WriteLine($"{outputLabel}_SERVICE_NOT_FOUND attempt={attempt}");
        if (dumpGattWhenServiceMissing)
        {
            await DumpGattAsync(device);
        }
        device.Dispose();
        return new StatusConnectionOpenResult(null, 4);
    }

    var service = servicesResult.Services[0];
    var status = await GetCharacteristicAsync(service, statusUuid, "status");
    if (status is null)
    {
        service.Dispose();
        device.Dispose();
        return new StatusConnectionOpenResult(null, 6);
    }

    return new StatusConnectionOpenResult(
        new StatusConnection(device, service, status),
        0);
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

static async Task<int> ScanThenTransferRecordNowAsync(
    int seconds,
    string targetName,
    string ackMode,
    ushort fragmentLength,
    string pairMode,
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
    return await TransferRecordNowAsync(
        found.Address,
        found.AddressType,
        ackMode,
        fragmentLength,
        pairMode,
        serviceUuid,
        statusUuid,
        metadataUuid,
        fragmentUuid,
        controlUuid);
}

static async Task<int> ScanThenAckThenPeekNextAsync(
    int seconds,
    string targetName,
    ushort fragmentLength,
    Guid advertisementUuid,
    Guid serviceUuid,
    Guid statusUuid,
    Guid metadataUuid,
    Guid fragmentUuid,
    Guid controlUuid)
{
    if (fragmentLength == 0)
    {
        Console.Error.WriteLine("fragment length must be non-zero");
        return 64;
    }

    var found = await ScanForTargetAsync(seconds, targetName, advertisementUuid, serviceUuid);
    if (found is null)
    {
        return 2;
    }

    await Task.Delay(TimeSpan.FromMilliseconds(300));
    var transfer = await TransferRecordDetailedAsync(
        found.Address,
        found.AddressType,
        true,
        fragmentLength,
        false,
        serviceUuid,
        statusUuid,
        metadataUuid,
        fragmentUuid,
        controlUuid,
        "ACK_THEN_PEEK_TRANSFER");
    if (transfer is null)
    {
        return 3;
    }

    await Task.Delay(TimeSpan.FromMilliseconds(800));
    var next = await ReadAuthorizedMetadataOnlyAsync(
        found.Address,
        found.AddressType,
        serviceUuid,
        statusUuid,
        metadataUuid,
        fragmentUuid,
        controlUuid,
        "POST_ACK_NEXT");
    if (next is null)
    {
        Console.WriteLine(
            $"ACK_THEN_PEEK_NEXT_RESULT success=False acked_sequence={transfer.Value.Metadata.Sequence} reason=no_next_metadata");
        return 4;
    }

    var advanced = next.Value.Sequence != transfer.Value.Metadata.Sequence;
    Console.WriteLine(
        $"ACK_THEN_PEEK_NEXT_RESULT success={advanced} acked_sequence={transfer.Value.Metadata.Sequence} next_sequence={next.Value.Sequence} next_payload_len={next.Value.PayloadLength}");
    return advanced ? 0 : 5;
}

static async Task<int> ScanThenTransferRecordWithNotificationsAsync(
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
    var shouldAck = string.Equals(ackMode, "ack", StringComparison.OrdinalIgnoreCase);
    if (!shouldAck && !string.Equals(ackMode, "no-ack", StringComparison.OrdinalIgnoreCase))
    {
        Console.Error.WriteLine(
            "usage: scan-transfer-record-notify [scan-seconds] [name] [no-ack|ack] [fragment-len]");
        return 64;
    }
    if (fragmentLength == 0)
    {
        Console.Error.WriteLine("fragment length must be non-zero");
        return 64;
    }

    var found = await ScanForTargetAsync(seconds, targetName, advertisementUuid, serviceUuid);
    if (found is null)
    {
        return 2;
    }

    await Task.Delay(TimeSpan.FromMilliseconds(300));
    var transfer = await TransferRecordDetailedAsync(
        found.Address,
        found.AddressType,
        shouldAck,
        fragmentLength,
        true,
        serviceUuid,
        statusUuid,
        metadataUuid,
        fragmentUuid,
        controlUuid,
        "NOTIFY_TRANSFER");
    return transfer is null ? 3 : 0;
}

static async Task<int> ScanThenCheckDisconnectPreservesRecordAsync(
    int seconds,
    string targetName,
    ushort fragmentLength,
    Guid advertisementUuid,
    Guid serviceUuid,
    Guid statusUuid,
    Guid metadataUuid,
    Guid fragmentUuid,
    Guid controlUuid)
{
    if (fragmentLength == 0)
    {
        Console.Error.WriteLine("fragment length must be non-zero");
        return 64;
    }

    var found = await ScanForTargetAsync(seconds, targetName, advertisementUuid, serviceUuid);
    if (found is null)
    {
        return 2;
    }

    await Task.Delay(TimeSpan.FromMilliseconds(300));
    var first = await ReadPartialRecordThenDisconnectAsync(
        found.Address,
        found.AddressType,
        fragmentLength,
        serviceUuid,
        statusUuid,
        metadataUuid,
        fragmentUuid,
        controlUuid);
    if (first is null)
    {
        return 3;
    }

    await Task.Delay(TimeSpan.FromMilliseconds(1_200));
    var second = await ReadAuthorizedMetadataOnlyAsync(
        found.Address,
        found.AddressType,
        serviceUuid,
        statusUuid,
        metadataUuid,
        fragmentUuid,
        controlUuid,
        "DISCONNECT_RECONNECT");
    if (second is null)
    {
        Console.WriteLine(
            $"DISCONNECT_PRESERVE_RESULT success=False first_sequence={first.Value.Sequence} reason=no_reconnect_metadata");
        return 4;
    }

    var preserved = first.Value.Sequence == second.Value.Sequence;
    Console.WriteLine(
        $"DISCONNECT_PRESERVE_RESULT success={preserved} first_sequence={first.Value.Sequence} second_sequence={second.Value.Sequence} first_payload_len={first.Value.PayloadLength} second_payload_len={second.Value.PayloadLength}");
    return preserved ? 0 : 5;
}

static async Task<int> ScanThenDrainThenCheckDisconnectPreservesRecordAsync(
    int seconds,
    string targetName,
    ushort fragmentLength,
    int targetPending,
    int maxDrain,
    int statusEvery,
    Guid advertisementUuid,
    Guid serviceUuid,
    Guid statusUuid,
    Guid metadataUuid,
    Guid fragmentUuid,
    Guid controlUuid)
{
    if (fragmentLength == 0)
    {
        Console.Error.WriteLine("fragment length must be non-zero");
        return 64;
    }
    if (targetPending < 0 || targetPending > ushort.MaxValue)
    {
        Console.Error.WriteLine("target pending must be between 0 and 65535");
        return 64;
    }
    if (maxDrain < 1)
    {
        Console.Error.WriteLine("max drain must be at least one");
        return 64;
    }
    if (statusEvery < 1)
    {
        Console.Error.WriteLine("status every must be at least one");
        return 64;
    }

    var found = await ScanForTargetAsync(seconds, targetName, advertisementUuid, serviceUuid);
    if (found is null)
    {
        return 2;
    }

    await Task.Delay(TimeSpan.FromMilliseconds(300));
    var drain = await DrainRecordsBeforeDisconnectAsync(
        found.Address,
        found.AddressType,
        fragmentLength,
        targetPending,
        maxDrain,
        statusEvery,
        serviceUuid,
        statusUuid,
        metadataUuid,
        fragmentUuid,
        controlUuid);
    if (drain is null)
    {
        Console.WriteLine("DRAIN_THEN_DISCONNECT_RESULT success=False phase=drain");
        return 3;
    }

    var drained = drain.Value.Sequences;
    Console.WriteLine(
        $"DRAIN_BEFORE_DISCONNECT_RESULT drained={drained.Count} target_pending={targetPending} final_pending={drain.Value.FinalStatus?.Pending} sequences={string.Join(",", drained)}");
    if (drain.Value.FinalStatus is null || drain.Value.FinalStatus.Value.Pending > targetPending)
    {
        Console.WriteLine(
            $"DRAIN_THEN_DISCONNECT_RESULT success=False phase=drain_threshold drained={drained.Count} target_pending={targetPending} final_pending={drain.Value.FinalStatus?.Pending}");
        return 4;
    }

    var first = await ReadPartialRecordThenDisconnectAsync(
        found.Address,
        found.AddressType,
        fragmentLength,
        serviceUuid,
        statusUuid,
        metadataUuid,
        fragmentUuid,
        controlUuid);
    if (first is null)
    {
        Console.WriteLine(
            $"DRAIN_THEN_DISCONNECT_RESULT success=False phase=partial drained={drained.Count}");
        return 5;
    }

    await Task.Delay(TimeSpan.FromMilliseconds(1_200));
    var second = await ReadAuthorizedMetadataOnlyAsync(
        found.Address,
        found.AddressType,
        serviceUuid,
        statusUuid,
        metadataUuid,
        fragmentUuid,
        controlUuid,
        "DRAIN_DISCONNECT_RECONNECT");
    if (second is null)
    {
        Console.WriteLine(
            $"DRAIN_THEN_DISCONNECT_RESULT success=False phase=reconnect drained={drained.Count} first_sequence={first.Value.Sequence}");
        return 6;
    }

    var preserved = first.Value.Sequence == second.Value.Sequence;
    Console.WriteLine(
        $"DRAIN_THEN_DISCONNECT_RESULT success={preserved} drained={drained.Count} target_pending={targetPending} final_pending={drain.Value.FinalStatus.Value.Pending} first_sequence={first.Value.Sequence} second_sequence={second.Value.Sequence} first_payload_len={first.Value.PayloadLength} second_payload_len={second.Value.PayloadLength}");
    return preserved ? 0 : 7;
}

static async Task<DrainRecordsResult?> DrainRecordsBeforeDisconnectAsync(
    ulong address,
    BluetoothAddressType addressType,
    ushort fragmentLength,
    int targetPending,
    int maxDrain,
    int statusEvery,
    Guid serviceUuid,
    Guid statusUuid,
    Guid metadataUuid,
    Guid fragmentUuid,
    Guid controlUuid)
{
    const string label = "DRAIN_BEFORE_DISCONNECT";
    Console.WriteLine(
        $"{label}_CONNECT address={FormatAddress(address)} address_type={addressType} target_pending={targetPending} max_drain={maxDrain} status_every={statusEvery}");
    using var device = await BluetoothLEDevice.FromBluetoothAddressAsync(address, addressType);
    if (device is null)
    {
        Console.WriteLine($"{label}_DEVICE_NOT_FOUND");
        return null;
    }

    var servicesResult = await GetGattServicesForUuidWithRetryAsync(device, serviceUuid, label);
    if (servicesResult.Status != GattCommunicationStatus.Success || servicesResult.Services.Count == 0)
    {
        return null;
    }

    using var service = servicesResult.Services[0];
    var status = await GetCharacteristicAsync(service, statusUuid, "status");
    var metadata = await GetCharacteristicAsync(service, metadataUuid, "metadata");
    var fragment = await GetCharacteristicAsync(service, fragmentUuid, "fragment");
    var control = await GetCharacteristicAsync(service, controlUuid, "control");
    if (status is null || metadata is null || fragment is null || control is null)
    {
        return null;
    }
    if (!await WaitForPairingOpenAsync(status, TimeSpan.FromSeconds(90)))
    {
        return null;
    }

    var initial = await ReadStatusSnapshotAsync(status, $"{label}_INITIAL");
    var latest = initial;
    var drained = new List<ulong>();
    while (drained.Count < maxDrain)
    {
        if (latest is not null && latest.Value.Pending <= targetPending)
        {
            break;
        }

        var transfer = await TransferRecordOnOpenConnectionAsync(
            metadata,
            fragment,
            control,
            true,
            fragmentLength,
            false,
            $"{label}_{drained.Count + 1}");
        if (transfer is null)
        {
            return null;
        }

        drained.Add(transfer.Value.Metadata.Sequence);
        if (drained.Count % statusEvery == 0 || drained.Count == maxDrain)
        {
            latest = await ReadStatusSnapshotAsync(status, $"{label}_STATUS_AFTER_{drained.Count}");
        }
    }

    return new DrainRecordsResult(drained, initial, latest);
}

static async Task<TransferRecordResult?> TransferRecordOnOpenConnectionAsync(
    GattCharacteristic metadata,
    GattCharacteristic fragment,
    GattCharacteristic control,
    bool shouldAck,
    ushort fragmentLength,
    bool verbose,
    string label)
{
    var recordMetadata = await RequestAndReadMetadataAsync(metadata, control, $"{label}_METADATA");
    if (recordMetadata is null)
    {
        return null;
    }

    var payload = new byte[recordMetadata.Value.PayloadLength];
    var offset = 0;
    while (offset < payload.Length)
    {
        var requested = (ushort)Math.Min(fragmentLength, payload.Length - offset);
        var requestFragment = EncodeControlFrame(
            2,
            recordMetadata.Value.Sequence,
            (ushort)offset,
            requested);
        if (!await WriteControlAsync(control, $"{label}_REQUEST_FRAGMENT", requestFragment))
        {
            return null;
        }

        var fragmentRead = await fragment.ReadValueAsync(BluetoothCacheMode.Uncached);
        if (verbose)
        {
            Console.WriteLine(
                $"{label}_FRAGMENT_READ status={fragmentRead.Status} protocol_error=0x{fragmentRead.ProtocolError:x} requested_offset={offset} requested_len={requested}");
        }
        if (fragmentRead.Status != GattCommunicationStatus.Success)
        {
            return null;
        }

        var fragmentBytes = BufferToBytes(fragmentRead.Value);
        if (!TryDecodeFragment(fragmentBytes, out var recordFragment))
        {
            return null;
        }

        if (verbose)
        {
            Console.WriteLine(
                $"{label}_FRAGMENT_DECODED version={recordFragment.Version} sequence={recordFragment.Sequence} offset={recordFragment.Offset} payload_len={recordFragment.Payload.Length} first_payload_hex={Convert.ToHexString(recordFragment.Payload.Take(16).ToArray())}");
        }

        if (!ValidateFragment(recordMetadata.Value, recordFragment, offset, requested, payload.Length))
        {
            Console.WriteLine($"{label}_FRAGMENT_VALIDATION_FAILED");
            return null;
        }

        Array.Copy(recordFragment.Payload, 0, payload, offset, recordFragment.Payload.Length);
        offset += recordFragment.Payload.Length;
    }

    var computedCrc = Crc32(payload);
    var crcMatches = computedCrc == recordMetadata.Value.Crc32;
    if (verbose)
    {
        Console.WriteLine(
            $"{label}_PAYLOAD_RESULT len={payload.Length} crc32=0x{computedCrc:x8} crc_matches={crcMatches} utf8_preview={Utf8Preview(payload)}");
    }
    if (!crcMatches)
    {
        return null;
    }

    var completeFrame = EncodeControlFrame(3, recordMetadata.Value.Sequence, 0, 0);
    if (!await WriteControlAsync(control, $"{label}_COMPLETE_RECORD", completeFrame))
    {
        return null;
    }

    if (shouldAck)
    {
        var ackFrame = EncodeControlFrame(4, recordMetadata.Value.Sequence, 0, 0);
        if (!await WriteControlAsync(control, $"{label}_ACK_RECORD", ackFrame))
        {
            return null;
        }
    }

    Console.WriteLine(
        $"{label}_RESULT success=True sequence={recordMetadata.Value.Sequence} payload_len={payload.Length} crc32=0x{computedCrc:x8} ack_requested={shouldAck}");
    return new TransferRecordResult(recordMetadata.Value, payload, computedCrc, shouldAck, 0);
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
    var servicesResult = await GetGattServicesForUuidWithRetryAsync(device, serviceUuid, "");
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

static async Task<int> TransferRecordNowAsync(
    ulong address,
    BluetoothAddressType addressType,
    string ackMode,
    ushort fragmentLength,
    string pairMode,
    Guid serviceUuid,
    Guid statusUuid,
    Guid metadataUuid,
    Guid fragmentUuid,
    Guid controlUuid)
{
    var shouldAck = string.Equals(ackMode, "ack", StringComparison.OrdinalIgnoreCase);
    if (!shouldAck && !string.Equals(ackMode, "no-ack", StringComparison.OrdinalIgnoreCase))
    {
        Console.Error.WriteLine(
            "usage: scan-transfer-record-now [scan-seconds] [name] [no-ack|ack] [fragment-len] [auto-pair|no-pair]");
        return 64;
    }
    var allowPair = string.Equals(pairMode, "auto-pair", StringComparison.OrdinalIgnoreCase);
    var skipPair = string.Equals(pairMode, "no-pair", StringComparison.OrdinalIgnoreCase);
    if (!allowPair && !skipPair)
    {
        Console.Error.WriteLine(
            "usage: scan-transfer-record-now [scan-seconds] [name] [no-ack|ack] [fragment-len] [auto-pair|no-pair]");
        return 64;
    }
    if (fragmentLength == 0)
    {
        Console.Error.WriteLine("fragment length must be non-zero");
        return 64;
    }

    const string label = "TRANSFER_NOW";
    Console.WriteLine(
        $"{label}_CONNECT address={FormatAddress(address)} address_type={addressType} ack_mode={(shouldAck ? "ack" : "no-ack")} fragment_len={fragmentLength} allow_pair={allowPair}");
    using var device = await BluetoothLEDevice.FromBluetoothAddressAsync(address, addressType);
    if (device is null)
    {
        Console.WriteLine($"{label}_DEVICE_NOT_FOUND");
        return 2;
    }

    Console.WriteLine($"{label}_DEVICE name={device.Name} connection_status={device.ConnectionStatus}");
    PrintPairingState(device, label);
    if (allowPair && !await EnsurePairedAsync(device, label))
    {
        Console.WriteLine($"{label}_RESULT success=False phase=pair");
        return 5;
    }

    var servicesResult = await GetGattServicesForUuidWithRetryAsync(device, serviceUuid, label);
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

    _ = await ReadStatusSnapshotAsync(status, $"{label}_INITIAL");
    var transfer = await TransferRecordOnOpenConnectionAsync(
        metadata,
        fragment,
        control,
        shouldAck,
        fragmentLength,
        true,
        label);
    if (transfer is null)
    {
        Console.WriteLine($"{label}_RESULT success=False phase=transfer");
        return 6;
    }

    _ = await ReadStatusSnapshotAsync(status, $"{label}_AFTER");
    Console.WriteLine(
        $"{label}_SUMMARY success=True sequence={transfer.Value.Metadata.Sequence} ack_requested={shouldAck} payload_len={transfer.Value.Metadata.PayloadLength}");
    return 0;
}

static async Task<TransferRecordResult?> TransferRecordDetailedAsync(
    ulong address,
    BluetoothAddressType addressType,
    bool shouldAck,
    ushort fragmentLength,
    bool useNotifications,
    Guid serviceUuid,
    Guid statusUuid,
    Guid metadataUuid,
    Guid fragmentUuid,
    Guid controlUuid,
    string label)
{
    Console.WriteLine(
        $"{label}_CONNECT address={FormatAddress(address)} address_type={addressType} ack={shouldAck} fragment_len={fragmentLength} notifications={useNotifications}");
    using var device = await BluetoothLEDevice.FromBluetoothAddressAsync(address, addressType);
    if (device is null)
    {
        Console.WriteLine($"{label}_DEVICE_NOT_FOUND");
        return null;
    }

    Console.WriteLine($"{label}_DEVICE name={device.Name} connection_status={device.ConnectionStatus}");
    var servicesResult = await GetGattServicesForUuidWithRetryAsync(device, serviceUuid, label);
    if (servicesResult.Status != GattCommunicationStatus.Success || servicesResult.Services.Count == 0)
    {
        return null;
    }

    using var service = servicesResult.Services[0];
    var status = await GetCharacteristicAsync(service, statusUuid, "status");
    var metadata = await GetCharacteristicAsync(service, metadataUuid, "metadata");
    var fragment = await GetCharacteristicAsync(service, fragmentUuid, "fragment");
    var control = await GetCharacteristicAsync(service, controlUuid, "control");
    if (status is null || metadata is null || fragment is null || control is null)
    {
        return null;
    }

    var notifications = new List<RecordFragment>();
    var notificationLock = new object();
    TypedEventHandler<GattCharacteristic, GattValueChangedEventArgs>? notificationHandler = null;
    if (useNotifications)
    {
        notificationHandler = (_, eventArgs) =>
        {
            var bytes = BufferToBytes(eventArgs.CharacteristicValue);
            if (!TryDecodeFragment(bytes, out var notified))
            {
                Console.WriteLine($"{label}_NOTIFICATION_DECODE_FAILED len={bytes.Length}");
                return;
            }

            lock (notificationLock)
            {
                notifications.Add(notified);
            }
            Console.WriteLine(
                $"{label}_NOTIFICATION sequence={notified.Sequence} offset={notified.Offset} payload_len={notified.Payload.Length} first_payload_hex={Convert.ToHexString(notified.Payload.Take(16).ToArray())}");
        };
        fragment.ValueChanged += notificationHandler;
        var subscribe = await fragment.WriteClientCharacteristicConfigurationDescriptorAsync(
            GattClientCharacteristicConfigurationDescriptorValue.Notify);
        Console.WriteLine($"{label}_NOTIFY_SUBSCRIBE status={subscribe}");
        if (subscribe != GattCommunicationStatus.Success)
        {
            fragment.ValueChanged -= notificationHandler;
            return null;
        }
    }

    try
    {
        if (!await WaitForPairingOpenAsync(status, TimeSpan.FromSeconds(90)))
        {
            return null;
        }

        var recordMetadata = await RequestAndReadMetadataAsync(metadata, control, $"{label}_METADATA");
        if (recordMetadata is null)
        {
            return null;
        }

        var payload = new byte[recordMetadata.Value.PayloadLength];
        var offset = 0;
        var notificationMatches = 0;
        while (offset < payload.Length)
        {
            var requested = (ushort)Math.Min(fragmentLength, payload.Length - offset);
            Task<RecordFragment?> notification = useNotifications
                ? WaitForNotificationAsync(
                    notifications,
                    notificationLock,
                    recordMetadata.Value.Sequence,
                    (ushort)offset,
                    TimeSpan.FromSeconds(3),
                    label)
                : Task.FromResult<RecordFragment?>(null);

            var requestFragment = EncodeControlFrame(
                2,
                recordMetadata.Value.Sequence,
                (ushort)offset,
                requested);
            if (!await WriteControlAsync(control, $"{label}_REQUEST_FRAGMENT", requestFragment))
            {
                return null;
            }

            var notified = await notification;
            if (useNotifications)
            {
                if (notified is null)
                {
                    Console.WriteLine(
                        $"{label}_NOTIFICATION_RESULT success=False sequence={recordMetadata.Value.Sequence} offset={offset}");
                    return null;
                }
                notificationMatches++;
            }

            var fragmentRead = await fragment.ReadValueAsync(BluetoothCacheMode.Uncached);
            Console.WriteLine(
                $"{label}_FRAGMENT_READ status={fragmentRead.Status} protocol_error=0x{fragmentRead.ProtocolError:x} requested_offset={offset} requested_len={requested}");
            if (fragmentRead.Status != GattCommunicationStatus.Success)
            {
                return null;
            }

            var fragmentBytes = BufferToBytes(fragmentRead.Value);
            if (!TryDecodeFragment(fragmentBytes, out var recordFragment))
            {
                return null;
            }

            Console.WriteLine(
                $"{label}_FRAGMENT_DECODED version={recordFragment.Version} sequence={recordFragment.Sequence} offset={recordFragment.Offset} payload_len={recordFragment.Payload.Length} first_payload_hex={Convert.ToHexString(recordFragment.Payload.Take(16).ToArray())}");

            if (!ValidateFragment(recordMetadata.Value, recordFragment, offset, requested, payload.Length))
            {
                Console.WriteLine($"{label}_FRAGMENT_VALIDATION_FAILED");
                return null;
            }
            if (notified is not null && !FragmentsMatch(recordFragment, notified.Value))
            {
                Console.WriteLine($"{label}_NOTIFICATION_MISMATCH offset={offset}");
                return null;
            }

            Array.Copy(recordFragment.Payload, 0, payload, offset, recordFragment.Payload.Length);
            offset += recordFragment.Payload.Length;
        }

        var computedCrc = Crc32(payload);
        var crcMatches = computedCrc == recordMetadata.Value.Crc32;
        Console.WriteLine(
            $"{label}_PAYLOAD_RESULT len={payload.Length} crc32=0x{computedCrc:x8} crc_matches={crcMatches} utf8_preview={Utf8Preview(payload)}");
        if (!crcMatches)
        {
            return null;
        }

        var completeFrame = EncodeControlFrame(3, recordMetadata.Value.Sequence, 0, 0);
        if (!await WriteControlAsync(control, $"{label}_COMPLETE_RECORD", completeFrame))
        {
            return null;
        }

        if (shouldAck)
        {
            var ackFrame = EncodeControlFrame(4, recordMetadata.Value.Sequence, 0, 0);
            if (!await WriteControlAsync(control, $"{label}_ACK_RECORD", ackFrame))
            {
                return null;
            }
        }

        if (useNotifications)
        {
            Console.WriteLine(
                $"{label}_NOTIFICATION_RESULT success=True sequence={recordMetadata.Value.Sequence} notifications={notificationMatches}");
        }
        Console.WriteLine(
            $"{label}_RESULT success=True sequence={recordMetadata.Value.Sequence} payload_len={payload.Length} crc32=0x{computedCrc:x8} ack_requested={shouldAck}");
        return new TransferRecordResult(
            recordMetadata.Value,
            payload,
            computedCrc,
            shouldAck,
            notificationMatches);
    }
    finally
    {
        if (notificationHandler is not null)
        {
            fragment.ValueChanged -= notificationHandler;
            var unsubscribe = await fragment.WriteClientCharacteristicConfigurationDescriptorAsync(
                GattClientCharacteristicConfigurationDescriptorValue.None);
            Console.WriteLine($"{label}_NOTIFY_UNSUBSCRIBE status={unsubscribe}");
        }
    }
}

static async Task<RecordMetadata?> ReadAuthorizedMetadataOnlyAsync(
    ulong address,
    BluetoothAddressType addressType,
    Guid serviceUuid,
    Guid statusUuid,
    Guid metadataUuid,
    Guid fragmentUuid,
    Guid controlUuid,
    string label)
{
    Console.WriteLine($"{label}_CONNECT address={FormatAddress(address)} address_type={addressType}");
    using var device = await BluetoothLEDevice.FromBluetoothAddressAsync(address, addressType);
    if (device is null)
    {
        Console.WriteLine($"{label}_DEVICE_NOT_FOUND");
        return null;
    }

    var servicesResult = await GetGattServicesForUuidWithRetryAsync(device, serviceUuid, label);
    if (servicesResult.Status != GattCommunicationStatus.Success || servicesResult.Services.Count == 0)
    {
        return null;
    }

    using var service = servicesResult.Services[0];
    var status = await GetCharacteristicAsync(service, statusUuid, "status");
    var metadata = await GetCharacteristicAsync(service, metadataUuid, "metadata");
    _ = await GetCharacteristicAsync(service, fragmentUuid, "fragment");
    var control = await GetCharacteristicAsync(service, controlUuid, "control");
    if (status is null || metadata is null || control is null)
    {
        return null;
    }
    if (!await WaitForPairingOpenAsync(status, TimeSpan.FromSeconds(20)))
    {
        return null;
    }

    var recordMetadata = await RequestAndReadMetadataAsync(metadata, control, label);
    if (recordMetadata is not null)
    {
        Console.WriteLine(
            $"{label}_RESULT success=True sequence={recordMetadata.Value.Sequence} payload_len={recordMetadata.Value.PayloadLength} crc32=0x{recordMetadata.Value.Crc32:x8}");
    }
    return recordMetadata;
}

static async Task<RecordMetadata?> ReadPartialRecordThenDisconnectAsync(
    ulong address,
    BluetoothAddressType addressType,
    ushort fragmentLength,
    Guid serviceUuid,
    Guid statusUuid,
    Guid metadataUuid,
    Guid fragmentUuid,
    Guid controlUuid)
{
    const string label = "DISCONNECT_PARTIAL";
    Console.WriteLine($"{label}_CONNECT address={FormatAddress(address)} address_type={addressType}");
    using var device = await BluetoothLEDevice.FromBluetoothAddressAsync(address, addressType);
    if (device is null)
    {
        Console.WriteLine($"{label}_DEVICE_NOT_FOUND");
        return null;
    }

    var servicesResult = await GetGattServicesForUuidWithRetryAsync(device, serviceUuid, label);
    if (servicesResult.Status != GattCommunicationStatus.Success || servicesResult.Services.Count == 0)
    {
        return null;
    }

    using var service = servicesResult.Services[0];
    var status = await GetCharacteristicAsync(service, statusUuid, "status");
    var metadata = await GetCharacteristicAsync(service, metadataUuid, "metadata");
    var fragment = await GetCharacteristicAsync(service, fragmentUuid, "fragment");
    var control = await GetCharacteristicAsync(service, controlUuid, "control");
    if (status is null || metadata is null || fragment is null || control is null)
    {
        return null;
    }
    if (!await WaitForPairingOpenAsync(status, TimeSpan.FromSeconds(90)))
    {
        return null;
    }

    var recordMetadata = await RequestAndReadMetadataAsync(metadata, control, label);
    if (recordMetadata is null)
    {
        return null;
    }

    if (recordMetadata.Value.PayloadLength > 0)
    {
        var requested = (ushort)Math.Min(fragmentLength, recordMetadata.Value.PayloadLength);
        var requestFragment = EncodeControlFrame(2, recordMetadata.Value.Sequence, 0, requested);
        if (!await WriteControlAsync(control, $"{label}_REQUEST_FRAGMENT", requestFragment))
        {
            return null;
        }

        var fragmentRead = await fragment.ReadValueAsync(BluetoothCacheMode.Uncached);
        Console.WriteLine(
            $"{label}_FRAGMENT_READ status={fragmentRead.Status} protocol_error=0x{fragmentRead.ProtocolError:x} requested_offset=0 requested_len={requested}");
        if (fragmentRead.Status != GattCommunicationStatus.Success)
        {
            return null;
        }
    }

    Console.WriteLine(
        $"{label}_RESULT success=True sequence={recordMetadata.Value.Sequence} payload_len={recordMetadata.Value.PayloadLength} disconnecting_before_complete=True");
    return recordMetadata;
}

static async Task<RecordMetadata?> RequestAndReadMetadataAsync(
    GattCharacteristic metadata,
    GattCharacteristic control,
    string label)
{
    var requestMetadata = EncodeControlFrame(1, 0, 0, 0);
    if (!await WaitForAuthorizedMetadataRequestAsync(control, requestMetadata, TimeSpan.FromSeconds(15)))
    {
        return null;
    }

    var metadataRead = await metadata.ReadValueAsync(BluetoothCacheMode.Uncached);
    Console.WriteLine(
        $"{label}_METADATA_READ status={metadataRead.Status} protocol_error=0x{metadataRead.ProtocolError:x}");
    if (metadataRead.Status != GattCommunicationStatus.Success)
    {
        return null;
    }

    var metadataBytes = BufferToBytes(metadataRead.Value);
    Console.WriteLine($"{label}_METADATA_BYTES len={metadataBytes.Length} hex={Convert.ToHexString(metadataBytes)}");
    if (!TryDecodeMetadata(metadataBytes, out var recordMetadata))
    {
        return null;
    }

    Console.WriteLine(
        $"{label}_METADATA_DECODED version={recordMetadata.Version} sequence={recordMetadata.Sequence} payload_len={recordMetadata.PayloadLength} flags=0x{recordMetadata.Flags:x2} crc32=0x{recordMetadata.Crc32:x8} current_boot={recordMetadata.CurrentBoot}");
    return recordMetadata;
}

static async Task<RecordFragment?> WaitForNotificationAsync(
    List<RecordFragment> notifications,
    object notificationLock,
    ulong sequence,
    ushort offset,
    TimeSpan timeout,
    string label)
{
    var deadline = DateTimeOffset.UtcNow + timeout;
    while (DateTimeOffset.UtcNow < deadline)
    {
        lock (notificationLock)
        {
            var index = notifications.FindIndex(
                fragment => fragment.Sequence == sequence && fragment.Offset == offset);
            if (index >= 0)
            {
                var fragment = notifications[index];
                notifications.RemoveAt(index);
                return fragment;
            }
        }
        await Task.Delay(TimeSpan.FromMilliseconds(25));
    }

    Console.WriteLine($"{label}_NOTIFICATION_TIMEOUT sequence={sequence} offset={offset}");
    return null;
}

static bool ValidateFragment(
    RecordMetadata metadata,
    RecordFragment fragment,
    int offset,
    ushort requested,
    int payloadLength)
{
    return fragment.Sequence == metadata.Sequence &&
        fragment.Offset == offset &&
        fragment.Payload.Length > 0 &&
        fragment.Payload.Length <= requested &&
        offset + fragment.Payload.Length <= payloadLength;
}

static bool FragmentsMatch(RecordFragment left, RecordFragment right)
{
    return left.Sequence == right.Sequence &&
        left.Offset == right.Offset &&
        left.Payload.SequenceEqual(right.Payload);
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
        var read = await ReadStatusValueWithRetryAsync(status, $"PAIRING_WAIT_READ attempt={attempt}");
        if (read.Status == GattCommunicationStatus.Success)
        {
            var bytes = BufferToBytes(read.Value);
            Console.WriteLine($"PAIRING_WAIT_STATUS_BYTES len={bytes.Length} hex={Convert.ToHexString(bytes)}");
            PrintStatusDecoded(bytes);
            if (!TryDecodeStatus(bytes, out var snapshot))
            {
                await Task.Delay(TimeSpan.FromMilliseconds(500));
                continue;
            }
            if (snapshot.Pairing == 1)
            {
                Console.WriteLine($"PAIRING_OPEN attempt={attempt}");
                return true;
            }
            if (snapshot.Pairing == 0 &&
                snapshot.BootButton == 1 &&
                snapshot.PairingRemainingMs == 0 &&
                snapshot.BootPressedMs is >= 10_000)
            {
                Console.WriteLine(
                    $"PAIRING_HELD_AFTER_EXPIRED pressed_ms={snapshot.BootPressedMs} action=release_boot_io9_before_retry");
                return false;
            }
        }

        await Task.Delay(TimeSpan.FromMilliseconds(500));
    }

    Console.WriteLine($"PAIRING_TIMEOUT attempts={attempt}");
    return false;
}

static async Task<StatusSnapshot?> ReadStatusSnapshotAsync(GattCharacteristic status, string label)
{
    var read = await ReadStatusValueWithRetryAsync(status, $"{label}_STATUS_READ");
    if (read.Status != GattCommunicationStatus.Success)
    {
        return null;
    }

    var bytes = BufferToBytes(read.Value);
    Console.WriteLine($"{label}_STATUS_BYTES len={bytes.Length} hex={Convert.ToHexString(bytes)}");
    PrintStatusDecoded(bytes);
    return TryDecodeStatus(bytes, out var snapshot) ? snapshot : null;
}

static async Task<StatusSnapshot?> ReadStatusSnapshotRecoverableAsync(GattCharacteristic status, string label)
{
    try
    {
        return await ReadStatusSnapshotAsync(status, label);
    }
    catch (Exception error) when (IsRecoverableGattException(error))
    {
        Console.WriteLine($"{label}_STATUS_READ_EXCEPTION type={error.GetType().Name} message={error.Message}");
        return null;
    }
}

static bool IsRecoverableGattException(Exception error) =>
    error is ObjectDisposedException or COMException;

static async Task<GattReadResult> ReadStatusValueWithRetryAsync(
    GattCharacteristic status,
    string label)
{
    const int maxAttempts = 5;

    for (var attempt = 1; ; attempt++)
    {
        var read = await status.ReadValueAsync(BluetoothCacheMode.Uncached);
        Console.WriteLine(
            $"{label} status={read.Status} protocol_error=0x{read.ProtocolError:x} attempt={attempt}");

        if (read.Status == GattCommunicationStatus.Success || attempt >= maxAttempts)
        {
            return read;
        }

        await Task.Delay(TimeSpan.FromMilliseconds(150 * attempt));
    }
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

static void PrintPairingState(BluetoothLEDevice device, string label)
{
    try
    {
        var pairing = device.DeviceInformation.Pairing;
        Console.WriteLine(
            $"{label}_PAIRING is_paired={pairing.IsPaired} can_pair={pairing.CanPair} protection_level={pairing.ProtectionLevel}");
    }
    catch (Exception error)
    {
        Console.WriteLine($"{label}_PAIRING_ERROR type={error.GetType().Name} message={error.Message}");
    }
}

static async Task<bool> EnsurePairedAsync(BluetoothLEDevice device, string label)
{
    try
    {
        var pairing = device.DeviceInformation.Pairing;
        if (pairing.IsPaired)
        {
            Console.WriteLine($"{label}_PAIR already_paired=True");
            return true;
        }
        if (!pairing.CanPair)
        {
            Console.WriteLine($"{label}_PAIR can_pair=False");
            return false;
        }

        if (await TryCustomConfirmOnlyPairAsync(pairing.Custom, label))
        {
            PrintPairingState(device, $"{label}_PAIR_AFTER");
            return true;
        }

        Console.WriteLine($"{label}_PAIR_FALLBACK_REQUEST protection_level=Encryption");
        var result = await pairing.PairAsync(DevicePairingProtectionLevel.Encryption);
        Console.WriteLine(
            $"{label}_PAIR_FALLBACK_RESULT status={result.Status} protection_level={result.ProtectionLevelUsed}");
        PrintPairingState(device, $"{label}_PAIR_AFTER");
        return IsPairedResult(result.Status) || device.DeviceInformation.Pairing.IsPaired;
    }
    catch (Exception error)
    {
        Console.WriteLine($"{label}_PAIR_ERROR type={error.GetType().Name} message={error.Message}");
        return false;
    }
}

static async Task<bool> TryCustomConfirmOnlyPairAsync(
    DeviceInformationCustomPairing customPairing,
    string label)
{
    TypedEventHandler<DeviceInformationCustomPairing, DevicePairingRequestedEventArgs> handler =
        (_, eventArgs) =>
        {
            Console.WriteLine($"{label}_PAIR_REQUESTED kind={eventArgs.PairingKind}");
            if (eventArgs.PairingKind == DevicePairingKinds.ConfirmOnly)
            {
                eventArgs.Accept();
                Console.WriteLine($"{label}_PAIR_REQUEST_ACCEPTED kind={eventArgs.PairingKind}");
            }
        };

    customPairing.PairingRequested += handler;
    try
    {
        Console.WriteLine($"{label}_PAIR_CUSTOM_REQUEST kinds=ConfirmOnly protection_level=Encryption");
        var result = await customPairing.PairAsync(
            DevicePairingKinds.ConfirmOnly,
            DevicePairingProtectionLevel.Encryption);
        Console.WriteLine(
            $"{label}_PAIR_CUSTOM_RESULT status={result.Status} protection_level={result.ProtectionLevelUsed}");
        return IsPairedResult(result.Status);
    }
    catch (Exception error)
    {
        Console.WriteLine($"{label}_PAIR_CUSTOM_ERROR type={error.GetType().Name} message={error.Message}");
        return false;
    }
    finally
    {
        customPairing.PairingRequested -= handler;
    }
}

static bool IsPairedResult(DevicePairingResultStatus status) =>
    status is DevicePairingResultStatus.Paired or DevicePairingResultStatus.AlreadyPaired;

static async Task<ProtectedReadResult> ReadProtectedCharacteristicAsync(
    GattCharacteristic characteristic,
    string label)
{
    var read = await characteristic.ReadValueAsync(BluetoothCacheMode.Uncached);
    Console.WriteLine($"{label}_READ status={read.Status} protocol_error=0x{read.ProtocolError:x}");
    if (read.Status != GattCommunicationStatus.Success)
    {
        return new ProtectedReadResult(read.Status, read.ProtocolError, null);
    }

    var bytes = BufferToBytes(read.Value);
    Console.WriteLine($"{label}_BYTES len={bytes.Length} hex={Convert.ToHexString(bytes)}");
    return new ProtectedReadResult(read.Status, read.ProtocolError, bytes);
}

static bool IsProtectedReadResultRejected(ProtectedReadResult result) =>
    IsProtectedReadStatusRejected(result.Status, result.ProtocolError);

static bool IsProtectedReadStatusRejected(GattCommunicationStatus status, byte? protocolError) =>
    status == GattCommunicationStatus.AccessDenied ||
    (status == GattCommunicationStatus.ProtocolError && IsProtectedAttError(protocolError));

static bool IsProtectedWriteRejected(GattCommunicationStatus? status, int? exceptionHResult) =>
    status == GattCommunicationStatus.AccessDenied ||
    status == GattCommunicationStatus.ProtocolError ||
    exceptionHResult is { } hresult && IsProtectedHResult(hresult);

static bool IsProtectedAttError(byte? protocolError) => protocolError is 0x05 or 0x08 or 0x0f;

static bool IsProtectedHResult(int hresult) =>
    hresult == unchecked((int)0x80650005) ||
    hresult == unchecked((int)0x80650008) ||
    hresult == unchecked((int)0x8065000f);

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
    if (!TryDecodeStatus(bytes, out var status))
    {
        return;
    }

    Console.WriteLine(
        $"STATUS_DECODED version={status.Version} runtime={DecodeRuntime(status.Runtime)} network={DecodeNetwork(status.Network)} upload={DecodeUpload(status.Upload)} pending={status.Pending} error_flags=0x{status.ErrorFlags:x8}");

    if (status.Pairing is null || status.BootButton is null || status.PairingRemainingMs is null)
    {
        return;
    }

    if (status.BootPressedMs is { } bootPressedMs)
    {
        Console.WriteLine(
            $"STATUS_PAIRING pairing={DecodePairing(status.Pairing.Value)} boot_button={DecodeBootButton(status.BootButton.Value)} remaining_ms={status.PairingRemainingMs.Value} pressed_ms={bootPressedMs}");
    }
    else
    {
        Console.WriteLine(
            $"STATUS_PAIRING pairing={DecodePairing(status.Pairing.Value)} boot_button={DecodeBootButton(status.BootButton.Value)} remaining_ms={status.PairingRemainingMs.Value}");
    }
}

static bool TryDecodeStatus(byte[] bytes, out StatusSnapshot status)
{
    status = default;
    if (bytes.Length < 10)
    {
        return false;
    }

    status = new StatusSnapshot(
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        BitConverter.ToUInt16(bytes, 4),
        BitConverter.ToUInt32(bytes, 6),
        bytes.Length >= 16 ? bytes[10] : null,
        bytes.Length >= 16 ? bytes[11] : null,
        bytes.Length >= 16 ? BitConverter.ToUInt32(bytes, 12) : null,
        bytes.Length >= 20 ? BitConverter.ToUInt32(bytes, 16) : null);
    return true;
}

static string DecodePairing(byte value) => value switch
{
    0 => "Closed",
    1 => "Open",
    _ => $"Unknown({value})",
};

static string DecodeNullablePairing(byte? value) =>
    value is { } actual ? DecodePairing(actual) : "Missing";

static string DecodeBootButton(byte value) => value switch
{
    0 => "Released",
    1 => "Pressed",
    _ => $"Unknown({value})",
};

static string DecodeNullableBootButton(byte? value) =>
    value is { } actual ? DecodeBootButton(actual) : "Missing";

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
