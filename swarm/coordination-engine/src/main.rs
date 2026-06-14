use aurelium_sdk::AureliumClient;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::env;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct MissionInfo {
    title: String,
    description: String,
    priority: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct MissionGeneratedEvent {
    goal_id: String,
    missions: Vec<MissionInfo>,
}

#[derive(Serialize, Deserialize, Debug)]
struct TaskAssignRequest {
    task_id: String,
    mission_id: String,
    title: String,
    description: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct TaskResolvedEvent {
    task_id: String,
    status: String,
    output: String,
    security_approval: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("info"))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting AURELIUM Swarm Coordination Engine...");

    let nats_url = env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());
    let db_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://aurelium:aurelium@localhost:5432/aurelium".to_string());

    info!("Connecting to NATS at {} and PostgreSQL...", nats_url);
    let client = AureliumClient::new(&nats_url, &db_url).await?;
    info!("Successfully connected to system dependencies.");

    // Spawn a task to listen to task resolutions
    let client_resolution = AureliumClient::new(&nats_url, &db_url).await?;
    tokio::spawn(async move {
        if let Err(e) = listen_for_resolutions(client_resolution).await {
            error!("Error in task resolution listener: {:?}", e);
        }
    });

    // Subscribe to mission.generated
    let mut subscription = client
        .nats()
        .subscribe("mission.generated".to_string())
        .await?;
    info!("Listening on NATS subject 'mission.generated'");

    while let Some(message) = subscription.next().await {
        let payload = match serde_json::from_slice::<MissionGeneratedEvent>(&message.payload) {
            Ok(event) => event,
            Err(e) => {
                error!("Failed to deserialize mission generated event: {:?}", e);
                continue;
            }
        };

        info!(
            "Received mission.generated event for goal_id: {}",
            payload.goal_id
        );

        let client_clone = AureliumClient::new(&nats_url, &db_url).await?;
        tokio::spawn(async move {
            if let Err(e) = process_missions(client_clone, payload).await {
                error!("Error processing missions: {:?}", e);
            }
        });
    }

    Ok(())
}

async fn process_missions(
    client: AureliumClient,
    event: MissionGeneratedEvent,
) -> Result<(), Box<dyn std::error::Error>> {
    let goal_uuid = Uuid::parse_str(&event.goal_id)?;

    // Fetch missions from DB to get their persistent UUIDs
    let rows = sqlx::query("SELECT id, title, description FROM missions WHERE goal_id = $1")
        .bind(goal_uuid)
        .fetch_all(client.db())
        .await?;

    info!(
        "Retrieved {} missions from database for goal {}.",
        rows.len(),
        goal_uuid
    );

    for row in rows {
        let mission_id: Uuid = sqlx::Row::get(&row, "id");
        let title: String = sqlx::Row::get(&row, "title");
        let description: String = sqlx::Row::get(&row, "description");

        // Determine agent mapping based on keywords
        let title_lower = title.to_lowercase();
        let desc_lower = description.to_lowercase();

        let agent_type = if title_lower.contains("security")
            || desc_lower.contains("security")
            || title_lower.contains("audit")
        {
            "security"
        } else if title_lower.contains("docker")
            || desc_lower.contains("docker")
            || title_lower.contains("k8s")
            || title_lower.contains("devops")
            || title_lower.contains("ci")
        {
            "devops"
        } else if title_lower.contains("ui")
            || desc_lower.contains("frontend")
            || title_lower.contains("dashboard")
            || title_lower.contains("console")
        {
            "frontend"
        } else {
            "backend"
        };

        let task_id = Uuid::new_v4();
        info!(
            "Mapping mission '{}' to {} agent. (Task ID: {})",
            title, agent_type, task_id
        );

        // 1. Persist task state to Postgres
        sqlx::query("INSERT INTO tasks (id, mission_id, agent_type, title, description, status) VALUES ($1, $2, $3, $4, $5, $6)")
            .bind(task_id)
            .bind(mission_id)
            .bind(agent_type)
            .bind(&title)
            .bind(&description)
            .bind("pending")
            .execute(client.db())
            .await?;

        // 2. Publish task assignment to NATS
        let assign_req = TaskAssignRequest {
            task_id: task_id.to_string(),
            mission_id: mission_id.to_string(),
            title: title.clone(),
            description: description.clone(),
        };
        let payload = serde_json::to_vec(&assign_req)?;
        let subject = format!("agent.{}.assign", agent_type);

        client.nats().publish(subject, payload.into()).await?;
        info!("Task {} published on event bus.", task_id);
    }

    Ok(())
}

async fn listen_for_resolutions(client: AureliumClient) -> Result<(), Box<dyn std::error::Error>> {
    let mut subscription = client
        .nats()
        .subscribe("agent.task.resolved".to_string())
        .await?;
    info!("Listening on NATS subject 'agent.task.resolved' for agent responses.");

    while let Some(message) = subscription.next().await {
        let payload = match serde_json::from_slice::<TaskResolvedEvent>(&message.payload) {
            Ok(event) => event,
            Err(e) => {
                error!("Failed to deserialize task resolved event: {:?}", e);
                continue;
            }
        };

        let task_uuid = match Uuid::parse_str(&payload.task_id) {
            Ok(uuid) => uuid,
            Err(_e) => {
                error!("Invalid task UUID: {}", payload.task_id);
                continue;
            }
        };

        info!(
            "Agent reported resolution for Task: {} with status: {}",
            task_uuid, payload.status
        );

        // Update task state in database
        let result = sqlx::query("UPDATE tasks SET status = $1, output = $2, security_approval = $3, updated_at = CURRENT_TIMESTAMP WHERE id = $4")
            .bind(&payload.status)
            .bind(&payload.output)
            .bind(payload.security_approval)
            .bind(task_uuid)
            .execute(client.db())
            .await;

        match result {
            Ok(_) => info!("Task {} successfully updated in database.", task_uuid),
            Err(e) => error!("Failed to update Task {} in database: {:?}", task_uuid, e),
        }
    }

    Ok(())
}
