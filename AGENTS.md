# Agent Entry

Start with [docs/README.md](docs/README.md).

## Documentation Map

- [docs/development_plan.md](docs/development_plan.md): phased work plan, acceptance criteria, and expected commit messages.
- [docs/walkthrough.md](docs/walkthrough.md): completed milestone log with validation commands and observed hardware results.
- [docs/architecture.md](docs/architecture.md): firmware architecture, task boundaries, storage flow, and status behavior.
- [docs/conventions.md](docs/conventions.md): Rust toolchain, formatting, linting, testing, and embedded build conventions.
- [docs/hardware_information.md](docs/hardware_information.md): tracked board hardware facts, pin mapping, and component references.

## Repository Layout

- [firmware/](firmware/): ESP32-C3 firmware package.
- [server/](server/): measurement ingestion server workspace.
- [docs/](docs/): tracked project documentation.

## Working Rules

- Follow `docs/development_plan.md` for phase scope and done criteria.
- Record completed milestones in `docs/walkthrough.md`.
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
