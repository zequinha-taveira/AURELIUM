mod mutation;
mod neo4j;

use aurelium_sdk::AureliumClient;
use futures::StreamExt;
use mutation::{mutate_genome, CapabilityGenome};
use neo4j::Neo4jClient;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Serialize, Deserialize, Debug)]
struct MutateRequest {
    capability_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct TelemetryReport {
    variant_id: String,
    latency_ms: f64,
    error_rate: f64,
    success_rate: f64,
}

#[derive(Serialize, Deserialize, Debug)]
struct EvaluateRequest {
    capability_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct EvaluateResponse {
    status: String, // "promoted" or "extinguished"
    winner: Option<String>,
    fitness: f64,
    baseline_fitness: f64,
}

struct VariantMetrics {
    variant_id: String,
    avg_latency: f64,
    avg_error: f64,
    avg_success: f64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("info"))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting AURELIUM Evolution Engine (Darwin)...");

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

    // 1. Subscribe to evolution.mutate
    let client_mutate = client.clone();
    let mut mutate_sub = client
        .nats()
        .subscribe("evolution.mutate".to_string())
        .await?;
    info!("Listening on NATS subject 'evolution.mutate' for mutation triggers.");

    tokio::spawn(async move {
        while let Some(msg) = mutate_sub.next().await {
            let req_str = match std::str::from_utf8(&msg.payload) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let capability_id = if let Ok(req) = serde_json::from_str::<MutateRequest>(req_str) {
                req.capability_id
            } else {
                req_str.trim().replace('"', "")
            };

            let client_c = client_mutate.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_mutate(client_c, &capability_id).await {
                    error!(
                        "Error during genome mutation for {}: {:?}",
                        capability_id, e
                    );
                }
            });
        }
    });

    // 2. Subscribe to evolution.telemetry.report
    let client_telemetry = client.clone();
    let mut telemetry_sub = client
        .nats()
        .subscribe("evolution.telemetry.report".to_string())
        .await?;
    info!("Listening on NATS subject 'evolution.telemetry.report' for metric collection.");

    tokio::spawn(async move {
        while let Some(msg) = telemetry_sub.next().await {
            let req_str = match std::str::from_utf8(&msg.payload) {
                Ok(s) => s,
                Err(_) => continue,
            };

            if let Ok(report) = serde_json::from_str::<TelemetryReport>(req_str) {
                let client_c = client_telemetry.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_telemetry(client_c, report).await {
                        error!("Error logging telemetry: {:?}", e);
                    }
                });
            }
        }
    });

    // 3. Subscribe to evolution.evaluate (Request-Reply RPC)
    let client_eval = client.clone();
    let neo4j_eval = neo4j_client.clone();
    let mut eval_sub = client
        .nats()
        .subscribe("evolution.evaluate".to_string())
        .await?;
    info!("Listening on NATS subject 'evolution.evaluate' for selection & evaluation.");

    while let Some(msg) = eval_sub.next().await {
        let reply = match msg.reply.clone() {
            Some(r) => r,
            None => {
                error!("Received evolution.evaluate request with no reply subject.");
                continue;
            }
        };

        let payload = msg.payload.clone();
        let client_c = client_eval.clone();
        let neo4j_c = neo4j_eval.clone();

        tokio::spawn(async move {
            let req_str = match std::str::from_utf8(&payload) {
                Ok(s) => s,
                Err(_) => "",
            };

            let capability_id = if let Ok(req) = serde_json::from_str::<EvaluateRequest>(req_str) {
                req.capability_id
            } else {
                req_str.trim().replace('"', "")
            };

            info!(
                "RPC Query: Evaluating variants for capability: {}",
                capability_id
            );
            let client_reply = client_c.clone();
            let response = match handle_evaluation(client_c, neo4j_c, &capability_id).await {
                Ok(res) => res,
                Err(e) => {
                    error!("Error during evaluation for {}: {:?}", capability_id, e);
                    EvaluateResponse {
                        status: "error".to_string(),
                        winner: None,
                        fitness: 0.0,
                        baseline_fitness: 0.0,
                    }
                }
            };

            if let Ok(resp_payload) = serde_json::to_vec(&response) {
                if let Err(e) = client_reply
                    .nats()
                    .publish(reply, resp_payload.into())
                    .await
                {
                    error!("Failed to publish evaluation response: {:?}", e);
                }
            }
        });
    }

    Ok(())
}

