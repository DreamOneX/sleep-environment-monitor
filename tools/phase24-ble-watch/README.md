# Phase 24 BLE Watch

Windows BLE central validation tool for Phase 24 ESP32-C3 BLE bring-up. It is
intentionally small and focused on manual integration checks against the
firmware's project GATT service.

Build from WSL with Windows .NET:

```bash
'/mnt/c/Program Files/dotnet/dotnet.exe' build "$(wslpath -w tools/phase24-ble-watch/phase24-ble-watch.csproj)"
```

Common commands:

```bash
# Scan for the board advertisement.
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/phase24-ble-watch/bin/Debug/net10.0-windows10.0.19041.0/phase24-ble-watch.dll)" 30 sleep-env-esp32c3

# Scan, connect, and read status.
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/phase24-ble-watch/bin/Debug/net10.0-windows10.0.19041.0/phase24-ble-watch.dll)" scan-read-status 30 sleep-env-esp32c3

# Scan, connect, and watch status for 60 seconds.
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/phase24-ble-watch/bin/Debug/net10.0-windows10.0.19041.0/phase24-ble-watch.dll)" scan-watch-status 30 sleep-env-esp32c3 60

# Confirm closed-window record access is rejected.
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/phase24-ble-watch/bin/Debug/net10.0-windows10.0.19041.0/phase24-ble-watch.dll)" scan-closed-window 30 sleep-env-esp32c3

# Wait for pairing window, transfer a full record, and do not ACK storage.
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/phase24-ble-watch/bin/Debug/net10.0-windows10.0.19041.0/phase24-ble-watch.dll)" scan-transfer-record 30 sleep-env-esp32c3 no-ack 128

# Wait for pairing window, transfer a full record, and request BLE storage ACK.
# This can erase/write the firmware measurement spool region.
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/phase24-ble-watch/bin/Debug/net10.0-windows10.0.19041.0/phase24-ble-watch.dll)" scan-transfer-record 30 sleep-env-esp32c3 ack 128
```

The tool prints Windows-observed UUIDs and raw binary status/metadata/fragment
frames. `scan-transfer-record` first polls the status characteristic until the
BOOT / IO9 pairing window is open, then requests metadata and fragments.

Manual transfer flow:

1. Start `scan-transfer-record`.
2. Wait for the tool to print `PAIRING_WAIT`.
3. Hold BOOT / IO9 for at least 3 seconds.
4. Release BOOT / IO9 after the tool prints `PAIRING_OPEN` or after the
   transfer completes.

If the pairing window expires while BOOT / IO9 is still held, the firmware
keeps the window closed until the button is released and pressed again. A
non-flashing `probe-rs reset --chip esp32c3` can also restore the diagnostic
state to `Released/pressed_ms=0` before another manual run.

For `ack` mode, declare the flash range before running hardware validation:
the firmware may write or erase the measurement spool region
`0x003c0000..0x00400000` through `storage_task`.
