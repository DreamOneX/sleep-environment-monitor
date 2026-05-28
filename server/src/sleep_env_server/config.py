"""Server configuration, TOML loading, and derived API paths."""

from __future__ import annotations

import os
import shutil
import tomllib
from collections.abc import Mapping
from dataclasses import dataclass, field, replace
from pathlib import Path
from typing import Any, Literal

from sleep_env_server.models import DiscoveryDocument

DEFAULT_HOST = "0.0.0.0"
DEFAULT_PORT = 8080
DEFAULT_UDP_DISCOVERY_PORT = 39022
DEFAULT_API_BASE = "/api/v1"
DEFAULT_LOG_LEVEL = "info"
DISCOVERY_DOCUMENT_PATH = "/.well-known/sleep-environment-monitor"
APP_DIR_NAME = "sleep-env-server"
CONFIG_FILE_NAME = "config.toml"
LOG_LEVELS = ("debug", "info", "warning", "error")
OUTPUT_MODES = ("auto", "rich", "plain", "json")
DEFAULT_TUI_THEME = "catppuccin-mocha"
DEFAULT_TUI_MEASUREMENTS_LIMIT = 200
TUI_THEMES = (DEFAULT_TUI_THEME, "graphite")
READ_SOURCES = ("merge", "sqlite", "jsonl")
DEDUP_STRATEGIES = ("keep_first", "keep_last", "overwrite", "reject")
CONFLICT_STRATEGIES = ("overwrite", "keep", "earliest", "latest", "error")
TIME_SOURCES = ("device_reported", "server_received")
DEFAULT_HISTORY_METRICS = ("temperature_c", "humidity_percent", "lux", "mic_db_rel")


def validate_port(value: int, name: str) -> int:
    """Validates a TCP or UDP port.

    Args:
        value: Candidate port value.
        name: Human-readable field name for error messages.

    Returns:
        The validated port.

    Raises:
        ValueError: If the port is outside the valid user-visible range.
    """
    if not 1 <= value <= 65535:
        raise ValueError(f"{name} must be in 1..65535")
    return value


def validate_choice(value: str, choices: tuple[str, ...], name: str) -> str:
    """Validates a string enum value."""
    if value not in choices:
        joined = ", ".join(choices)
        raise ValueError(f"{name} must be one of: {joined}")
    return value


def package_root() -> Path:
    """Returns the server package root directory."""
    return Path(__file__).resolve().parents[2]


def example_config_path() -> Path:
    """Returns the tracked example configuration path."""
    return package_root() / "config.example.toml"


def default_config_path(
    *,
    environ: Mapping[str, str] | None = None,
    home: Path | None = None,
) -> Path:
    """Returns the XDG default configuration path.

    Args:
        environ: Environment mapping. Defaults to ``os.environ``.
        home: Home directory override for tests.

    Returns:
        Default TOML configuration path.
    """
    env = os.environ if environ is None else environ
    if xdg_config_home := env.get("XDG_CONFIG_HOME"):
        base = Path(xdg_config_home)
    else:
        base = (Path.home() if home is None else home) / ".config"
    return base / APP_DIR_NAME / CONFIG_FILE_NAME


def ensure_default_config(path: Path, *, template_path: Path | None = None) -> bool:
    """Creates the default TOML configuration when missing.

    Args:
        path: Destination configuration path.
        template_path: Optional template path. Defaults to the tracked example.

    Returns:
        ``True`` if a new file was created, otherwise ``False``.
    """
    if path.exists():
        return False
    source = example_config_path() if template_path is None else template_path
    path.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, path)
    return True


def resolve_config_path(
    explicit_path: str | None = None,
    *,
    environ: Mapping[str, str] | None = None,
    home: Path | None = None,
    generate_default: bool = True,
) -> tuple[Path, bool]:
    """Resolves a config path and optionally creates the XDG default file.

    Args:
        explicit_path: User-supplied ``--config`` path.
        environ: Environment mapping for XDG resolution.
        home: Home directory override for tests.
        generate_default: Whether the default config may be created.

    Returns:
        A tuple of resolved path and whether it was generated.

    Raises:
        ValueError: If an explicit path does not exist.
    """
    if explicit_path:
        path = Path(explicit_path)
        if not path.exists():
            raise ValueError(f"config file does not exist: {path}")
        return path, False

    path = default_config_path(environ=environ, home=home)
    generated = ensure_default_config(path) if generate_default else False
    return path, generated


