# ============================================================================
# AURELIUM SDK — Python
# Unified client for connecting to all AURELIUM infrastructure services
# ============================================================================

import asyncio
import json
import logging
from typing import Any, Callable, Optional, TypeVar

import asyncpg
from nats.aio.client import Client as NATSClient

logger = logging.getLogger("aurelium.sdk")

T = TypeVar("T")


# ============================================================================
# Error Types
# ============================================================================


class AureliumError(Exception):
    """Base error for all AURELIUM SDK operations."""

    def __init__(self, message: str, code: str, cause: Optional[Exception] = None):
        super().__init__(message)
        self.code = code
        self.cause = cause


class NatsConnectionError(AureliumError):
    def __init__(self, message: str, cause: Optional[Exception] = None):
        super().__init__(message, "NATS_CONNECTION_FAILED", cause)


class DatabaseConnectionError(AureliumError):
    def __init__(self, message: str, cause: Optional[Exception] = None):
        super().__init__(message, "DB_CONNECTION_FAILED", cause)


# ============================================================================
# Configuration
# ============================================================================


class AureliumConfig:
    """Configuration for building an AureliumClient."""

    def __init__(
        self,
        nats_url: str = "nats://localhost:4222",
        database_url: str = "postgresql://aurelium:aurelium@localhost:5432/aurelium",
        qdrant_url: Optional[str] = None,
        neo4j_url: Optional[str] = None,
        neo4j_user: Optional[str] = None,
        neo4j_password: Optional[str] = None,
        max_retries: int = 3,
        retry_base_delay: float = 0.5,
    ):
        self.nats_url = nats_url
        self.database_url = database_url
        self.qdrant_url = qdrant_url
        self.neo4j_url = neo4j_url
        self.neo4j_user = neo4j_user
        self.neo4j_password = neo4j_password
        self.max_retries = max_retries
        self.retry_base_delay = retry_base_delay


# ============================================================================
# Client
# ============================================================================


class AureliumClient:
    """Unified client for all AURELIUM infrastructure services."""

    def __init__(self, config: Optional[AureliumConfig] = None, **kwargs):
        """
        Initialize with either an AureliumConfig or keyword arguments.

        Backward-compatible: AureliumClient(nats_url="...", db_url="...")
        """
        if config:
            self._config = config
        else:
            self._config = AureliumConfig(
                nats_url=kwargs.get("nats_url", "nats://localhost:4222"),
                database_url=kwargs.get(
                    "db_url",
                    "postgresql://aurelium:aurelium@localhost:5432/aurelium",
                ),
                qdrant_url=kwargs.get("qdrant_url"),
                neo4j_url=kwargs.get("neo4j_url"),
                neo4j_user=kwargs.get("neo4j_user"),
                neo4j_password=kwargs.get("neo4j_password"),
            )

        self._nats = NATSClient()
        self._db_pool: Optional[asyncpg.Pool] = None

    # -----------------------------------------------------------------------
    # Connection
    # -----------------------------------------------------------------------

    async def connect(self) -> None:
        """Connect to all configured infrastructure services with retry."""
        await self._connect_nats()
        await self._connect_db()
        logger.info("AURELIUM SDK connected to all services.")

    async def _connect_nats(self) -> None:
        delay = self._config.retry_base_delay
        for attempt in range(1, self._config.max_retries + 1):
            try:
                await self._nats.connect(self._config.nats_url)
                logger.info(f"NATS connected at {self._config.nats_url}")
                return
            except Exception as e:
                if attempt == self._config.max_retries:
                    raise NatsConnectionError(
                        f"Failed to connect to NATS after {self._config.max_retries} attempts",
                        cause=e,
                    )
                logger.warning(
                    f"NATS connection attempt {attempt}/{self._config.max_retries} failed. "
                    f"Retrying in {delay}s..."
                )
                await asyncio.sleep(delay)
                delay *= 2

    async def _connect_db(self) -> None:
        delay = self._config.retry_base_delay
        for attempt in range(1, self._config.max_retries + 1):
            try:
                self._db_pool = await asyncpg.create_pool(self._config.database_url)
                logger.info("PostgreSQL connected.")
                return
            except Exception as e:
                if attempt == self._config.max_retries:
                    raise DatabaseConnectionError(
                        f"Failed to connect to PostgreSQL after {self._config.max_retries} attempts",
                        cause=e,
                    )
                logger.warning(
                    f"PostgreSQL connection attempt {attempt}/{self._config.max_retries} failed. "
                    f"Retrying in {delay}s..."
                )
                await asyncio.sleep(delay)
                delay *= 2

    # -----------------------------------------------------------------------
    # Accessors
    # -----------------------------------------------------------------------

    @property
    def nats(self) -> NATSClient:
        return self._nats

    @property
    def db(self) -> asyncpg.Pool:
        if self._db_pool is None:
            raise RuntimeError("Database pool not initialized. Call connect() first.")
        return self._db_pool

    @property
    def qdrant_url(self) -> Optional[str]:
        return self._config.qdrant_url

    @property
    def neo4j_url(self) -> Optional[str]:
        return self._config.neo4j_url

    # -----------------------------------------------------------------------
    # Event Publishing Helpers
    # -----------------------------------------------------------------------

    async def publish(self, subject: str, payload: Any) -> None:
        """Publish a typed event to NATS."""
        data = json.dumps(payload).encode()
        await self._nats.publish(subject, data)

    async def request(
        self, subject: str, payload: Any, timeout: float = 30.0
    ) -> dict:
        """Make a NATS request-reply call with timeout."""
        data = json.dumps(payload).encode()
        reply = await self._nats.request(subject, data, timeout=timeout)
        return json.loads(reply.data.decode())

    async def subscribe(
        self, subject: str, handler: Callable
    ):
        """Subscribe to a NATS subject with a handler callback."""

        async def _wrapper(msg):
            try:
                payload = json.loads(msg.data.decode())
                await handler(payload, msg)
            except Exception as e:
                logger.error(f"Error processing message on {subject}: {e}")

        return await self._nats.subscribe(subject, cb=_wrapper)

    # -----------------------------------------------------------------------
    # Database Helpers
    # -----------------------------------------------------------------------

    async def query(self, sql: str, *args) -> list:
        """Execute a query and return all rows."""
        return await self.db.fetch(sql, *args)

    async def query_one(self, sql: str, *args) -> Optional[asyncpg.Record]:
        """Execute a query and return a single row or None."""
        return await self.db.fetchrow(sql, *args)

    async def execute(self, sql: str, *args) -> str:
        """Execute a query that doesn't return rows (INSERT, UPDATE, DELETE)."""
        return await self.db.execute(sql, *args)

    # -----------------------------------------------------------------------
    # Lifecycle
    # -----------------------------------------------------------------------

    async def close(self) -> None:
        """Gracefully close all connections."""
        await self._nats.close()
        if self._db_pool:
            await self._db_pool.close()
        logger.info("AURELIUM SDK disconnected.")

    async def __aenter__(self):
        await self.connect()
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb):
        await self.close()
