# BLE Upload Channel

This document records the Phase 24 Bluetooth Low Energy upload channel status,
protocol boundary, authorization policy, and remaining validation work.

## Current Status

Phase 24A is implemented as a compile-integration milestone:

- `ble-upload` enables the project BLE code path and `esp-radio/ble`.
- `radio-coex` enables `esp-radio/coex` for explicit BLE+Wi-Fi coexistence
  builds.
- `tasks::ble` defines the project-specific protocol constants and structured
  status, metadata, fragment, control, and ACK-policy helper types.
- The firmware can construct `esp_radio::ble::controller::BleConnector` from
  the ESP32-C3 `BT` peripheral and spawn a BLE task boundary.
- BLE runtime behavior remains disabled unless the firmware is built with
  `--features ble-upload`.

Phase 24B adds the hardware-independent transfer and ACK core:

- Stored payloads expose record flags to upload clients.
- Storage responses are routed separately for Wi-Fi and BLE clients.
- Storage ACK commands are sequence-checked, so a stale ACK cannot delete a
  different oldest pending record.
- `tasks::ble` models oldest-record metadata, ordered fragment delivery,
  complete-record confirmation, disconnect reset, and ACK decisions.
- BLE ACK decisions remain pure logic; the target BLE task still does not ACK
  storage at runtime.

Phase 24C adds the BOOT / IO9 pairing-window core:

- BOOT / IO9 is read only in `ble-upload` target builds.
- The pin is configured as an input with the default no-pull configuration.
- The BLE task monitors an active-low long press and opens a timed pairing
  window in pure state-machine logic.
- Hardware-independent tests cover active-low interpretation, short press,
  long press, retrigger-after-release, and window timeout behavior.

Phase 24D adds the first real GATT runtime skeleton:

- `ble-upload` now enables the TrouBLE BLE host dependency and the ESP32-C3
  build compiles a real BLE peripheral host on top of
  `esp_radio::ble::controller::BleConnector`.
- The BLE task owns a project-specific GATT service with status, record
  metadata, record fragment, and control characteristics.
- The status characteristic is readable and reflects the BLE runtime state for
  host pending, advertising, connected, and error states.
- The record metadata, record fragment, and control characteristics are present
  as the project protocol shape, but access is disabled until pairing,
  authorization, record transfer, and ACK handling are implemented.
- BOOT / IO9 pairing-window monitoring remains a separate BLE feature task so
  GATT advertising and connection waits do not stop the gesture state machine.

Phase 24E adds an authorized read-only record transfer skeleton:

- The BOOT / IO9 pairing-window task now shares its current state with the GATT
  task.
- Record metadata, record fragment, and control access require the pairing
  window to be open. Closed-window access is rejected with ATT authorization
  errors.
- Authorized metadata/control requests read the oldest pending record through
  `storage_task` using the BLE storage client.
- The BLE task can prepare project-structured metadata and ordered fragments in
  the GATT characteristics.
- `CompleteRecord` only marks the in-memory transfer session complete.
- `AckRecord` is explicitly rejected; BLE runtime still does not acknowledge or
  delete storage records.

Phase 24F adds BLE runtime ACK wiring:

- Wi-Fi and uploader tasks keep publishing their existing status `Signal`s for
  the LED/status task and also update a shared latest network/upload status
  snapshot.
- The BLE task reads that shared snapshot instead of consuming the existing
  single-consumer status `Signal`s.
- Authorized `AckRecord` now evaluates the existing BLE ACK policy against the
  latest Wi-Fi/upload snapshot.
- BLE suppresses storage ACK while Wi-Fi is connected or IP-ready and the last
  upload result is success.
- When the ACK policy permits BLE drain, BLE sends
  `StorageCommand::Ack { client: StorageClient::Ble, sequence }`.
- `storage_task` remains the only owner of flash-backed record deletion, and
  its sequence check prevents stale BLE ACKs from deleting a different oldest
  pending record.

Phase 24G adds independent radio feature selection:

