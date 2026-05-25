using System.Collections.Concurrent;
using Windows.Devices.Bluetooth.Advertisement;

using static OutputFormat;

internal static class Scanner
{
    public static async Task<int> ScanForAdvertisementsAsync(
        int seconds,
        string targetName,
        Guid projectUuid)
    {
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
    }

    public static async Task<AdvertisementRecord?> ScanForTargetAsync(
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
}
