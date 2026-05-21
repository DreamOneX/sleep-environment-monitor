# Agent Entry

Start with [docs/README.md](docs/README.md).

## Documentation Map

- [docs/00-project/00-development-plan.md](docs/00-project/00-development-plan.md): phased work plan, acceptance criteria, and expected commit messages.
- [docs/00-project/01-walkthrough.md](docs/00-project/01-walkthrough.md): completed milestone log with validation commands and observed hardware results.
- [docs/10-firmware/00-architecture.md](docs/10-firmware/00-architecture.md): firmware architecture, task boundaries, storage flow, and status behavior.
- [docs/10-firmware/01-hardware.md](docs/10-firmware/01-hardware.md): tracked board hardware facts, pin mapping, and component references.
- [docs/10-firmware/02-conventions.md](docs/10-firmware/02-conventions.md): Rust toolchain, formatting, linting, testing, and embedded build conventions.
- [docs/10-firmware/03-network.md](docs/10-firmware/03-network.md): firmware networking responsibilities, REST upload, discovery, time sync, and BLE upload boundary.
- [docs/10-firmware/04-configuration.md](docs/10-firmware/04-configuration.md): planned firmware configuration boundary for Phase 21.
- [docs/10-firmware/05-ble.md](docs/10-firmware/05-ble.md): planned BLE upload channel, GATT protocol boundary, pairing entry, and Wi-Fi coexistence rules.
- [docs/20-server/00-overview.md](docs/20-server/00-overview.md): measurement ingestion server role, current temporary receiver, and formal server direction.
- [docs/20-server/01-rest-api.md](docs/20-server/01-rest-api.md): REST API contract between firmware and server.
- [docs/20-server/02-toolchain.md](docs/20-server/02-toolchain.md): planned Python server toolchain, style policy, formatter/linter policy, and unit-test expectations.
- [docs/20-server/03-cli.md](docs/20-server/03-cli.md): planned formal server CLI behavior.
- [docs/30-integration/00-network-roadmap.md](docs/30-integration/00-network-roadmap.md): cross-component network roadmap.

## Repository Layout

- [firmware/](firmware/): ESP32-C3 firmware package.
- [server/](server/): measurement ingestion server workspace.
- [docs/](docs/): tracked project documentation.

## Working Rules

- Follow `docs/00-project/00-development-plan.md` for phase scope and done criteria.
- Record completed milestones in `docs/00-project/01-walkthrough.md`.
- Commit each completed milestone with a descriptive message.
- Before flash-write validation, state the exact flash range being exercised.
- Prefer hardware-independent tests for pure logic, then target builds, then hardware checks when the phase requires them.
- Keep documentation links relative to the file location after moving files.

## Standard Verification

```bash
cargo fmt
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
```
