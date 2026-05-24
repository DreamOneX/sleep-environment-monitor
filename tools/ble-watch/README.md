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

# Directly request protected metadata without waiting for the BOOT / IO9
# authorization window. Use expect-success after saved bonding is validated;
# use expect-reject after clearing saved BLE authorization records.
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-read-metadata-now 30 sleep-env-esp32c3 expect-success auto-pair
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-read-metadata-now 30 sleep-env-esp32c3 expect-success no-pair
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-read-metadata-now 30 sleep-env-esp32c3 expect-reject no-pair

# Remove the board pairing record from Windows before re-pairing.
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-unpair 30 sleep-env-esp32c3

# Remove the Windows pairing record, wait for the BOOT / IO9 authorization
# window, pair again, then read protected metadata. Capture firmware RTT logs
# during this run; central-side success alone does not validate auth-record
# update/replacement.
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-unpair-then-pair-metadata 30 sleep-env-esp32c3 90

# Watch the status characteristic while the operator performs the runtime
# BOOT / IO9 saved-auth clear gesture. This avoids relying on chat timing.
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-watch-clear-gesture 30 sleep-env-esp32c3 180 8000

# Wait for pairing window, transfer a full record, and do not ACK storage.
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-transfer-record 30 sleep-env-esp32c3 no-ack 128

# Wait for pairing window, transfer a full record, and request BLE storage ACK.
# This can erase/write the firmware measurement spool region.
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-transfer-record 30 sleep-env-esp32c3 ack 128

# Transfer through the current saved-bond or auto-pair path without waiting for
# the BOOT / IO9 authorization window, then request BLE storage ACK. This is
# useful for Wi-Fi/BLE ACK-policy checks while Wi-Fi upload is already running.
# This can erase/write the firmware measurement spool region if firmware accepts
# the ACK.
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-transfer-record-now 30 sleep-env-esp32c3 ack 128 auto-pair

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
frames. It also prints Windows pairing state for commands that connect to the
device. GATT service and characteristic lookup retries Uncached reads and then
uses Cached lookup as a Windows stale-cache recovery fallback; status values
used for runtime decisions are still read Uncached. `scan-read-status` may
recreate the WinRT device/GATT objects after repeated status-read failures, and
`scan-watch-clear-gesture` reconnects after a transient status-read failure
instead of ending the watch immediately. If Windows keeps returning
`Unreachable` for GATT status reads, `scan-unpair` can remove the stale
Windows-side pairing/cache state before re-pairing.
`scan-transfer-record` first polls the status characteristic until the
BOOT / IO9 pairing window is open, then requests metadata and fragments.
`scan-transfer-record-now` does not wait for the BOOT / IO9 window. `auto-pair`
uses Windows Custom Pairing with ConfirmOnly acceptance if Windows or firmware
needs a current bond; `no-pair` skips pairing and relies on an existing saved
authorization path. The command reads the initial and final status snapshots
around the transfer so ACK-policy checks can record the concurrent Wi-Fi/upload
state.
`scan-transfer-record-notify` also subscribes to fragment notifications and
requires each requested fragment notification to match the subsequently read
fragment.
`scan-drain-then-disconnect-preserves-record` drains records first so the
disconnect-preservation check is not confused by full-spool drop-oldest
behavior.
`scan-read-metadata-now` does not wait for the BOOT / IO9 window. `auto-pair`
uses Windows Custom Pairing with ConfirmOnly acceptance for first saved-bond
validation. `no-pair` skips pairing and is intended for reboot restore
validation or rejection validation after saved authorization records are
cleared.
`scan-unpair` removes the Windows-side pairing record for the scanned board.
Use it when Windows still reports the device as paired but the firmware has
cleared or rejected the saved BLE authorization record. Windows Settings may
show this custom GATT peripheral as paired but not connected when no central
application such as `ble-watch` is holding a GATT session; that state alone is
expected and is not a firmware connection failure.
`scan-unpair-then-pair-metadata` removes the Windows-side pairing record,
opens a status connection to wait for the BOOT / IO9 authorization window,
disconnects, reconnects so firmware can configure the new link as bondable,
runs Windows Custom Pairing with ConfirmOnly acceptance, and reads protected
metadata. Use it with RTT capture when validating real auth-record update or
replacement behavior. The central command must be paired with firmware logs
such as `ble auth record updated`, `ble auth record appended`, or
`ble auth record capacity full; replacing oldest bond record`; the tool output
alone is not accepted as firmware auth-record replacement/update evidence.

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

