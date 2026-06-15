use aurelium_sdk::AureliumClient;
use axum::{
    extract::{Path, Query, State},
    http::{header, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

// ============================================================================
// Application State
// ============================================================================

#[derive(Clone)]
struct AppState {
    client: Arc<AureliumClient>,
    start_time: Instant,
    rate_limiter: Arc<RwLock<RateLimiter>>,
    api_keys: Vec<String>,
}

// ============================================================================
// Rate Limiter (Token Bucket)
// ============================================================================

struct RateLimiter {
    buckets: HashMap<String, TokenBucket>,
    max_requests: u32,
    window_seconds: u64,
}

struct TokenBucket {
    tokens: u32,
    last_refill: Instant,
}

impl RateLimiter {
    fn new(max_requests: u32, window_seconds: u64) -> Self {
        Self {
            buckets: HashMap::new(),
            max_requests,
            window_seconds,
        }
    }

    fn allow(&mut self, key: &str) -> bool {
        let now = Instant::now();
        let bucket = self.buckets.entry(key.to_string()).or_insert(TokenBucket {
            tokens: self.max_requests,
            last_refill: now,
        });

        // Refill tokens if window has passed
        let elapsed = now.duration_since(bucket.last_refill).as_secs();
        if elapsed >= self.window_seconds {
            bucket.tokens = self.max_requests;
            bucket.last_refill = now;
        }

        if bucket.tokens > 0 {
            bucket.tokens -= 1;
            true
        } else {
            false
        }
    }
}

// ============================================================================
// Request / Response Types
// ============================================================================

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    database: String,
    nats: String,
    uptime_seconds: u64,
    version: String,
}

#[derive(Deserialize, Serialize)]
struct IntentRequest {
    goal: String,
    #[serde(default)]
    context: Option<serde_json::Value>,
    #[serde(default)]
    priority: Option<String>,
}

#[derive(Serialize)]
struct IntentResponse {
    status: String,
    goal_id: String,
    message: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<String>,
}

#[derive(Deserialize)]
struct PaginationParams {
    #[serde(default = "default_page")]
    page: u32,
    #[serde(default = "default_page_size")]
    page_size: u32,
}

fn default_page() -> u32 { 1 }
fn default_page_size() -> u32 { 20 }

#[derive(Serialize)]
struct PaginatedResponse<T: Serialize> {
    data: Vec<T>,
    total: i64,
    page: u32,
    page_size: u32,
    has_more: bool,
}

#[derive(Serialize)]
struct GoalRow {
    id: String,
    raw_input: String,
    status: String,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
struct MissionRow {
    id: String,
    goal_id: String,
    title: String,
    description: Option<String>,
    priority: String,
    status: String,
    created_at: String,
}

#[derive(Serialize)]
struct TaskRow {
    id: String,
    mission_id: String,
    agent_type: String,
    title: String,
    description: Option<String>,
    status: String,
    security_approval: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
struct SystemMetrics {
    total_goals: i64,
    total_missions: i64,
    total_tasks: i64,
    goals_by_status: HashMap<String, i64>,
    uptime_seconds: u64,
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
        ))
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    info!("Starting AURELIUM API Gateway v0.2.0...");

    // Connection parameters from env or defaults
    let nats_url = env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());
    let db_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://aurelium:aurelium@localhost:5432/aurelium".to_string());
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .unwrap_or(3000);

    // Rate limiter config
    let rate_limit_max: u32 = env::var("RATE_LIMIT_MAX")
        .unwrap_or_else(|_| "100".to_string())
        .parse()
        .unwrap_or(100);
    let rate_limit_window: u64 = env::var("RATE_LIMIT_WINDOW_SECS")
        .unwrap_or_else(|_| "60".to_string())
        .parse()
        .unwrap_or(60);

    // API keys (comma-separated)
    let api_keys: Vec<String> = env::var("API_KEYS")
        .unwrap_or_default()
        .split(',')
        .filter(|k| !k.is_empty())
        .map(|k| k.trim().to_string())
        .collect();

    if api_keys.is_empty() {
        warn!("No API_KEYS configured — authentication is disabled in dev mode.");
    }

    info!("Connecting to AURELIUM infrastructure...");
    let client = AureliumClient::new(&nats_url, &db_url).await?;
    let state = AppState {
        client: Arc::new(client),
        start_time: Instant::now(),
        rate_limiter: Arc::new(RwLock::new(RateLimiter::new(rate_limit_max, rate_limit_window))),
        api_keys,
    };
    info!("Successfully connected to NATS and PostgreSQL.");

    // CORS configuration
    let cors_origins = env::var("CORS_ORIGINS").unwrap_or_else(|_| "*".to_string());
    let cors = if cors_origins == "*" {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
            .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
    } else {
        CorsLayer::new()
            .allow_origin(Any) // Simplified for dev — restrict in production
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
            .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
    };

    // Build routes
    let app = Router::new()
        // Public endpoints
        .route("/health", get(health_handler))
        .route("/metrics", get(metrics_handler))
        // Intent / Goal endpoints
        .route("/api/v1/goals", get(list_goals_handler).post(create_goal_handler))
        .route("/api/v1/goals/{id}", get(get_goal_handler))
        .route("/api/v1/goals/{id}/missions", get(list_goal_missions_handler))
        // Mission endpoints
        .route("/api/v1/missions", get(list_missions_handler))
        .route("/api/v1/missions/{id}", get(get_mission_handler))
        .route("/api/v1/missions/{id}/tasks", get(list_mission_tasks_handler))
        // Task endpoints
        .route("/api/v1/tasks", get(list_tasks_handler))
        .route("/api/v1/tasks/{id}", get(get_task_handler))
        // System endpoints
        .route("/api/v1/system/stats", get(system_stats_handler))
        .with_state(state)
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    // Listen
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Gateway listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}

