use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use quick_xml::se::to_string as to_xml;
use serde::Deserialize;
use std::{collections::HashMap, sync::Arc};
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::{
    message::{MessageAttributeValue},
    queue_service::QueueService,
    sqs_types::*,
    ui,
};

pub struct AppState {
    pub queue_service: Arc<QueueService>,
    pub base_url: String,
}

#[derive(Debug, Deserialize)]
struct ActionQuery {
    #[serde(rename = "Action")]
    action: String,
}

pub fn create_router(queue_service: Arc<QueueService>, base_url: String, enable_ui: bool) -> Router {
    let state = Arc::new(AppState {
        queue_service,
        base_url,
    });

    let mut router = Router::new()
        .route("/", post(handle_sqs_action))
        .route("/:queue_name", post(handle_queue_action))
        .route("/health", get(health_check))
        .route("/health/ready", get(readiness_check))
        .route("/health/live", get(liveness_check))
        .route("/metrics", get(metrics_endpoint));

    // Add UI routes if enabled
    if enable_ui {
        router = router
            .route("/ui", get(ui::dashboard))
            .route("/ui/queue/:queue_name", get(ui::queue_messages))
            .route("/ui/create-queue", post(ui::create_queue_ui))
            .route("/ui/delete-queue/:queue_name", post(ui::delete_queue_ui))
            .route("/ui/delete-message/:message_id", post(ui::delete_message_ui))
            .route("/ui/restore-message/:message_id", post(ui::restore_message_ui))
            // JSON API endpoints for AJAX calls
            .route("/api/ui/delete-queue/:queue_name", post(ui::delete_queue_json))
            .route("/api/ui/delete-message/:message_id", post(ui::delete_message_json))
            .route("/api/ui/restore-message/:message_id", post(ui::restore_message_json));
    }

    router
        .with_state(state)
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CorsLayer::permissive())
        )
}

async fn handle_sqs_action(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ActionQuery>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let _content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    match query.action.as_str() {
        "ListQueues" => handle_list_queues(state).await,
        "CreateQueue" => {
            if let Ok(params) = parse_form_params(&body) {
                if let Some(queue_name) = params.get("QueueName") {
                    handle_create_queue(state, queue_name).await
                } else {
                    error_response("MissingParameter", "QueueName parameter is required")
                }
            } else {
                error_response("InvalidParameterFormat", "Invalid request format")
            }
        },
        _ => error_response("InvalidAction", &format!("Unknown action: {}", query.action)),
    }
}

async fn handle_queue_action(
    State(state): State<Arc<AppState>>,
    Path(queue_name): Path<String>,
    Query(query): Query<ActionQuery>,
    body: String,
) -> Response {
    match query.action.as_str() {
        "SendMessage" => {
            if let Ok(params) = parse_form_params(&body) {
                handle_send_message(state, &queue_name, params).await
            } else {
                error_response("InvalidParameterFormat", "Invalid request format")
            }
        },
        "ReceiveMessage" => {
            let params = parse_form_params(&body).unwrap_or_default();
            handle_receive_message(state, &queue_name, params).await
        },
        "DeleteMessage" => {
            if let Ok(params) = parse_form_params(&body) {
                handle_delete_message(state, &queue_name, params).await
            } else {
                error_response("InvalidParameterFormat", "Invalid request format")
            }
        },
        "GetQueueAttributes" => {
            handle_get_queue_attributes(state, &queue_name).await
        },
        _ => error_response("InvalidAction", &format!("Unknown action: {}", query.action)),
    }
}

async fn handle_list_queues(state: Arc<AppState>) -> Response {
    match state.queue_service.list_queues().await {
        Ok(queues) => {
            let queue_urls: Vec<String> = queues
                .into_iter()
                .map(|(name, _)| format!("{}/{}", state.base_url, name))
                .collect();

            let response = ListQueuesResponse {
                list_queues_result: ListQueuesResult { queue_urls },
            };

            xml_response(response)
        },
        Err(_) => error_response("InternalError", "Failed to list queues"),
    }
}

async fn handle_create_queue(state: Arc<AppState>, queue_name: &str) -> Response {
    match state.queue_service.create_queue(queue_name).await {
        Ok(()) => {
            let response = CreateQueueResponse {
                create_queue_result: CreateQueueResult {
                    queue_url: format!("{}/{}", state.base_url, queue_name),
                },
            };
            xml_response(response)
        },
        Err(_) => error_response("InternalError", "Failed to create queue"),
    }
}

