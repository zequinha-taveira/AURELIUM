"""AURELIUM SDK — Python client for the Living Software Fabric."""

from aurelium_sdk.client import (
    AureliumClient,
    AureliumConfig,
    AureliumError,
    NatsConnectionError,
    DatabaseConnectionError,
)

__all__ = [
    "AureliumClient",
    "AureliumConfig",
    "AureliumError",
    "NatsConnectionError",
    "DatabaseConnectionError",
]

__version__ = "0.2.0"