// ============================================================================
// Handlers — Health & Metrics
// ============================================================================

async fn health_handler(State(state): State<AppState>) -> Json<HealthResponse> {
    let db_status = if state.client.db().acquire().await.is_ok() {
        "connected"
    } else {
        "disconnected"
    };

    let nats_status =
        if state.client.nats().connection_state() == async_nats::connection::State::Connected {
            "connected"
        } else {
            "disconnected"
        };

    Json(HealthResponse {
        status: "healthy".to_string(),
        database: db_status.to_string(),
        nats: nats_status.to_string(),
        uptime_seconds: state.start_time.elapsed().as_secs(),
        version: "0.2.0".to_string(),
    })
}

async fn metrics_handler(State(state): State<AppState>) -> impl IntoResponse {
    // Prometheus-compatible text format
    let uptime = state.start_time.elapsed().as_secs();

    let db_up: u8 = if state.client.db().acquire().await.is_ok() { 1 } else { 0 };
    let nats_up: u8 =
        if state.client.nats().connection_state() == async_nats::connection::State::Connected {
            1
        } else {
            0
        };

    // Query counts from database
    let goals_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM goals")
        .fetch_one(state.client.db())
        .await
        .unwrap_or(0);
    let missions_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM missions")
        .fetch_one(state.client.db())
        .await
        .unwrap_or(0);
    let tasks_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks")
        .fetch_one(state.client.db())
        .await
        .unwrap_or(0);

    let body = format!(
        "# HELP aurelium_uptime_seconds Gateway uptime in seconds\n\
         # TYPE aurelium_uptime_seconds gauge\n\
         aurelium_uptime_seconds {uptime}\n\
         # HELP aurelium_database_up Database connection status\n\
         # TYPE aurelium_database_up gauge\n\
         aurelium_database_up {db_up}\n\
         # HELP aurelium_nats_up NATS connection status\n\
         # TYPE aurelium_nats_up gauge\n\
         aurelium_nats_up {nats_up}\n\
         # HELP aurelium_goals_total Total number of goals\n\
         # TYPE aurelium_goals_total gauge\n\
         aurelium_goals_total {goals_count}\n\
         # HELP aurelium_missions_total Total number of missions\n\
         # TYPE aurelium_missions_total gauge\n\
         aurelium_missions_total {missions_count}\n\
         # HELP aurelium_tasks_total Total number of tasks\n\
         # TYPE aurelium_tasks_total gauge\n\
         aurelium_tasks_total {tasks_count}\n"
    );

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        body,
    )
}