def load_toml_file(path: Path) -> dict[str, Any]:
    """Loads a TOML file into a dictionary."""
    with path.open("rb") as stream:
        data = tomllib.load(stream)
    if not isinstance(data, dict):
        raise ValueError("config root must be a TOML table")
    return data


@dataclass(frozen=True)
class ServerConfig:
    """Runtime configuration for HTTP and UDP discovery."""

    host: str = DEFAULT_HOST
    port: int = DEFAULT_PORT
    udp_discovery_port: int = DEFAULT_UDP_DISCOVERY_PORT
    api_base: str = DEFAULT_API_BASE
    log_level: str = DEFAULT_LOG_LEVEL

    def __post_init__(self) -> None:
        """Validates configuration values after dataclass initialization."""
        if not self.host:
            raise ValueError("host must not be empty")
        validate_port(self.port, "port")
        validate_port(self.udp_discovery_port, "udp_discovery_port")
        if not self.api_base.startswith("/"):
            raise ValueError("api_base must start with '/'")
        if self.api_base != "/" and self.api_base.endswith("/"):
            raise ValueError("api_base must not end with '/'")
        validate_choice(self.log_level, LOG_LEVELS, "log_level")

    @property
    def measurement_upload_path(self) -> str:
        """Returns the versioned measurement upload path."""
        return f"{self.api_base}/measurements"

    @property
    def time_path(self) -> str:
        """Returns the server time endpoint path."""
        return f"{self.api_base}/time"

    @property
    def history_measurements_path(self) -> str:
        """Returns the history measurements path."""
        return f"{self.api_base}/history/measurements"

    @property
    def history_summary_path(self) -> str:
        """Returns the history summary path."""
        return f"{self.api_base}/history/summary"

    @property
    def discovery_document_path(self) -> str:
        """Returns the well-known discovery document path."""
        return DISCOVERY_DOCUMENT_PATH

    def discovery_document(self) -> DiscoveryDocument:
        """Builds the HTTP discovery document for this configuration.

        Returns:
            Discovery metadata served from the well-known endpoint.
        """
        return DiscoveryDocument(
            api_base=self.api_base,
            measurement_upload=self.measurement_upload_path,
            time=self.time_path,
            udp_discovery_port=self.udp_discovery_port,
        )


@dataclass(frozen=True)
class OutputConfig:
    """Runtime output configuration."""

    mode: Literal["auto", "rich", "plain", "json"] = "auto"
    dashboard: bool = True

    def __post_init__(self) -> None:
        """Validates output configuration."""
        validate_choice(self.mode, OUTPUT_MODES, "output.mode")


@dataclass(frozen=True)
class TuiConfig:
    """Textual TUI visual configuration."""

    theme: Literal["catppuccin-mocha", "graphite"] = DEFAULT_TUI_THEME
    transparent: bool = False
    autostart: bool = True
    measurements_limit: int = DEFAULT_TUI_MEASUREMENTS_LIMIT

    def __post_init__(self) -> None:
        """Validates TUI configuration."""
        validate_choice(self.theme, TUI_THEMES, "tui.theme")
        if self.measurements_limit < 1:
            raise ValueError("tui.measurements_limit must be positive")


@dataclass(frozen=True)
class AckPolicyConfig:
    """Per-policy or per-target ACK behavior."""

    required_for_ack: bool = False
    sufficient_for_ack: bool = False


@dataclass(frozen=True)
class LimitConfig:
    """Retention limits for a policy profile."""

    time_limit: str | int = "10d"
    size_limit: str | int = "100MB"


