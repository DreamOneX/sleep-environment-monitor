# Firmware

This directory contains the ESP32-C3 firmware package.

The root Cargo workspace has `firmware` as its default member. Use root-level Cargo commands for normal development so build output continues to land in the shared root `target/` directory.

## Common Commands

Run from the repository root unless otherwise noted:

```bash
cargo fmt
cargo test --lib
cargo build --target riscv32imc-unknown-none-elf
cargo clippy --all-targets
cargo clippy --target riscv32imc-unknown-none-elf
```

The package name and firmware binary name remain `sleep-environment-monitor`.