- `wifi-upload` is the default firmware feature and selects `esp-radio/wifi`.
- `ble-upload` can be built without default features to compile a BLE-only
  upload boundary.
- `radio-coex` explicitly selects `ble-upload`, `wifi-upload`, and
  `esp-radio/coex` for BLE+Wi-Fi coexistence builds.
- `esp-radio/coex` is not enabled in BLE-only builds because `esp-radio 0.18.0`
  references its Wi-Fi module when coexistence is enabled.
- Target-side Wi-Fi setup, DHCP runner, and REST uploader startup are gated on
  `wifi-upload`.
- With `wifi-upload` disabled, sampling, aggregation, storage, status LED, and
  optional BLE task startup still compile.

Phase 24H adds a BLE status runtime snapshot:

- BLE status reads now combine the BLE runtime state, latest network/upload
  snapshot, pending storage record count, and latest firmware error flags.
- The LED/status task keeps using its existing single-consumer `Signal`s.
- `storage_task` publishes pending-count updates after recovery, append, and
  ACK paths.
- Aggregation and storage error paths publish the error flags used by BLE
  status.
- The BLE status characteristic is refreshed before status reads and on BLE
  runtime state transitions.

Phase 24I fixes the first hardware-observed advertising startup issue:

- The legacy advertising payload now carries flags plus the project 128-bit
  service UUID.
- The scan response now carries the complete local name.
- A hardware-independent regression test keeps both payloads within the
  31-byte legacy BLE advertising limit.
- A BLE+Wi-Fi coexistence build was flashed to the ESP32-C3 and RTT logs
  confirmed that the firmware reaches `ble advertising name=sleep-env-esp32c3`.

Phase 24J validates first central-side access:

- A Windows BLE central can discover the ESP32-C3 by the project service UUID
  and the `sleep-env-esp32c3` scan-response local name.
- The central can connect, discover the project GATT service, and read the
  status characteristic as the Phase 24H binary status frame.
- With the BOOT / IO9 pairing window closed, metadata reads, fragment reads,
  and control writes are rejected with ATT authorization errors.
- No new firmware was flashed for this validation slice; it used the Phase 24I
  BLE+Wi-Fi image already on the board.

Phase 24K validates BOOT / IO9 pairing-window entry diagnostics:

- BLE status reads now keep the original 10-byte Phase 24H status prefix and
  append pairing diagnostics: pairing state, BOOT / IO9 button state, pairing
  window remaining milliseconds, and accumulated BOOT press milliseconds.
- A Windows BLE central read the 20-byte status frame from the ESP32-C3.
- A BOOT / IO9 long press was observed as `Pressed`, accumulated past the
  2-second threshold, and opened the pairing window with about 60 seconds
  remaining.
- The validation also observed the expected retrigger behavior: after the
  pairing window expires, the same continuous press does not reopen the window
  until BOOT / IO9 is released and pressed again.

Phase 24L validates the first full BLE record transfer and ACK-mode drain:

- The Windows BLE central validation tool now lives in
  `tools/ble-watch`.
- Authorized `scan-transfer-record ... no-ack 128` read metadata, read ordered
  fragments, validated the payload CRC, and accepted `CompleteRecord` without
  requesting storage ACK.
- Authorized `scan-transfer-record ... ack 128` read metadata, read ordered
  fragments, validated the payload CRC, accepted `CompleteRecord`, and sent
  `AckRecord` while Wi-Fi upload was unavailable.
- The ACK-mode validation exercised the measurement spool flash range
  `0x003c0000..0x00400000` through `storage_task`; it did not flash firmware.
- A later post-ACK no-ACK transfer recheck was attempted but did not complete
  because the manual BOOT / IO9 pairing window was not reopened during the
  transfer wait.

Phase 24M validates BLE fragment notifications:

- The Windows BLE central validation tool has been renamed to
  `tools/ble-watch`.
- `scan-transfer-record-notify ... no-ack 128` subscribed to fragment
  notifications, requested metadata and ordered fragments, observed one
  notification per requested fragment, confirmed each notification matched the
  corresponding fragment read, validated payload CRC, and accepted
  `CompleteRecord` without requesting storage ACK.
