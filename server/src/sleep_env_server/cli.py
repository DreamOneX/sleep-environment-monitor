"""Argparse command line interface for the formal server."""

from __future__ import annotations

import argparse
import sys
from collections.abc import Sequence
from typing import TextIO

from sleep_env_server.app import create_app
from sleep_env_server.config import (
    LOG_LEVELS,
    READ_SOURCES,
    AppConfig,
    ServerConfig,
    apply_cli_overrides,
    load_app_config,
)
from sleep_env_server.discovery import (
    DISCOVERY_QUERY,
    HostResolver,
    UdpDiscoveryResponder,
    build_udp_discovery_payload,
    local_address_for_peer,
)
from sleep_env_server.output import OutputMode, ServerOutput
from sleep_env_server.storage import (
    ConfiguredMeasurementSink,
    MeasurementRecord,
    StorageMaintenanceThread,
    history_record_to_dict,
    history_summary_to_dict,
    list_configured_history_records,
    summarize_records,
)
from sleep_env_server.tui import ServerTuiApp

PRINT_OUTPUT_MODES = ("rich", "plain", "json")
HISTORY_OUTPUT_MODES = ("auto", "rich", "plain", "json")


def port_argument(value: str) -> int:
    """Parses and validates a command-line port value."""
    try:
        port = int(value, 10)
    except ValueError as exc:
        raise argparse.ArgumentTypeError("must be an integer") from exc
    if not 1 <= port <= 65535:
        raise argparse.ArgumentTypeError("must be in 1..65535")
    return port


def nonnegative_int_argument(value: str) -> int:
    """Parses and validates a non-negative integer CLI argument."""
    try:
        parsed = int(value, 10)
    except ValueError as exc:
        raise argparse.ArgumentTypeError("must be an integer") from exc
    if parsed < 0:
        raise argparse.ArgumentTypeError("must be greater than or equal to 0")
    return parsed


def build_parser() -> argparse.ArgumentParser:
    """Builds the CLI argument parser."""
    parser = argparse.ArgumentParser(prog="sleep-env-server")
    subparsers = parser.add_subparsers(dest="command", required=True)

    serve = subparsers.add_parser("serve", help="run HTTP API and UDP discovery")
    add_config_arguments(serve)
    serve.add_argument("--log-level", choices=LOG_LEVELS, default=None)
    output_group = serve.add_mutually_exclusive_group()
    output_group.add_argument("--json-log", action="store_true")
    output_group.add_argument("--no-rich", action="store_true")
    serve.set_defaults(handler=run_serve)

    tui = subparsers.add_parser("tui", help="run HTTP API and UDP discovery in a TUI")
    add_config_arguments(tui)
    tui.add_argument("--log-level", choices=LOG_LEVELS, default=None)
    tui.set_defaults(handler=run_tui)

    check = subparsers.add_parser("check-config", help="validate server configuration")
    add_config_arguments(check)
    check.set_defaults(handler=run_check_config)

    print_discovery = subparsers.add_parser(
        "print-discovery",
        help="print HTTP and UDP discovery metadata",
    )
    add_config_arguments(print_discovery)
    print_discovery.add_argument("--output", choices=PRINT_OUTPUT_MODES, default="rich")
    print_discovery.set_defaults(handler=run_print_discovery)

    history = subparsers.add_parser("history", help="print persisted measurement history")
    history.add_argument("--config", default=None)
    history.add_argument("--output", choices=HISTORY_OUTPUT_MODES, default="auto")
    history.add_argument("--read-source", choices=READ_SOURCES, default=None)
    history.add_argument("--device-id", default=None)
    history.add_argument("--start-unix-ms", type=nonnegative_int_argument, default=None)
    history.add_argument("--end-unix-ms", type=nonnegative_int_argument, default=None)
    history.add_argument("--limit", type=nonnegative_int_argument, default=None)
    history.set_defaults(handler=run_history)

    return parser


def add_config_arguments(parser: argparse.ArgumentParser) -> None:
    """Adds common server configuration flags to a subparser."""
    parser.add_argument("--config", default=None)
    parser.add_argument("--host", default=None)
    parser.add_argument("--port", type=port_argument, default=None)
    parser.add_argument("--udp-discovery-port", type=port_argument, default=None)


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    """Parses CLI arguments."""
    return build_parser().parse_args(argv)


def config_from_args(args: argparse.Namespace) -> ServerConfig:
    """Builds server configuration from parsed CLI arguments without file I/O.

    This helper remains useful for parser unit tests. Runtime command handlers
    use ``app_config_from_args`` so TOML loading and XDG generation are honored.
    """
    return apply_cli_overrides(AppConfig(), args).server


def app_config_from_args(args: argparse.Namespace) -> AppConfig:
    """Loads TOML configuration and applies CLI overrides."""
    return apply_cli_overrides(load_app_config(getattr(args, "config", None)), args)


def select_serve_output_mode(
    args: argparse.Namespace,
    *,
    stdout_isatty: bool,
    configured_mode: str = "auto",
) -> OutputMode:
    """Selects the serve output mode from parsed flags, config, and stdout state."""
    if getattr(args, "json_log", False):
        return "json"
    if getattr(args, "no_rich", False):
        return "plain"
    if configured_mode == "json":
        return "json"
    if configured_mode == "plain":
        return "plain"
    if configured_mode == "rich":
        return "rich"
    if not stdout_isatty:
        return "plain"
    return "rich"


def select_history_output_mode(
    args: argparse.Namespace,
    *,
    stdout_isatty: bool,
    configured_mode: str = "auto",
) -> OutputMode:
    """Selects the history output mode from flags, config, and stdout state."""
    requested = getattr(args, "output", "auto")
    if requested in ("json", "plain", "rich"):
        return requested
    if configured_mode in ("json", "plain", "rich"):
        return configured_mode
    if not stdout_isatty:
        return "plain"
    return "rich"


