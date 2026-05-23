# BLE Upload Channel

This document defines the planned Phase 24 Bluetooth Low Energy upload channel.

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

Phase 24A through 24J do not change the flash format or measurement JSON
payload shape. The GATT host/server, authorized read-only transfer path,
runtime ACK wiring, independent radio feature matrix, structured status
snapshot, board-side advertising startup, central-side discovery, central
connection, structured status read, and closed-window measurement access
rejection now compile or run, but real pairing/security or authorized-window
entry, live BLE record transfer, notifications, and BLE storage-drain behavior
have not been validated yet. Full BLE upload bring-up remains future Phase 24
work.

## Goals

BLE must be a real Bluetooth Low Energy feature:

- Use BLE GATT services and characteristics.
- Use structured records, fragments, status, and acknowledgements.
- Keep Wi-Fi and BLE independently enabled or disabled by firmware config.
- Let BLE upload persisted measurement records when a paired central is nearby.
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

## Protocol Boundary

Phase 24 should define a project-specific GATT service instead of using a
generic UART service.

Planned characteristics:

| Characteristic | Direction | Purpose |
|---|---|---|
| Status | peripheral to central | BLE, Wi-Fi, storage, and upload state summary. |
| Record metadata | peripheral to central | Oldest pending record sequence, length, and flags. |
| Record fragment | peripheral to central | Chunked measurement record bytes with offset and length. |
| Control / ACK | central to peripheral | Request next fragment, finish record, or acknowledge receipt. |

The wire format should be binary or tightly structured. If JSON measurement
field fragments are reused internally, they must be framed by the BLE protocol
with explicit sequence, offset, length, and CRC or equivalent integrity checks.

## Storage And ACK Rules

`storage_task` remains the only owner of the persistent measurement spool.

BLE and Wi-Fi upload paths read the same oldest-pending ordering. They must not
delete records directly.

Acknowledgement policy:

- Wi-Fi REST upload still acknowledges storage only after HTTP 2xx.
- If Wi-Fi upload is available and succeeding, BLE may transmit copies but must
  not acknowledge the spool.
- If Wi-Fi upload is disabled or unavailable, BLE may acknowledge exactly one
  oldest record only after the paired central confirms complete receipt.
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
  paired-central acknowledgement.
- Both enabled: Wi-Fi remains the primary durable ACK path while it is working;
  BLE can expose status and copy records without ACK.
- Both disabled: sampling and persistent storage continue until the spool fills,
  then the existing drop-oldest policy applies.

The firmware must avoid making sensor, microphone, aggregation, or storage
tasks depend on either radio path.

## Pairing Entry

The current board has no dedicated pairing button. The planned pairing entry is
the BOOT / IO9 button, used only as a runtime input.

Constraints:

- Do not configure IO9 as an output.
- Do not enable an internal pull-down that fights the board's default pull-up.
- Do not require any hardware capacitor or debounce capacitor on IO9.
- Preserve the existing boot behavior where holding BOOT during reset or power
  on enters download mode.
- Confirm by hardware validation before relying on IO9 for user-facing pairing.

The exact gesture should be defined during implementation, for example a
runtime long press after boot. A boot-time held button must continue to mean
download mode, not BLE pairing.

## Security

BLE measurement access requires pairing or an equivalent authorization step.

Phase 24 documentation and implementation should ensure:

- Advertising does not contain measurement data or credentials.
- Unpaired centrals cannot read measurement records.
- Pairing state and any authorization material are handled explicitly.
- Debug-only open access, if used for bring-up, is gated by config and clearly
  marked as unsafe for deployed firmware.

## Future Implementation Tests

Phase 24 implementation should add hardware-independent tests for:

- BLE protocol frame encode/decode.
- Fragment ordering and bounds checks.
- ACK handling when Wi-Fi is available versus unavailable.
- Disconnect before ACK preserving pending records.
- Idempotent ACK behavior when Wi-Fi and BLE observe the same record.
- BLE feature enable/disable config selection.
- BOOT / IO9 pairing gesture state logic.

Hardware checks should confirm BLE advertising, pairing, GATT transfer,
disconnect recovery, Wi-Fi coexistence, and that BOOT still enters download
mode during reset or power-on.