- A `scan-disconnect-preserves-record` attempt did not reach metadata access
  because the board still reported BOOT / IO9 as continuously pressed after an
  expired authorization window. No disconnect-preservation conclusion was drawn
  from that attempt.

Phase 24N strengthens hardware-independent Wi-Fi/BLE ACK race coverage:

- A storage unit test now models Wi-Fi acknowledging the current oldest record
  before BLE attempts to acknowledge the same stale sequence.
- The stale BLE ACK returns no acknowledgement and leaves the next oldest
  record pending.
- This is not hardware validation of a live Wi-Fi/BLE runtime race.

Phase 24O adds a BLE authorization metadata and auto-pair policy boundary:

- `0x003bf000..0x003c0000` is reserved for BLE authorization metadata,
  immediately before the measurement spool.
- `storage::ble_auth` defines a header with magic, header format version,
  authorization-record-set version, record count, record-set checksum, and
  header checksum.
- On `ble-upload` builds, startup reads that header only. If the header is
  missing, invalid, empty, has a mismatched authorization-record-set version,
  has a mismatched authorization-record-set compatibility checksum, or has a
  header checksum mismatch, the pairing task can automatically open the
  temporary authorization window.
- The auto-open behavior is gated by
  `config::ble::AUTO_PAIR_ON_AUTH_RECORD_RESET`.
- This slice does not write the BLE authorization metadata sector and does not
  persist real bonded peers, pairing keys, allowlists, or authorization
  records.

Phase 24Q adds a compile-validated BLE security and bond-record persistence
path:

- `storage::ble_auth` now defines fixed-size structured authorization records
  containing peer identity address, LTK, optional IRK, security level, bonded
  flag, record CRC, and record-set checksum.
- `ble-upload` target code seeds TrouBLE security from hardware TRNG before
  host build, restores saved bond records from the BLE auth sector, requests
  security proactively only while the BOOT / IO9 pairing window is open, and
  stores a bond record on `PairingComplete`. Saved-bond reconnects rely on the
  encrypted measurement characteristics to trigger link encryption.
- Measurement metadata, fragment, and control characteristics require
  encryption. Outside the BOOT / IO9 temporary authorization window, encrypted
  connections must match a saved authorization record before accessing
  measurement records.
- No hardware pairing, reboot restore, phone/gateway interoperability, or BLE
  auth flash write/erase/update validation has been run for this path yet.

Phase 24R adds the saved-bond hardware validation path and partially validates
it on the current Windows central:

- `tools/ble-watch` adds `scan-read-metadata-now`, which connects and requests
  protected metadata without waiting for the BOOT / IO9 temporary authorization
  window. It can be run in `expect-success` mode after saved bonding is
  validated, or in `expect-reject` mode after saved authorization records are
  cleared.
- `tools/ble-watch` prints Windows pairing state for connection-oriented
  commands so validation notes can distinguish GATT access success from the
  Windows central's paired/unpaired state.
- Runtime BOOT / IO9 handling keeps the 2 second long press as the temporary
  authorization-window gesture and adds an 8 second hold as the user operation
  to clear saved BLE authorization records. The clear operation erases the BLE
  auth metadata sector `0x003bf000..0x003c0000` and reopens the temporary
  authorization window.
- The 8 second clear gesture is a runtime-only operation after firmware boot.
  Holding BOOT / IO9 during reset or power-on must continue to enter the ESP32-C3
  download mode, not BLE pairing or BLE record clearing.
- Hardware validation on 2026-05-25 confirmed Windows Custom ConfirmOnly
  pairing, BLE auth-sector write after `PairingComplete`, startup restore of
  one saved authorization record after reboot, and encrypted metadata access
  with `scan-read-metadata-now ... expect-success no-pair`.
- The same hardware run did not validate the 8 second clear gesture, LED3
  visual behavior, BOOT download-mode preservation, live Wi-Fi/BLE ACK race
  behavior, rejection after runtime clearing, or auth record replacement.

