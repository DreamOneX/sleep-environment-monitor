# BLE Upload Channel

This document defines the planned Phase 24 Bluetooth Low Energy upload channel.

## Current Status

Phase 24A is implemented as a compile-integration milestone:

- `ble-upload` enables the project BLE code path and `esp-radio/ble`.
- `radio-coex` enables `esp-radio/coex`; `ble-upload` selects it so BLE feature
  builds compile with the existing Wi-Fi path still present.
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

Phase 24A and 24B do not change the flash format or measurement JSON payload
shape. No GATT server, advertising, pairing, central connection, live BLE
record transfer, or BLE storage-drain behavior has been validated yet. Full BLE
runtime bring-up remains future Phase 24 work.

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