async fn handle_send_message(
    state: Arc<AppState>,
    queue_name: &str,
    params: HashMap<String, String>,
) -> Response {
    let message_body = match params.get("MessageBody") {
        Some(body) => body,
        None => return error_response("MissingParameter", "MessageBody parameter is required"),
    };

    let message_attributes = parse_message_attributes(&params);
    let deduplication_id = params.get("MessageDeduplicationId").cloned();

    match state.queue_service.send_message(
        queue_name,
        message_body,
        message_attributes,
        deduplication_id,
    ).await {
        Ok(message_id) => {
            let response = SendMessageResponse {
                send_message_result: SendMessageResult {
                    message_id,
                    md5_of_body: format!("{:x}", md5::compute(message_body)),
                },
            };
            xml_response(response)
        },
        Err(_) => error_response("InternalError", "Failed to send message"),
    }
}

async fn handle_receive_message(
    state: Arc<AppState>,
    queue_name: &str,
    _params: HashMap<String, String>,
) -> Response {
    match state.queue_service.receive_message(queue_name).await {
        Ok(Some(received_msg)) => {
            let mut message_attributes = HashMap::new();
            if let Some(attrs) = received_msg.attributes {
                for (key, value) in attrs {
                    message_attributes.insert(key, MessageAttribute {
                        string_value: value.string_value,
                        binary_value: value.binary_value,
                        data_type: value.data_type,
                    });
                }
            }

            let sqs_message = SqsMessage {
                message_id: received_msg.id,
                receipt_handle: received_msg.receipt_handle,
                body: received_msg.body,
                attributes: HashMap::new(),
                message_attributes,
            };

            let response = ReceiveMessageResponse {
                receive_message_result: ReceiveMessageResult {
                    messages: vec![sqs_message],
                },
            };

            xml_response(response)
        },
        Ok(None) => {
            let response = ReceiveMessageResponse {
                receive_message_result: ReceiveMessageResult {
                    messages: vec![],
                },
            };
            xml_response(response)
        },
        Err(_) => error_response("InternalError", "Failed to receive message"),
    }
}

async fn handle_delete_message(
    state: Arc<AppState>,
    _queue_name: &str,
    params: HashMap<String, String>,
) -> Response {
    let receipt_handle = match params.get("ReceiptHandle") {
        Some(handle) => handle,
        None => return error_response("MissingParameter", "ReceiptHandle parameter is required"),
    };

    match state.queue_service.delete_message(receipt_handle).await {
        Ok(_) => {
            let response = DeleteMessageResponse {
                delete_message_result: DeleteMessageResult {},
            };
            xml_response(response)
        },
        Err(_) => error_response("InternalError", "Failed to delete message"),
    }
}

async fn handle_get_queue_attributes(state: Arc<AppState>, queue_name: &str) -> Response {
    match state.queue_service.get_queue_attributes(queue_name).await {
        Ok(Some(attrs)) => {
            let attributes = vec![
                QueueAttribute {
                    name: "ApproximateNumberOfMessages".to_string(),
                    value: attrs.approximate_number_of_messages.to_string(),
                },
                QueueAttribute {
                    name: "ApproximateNumberOfMessagesNotVisible".to_string(),
                    value: attrs.approximate_number_of_messages_not_visible.to_string(),
                },
                QueueAttribute {
                    name: "CreatedTimestamp".to_string(),
                    value: attrs.created_timestamp,
                },
            ];

            let response = GetQueueAttributesResponse {
                get_queue_attributes_result: GetQueueAttributesResult { attributes },
            };

            xml_response(response)
        },
        Ok(None) => error_response("AWS.SimpleQueueService.NonExistentQueue", "Queue does not exist"),
        Err(_) => error_response("InternalError", "Failed to get queue attributes"),
    }
}

fn parse_form_params(body: &str) -> Result<HashMap<String, String>, ()> {
    let mut params = HashMap::new();
    for pair in body.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            let key = urlencoding::decode(key).map_err(|_| ())?.to_string();
            let value = urlencoding::decode(value).map_err(|_| ())?.to_string();
            params.insert(key, value);
        }
    }
    Ok(params)
}

fn parse_message_attributes(params: &HashMap<String, String>) -> Option<HashMap<String, MessageAttributeValue>> {
    let mut attributes = HashMap::new();
    let mut i = 1;
    
    loop {
        let name_key = format!("MessageAttribute.{}.Name", i);
        let value_key = format!("MessageAttribute.{}.Value.StringValue", i);
        let type_key = format!("MessageAttribute.{}.Value.DataType", i);
        
        if let (Some(name), Some(value), Some(data_type)) = (
            params.get(&name_key),
            params.get(&value_key),
            params.get(&type_key),
        ) {
            attributes.insert(name.clone(), MessageAttributeValue {
                string_value: Some(value.clone()),
                binary_value: None,
                data_type: data_type.clone(),
            });
            i += 1;
        } else {
            break;
        }
    }
    
    if attributes.is_empty() {
        None
    } else {
        Some(attributes)
    }
}

