# Server Persistence And Configuration

This document defines the planned Phase 26 server persistence, TOML
configuration, history API, and Rich display behavior.

Implementation status through Milestone 65:

- TOML loading, XDG default generation, and CLI overrides are implemented.
- SQLite and JSONL stores are implemented with canonical reads, duplicate
  handling, summaries, and JSONL compaction.
- `sleep-env-server serve` routes uploads through configured durable stores and
  applies the configured ACK policy before returning HTTP 2xx.
- Startup and periodic backfill helpers copy missing canonical records between
  enabled stores.
- Retention cleanup, authenticated history API, Rich live dashboard, and local
  history CLI remain pending for later Phase 26 milestones.

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
- Failures are reported through bounded diagnostics and dashboard status.

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
- SQLite deletes records outside the retained canonical window.
- JSONL uses atomic compaction to keep retained canonical records and required
  audit state.

Backfill runs once at startup and then on a configured background interval.
Backfill can read all enabled targets or one target, exclude targets, and apply
the configured conflict rule.

## History Read API

History API is disabled by default. When enabled, it requires:

```text
Authorization: Bearer <configured-token>
```

The first planned read endpoints are:

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/api/v1/history/measurements` | Paginated records with device and time filters. |
| `GET` | `/api/v1/history/summary` | Counts and metric summaries for a selected range. |

The read source is configurable. It may be a specific backend or a merged view
that applies configured deduplication and conflict rules.

## Rich Output

Interactive `serve` sessions use a Rich dashboard unless disabled by output
mode. JSON and plain output remain stable for scripts.

The live dashboard shows the current process receive stream:

- Recent temperature, humidity, lux, and relative sound dB trends.
- Upload count and duplicate count.
- Backend write status.
- ACK and backfill status.
- Conflict and retention warnings.

Offline history commands read configured storage and show summary, tail, and the
same default metric trends.
