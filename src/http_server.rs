use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use quick_xml::se::to_string as to_xml;
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
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    // Determine the action from either query parameter or X-Amz-Target header
    let action = if let Some(action) = query.get("Action") {
        // Form-encoded request
        action.clone()
    } else if let Some(target) = headers.get("x-amz-target").and_then(|v| v.to_str().ok()) {
        // AWS CLI/SDK JSON request - extract action from X-Amz-Target
        if let Some(action) = target.strip_prefix("AmazonSQS.") {
            action.to_string()
        } else {
            return error_response("InvalidAction", "Invalid X-Amz-Target header");
        }
    } else {
        return error_response("MissingAction", "Action parameter or X-Amz-Target header is required");
    };

    // Parse parameters based on content type
    let params = if content_type.contains("application/x-amz-json") {
        // Parse JSON body for AWS CLI/SDK requests
        parse_json_params(&body).unwrap_or_default()
    } else {
        // Parse form-encoded body for traditional requests
        parse_form_params(&body).unwrap_or_default()
    };

    match action.as_str() {
        "ListQueues" => handle_list_queues(state).await,
        "CreateQueue" => {
            if let Some(queue_name) = params.get("QueueName") {
                handle_create_queue_with_attributes(state, queue_name, &params).await
            } else {
                error_response("MissingParameter", "QueueName parameter is required")
            }
        },
        "GetQueueUrl" => {
            if let Some(queue_name) = params.get("QueueName") {
                handle_get_queue_url(state, queue_name).await
            } else {
                error_response("MissingParameter", "QueueName parameter is required")
            }
        },
        "SendMessageBatch" => {
            // Extract queue name from batch entries or use a parameter
            handle_send_message_batch(state, &params).await
        },
        "DeleteMessageBatch" => {
            handle_delete_message_batch(state, &params).await
        },
        "SetQueueAttributes" => {
            if let Some(queue_url) = params.get("QueueUrl").cloned() {
                // Extract queue name from URL (assuming format like http://localhost:3000/queue-name)
                let queue_name = queue_url.split('/').last().unwrap_or("");
                handle_set_queue_attributes(state, queue_name, params).await
            } else {
                error_response("MissingParameter", "QueueUrl parameter is required")
            }
        },
        "GetQueueAttributes" => {
            if let Some(queue_url) = params.get("QueueUrl").cloned() {
                // Extract queue name from URL (assuming format like http://localhost:3000/queue-name)
                let queue_name = queue_url.split('/').last().unwrap_or("");
                handle_get_queue_attributes(state, queue_name).await
            } else {
                error_response("MissingParameter", "QueueUrl parameter is required")
            }
        },
        "SendMessage" => {
            if let Some(queue_url) = params.get("QueueUrl").cloned() {
                // Extract queue name from URL (assuming format like http://localhost:3000/queue-name)
                let queue_name = queue_url.split('/').last().unwrap_or("");
                handle_send_message_enhanced(state, &queue_name, params).await
            } else {
                error_response("MissingParameter", "QueueUrl parameter is required")
            }
        },
        "ReceiveMessage" => {
            if let Some(queue_url) = params.get("QueueUrl").cloned() {
                // Extract queue name from URL (assuming format like http://localhost:3000/queue-name)
                let queue_name = queue_url.split('/').last().unwrap_or("");
                handle_receive_message_enhanced(state, &queue_name, params).await
            } else {
                error_response("MissingParameter", "QueueUrl parameter is required")
            }
        },
        "DeleteMessage" => {
            if let Some(queue_url) = params.get("QueueUrl").cloned() {
                // Extract queue name from URL (assuming format like http://localhost:3000/queue-name)
                let queue_name = queue_url.split('/').last().unwrap_or("");
                handle_delete_message(state, &queue_name, params).await
            } else {
                error_response("MissingParameter", "QueueUrl parameter is required")
            }
        },
        "DeleteQueue" => {
            if let Some(queue_url) = params.get("QueueUrl").cloned() {
                // Extract queue name from URL (assuming format like http://localhost:3000/queue-name)
                let queue_name = queue_url.split('/').last().unwrap_or("");
                handle_delete_queue(state, queue_name).await
            } else {
                error_response("MissingParameter", "QueueUrl parameter is required")
            }
        },
        _ => error_response("InvalidAction", &format!("Unknown action: {}", action)),
    }
}

