# BLE Watch

Windows BLE central validation tool for Phase 24 ESP32-C3 BLE bring-up. It is
intentionally small and focused on manual integration checks against the
firmware's project GATT service.

Build from WSL with Windows .NET:

```bash
'/mnt/c/Program Files/dotnet/dotnet.exe' build "$(wslpath -w tools/ble-watch/ble-watch.csproj)"
```

Common commands:

```bash
# Scan for the board advertisement.
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" 30 sleep-env-esp32c3

# Scan, connect, and read status.
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-read-status 30 sleep-env-esp32c3

# Scan, connect, and watch status for 60 seconds.
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-watch-status 30 sleep-env-esp32c3 60

# Confirm closed-window record access is rejected.
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-closed-window 30 sleep-env-esp32c3

# Wait for pairing window, transfer a full record, and do not ACK storage.
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-transfer-record 30 sleep-env-esp32c3 no-ack 128

# Wait for pairing window, transfer a full record, and request BLE storage ACK.
# This can erase/write the firmware measurement spool region.
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-transfer-record 30 sleep-env-esp32c3 ack 128

# Wait for pairing window, transfer a full record, and require fragment notifications.
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-transfer-record-notify 30 sleep-env-esp32c3 no-ack 128

# Read one fragment, disconnect before CompleteRecord/ACK, then reconnect and
# confirm the same sequence is still oldest.
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-disconnect-preserves-record 30 sleep-env-esp32c3 128

# Drain records with ACK first, then read one fragment, disconnect before
# CompleteRecord/ACK, reconnect, and confirm the same sequence is still oldest.
# This can erase/write the firmware measurement spool region.
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-drain-then-disconnect-preserves-record 30 sleep-env-esp32c3 128 25 40 8

# ACK one record, then reconnect and confirm the oldest sequence advanced.
# This can erase/write the firmware measurement spool region.
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-ack-then-peek-next 30 sleep-env-esp32c3 128
```

The tool prints Windows-observed UUIDs and raw binary status/metadata/fragment
frames. `scan-transfer-record` first polls the status characteristic until the
BOOT / IO9 pairing window is open, then requests metadata and fragments.
`scan-transfer-record-notify` also subscribes to fragment notifications and
requires each requested fragment notification to match the subsequently read
fragment.
`scan-drain-then-disconnect-preserves-record` drains records first so the
disconnect-preservation check is not confused by full-spool drop-oldest
behavior.

Manual transfer flow:

1. Start `scan-transfer-record`.
2. Wait for the tool to print `PAIRING_WAIT`.
3. Hold BOOT / IO9 for at least 3 seconds.
4. Release BOOT / IO9 after the tool prints `PAIRING_OPEN` or after the
   transfer completes.

If the pairing window expires while BOOT / IO9 is still held, the firmware
keeps the window closed until the button is released and pressed again. A
non-flashing `probe-rs reset --chip esp32c3` can also restore the diagnostic
state to `Released/pressed_ms=0` before another manual run. When the tool sees
that expired-held state, it prints `PAIRING_HELD_AFTER_EXPIRED` and exits
instead of waiting for the full pairing timeout.

For `ack`, `scan-ack-then-peek-next`, or
`scan-drain-then-disconnect-preserves-record` mode, declare the flash range
before running hardware validation: the firmware may write or erase the
measurement spool region `0x003c0000..0x00400000` through `storage_task`.
These commands do not deliberately write the BLE auth metadata sector
`0x003bf000..0x003c0000`.