fn xml_response<T: serde::Serialize>(data: T) -> Response {
    match to_xml(&data) {
        Ok(xml) => {
            let full_xml = format!(r#"<?xml version="1.0" encoding="UTF-8"?>{}"#, xml);
            (
                StatusCode::OK,
                [("Content-Type", "application/xml")],
                full_xml,
            ).into_response()
        },
        Err(_) => error_response("InternalError", "Failed to serialize response"),
    }
}

fn error_response(code: &str, message: &str) -> Response {
    let error = ErrorResponse {
        error: SqsError {
            error_type: "Sender".to_string(),
            code: code.to_string(),
            message: message.to_string(),
        },
    };

    match to_xml(&error) {
        Ok(xml) => {
            let full_xml = format!(r#"<?xml version="1.0" encoding="UTF-8"?>{}"#, xml);
            (
                StatusCode::BAD_REQUEST,
                [("Content-Type", "application/xml")],
                full_xml,
            ).into_response()
        },
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal server error",
        ).into_response(),
    }
}

// Health check handlers for production monitoring
async fn health_check(State(state): State<Arc<AppState>>) -> Response {
    let health_status = get_system_health(&state.queue_service).await;
    
    let response = serde_json::json!({
        "status": health_status.status,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "service": "qlite-sqs",
        "version": env!("CARGO_PKG_VERSION"),
        "checks": {
            "database": health_status.database_ok,
            "queues": health_status.queue_count,
            "retention_service": health_status.retention_active
        }
    });
    
    let status_code = if health_status.status == "healthy" {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    
    (
        status_code,
        [("Content-Type", "application/json")],
        response.to_string(),
    ).into_response()
}

async fn readiness_check(State(state): State<Arc<AppState>>) -> Response {
    // Check if the service is ready to handle requests
    match state.queue_service.list_queues().await {
        Ok(_) => (
            StatusCode::OK,
            [("Content-Type", "application/json")],
            serde_json::json!({"status": "ready"}).to_string(),
        ).into_response(),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            [("Content-Type", "application/json")],
            serde_json::json!({"status": "not ready", "reason": "database unavailable"}).to_string(),
        ).into_response(),
    }
}

async fn liveness_check() -> Response {
    // Simple liveness check - if this handler runs, the service is alive
    (
        StatusCode::OK,
        [("Content-Type", "application/json")],
        serde_json::json!({
            "status": "alive",
            "timestamp": chrono::Utc::now().to_rfc3339()
        }).to_string(),
    ).into_response()
}

async fn metrics_endpoint(State(state): State<Arc<AppState>>) -> Response {
    let health_status = get_system_health(&state.queue_service).await;
    
    let metrics = format!(
        "# HELP qlite_queues_total Total number of queues\n\
         # TYPE qlite_queues_total gauge\n\
         qlite_queues_total {}\n\
         # HELP qlite_health_status Health status (1=healthy, 0=unhealthy)\n\
         # TYPE qlite_health_status gauge\n\
         qlite_health_status {}\n\
         # HELP qlite_retention_active Retention service status (1=active, 0=inactive)\n\
         # TYPE qlite_retention_active gauge\n\
         qlite_retention_active {}\n",
        health_status.queue_count,
        if health_status.status == "healthy" { 1 } else { 0 },
        if health_status.retention_active { 1 } else { 0 }
    );
    
    (
        StatusCode::OK,
        [("Content-Type", "text/plain")],
        metrics,
    ).into_response()
}

#[derive(Debug)]
struct SystemHealth {
    status: String,
    database_ok: bool,
    queue_count: usize,
    retention_active: bool,
}

async fn get_system_health(queue_service: &QueueService) -> SystemHealth {
    let database_ok = (queue_service.list_queues().await).is_ok();
    
    let queue_count = match queue_service.list_queues().await {
        Ok(queues) => queues.len(),
        Err(_) => 0,
    };
    
    let retention_active = true; // Assume retention service is active if server is running
    
    let status = if database_ok {
        "healthy"
    } else {
        "unhealthy"
    }.to_string();
    
    SystemHealth {
        status,
        database_ok,
        queue_count,
        retention_active,
    }
}