@dataclass(frozen=True)
class DeduplicationConfig:
    """Duplicate handling strategy."""

    strategy: Literal["keep_first", "keep_last", "overwrite", "reject"] = "keep_first"

    def __post_init__(self) -> None:
        """Validates deduplication strategy."""
        validate_choice(self.strategy, DEDUP_STRATEGIES, "deduplication.strategy")


@dataclass(frozen=True)
class BackfillConfig:
    """Backfill source and conflict behavior."""

    source: str = "all"
    exclude: tuple[str, ...] = ()
    conflict: Literal["overwrite", "keep", "earliest", "latest", "error"] = "error"

    def __post_init__(self) -> None:
        """Validates backfill configuration."""
        validate_choice(self.conflict, CONFLICT_STRATEGIES, "backfill.conflict")


@dataclass(frozen=True)
class PolicyProfileConfig:
    """Storage policy profile."""

    parent: str | None = None
    limit: LimitConfig = field(default_factory=LimitConfig)
    deduplication: DeduplicationConfig = field(default_factory=DeduplicationConfig)
    ack: AckPolicyConfig = field(default_factory=AckPolicyConfig)
    backfill: BackfillConfig = field(default_factory=BackfillConfig)


@dataclass(frozen=True)
class StoragePolicyConfig:
    """Storage policy collection."""

    default_profile: str = "default"
    default_parent: str | None = None
    profiles: dict[str, PolicyProfileConfig] = field(
        default_factory=lambda: {
            "default": PolicyProfileConfig(),
            "no_limit": PolicyProfileConfig(
                parent="default",
                limit=LimitConfig(time_limit=-1, size_limit=-1),
            ),
        }
    )

    def __post_init__(self) -> None:
        """Validates profile references."""
        if self.default_profile not in self.profiles:
            raise ValueError("storage.policy.default_profile references an unknown profile")
        for name, profile in self.profiles.items():
            if profile.parent and profile.parent not in self.profiles:
                raise ValueError(f"storage policy profile {name} has unknown parent")
            self._check_profile_loop(name)

    def _check_profile_loop(self, name: str) -> None:
        """Rejects parent cycles for one profile."""
        seen: set[str] = set()
        current: str | None = name
        while current is not None:
            if current in seen:
                raise ValueError(f"storage policy profile {name} has an inheritance loop")
            seen.add(current)
            current = self.profiles[current].parent


@dataclass(frozen=True)
class StorageTargetConfig:
    """Configuration for one persistent storage target."""

    enabled: bool = False
    path: str = ""
    policy: str = "default"
    ack: AckPolicyConfig = field(default_factory=AckPolicyConfig)


@dataclass(frozen=True)
class StorageConfig:
    """Composite storage configuration."""

    enabled: bool = True
    required_for_ack: bool = True
    reconcile_on_start: bool = True
    reconcile_interval_seconds: int = 300
    time_source: Literal["device_reported", "server_received"] = "device_reported"
    time_fallback: Literal["server_received"] = "server_received"
    policy: StoragePolicyConfig = field(default_factory=StoragePolicyConfig)
    sqlite: StorageTargetConfig = field(
        default_factory=lambda: StorageTargetConfig(
            enabled=True,
            path="./sleep-environment.db",
            policy="no_limit",
            ack=AckPolicyConfig(required_for_ack=True, sufficient_for_ack=True),
        )
    )
    jsonl: StorageTargetConfig = field(
        default_factory=lambda: StorageTargetConfig(
            enabled=False,
            path="./sleep-environment.jsonl",
            policy="default",
        )
    )

    def __post_init__(self) -> None:
        """Validates storage configuration."""
        if self.reconcile_interval_seconds < 0:
            raise ValueError("storage.reconcile_interval_seconds must be non-negative")
        validate_choice(self.time_source, TIME_SOURCES, "storage.time_source")
        if self.time_fallback != "server_received":
            raise ValueError("storage.time_fallback must be server_received")
        for target_name, target in (("sqlite", self.sqlite), ("jsonl", self.jsonl)):
            if target.policy not in self.policy.profiles:
                raise ValueError(f"storage.{target_name}.policy references an unknown profile")
            if target.enabled and not target.path:
                raise ValueError(f"storage.{target_name}.path must not be empty when enabled")


