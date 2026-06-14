mod neo4j;
mod parser;

use aurelium_sdk::AureliumClient;
use futures::StreamExt;
use neo4j::Neo4jClient;
use parser::parse_genome;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Serialize, Deserialize, Debug)]
struct DependencyRequest {
    capability_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct DependencyResponse {
    dependencies: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct ValidateRequest {
    capability_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct ValidateResponse {
    valid: bool,
    missing_dependencies: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("info"))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting AURELIUM Semantic Genome (Digital DNA) Engine...");

    let nats_url = env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());
    let db_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://aurelium:aurelium@localhost:5432/aurelium".to_string());

    let neo4j_url = env::var("NEO4J_URL").unwrap_or_else(|_| "http://localhost:7474".to_string());
    let neo4j_user = env::var("NEO4J_USER").unwrap_or_else(|_| "neo4j".to_string());
    let neo4j_pass = env::var("NEO4J_PASSWORD").unwrap_or_else(|_| "aurelium".to_string());

    info!("Connecting to NATS, PostgreSQL, and Neo4j REST API...");
    let client = Arc::new(AureliumClient::new(&nats_url, &db_url).await?);
    let neo4j_client = Arc::new(Neo4jClient::new(&neo4j_url, &neo4j_user, &neo4j_pass));
    info!("Successfully connected to system dependencies.");

    // 1. Subscribe to genome.register (one-way events)
    let client_reg = client.clone();
    let neo4j_reg = neo4j_client.clone();
    let mut reg_sub = client
        .nats()
        .subscribe("genome.register".to_string())
        .await?;
    info!("Listening on NATS subject 'genome.register' for registrations.");

    tokio::spawn(async move {
        while let Some(msg) = reg_sub.next().await {
            let yaml_str = match std::str::from_utf8(&msg.payload) {
                Ok(s) => s,
                Err(e) => {
                    error!("Invalid UTF-8 payload in genome.register: {:?}", e);
                    continue;
                }
            };

            let client_c = client_reg.clone();
            let neo4j_c = neo4j_reg.clone();
            let yaml_c = yaml_str.to_string();

            tokio::spawn(async move {
                if let Err(e) = handle_registration(client_c, neo4j_c, &yaml_c).await {
                    error!("Error during genome registration: {:?}", e);
                }
            });
        }
    });

    // 2. Subscribe to genome.get_dependencies (Request-Reply RPC)
    let neo4j_dep = neo4j_client.clone();
    let client_dep = client.clone();
    let mut dep_sub = client
        .nats()
        .subscribe("genome.get_dependencies".to_string())
        .await?;
    info!("Listening on NATS subject 'genome.get_dependencies' for RPC queries.");

    tokio::spawn(async move {
        while let Some(msg) = dep_sub.next().await {
            let reply = match msg.reply.clone() {
                Some(r) => r,
                None => {
                    error!("Received genome.get_dependencies query with no reply subject.");
                    continue;
                }
            };

            let payload = msg.payload.clone();
            let client_c = client_dep.clone();
            let neo4j_c = neo4j_dep.clone();

            tokio::spawn(async move {
                let req_str = match std::str::from_utf8(&payload) {
                    Ok(s) => s,
                    Err(_) => "",
                };

                // Try parsing as JSON first, fallback to raw string
                let capability_id =
                    if let Ok(req) = serde_json::from_str::<DependencyRequest>(req_str) {
                        req.capability_id
                    } else {
                        req_str.trim().replace('"', "")
                    };

                info!(
                    "RPC Query: Fetching dependencies for capability: {}",
                    capability_id
                );
                let response = match neo4j_c.get_recursive_dependencies(&capability_id).await {
                    Ok(deps) => DependencyResponse { dependencies: deps },
                    Err(e) => {
                        error!("Failed to fetch dependencies from Neo4j: {:?}", e);
                        DependencyResponse {
                            dependencies: vec![],
                        }
                    }
                };

                if let Ok(resp_payload) = serde_json::to_vec(&response) {
                    if let Err(e) = client_c.nats().publish(reply, resp_payload.into()).await {
                        error!("Failed to publish dependency response: {:?}", e);
                    }
                }
            });
        }
    });

    // 3. Subscribe to genome.validate (Request-Reply RPC)
    let client_val = client.clone();
    let mut val_sub = client
        .nats()
        .subscribe("genome.validate".to_string())
        .await?;
    info!("Listening on NATS subject 'genome.validate' for validation requests.");

    while let Some(msg) = val_sub.next().await {
        let reply = match msg.reply.clone() {
            Some(r) => r,
            None => {
                error!("Received genome.validate request with no reply subject.");
                continue;
            }
        };

        let payload = msg.payload.clone();
        let client_c = client_val.clone();

        tokio::spawn(async move {
            let req_str = match std::str::from_utf8(&payload) {
                Ok(s) => s,
                Err(_) => "",
            };

            let capability_id = if let Ok(req) = serde_json::from_str::<ValidateRequest>(req_str) {
                req.capability_id
            } else {
                req_str.trim().replace('"', "")
            };

            info!("RPC Query: Validating capability: {}", capability_id);
            let response = match validate_capability(&client_c, &capability_id).await {
                Ok(res) => res,
                Err(e) => {
                    error!("Error during validation: {:?}", e);
                    ValidateResponse {
                        valid: false,
                        missing_dependencies: vec![],
                    }
                }
            };

            if let Ok(resp_payload) = serde_json::to_vec(&response) {
                if let Err(e) = client_c.nats().publish(reply, resp_payload.into()).await {
                    error!("Failed to publish validate response: {:?}", e);
                }
            }
        });
    }

    Ok(())
}

async fn handle_registration(
    client: Arc<AureliumClient>,
    neo4j: Arc<Neo4jClient>,
    yaml_str: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Parse YAML
    let genome = parse_genome(yaml_str)?;
    info!(
        "Parsed capability genome: {} (v{})",
        genome.id, genome.version
    );

    // 2. Save to Postgres
    sqlx::query(
        "INSERT INTO capabilities (id, name, version, description, raw_yaml, updated_at) \
         VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP) \
         ON CONFLICT (id) DO UPDATE SET \
            name = EXCLUDED.name, \
            version = EXCLUDED.version, \
            description = EXCLUDED.description, \
            raw_yaml = EXCLUDED.raw_yaml, \
            updated_at = CURRENT_TIMESTAMP",
    )
    .bind(&genome.id)
    .bind(&genome.name)
    .bind(&genome.version)
    .bind(&genome.description)
    .bind(yaml_str)
    .execute(client.db())
    .await?;

    info!("Saved genome metadata for {} to PostgreSQL.", genome.id);

    // 3. Sync to Neo4j
    neo4j.sync_genome(&genome).await?;

    info!("Finished registering capability genome: {}", genome.id);
    Ok(())
}

async fn validate_capability(
    client: &AureliumClient,
    capability_id: &str,
) -> Result<ValidateResponse, Box<dyn std::error::Error>> {
    // 1. Fetch capability raw yaml from postgres
    let row = sqlx::query("SELECT raw_yaml FROM capabilities WHERE id = $1")
        .bind(capability_id)
        .fetch_optional(client.db())
        .await?;

    let raw_yaml = match row {
        Some(r) => {
            let s: String = sqlx::Row::get(&r, "raw_yaml");
            s
        }
        None => {
            return Ok(ValidateResponse {
                valid: false,
                missing_dependencies: vec![capability_id.to_string()],
            });
        }
    };

    // 2. Parse YAML to find dependencies
    let genome = parse_genome(&raw_yaml)?;
    let mut missing_dependencies = Vec::new();

    if let Some(dependencies) = &genome.dependencies {
        for dep in dependencies {
            // Check if dependency exists in postgres capabilities table
            let count_row = sqlx::query("SELECT COUNT(*) FROM capabilities WHERE id = $1")
                .bind(dep)
                .fetch_one(client.db())
                .await?;

            let count: i64 = sqlx::Row::get(&count_row, 0);
            if count == 0 {
                missing_dependencies.push(dep.clone());
            }
        }
    }

    let valid = missing_dependencies.is_empty();
    Ok(ValidateResponse {
        valid,
        missing_dependencies,
    })
}
