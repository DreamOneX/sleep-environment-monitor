# Handoff

Last updated: 2026-05-24.

## Current State

Phase 24Q is implemented in the current working tree, with compile/unit
verification refreshed in this handoff. Phase 24 is not complete yet.

Phase 24P added live BLE evidence for two storage-transfer checks and the
current LED status boundary:

- Post-ACK oldest-record advancement succeeded with `scan-ack-then-peek-next`:
  ACKed sequence `108009`, then observed next oldest sequence `108010`.
- Disconnect before `CompleteRecord` or `AckRecord` preserved the oldest record
  after a drain precondition: first and second metadata reads both reported
  sequence `109130`, payload length `199`.
- LED facts are corrected: LED1 is green power on the 3.3 V rail and is not
  MCU-controlled; LED2 is red active-low on IO0; LED3 is blue active-low on IO1.
- Firmware keeps LED2 as heartbeat with a short boot/reset fast-flash and uses
  LED3 as normal status plus time-bounded BLE status overlay.

Phase 24Q adds a compile-validated BLE security and authorization-record
persistence path:

- `firmware/src/storage/ble_auth.rs` now models fixed-size BLE auth records
  with identity address, LTK, optional IRK, security level, bonded flag, record
  CRC, record-set checksum, version policy, load/store/clear helpers, and pure
  tests.
- `firmware/src/tasks/ble.rs` seeds TrouBLE security from TRNG, restores saved
  bond records before host build, requests encryption when a pairing window or
  saved auth exists, requires encrypted matching saved auth outside the BOOT /
  IO9 window, and stores a bond record on `PairingComplete`.
- This is a compile/static milestone only for saved pairing. No hardware
  pairing, reboot restore, phone/gateway interoperability, or BLE auth-sector
  write/erase/update validation has been run for Phase 24Q.

No firmware image was flashed during this handoff state.

Important flash ranges:

- BLE auth metadata sector: `0x003bf000..0x003c0000`. Current target code can
  write/erase this sector when BLE-enabled firmware receives
  `PairingComplete { bond: Some(..) }`, but this handoff did not deliberately
  exercise that range.
- Measurement spool: `0x003c0000..0x00400000`. BLE ACK/drain validation may
  exercise normal spool writes/erases through `storage_task` in this range.

Before any future flash-write validation, state the exact range being
exercised.

## Implemented

- `tools/ble-watch` includes `scan-drain-then-disconnect-preserves-record`.
- `firmware/src/board.rs` maps MCU-controlled LEDs as `PIN_LED2 = 0` and
  `PIN_LED3 = 1`.
- `firmware/src/tasks/led.rs` fast-flashes red LED2 at boot/reset and then runs
  the heartbeat.
- `firmware/src/util/status.rs` exposes pure LED status mapping for the blue
  LED3 BLE overlay, including pairing fast blink, BLE runtime slow blink, and
  boot/trigger indication windows.
- `firmware/src/bin/main.rs` routes GPIO0 to the LED2 heartbeat task and GPIO1
  to the LED3 status task, and wires BLE runtime/pairing signals into LED3.
- `firmware/src/tasks/ble.rs` publishes BLE runtime and pairing status for the
  LED overlay, owns the BLE security compile path, and stores/loads BLE auth
  records through the reserved auth sector.
- `firmware/src/bin/main.rs` obtains a BLE security seed from TRNG before
  reusing ADC1 for the microphone path.
- Documentation records Phase 24P hardware evidence, Phase 24Q compile
  evidence, current LED mapping, and the remaining Phase 24 validation gaps.

The hardware-validated BLE authorization path is still the temporary BOOT /
IO9 window. Saved BLE pairing records now have a target compile path, but they
are not accepted as hardware-validated behavior until pairing, flash
write/erase/update, reboot restore, version/checksum reset, and user clearing
are tested.

## Subagents

Requested subagents completed or reported during this handoff:

- Documentation/fact check: no high-severity current fact conflicts were found;
  old "paired central" wording and old LED2 status wording were updated or
  marked as historical where relevant.
- Architecture file-tree/config update: directly updated
  `docs/10-firmware/00-architecture.md` with directory-first file tree and
  `config.rs` ownership details.
- Wi-Fi config check: default `FZU` open network remains a bring-up default and
  should not be treated as a deployment credential model; future work should
  avoid committing real passwords, mark WPA/mixed modes as legacy, and consider
  test/build-time validation for the default credential triple.
- Duplicate-code check: Phase 24 should avoid broad refactors; Phase 25 should
  first clean `tools/ble-watch` command/GATT/transfer duplication, then smaller
  firmware storage/GATT status publishing duplication.
- `cfg(target_arch = "riscv32")` check: most cfgs are valid embedded/host-test
  boundaries; the real boundary debt is the large `tasks/ble.rs` module, which
  should be split in Phase 25 after behavior is frozen.
- Phase 25 refactor plan: split `tools/ble-watch`, `tasks/upload.rs`, and
  `tasks/ble.rs` by equivalent moves only. Freeze BLE UUIDs, status/metadata /
  control frame bytes, Wi-Fi HTTP 2xx ACK semantics, BLE ACK policy, flash
  ranges, and payload JSON shape.

## Verification

Passed after the latest code and documentation edits:

```bash
cargo fmt
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo build --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
cargo clippy --target riscv32imc-unknown-none-elf --features ble-upload,radio-coex
git diff --check
```

Observed result:

- `cargo test --lib`: `181 passed; 0 failed`.
- Normal ESP32-C3 target build: passed.
- BLE+Wi-Fi coexistence ESP32-C3 target build: passed.
- Host clippy, normal target clippy, and BLE+coex target clippy: passed.
- `tools/ble-watch` Windows .NET build was not rerun in Phase 24Q because no
  C# tool code changed.
- `git diff --check`: passed.

Milestone commit message:

```text
feat: add BLE auth persistence compile path
```

## Remaining Phase 24 Work

- Validate live Wi-Fi/BLE ACK race behavior on hardware/runtime.
- Validate BOOT / IO9 still enters download mode during reset or power-on.
- Validate real BLE pairing, saved bond restore across reboot, and rejected
  unauthorized/unencrypted access on hardware.
- Validate BLE auth metadata write/erase/update behavior, version/checksum
  reset behavior, automatic pairing-window opening after auth-record reset, and
  user clearing.
- Manually accept LED3 hardware visual behavior: pairing/authorization fast
  blink, advertising/connecting/connected slow blink, 180 second boot BLE
  status window, and 10 second BOOT / IO9-triggered BLE status window.

## Phase 25 Notes

Phase 25 should start with documentation and baseline freezing, then split long
files without behavior changes:

- `tools/ble-watch/Program.cs`: split CLI, BLE profile constants, scanner, GATT
  client, transfer client, protocol helpers, models, and WinRT helpers.
- `firmware/src/tasks/upload.rs`: split pure endpoint/JSON/HTTP/parse/time
  logic from target runtime uploader.
- `firmware/src/tasks/ble.rs`: split profile/status/protocol/transfer/ACK /
  pairing/auth/storage bridge/GATT/runtime modules, keeping original public
  paths re-exported during the move.
- Later candidates: `firmware/src/storage/spool.rs` and
  `firmware/src/tasks/storage.rs`.

Phase 25 must not change BLE UUIDs, frame layouts, ACK conditions, Wi-Fi retry /
discovery / time-sync behavior, flash ranges, or measurement payload JSON shape.