Phase 24T validates the auth metadata reset policy on hardware:

- Only the BLE auth metadata sector `0x003bf000..0x003c0000` was deliberately
  written or erased.
- Missing/erased metadata, invalid header magic, an empty current-version
  record set, records-version mismatch, compatibility-checksum mismatch, and
  header checksum mismatch all opened the temporary authorization window on
  boot.
- After the final reset-pattern authorization window closed,
  `scan-read-metadata-now ... expect-reject no-pair` confirmed that an
  unpaired central could not access protected metadata.
- This validation did not exercise BLE ACK/drain or the measurement spool
  `0x003c0000..0x00400000`.
- This validation did not validate the runtime 8 second BOOT / IO9 clear
  gesture itself.

Phase 24V validates the runtime saved-auth clear effect on hardware:

- A saved Windows authorization record was rebuilt and confirmed with
  `scan-read-metadata-now ... expect-success no-pair`.
- `scan-watch-clear-gesture ... 8000` observed the BOOT / IO9 press after a
  released state, the 8 second hold threshold, and the refreshed temporary
  authorization window.
- A following `scan-read-metadata-now ... expect-reject no-pair` confirmed the
  previous saved authorization no longer granted protected metadata access.
- The watch did not reach final release success because status continued to
  report `boot_button=Pressed` after the operator released IO9. The operator
  did not hold IO9 for 40 seconds or longer; this remains an IO9
  release-diagnostics follow-up.
- This validation did not flash firmware and did not deliberately exercise the
  measurement spool `0x003c0000..0x00400000`.

Phase 24W improves `tools/ble-watch` diagnostics for the remaining BOOT / IO9
release follow-up:

- `scan-watch-clear-gesture` remains strict and still fails unless it observes
  release before press, press-after-release, the 8 second hold threshold, the
  refreshed authorization window, and release after hold.
- The tool now prints `CLEAR_GESTURE_CLEAR_EFFECT_OBSERVED` once hold-threshold
  and refreshed-window evidence are both present.
- If the watch ends after clear-effect evidence but before final release
  observation, it prints `CLEAR_GESTURE_RELEASE_DIAGNOSTIC_MISSING`.
- This is tool evidence only; it does not change firmware behavior or close the
  release-diagnostics hardware item without a new hardware run.

Phase 24A through 24W do not change the measurement spool flash format or
measurement JSON payload shape. The GATT host/server, authorized read-only
transfer path, runtime ACK wiring, independent radio feature matrix,
structured status snapshot, board-side advertising startup, central-side
discovery, central connection, structured status read, closed-window
measurement access rejection, BOOT / IO9 authorized-window entry, full record
reads, `CompleteRecord`, and ACK-mode BLE storage drain now compile or run.
BLE notification behavior has also been hardware-validated with the Windows
central. Storage-level stale ACK protection for a Wi-Fi/BLE race is covered by
unit tests. Phase 24P additionally validates post-ACK oldest-record
advancement and live disconnect-before-Complete/ACK preservation with the
Windows central after draining enough records to avoid full-spool drop-oldest
interference. Phase 24Q compile-validates saved authorization records. Phase 24R
hardware-validates the first saved-bond path on Windows: pairing stores one
bond record in the BLE auth sector, reboot restore reports one valid restored
record, and `no-pair` encrypted metadata access succeeds through the saved
bond. Phase 24T hardware-validates auth metadata reset auto-pair behavior and
unpaired protected-metadata rejection after a reset/invalid-auth window closes.
Phase 24V hardware-validates the runtime saved-auth clear effect and rejected
protected metadata access after that runtime clear watch.
Phase 24W adds clearer tool diagnostics for the unresolved release observation
after the runtime clear hold.
Full BLE upload bring-up remains future Phase 24 work because live Wi-Fi/BLE
ACK race behavior, BOOT download-mode preservation, LED3 hardware visual
behavior, BOOT / IO9 release diagnostics after the runtime clear hold, and
record replacement are still unvalidated. Future work must validate those
remaining paths. LED3 BLE operation feedback now has a compile/unit-tested
firmware boundary, but the actual blue LED patterns have not been visually
accepted on hardware yet.

