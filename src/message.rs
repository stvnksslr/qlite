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

    pub fn with_sequence_number(mut self, sequence_number: i64) -> Self {
        self.sequence_number = Some(sequence_number);
        self
    }

    pub fn is_ready_for_delivery(&self) -> bool {
        match self.delay_until {
            Some(delay_until) => Utc::now() >= delay_until,
            None => true,
        }
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
    pub fn new(name: String) -> Self {
        let is_fifo = name.ends_with(".fifo");
        Self {
            name,
            created_at: Utc::now(),
            is_fifo,
            content_based_deduplication: false,
            visibility_timeout_seconds: 30,
            message_retention_period_seconds: 345600, // 4 days
            max_receive_count: None,
            dead_letter_target_arn: None,
        }
    }

    pub fn with_attributes(mut self, attributes: &HashMap<String, String>) -> Self {
        if let Some(value) = attributes.get("ContentBasedDeduplication") {
            self.content_based_deduplication = value == "true";
        }
        if let Some(value) = attributes.get("VisibilityTimeout") {
            if let Ok(timeout) = value.parse::<u32>() {
                self.visibility_timeout_seconds = timeout;
            }
        }
        if let Some(value) = attributes.get("MessageRetentionPeriod") {
            if let Ok(period) = value.parse::<u32>() {
                self.message_retention_period_seconds = period;
            }
        }
        if let Some(value) = attributes.get("MaxReceiveCount") {
            if let Ok(count) = value.parse::<u32>() {
                self.max_receive_count = Some(count);
            }
        }
        if let Some(value) = attributes.get("DeadLetterTargetArn") {
            self.dead_letter_target_arn = Some(value.clone());
        }
        self
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