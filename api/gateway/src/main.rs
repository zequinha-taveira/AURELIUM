use axum::{
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    database: String,
    nats: String,
}

#[derive(Deserialize)]
struct IntentRequest {
    goal: String,
}

#[derive(Serialize)]
struct IntentResponse {
    status: String,
    mission_id: String,
}

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("info"))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting AURELIUM API Gateway...");

    // Build routes
    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/gateway/intent", post(intent_handler));

    // Listen on 0.0.0.0:3000
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    info!("Gateway listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        database: "disconnected".to_string(), // In actual code we would check the db pool from SDK
        nats: "disconnected".to_string(),     // In actual code we would check the nats client
    })
}

async fn intent_handler(Json(payload): Json<IntentRequest>) -> Json<IntentResponse> {
    info!("Received intent goal: {}", payload.goal);
    let mock_id = format!(
        "mission_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );
    Json(IntentResponse {
        status: "queued".to_string(),
        mission_id: mock_id,
    })
}
