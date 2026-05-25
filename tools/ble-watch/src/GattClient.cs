using System.Runtime.InteropServices;
using Windows.Devices.Bluetooth;
using Windows.Devices.Bluetooth.GenericAttributeProfile;
using Windows.Storage.Streams;

using static BleProtocol;
using static GattHelpers;
using static OutputFormat;
using static PairingHelpers;

internal static class GattClient
{
    public static async Task DumpGattAsync(BluetoothLEDevice device)
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

    public static async Task<GattCharacteristic?> GetCharacteristicAsync(
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

    public static async Task<GattCharacteristicsResult> GetCharacteristicsForUuidWithRetryAsync(
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

    public static async Task<GattDeviceServicesResult> GetGattServicesForUuidWithRetryAsync(
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

    public static async Task<StatusConnectionOpenResult> OpenStatusConnectionAsync(
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

    public static async Task<bool> WaitForPairingOpenAsync(
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

    public static async Task<StatusSnapshot?> ReadStatusSnapshotAsync(GattCharacteristic status, string label)
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

    public static async Task<StatusSnapshot?> ReadStatusSnapshotRecoverableAsync(GattCharacteristic status, string label)
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

    public static bool IsRecoverableGattException(Exception error) =>
        error is ObjectDisposedException or COMException;

    public static async Task<GattReadResult> ReadStatusValueWithRetryAsync(
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

    public static async Task<bool> WaitForAuthorizedMetadataRequestAsync(
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

    public static async Task<bool> WriteControlAsync(GattCharacteristic control, string label, byte[] frame)
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
}
