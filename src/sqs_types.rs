use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// SQS Message Attribute (used in responses)
#[derive(Debug, Deserialize, Serialize)]
pub struct MessageAttribute {
    #[serde(rename = "StringValue")]
    pub string_value: Option<String>,
    #[serde(rename = "BinaryValue")]
    pub binary_value: Option<String>,
    #[serde(rename = "DataType")]
    pub data_type: String,
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
pub struct GetQueueUrlResponse {
    #[serde(rename = "GetQueueUrlResult")]
    pub get_queue_url_result: GetQueueUrlResult,
}

#[derive(Debug, Serialize)]
pub struct GetQueueUrlResult {
    #[serde(rename = "QueueUrl")]
    pub queue_url: String,
}

#[derive(Debug, Serialize)]
pub struct SetQueueAttributesResponse {
    #[serde(rename = "SetQueueAttributesResult")]
    pub set_queue_attributes_result: SetQueueAttributesResult,
}

#[derive(Debug, Serialize)]
pub struct SetQueueAttributesResult {}

#[derive(Debug, Serialize)]
pub struct SendMessageBatchResponse {
    #[serde(rename = "SendMessageBatchResult")]
    pub send_message_batch_result: SendMessageBatchResult,
}

#[derive(Debug, Serialize)]
pub struct SendMessageBatchResult {
    #[serde(rename = "SendMessageBatchResultEntry", default)]
    pub successful: Vec<SendMessageBatchResultEntry>,
    #[serde(rename = "BatchResultErrorEntry", default)]
    pub failed: Vec<BatchResultErrorEntry>,
}

#[derive(Debug, Serialize)]
pub struct SendMessageBatchResultEntry {
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "MessageId")]
    pub message_id: String,
    #[serde(rename = "MD5OfBody")]
    pub md5_of_body: String,
}

#[derive(Debug, Serialize)]
pub struct BatchResultErrorEntry {
    #[serde(rename = "Id")]
    pub id: String,
    #[serde(rename = "Code")]
    pub code: String,
    #[serde(rename = "Message")]
    pub message: String,
    #[serde(rename = "SenderFault")]
    pub sender_fault: bool,
}

#[derive(Debug, Serialize)]
pub struct DeleteMessageBatchResponse {
    #[serde(rename = "DeleteMessageBatchResult")]
    pub delete_message_batch_result: DeleteMessageBatchResult,
}

#[derive(Debug, Serialize)]
pub struct DeleteMessageBatchResult {
    #[serde(rename = "DeleteMessageBatchResultEntry", default)]
    pub successful: Vec<DeleteMessageBatchResultEntry>,
    #[serde(rename = "BatchResultErrorEntry", default)]
    pub failed: Vec<BatchResultErrorEntry>,
}

#[derive(Debug, Serialize)]
pub struct DeleteMessageBatchResultEntry {
    #[serde(rename = "Id")]
    pub id: String,
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