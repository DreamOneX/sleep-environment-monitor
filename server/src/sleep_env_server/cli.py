"""Argparse command line interface for the formal server."""

from __future__ import annotations

import argparse
import sys
from collections.abc import Sequence
from typing import TextIO

from sleep_env_server.app import create_app
from sleep_env_server.config import (
    LOG_LEVELS,
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
from sleep_env_server.storage import InMemoryMeasurementSink

PRINT_OUTPUT_MODES = ("rich", "plain", "json")


def port_argument(value: str) -> int:
    """Parses and validates a command-line port value."""
    try:
        port = int(value, 10)
    except ValueError as exc:
        raise argparse.ArgumentTypeError("must be an integer") from exc
    if not 1 <= port <= 65535:
        raise argparse.ArgumentTypeError("must be in 1..65535")
    return port


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
    sink = InMemoryMeasurementSink()
    app = create_app(config, sink=sink, output=output)
    discovery = UdpDiscoveryResponder(config, output)

    output.startup(config, config.log_level)
    discovery.start()
    try:
        uvicorn.run(app, host=config.host, port=config.port, log_level=config.log_level)
    except KeyboardInterrupt:
        output.shutdown_requested()
    finally:
        discovery.stop()
        discovery.join(timeout=1.0)
        output.stopped()
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
