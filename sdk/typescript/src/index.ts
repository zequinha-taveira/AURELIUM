import { connect, NatsConnection } from 'nats';
import { Pool } from 'pg';

export class AureliumClient {
  private natsConnection?: NatsConnection;
  private dbPool: Pool;

  constructor(private natsUrl: string, private dbUrl: string) {
    this.dbPool = new Pool({ connectionString: dbUrl });
  }

  async connect(): Promise<void> {
    this.natsConnection = await connect({ servers: this.natsUrl });
  }

  get nats(): NatsConnection {
    if (!this.natsConnection) {
      throw new Error('NATS not connected. Call connect() first.');
    }
    return this.natsConnection;
  }

  get db(): Pool {
    return this.dbPool;
  }

  async close(): Promise<void> {
    if (this.natsConnection) {
      await this.natsConnection.close();
    }
    await this.dbPool.end();
  }
}
