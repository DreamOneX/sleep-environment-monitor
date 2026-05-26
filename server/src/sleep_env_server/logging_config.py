"""Logging configuration for service and TUI runtime entry points."""

from __future__ import annotations

import json
import logging
import sys
from typing import TextIO

from rich.console import Console
from rich.logging import RichHandler

from sleep_env_server.output import OutputMode


class JsonLogHandler(logging.Handler):
    """Writes standard logging records as compact JSONL events."""

    def __init__(self, stream: TextIO) -> None:
        """Initializes the handler."""
        super().__init__()
        self.stream = stream

    def emit(self, record: logging.LogRecord) -> None:
        """Writes one log record."""
        payload = {
            "event": "log",
            "level": record.levelname.lower(),
            "logger": record.name,
            "message": record.getMessage(),
        }
        self.stream.write(json.dumps(payload, separators=(",", ":"), sort_keys=True))
        self.stream.write("\n")
        self.stream.flush()


def configure_service_logging(
    mode: OutputMode,
    *,
    stream: TextIO | None = None,
    log_level: str = "info",
    force_terminal: bool | None = None,
) -> logging.Handler:
    """Configures Uvicorn and package logging for the service process."""
    target_stream = stream if stream is not None else sys.stdout
    handler = _build_handler(mode, target_stream, force_terminal=force_terminal)
    level = getattr(logging, log_level.upper(), logging.INFO)
    handler.setLevel(level)

    for name in ("uvicorn", "sleep_env_server"):
        logger = logging.getLogger(name)
        logger.handlers = [handler]
        logger.propagate = False
        logger.setLevel(level)

    for name in ("uvicorn.error", "uvicorn.access"):
        logger = logging.getLogger(name)
        logger.handlers = []
        logger.propagate = True
        logger.setLevel(level)

    return handler


def _build_handler(
    mode: OutputMode,
    stream: TextIO,
    *,
    force_terminal: bool | None,
) -> logging.Handler:
    """Builds the output-mode-specific logging handler."""
    if mode == "json":
        return JsonLogHandler(stream)
    if mode == "rich":
        return RichHandler(
            console=Console(
                file=stream,
                force_terminal=force_terminal,
                highlight=False,
                markup=False,
            ),
            markup=False,
            rich_tracebacks=False,
            show_path=False,
        )

    handler = logging.StreamHandler(stream)
    handler.setFormatter(logging.Formatter("%(levelname)s %(name)s %(message)s"))
    return handler