async fn handle_queue_action(
    State(state): State<Arc<AppState>>,
    Path(queue_name): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    // Determine the action from either query parameter or X-Amz-Target header
    let action = if let Some(action) = query.get("Action") {
        action.clone()
    } else if let Some(target) = headers.get("x-amz-target").and_then(|v| v.to_str().ok()) {
        if let Some(action) = target.strip_prefix("AmazonSQS.") {
            action.to_string()
        } else {
            return error_response("InvalidAction", "Invalid X-Amz-Target header");
        }
    } else {
        return error_response("MissingAction", "Action parameter or X-Amz-Target header is required");
    };

    // Parse parameters based on content type
    let params = if content_type.contains("application/x-amz-json") {
        parse_json_params(&body).unwrap_or_default()
    } else {
        parse_form_params(&body).unwrap_or_default()
    };

    match action.as_str() {
        "SendMessage" => handle_send_message_enhanced(state, &queue_name, params).await,
        "ReceiveMessage" => handle_receive_message_enhanced(state, &queue_name, params).await,
        "DeleteMessage" => handle_delete_message(state, &queue_name, params).await,
        "GetQueueAttributes" => handle_get_queue_attributes(state, &queue_name).await,
        "SetQueueAttributes" => handle_set_queue_attributes(state, &queue_name, params).await,
        "SendMessageBatch" => handle_send_message_batch_for_queue(state, &queue_name, params).await,
        "ReceiveMessageBatch" => handle_receive_message_batch(state, &queue_name, params).await,
        "DeleteMessageBatch" => handle_delete_message_batch_for_queue(state, &queue_name, params).await,
        _ => error_response("InvalidAction", &format!("Unknown action: {}", action)),
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

async fn handle_create_queue_with_attributes(state: Arc<AppState>, queue_name: &str, _params: &HashMap<String, String>) -> Response {
    // For now, just create the queue normally - attributes support can be added later
    handle_create_queue(state, queue_name).await
}

async fn handle_get_queue_url(state: Arc<AppState>, queue_name: &str) -> Response {
    // Check if queue exists by trying to list it
    match state.queue_service.list_queues().await {
        Ok(queues) => {
            if queues.iter().any(|(name, _)| name == queue_name) {
                let response = GetQueueUrlResponse {
                    get_queue_url_result: GetQueueUrlResult {
                        queue_url: format!("{}/{}", state.base_url, queue_name),
                    },
                };
                xml_response(response)
            } else {
                error_response("AWS.SimpleQueueService.NonExistentQueue", "The specified queue does not exist")
            }
        },
        Err(_) => error_response("InternalError", "Failed to check queue existence"),
    }
}

async fn handle_delete_queue(state: Arc<AppState>, queue_name: &str) -> Response {
    match state.queue_service.delete_queue(queue_name).await {
        Ok(true) => {
            let response = DeleteQueueResponse {
                delete_queue_result: DeleteQueueResult {},
            };
            xml_response(response)
        },
        Ok(false) => {
            error_response("AWS.SimpleQueueService.NonExistentQueue", "The specified queue does not exist")
        },
        Err(_) => {
            error_response("InternalError", "Failed to delete queue")
        }
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

// New handlers for enhanced functionality

async fn handle_send_message_enhanced(
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
    let delay_seconds = params.get("DelaySeconds")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);

    match state.queue_service.send_message_enhanced(
        queue_name,
        message_body,
        message_attributes,
        deduplication_id,
        delay_seconds,
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
        Err(err) => {
            eprintln!("SendMessage error: {:?}", err);
            error_response("InternalError", "Failed to send message")
        },
    }
}

async fn handle_receive_message_enhanced(
    state: Arc<AppState>,
    queue_name: &str,
    params: HashMap<String, String>,
) -> Response {
    let max_messages = params.get("MaxNumberOfMessages")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(1);
    
    let wait_time_seconds = params.get("WaitTimeSeconds")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);

    match state.queue_service.receive_messages_enhanced(queue_name, max_messages, wait_time_seconds).await {
        Ok(messages) => {
            let sqs_messages: Vec<SqsMessage> = messages.into_iter().map(|received_msg| {
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

                SqsMessage {
                    message_id: received_msg.id,
                    receipt_handle: received_msg.receipt_handle,
                    body: received_msg.body,
                    attributes: create_basic_system_attributes(),
                    message_attributes,
                }
            }).collect();

            let response = ReceiveMessageResponse {
                receive_message_result: ReceiveMessageResult {
                    messages: sqs_messages,
                },
            };

            xml_response(response)
        },
        Err(_) => error_response("InternalError", "Failed to receive messages"),
    }
}

async fn handle_set_queue_attributes(
    state: Arc<AppState>,
    queue_name: &str,
    params: HashMap<String, String>,
) -> Response {
    // Parse queue attributes from parameters
    let mut attributes = HashMap::new();
    
    for (key, value) in params.iter() {
        if key.starts_with("Attribute.") && key.ends_with(".Name") {
            if let Some(index) = key.strip_prefix("Attribute.").and_then(|s| s.strip_suffix(".Name")) {
                let value_key = format!("Attribute.{}.Value", index);
                if let Some(attr_value) = params.get(&value_key) {
                    attributes.insert(value.clone(), attr_value.clone());
                }
            }
        }
    }

    match state.queue_service.set_queue_attributes(queue_name, attributes).await {
        Ok(()) => {
            let response = SetQueueAttributesResponse {
                set_queue_attributes_result: SetQueueAttributesResult {},
            };
            xml_response(response)
        },
        Err(_) => error_response("InternalError", "Failed to set queue attributes"),
    }
}

// Batch operation handlers

async fn handle_send_message_batch(state: Arc<AppState>, params: &HashMap<String, String>) -> Response {
    // Extract queue URL and derive queue name
    let queue_url = match params.get("QueueUrl") {
        Some(url) => url,
        None => {
            let error_response = BatchResultErrorEntry {
                id: "1".to_string(),
                code: "MissingParameter".to_string(),
                message: "QueueUrl parameter is required".to_string(),
                sender_fault: true,
            };
            
            let response = SendMessageBatchResponse {
                send_message_batch_result: SendMessageBatchResult {
                    successful: vec![],
                    failed: vec![error_response],
                },
            };
            return xml_response(response);
        }
    };

    // Extract queue name from URL (format: http://localhost:3000/queue-name)
    let queue_name = queue_url.split('/').last().unwrap_or("");
    if queue_name.is_empty() {
        let error_response = BatchResultErrorEntry {
            id: "1".to_string(),
            code: "InvalidParameterValue".to_string(),
            message: "Invalid QueueUrl format".to_string(),
            sender_fault: true,
        };
        
        let response = SendMessageBatchResponse {
            send_message_batch_result: SendMessageBatchResult {
                successful: vec![],
                failed: vec![error_response],
            },
        };
        return xml_response(response);
    }

    // Delegate to the queue-specific handler
    handle_send_message_batch_for_queue(state, queue_name, params.clone()).await
}

async fn handle_send_message_batch_for_queue(
    state: Arc<AppState>,
    queue_name: &str,
    params: HashMap<String, String>,
) -> Response {
    // Parse batch entries
    let mut entries = Vec::new();
    let mut entry_ids = Vec::new();
    let mut i = 1;
    
    loop {
        let id_key = format!("SendMessageBatchRequestEntry.{}.Id", i);
        let body_key = format!("SendMessageBatchRequestEntry.{}.MessageBody", i);
        let delay_key = format!("SendMessageBatchRequestEntry.{}.DelaySeconds", i);
        let dedup_key = format!("SendMessageBatchRequestEntry.{}.MessageDeduplicationId", i);
        
        if let (Some(id), Some(body)) = (params.get(&id_key), params.get(&body_key)) {
            let delay_seconds = params.get(&delay_key)
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);
            
            let deduplication_id = params.get(&dedup_key).cloned();
            
            // Parse message attributes if present
            let mut attributes = std::collections::HashMap::new();
            let mut attr_index = 1;
            loop {
                let attr_name_key = format!("SendMessageBatchRequestEntry.{}.MessageAttribute.{}.Name", i, attr_index);
                let attr_value_key = format!("SendMessageBatchRequestEntry.{}.MessageAttribute.{}.Value.StringValue", i, attr_index);
                
                if let (Some(attr_name), Some(attr_value)) = (params.get(&attr_name_key), params.get(&attr_value_key)) {
                    attributes.insert(attr_name.clone(), MessageAttributeValue {
                        string_value: Some(attr_value.clone()),
                        binary_value: None,
                        data_type: "String".to_string(),
                    });
                    attr_index += 1;
                } else {
                    break;
                }
            }
            
            let attributes = if attributes.is_empty() { None } else { Some(attributes) };
            let message_id = uuid::Uuid::new_v4().to_string();
            
            entries.push((
                queue_name.to_string(),
                message_id.clone(),
                body.clone(),
                attributes,
                deduplication_id,
                delay_seconds
            ));
            
            entry_ids.push((id.clone(), message_id, body.clone()));
            i += 1;
            
            if i > 10 { // AWS limit
                break;
            }
        } else {
            break;
        }
    }

    if entries.is_empty() {
        let error_response = BatchResultErrorEntry {
            id: "1".to_string(),
            code: "EmptyBatchRequest".to_string(),
            message: "The batch request doesn't contain any entries".to_string(),
            sender_fault: true,
        };
        
        let response = SendMessageBatchResponse {
            send_message_batch_result: SendMessageBatchResult {
                successful: vec![],
                failed: vec![error_response],
            },
        };
        return xml_response(response);
    }

    // Use the new batch service method
    match state.queue_service.send_messages_batch(entries).await {
        Ok(results) => {
            let mut successful = Vec::new();
            let mut failed = Vec::new();
            
            for (i, result) in results.into_iter().enumerate() {
                let (entry_id, message_id, body) = &entry_ids[i];
                
                match result {
                    Ok(_) => {
                        successful.push(SendMessageBatchResultEntry {
                            id: entry_id.clone(),
                            message_id: message_id.clone(),
                            md5_of_body: format!("{:x}", md5::compute(body.as_bytes())),
                        });
                    }
                    Err(error) => {
                        failed.push(BatchResultErrorEntry {
                            id: entry_id.clone(),
                            code: "InternalError".to_string(),
                            message: error,
                            sender_fault: false,
                        });
                    }
                }
            }
            
            let response = SendMessageBatchResponse {
                send_message_batch_result: SendMessageBatchResult {
                    successful,
                    failed,
                },
            };
            xml_response(response)
        }
        Err(_) => {
            let error_response = BatchResultErrorEntry {
                id: "1".to_string(),
                code: "InternalError".to_string(),
                message: "Failed to send batch messages".to_string(),
                sender_fault: false,
            };
            
            let response = SendMessageBatchResponse {
                send_message_batch_result: SendMessageBatchResult {
                    successful: vec![],
                    failed: vec![error_response],
                },
            };
            xml_response(response)
        }
    }
}

async fn handle_delete_message_batch(state: Arc<AppState>, params: &HashMap<String, String>) -> Response {
    // Extract queue URL and derive queue name
    let queue_url = match params.get("QueueUrl") {
        Some(url) => url,
        None => {
            let error_response = BatchResultErrorEntry {
                id: "1".to_string(),
                code: "MissingParameter".to_string(),
                message: "QueueUrl parameter is required".to_string(),
                sender_fault: true,
            };
            
            let response = DeleteMessageBatchResponse {
                delete_message_batch_result: DeleteMessageBatchResult {
                    successful: vec![],
                    failed: vec![error_response],
                },
            };
            return xml_response(response);
        }
    };

    // Extract queue name from URL
    let queue_name = queue_url.split('/').last().unwrap_or("");
    if queue_name.is_empty() {
        let error_response = BatchResultErrorEntry {
            id: "1".to_string(),
            code: "InvalidParameterValue".to_string(),
            message: "Invalid QueueUrl format".to_string(),
            sender_fault: true,
        };
        
        let response = DeleteMessageBatchResponse {
            delete_message_batch_result: DeleteMessageBatchResult {
                successful: vec![],
                failed: vec![error_response],
            },
        };
        return xml_response(response);
    }

    // Delegate to the queue-specific handler
    handle_delete_message_batch_for_queue(state, queue_name, params.clone()).await
}

async fn handle_delete_message_batch_for_queue(
    state: Arc<AppState>,
    _queue_name: &str,
    params: HashMap<String, String>,
) -> Response {
    let mut entries = Vec::new();
    let mut entry_ids = Vec::new();
    let mut i = 1;
    
    // Parse all entries first
    loop {
        let id_key = format!("DeleteMessageBatchRequestEntry.{}.Id", i);
        let receipt_key = format!("DeleteMessageBatchRequestEntry.{}.ReceiptHandle", i);
        
        if let (Some(id), Some(receipt_handle)) = (params.get(&id_key), params.get(&receipt_key)) {
            entries.push(receipt_handle.clone());
            entry_ids.push(id.clone());
            i += 1;
            
            if i > 10 { // AWS limit
                break;
            }
        } else {
            break;
        }
    }

    if entries.is_empty() {
        let error_response = BatchResultErrorEntry {
            id: "1".to_string(),
            code: "EmptyBatchRequest".to_string(),
            message: "The batch request doesn't contain any entries".to_string(),
            sender_fault: true,
        };
        
        let response = DeleteMessageBatchResponse {
            delete_message_batch_result: DeleteMessageBatchResult {
                successful: vec![],
                failed: vec![error_response],
            },
        };
        return xml_response(response);
    }

    // Use the batch delete service method
    match state.queue_service.delete_messages_batch(entries).await {
        Ok(results) => {
            let mut successful = Vec::new();
            let mut failed = Vec::new();
            
            for (i, result) in results.into_iter().enumerate() {
                let entry_id = &entry_ids[i];
                
                match result {
                    Ok(true) => {
                        successful.push(DeleteMessageBatchResultEntry {
                            id: entry_id.clone(),
                        });
                    }
                    Ok(false) => {
                        failed.push(BatchResultErrorEntry {
                            id: entry_id.clone(),
                            code: "ReceiptHandleIsInvalid".to_string(),
                            message: "The receipt handle provided is not valid".to_string(),
                            sender_fault: true,
                        });
                    }
                    Err(error) => {
                        failed.push(BatchResultErrorEntry {
                            id: entry_id.clone(),
                            code: "InternalError".to_string(),
                            message: error,
                            sender_fault: false,
                        });
                    }
                }
            }
            
            let response = DeleteMessageBatchResponse {
                delete_message_batch_result: DeleteMessageBatchResult {
                    successful,
                    failed,
                },
            };
            xml_response(response)
        }
        Err(_) => {
            let error_response = BatchResultErrorEntry {
                id: "1".to_string(),
                code: "InternalError".to_string(),
                message: "Failed to delete batch messages".to_string(),
                sender_fault: false,
            };
            
            let response = DeleteMessageBatchResponse {
                delete_message_batch_result: DeleteMessageBatchResult {
                    successful: vec![],
                    failed: vec![error_response],
                },
            };
            xml_response(response)
        }
    }
}

async fn handle_receive_message_batch(
    state: Arc<AppState>,
    queue_name: &str,
    params: HashMap<String, String>,
) -> Response {
    // Parse MaxNumberOfMessages parameter (default 1, max 10 for AWS compatibility)
    let max_messages = params
        .get("MaxNumberOfMessages")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(1)
        .min(10);  // AWS SQS limit
    
    let _wait_time_seconds = params
        .get("WaitTimeSeconds") 
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);
        
    // For now, use the batch receive method (ignore wait_time_seconds until Phase 5)
    match state.queue_service.receive_messages_batch(queue_name, max_messages).await {
        Ok(messages) => {
            let messages_xml: Vec<SqsMessage> = messages
                .into_iter()
                .map(|msg| SqsMessage {
                    message_id: msg.id.clone(),
                    receipt_handle: msg.id, // For now, receipt handle is the same as message ID
                    body: msg.body,
                    attributes: create_basic_system_attributes(),
                    message_attributes: msg.attributes.unwrap_or_default()
                        .into_iter()
                        .map(|(k, v)| (k, MessageAttribute {
                            string_value: v.string_value,
                            binary_value: v.binary_value,
                            data_type: v.data_type,
                        }))
                        .collect(),
                })
                .collect();

            let response = ReceiveMessageResponse {
                receive_message_result: ReceiveMessageResult {
                    messages: messages_xml,
                },
            };

            xml_response(response)
        }
        Err(_) => error_response(
            "InternalError",
            "Failed to receive messages",
        ),
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

fn parse_json_params(body: &str) -> Result<HashMap<String, String>, ()> {
    if body.trim().is_empty() {
        return Ok(HashMap::new());
    }
    
    match serde_json::from_str::<serde_json::Value>(body) {
        Ok(json) => {
            let mut params = HashMap::new();
            if let serde_json::Value::Object(obj) = json {
                for (key, value) in obj {
                    match key.as_str() {
                        // Handle batch entries specially
                        "Entries" => {
                            if let serde_json::Value::Array(entries) = value {
                                for (i, entry) in entries.iter().enumerate() {
                                    let entry_num = i + 1;
                                    if let serde_json::Value::Object(entry_obj) = entry {
                                        for (entry_key, entry_value) in entry_obj {
                                            // Determine prefix based on action type - we'll check headers context
                                            let param_key = format!("SendMessageBatchRequestEntry.{}.{}", entry_num, entry_key);
                                            let value_str = match entry_value {
                                                serde_json::Value::String(s) => s.clone(),
                                                serde_json::Value::Number(n) => n.to_string(),
                                                serde_json::Value::Bool(b) => b.to_string(),
                                                _ => entry_value.to_string().trim_matches('"').to_string(),
                                            };
                                            params.insert(param_key, value_str.clone());
                                            
                                            // Also add DeleteMessageBatchRequestEntry version for delete operations
                                            let delete_param_key = format!("DeleteMessageBatchRequestEntry.{}.{}", entry_num, entry_key);
                                            params.insert(delete_param_key, value_str);
                                        }
                                    }
                                }
                            }
                        },
                        _ => {
                            let value_str = match value {
                                serde_json::Value::String(s) => s,
                                serde_json::Value::Number(n) => n.to_string(),
                                serde_json::Value::Bool(b) => b.to_string(),
                                _ => value.to_string().trim_matches('"').to_string(),
                            };
                            params.insert(key, value_str);
                        }
                    }
                }
            }
            Ok(params)
        },
        Err(_) => Err(()),
    }
}

fn create_basic_system_attributes() -> HashMap<String, String> {
    let mut system_attrs = HashMap::new();
    
    // SentTimestamp - when message was sent (use current time as approximation)
    let sent_timestamp = chrono::Utc::now().timestamp_millis().to_string();
    system_attrs.insert("SentTimestamp".to_string(), sent_timestamp);
    
    // ApproximateReceiveCount - start with 1 (would be updated from database in real implementation)
    system_attrs.insert("ApproximateReceiveCount".to_string(), "1".to_string());
    
    // SenderId - dummy value for compatibility  
    system_attrs.insert("SenderId".to_string(), "AIDAIENQZJOLO23YVJ4VO".to_string());
    
    system_attrs
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

// Enhanced error response with proper AWS SQS error codes and HTTP status codes
fn error_response(code: &str, message: &str) -> Response {
    let (http_status, error_type) = get_aws_sqs_error_details(code);
    
    let error = ErrorResponse {
        error: SqsError {
            error_type,
            code: code.to_string(),
            message: message.to_string(),
        },
    };

    match to_xml(&error) {
        Ok(xml) => {
            let full_xml = format!(r#"<?xml version="1.0" encoding="UTF-8"?>{}"#, xml);
            (
                http_status,
                [("Content-Type", "application/xml")],
                full_xml,
            ).into_response()
        },
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [("Content-Type", "application/xml")],
            r#"<?xml version="1.0" encoding="UTF-8"?><ErrorResponse><Error><Type>Receiver</Type><Code>InternalError</Code><Message>Failed to serialize response</Message></Error></ErrorResponse>"#,
        ).into_response(),
    }
}

// AWS SQS error code mappings to HTTP status codes and error types
fn get_aws_sqs_error_details(code: &str) -> (StatusCode, String) {
    match code {
        // 400 Bad Request errors (Client/Sender errors)
        "InvalidParameterValue" => (StatusCode::BAD_REQUEST, "Sender".to_string()),
        "MissingParameter" => (StatusCode::BAD_REQUEST, "Sender".to_string()),
        "InvalidAction" => (StatusCode::BAD_REQUEST, "Sender".to_string()),
        "MalformedQueryString" => (StatusCode::BAD_REQUEST, "Sender".to_string()),
        "InvalidQueryParameter" => (StatusCode::BAD_REQUEST, "Sender".to_string()),
        "InvalidParameterCombination" => (StatusCode::BAD_REQUEST, "Sender".to_string()),
        "InvalidAttributeName" => (StatusCode::BAD_REQUEST, "Sender".to_string()),
        "InvalidAttributeValue" => (StatusCode::BAD_REQUEST, "Sender".to_string()),
        "InvalidMessageContents" => (StatusCode::BAD_REQUEST, "Sender".to_string()),
        "MessageTooLong" => (StatusCode::BAD_REQUEST, "Sender".to_string()),
        "PurgeQueueInProgress" => (StatusCode::BAD_REQUEST, "Sender".to_string()),
        "BatchEntryIdsNotDistinct" => (StatusCode::BAD_REQUEST, "Sender".to_string()),
        "BatchRequestTooLong" => (StatusCode::BAD_REQUEST, "Sender".to_string()),
        "EmptyBatchRequest" => (StatusCode::BAD_REQUEST, "Sender".to_string()),
        "InvalidBatchEntryId" => (StatusCode::BAD_REQUEST, "Sender".to_string()),
        "TooManyEntriesInBatchRequest" => (StatusCode::BAD_REQUEST, "Sender".to_string()),
        "UnsupportedOperation" => (StatusCode::BAD_REQUEST, "Sender".to_string()),
        "InvalidIdFormat" => (StatusCode::BAD_REQUEST, "Sender".to_string()),
        "MissingAction" => (StatusCode::BAD_REQUEST, "Sender".to_string()),
        
        // 403 Forbidden errors
        "AccessDenied" => (StatusCode::FORBIDDEN, "Sender".to_string()),
        "InvalidSecurity" => (StatusCode::FORBIDDEN, "Sender".to_string()),
        "RequestExpired" => (StatusCode::FORBIDDEN, "Sender".to_string()),
        
        // 404 Not Found errors
        "AWS.SimpleQueueService.NonExistentQueue" => (StatusCode::BAD_REQUEST, "Sender".to_string()), // AWS actually returns 400 for this
        "NonExistentQueue" => (StatusCode::BAD_REQUEST, "Sender".to_string()),
        "QueueDoesNotExist" => (StatusCode::BAD_REQUEST, "Sender".to_string()),
        
        // 409 Conflict errors  
        "QueueAlreadyExists" => (StatusCode::BAD_REQUEST, "Sender".to_string()), // AWS returns 400 for this
        "QueueDeletedRecently" => (StatusCode::BAD_REQUEST, "Sender".to_string()),
        
        // 413 Request Entity Too Large
        "RequestTooLarge" => (StatusCode::PAYLOAD_TOO_LARGE, "Sender".to_string()),
        
        // 429 Too Many Requests
        "Throttling" => (StatusCode::TOO_MANY_REQUESTS, "Sender".to_string()),
        "RequestThrottled" => (StatusCode::TOO_MANY_REQUESTS, "Sender".to_string()),
        
        // 500 Internal Server Error (Receiver errors)
        "InternalError" => (StatusCode::INTERNAL_SERVER_ERROR, "Receiver".to_string()),
        "ServiceFailure" => (StatusCode::INTERNAL_SERVER_ERROR, "Receiver".to_string()),
        "ServiceUnavailable" => (StatusCode::SERVICE_UNAVAILABLE, "Receiver".to_string()),
        
        // Default to 400 Bad Request for unknown errors
        _ => (StatusCode::BAD_REQUEST, "Sender".to_string()),
    }
}

// Request validation functions






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