async fn handle_mutate(
    client: Arc<AureliumClient>,
    capability_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Triggering mutation for capability: {}", capability_id);

    // 1. Fetch baseline raw yaml from postgres
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
            return Err(format!("Base capability {} not found in registry", capability_id).into());
        }
    };

    // 2. Perform mutation
    let (mutated_genome, mutated_yaml) = mutate_genome(&raw_yaml)?;
    info!("Generated mutation variant: {}", mutated_genome.id);

    // 3. Save to genome_variants in PG
    sqlx::query(
        "INSERT INTO genome_variants (id, parent_id, version, raw_yaml, status) \
         VALUES ($1, $2, $3, $4, 'active')",
    )
    .bind(&mutated_genome.id)
    .bind(capability_id)
    .bind(&mutated_genome.version)
    .bind(&mutated_yaml)
    .execute(client.db())
    .await?;

    info!(
        "Saved mutation variant metadata for {} to database.",
        mutated_genome.id
    );

    // 4. Register the variant capability in NATS so Semantic Genome Engine indexes it
    client
        .nats()
        .publish("genome.register".to_string(), mutated_yaml.into())
        .await?;

    info!(
        "Published registration request for variant: {}",
        mutated_genome.id
    );
    Ok(())
}

async fn handle_telemetry(
    client: Arc<AureliumClient>,
    report: TelemetryReport,
) -> Result<(), Box<dyn std::error::Error>> {
    info!(
        "Logging telemetry for variant '{}': latency={}ms error={} success={}",
        report.variant_id, report.latency_ms, report.error_rate, report.success_rate
    );

    sqlx::query(
        "INSERT INTO variant_telemetry (variant_id, latency_ms, error_rate, success_rate) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(&report.variant_id)
    .bind(report.latency_ms)
    .bind(report.error_rate)
    .bind(report.success_rate)
    .execute(client.db())
    .await?;

    Ok(())
}

async fn handle_evaluation(
    client: Arc<AureliumClient>,
    neo4j: Arc<Neo4jClient>,
    capability_id: &str,
) -> Result<EvaluateResponse, Box<dyn std::error::Error>> {
    info!("Starting evaluation for capability: {}", capability_id);

    // 1. Fetch active variants from DB
    let variant_rows = sqlx::query(
        "SELECT id, raw_yaml FROM genome_variants WHERE parent_id = $1 AND status = 'active'",
    )
    .bind(capability_id)
    .fetch_all(client.db())
    .await?;

    if variant_rows.is_empty() {
        info!(
            "No active variants found to evaluate for capability: {}",
            capability_id
        );
        return Ok(EvaluateResponse {
            status: "extinguished".to_string(),
            winner: None,
            fitness: 0.0,
            baseline_fitness: 0.0,
        });
    }

    // 2. Fetch average telemetry for baseline & active variants
    let metrics_rows = sqlx::query(
        "SELECT variant_id, AVG(latency_ms) as avg_latency, AVG(error_rate) as avg_error, AVG(success_rate) as avg_success \
         FROM variant_telemetry \
         WHERE variant_id = $1 OR variant_id IN (SELECT id FROM genome_variants WHERE parent_id = $1 AND status = 'active') \
         GROUP BY variant_id"
    )
    .bind(capability_id)
    .fetch_all(client.db())
    .await?;

    let mut metrics_list = Vec::new();
    for row in metrics_rows {
        let variant_id: String = sqlx::Row::get(&row, "variant_id");
        let avg_latency: f64 = sqlx::Row::get(&row, "avg_latency");
        let avg_error: f64 = sqlx::Row::get(&row, "avg_error");
        let avg_success: f64 = sqlx::Row::get(&row, "avg_success");
        metrics_list.push(VariantMetrics {
            variant_id,
            avg_latency,
            avg_error,
            avg_success,
        });
    }

    // Function to calculate fitness: success / (1.0 + latency * 0.001) * (1.0 - error)
    let calc_fitness = |m: &VariantMetrics| -> f64 {
        m.avg_success / (1.0 + m.avg_latency * 0.001) * (1.0 - m.avg_error)
    };

    // Find baseline metrics and fitness
    let baseline_metrics = metrics_list.iter().find(|m| m.variant_id == capability_id);
    let baseline_fitness = match baseline_metrics {
        Some(m) => calc_fitness(m),
        None => {
            // Default baseline fitness if no telemetry has been logged yet
            info!(
                "No telemetry logged for baseline capability '{}', using default baseline fitness.",
                capability_id
            );
            // Default: success=1.0, latency=100.0, error=0.0 -> fitness = 1.0 / 1.1 = 0.909
            1.0 / (1.0 + 100.0 * 0.001) * (1.0 - 0.0)
        }
    };

    info!("Baseline capability fitness score: {:.4}", baseline_fitness);

    // Find best variant
    let mut best_variant_id: Option<String> = None;
    let mut best_fitness = 0.0;

    for variant_row in &variant_rows {
        let v_id: String = sqlx::Row::get(variant_row, "id");
        let v_metrics = metrics_list.iter().find(|m| m.variant_id == v_id);
        let v_fitness = match v_metrics {
            Some(m) => calc_fitness(m),
            None => {
                // Default variant fitness if no telemetry has been logged yet
                1.0 / (1.0 + 100.0 * 0.001) * (1.0 - 0.0)
            }
        };

        info!("Variant '{}' fitness score: {:.4}", v_id, v_fitness);

        if v_fitness > best_fitness {
            best_fitness = v_fitness;
            best_variant_id = Some(v_id);
        }
    }

    // 3. Compare and apply Natural Selection
    if let Some(winner_id) = best_variant_id {
        if best_fitness > baseline_fitness {
            // PROMOTION (Reproduction & Selection)
            info!(
                "Variant '{}' out-performed baseline! Promoting...",
                winner_id
            );

            // Fetch winner raw YAML
            let winner_row = variant_rows
                .iter()
                .find(|r| {
                    let id: String = sqlx::Row::get(*r, "id");
                    id == winner_id
                })
                .unwrap();
            let winner_yaml: String = sqlx::Row::get(winner_row, "raw_yaml");

            // Parse winner YAML, update its ID back to the main baseline ID
            let mut winner_genome: CapabilityGenome = serde_yaml::from_str(&winner_yaml)?;
            let new_version = winner_genome.version.clone();
            winner_genome.id = capability_id.to_string();

            let promoted_yaml = serde_yaml::to_string(&winner_genome)?;

            // Update baseline in Postgres capabilities table
            sqlx::query(
                "UPDATE capabilities SET name = $1, version = $2, description = $3, raw_yaml = $4, updated_at = CURRENT_TIMESTAMP WHERE id = $5"
            )
            .bind(&winner_genome.name)
            .bind(&new_version)
            .bind(&winner_genome.description)
            .bind(&promoted_yaml)
            .bind(capability_id)
            .execute(client.db())
            .await?;

            // Publish genome.register to update Semantic Genome registry (PG & Neo4j graph nodes)
            client
                .nats()
                .publish("genome.register".to_string(), promoted_yaml.into())
                .await?;

            // Mark winning variant status as promoted in PG
            sqlx::query("UPDATE genome_variants SET status = 'promoted' WHERE id = $1")
                .bind(&winner_id)
                .execute(client.db())
                .await?;

            // Mark all other variants as retired in PG
            sqlx::query("UPDATE genome_variants SET status = 'retired' WHERE parent_id = $1 AND status = 'active'")
                .bind(capability_id)
                .execute(client.db())
                .await?;

            // Extinguish other variants from Neo4j
            for v_row in &variant_rows {
                let v_id: String = sqlx::Row::get(v_row, "id");
                if v_id != winner_id {
                    let _ = neo4j.delete_capability(&v_id).await;
                }
            }

            return Ok(EvaluateResponse {
                status: "promoted".to_string(),
                winner: Some(winner_id),
                fitness: best_fitness,
                baseline_fitness,
            });
        }
    }

    // EXTINCTION (Variants did not out-perform baseline)
    info!("No variants out-performed the baseline capability. Extinguishing all variants...");

    // Mark active variants as retired in PG
    sqlx::query(
        "UPDATE genome_variants SET status = 'retired' WHERE parent_id = $1 AND status = 'active'",
    )
    .bind(capability_id)
    .execute(client.db())
    .await?;

    // Delete variants from Neo4j
    for v_row in &variant_rows {
        let v_id: String = sqlx::Row::get(v_row, "id");
        let _ = neo4j.delete_capability(&v_id).await;
    }

    Ok(EvaluateResponse {
        status: "extinguished".to_string(),
        winner: None,
        fitness: 0.0,
        baseline_fitness,
    })
}