@dataclass(frozen=True)
class HistoryApiConfig:
    """History read API configuration."""

    enabled: bool = False
    bearer_token: str = ""
    read_source: Literal["merge", "sqlite", "jsonl"] = "merge"
    merge_sources: tuple[str, ...] = ("sqlite", "jsonl")
    merge_conflict: Literal["overwrite", "keep", "earliest", "latest", "error"] = "error"

    def __post_init__(self) -> None:
        """Validates history API configuration."""
        validate_choice(self.read_source, READ_SOURCES, "history_api.read_source")
        validate_choice(self.merge_conflict, CONFLICT_STRATEGIES, "history_api.merge.conflict")
        for source in self.merge_sources:
            validate_choice(source, ("sqlite", "jsonl"), "history_api.merge.sources")
        if self.enabled and not self.bearer_token:
            raise ValueError("history_api.bearer_token must be set when history API is enabled")


@dataclass(frozen=True)
class HistoryCliConfig:
    """Local history CLI configuration."""

    read_source: Literal["merge", "sqlite", "jsonl"] = "merge"
    tail_count: int = 20
    metrics: tuple[str, ...] = DEFAULT_HISTORY_METRICS

    def __post_init__(self) -> None:
        """Validates history CLI configuration."""
        validate_choice(self.read_source, READ_SOURCES, "history_cli.read_source")
        if self.tail_count < 0:
            raise ValueError("history_cli.tail_count must be non-negative")
        if not self.metrics:
            raise ValueError("history_cli.metrics must not be empty")


@dataclass(frozen=True)
class AppConfig:
    """Full server application configuration."""

    server: ServerConfig = field(default_factory=ServerConfig)
    output: OutputConfig = field(default_factory=OutputConfig)
    tui: TuiConfig = field(default_factory=TuiConfig)
    storage: StorageConfig = field(default_factory=StorageConfig)
    history_api: HistoryApiConfig = field(default_factory=HistoryApiConfig)
    history_cli: HistoryCliConfig = field(default_factory=HistoryCliConfig)
    config_path: Path | None = None
    generated_config: bool = False


def app_config_from_mapping(data: Mapping[str, Any]) -> AppConfig:
    """Builds an application config from a loaded TOML mapping."""
    return AppConfig(
        server=_parse_server(_table(data, "server")),
        output=_parse_output(_table(data, "output")),
        tui=_parse_tui(_table(data, "tui")),
        storage=_parse_storage(_table(data, "storage")),
        history_api=_parse_history_api(_table(data, "history_api")),
        history_cli=_parse_history_cli(_table(data, "history_cli")),
    )


def load_app_config(
    explicit_path: str | None = None,
    *,
    environ: Mapping[str, str] | None = None,
    home: Path | None = None,
    generate_default: bool = True,
) -> AppConfig:
    """Loads application config from TOML or generated defaults."""
    path, generated = resolve_config_path(
        explicit_path,
        environ=environ,
        home=home,
        generate_default=generate_default,
    )
    config = app_config_from_mapping(load_toml_file(path))
    return replace(config, config_path=path, generated_config=generated)


def apply_cli_overrides(config: AppConfig, args: Any) -> AppConfig:
    """Applies command-line overrides to an app config."""
    server = config.server
    output = config.output
    tui = config.tui

    for attr in ("host", "port", "udp_discovery_port", "log_level"):
        value = getattr(args, attr, None)
        if value is not None:
            server = replace(server, **{attr: value})

    if getattr(args, "json_log", False):
        output = replace(output, mode="json")
    elif getattr(args, "rich_log", False):
        output = replace(output, mode="rich")
    elif getattr(args, "no_rich", False):
        output = replace(output, mode="plain")

    if getattr(args, "transparent", False):
        tui = replace(tui, transparent=True)
    if getattr(args, "no_autostart", False):
        tui = replace(tui, autostart=False)

    return replace(config, server=server, output=output, tui=tui)


