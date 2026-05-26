# Project Todo List

Last updated: 2026-05-26.

This file is the current project task ledger. Any future task status change,
acceptance result, or newly discovered risk must be synchronized here in the
same change that records the evidence elsewhere.

## Current Baseline

Phase 24Z, the follow-up BLE+Wi-Fi coexistence heap fix, and the live
Wi-Fi/BLE ACK suppression validation are implemented. The current hardware
evidence confirms the Windows saved-bond path, BLE auth metadata reset policy,
runtime saved-auth clear effect, BLE+Wi-Fi startup without the previous Wi-Fi
controller initialization error `257`, and live BLE ACK suppression while
Wi-Fi/IP is ready and REST upload is succeeding. Pure auth-record upsert policy
coverage is also in place, and firmware logs BOOT / IO9 initial and transition
samples. The Windows `ble-watch` tool has been hardened against stale
WinRT/GATT cache failures and emits clear runtime clear/release diagnostics:

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
- A runtime BOOT / IO9 clear watch with the explicit runtime GPIO9 internal
  pull-up firmware observed `Released`, `Pressed`, the 8 second hold
  threshold, refreshed authorization window, and final `Released` after hold.
  A following
  `scan-read-metadata-now ... expect-reject no-pair` confirmed the previous
  saved authorization no longer granted protected metadata access.
- Pure tests cover BLE auth-record upsert behavior for same identity-address
  update, same IRK update, append, full-capacity replacement of the oldest
  record at index `0`, stored-count clamping, and zero-capacity handling.
- The BLE pairing task logs BOOT / IO9 initial and Pressed/Released transition
  samples with accumulated press milliseconds and pairing-window remaining
  milliseconds. Phase 24Z matched those firmware logs against `ble-watch`
  release-after-hold output.
- A 2026-05-25 hardware review corrected BOOT / IO9 assumptions: the board has
  no discrete IO9 pull-up resistor, ESP32-C3 boot/strap weak pull-up is a boot
  phase fact and must not be assumed at runtime, and the BOOT button has an
  IO9-to-GND capacitor in parallel. BLE feature builds now configure the
  runtime GPIO9 internal pull-up explicitly when reading BOOT / IO9.
- BLE+Wi-Fi feature builds now add an extra internal heap region. A hardware
  `cargo run --target riscv32imc-unknown-none-elf --features
  ble-upload,radio-coex` run showed `wifi connecting`, `wifi connected`, an
  IPv4 config, BLE advertising, and a `ble-watch scan-read-status` result of
  `runtime=Connected network=IpReady upload=TimeFailed pending=32`, resolving
  the Phase 24Z Wi-Fi init error `257` as a startup blocker.
- With the formal server running, firmware logs showed discovery, time sync,
  and REST upload success. `scan-transfer-record-now 30 sleep-env-esp32c3 ack
  128 auto-pair` transferred sequence `136853`; the central requested ACK, and
  firmware logged `ble storage ACK suppressed sequence=136853
  network_state=IpReady upload_result=Success`. This accepts the live
  Wi-Fi/BLE ACK-policy behavior for the observed hardware case.
- A 2026-05-26 existing Windows-central re-pair run accepted real existing-peer
  auth-record update behavior. RTT logs showed `ble auth record updated
  index=0` and `ble auth bond stored count=1 offset=0x003bf000 len=4096`, and
  `scan-read-metadata-now ... expect-success auto-pair` read protected metadata
  through the refreshed bond.
- 2026-05-26 manual LED3 observation accepted the BLE slow-blink and
  fast-blink patterns for the exercised states. While `ble-watch
  scan-watch-status` held a BLE connection with `pairing=Closed`, the operator
  reported `LED3 慢闪`. During a BOOT / IO9-triggered `pairing=Open` window,
  the operator reported `LED3 快闪`.

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
- `scan-watch-status` can reconnect after a recoverable stale WinRT/GATT
  status-read object during long visual-observation runs.