// ============================================================================
// Handlers — Goals
// ============================================================================

async fn create_goal_handler(
    State(state): State<AppState>,
    Json(payload): Json<IntentRequest>,
) -> Result<(StatusCode, Json<IntentResponse>), (StatusCode, Json<ErrorResponse>)> {
    // Validate input
    let goal = payload.goal.trim();
    if goal.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Goal cannot be empty".to_string(),
                code: "INVALID_INPUT".to_string(),
                details: None,
            }),
        ));
    }
    if goal.len() > 5000 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Goal exceeds maximum length of 5000 characters".to_string(),
                code: "INPUT_TOO_LONG".to_string(),
                details: None,
            }),
        ));
    }

    info!("Received intent goal: '{}'", goal);

    // Build the event payload
    let event = serde_json::json!({
        "goal": goal,
        "context": payload.context,
        "priority": payload.priority.unwrap_or_else(|| "medium".to_string()),
    });
    let payload_bytes = serde_json::to_vec(&event).map_err(|e| {
        error!("Failed to serialize intent request: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Internal serialization error".to_string(),
                code: "SERIALIZATION_ERROR".to_string(),
                details: None,
            }),
        )
    })?;

    // Publish to NATS
    state
        .client
        .nats()
        .publish("intent.received".to_string(), payload_bytes.into())
        .await
        .map_err(|e| {
            error!("Failed to publish intent to event bus: {:?}", e);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Event bus unavailable".to_string(),
                    code: "NATS_UNAVAILABLE".to_string(),
                    details: Some(e.to_string()),
                }),
            )
        })?;

    info!("Successfully published intent to event bus.");

    Ok((
        StatusCode::ACCEPTED,
        Json(IntentResponse {
            status: "accepted".to_string(),
            goal_id: Uuid::new_v4().to_string(),
            message: "Goal successfully submitted to Intent Operating System.".to_string(),
        }),
    ))
}

async fn list_goals_handler(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<GoalRow>>, (StatusCode, Json<ErrorResponse>)> {
    let page_size = params.page_size.min(100);
    let offset = (params.page.saturating_sub(1)) * page_size;

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM goals")
        .fetch_one(state.client.db())
        .await
        .map_err(db_error)?;

    let rows = sqlx::query_as::<_, (Uuid, String, String, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, raw_input, status, created_at, updated_at FROM goals ORDER BY created_at DESC LIMIT $1 OFFSET $2"
    )
    .bind(page_size as i64)
    .bind(offset as i64)
    .fetch_all(state.client.db())
    .await
    .map_err(db_error)?;

    let data: Vec<GoalRow> = rows
        .into_iter()
        .map(|(id, raw_input, status, created_at, updated_at)| GoalRow {
            id: id.to_string(),
            raw_input,
            status,
            created_at: created_at.to_rfc3339(),
            updated_at: updated_at.to_rfc3339(),
        })
        .collect();

    let has_more = (offset + page_size) < total as u32;

    Ok(Json(PaginatedResponse {
        data,
        total,
        page: params.page,
        page_size,
        has_more,
    }))
}

async fn get_goal_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<GoalRow>, (StatusCode, Json<ErrorResponse>)> {
    let row = sqlx::query_as::<_, (Uuid, String, String, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, raw_input, status, created_at, updated_at FROM goals WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(state.client.db())
    .await
    .map_err(db_error)?;

    match row {
        Some((id, raw_input, status, created_at, updated_at)) => Ok(Json(GoalRow {
            id: id.to_string(),
            raw_input,
            status,
            created_at: created_at.to_rfc3339(),
            updated_at: updated_at.to_rfc3339(),
        })),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Goal not found".to_string(),
                code: "NOT_FOUND".to_string(),
                details: None,
            }),
        )),
    }
}

