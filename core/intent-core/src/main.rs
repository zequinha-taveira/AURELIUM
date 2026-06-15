use aurelium_sdk::AureliumClient;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::env;
use std::time::Duration;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

// ============================================================================
// Event Types
// ============================================================================

#[derive(Serialize, Deserialize, Debug, Clone)]
struct IntentReceivedEvent {
    goal: String,
    #[serde(default)]
    context: Option<serde_json::Value>,
    #[serde(default = "default_priority")]
    priority: String,
}

fn default_priority() -> String {
    "medium".to_string()
}

#[derive(Serialize, Deserialize, Debug)]
struct GoalDecomposeRequest {
    goal_id: String,
    goal: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct DecomposedMission {
    title: String,
    description: String,
    priority: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct DecomposedResponse {
    missions: Vec<DecomposedMission>,
}

#[derive(Serialize, Deserialize, Debug)]
struct MissionGeneratedEvent {
    goal_id: String,
    missions: Vec<DecomposedMission>,
}

#[derive(Serialize, Deserialize, Debug)]
struct IntentFailedEvent {
    goal_id: String,
    reason: String,
    error: Option<String>,
}

// ============================================================================
// Configuration
// ============================================================================

struct IntentCoreConfig {
    nats_url: String,
    db_url: String,
    decompose_timeout_secs: u64,
    max_retries: u32,
    max_goal_length: usize,
}

impl IntentCoreConfig {
    fn from_env() -> Self {
        Self {
            nats_url: env::var("NATS_URL")
                .unwrap_or_else(|_| "nats://localhost:4222".to_string()),
            db_url: env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://aurelium:aurelium@localhost:5432/aurelium".to_string()
            }),
            decompose_timeout_secs: env::var("DECOMPOSE_TIMEOUT_SECS")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .unwrap_or(30),
            max_retries: env::var("MAX_RETRIES")
                .unwrap_or_else(|_| "3".to_string())
                .parse()
                .unwrap_or(3),
            max_goal_length: env::var("MAX_GOAL_LENGTH")
                .unwrap_or_else(|_| "5000".to_string())
                .parse()
                .unwrap_or(5000),
        }
    }
}

// ============================================================================
// Metrics (simple counters for now)
// ============================================================================

use std::sync::atomic::{AtomicU64, Ordering};

static GOALS_RECEIVED: AtomicU64 = AtomicU64::new(0);
static GOALS_COMPLETED: AtomicU64 = AtomicU64::new(0);
static GOALS_FAILED: AtomicU64 = AtomicU64::new(0);

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize structured JSON logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
        ))
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    let config = IntentCoreConfig::from_env();

    info!(
        service = "intent-core",
        version = "0.2.0",
        "Starting AURELIUM Intent Core Service..."
    );

    info!("Connecting to NATS at {} and PostgreSQL...", config.nats_url);
    let client = AureliumClient::new(&config.nats_url, &config.db_url).await?;
    info!("Successfully connected to system dependencies.");

    // Subscribe to intent.received events
    let mut subscription = client
        .nats()
        .subscribe("intent.received".to_string())
        .await?;
    info!("Listening on NATS subject 'intent.received'");

    // Spawn metrics reporter
    let metrics_nats_url = config.nats_url.clone();
    let metrics_db_url = config.db_url.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            info!(
                goals_received = GOALS_RECEIVED.load(Ordering::Relaxed),
                goals_completed = GOALS_COMPLETED.load(Ordering::Relaxed),
                goals_failed = GOALS_FAILED.load(Ordering::Relaxed),
                "Intent Core metrics report"
            );
        }
    });

    while let Some(message) = subscription.next().await {
        let payload = match serde_json::from_slice::<IntentReceivedEvent>(&message.payload) {
            Ok(event) => event,
            Err(e) => {
                error!("Failed to deserialize intent event: {:?}", e);
                continue;
            }
        };

        GOALS_RECEIVED.fetch_add(1, Ordering::Relaxed);
        info!(goal = %payload.goal, priority = %payload.priority, "Received new goal");

        // Validate input
        if payload.goal.trim().is_empty() {
            warn!("Received empty goal, skipping");
            continue;
        }
        if payload.goal.len() > config.max_goal_length {
            warn!(
                len = payload.goal.len(),
                max = config.max_goal_length,
                "Goal exceeds maximum length, skipping"
            );
            continue;
        }

        // Spawn async task to process
        let nats_url = config.nats_url.clone();
        let db_url = config.db_url.clone();
        let timeout = config.decompose_timeout_secs;
        let max_retries = config.max_retries;

        tokio::spawn(async move {
            match process_intent_with_retry(&nats_url, &db_url, payload, timeout, max_retries)
                .await
            {
                Ok(()) => {
                    GOALS_COMPLETED.fetch_add(1, Ordering::Relaxed);
                }
                Err(e) => {
                    GOALS_FAILED.fetch_add(1, Ordering::Relaxed);
                    error!("Error processing intent: {:?}", e);
                }
            }
        });
    }

    Ok(())
}

// ============================================================================
// Processing with Retry
// ============================================================================

