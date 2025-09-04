use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub queue_name: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
    pub visibility_timeout: Option<DateTime<Utc>>,
    pub receive_count: u32,
    pub attributes: Option<HashMap<String, MessageAttributeValue>>,
    pub deduplication_id: Option<String>,
    pub delay_until: Option<DateTime<Utc>>,
    pub message_group_id: Option<String>,
    pub sequence_number: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageAttributeValue {
    #[serde(rename = "StringValue")]
    pub string_value: Option<String>,
    #[serde(rename = "BinaryValue")]
    pub binary_value: Option<String>,
    #[serde(rename = "DataType")]
    pub data_type: String,
}

impl Message {
    pub fn new(queue_name: String, body: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            queue_name,
            body,
            created_at: Utc::now(),
            visibility_timeout: None,
            receive_count: 0,
            attributes: None,
            deduplication_id: None,
            delay_until: None,
            message_group_id: None,
            sequence_number: None,
        }
    }

    pub fn with_attributes(mut self, attributes: HashMap<String, MessageAttributeValue>) -> Self {
        self.attributes = Some(attributes);
        self
    }

    pub fn with_deduplication_id(mut self, deduplication_id: String) -> Self {
        self.deduplication_id = Some(deduplication_id);
        self
    }

    pub fn with_delay_seconds(mut self, delay_seconds: u32) -> Self {
        if delay_seconds > 0 {
            self.delay_until = Some(Utc::now() + chrono::Duration::seconds(delay_seconds as i64));
        }
        self
    }

    pub fn with_message_group_id(mut self, message_group_id: String) -> Self {
        self.message_group_id = Some(message_group_id);
        self
    }


}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Queue {
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub is_fifo: bool,
    pub content_based_deduplication: bool,
    pub visibility_timeout_seconds: u32,
    pub message_retention_period_seconds: u32,
    pub max_receive_count: Option<u32>,
    pub dead_letter_target_arn: Option<String>,
}

impl Queue {

}

#[derive(Debug, Clone)]
pub struct ReceivedMessage {
    pub id: String,
    pub body: String,
    pub receipt_handle: String,
    pub attributes: Option<HashMap<String, MessageAttributeValue>>,
}

impl ReceivedMessage {
    pub fn new(id: String, body: String, attributes: Option<HashMap<String, MessageAttributeValue>>) -> Self {
        Self {
            receipt_handle: id.clone(),
            id,
            body,
            attributes,
        }
    }
}