## Goals

BLE must be a real Bluetooth Low Energy feature:

- Use BLE GATT services and characteristics.
- Use structured records, fragments, status, and acknowledgements.
- Keep Wi-Fi and BLE independently enabled or disabled by firmware config.
- Let BLE upload persisted measurement records when an authorized or paired
  central is nearby.
- Preserve sampling and persistent storage when BLE or Wi-Fi is unavailable.

BLE must not be:

- Bluetooth Classic SPP.
- A transparent UART or serial-port replacement.
- Nordic UART Service style byte streaming.
- CSV or JSON text pushed through a generic serial characteristic.

## Role

The ESP32-C3 firmware acts as a BLE peripheral. The peer is a BLE central, such
as a phone, nearby gateway, or test tool. This repository does not define or
implement a mobile app or gateway in Phase 24.

The BLE central role only means "the device that connects to the board and
consumes the GATT protocol." It does not change the firmware/server REST API.

For the current Windows validation central, Windows Settings showing the board
as paired but not connected is expected when no central application is holding a
GATT session. The Phase 24 acceptance signal is `ble-watch` GATT access and
status/record behavior, not the passive connected label in Windows Settings.
If Windows still reports a saved pairing while firmware rejects protected GATT
access after auth records were cleared or reset, treat it as a stale
central-side bond and remove the Windows pairing record before re-pairing.

## Protocol Boundary

Phase 24 uses a project-specific GATT service instead of a generic UART
service.

Current characteristics:

| Characteristic | Direction | Purpose |
|---|---|---|
| Status | peripheral to central | BLE, Wi-Fi, storage, and upload state summary. |
| Record metadata | peripheral to central | Oldest pending record sequence, length, and flags. |
| Record fragment | peripheral to central | Chunked measurement record bytes with offset and length. |
| Control / ACK | central to peripheral | Request next fragment, finish record, or acknowledge receipt. |

The wire format is binary and structured. JSON measurement payload bytes may be
carried as record data, but they are framed by the BLE protocol with explicit
sequence, offset, length, and CRC metadata.

## Storage And ACK Rules

`storage_task` remains the only owner of the persistent measurement spool.

BLE and Wi-Fi upload paths read the same oldest-pending ordering. They must not
delete records directly.

Acknowledgement policy:

- Wi-Fi REST upload still acknowledges storage only after HTTP 2xx.
- If Wi-Fi upload is available and succeeding, BLE may transmit copies but must
  not acknowledge the spool.
- If Wi-Fi upload is disabled or unavailable, BLE may acknowledge exactly one
  oldest record only after the authorized central confirms complete receipt.
- If BLE disconnects mid-record, the record remains pending.
- If Wi-Fi and BLE race on the same record, storage acknowledgement must be
  idempotent and must remove at most one oldest pending record.

Wi-Fi upload is considered unavailable when Wi-Fi is disabled, disconnected, has
no IP configuration, cannot resolve an endpoint, fails transport, or receives a
non-2xx HTTP response.

## Wi-Fi Coexistence

Wi-Fi and BLE are independent features:

- Wi-Fi enabled, BLE disabled: normal REST upload path.
- Wi-Fi disabled, BLE enabled: local BLE upload path can drain records after
  authorized-central acknowledgement.
- Both enabled: Wi-Fi remains the primary durable ACK path while it is working;
  BLE can expose status and copy records without ACK.
- Both disabled: sampling and persistent storage continue until the spool fills,
  then the existing drop-oldest policy applies.

The firmware must avoid making sensor, microphone, aggregation, or storage
tasks depend on either radio path.

## Pairing Entry

The current board has no dedicated pairing button. The Phase 24 temporary
authorization entry is the BOOT / IO9 button, used only as a runtime input.

Constraints:

