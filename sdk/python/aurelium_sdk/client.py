import asyncio
import asyncpg
from nats.aio.client import Client as NATSClient


class AureliumClient:
    def __init__(self, nats_url: str, db_url: str):
        self.nats_url = nats_url
        self.db_url = db_url
        self.nats_client = NATSClient()
        self.db_pool = None

    async def connect(self):
        await self.nats_client.connect(self.nats_url)
        self.db_pool = await asyncpg.create_pool(self.db_url)

    @property
    def nats(self) -> NATSClient:
        return self.nats_client

    @property
    def db(self) -> asyncpg.Pool:
        if self.db_pool is None:
            raise RuntimeError("Database pool not initialized. Call connect() first.")
        return self.db_pool

    async def close(self):
        await self.nats_client.close()
        if self.db_pool:
            await self.db_pool.close()