- `scan-watch-clear-gesture` now prints
  `CLEAR_GESTURE_CLEAR_EFFECT_OBSERVED` after the hold threshold plus refreshed
  pairing-window evidence, and
  `CLEAR_GESTURE_RELEASE_DIAGNOSTIC_MISSING` if that evidence exists but final
  release is not observed before timeout.
- `scan-unpair` was used to recover Windows stale pairing/cache state when
  needed. After Phase 24Z, firmware auth metadata is cleared or missing and
  protected metadata access is rejected when the temporary authorization window
  is closed.

Phase 24 is complete. The final acceptance pass manually accepted the
180 second post-boot LED3 BLE status window, accepted BOOT / IO9 download-mode
preservation during reset or power-on, and marks phone/gateway interoperability
beyond the Windows `ble-watch` central as `skipped / not planned` for Phase 24.

Current runtime clear-gesture state:

- Phase 24V first proved the clear effect but exposed a release-diagnostics
  mismatch where status continued to report `Pressed` after operator release.
  The operator did not hold IO9 for 40 seconds or longer; that observation is
  historical and drove the Phase 24Y/24Z retest.
- Phase 24Z flashed a BLE+Wi-Fi firmware build with GPIO9 configured as
  input-only plus MCU internal pull-up at runtime. The declared app-image flash
  range was approximately `0x00010000..0x003bf000`.
- `scan-watch-clear-gesture 30 sleep-env-esp32c3 180 8000` observed
  `CLEAR_GESTURE_RELEASED`, `CLEAR_GESTURE_PRESSED_AFTER_RELEASE`,
  `CLEAR_GESTURE_HOLD_THRESHOLD pressed_ms=8100`, refreshed pairing window
  `remaining_ms=59900`, final release after hold, and
  `CLEAR_GESTURE_RESULT success=True`.
- Firmware RTT logs matched the central result with
  `ble boot/io9 transition state=Pressed`, `ble auth records clear requested
  pressed_ms=8000`, `ble auth records cleared offset=0x003bf000 len=4096`, and
  `ble boot/io9 transition state=Released`.
- A follow-up `scan-read-metadata-now 30 sleep-env-esp32c3 expect-reject
  no-pair` after the refreshed authorization window expired reported
  `metadata_success=False rejected=True`, proving the previous saved
  authorization no longer grants protected metadata access.
- The latest manual BOOT / IO9 LED3 fast-blink check was held long enough to
  trigger the saved-auth clear threshold again. RTT logs showed `ble auth
  records clear requested pressed_ms=8000` and `ble auth records cleared
  offset=0x003bf000 len=4096`. A following hard reset restored the normal
  application BLE status path after Windows-side stale pairing was removed.

Windows Settings showing the board as paired but not connected is expected
when `ble-watch` or another central application is not holding a GATT session.
If Windows still reports paired while firmware rejects protected access after
BLE auth records are cleared or reset, remove the Windows-side pairing with
`scan-unpair` before re-pairing.

## Phase 24 Remaining Acceptance

- [x] Manually accept LED3 visual behavior for the exercised BLE states:
  advertising-or-connected slow blink and BOOT / IO9-triggered pairing or
  authorization fast blink. The fast-blink run also covers the BOOT / IO9
  trigger case where the pairing window remains open longer than 10 seconds.
- [x] Manually accept the 180 second post-boot BLE status window on LED3 after
  reset or power-on.
- [x] Validate BOOT download-mode preservation during reset or power-on.
  Holding BOOT / IO9 at reset or power-on must enter ESP32-C3 download mode,
  not BLE pairing or BLE record clearing.
- [x] Mark phone/gateway interoperability beyond the current Windows BLE
  central validation tool as `skipped / not planned` for Phase 24.

## Phase 25 Refactor And Maintenance

Phase 25 is complete. The work was behavior-preserving and did not flash
firmware or deliberately write/erase flash. BLE UUIDs, status/metadata /
fragment/control frame bytes, ACK policy, Wi-Fi HTTP 2xx ACK semantics, flash
ranges, REST JSON payload shape, BOOT / IO9 behavior, and LED3 BLE overlay
rules remain frozen.