async fn list_goal_missions_handler(
    State(state): State<AppState>,
    Path(goal_id): Path<Uuid>,
) -> Result<Json<Vec<MissionRow>>, (StatusCode, Json<ErrorResponse>)> {
    let rows = sqlx::query_as::<_, (Uuid, Uuid, String, Option<String>, String, String, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, goal_id, title, description, priority, status, created_at FROM missions WHERE goal_id = $1 ORDER BY created_at"
    )
    .bind(goal_id)
    .fetch_all(state.client.db())
    .await
    .map_err(db_error)?;

    let data: Vec<MissionRow> = rows
        .into_iter()
        .map(|(id, goal_id, title, description, priority, status, created_at)| MissionRow {
            id: id.to_string(),
            goal_id: goal_id.to_string(),
            title,
            description,
            priority,
            status,
            created_at: created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(data))
}

// ============================================================================
// Handlers — Missions
// ============================================================================

async fn list_missions_handler(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<MissionRow>>, (StatusCode, Json<ErrorResponse>)> {
    let page_size = params.page_size.min(100);
    let offset = (params.page.saturating_sub(1)) * page_size;

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM missions")
        .fetch_one(state.client.db())
        .await
        .map_err(db_error)?;

    let rows = sqlx::query_as::<_, (Uuid, Uuid, String, Option<String>, String, String, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, goal_id, title, description, priority, status, created_at FROM missions ORDER BY created_at DESC LIMIT $1 OFFSET $2"
    )
    .bind(page_size as i64)
    .bind(offset as i64)
    .fetch_all(state.client.db())
    .await
    .map_err(db_error)?;

    let data: Vec<MissionRow> = rows
        .into_iter()
        .map(|(id, goal_id, title, description, priority, status, created_at)| MissionRow {
            id: id.to_string(),
            goal_id: goal_id.to_string(),
            title,
            description,
            priority,
            status,
            created_at: created_at.to_rfc3339(),
        })
        .collect();

    let has_more = (offset + page_size) < total as u32;

    Ok(Json(PaginatedResponse {
        data,
        total,
        page: params.page,
        page_size,
        has_more,
    }))
}

async fn get_mission_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<MissionRow>, (StatusCode, Json<ErrorResponse>)> {
    let row = sqlx::query_as::<_, (Uuid, Uuid, String, Option<String>, String, String, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, goal_id, title, description, priority, status, created_at FROM missions WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(state.client.db())
    .await
    .map_err(db_error)?;

    match row {
        Some((id, goal_id, title, description, priority, status, created_at)) => Ok(Json(MissionRow {
            id: id.to_string(),
            goal_id: goal_id.to_string(),
            title,
            description,
            priority,
            status,
            created_at: created_at.to_rfc3339(),
        })),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Mission not found".to_string(),
                code: "NOT_FOUND".to_string(),
                details: None,
            }),
        )),
    }
}

