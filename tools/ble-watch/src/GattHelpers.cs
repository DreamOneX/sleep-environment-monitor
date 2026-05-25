using Windows.Devices.Bluetooth;
using System.Runtime.InteropServices;
using Windows.Devices.Bluetooth.GenericAttributeProfile;
using Windows.Storage.Streams;

internal static class GattHelpers
{
    public static byte[] BufferToBytes(IBuffer buffer)
    {
        using var reader = DataReader.FromBuffer(buffer);
        var bytes = new byte[buffer.Length];
        reader.ReadBytes(bytes);
        return bytes;
    }

    public static async Task<ProtectedReadResult> ReadProtectedCharacteristicAsync(
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

    public static bool IsProtectedReadResultRejected(ProtectedReadResult result) =>
        IsProtectedReadStatusRejected(result.Status, result.ProtocolError);

    public static bool IsProtectedReadStatusRejected(GattCommunicationStatus status, byte? protocolError) =>
        status == GattCommunicationStatus.AccessDenied ||
        (status == GattCommunicationStatus.ProtocolError && IsProtectedAttError(protocolError));

    public static bool IsProtectedWriteRejected(GattCommunicationStatus? status, int? exceptionHResult) =>
        status == GattCommunicationStatus.AccessDenied ||
        status == GattCommunicationStatus.ProtocolError ||
        exceptionHResult is { } hresult && IsProtectedHResult(hresult);

    private static bool IsProtectedAttError(byte? protocolError) => protocolError is 0x05 or 0x08 or 0x0f;

    private static bool IsProtectedHResult(int hresult) =>
        hresult == unchecked((int)0x80650005) ||
        hresult == unchecked((int)0x80650008) ||
        hresult == unchecked((int)0x8065000f);
}
