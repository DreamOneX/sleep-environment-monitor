# Handoff

Last updated: 2026-05-25.

When future work changes task status, records a new acceptance result, or finds
a new risk, update [03-todo.md](03-todo.md) in the same documentation pass.

## Current State

Phase 24V is implemented in the current working tree. Phase 24 is not complete
yet.

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

Phase 24Q added a compile-validated BLE security and authorization-record
persistence path. Phase 24R then validated the first Windows saved-bond restore
path on hardware and added a runtime saved-auth clear gesture:

- `firmware/src/storage/ble_auth.rs` now models fixed-size BLE auth records
  with identity address, LTK, optional IRK, security level, bonded flag, record
  CRC, record-set checksum, version policy, load/store/clear helpers, and pure
  tests.
- `firmware/src/tasks/ble.rs` seeds TrouBLE security from TRNG, restores saved
  bond records before host build, proactively requests security only while the
  BOOT / IO9 pairing window is open, requires encrypted matching saved auth
  outside the BOOT / IO9 window, and stores a bond record on
  `PairingComplete`.
- `tools/ble-watch` now includes `scan-read-metadata-now`, Windows Custom
  ConfirmOnly pairing, Windows pairing-state logging, `no-pair` mode,
  `scan-unpair`, and `scan-watch-clear-gesture`.
- Runtime BOOT / IO9 handling keeps the 2 second authorization-window gesture
  and adds an 8 second saved-auth clear gesture that erases
  `0x003bf000..0x003c0000` and reopens the temporary window.
- Hardware validation confirmed Windows pairing, BLE auth-sector write after
  `PairingComplete`, reboot restore of one saved authorization record, and
  encrypted metadata access with `scan-read-metadata-now ... expect-success
  no-pair`.

Phase 24S separated plain Wi-Fi/IP-not-ready indication from explicit network
faults in the blue LED3 policy:

- `config::led::WIFI_UNREADY_STATUS_WINDOW_SECS` defaults to `0`, disabling
  the plain Wi-Fi-unready slow-blink hint.
- Explicit `ErrorFlags::WIFI`, `ErrorFlags::IP`, and
  `ErrorFlags::DISCOVERY` still slow-blink LED3 through
  `ErrorFlags::NETWORK_MASK`.
- `ErrorFlags::NETWORK_MASK` remains scoped to the Wi-Fi/IP/discovery REST
  upload path and does not include BLE advertising, BLE connection, or BLE
  authorization state.
- BLE LED wording is aligned to the implemented runtime states:
  pairing/authorization fast blink and advertising-or-connected slow blink.

Phase 24T hardware-validated the BLE auth metadata reset policy:

- The existing BLE auth metadata sector was backed up from
  `0x003bf000..0x003c0000` to `/tmp/ble-auth-before-phase24t.bin`.
- Only `0x003bf000..0x003c0000` was deliberately written or erased with
  `cargo espflash write-bin` and `cargo espflash erase-region`.
- Missing/erased metadata, bad header magic, empty current-version records,
  records-version mismatch, compatibility-checksum mismatch, and header
  checksum mismatch all auto-opened the temporary authorization window on boot.
- After the final reset window closed, `scan-read-metadata-now ... expect-reject
  no-pair` confirmed an unpaired central was rejected at protected metadata
  control write.
- This did not validate the runtime 8 second BOOT / IO9 clear gesture itself.

Phase 24U hardened the Windows central validation tool after stale WinRT/GATT
objects blocked more hardware validation:

- GATT service and characteristic lookup now retry Uncached lookup and then use
  Cached lookup as a Windows stale-cache recovery fallback.
- Status reads retry but still use only Uncached status values for runtime
  decisions.
- `scan-read-status` can recreate the Windows `BluetoothLEDevice` / GATT
  objects after repeated status-read failures.
- `scan-watch-clear-gesture` can reconnect after a transient status-read
  failure instead of immediately ending the delay-safe watch.
- `scan-unpair` cleared the Windows-side pairing/cache state during recovery.

Phase 24V hardware-validated the runtime saved-auth clear effect:

- After Phase 24T left the board with invalid auth metadata and Windows unpaired,
  a non-flashing `probe-rs reset --chip esp32c3` reopened the temporary
  authorization window and restored BOOT / IO9 diagnostics to
  `boot_button=Released pressed_ms=0`.
- `scan-read-metadata-now 30 sleep-env-esp32c3 expect-success auto-pair`
  rebuilt one Windows saved-bond auth record. This may write only the BLE auth
  metadata sector `0x003bf000..0x003c0000`.
- `scan-read-metadata-now 30 sleep-env-esp32c3 expect-success no-pair` then
  confirmed protected metadata access through the saved authorization record.
