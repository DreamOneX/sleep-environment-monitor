using System.Text;

internal static class BleProtocol
{
    public static byte[] EncodeControlFrame(byte opcode, ulong sequence, ushort offset, ushort length)
    {
        var frame = new byte[14];
        frame[0] = 1;
        frame[1] = opcode;
        BitConverter.GetBytes(sequence).CopyTo(frame, 2);
        BitConverter.GetBytes(offset).CopyTo(frame, 10);
        BitConverter.GetBytes(length).CopyTo(frame, 12);
        return frame;
    }

    public static bool TryDecodeMetadata(byte[] bytes, out RecordMetadata metadata)
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

    public static bool TryDecodeFragment(byte[] bytes, out RecordFragment fragment)
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

    public static uint Crc32(byte[] data)
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

    public static string Utf8Preview(byte[] payload)
    {
        var previewBytes = payload.Take(120).ToArray();
        var text = Encoding.UTF8.GetString(previewBytes)
            .Replace("\r", "\\r", StringComparison.Ordinal)
            .Replace("\n", "\\n", StringComparison.Ordinal);
        return payload.Length > previewBytes.Length ? text + "..." : text;
    }

    public static void PrintStatusDecoded(byte[] bytes)
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

    public static bool TryDecodeStatus(byte[] bytes, out StatusSnapshot status)
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

    public static string DecodePairing(byte value) => value switch
    {
        0 => "Closed",
        1 => "Open",
        _ => $"Unknown({value})",
    };

    public static string DecodeNullablePairing(byte? value) =>
        value is { } actual ? DecodePairing(actual) : "Missing";

    public static string DecodeBootButton(byte value) => value switch
    {
        0 => "Released",
        1 => "Pressed",
        _ => $"Unknown({value})",
    };

    public static string DecodeNullableBootButton(byte? value) =>
        value is { } actual ? DecodeBootButton(actual) : "Missing";

    private static string DecodeRuntime(byte value) => value switch
    {
        0 => "Disabled",
        1 => "ControllerReady",
        2 => "HostPending",
        3 => "Advertising",
        4 => "Connected",
        5 => "Error",
        _ => $"Unknown({value})",
    };

    private static string DecodeNetwork(byte value) => value switch
    {
        0 => "Disconnected",
        1 => "Connecting",
        2 => "Connected",
        3 => "IpReady",
        _ => $"Unknown({value})",
    };

    private static string DecodeUpload(byte value) => value switch
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
}
