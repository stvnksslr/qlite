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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Queue {
    pub name: String,
    pub created_at: DateTime<Utc>,
}

impl Queue {
    pub fn new(name: String) -> Self {
        Self {
            name,
            created_at: Utc::now(),
        }
    }
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