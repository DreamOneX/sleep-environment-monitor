using Windows.Devices.Bluetooth;
using Windows.Devices.Enumeration;
using Windows.Foundation;

internal static class PairingHelpers
{
    public static void PrintPairingState(BluetoothLEDevice device, string label)
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

    public static async Task<bool> EnsurePairedAsync(BluetoothLEDevice device, string label)
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

    private static async Task<bool> TryCustomConfirmOnlyPairAsync(
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

    private static bool IsPairedResult(DevicePairingResultStatus status) =>
        status is DevicePairingResultStatus.Paired or DevicePairingResultStatus.AlreadyPaired;
}
