// ============================================================================
// AURELIUM SDK — Rust
// Unified client for connecting to all AURELIUM infrastructure services
// ============================================================================

use std::time::Duration;
use tracing::{error, info};

// ============================================================================
// Error Types
// ============================================================================

/// Unified error type for AURELIUM SDK operations
#[derive(Debug)]
pub enum AureliumError {
    /// NATS connection or messaging error
    Nats(async_nats::Error),
    /// PostgreSQL connection or query error
    Database(sqlx::Error),
    /// Qdrant vector database error
    Qdrant(String),
    /// Neo4j graph database error
    Neo4j(String),
    /// Serialization/Deserialization error
    Serialization(serde_json::Error),
    /// Configuration error
    Config(String),
    /// Timeout error
    Timeout(String),
    /// Generic error
    Other(String),
}

impl std::fmt::Display for AureliumError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AureliumError::Nats(e) => write!(f, "NATS error: {}", e),
            AureliumError::Database(e) => write!(f, "Database error: {}", e),
            AureliumError::Qdrant(e) => write!(f, "Qdrant error: {}", e),
            AureliumError::Neo4j(e) => write!(f, "Neo4j error: {}", e),
            AureliumError::Serialization(e) => write!(f, "Serialization error: {}", e),
            AureliumError::Config(e) => write!(f, "Config error: {}", e),
            AureliumError::Timeout(e) => write!(f, "Timeout error: {}", e),
            AureliumError::Other(e) => write!(f, "Error: {}", e),
        }
    }
}

impl std::error::Error for AureliumError {}

impl From<async_nats::Error> for AureliumError {
    fn from(e: async_nats::Error) -> Self {
        AureliumError::Nats(e)
    }
}

impl From<sqlx::Error> for AureliumError {
    fn from(e: sqlx::Error) -> Self {
        AureliumError::Database(e)
    }
}

impl From<serde_json::Error> for AureliumError {
    fn from(e: serde_json::Error) -> Self {
        AureliumError::Serialization(e)
    }
}

pub type AureliumResult<T> = Result<T, AureliumError>;

// ============================================================================
// Client Configuration
// ============================================================================

/// Configuration for building an AureliumClient
pub struct AureliumConfig {
    pub nats_url: String,
    pub database_url: String,
    pub qdrant_url: Option<String>,
    pub neo4j_url: Option<String>,
    pub neo4j_user: Option<String>,
    pub neo4j_password: Option<String>,
    /// Maximum number of connection retry attempts
    pub max_retries: u32,
    /// Base delay between retries (doubles each attempt)
    pub retry_base_delay: Duration,
}

impl Default for AureliumConfig {
    fn default() -> Self {
        Self {
            nats_url: "nats://localhost:4222".to_string(),
            database_url: "postgres://aurelium:aurelium@localhost:5432/aurelium".to_string(),
            qdrant_url: None,
            neo4j_url: None,
            neo4j_user: None,
            neo4j_password: None,
            max_retries: 3,
            retry_base_delay: Duration::from_millis(500),
        }
    }
}

impl AureliumConfig {
    /// Create a new config with required NATS and DB URLs
    pub fn new(nats_url: &str, database_url: &str) -> Self {
        Self {
            nats_url: nats_url.to_string(),
            database_url: database_url.to_string(),
            ..Default::default()
        }
    }

    /// Set the Qdrant URL for vector operations
    pub fn with_qdrant(mut self, url: &str) -> Self {
        self.qdrant_url = Some(url.to_string());
        self
    }

    /// Set the Neo4j URL and credentials for graph operations
    pub fn with_neo4j(mut self, url: &str, user: &str, password: &str) -> Self {
        self.neo4j_url = Some(url.to_string());
        self.neo4j_user = Some(user.to_string());
        self.neo4j_password = Some(password.to_string());
        self
    }

    /// Set retry parameters
    pub fn with_retries(mut self, max_retries: u32, base_delay: Duration) -> Self {
        self.max_retries = max_retries;
        self.retry_base_delay = base_delay;
        self
    }
}

// ============================================================================
// Main Client
// ============================================================================

/// Unified client for all AURELIUM infrastructure services
pub struct AureliumClient {
    nats_client: async_nats::Client,
    db_pool: sqlx::PgPool,
    qdrant_url: Option<String>,
    neo4j_url: Option<String>,
    neo4j_user: Option<String>,
    neo4j_password: Option<String>,
}

impl AureliumClient {
    /// Create a new client with just NATS and PostgreSQL (backward-compatible)
    pub async fn new(nats_url: &str, db_url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let config = AureliumConfig::new(nats_url, db_url);
        Self::from_config(config).await.map_err(|e| e.into())
    }