Runtime BLE auth clearing flow:

1. Hold BOOT / IO9 after firmware has booted.
2. About 2 seconds opens the temporary authorization window.
3. Continue holding until about 8 seconds to request BLE auth-record clearing.
4. Release BOOT / IO9 after firmware logs the clear or after the tool observes
   the reopened pairing window.

Delay-safe clear validation flow:

Before using this flow for Phase 24Y release diagnostics, flash firmware that
includes the Phase 24Y BOOT / IO9 transition logs and state the app-image flash
range first: approximately `0x00010000..0x003bf000`. Capture RTT/defmt logs
with the `scan-watch-clear-gesture` output. For manual timing, send a
PowerShell `New-BurntToastNotification` first and wait for the operator to
reply in chat that the notification was received and they are ready.

1. Start `scan-watch-clear-gesture`.
2. During the watch window, release BOOT / IO9 until the tool can observe
   `CLEAR_GESTURE_RELEASED`.
3. Hold BOOT / IO9 for about 9 to 10 seconds.
4. Release BOOT / IO9 after the hold. The command succeeds only after it has
   observed release before the press, the 8 second hold threshold, a refreshed
   near-full pairing window after that threshold, and release after the hold.

If the tool observes the hold threshold and refreshed pairing window before it
observes the final release, it prints `CLEAR_GESTURE_CLEAR_EFFECT_OBSERVED`.
If the watch ends with that clear-effect evidence but without final release
observation, it prints `CLEAR_GESTURE_RELEASE_DIAGNOSTIC_MISSING` and keeps the
command failed. Treat that as evidence to investigate BOOT / IO9 release
diagnostics, not as proof that the operator kept holding BOOT / IO9.

After the clear gesture succeeds, wait for the temporary authorization window
to close and run `scan-read-metadata-now ... expect-reject no-pair` to confirm
the old saved authorization record no longer grants protected GATT access.

Auth-record update/replacement validation flow:

1. Capture firmware RTT/defmt logs while running the command.
2. Start `scan-unpair-then-pair-metadata`.
3. Wait for the tool to print `PAIRING_WAIT`.
4. Hold BOOT / IO9 for at least 3 seconds to open the authorization window,
   then release it after the tool prints `PAIRING_OPEN`.
5. Accept the run only if the tool reports
   `UNPAIR_PAIR_METADATA_SUMMARY success=True` and firmware logs the expected
   auth-record action plus `ble auth bond stored`.

For an existing-peer update check, the expected firmware auth-record action is
`ble auth record updated`. For a second-peer or full-capacity replacement check,
use a distinct central device and expect `ble auth record appended` or
`ble auth record capacity full; replacing oldest bond record` as appropriate.

The 3 second hold used in older manual transfer instructions is only an
operator-side suggestion to exceed the firmware's about-2-second authorization
threshold. It is not a 3 second firmware window. The temporary authorization
window lasts about 60 seconds after it opens.

For `ack`, `scan-transfer-record-now ... ack`,
`scan-ack-then-peek-next`, or `scan-drain-then-disconnect-preserves-record`
mode, declare the flash range before running hardware validation: the firmware
may write or erase the measurement spool region `0x003c0000..0x00400000`
through `storage_task`.
Pairing, saved-bond restore validation, and the runtime clear gesture may write
or erase the BLE auth metadata sector `0x003bf000..0x003c0000`.
`scan-unpair-then-pair-metadata` may also write the BLE auth metadata sector
when pairing completes and firmware stores the refreshed bond.
