# Project Todo List

Last updated: 2026-05-25.

This file is the current project task ledger. Any future task status change,
acceptance result, or newly discovered risk must be synchronized here in the
same change that records the evidence elsewhere.

## Current Baseline

Phase 24V is implemented. The current hardware evidence confirms the Windows
saved-bond path, BLE auth metadata reset policy, and runtime saved-auth clear
effect. The Windows `ble-watch` tool has also been hardened against stale
WinRT/GATT cache failures:

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
- A runtime BOOT / IO9 clear watch observed the 8 second hold threshold and
  refreshed authorization window, and a following
  `scan-read-metadata-now ... expect-reject no-pair` confirmed the previous
  saved authorization no longer granted protected metadata access.

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
- `scan-unpair` was used to recover Windows stale pairing/cache state. The
  latest Phase 24V end state is Windows unpaired, firmware auth metadata
  cleared or missing, and the temporary authorization window opening after
  reset.

Phase 24 is still open because several runtime, visual, interoperability, and
reset/erase paths have not been accepted on hardware.

Current runtime clear-gesture state:

- After Phase 24T, a Windows saved-bond auth record was rebuilt with
  `scan-read-metadata-now 30 sleep-env-esp32c3 expect-success auto-pair`.
  That operation may write only `0x003bf000..0x003c0000`.
- `scan-watch-clear-gesture 30 sleep-env-esp32c3 180 8000` then observed
  `CLEAR_GESTURE_PRESSED_AFTER_RELEASE`,
  `CLEAR_GESTURE_HOLD_THRESHOLD pressed_ms=8000`, and
  `CLEAR_GESTURE_WINDOW_REFRESHED remaining_ms=60000`.
- The watch did not print final `CLEAR_GESTURE_RESULT success=True` because
  firmware status kept reporting `boot_button=Pressed` after the operator
  released IO9. The operator did not hold IO9 for 40 seconds or longer; treat
  this as an IO9 release-diagnostics mismatch.
- A follow-up `scan-read-metadata-now 30 sleep-env-esp32c3 expect-reject
  no-pair` reported `metadata_success=False rejected=True`, proving the
  previous saved authorization no longer grants protected metadata access.
- After a non-flashing reset and `scan-unpair`, status read reported
  `pairing=Open boot_button=Released remaining_ms=39050 pressed_ms=0`. The
  next saved-bond or replacement/update test must re-pair first.

Windows Settings showing the board as paired but not connected is expected
when `ble-watch` or another central application is not holding a GATT session.
If Windows still reports paired while firmware rejects protected access after
BLE auth records are cleared or reset, remove the Windows-side pairing with
`scan-unpair` before re-pairing.

## Phase 24 Remaining Acceptance

- [ ] Investigate and retest BOOT / IO9 release diagnostics after the runtime
  8 second saved-auth clear hold. Phase 24V validated the clear effect through
  hold-threshold/window-refresh evidence and protected metadata rejection, but
  the status stream continued to report `Pressed` after operator release until
  reset.
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
- [ ] Continue hardware validation for BOOT / IO9 release diagnostics and BLE
  auth record replacement/update paths beyond the first observed bond write,
  Phase 24T reset-pattern validation, and Phase 24V runtime clear effect.
- [ ] Recheck BOOT / IO9 electrical and UX behavior before treating it as a
  deployed user-facing pairing or clearing control.
- [ ] Validate mobile phone and gateway behavior once a real central app or
  gateway exists.
