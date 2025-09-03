use askama::Template;
use axum::{
    extract::{Path, State},
    response::Html,
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
    let messages_data = state.queue_service.get_queue_messages(queue_name).await?;
    
    let mut messages = Vec::new();
    for (id, body, created_at, visibility_timeout, receive_count, attributes, deduplication_id) in messages_data {
        messages.push(MessageInfo {
            id,
            body,
            created_at,
            visibility_timeout: visibility_timeout.unwrap_or_else(|| "None".to_string()),
            receive_count,
            attributes: attributes.unwrap_or_else(|| "None".to_string()),
            deduplication_id: deduplication_id.unwrap_or_else(|| "None".to_string()),
        });
    }
    
    Ok(messages)
}