def run_serve(
    args: argparse.Namespace,
    *,
    stream: TextIO | None = None,
    stdout_isatty: bool | None = None,
) -> int:
    """Runs the HTTP API and UDP discovery responder."""
    import uvicorn

    app_config = app_config_from_args(args)
    config = app_config.server
    is_tty = sys.stdout.isatty() if stdout_isatty is None else stdout_isatty
    output = ServerOutput(
        select_serve_output_mode(
            args,
            stdout_isatty=is_tty,
            configured_mode=app_config.output.mode,
        ),
        stream=stream,
    )
    sink = ConfiguredMeasurementSink(app_config.storage)
    app = create_app(config, sink=sink, output=output, history_api=app_config.history_api)
    discovery = UdpDiscoveryResponder(config, output)
    maintenance: StorageMaintenanceThread | None = None

    output.startup(config, config.log_level)
    if app_config.storage.reconcile_on_start:
        output.storage_reconciled(copied=sink.reconcile_once())
    sink.enforce_retention_once()
    if app_config.storage.reconcile_interval_seconds > 0 and sink.stores:
        maintenance = StorageMaintenanceThread(
            sink,
            app_config.storage.reconcile_interval_seconds,
        )
        maintenance.start()
    discovery.start()
    try:
        uvicorn.run(app, host=config.host, port=config.port, log_level=config.log_level)
    except KeyboardInterrupt:
        output.shutdown_requested()
    finally:
        if maintenance is not None:
            maintenance.stop()
            maintenance.join(timeout=1.0)
        discovery.stop()
        discovery.join(timeout=1.0)
        output.stopped()
    return 0


def run_tui(args: argparse.Namespace) -> int:
    """Runs the Textual local operator UI."""
    app_config = app_config_from_args(args)
    ServerTuiApp(app_config).run()
    return 0


def run_check_config(args: argparse.Namespace, *, stream: TextIO | None = None) -> int:
    """Validates configuration without opening sockets."""
    app_config = app_config_from_args(args)
    output = ServerOutput("plain", stream=stream)
    output.config_ok(app_config.server)
    return 0


def run_print_discovery(
    args: argparse.Namespace,
    *,
    stream: TextIO | None = None,
    host_resolver: HostResolver = local_address_for_peer,
) -> int:
    """Prints the discovery document and UDP response payload."""
    app_config = app_config_from_args(args)
    config = app_config.server
    output = ServerOutput(args.output, stream=stream)
    document = config.discovery_document()
    udp_response = build_udp_discovery_payload(
        config,
        "127.0.0.1",
        host_resolver=host_resolver,
    )
    output.discovery_snapshot(
        document=document,
        udp_query=DISCOVERY_QUERY,
        udp_response=udp_response,
    )
    return 0


def run_history(
    args: argparse.Namespace,
    *,
    stream: TextIO | None = None,
    stdout_isatty: bool | None = None,
) -> int:
    """Prints local persisted history summary, recent rows, and trends."""
    app_config = app_config_from_args(args)
    is_tty = sys.stdout.isatty() if stdout_isatty is None else stdout_isatty
    output = ServerOutput(
        select_history_output_mode(
            args,
            stdout_isatty=is_tty,
            configured_mode=app_config.output.mode,
        ),
        stream=stream,
    )
    sink = ConfiguredMeasurementSink(app_config.storage)
    read_source = getattr(args, "read_source", None) or app_config.history_cli.read_source
    records = list_configured_history_records(
        sink.stores,
        read_source=read_source,
        merge_sources=app_config.history_api.merge_sources,
        merge_conflict=app_config.history_api.merge_conflict,
        device_id=getattr(args, "device_id", None),
        start_unix_ms=getattr(args, "start_unix_ms", None),
        end_unix_ms=getattr(args, "end_unix_ms", None),
        limit=1_000_000,
    )
    tail_count = (
        getattr(args, "limit", None)
        if getattr(args, "limit", None) is not None
        else app_config.history_cli.tail_count
    )
    tail = records[-tail_count:] if tail_count else []
    output.history_snapshot(
        summary=history_summary_to_dict(summarize_records(records)),
        records=[history_record_to_dict(record) for record in tail],
        trends=build_metric_trends(records, app_config.history_cli.metrics),
    )
    return 0


def build_metric_trends(
    records: list[MeasurementRecord],
    metrics: tuple[str, ...],
    *,
    width: int = 24,
) -> dict[str, str]:
    """Builds compact ASCII metric trend bars from recent history records."""
    trends: dict[str, str] = {}
    for metric in metrics:
        values = [
            float(value)
            for record in records
            if (value := getattr(record.upload, metric, None)) is not None
        ]
        trends[metric] = _metric_trend(values[-width:])
    return trends


def _metric_trend(values: list[float]) -> str:
    """Builds one compact ASCII trend line."""
    if not values:
        return ""
    if len(values) == 1:
        return f"{values[0]:.2f}"
    minimum = min(values)
    maximum = max(values)
    if minimum == maximum:
        return f"{'=' * len(values)} {minimum:.2f}"
    ramp = " .:-=+*#%@"
    span = maximum - minimum
    chars = [ramp[round((value - minimum) / span * (len(ramp) - 1))] for value in values]
    return f"{''.join(chars)} {minimum:.2f}..{maximum:.2f}"


def main(argv: Sequence[str] | None = None) -> int:
    """CLI entrypoint."""
    args = parse_args(argv)
    try:
        return args.handler(args)
    except ValueError as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