async fn process_intent_with_retry(
    nats_url: &str,
    db_url: &str,
    event: IntentReceivedEvent,
    timeout_secs: u64,
    max_retries: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut last_error: Option<Box<dyn std::error::Error>> = None;
    let mut delay = Duration::from_millis(500);

    for attempt in 1..=max_retries {
        let client = AureliumClient::new(nats_url, db_url).await?;

        match process_intent(&client, &event, timeout_secs).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                if attempt == max_retries {
                    error!(
                        attempt = attempt,
                        max_retries = max_retries,
                        error = %e,
                        "Final retry attempt failed"
                    );
                    // Publish failure event
                    let _ = publish_failure(&client, &event, &e.to_string()).await;
                    last_error = Some(e);
                } else {
                    warn!(
                        attempt = attempt,
                        max_retries = max_retries,
                        error = %e,
                        retry_in_ms = delay.as_millis() as u64,
                        "Intent processing failed, retrying..."
                    );
                    tokio::time::sleep(delay).await;
                    delay *= 2; // Exponential backoff
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| "Unknown error".into()))
}

// ============================================================================
// Core Processing Logic
// ============================================================================

async fn process_intent(
    client: &AureliumClient,
    event: &IntentReceivedEvent,
    timeout_secs: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let goal_id = Uuid::new_v4();

    // 1. Save Goal to Postgres in pending state
    info!(goal_id = %goal_id, goal = %event.goal, "Persisting new goal");
    sqlx::query(
        "INSERT INTO goals (id, raw_input, status) VALUES ($1, $2, $3)"
    )
    .bind(goal_id)
    .bind(&event.goal)
    .bind("processing")
    .execute(client.db())
    .await?;

    // 2. Request decomposition from AI Agent over NATS
    let decompose_req = GoalDecomposeRequest {
        goal_id: goal_id.to_string(),
        goal: event.goal.clone(),
        context: event.context.clone(),
    };
    let payload = serde_json::to_vec(&decompose_req)?;

    info!(goal_id = %goal_id, "Requesting goal decomposition from AI Swarm");

    // Using NATS Request-Reply pattern with configurable timeout
    let reply_future = client
        .nats()
        .request("intent.decompose".to_string(), payload.into());
    let reply_message = match tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        reply_future,
    )
    .await
    {
        Ok(Ok(msg)) => msg,
        Ok(Err(e)) => {
            error!(goal_id = %goal_id, error = %e, "NATS request error during decomposition");
            mark_goal_failed(client, goal_id).await?;
            return Err(e.into());
        }
        Err(_) => {
            error!(goal_id = %goal_id, timeout_secs = timeout_secs, "Timeout waiting for AI Agent");
            mark_goal_failed(client, goal_id).await?;
            return Err("AI Agent decomposition timeout".into());
        }
    };

    // 3. Parse decomposed response
    let decomp_res: DecomposedResponse = match serde_json::from_slice(&reply_message.payload) {
        Ok(res) => res,
        Err(e) => {
            error!(goal_id = %goal_id, error = %e, "Failed to parse AI Agent response");
            mark_goal_failed(client, goal_id).await?;
            return Err(e.into());
        }
    };

    if decomp_res.missions.is_empty() {
        warn!(goal_id = %goal_id, "AI Agent returned zero missions");
        mark_goal_failed(client, goal_id).await?;
        return Err("AI Agent returned no missions".into());
    }

    info!(
        goal_id = %goal_id,
        mission_count = decomp_res.missions.len(),
        "Received missions from AI Agent"
    );

    // 4. Save missions to Postgres
    for mission in &decomp_res.missions {
        let mission_id = Uuid::new_v4();
        info!(
            goal_id = %goal_id,
            mission_id = %mission_id,
            title = %mission.title,
            priority = %mission.priority,
            "Saving mission"
        );
        sqlx::query(
            "INSERT INTO missions (id, goal_id, title, description, priority, status) VALUES ($1, $2, $3, $4, $5, $6)"
        )
        .bind(mission_id)
        .bind(goal_id)
        .bind(&mission.title)
        .bind(&mission.description)
        .bind(&mission.priority)
        .bind("pending")
        .execute(client.db())
        .await?;
    }

    // 5. Update goal status to completed
    sqlx::query("UPDATE goals SET status = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2")
        .bind("completed")
        .bind(goal_id)
        .execute(client.db())
        .await?;

    // 6. Publish mission.generated event
    let generated_event = MissionGeneratedEvent {
        goal_id: goal_id.to_string(),
        missions: decomp_res.missions,
    };
    let event_payload = serde_json::to_vec(&generated_event)?;
    client
        .nats()
        .publish("mission.generated".to_string(), event_payload.into())
        .await?;

    info!(goal_id = %goal_id, "Successfully processed intent goal");
    Ok(())
}

// ============================================================================
// Helpers
// ============================================================================

async fn mark_goal_failed(
    client: &AureliumClient,
    goal_id: Uuid,
) -> Result<(), Box<dyn std::error::Error>> {
    sqlx::query("UPDATE goals SET status = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2")
        .bind("failed")
        .bind(goal_id)
        .execute(client.db())
        .await?;
    Ok(())
}

async fn publish_failure(
    client: &AureliumClient,
    event: &IntentReceivedEvent,
    error_msg: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let failure = IntentFailedEvent {
        goal_id: "unknown".to_string(),
        reason: format!("Failed to process goal: '{}'", event.goal),
        error: Some(error_msg.to_string()),
    };
    let payload = serde_json::to_vec(&failure)?;
    client
        .nats()
        .publish("intent.failed".to_string(), payload.into())
        .await?;
    Ok(())
}
