# Handoff

Last updated: 2026-05-24.

## Current State

Phase 24O is implemented and verified in the current working tree. The
milestone adds BLE authorization metadata header parsing plus a config-gated
auto-pair-on-boot policy.

No firmware image has been flashed during this handoff state. No new hardware
flash write/erase validation has been run.

Important flash ranges:

- BLE auth metadata sector: `0x003bf000..0x003c0000`. Phase 24O target code
  reads only the header; it does not write or erase this sector.
- Measurement spool: `0x003c0000..0x00400000`. Phase 24O should not change the
  spool format or measurement JSON payload shape.

Before any future flash-write validation, state the exact range being
exercised.

## Implemented

- `firmware/src/board.rs` reserves a 4 KiB BLE auth metadata sector before the
  existing measurement spool.
- `firmware/src/drivers/flash.rs` validates both BLE auth and spool regions and
  adds a target-only read-only `RomBleAuthFlash`.
- `firmware/src/storage/ble_auth.rs` defines the BLE auth metadata header,
  checksum/version inspection, and auto-pair policy helper with unit tests.
- `firmware/src/config.rs` adds BLE auth record version/checksum policy
  constants, the auto-pair switch, and stricter Wi-Fi credential validation.
- `firmware/src/tasks/ble.rs` reads the BLE auth header at startup in
  `ble-upload` builds and opens the RAM-only authorization window when policy
  requires it.
- `firmware/src/tasks/wifi.rs` now keeps the ESP radio authentication-method
  adapter at the Wi-Fi use site instead of in `config.rs`.
- Documentation records Phase 24O and the current `tools/ble-watch` path.

Phase 24O does not implement real BLE bonding, pairing-key storage, peer
allowlists, authorization-record persistence, or user-controlled clearing. The
current authorization state remains RAM-only.

## Subagents

All requested subagents have completed.

- Documentation drift check: requested Phase 24O walkthrough entry, architecture
  Phase 24A-O updates, config doc updates, README/BLE wording cleanup, and
  current `tools/ble-watch` command paths.
- Wi-Fi config check: default `FZU` open network is acceptable only as bring-up
  default; Wi-Fi validation should use byte limits and 64-byte hex PSK checks.
- Duplicate-code check: recommended sharing ROM flash range/read helpers and
  centralizing BLE pairing-window open transition; these have been addressed in
  the working tree.
- `cfg(target_arch = "riscv32")` check: most cfgs are valid embedded/host-test
  boundaries; Phase 25 should split `tasks/ble.rs`; HAL adapters should live at
  use sites where practical.
- Phase 25 refactor plan: do equivalent module-boundary refactors only. Freeze
  BLE UUIDs, status/metadata/control frame bytes, Wi-Fi HTTP 2xx ACK semantics,
  BLE ACK policy, flash ranges, and payload JSON shape.
- Architecture file-tree/config subagent: updated only
  `docs/10-firmware/00-architecture.md` with directory-first file tree,
  `config.rs` ownership details, and BLE auth metadata facts.

## Verification

These commands passed after the latest code and documentation changes:

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

- `cargo test --lib`: `168 passed; 0 failed`.
- Normal ESP32-C3 target build: passed.
- BLE+Wi-Fi coexistence ESP32-C3 target build: passed.
- Host clippy, normal target clippy, and BLE+coex target clippy: passed.
- `git diff --check`: passed.

Milestone commit message:

```text
feat: add BLE auth metadata auto-pair policy
```

## Phase 25 Notes

Phase 25 should start with documentation and baseline freezing, then split long
files without behavior changes:

- `tools/ble-watch/Program.cs`: split CLI, BLE profile constants, scanner, GATT
  client, transfer client, protocol helpers, and models.
- `firmware/src/tasks/upload.rs`: split pure JSON/HTTP/discovery/time logic from
  target runtime uploader.
- `firmware/src/tasks/ble.rs`: split protocol, transfer, pairing, auth,
  storage bridge, GATT, and target runtime modules.
- Later candidates: `firmware/src/storage/spool.rs` and
  `firmware/src/tasks/storage.rs`.

Phase 25 must not change BLE UUIDs, frame layouts, ACK conditions, Wi-Fi retry /
discovery / time-sync behavior, flash ranges, or measurement payload JSON shape.
