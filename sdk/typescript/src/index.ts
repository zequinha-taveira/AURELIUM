// ============================================================================
// AURELIUM SDK — TypeScript
// Unified client for connecting to all AURELIUM infrastructure services
// ============================================================================

import { connect, NatsConnection, Subscription, StringCodec } from 'nats';
import { Pool, PoolClient, QueryResult } from 'pg';

// ============================================================================
// Error Types
// ============================================================================

export class AureliumError extends Error {
  constructor(
    message: string,
    public readonly code: string,
    public readonly cause?: Error
  ) {
    super(message);
    this.name = 'AureliumError';
  }
}

// ============================================================================
// Configuration
// ============================================================================

export interface AureliumConfig {
  natsUrl: string;
  databaseUrl: string;
  qdrantUrl?: string;
  neo4jUrl?: string;
  neo4jUser?: string;
  neo4jPassword?: string;
  /** Maximum number of connection retry attempts */
  maxRetries?: number;
  /** Base delay between retries in ms (doubles each attempt) */
  retryBaseDelayMs?: number;
}

// ============================================================================
// Client
// ============================================================================

export class AureliumClient {
  private natsConnection?: NatsConnection;
  private dbPool: Pool;
  private config: Required<Pick<AureliumConfig, 'maxRetries' | 'retryBaseDelayMs'>> & AureliumConfig;
  private sc = StringCodec();

  constructor(config: AureliumConfig) {
    this.config = {
      maxRetries: 3,
      retryBaseDelayMs: 500,
      ...config,
    };
    this.dbPool = new Pool({ connectionString: config.databaseUrl });
  }

  /**
   * Connect to all configured infrastructure services with retry logic
   */
  async connect(): Promise<void> {
    // Connect to NATS with retry
    let delay = this.config.retryBaseDelayMs;
    for (let attempt = 1; attempt <= this.config.maxRetries; attempt++) {
      try {
        this.natsConnection = await connect({ servers: this.config.natsUrl });
        break;
      } catch (err) {
        if (attempt === this.config.maxRetries) {
          throw new AureliumError(
            `Failed to connect to NATS after ${this.config.maxRetries} attempts`,
            'NATS_CONNECTION_FAILED',
            err as Error
          );
        }
        console.warn(`NATS connection attempt ${attempt}/${this.config.maxRetries} failed. Retrying in ${delay}ms...`);
        await this.sleep(delay);
        delay *= 2;
      }
    }

    // Test DB connection
    try {
      const client = await this.dbPool.connect();
      client.release();
    } catch (err) {
      throw new AureliumError(
        'Failed to connect to PostgreSQL',
        'DB_CONNECTION_FAILED',
        err as Error
      );
    }
  }

  // -----------------------------------------------------------------------
  // Accessors
  // -----------------------------------------------------------------------

  get nats(): NatsConnection {
    if (!this.natsConnection) {
      throw new AureliumError('NATS not connected. Call connect() first.', 'NOT_CONNECTED');
    }
    return this.natsConnection;
  }

  get db(): Pool {
    return this.dbPool;
  }

  get qdrantUrl(): string | undefined {
    return this.config.qdrantUrl;
  }

  get neo4jUrl(): string | undefined {
    return this.config.neo4jUrl;
  }

  // -----------------------------------------------------------------------
  // Event Publishing Helpers
  // -----------------------------------------------------------------------

  /**
   * Publish a typed event to NATS
   */
  async publish<T>(subject: string, payload: T): Promise<void> {
    const data = this.sc.encode(JSON.stringify(payload));
    this.nats.publish(subject, data);
  }

  /**
   * Make a NATS request-reply call with timeout
   */
  async request<T, R>(subject: string, payload: T, timeoutMs: number = 30000): Promise<R> {
    const data = this.sc.encode(JSON.stringify(payload));
    const reply = await this.nats.request(subject, data, { timeout: timeoutMs });
    const decoded = this.sc.decode(reply.data);
    return JSON.parse(decoded) as R;
  }

  /**
   * Subscribe to a NATS subject with a typed handler
   */
  async subscribe<T>(
    subject: string,
    handler: (payload: T, reply?: string) => Promise<void>
  ): Promise<Subscription> {
    const sub = this.nats.subscribe(subject);
    (async () => {
      for await (const msg of sub) {
        try {
          const decoded = this.sc.decode(msg.data);
          const payload = JSON.parse(decoded) as T;
          await handler(payload, msg.reply);
        } catch (err) {
          console.error(`Error processing message on ${subject}:`, err);
        }
      }
    })();
    return sub;
  }

  /**
   * Reply to a NATS request
   */
  async reply<T>(replySubject: string, payload: T): Promise<void> {
    const data = this.sc.encode(JSON.stringify(payload));
    this.nats.publish(replySubject, data);
  }

  // -----------------------------------------------------------------------
  // Database Helpers
  // -----------------------------------------------------------------------

  /**
   * Execute a parameterized query
   */
  async query<T = any>(sql: string, params?: any[]): Promise<QueryResult<T>> {
    return this.dbPool.query<T>(sql, params);
  }

  /**
   * Execute a query and return a single row or null
   */
  async queryOne<T = any>(sql: string, params?: any[]): Promise<T | null> {
    const result = await this.dbPool.query<T>(sql, params);
    return result.rows[0] || null;
  }

  /**
   * Get a dedicated client from the pool (for transactions)
   */
  async getClient(): Promise<PoolClient> {
    return this.dbPool.connect();
  }

  // -----------------------------------------------------------------------
  // Lifecycle
  // -----------------------------------------------------------------------

  /**
   * Gracefully close all connections
   */
  async close(): Promise<void> {
    if (this.natsConnection) {
      await this.natsConnection.drain();
      await this.natsConnection.close();
    }
    await this.dbPool.end();
  }

  // -----------------------------------------------------------------------
  // Private helpers
  // -----------------------------------------------------------------------

  private sleep(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }
}

// Re-export for convenience
export { NatsConnection, Subscription } from 'nats';
export { Pool, PoolClient, QueryResult } from 'pg';
