# Project Todo List

Last updated: 2026-05-25.

This file is the current project task ledger. Any future task status change,
acceptance result, or newly discovered risk must be synchronized here in the
same change that records the evidence elsewhere.

## Current Baseline

Phase 24U is implemented. The current hardware evidence confirms the Windows
saved-bond path and BLE auth metadata reset policy, and the Windows
`ble-watch` tool has been hardened against stale WinRT/GATT cache failures:

- Windows Custom ConfirmOnly pairing completed.
- `PairingComplete` wrote one BLE authorization record to
  `0x003bf000..0x003c0000`.
- Reboot restored one saved authorization record.
- Encrypted metadata access succeeded with
  `scan-read-metadata-now ... expect-success no-pair`.
- Missing/erased BLE auth metadata, invalid header magic, empty current-version
  records, records-version mismatch, compatibility-checksum mismatch, and
  header checksum mismatch all opened the temporary authorization window on
  boot.
- After a reset/invalid-auth pairing window closed,
  `scan-read-metadata-now ... expect-reject no-pair` confirmed an unpaired
  central could not access protected metadata.

Phase 24S also separates plain Wi-Fi/IP-not-ready LED indication from explicit
network faults:

- `config::led::WIFI_UNREADY_STATUS_WINDOW_SECS` defaults to `0`, disabling
  the blue LED3 slow-blink hint for plain Wi-Fi-not-ready state.
- Explicit `ErrorFlags::WIFI`, `ErrorFlags::IP`, and
  `ErrorFlags::DISCOVERY` still slow-blink LED3 through
  `ErrorFlags::NETWORK_MASK`.
- `ErrorFlags::NETWORK_MASK` is scoped to the Wi-Fi/IP/discovery REST upload
  path and does not include BLE advertising, BLE connection, or BLE
  authorization state.

Phase 24U keeps firmware behavior unchanged but improves the Windows central
validation tool:

- GATT service and characteristic lookup retry with Uncached lookup and Cached
  fallback.
- Status reads retry with Uncached values only for runtime decisions.
- `scan-read-status` can recreate WinRT device/GATT objects after repeated
  status-read failures.
- `scan-watch-clear-gesture` can reconnect after transient status-read
  failures.
- `scan-unpair` was used to recover Windows stale pairing/cache state; the
  Windows central is currently unpaired.

Phase 24 is still open because several runtime, visual, interoperability, and
reset/erase paths have not been accepted on hardware.

Current runtime clear-gesture state:

- After Phase 24T, a new Windows saved-bond auth record was rebuilt with
  `scan-read-metadata-now 30 sleep-env-esp32c3 expect-success auto-pair`.
  That operation may write only `0x003bf000..0x003c0000`.
- The latest `scan-watch-clear-gesture 30 sleep-env-esp32c3 180 8000` run
  connected and watched status, but observed no IO9 / BOOT press:
  `pressed_after_release=False`, `hold_threshold=False`, and
  `refreshed_window=False`.
- A follow-up `scan-read-metadata-now 30 sleep-env-esp32c3 expect-success
  no-pair` succeeded, so the saved auth record is still usable. The latest
  clear-gesture result means "no operator press observed", not a firmware clear
  failure.
- Phase 24U then used `scan-unpair` to recover Windows stale GATT state. The
  Windows central is now unpaired, so the next clear-gesture validation must
  first rebuild or otherwise confirm a saved-bond auth record before proving
  that the 8 second BOOT / IO9 gesture clears it.

Windows Settings showing the board as paired but not connected is expected
when `ble-watch` or another central application is not holding a GATT session.
If Windows still reports paired while firmware rejects protected access after
BLE auth records are cleared or reset, remove the Windows-side pairing with
`scan-unpair` before re-pairing.

## Phase 24 Remaining Acceptance

- [ ] Validate the runtime 8 second BOOT / IO9 saved-auth clear gesture after
  firmware boot. Expected effect: erase the BLE auth metadata sector
  `0x003bf000..0x003c0000` and reopen the temporary authorization window.
  Use `scan-watch-clear-gesture` for delay-safe operator coordination, then
  verify protected access rejection with
  `scan-read-metadata-now ... expect-reject no-pair` after the temporary
  window closes.
- [ ] Manually accept LED3 visual behavior on hardware: pairing or
  authorization fast blink, advertising-or-connected slow blink, the
  180 second boot BLE status window, and the 10 second BOOT / IO9-triggered BLE
  status window.
- [ ] Validate BOOT download-mode preservation during reset or power-on.
  Holding BOOT / IO9 at reset or power-on must enter ESP32-C3 download mode,
  not BLE pairing or BLE record clearing.
- [ ] Validate live Wi-Fi/BLE ACK race behavior on hardware/runtime. Unit tests
  cover stale BLE ACK protection, but a real coexistence run is still needed.
- [ ] Validate phone or gateway interoperability beyond the current Windows BLE
  central validation tool.
- [ ] Validate BLE auth record replacement/update behavior when another bond is
  stored or an existing peer is updated.

## Phase 25 Refactor And Maintenance

Phase 25 should start only after Phase 24 behavior is frozen or the remaining
validation gaps are intentionally carried forward. These items should be
equivalent moves unless explicitly documented otherwise.

- [ ] Freeze the BLE UUIDs, status/metadata/control frame bytes, ACK policy,
  Wi-Fi HTTP 2xx ACK semantics, flash ranges, and measurement payload JSON
  shape before refactoring.
- [ ] Split `tools/ble-watch/Program.cs` into CLI, BLE profile constants,
  scanner, GATT client, transfer client, protocol helpers, models, and WinRT
  helpers.
- [ ] Split `firmware/src/tasks/upload.rs` into pure endpoint/JSON/HTTP/parse
  and time logic plus target runtime uploader code.
- [ ] Split `firmware/src/tasks/ble.rs` into profile, status, protocol,
  transfer, ACK, pairing, auth, storage bridge, GATT, and runtime modules while
  preserving public paths through re-exports during the move.
- [ ] Keep `docs/10-firmware/05-ble.md` as the BLE ACK/security rule authority
  during the split.
- [ ] Keep LED overlay rules centralized and avoid duplicate status policy
  implementations.
- [ ] Consider later, lower-priority splits for `firmware/src/storage/spool.rs`
  and `firmware/src/tasks/storage.rs`.

## Future Security, Configuration, And Hardware Validation

- [ ] Replace bring-up Wi-Fi defaults with a deployment-safe credential model.
  The default open `FZU` network remains a bring-up default and must not be
  treated as a production credential pattern.
- [ ] Add or preserve build/test validation for Wi-Fi credential configuration,
  including open, WPA, and mixed legacy modes if they remain supported.
- [ ] Keep real secrets out of tracked configuration and documentation.
- [ ] Revalidate flash-write operations by first stating the exact flash range
  being exercised. Current important ranges are:
  `0x003bf000..0x003c0000` for BLE auth metadata and
  `0x003c0000..0x00400000` for the measurement spool.
- [ ] Continue hardware validation for BLE auth runtime clear and record
  replacement/update paths beyond the first observed bond write and Phase 24T
  reset-pattern validation.
- [ ] Recheck BOOT / IO9 electrical and UX behavior before treating it as a
  deployed user-facing pairing or clearing control.
- [ ] Validate mobile phone and gateway behavior once a real central app or
  gateway exists.
