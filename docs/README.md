# Documentation

This directory contains the tracked project documentation.

## Repository Layout

- [../firmware/](../firmware/): ESP32-C3 firmware package.
- [../server/](../server/): measurement ingestion server workspace.
- [../Cargo.toml](../Cargo.toml): root Cargo workspace with `firmware` as the default member.

## Start Here

1. [00-project/00-development-plan.md](00-project/00-development-plan.md) defines the phase plan and acceptance criteria.
2. [00-project/01-walkthrough.md](00-project/01-walkthrough.md) records completed milestones, validation commands, and observed results.
3. [10-firmware/00-architecture.md](10-firmware/00-architecture.md) explains the firmware structure and persistent-storage design.

## Project

- [00-project/00-development-plan.md](00-project/00-development-plan.md): phase plan, acceptance criteria, and expected commit messages.
- [00-project/01-walkthrough.md](00-project/01-walkthrough.md): completed milestones, validation commands, and observed results.

## Firmware

- [10-firmware/00-architecture.md](10-firmware/00-architecture.md): firmware architecture, task boundaries, storage flow, and status behavior.
- [10-firmware/01-hardware.md](10-firmware/01-hardware.md): board hardware facts, pin mapping, and component references.
- [10-firmware/02-conventions.md](10-firmware/02-conventions.md): firmware Rust toolchain, formatting, linting, testing, and embedded build conventions.
- [10-firmware/03-network.md](10-firmware/03-network.md): firmware network responsibilities, REST upload, discovery, time sync, and BLE readiness.
- [10-firmware/04-configuration.md](10-firmware/04-configuration.md): firmware configuration boundary and deployment knobs.

## Server

- [20-server/00-overview.md](20-server/00-overview.md): formal measurement ingestion server role and boundaries.
- [20-server/01-rest-api.md](20-server/01-rest-api.md): REST API contract between firmware and server.
- [20-server/02-toolchain.md](20-server/02-toolchain.md): Python server toolchain, style policy, formatter/linter policy, and test expectations.
- [20-server/03-cli.md](20-server/03-cli.md): formal server CLI behavior.

## Integration

- [30-integration/00-network-roadmap.md](30-integration/00-network-roadmap.md): network roadmap across firmware, server, discovery, time, and future provisioning.

## Maintenance

- Keep phase scope and future work in `00-project/00-development-plan.md`.
- Append completed work and validation evidence to `00-project/01-walkthrough.md`.
- Keep implementation structure and data-flow decisions in `10-firmware/00-architecture.md`.
- Keep firmware build and style expectations in `10-firmware/02-conventions.md`.
- Keep hardware facts in `10-firmware/01-hardware.md`.
- Keep the server API contract in `20-server/01-rest-api.md`.
- Keep server toolchain, CLI, style, and test policy in `20-server/02-toolchain.md` and `20-server/03-cli.md`.
