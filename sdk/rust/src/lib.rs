pub struct AureliumClient {
    nats_client: async_nats::Client,
    db_pool: sqlx::PgPool,
}

impl AureliumClient {
    pub async fn new(nats_url: &str, db_url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let nats_client = async_nats::connect(nats_url).await?;
        let db_pool = sqlx::PgPool::connect(db_url).await?;
        Ok(Self {
            nats_client,
            db_pool,
        })
    }

    pub fn nats(&self) -> &async_nats::Client {
        &self.nats_client
    }

    pub fn db(&self) -> &sqlx::PgPool {
        &self.db_pool
    }
}