def _parse_server(data: Mapping[str, Any]) -> ServerConfig:
    """Parses the ``server`` table."""
    return ServerConfig(
        host=_str(data, "host", DEFAULT_HOST),
        port=_int(data, "port", DEFAULT_PORT),
        udp_discovery_port=_int(data, "udp_discovery_port", DEFAULT_UDP_DISCOVERY_PORT),
        api_base=_str(data, "api_base", DEFAULT_API_BASE),
        log_level=_str(data, "log_level", DEFAULT_LOG_LEVEL),
    )


def _parse_output(data: Mapping[str, Any]) -> OutputConfig:
    """Parses the ``output`` table."""
    return OutputConfig(
        mode=_str(data, "mode", "auto"),  # type: ignore[arg-type]
        dashboard=_bool(data, "dashboard", True),
    )


def _parse_tui(data: Mapping[str, Any]) -> TuiConfig:
    """Parses the ``tui`` table."""
    return TuiConfig(
        theme=_str(data, "theme", DEFAULT_TUI_THEME),  # type: ignore[arg-type]
        transparent=_bool(data, "transparent", False),
        autostart=_bool(data, "autostart", True),
        measurements_limit=_int(data, "measurements_limit", DEFAULT_TUI_MEASUREMENTS_LIMIT),
    )


def _parse_storage(data: Mapping[str, Any]) -> StorageConfig:
    """Parses the ``storage`` table."""
    policy = _parse_storage_policy(_table(data, "policy"))
    return StorageConfig(
        enabled=_bool(data, "enabled", True),
        required_for_ack=_bool(data, "required_for_ack", True),
        reconcile_on_start=_bool(data, "reconcile_on_start", True),
        reconcile_interval_seconds=_int(data, "reconcile_interval_seconds", 300),
        time_source=_str(data, "time_source", "device_reported"),  # type: ignore[arg-type]
        time_fallback=_str(data, "time_fallback", "server_received"),  # type: ignore[arg-type]
        policy=policy,
        sqlite=_parse_storage_target(
            _table(data, "sqlite"),
            default=StorageConfig().sqlite,
        ),
        jsonl=_parse_storage_target(
            _table(data, "jsonl"),
            default=StorageConfig().jsonl,
        ),
    )


def _parse_storage_policy(data: Mapping[str, Any]) -> StoragePolicyConfig:
    """Parses the storage policy table."""
    profiles_data = _nested_table(data, "profile")
    profiles: dict[str, PolicyProfileConfig] = {}
    if profiles_data:
        for name, profile_data in profiles_data.items():
            if not isinstance(profile_data, Mapping):
                raise ValueError(f"storage.policy.profile.{name} must be a table")
            profiles[name] = _parse_policy_profile(profile_data)
    else:
        profiles = StoragePolicyConfig().profiles

    default_parent = _optional_str(data, "default_parent")
    return StoragePolicyConfig(
        default_profile=_str(data, "default_profile", "default"),
        default_parent=default_parent if default_parent else None,
        profiles=profiles,
    )


def _parse_policy_profile(data: Mapping[str, Any]) -> PolicyProfileConfig:
    """Parses one storage policy profile."""
    parent = _optional_str(data, "parent")
    return PolicyProfileConfig(
        parent=parent if parent else None,
        limit=_parse_limit(_table(data, "limit")),
        deduplication=_parse_dedup(_table(data, "deduplication")),
        ack=_parse_ack(_table(data, "ack")),
        backfill=_parse_backfill(_table(data, "backfill")),
    )


def _parse_limit(data: Mapping[str, Any]) -> LimitConfig:
    """Parses profile limit settings."""
    return LimitConfig(
        time_limit=_str_or_int(data, "time_limit", "10d"),
        size_limit=_str_or_int(data, "size_limit", "100MB"),
    )


def _parse_dedup(data: Mapping[str, Any]) -> DeduplicationConfig:
    """Parses deduplication settings."""
    return DeduplicationConfig(strategy=_str(data, "strategy", "keep_first"))  # type: ignore[arg-type]