- Do not configure IO9 as an output.
- Do not enable an internal pull-down that fights the board's default pull-up.
- Do not require any hardware capacitor or debounce capacitor on IO9.
- Preserve the existing boot behavior where holding BOOT during reset or power
  on enters download mode.
- Continue validating download-mode preservation before relying on IO9 for
  deployed user-facing pairing.

The implemented temporary authorization gesture is an active-low runtime long
press after boot. About 2 seconds opens the temporary authorization window.
Continuing the same runtime hold to about 8 seconds requests clearing saved BLE
authorization records, erasing `0x003bf000..0x003c0000` and reopening the
temporary authorization window. A boot-time held button must continue to mean
download mode, not BLE pairing or BLE record clearing. Older manual validation
notes that say to hold BOOT / IO9 for 3 seconds mean only "hold long enough to
cross the about-2-second threshold"; there is no 3 second firmware window. The
temporary authorization window lasts about 60 seconds after it opens.

## Observable BLE Status

General board LED semantics are defined in
[00-architecture.md](00-architecture.md#4-firmware-led-semantics). This section
only records the BLE overlay requirement for Phase 24.

Phase 24 completion requires BLE-related operations to be visible on blue LED3
without taking over red LED2 heartbeat semantics.

Minimum BLE overlay behavior:

- Fast-blink LED3 while the BLE pairing or authorization window is open.
- Slow-blink LED3 while BLE is advertising or connected.
- Return LED3 to the normal firmware status policy when no BLE indication
  window is active.

BLE indication timing:

- For the first 180 seconds after boot, LED3 must represent BLE status when BLE
  is enabled.
- After any BOOT / IO9 press or pairing/authorization trigger, LED3 must
  represent BLE status for at least the next 10 seconds.
- If the trigger opens a pairing or authorization window longer than 10
  seconds, LED3 must continue the pairing-window fast blink for the full open
  window.

The LED3 pattern decision and timing-window logic are implemented as
hardware-independent status mapping with unit tests. Manual integration must
still verify the actual active-low blue LED behavior on hardware.

## Security

BLE measurement access requires pairing or an equivalent authorization step.

Phase 24 documentation and implementation enforce or track these security
rules:

- Advertising does not contain measurement data or credentials.
- Unpaired and unauthorized centrals cannot read measurement records.
- Pairing state and any authorization material are handled explicitly.
- Current hardware-validated Phase 24 authorization is the volatile BOOT / IO9
  window, the Phase 24R Windows saved-bond restore path, the Phase 24T auth
  metadata reset policy, and the Phase 24V runtime clear effect. The saved-bond
  path stores records in
  `0x003bf000..0x003c0000`; missing, empty, invalid,
  records-version-mismatched, compatibility-checksum-mismatched, and
  header-checksum-mismatched auth metadata auto-opens the authorization window
  on boot. The runtime clear path can erase the same sector and reopen the
  temporary authorization window. Future work must validate record
  replacement/update behavior and investigate the BOOT / IO9 release-diagnostics
  mismatch observed after the runtime clear hold.
- Debug-only open access, if used for bring-up, is gated by config and clearly
  marked as unsafe for deployed firmware.

## Hardware-Independent Coverage

Phase 24 has hardware-independent tests for:

- BLE protocol frame encode/decode.
- Fragment ordering and bounds checks.
- ACK handling when Wi-Fi is available versus unavailable.
- Disconnect before ACK preserving pending records.
- Idempotent ACK behavior when Wi-Fi and BLE observe the same record.
- BLE feature enable/disable config selection.
- BOOT / IO9 pairing gesture state logic.
- BOOT / IO9 runtime auth-record clear gesture timing.
- BLE authorization record encode/load/store/clear behavior and auto-pair
  policy.
- LED3 BLE status pattern selection and boot/BOOT-trigger indication-window
  timing.

Remaining hardware checks should confirm live Wi-Fi/BLE coexistence races,
BLE auth record replacement/update behavior, LED3 BLE status indication timing
and patterns, BOOT / IO9 release diagnostics after the runtime clear hold, and
that BOOT still enters download mode during reset or power-on.
