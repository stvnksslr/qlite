use askama::Template;
use axum::{
    extract::{Form, Path, State},
    http::StatusCode,
    response::{Html, Json, Redirect},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::http_server::AppState;

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate {
    pub queues: Vec<QueueInfo>,
    pub messages: Vec<MessageInfo>,
    pub total_queues: usize,
    pub total_messages: usize,
    pub total_available_messages: usize,
    pub total_in_flight_messages: usize,
}

#[derive(Template)]
#[template(path = "messages.html")]
pub struct MessagesTemplate {
    pub messages: Vec<MessageInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueInfo {
    pub name: String,
    pub created_at: String,
    pub available_messages: u32,
    pub in_flight_messages: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageInfo {
    pub id: String,
    pub body: String,
    pub created_at: String,
    pub visibility_timeout: String,
    pub receive_count: u32,
    pub attributes: String,
    pub deduplication_id: String,
    pub status: String,
    pub processed_at: String,
    pub deleted_at: String,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse {
    pub success: bool,
    pub message: String,
}

pub async fn dashboard(State(state): State<Arc<AppState>>) -> Result<Html<String>, String> {
    // Get all queues
    let queues_data = state
        .queue_service
        .list_queues()
        .await
        .map_err(|e| format!("Failed to list queues: {}", e))?;

    let mut queues = Vec::new();
    let mut total_available = 0u32;
    let mut total_in_flight = 0u32;

    // Get queue attributes for each queue
    for (queue_name, created_at) in queues_data {
        if let Ok(Some(attrs)) = state.queue_service.get_queue_attributes(&queue_name).await {
            total_available += attrs.approximate_number_of_messages;
            total_in_flight += attrs.approximate_number_of_messages_not_visible;

            queues.push(QueueInfo {
                name: queue_name,
                created_at,
                available_messages: attrs.approximate_number_of_messages,
                in_flight_messages: attrs.approximate_number_of_messages_not_visible,
            });
        }
    }

    let template = DashboardTemplate {
        total_queues: queues.len(),
        total_messages: (total_available + total_in_flight) as usize,
        total_available_messages: total_available as usize,
        total_in_flight_messages: total_in_flight as usize,
        queues,
        messages: vec![], // Empty by default, populated when a queue is selected
    };

    let html = template
        .render()
        .map_err(|e| format!("Template render error: {}", e))?;

    Ok(Html(html))
}

pub async fn queue_messages(
    Path(queue_name): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, String> {
    // Get messages for the specific queue by reading directly from database
    // Since we don't have a direct method, we'll simulate it by getting queue info
    // and then fetching some messages (this is a simplified approach)

    let messages = get_queue_messages(&state, &queue_name)
        .await
        .map_err(|e| format!("Failed to get messages: {}", e))?;

    let template = MessagesTemplate { messages };

    let html = template
        .render()
        .map_err(|e| format!("Template render error: {}", e))?;

    Ok(Html(html))
}

async fn get_queue_messages(
    state: &Arc<AppState>,
    queue_name: &str,
) -> Result<Vec<MessageInfo>, Box<dyn std::error::Error>> {
    let messages_data = state
        .queue_service
        .get_all_queue_messages(queue_name)
        .await?;

    let mut messages = Vec::new();
    for (
        id,
        body,
        created_at,
        visibility_timeout,
        receive_count,
        attributes,
        deduplication_id,
        status,
        processed_at,
        deleted_at,
    ) in messages_data
    {
        messages.push(MessageInfo {
            id,
            body,
            created_at,
            visibility_timeout: visibility_timeout.unwrap_or_else(|| "None".to_string()),
            receive_count,
            attributes: attributes.unwrap_or_else(|| "None".to_string()),
            deduplication_id: deduplication_id.unwrap_or_else(|| "None".to_string()),
            status,
            processed_at: processed_at.unwrap_or_else(|| "Never".to_string()),
            deleted_at: deleted_at.unwrap_or_else(|| "Never".to_string()),
        });
    }

    Ok(messages)
}

// Form structures for UI operations
#[derive(Debug, Deserialize)]
pub struct CreateQueueForm {
    pub queue_name: String,
    pub queue_type: Option<String>,
    pub visibility_timeout_seconds: Option<u32>,
    pub message_retention_period_seconds: Option<u32>,
    pub max_receive_count: Option<u32>,
    pub delay_seconds: Option<u32>,
    pub receive_message_wait_time_seconds: Option<u32>,
    pub dead_letter_target_queue: Option<String>,
    pub content_based_deduplication: Option<String>,
}

// UI handler functions for queue and message management
pub async fn create_queue_ui(
    State(state): State<Arc<AppState>>,
    Form(form): Form<CreateQueueForm>,
) -> Result<Redirect, String> {
    use crate::config::QueueConfig;

    if form.queue_name.trim().is_empty() {
        return Err("Queue name cannot be empty".to_string());
    }

    // Determine if it's a FIFO queue based only on explicit type selection
    let is_fifo = form
        .queue_type
        .as_ref()
        .map(|t| t == "fifo")
        .unwrap_or(false);

    // For simple queue creation (no advanced options), use the basic method
    if form.visibility_timeout_seconds.is_none()
        && form.message_retention_period_seconds.is_none()
        && form.max_receive_count.is_none()
        && form.delay_seconds.is_none()
        && form.receive_message_wait_time_seconds.is_none()
        && form
            .dead_letter_target_queue
            .as_ref()
            .is_none_or(|s| s.trim().is_empty())
        && !is_fifo
    {
        match state.queue_service.create_queue(&form.queue_name).await {
            Ok(_) => return Ok(Redirect::to("/ui")),
            Err(e) => return Err(format!("Failed to create queue: {}", e)),
        }
    }

    // For advanced options or FIFO queues, use the config method
    let mut config = QueueConfig {
        name: form.queue_name.clone(),
        is_fifo,
        ..Default::default()
    };

    // Apply custom configuration values
    if let Some(timeout) = form.visibility_timeout_seconds {
        config.visibility_timeout_seconds = timeout;
    }

    if let Some(retention) = form.message_retention_period_seconds {
        config.message_retention_period_seconds = retention;
    }

    if let Some(max_receive) = form.max_receive_count {
        config.max_receive_count = Some(max_receive);
    }

    if let Some(delay) = form.delay_seconds {
        config.delay_seconds = delay;
    }

    if let Some(wait_time) = form.receive_message_wait_time_seconds {
        config.receive_message_wait_time_seconds = wait_time;
    }

    if let Some(dlq_queue) = form
        .dead_letter_target_queue
        .filter(|s| !s.trim().is_empty())
    {
        // Convert queue name to a simple ARN-like format for internal use
        config.dead_letter_target_arn = Some(format!("qlite://queue/{}", dlq_queue));
    }

    // FIFO-specific options
    if is_fifo {
        config.content_based_deduplication = form
            .content_based_deduplication
            .map(|v| v == "on")
            .unwrap_or(true); // Default to true for FIFO queues
    }

    match state.queue_service.create_queue_with_config(&config).await {
        Ok(_) => Ok(Redirect::to("/ui")),
        Err(e) => Err(format!("Failed to create queue: {}", e)),
    }
}

pub async fn delete_queue_ui(
    State(state): State<Arc<AppState>>,
    Path(queue_name): Path<String>,
) -> Result<Redirect, String> {
    match state.queue_service.delete_queue(&queue_name).await {
        Ok(_) => Ok(Redirect::to("/ui")),
        Err(e) => Err(format!("Failed to delete queue: {}", e)),
    }
}

pub async fn delete_message_ui(
    State(state): State<Arc<AppState>>,
    Path(message_id): Path<String>,
) -> Result<Redirect, String> {
    match state.queue_service.delete_message(&message_id).await {
        Ok(_) => Ok(Redirect::to("/ui")),
        Err(e) => Err(format!("Failed to delete message: {}", e)),
    }
}

pub async fn restore_message_ui(
    State(state): State<Arc<AppState>>,
    Path(message_id): Path<String>,
) -> Result<Redirect, String> {
    match state.queue_service.restore_message(&message_id).await {
        Ok(_) => Ok(Redirect::to("/ui")),
        Err(e) => Err(format!("Failed to restore message: {}", e)),
    }
}

// JSON API endpoints for AJAX calls that preserve UI state
pub async fn delete_queue_json(
    State(state): State<Arc<AppState>>,
    Path(queue_name): Path<String>,
) -> Result<Json<ApiResponse>, (StatusCode, Json<ApiResponse>)> {
    match state.queue_service.delete_queue(&queue_name).await {
        Ok(_) => Ok(Json(ApiResponse {
            success: true,
            message: format!("Queue '{}' deleted successfully", queue_name),
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse {
                success: false,
                message: format!("Failed to delete queue: {}", e),
            }),
        )),
    }
}

pub async fn delete_message_json(
    State(state): State<Arc<AppState>>,
    Path(message_id): Path<String>,
) -> Result<Json<ApiResponse>, (StatusCode, Json<ApiResponse>)> {
    match state.queue_service.delete_message(&message_id).await {
        Ok(_) => Ok(Json(ApiResponse {
            success: true,
            message: "Message deleted successfully".to_string(),
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse {
                success: false,
                message: format!("Failed to delete message: {}", e),
            }),
        )),
    }
}

pub async fn restore_message_json(
    State(state): State<Arc<AppState>>,
    Path(message_id): Path<String>,
) -> Result<Json<ApiResponse>, (StatusCode, Json<ApiResponse>)> {
    match state.queue_service.restore_message(&message_id).await {
        Ok(_) => Ok(Json(ApiResponse {
            success: true,
            message: "Message restored successfully".to_string(),
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse {
                success: false,
                message: format!("Failed to restore message: {}", e),
            }),
        )),
    }
}
