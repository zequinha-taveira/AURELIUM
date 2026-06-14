use aurelium_sdk::AureliumClient;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::env;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct IntentReceivedEvent {
    goal: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct GoalDecomposeRequest {
    goal_id: String,
    goal: String,
}

#[derive(Serialize, Deserialize, Debug)]
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("info"))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting AURELIUM Intent Core Service...");

    // Connection URLs from environment or defaults
    let nats_url = env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());
    let db_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://aurelium:aurelium@localhost:5432/aurelium".to_string());

    info!("Connecting to NATS at {} and PostgreSQL...", nats_url);
    let client = AureliumClient::new(&nats_url, &db_url).await?;
    info!("Successfully connected to system dependencies.");

    // Subscribe to intent.received events
    let mut subscription = client
        .nats()
        .subscribe("intent.received".to_string())
        .await?;
    info!("Listening on NATS subject 'intent.received'");

    while let Some(message) = subscription.next().await {
        let payload = match serde_json::from_slice::<IntentReceivedEvent>(&message.payload) {
            Ok(event) => event,
            Err(e) => {
                error!("Failed to deserialize intent event: {:?}", e);
                continue;
            }
        };

        info!("Received goal: '{}'", payload.goal);

        // Spawn a task to process the intent asynchronously
        let client_clone = AureliumClient::new(&nats_url, &db_url).await?;
        tokio::spawn(async move {
            if let Err(e) = process_intent(client_clone, payload).await {
                error!("Error processing intent: {:?}", e);
            }
        });
    }

    Ok(())
}

async fn process_intent(
    client: AureliumClient,
    event: IntentReceivedEvent,
) -> Result<(), Box<dyn std::error::Error>> {
    let goal_id = Uuid::new_v4();

    // 1. Save Goal to Postgres in pending state
    info!("Persisting new goal: {} (ID: {})", event.goal, goal_id);
    sqlx::query("INSERT INTO goals (id, raw_input, status) VALUES ($1, $2, $3)")
        .bind(goal_id)
        .bind(&event.goal)
        .bind("pending")
        .execute(client.db())
        .await?;

    // 2. Request decomposition from AI Agent over NATS
    let decompose_req = GoalDecomposeRequest {
        goal_id: goal_id.to_string(),
        goal: event.goal.clone(),
    };
    let payload = serde_json::to_vec(&decompose_req)?;

    info!(
        "Requesting goal decomposition from AI Swarm for goal: {}",
        goal_id
    );

    // Using NATS Request-Reply pattern with 30s timeout
    let reply_future = client
        .nats()
        .request("intent.decompose".to_string(), payload.into());
    let reply_message =
        match tokio::time::timeout(std::time::Duration::from_secs(30), reply_future).await {
            Ok(Ok(msg)) => msg,
            Ok(Err(e)) => {
                error!("NATS request error during decomposition: {:?}", e);
                mark_goal_failed(&client, goal_id).await?;
                return Err(e.into());
            }
            Err(_) => {
                error!("Timeout waiting for AI Agent decomposition response.");
                mark_goal_failed(&client, goal_id).await?;
                return Err("AI Agent decomposition timeout".into());
            }
        };

    // 3. Parse decomposed response
    let decomp_res: DecomposedResponse = match serde_json::from_slice(&reply_message.payload) {
        Ok(res) => res,
        Err(e) => {
            error!("Failed to parse AI Agent response: {:?}", e);
            mark_goal_failed(&client, goal_id).await?;
            return Err(e.into());
        }
    };

    info!(
        "Received {} missions from AI Agent.",
        decomp_res.missions.len()
    );

    // 4. Save missions to Postgres
    for mission in &decomp_res.missions {
        info!("Saving mission: '{}' ({})", mission.title, mission.priority);
        sqlx::query("INSERT INTO missions (goal_id, title, description, priority, status) VALUES ($1, $2, $3, $4, $5)")
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

    info!("Successfully processed intent goal: {}", goal_id);
    Ok(())
}

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
