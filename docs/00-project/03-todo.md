# Project Todo List

Last updated: 2026-05-25.

This file is the current project task ledger. Any future task status change,
acceptance result, or newly discovered risk must be synchronized here in the
same change that records the evidence elsewhere.

## Current Baseline

Phase 24R is implemented and partially hardware-validated. The current hardware
evidence confirms the Windows saved-bond path:

- Windows Custom ConfirmOnly pairing completed.
- `PairingComplete` wrote one BLE authorization record to
  `0x003bf000..0x003c0000`.
- Reboot restored one saved authorization record.
- Encrypted metadata access succeeded with
  `scan-read-metadata-now ... expect-success no-pair`.

Phase 24 is still open because several runtime, visual, interoperability, and
reset/erase paths have not been accepted on hardware.

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
  authorization fast blink, advertising/connecting/connected slow blink, the
  180 second boot BLE status window, and the 10 second BOOT / IO9-triggered BLE
  status window.
- [ ] Validate BOOT download-mode preservation during reset or power-on.
  Holding BOOT / IO9 at reset or power-on must enter ESP32-C3 download mode,
  not BLE pairing or BLE record clearing.
- [ ] Validate live Wi-Fi/BLE ACK race behavior on hardware/runtime. Unit tests
  cover stale BLE ACK protection, but a real coexistence run is still needed.
- [ ] Validate unauthorized or unencrypted protected-characteristic rejection
  after saved authorization records are cleared.
- [ ] Validate BLE auth metadata version/checksum reset on hardware, including
  automatic pairing-window opening after missing, invalid, empty,
  version-mismatched, or compatibility-checksum-mismatched auth records.
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
- [ ] Continue hardware validation for BLE auth erase/update/reset paths beyond
  the first observed bond write.
- [ ] Recheck BOOT / IO9 electrical and UX behavior before treating it as a
  deployed user-facing pairing or clearing control.
- [ ] Validate mobile phone and gateway behavior once a real central app or
  gateway exists.