- [x] Freeze the BLE UUIDs, status/metadata/control frame bytes, ACK policy,
  Wi-Fi HTTP 2xx ACK semantics, flash ranges, and measurement payload JSON
  shape before refactoring.
- [x] Split `tools/ble-watch/Program.cs` into BLE profile constants, scanner,
  protocol helpers, models, protected GATT helpers, WinRT pairing helpers, and
  output formatting while preserving CLI commands and output labels.
- [x] Split `firmware/src/tasks/upload.rs` into pure endpoint/JSON/HTTP/parse
  and time logic plus target runtime uploader code.
- [x] Split `firmware/src/tasks/ble.rs` into profile, status, protocol,
  transfer, ACK, pairing, auth, storage bridge, GATT, and runtime modules while
  preserving public paths.
- [x] Split `firmware/src/storage/spool.rs` into memory queue, wire codec, and
  flash-backed log modules while preserving public paths.
- [x] Split `firmware/src/storage/ble_auth.rs` into types/status, upsert
  policy, header codec, record codec, and flash load/store/clear modules while
  preserving public paths.
- [x] Split `firmware/src/tasks/storage.rs` into backlog/metrics,
  command/response protocol, and target runtime modules while preserving public
  paths.
- [x] Keep `docs/10-firmware/05-ble.md` as the BLE ACK/security rule authority
  during the split.
- [x] Keep LED overlay rules centralized and avoid duplicate status policy
  implementations.
- [ ] Consider later, lower-priority splits for `firmware/src/drivers/flash.rs`
  if flash layout, ROM adapter, and smoke-test code continue to grow.

## Phase 26 Server Persistence, Configuration, And History

Phase 26 is complete for hardware-free server implementation and validation.
No human-assisted or hardware verification was required for this server phase.

- [x] Added documentation-first Phase 26 persistence/configuration/history plan
  and tracked TOML example configuration.
- [x] Implemented XDG/default TOML loading, default config generation, explicit
  `--config`, and CLI overrides.
- [x] Implemented SQLite and JSONL storage with duplicate handling, canonical
  reads, summaries, JSONL compaction, and retention cleanup.
- [x] Implemented configurable storage ACK policy and durable upload routing.
- [x] Implemented startup/periodic backfill with source, exclude, and conflict
  handling.
- [x] Implemented Bearer-protected history read endpoints.
- [x] Implemented Rich local serve output and offline `history` CLI summary,
  tail, and metric trends.
- [x] Recorded milestone validation evidence and committed every Phase 26
  milestone separately.

## Phase 27 Server TUI Runtime

Phase 27 moves local interactive server operation to a dedicated Textual TUI.
`serve` remains the scriptable service entry point and should no longer render
live measurement charts.

- [x] Added documentation-first Phase 27 TUI plan.
- [x] Add Textual dependency and `sleep-env-server tui` command shell.
- [x] Route upload, storage, discovery, and shutdown events into the TUI.
- [ ] Simplify `serve` output to logs/events and remove live chart rendering.
- [ ] Cover TUI smoke behavior, event bridge delivery, and service logging with
  automated hardware-free tests.
- [ ] Record milestone validation evidence and commit every Phase 27 milestone
  separately.

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
- [ ] Continue hardware validation for BLE auth record replacement/update paths
  beyond the pure Phase 24X upsert policy, the first observed bond write,
  Phase 24T reset-pattern validation, Phase 24Z runtime clear/release
  validation, and the 2026-05-26 existing-peer update run. A distinct
  second-central append or full-capacity replacement run remains useful future
  coverage but is no longer the open Phase 24 existing-peer update item.
- [ ] Recheck BOOT / IO9 electrical and UX behavior before treating it as a
  deployed user-facing pairing or clearing control. Current board facts: no
  discrete IO9 pull-up, ESP32-C3 weak pull-up only during boot/strap sampling,
  runtime firmware must explicitly enable the MCU internal pull-up, and BOOT
  has an IO9-to-GND capacitor in parallel.
- [ ] Validate mobile phone and gateway behavior once a real central app or
  gateway exists. This is explicitly outside Phase 24 and currently
  `not planned`.
