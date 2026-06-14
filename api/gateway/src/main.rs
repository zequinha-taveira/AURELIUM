use aurelium_sdk::AureliumClient;
use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::env;
use std::net::SocketAddr;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone)]
struct AppState {
    client: std::sync::Arc<AureliumClient>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    database: String,
    nats: String,
}

#[derive(Deserialize, Serialize)]
struct IntentRequest {
    goal: String,
}

#[derive(Serialize)]
struct IntentResponse {
    status: String,
    message: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("info"))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting AURELIUM API Gateway...");

    // Connection parameters from env or default
    let nats_url = env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());
    let db_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://aurelium:aurelium@localhost:5432/aurelium".to_string());

    info!("Connecting to AURELIUM infrastructure...");
    let client = AureliumClient::new(&nats_url, &db_url).await?;
    let state = AppState {
        client: std::sync::Arc::new(client),
    };
    info!("Successfully connected to NATS and PostgreSQL.");

    // Build routes
    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/gateway/intent", post(intent_handler))
        .with_state(state);

    // Listen on 0.0.0.0:3000
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("Gateway listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}

async fn health_handler(State(state): State<AppState>) -> Json<HealthResponse> {
    // Basic verification of connections
    let db_status = if state.client.db().acquire().await.is_ok() {
        "connected".to_string()
    } else {
        "disconnected".to_string()
    };

    let nats_status =
        if state.client.nats().connection_state() == async_nats::connection::State::Connected {
            "connected".to_string()
        } else {
            "disconnected".to_string()
        };

    Json(HealthResponse {
        status: "healthy".to_string(),
        database: db_status,
        nats: nats_status,
    })
}

async fn intent_handler(
    State(state): State<AppState>,
    Json(payload): Json<IntentRequest>,
) -> Result<Json<IntentResponse>, StatusCode> {
    info!("Received intent goal: '{}'", payload.goal);

    let payload_bytes = serde_json::to_vec(&payload).map_err(|e| {
        error!("Failed to serialize intent request: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Publish to NATS intent.received topic
    state
        .client
        .nats()
        .publish("intent.received".to_string(), payload_bytes.into())
        .await
        .map_err(|e| {
            error!("Failed to publish intent to event bus: {:?}", e);
            StatusCode::SERVICE_UNAVAILABLE
        })?;

    info!("Successfully published intent to event bus.");

    Ok(Json(IntentResponse {
        status: "accepted".to_string(),
        message: "Goal successfully submitted to Intent Operating System.".to_string(),
    }))
}
