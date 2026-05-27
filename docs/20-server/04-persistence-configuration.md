# Server Persistence And Configuration

This document defines the implemented server persistence, TOML configuration,
history API, and local operator display behavior.

Implementation status through Phase 28:

- TOML loading, XDG default generation, and CLI overrides are implemented.
- SQLite and JSONL stores are implemented with canonical reads, duplicate
  handling, summaries, and JSONL compaction.
- `sleep-env-server serve` routes uploads through configured durable stores,
  applies the configured ACK policy before returning HTTP 2xx, and registers
  Bearer-protected history read endpoints when enabled.
- Startup and periodic backfill helpers copy missing canonical records between
  enabled stores.
- Phase 26 Rich serve output showed a local live measurements/trends dashboard.
  Phase 27 replaces that serve chart with a dedicated Textual TUI entry point,
  and Phase 28 makes Rich service logging explicit through `serve --rich-log`.
- `sleep-env-server history` prints summary, recent rows, and metric trends.
- Retention cleanup is enforced on startup and in the periodic maintenance loop.

The implementation must preserve the existing firmware-facing REST and UDP
contract. Firmware still uploads measurements to `POST /api/v1/measurements`
and treats HTTP 2xx as the only upload ACK condition.

## Documentation-First Rule

Phase 26 is implemented in milestones. Each milestone must:

- Update the relevant docs before or with implementation.
- Record completion evidence in [../00-project/01-walkthrough.md](../00-project/01-walkthrough.md).
- End with at least one git commit.

## TOML Configuration

The server reads TOML configuration before applying CLI overrides.

Default path behavior:

- If `--config PATH` is not supplied, read
  `$XDG_CONFIG_HOME/sleep-env-server/config.toml`.
- If `XDG_CONFIG_HOME` is unset, read
  `~/.config/sleep-env-server/config.toml`.
- If the default file does not exist, every command creates it from
  [../../server/config.example.toml](../../server/config.example.toml).
- If `--config PATH` is supplied, the file must already exist.

CLI flags override TOML fields for the active invocation. Relative data paths
inside generated configuration are resolved against the process current working
directory.

## Storage Backends

The server supports two durable backends:

- SQLite: structured store, default enabled.
- JSONL: append-oriented store, default disabled.

The backends may be enabled independently, together, or not at all. Each
accepted record stores:

- The schema-version-1 measurement payload.
- Parsed measurement fields for querying and summaries.
- `device_id` and `sequence`.
- Server receive time.
- The time source used for display or retention.
- Duplicate, conflict, and backend status metadata.

The in-memory sink remains useful for current-process status and for operation
when storage does not participate in ACK.

## ACK Policy

`[storage].required_for_ack` controls whether durable storage participates in
HTTP upload ACK.

When `required_for_ack = false`:

- A valid upload may return `204` even if all storage writes fail.
- Backend-level ACK settings are ignored.
- Failures are reported through bounded diagnostics and, in TUI mode, the
  operator event/status panels.

When `required_for_ack = true`:

- Backend-level `sufficient_for_ack` is checked first.
- If any enabled backend marked `sufficient_for_ack = true` stores the record
  successfully, the upload returns `204`.
- If no sufficient backend succeeds, every enabled backend marked
  `required_for_ack = true` must store the record successfully.
- If no ACK path can succeed, the upload returns non-2xx.

A backend may be both required and sufficient. If duplicate handling rejects a
record and that rejection prevents the ACK policy from being satisfied, the
upload returns non-2xx.

## Policy Profiles

Storage targets use policy profiles. Profiles support one parent and must reject
inheritance loops.

Supported deduplication strategies:

- `keep_first`: first valid record for a key remains canonical.
- `keep_last`: all versions may remain auditable, but the last version is
  canonical for reads.
- `overwrite`: later versions replace older canonical data. JSONL compaction may
  remove older versions.
- `reject`: conflicting duplicates are storage failures.

Retention limits:

- `time_limit` may be a duration string such as `10d` or `-1`.
- `size_limit` may be a size string such as `100MB` or `-1`.
- `-1` means unlimited.
- Time limits currently support `ms`, `s`, `m`, `h`, and `d` suffixes.
- Size limits currently support byte, KB/MB/GB, and KiB/MiB/GiB suffixes.
- SQLite deletes canonical records outside the retained window.
- JSONL uses atomic rewrite/compaction to keep retained canonical records.
- Size retention is based on the canonical JSON event byte budget and retains
  newest canonical records first.

Backfill runs once at startup and then on a configured background interval.
Backfill can read all enabled targets or one target, exclude targets, and apply
the configured conflict rule.

## History Read API

History API is disabled by default. When enabled, it requires:

```text
Authorization: Bearer <configured-token>
```

Implemented read endpoints are:

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/api/v1/history/measurements` | Paginated records with device and time filters. |
| `GET` | `/api/v1/history/summary` | Counts and metric summaries for a selected range. |

The read source is configurable. It may be a specific backend or a merged view
that applies configured deduplication and conflict rules.

Supported query parameters:

- `device_id`
- `start_unix_ms`
- `end_unix_ms`
- `limit` and `offset` for `/history/measurements`

`/history/measurements` returns `records`, `limit`, and `offset`. Each record
contains receive/display time metadata plus the original schema-version-1
payload. `/history/summary` returns count, device ids, first/last receive time,
and metric averages.

## Local Operator Output

`sleep-env-server serve` is the service/process entry point. It emits bounded
plain logs by default, JSONL logs with `--json-log`, or Rich logs with
`--rich-log`, and does not render live measurement charts.

`sleep-env-server tui` is the full-screen local operator entry point. It starts
the same HTTP API, UDP discovery responder, storage, backfill, and retention
maintenance as `serve`, then presents the current process receive stream:

- Top-line temperature, humidity, lux, and relative sound dB metric cards.
- Recent accepted measurements.
- Recent temperature, humidity, lux, and relative sound dB trends.
- Duplicate status for displayed rows.
- Bounded startup, upload, storage, discovery, and shutdown diagnostics.
- Built-in help for `q`, `Ctrl+C`, `c`, `r`, and `?`.

TUI visual configuration lives under `[tui]`:

- `theme = "catppuccin-mocha"` selects the default Catppuccin Mocha palette.
- `theme = "graphite"` remains accepted for compatibility with older local
  configuration.
- `transparent = true` uses transparent Textual backgrounds so terminals with
  window transparency can show through. This is terminal background
  transparency, not an operating-system window alpha setting.

Offline history commands read configured storage and show summary, tail, and the
same default metric trends.