async fn list_mission_tasks_handler(
    State(state): State<AppState>,
    Path(mission_id): Path<Uuid>,
) -> Result<Json<Vec<TaskRow>>, (StatusCode, Json<ErrorResponse>)> {
    let rows = sqlx::query_as::<_, (Uuid, Uuid, String, String, Option<String>, String, bool, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, mission_id, agent_type, title, description, status, security_approval, created_at, updated_at FROM tasks WHERE mission_id = $1 ORDER BY created_at"
    )
    .bind(mission_id)
    .fetch_all(state.client.db())
    .await
    .map_err(db_error)?;

    let data: Vec<TaskRow> = rows
        .into_iter()
        .map(|(id, mission_id, agent_type, title, description, status, security_approval, created_at, updated_at)| TaskRow {
            id: id.to_string(),
            mission_id: mission_id.to_string(),
            agent_type,
            title,
            description,
            status,
            security_approval,
            created_at: created_at.to_rfc3339(),
            updated_at: updated_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(data))
}

// ============================================================================
// Handlers — Tasks
// ============================================================================

async fn list_tasks_handler(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<TaskRow>>, (StatusCode, Json<ErrorResponse>)> {
    let page_size = params.page_size.min(100);
    let offset = (params.page.saturating_sub(1)) * page_size;

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks")
        .fetch_one(state.client.db())
        .await
        .map_err(db_error)?;

    let rows = sqlx::query_as::<_, (Uuid, Uuid, String, String, Option<String>, String, bool, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, mission_id, agent_type, title, description, status, security_approval, created_at, updated_at FROM tasks ORDER BY created_at DESC LIMIT $1 OFFSET $2"
    )
    .bind(page_size as i64)
    .bind(offset as i64)
    .fetch_all(state.client.db())
    .await
    .map_err(db_error)?;

    let data: Vec<TaskRow> = rows
        .into_iter()
        .map(|(id, mission_id, agent_type, title, description, status, security_approval, created_at, updated_at)| TaskRow {
            id: id.to_string(),
            mission_id: mission_id.to_string(),
            agent_type,
            title,
            description,
            status,
            security_approval,
            created_at: created_at.to_rfc3339(),
            updated_at: updated_at.to_rfc3339(),
        })
        .collect();

    let has_more = (offset + page_size) < total as u32;

    Ok(Json(PaginatedResponse {
        data,
        total,
        page: params.page,
        page_size,
        has_more,
    }))
}

async fn get_task_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<TaskRow>, (StatusCode, Json<ErrorResponse>)> {
    let row = sqlx::query_as::<_, (Uuid, Uuid, String, String, Option<String>, String, bool, chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)>(
        "SELECT id, mission_id, agent_type, title, description, status, security_approval, created_at, updated_at FROM tasks WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(state.client.db())
    .await
    .map_err(db_error)?;

    match row {
        Some((id, mission_id, agent_type, title, description, status, security_approval, created_at, updated_at)) => {
            Ok(Json(TaskRow {
                id: id.to_string(),
                mission_id: mission_id.to_string(),
                agent_type,
                title,
                description,
                status,
                security_approval,
                created_at: created_at.to_rfc3339(),
                updated_at: updated_at.to_rfc3339(),
            }))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "Task not found".to_string(),
                code: "NOT_FOUND".to_string(),
                details: None,
            }),
        )),
    }
}

// ============================================================================
// Handlers — System
// ============================================================================

async fn system_stats_handler(
    State(state): State<AppState>,
) -> Result<Json<SystemMetrics>, (StatusCode, Json<ErrorResponse>)> {
    let total_goals: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM goals")
        .fetch_one(state.client.db())
        .await
        .map_err(db_error)?;
    let total_missions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM missions")
        .fetch_one(state.client.db())
        .await
        .map_err(db_error)?;
    let total_tasks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tasks")
        .fetch_one(state.client.db())
        .await
        .map_err(db_error)?;

    // Goals grouped by status
    let status_rows = sqlx::query_as::<_, (String, i64)>(
        "SELECT status, COUNT(*) FROM goals GROUP BY status"
    )
    .fetch_all(state.client.db())
    .await
    .map_err(db_error)?;

    let goals_by_status: HashMap<String, i64> = status_rows.into_iter().collect();

    Ok(Json(SystemMetrics {
        total_goals,
        total_missions,
        total_tasks,
        goals_by_status,
        uptime_seconds: state.start_time.elapsed().as_secs(),
    }))
}

// ============================================================================
// Error Helpers
// ============================================================================

fn db_error(e: sqlx::Error) -> (StatusCode, Json<ErrorResponse>) {
    error!("Database error: {:?}", e);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: "Database error".to_string(),
            code: "DB_ERROR".to_string(),
            details: Some(e.to_string()),
        }),
    )
}