def _parse_ack(data: Mapping[str, Any]) -> AckPolicyConfig:
    """Parses ACK settings."""
    return AckPolicyConfig(
        required_for_ack=_bool(data, "required_for_ack", False),
        sufficient_for_ack=_bool(data, "sufficient_for_ack", False),
    )


def _parse_backfill(data: Mapping[str, Any]) -> BackfillConfig:
    """Parses backfill settings."""
    return BackfillConfig(
        source=_str(data, "source", "all"),
        exclude=_str_tuple(data, "exclude", ()),
        conflict=_str(data, "conflict", "error"),  # type: ignore[arg-type]
    )


def _parse_storage_target(
    data: Mapping[str, Any],
    *,
    default: StorageTargetConfig,
) -> StorageTargetConfig:
    """Parses one storage target table."""
    return StorageTargetConfig(
        enabled=_bool(data, "enabled", default.enabled),
        path=_str(data, "path", default.path),
        policy=_str(data, "policy", default.policy),
        ack=_parse_ack(_table(data, "ack")) if "ack" in data else default.ack,
    )


def _parse_history_api(data: Mapping[str, Any]) -> HistoryApiConfig:
    """Parses history API settings."""
    merge = _table(data, "merge")
    return HistoryApiConfig(
        enabled=_bool(data, "enabled", False),
        bearer_token=_str(data, "bearer_token", ""),
        read_source=_str(data, "read_source", "merge"),  # type: ignore[arg-type]
        merge_sources=_str_tuple(merge, "sources", ("sqlite", "jsonl")),
        merge_conflict=_str(merge, "conflict", "error"),  # type: ignore[arg-type]
    )


def _parse_history_cli(data: Mapping[str, Any]) -> HistoryCliConfig:
    """Parses local history CLI settings."""
    return HistoryCliConfig(
        read_source=_str(data, "read_source", "merge"),  # type: ignore[arg-type]
        tail_count=_int(data, "tail_count", 20),
        metrics=_str_tuple(data, "metrics", DEFAULT_HISTORY_METRICS),
    )


def _table(data: Mapping[str, Any], key: str) -> Mapping[str, Any]:
    """Returns a nested table or an empty mapping."""
    value = data.get(key, {})
    if not isinstance(value, Mapping):
        raise ValueError(f"{key} must be a table")
    return value


def _nested_table(data: Mapping[str, Any], key: str) -> Mapping[str, Any]:
    """Returns a required nested table type or an empty mapping."""
    return _table(data, key)


def _str(data: Mapping[str, Any], key: str, default: str) -> str:
    """Reads a string setting."""
    value = data.get(key, default)
    if not isinstance(value, str):
        raise ValueError(f"{key} must be a string")
    return value


def _optional_str(data: Mapping[str, Any], key: str) -> str | None:
    """Reads an optional string setting."""
    if key not in data:
        return None
    return _str(data, key, "")


def _str_or_int(data: Mapping[str, Any], key: str, default: str | int) -> str | int:
    """Reads a string or integer setting."""
    value = data.get(key, default)
    if not isinstance(value, str | int):
        raise ValueError(f"{key} must be a string or integer")
    return value


def _int(data: Mapping[str, Any], key: str, default: int) -> int:
    """Reads an integer setting."""
    value = data.get(key, default)
    if not isinstance(value, int):
        raise ValueError(f"{key} must be an integer")
    return value


def _bool(data: Mapping[str, Any], key: str, default: bool) -> bool:
    """Reads a boolean setting."""
    value = data.get(key, default)
    if not isinstance(value, bool):
        raise ValueError(f"{key} must be a boolean")
    return value


def _str_tuple(data: Mapping[str, Any], key: str, default: tuple[str, ...]) -> tuple[str, ...]:
    """Reads a string-list setting as a tuple."""
    value = data.get(key, list(default))
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        raise ValueError(f"{key} must be a list of strings")
    return tuple(value)
