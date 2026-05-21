#!/usr/bin/env python3
"""Compatibility wrapper for the Phase 23 formal server CLI."""

from __future__ import annotations

import os
import shutil
import sys
from pathlib import Path


def main() -> int:
    """Runs the formal server with legacy script defaults."""
    server_dir = Path(__file__).resolve().parent
    server_src = server_dir / "src"
    sys.path.insert(0, str(server_src))

    args = sys.argv[1:]
    if not args or args[0].startswith("-"):
        args = ["serve", *args]

    try:
        from sleep_env_server.cli import main as cli_main
    except ModuleNotFoundError:
        if shutil.which("uv") is None:
            raise
        os.environ.setdefault("UV_CACHE_DIR", str(server_dir / ".cache" / "uv"))
        os.chdir(server_dir)
        os.execvp("uv", ["uv", "run", "sleep-env-server", *args])
        raise

    return cli_main(args)


if __name__ == "__main__":
    raise SystemExit(main())