- `scan-watch-clear-gesture 30 sleep-env-esp32c3 180 8000` observed
  `CLEAR_GESTURE_PRESSED_AFTER_RELEASE`,
  `CLEAR_GESTURE_HOLD_THRESHOLD pressed_ms=8000`, and
  `CLEAR_GESTURE_WINDOW_REFRESHED remaining_ms=60000`.
- The watch did not produce final `CLEAR_GESTURE_RESULT success=True` because
  firmware status frames continued to report `boot_button=Pressed` after the
  operator released IO9. The operator did not hold IO9 for 40 seconds or
  longer; treat this as an IO9 release-diagnostics mismatch.
- `scan-read-metadata-now 30 sleep-env-esp32c3 expect-reject no-pair` reported
  `METADATA_NOW_RESULT success=True metadata_success=False rejected=True
  phase=control_write`, proving the previous saved authorization no longer
  grants protected metadata access.
- After a non-flashing `probe-rs reset --chip esp32c3`, `scan-unpair` reported
  `UNPAIR_RESULT status=Unpaired`, and a final status read decoded
  `pairing=Open boot_button=Released remaining_ms=39050 pressed_ms=0`.

Firmware was flashed during Phase 24R hardware validation with the BLE+Wi-Fi
build using `probe-rs` through the ESP JTAG interface. Before flashing, the
declared ranges were:

Important flash ranges:

- App firmware region: approximately `0x00010000..0x003bf000`.
- BLE auth metadata sector: `0x003bf000..0x003c0000`. Phase 24R deliberately
  exercised a write to this sector through `PairingComplete` and restored one
  saved record after reboot. Phase 24T deliberately erased or overwrote this
  same sector to validate reset/invalid metadata auto-pair behavior. Phase 24V
  validated the runtime clear effect through hold-threshold/window-refresh
  evidence and rejected protected metadata access after the clear watch.
- Measurement spool: `0x003c0000..0x00400000`. BLE ACK/drain validation may
  exercise normal spool writes/erases through `storage_task` in this range.
  During the long Phase 24R runtime, the storage task continued appending and
  dropping oldest records as the spool filled.

Before any future flash-write validation, state the exact range being
exercised.

## Implemented

- `tools/ble-watch` includes `scan-drain-then-disconnect-preserves-record`.
- `tools/ble-watch` retries Windows GATT service and characteristic lookup,
  retries Uncached status reads, recreates status connections for
  `scan-read-status`, and reconnects the runtime clear-gesture watch after a
  transient status-read failure.
- `firmware/src/board.rs` maps MCU-controlled LEDs as `PIN_LED2 = 0` and
  `PIN_LED3 = 1`.
- `firmware/src/tasks/led.rs` fast-flashes red LED2 at boot/reset and then runs
  the heartbeat.
- `firmware/src/util/status.rs` exposes pure LED status mapping for the blue
  LED3 BLE overlay, including pairing fast blink, BLE runtime slow blink, and
  boot/trigger indication windows.
- `firmware/src/tasks/led.rs` uses
  `config::led::WIFI_UNREADY_STATUS_WINDOW_SECS` to decide whether plain
  Wi-Fi/IP-not-ready state should temporarily slow-blink LED3. The default `0`
  keeps that hint disabled, while explicit network error flags still slow-blink.
- `firmware/src/bin/main.rs` routes GPIO0 to the LED2 heartbeat task and GPIO1
  to the LED3 status task, and wires BLE runtime/pairing signals into LED3.
- `firmware/src/tasks/ble.rs` publishes BLE runtime and pairing status for the
  LED overlay, owns the BLE security compile path, and stores/loads BLE auth
  records through the reserved auth sector. It also clears the saved auth
  sector on the 8 second runtime BOOT / IO9 gesture.
- `firmware/src/bin/main.rs` obtains a BLE security seed from TRNG before
  reusing ADC1 for the microphone path.
- Documentation records Phase 24P storage-transfer evidence, Phase 24R
  saved-bond restore evidence, Phase 24S LED status/config boundary, Phase 24T
  auth metadata reset evidence, Phase 24U Windows GATT recovery tooling,
  Phase 24V runtime clear-effect evidence, current LED mapping, and the
  remaining Phase 24 validation gaps.

The hardware-validated BLE authorization paths are now the temporary BOOT / IO9
window, the Windows saved-bond restore path, the auth metadata reset policy,
and the Phase 24V runtime clear effect. The latest board state is Windows
unpaired with firmware auth metadata cleared or missing; after reset the
temporary authorization window opened. The next saved-bond or replacement test
must re-pair first. Windows Settings may show the custom GATT peripheral as
paired but not connected when `ble-watch` is not holding a GATT session; that
passive Settings label is not a Phase 24 acceptance signal. If Windows still
reports paired while firmware rejects protected access after an auth reset or
clear, use `scan-unpair` before re-pairing. Phone/gateway interoperability,
BOOT / IO9 release diagnostics after the clear hold, record replacement, and
LED3 visual behavior remain unvalidated.