    /// Create a client from a full configuration
    pub async fn from_config(config: AureliumConfig) -> AureliumResult<Self> {
        info!("Connecting to NATS at {}...", config.nats_url);
        let nats_client = Self::connect_nats_with_retry(
            &config.nats_url,
            config.max_retries,
            config.retry_base_delay,
        )
        .await?;
        info!("NATS connected successfully.");

        info!("Connecting to PostgreSQL...");
        let db_pool = Self::connect_db_with_retry(
            &config.database_url,
            config.max_retries,
            config.retry_base_delay,
        )
        .await?;
        info!("PostgreSQL connected successfully.");

        Ok(Self {
            nats_client,
            db_pool,
            qdrant_url: config.qdrant_url,
            neo4j_url: config.neo4j_url,
            neo4j_user: config.neo4j_user,
            neo4j_password: config.neo4j_password,
        })
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    /// Get a reference to the NATS client
    pub fn nats(&self) -> &async_nats::Client {
        &self.nats_client
    }

    /// Get a reference to the PostgreSQL connection pool
    pub fn db(&self) -> &sqlx::PgPool {
        &self.db_pool
    }

    /// Get the Qdrant URL if configured
    pub fn qdrant_url(&self) -> Option<&str> {
        self.qdrant_url.as_deref()
    }

    /// Get the Neo4j URL if configured
    pub fn neo4j_url(&self) -> Option<&str> {
        self.neo4j_url.as_deref()
    }

    /// Get Neo4j credentials if configured
    pub fn neo4j_credentials(&self) -> Option<(&str, &str)> {
        match (&self.neo4j_user, &self.neo4j_password) {
            (Some(user), Some(pass)) => Some((user.as_str(), pass.as_str())),
            _ => None,
        }
    }

    // -----------------------------------------------------------------------
    // Event Publishing Helpers
    // -----------------------------------------------------------------------

    /// Publish a typed event to NATS
    pub async fn publish<T: serde::Serialize>(
        &self,
        subject: &str,
        payload: &T,
    ) -> AureliumResult<()> {
        let bytes = serde_json::to_vec(payload)?;
        self.nats_client
            .publish(subject.to_string(), bytes.into())
            .await
            .map_err(|e| AureliumError::Nats(e.into()))?;
        Ok(())
    }

    /// Make a NATS request-reply call with timeout
    pub async fn request<T: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        subject: &str,
        payload: &T,
        timeout: Duration,
    ) -> AureliumResult<R> {
        let bytes = serde_json::to_vec(payload)?;
        let reply = tokio::time::timeout(
            timeout,
            self.nats_client
                .request(subject.to_string(), bytes.into()),
        )
        .await
        .map_err(|_| AureliumError::Timeout(format!("Request to '{}' timed out", subject)))?
        .map_err(|e| AureliumError::Nats(e.into()))?;

        let response: R = serde_json::from_slice(&reply.payload)?;
        Ok(response)
    }

    // -----------------------------------------------------------------------
    // Connection helpers with retry
    // -----------------------------------------------------------------------

    async fn connect_nats_with_retry(
        url: &str,
        max_retries: u32,
        base_delay: Duration,
    ) -> AureliumResult<async_nats::Client> {
        let mut delay = base_delay;
        for attempt in 1..=max_retries {
            match async_nats::connect(url).await {
                Ok(client) => return Ok(client),
                Err(e) => {
                    if attempt == max_retries {
                        error!("Failed to connect to NATS after {} attempts: {}", max_retries, e);
                        return Err(AureliumError::Nats(e.into()));
                    }
                    error!(
                        "NATS connection attempt {}/{} failed: {}. Retrying in {:?}...",
                        attempt, max_retries, e, delay
                    );
                    tokio::time::sleep(delay).await;
                    delay *= 2;
                }
            }
        }
        Err(AureliumError::Other("Unreachable".to_string()))
    }

    async fn connect_db_with_retry(
        url: &str,
        max_retries: u32,
        base_delay: Duration,
    ) -> AureliumResult<sqlx::PgPool> {
        let mut delay = base_delay;
        for attempt in 1..=max_retries {
            match sqlx::PgPool::connect(url).await {
                Ok(pool) => return Ok(pool),
                Err(e) => {
                    if attempt == max_retries {
                        error!(
                            "Failed to connect to PostgreSQL after {} attempts: {}",
                            max_retries, e
                        );
                        return Err(AureliumError::Database(e));
                    }
                    error!(
                        "PostgreSQL connection attempt {}/{} failed: {}. Retrying in {:?}...",
                        attempt, max_retries, e, delay
                    );
                    tokio::time::sleep(delay).await;
                    delay *= 2;
                }
            }
        }
        Err(AureliumError::Other("Unreachable".to_string()))
    }
}
