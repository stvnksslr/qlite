use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// SQS Request Types
#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    #[serde(rename = "MessageBody")]
    pub message_body: String,
    #[serde(rename = "MessageAttributes", default)]
    pub message_attributes: HashMap<String, MessageAttribute>,
    #[serde(rename = "MessageDeduplicationId")]
    pub message_deduplication_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MessageAttribute {
    #[serde(rename = "StringValue")]
    pub string_value: Option<String>,
    #[serde(rename = "BinaryValue")]
    pub binary_value: Option<String>,
    #[serde(rename = "DataType")]
    pub data_type: String,
}

#[derive(Debug, Deserialize)]
pub struct ReceiveMessageRequest {
    #[serde(rename = "MaxNumberOfMessages", default = "default_max_messages")]
    pub max_number_of_messages: u32,
    #[serde(rename = "WaitTimeSeconds", default)]
    pub wait_time_seconds: u32,
}

fn default_max_messages() -> u32 {
    1
}

#[derive(Debug, Deserialize)]
pub struct DeleteMessageRequest {
    #[serde(rename = "ReceiptHandle")]
    pub receipt_handle: String,
}

// SQS Response Types
#[derive(Debug, Serialize)]
pub struct SendMessageResponse {
    #[serde(rename = "SendMessageResult")]
    pub send_message_result: SendMessageResult,
}

#[derive(Debug, Serialize)]
pub struct SendMessageResult {
    #[serde(rename = "MessageId")]
    pub message_id: String,
    #[serde(rename = "MD5OfBody")]
    pub md5_of_body: String,
}

#[derive(Debug, Serialize)]
pub struct ReceiveMessageResponse {
    #[serde(rename = "ReceiveMessageResult")]
    pub receive_message_result: ReceiveMessageResult,
}

#[derive(Debug, Serialize)]
pub struct ReceiveMessageResult {
    #[serde(rename = "Message", default)]
    pub messages: Vec<SqsMessage>,
}

#[derive(Debug, Serialize)]
pub struct SqsMessage {
    #[serde(rename = "MessageId")]
    pub message_id: String,
    #[serde(rename = "ReceiptHandle")]
    pub receipt_handle: String,
    #[serde(rename = "Body")]
    pub body: String,
    #[serde(rename = "Attributes", default)]
    pub attributes: HashMap<String, String>,
    #[serde(rename = "MessageAttributes", default)]
    pub message_attributes: HashMap<String, MessageAttribute>,
}

#[derive(Debug, Serialize)]
pub struct DeleteMessageResponse {
    #[serde(rename = "DeleteMessageResult")]
    pub delete_message_result: DeleteMessageResult,
}

#[derive(Debug, Serialize)]
pub struct DeleteMessageResult {}

#[derive(Debug, Serialize)]
pub struct ListQueuesResponse {
    #[serde(rename = "ListQueuesResult")]
    pub list_queues_result: ListQueuesResult,
}

#[derive(Debug, Serialize)]
pub struct ListQueuesResult {
    #[serde(rename = "QueueUrl", default)]
    pub queue_urls: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct GetQueueAttributesResponse {
    #[serde(rename = "GetQueueAttributesResult")]
    pub get_queue_attributes_result: GetQueueAttributesResult,
}

#[derive(Debug, Serialize)]
pub struct GetQueueAttributesResult {
    #[serde(rename = "Attribute")]
    pub attributes: Vec<QueueAttribute>,
}

#[derive(Debug, Serialize)]
pub struct QueueAttribute {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Value")]
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct CreateQueueResponse {
    #[serde(rename = "CreateQueueResult")]
    pub create_queue_result: CreateQueueResult,
}

#[derive(Debug, Serialize)]
pub struct CreateQueueResult {
    #[serde(rename = "QueueUrl")]
    pub queue_url: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    #[serde(rename = "Error")]
    pub error: SqsError,
}

#[derive(Debug, Serialize)]
pub struct SqsError {
    #[serde(rename = "Type")]
    pub error_type: String,
    #[serde(rename = "Code")]
    pub code: String,
    #[serde(rename = "Message")]
    pub message: String,
}