## Subagents

Requested subagents completed or reported during this handoff:

- Documentation/fact check: no high-severity current fact conflicts were found.
  The important follow-up was to keep old "paired central" wording precise and
  keep old LED2 status wording marked as historical where relevant.
- Architecture file-tree/config update: directly updated
  `docs/10-firmware/00-architecture.md` with directory-first file tree and
  `config.rs` ownership details.
- Wi-Fi config check: default `FZU` open network remains a bring-up default and
  should not be treated as a deployment credential model; future work should
  avoid committing real passwords, mark WPA/mixed modes as legacy, and consider
  test/build-time validation for the default credential triple.
- Duplicate-code check: Phase 24 should avoid broad refactors; Phase 25 should
  keep `docs/10-firmware/05-ble.md` as the BLE ACK/security rule authority,
  keep LED overlay rules centralized, and split protocol/helper duplication
  only after behavior is frozen.
- `cfg(target_arch = "riscv32")` check: most cfgs are valid embedded/host-test
  boundaries; the real boundary debt is the large `tasks/ble.rs` module, which
  should be split in Phase 25 after behavior is frozen.
- Phase 25 refactor plan: split `tools/ble-watch`, `tasks/upload.rs`, and
  `tasks/ble.rs` by equivalent moves only. Freeze BLE UUIDs, status/metadata /
  control frame bytes, Wi-Fi HTTP 2xx ACK semantics, BLE ACK policy, flash
  ranges, and payload JSON shape.

## Verification

Phase 24T firmware and documentation verification passed:

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

- `cargo test --lib`: `185 passed; 0 failed`.
- Normal ESP32-C3 target build: passed.
- BLE+Wi-Fi coexistence ESP32-C3 target build: passed.
- Host clippy, normal target clippy, and BLE+coex target clippy: passed.
- `git diff --check`: passed.

Milestone commit message:

```text
test: validate BLE auth metadata reset policy
```

Phase 24U tool verification:

```bash
'/mnt/c/Program Files/dotnet/dotnet.exe' build "$(wslpath -w tools/ble-watch/ble-watch.csproj)"
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-unpair 30 sleep-env-esp32c3
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-read-status 30 sleep-env-esp32c3
git diff --check
```

Observed result:

- The Windows .NET build passed with 0 warnings and 0 errors.
- `scan-unpair` reported `UNPAIR_RESULT status=Unpaired`.
- A following `scan-read-status` succeeded after stale GATT recovery and
  decoded `runtime=Connected network=Disconnected upload=Failed pending=32
  error_flags=0x00000000 pairing=Closed boot_button=Released remaining_ms=0
  pressed_ms=0`.
- No firmware image was flashed and no firmware flash sector was deliberately
  written or erased.

Milestone commit message:

```text
test: harden BLE watch GATT recovery
```

Phase 24V hardware verification:

```bash
probe-rs reset --chip esp32c3
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-read-status 30 sleep-env-esp32c3
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-read-metadata-now 30 sleep-env-esp32c3 expect-success auto-pair
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-read-metadata-now 30 sleep-env-esp32c3 expect-success no-pair
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-watch-clear-gesture 30 sleep-env-esp32c3 180 8000
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-read-metadata-now 30 sleep-env-esp32c3 expect-reject no-pair
probe-rs reset --chip esp32c3
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-unpair 30 sleep-env-esp32c3
'/mnt/c/Program Files/dotnet/dotnet.exe' "$(wslpath -w tools/ble-watch/bin/Debug/net10.0-windows10.0.19041.0/ble-watch.dll)" scan-read-status 30 sleep-env-esp32c3
git diff --check
```

Observed result:

- Saved auth was rebuilt and confirmed with `expect-success no-pair`.
- The clear watch observed an 8 second hold threshold and a refreshed
  authorization window.
- Protected metadata access was rejected after the clear watch, proving the
  previous saved authorization no longer granted access.
- Firmware status kept reporting `Pressed` after operator release; the operator
  did not hold IO9 for 40 seconds or longer.
- No firmware image was flashed; the runtime clear path may erase only
  `0x003bf000..0x003c0000`.

Milestone commit message:

```text
test: validate BLE runtime auth clear effect
```

## Remaining Phase 24 Work

- Validate live Wi-Fi/BLE ACK race behavior on hardware/runtime.
- Validate BOOT / IO9 still enters download mode during reset or power-on.
- Validate phone/gateway interoperability beyond the Windows central.
- Investigate and retest BOOT / IO9 release diagnostics after the runtime
  8 second clear hold. Phase 24V validated the clear effect, but the status
  stream kept reporting `Pressed` after operator release until reset.
- Validate BLE auth record replacement/update behavior when another bond is
  stored or an existing peer is updated.
- Manually accept LED3 hardware visual behavior: pairing/authorization fast
  blink, advertising-or-connected slow blink, 180 second boot BLE
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
